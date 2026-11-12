//! Render every screen to PNG without a terminal, for design review.
//!
//! ```text
//! cargo run -p nahuali-tui --release --example screenshots [-- OUT_DIR]
//! ```
//!
//! Each screen is drawn into a ratatui buffer at 120×36 and 80×24 exactly as
//! the TUI draws it, then rasterised with a monospace font. Two variants are
//! written where the axolotl appears: the half-block fallback as drawn, and
//! the image-protocol look (Kitty / iTerm2), where the real sprite frame is
//! composited into the cells the protocol would cover.

mod raster;

use std::path::PathBuf;

use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};
use nahuali_tui::App;
use nahuali_tui::mascot::Sprites;
use nahuali_tui::theme::{ColorMode, Theme, raw};
use nahuali_view::mock::MockSource;
use nahuali_view::{UiState, View};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

struct Shot {
    name: &'static str,
    view: View,
    alarm: bool,
    query: &'static str,
    help: bool,
}

const SHOTS: &[Shot] = &[
    Shot { name: "today", view: View::Today, alarm: false, query: "", help: false },
    Shot { name: "journal", view: View::Journal, alarm: false, query: "", help: false },
    Shot { name: "intentions", view: View::Intentions, alarm: false, query: "", help: false },
    Shot { name: "health", view: View::Health, alarm: false, query: "", help: false },
    Shot { name: "health-alarm", view: View::Health, alarm: true, query: "", help: false },
    Shot {
        name: "search",
        view: View::Search,
        alarm: false,
        query: "why did we keep the store local",
        help: false,
    },
    Shot { name: "today-help", view: View::Today, alarm: false, query: "", help: true },
];

fn to3(rgb: raw_rgb::Rgb) -> [u8; 3] {
    [rgb.0, rgb.1, rgb.2]
}

mod raw_rgb {
    pub use nahuali_tui::theme::Rgb;
}

/// Cells of `rect` still holding Kitty placeholders (nothing drew over them).
fn sprite_cells(buf: &Buffer, rect: Rect) -> Vec<(u16, u16)> {
    let mut cells = Vec::new();
    for y in rect.top()..rect.bottom() {
        for x in rect.left()..rect.right() {
            let symbol = buf[(x, y)].symbol();
            if symbol.contains('\u{10EEEE}') {
                cells.push((x, y));
            }
        }
    }
    cells
}

fn composite(
    img: &mut RgbaImage,
    sprite: &RgbaImage,
    rect: Rect,
    cells: &[(u16, u16)],
    cw: u32,
    ch: u32,
    bg: [u8; 3],
) {
    let inside = |px: u32, py: u32| cells.contains(&((px / cw) as u16, (py / ch) as u16));
    let (x0, y0) = (u32::from(rect.x) * cw, u32::from(rect.y) * ch);
    let (w, h) = (u32::from(rect.width) * cw, u32::from(rect.height) * ch);
    // Clear the half-block cells the protocol would replace.
    for y in y0..(y0 + h).min(img.height()) {
        for x in x0..(x0 + w).min(img.width()) {
            if inside(x, y) {
                img.put_pixel(x, y, Rgba([bg[0], bg[1], bg[2], 255]));
            }
        }
    }
    let scale = (w as f32 / sprite.width() as f32).min(h as f32 / sprite.height() as f32);
    let (sw, sh) =
        ((sprite.width() as f32 * scale).round() as u32, (sprite.height() as f32 * scale).round() as u32);
    let resized = imageops::resize(sprite, sw.max(1), sh.max(1), FilterType::Lanczos3);
    let (ox, oy) = (x0 + (w - sw) / 2, y0 + (h - sh) / 2);
    for (x, y, p) in resized.enumerate_pixels() {
        let (tx, ty) = (ox + x, oy + y);
        if tx >= img.width() || ty >= img.height() || !inside(tx, ty) {
            continue;
        }
        let a = f32::from(p.0[3]) / 255.0;
        let dst = img.get_pixel_mut(tx, ty);
        for (channel, source) in dst.0.iter_mut().zip(p.0).take(3) {
            *channel = (f32::from(*channel) * (1.0 - a) + f32::from(source) * a).round() as u8;
        }
    }
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/screenshots/tui")));
    std::fs::create_dir_all(&out).expect("create output dir");
    let fonts = raster::Fonts::discover(30.0);
    let theme = Theme::new(ColorMode::TrueColor);
    let ink = to3(raw::INK);
    let bg = to3(raw::BG);
    for shot in SHOTS {
        for (w, h) in [(120u16, 36u16), (80, 24)] {
            let src = MockSource::new();
            src.set_chain_broken(shot.alarm);
            let mut ui = UiState::new(false);
            ui.view = shot.view;
            ui.query = shot.query.to_string();
            ui.searching = !shot.query.is_empty();
            let draw = |sprites: Sprites| {
                let mut app = App::new(&src, ui.clone(), sprites, theme);
                app.help = shot.help;
                let target = app.mascot_target();
                app.sprites.force(target, 0);
                let area = Rect::new(0, 0, w, h);
                let mut buf = Buffer::empty(area);
                app.render(area, &mut buf);
                let placements = app.sprites.placements().to_vec();
                let images: Vec<RgbaImage> =
                    placements.iter().map(|p| app.sprites.image(p.state, p.frame).clone()).collect();
                (buf, placements, images)
            };
            let base = format!("{}-{w}x{h}", shot.name);
            // Half blocks, exactly as drawn.
            let (buf, placements, _) = draw(Sprites::for_tests(ColorMode::TrueColor, theme.bg()));
            let half = raster::rasterize(&buf, &fonts, ink, bg);
            if placements.is_empty() {
                half.save(out.join(format!("{base}.png"))).expect("save png");
                println!("{base}.png");
                continue;
            }
            half.save(out.join(format!("{base}-halfblocks.png"))).expect("save png");
            // Kitty: the real protocol writes placeholders into the buffer;
            // the sprite shows wherever placeholders survived the frame.
            let (buf, placements, images) = draw(Sprites::kitty_for_tests(ColorMode::TrueColor, theme.bg()));
            let mut image = raster::rasterize(&buf, &fonts, ink, bg);
            for (p, sprite) in placements.iter().zip(&images) {
                let cells = sprite_cells(&buf, p.rect);
                composite(&mut image, sprite, p.rect, &cells, fonts.cell_w, fonts.cell_h, bg);
            }
            image.save(out.join(format!("{base}.png"))).expect("save png");
            println!("{base}.png (kitty) + halfblocks");
        }
    }
    // The 256-colour fallback (Apple Terminal): theme and sprite both go
    // through their xterm-256 stand-ins.
    let theme256 = Theme::new(ColorMode::Ansi256);
    for (w, h) in [(120u16, 36u16), (80, 24)] {
        let src = MockSource::new();
        let sprites = Sprites::for_tests(ColorMode::Ansi256, theme256.bg());
        let mut app = App::new(&src, UiState::new(false), sprites, theme256);
        let target = app.mascot_target();
        app.sprites.force(target, 0);
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        app.render(area, &mut buf);
        let image = raster::rasterize(&buf, &fonts, ink, bg);
        let name = format!("today-{w}x{h}-256color.png");
        image.save(out.join(&name)).expect("save png");
        println!("{name}");
    }
}
