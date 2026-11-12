//! Turn a ratatui buffer into a PNG the way a terminal would: a monospace
//! font for text, box drawing and block elements drawn geometrically so
//! borders join and half blocks are exact.

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

pub struct Fonts {
    regular: FontArc,
    bold: FontArc,
    fallbacks: Vec<FontArc>,
    scale: PxScale,
    pub cell_w: u32,
    pub cell_h: u32,
    baseline: f32,
}

fn load(path: &str, index: u32) -> Option<FontArc> {
    let bytes = std::fs::read(path).ok()?;
    if path.ends_with(".ttc") {
        ab_glyph::FontVec::try_from_vec_and_index(bytes, index).ok().map(FontArc::new)
    } else {
        FontArc::try_from_vec(bytes).ok()
    }
}

impl Fonts {
    pub fn discover(px: f32) -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let regular_candidates = [
            std::env::var("NAHUALI_SHOT_FONT").unwrap_or_default(),
            format!("{home}/Library/Fonts/JetBrainsMono-Regular.ttf"),
            "/System/Library/Fonts/SFNSMono.ttf".into(),
            "/System/Library/Fonts/Menlo.ttc".into(),
        ];
        let bold_candidates = [
            std::env::var("NAHUALI_SHOT_FONT_BOLD").unwrap_or_default(),
            format!("{home}/Library/Fonts/JetBrainsMono-Bold.ttf"),
            "/System/Library/Fonts/Menlo.ttc".into(),
        ];
        let regular = regular_candidates
            .iter()
            .filter(|p| !p.is_empty())
            .find_map(|p| load(p, 0))
            .expect("a monospace font (set NAHUALI_SHOT_FONT)");
        let bold = bold_candidates
            .iter()
            .filter(|p| !p.is_empty())
            .find_map(|p| load(p, 1).or_else(|| load(p, 0)))
            .unwrap_or_else(|| regular.clone());
        let fallbacks = [
            "/System/Library/Fonts/Menlo.ttc",
            "/System/Library/Fonts/Apple Symbols.ttf",
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ]
        .iter()
        .filter_map(|p| load(p, 0))
        .collect();
        let scale = PxScale::from(px);
        let scaled = regular.as_scaled(scale);
        let advance = scaled.h_advance(regular.glyph_id('M'));
        let cell_w = advance.round() as u32;
        let cell_h = px.round() as u32;
        let ascent = scaled.ascent();
        let descent = scaled.descent();
        let baseline = (cell_h as f32 - (ascent - descent)) / 2.0 + ascent;
        Self { regular, bold, fallbacks, scale, cell_w, cell_h, baseline }
    }
}

pub fn xterm(i: u8) -> [u8; 3] {
    const BASE: [[u8; 3]; 16] = [
        [0, 0, 0],
        [205, 49, 49],
        [13, 188, 121],
        [229, 229, 16],
        [36, 114, 200],
        [188, 63, 188],
        [17, 168, 205],
        [229, 229, 229],
        [102, 102, 102],
        [241, 76, 76],
        [35, 209, 139],
        [245, 245, 67],
        [59, 142, 234],
        [214, 112, 214],
        [41, 184, 219],
        [255, 255, 255],
    ];
    match i {
        0..=15 => BASE[usize::from(i)],
        16..=231 => {
            let n = i - 16;
            let step = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            [step(n / 36), step((n / 6) % 6), step(n % 6)]
        }
        _ => {
            let g = 8 + (i - 232) * 10;
            [g, g, g]
        }
    }
}

fn rgb(color: Color, default: [u8; 3]) -> [u8; 3] {
    match color {
        Color::Rgb(r, g, b) => [r, g, b],
        Color::Indexed(i) => xterm(i),
        Color::Reset => default,
        Color::Black => xterm(0),
        Color::Red => xterm(1),
        Color::Green => xterm(2),
        Color::Yellow => xterm(3),
        Color::Blue => xterm(4),
        Color::Magenta => xterm(5),
        Color::Cyan => xterm(6),
        Color::Gray => xterm(7),
        Color::DarkGray => xterm(8),
        Color::LightRed => xterm(9),
        Color::LightGreen => xterm(10),
        Color::LightYellow => xterm(11),
        Color::LightBlue => xterm(12),
        Color::LightMagenta => xterm(13),
        Color::LightCyan => xterm(14),
        Color::White => xterm(15),
    }
}

fn blend(img: &mut RgbaImage, x: i64, y: i64, color: [u8; 3], alpha: f32) {
    if x < 0 || y < 0 || x >= i64::from(img.width()) || y >= i64::from(img.height()) {
        return;
    }
    let a = alpha.clamp(0.0, 1.0);
    let p = img.get_pixel_mut(x as u32, y as u32);
    for (channel, target) in p.0.iter_mut().zip(color) {
        *channel = (f32::from(*channel) * (1.0 - a) + f32::from(target) * a).round() as u8;
    }
    p.0[3] = 255;
}

fn rect(img: &mut RgbaImage, x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 3]) {
    // Anti-aliased axis-aligned rectangle with fractional edges.
    let (sx, ex) = (x0.floor() as i64, x1.ceil() as i64);
    let (sy, ey) = (y0.floor() as i64, y1.ceil() as i64);
    for y in sy..ey {
        let cover_y = (y1.min(y as f32 + 1.0) - y0.max(y as f32)).clamp(0.0, 1.0);
        for x in sx..ex {
            let cover_x = (x1.min(x as f32 + 1.0) - x0.max(x as f32)).clamp(0.0, 1.0);
            blend(img, x, y, color, cover_x * cover_y);
        }
    }
}

/// Distance from p to the segment ab.
fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) };
    let (qx, qy) = (a.0 + t * dx, a.1 + t * dy);
    ((p.0 - qx).powi(2) + (p.1 - qy).powi(2)).sqrt()
}

/// Rounded corner: arms from the cell centre towards two edges, joined by a
/// quarter circle. `dx`, `dy` are the arm directions (±1).
/// A cell rectangle in pixels.
#[derive(Clone, Copy)]
struct Cell {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

fn corner(img: &mut RgbaImage, cell: Cell, (dx, dy): (f32, f32), stroke: f32, color: [u8; 3]) {
    let Cell { x, y, w, h } = cell;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let r = (w / 2.0).min(h / 2.0);
    // Arc centre sits r inside the corner formed by the two arms.
    let (ax, ay) = (cx + dx * r, cy + dy * r);
    let arm_x_end = if dx > 0.0 { x + w } else { x };
    let arm_y_end = if dy > 0.0 { y + h } else { y };
    for py in (y.floor() as i64)..((y + h).ceil() as i64) {
        for px in (x.floor() as i64)..((x + w).ceil() as i64) {
            let p = (px as f32 + 0.5, py as f32 + 0.5);
            let d_h = seg_dist(p, (ax, cy), (arm_x_end, cy));
            let d_v = seg_dist(p, (cx, ay), (cx, arm_y_end));
            // Arc: points whose angle lies in the quadrant facing the corner.
            let (vx, vy) = (p.0 - ax, p.1 - ay);
            let in_quadrant = vx * dx <= 0.0 && vy * dy <= 0.0;
            let d_arc = if in_quadrant { ((vx * vx + vy * vy).sqrt() - r).abs() } else { f32::MAX };
            let d = d_h.min(d_v).min(d_arc);
            let alpha = stroke / 2.0 + 0.5 - d;
            if alpha > 0.0 {
                blend(img, px, py, color, alpha);
            }
        }
    }
}

/// Draw box-drawing and block characters geometrically. Returns false if
/// the symbol is not one of them.
fn geometric(img: &mut RgbaImage, symbol: &str, x: f32, y: f32, w: f32, h: f32, fg: [u8; 3]) -> bool {
    let light = 2.0;
    let heavy = 4.0;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let hline =
        |img: &mut RgbaImage, x0: f32, x1: f32, s: f32| rect(img, x0, cy - s / 2.0, x1, cy + s / 2.0, fg);
    let vline =
        |img: &mut RgbaImage, y0: f32, y1: f32, s: f32| rect(img, cx - s / 2.0, y0, cx + s / 2.0, y1, fg);
    match symbol {
        "─" => hline(img, x, x + w, light),
        "━" => hline(img, x, x + w, heavy),
        "│" => vline(img, y, y + h, light),
        "╭" => corner(img, Cell { x, y, w, h }, (1.0, 1.0), light, fg),
        "╮" => corner(img, Cell { x, y, w, h }, (-1.0, 1.0), light, fg),
        "╰" => corner(img, Cell { x, y, w, h }, (1.0, -1.0), light, fg),
        "╯" => corner(img, Cell { x, y, w, h }, (-1.0, -1.0), light, fg),
        "├" => {
            vline(img, y, y + h, light);
            hline(img, cx, x + w, light);
        }
        "┤" => {
            vline(img, y, y + h, light);
            hline(img, x, cx, light);
        }
        "█" => rect(img, x, y, x + w, y + h, fg),
        "▀" => rect(img, x, y, x + w, y + h / 2.0, fg),
        "▄" => rect(img, x, y + h / 2.0, x + w, y + h, fg),
        "▌" => rect(img, x, y, x + w / 2.0, y + h, fg),
        "▐" => rect(img, x + w / 2.0, y, x + w, y + h, fg),
        "▏" => rect(img, x, y, x + w / 8.0 + 1.0, y + h, fg),
        "▁" | "▂" | "▃" | "▅" | "▆" | "▇" => {
            let eighths = match symbol {
                "▁" => 1.0,
                "▂" => 2.0,
                "▃" => 3.0,
                "▅" => 5.0,
                "▆" => 6.0,
                _ => 7.0,
            };
            rect(img, x, y + h * (1.0 - eighths / 8.0), x + w, y + h, fg);
        }
        _ => return false,
    }
    true
}

fn glyph(fonts: &Fonts, img: &mut RgbaImage, symbol: &str, x: f32, y: f32, bold: bool, fg: [u8; 3]) {
    let Some(c) = symbol.chars().next() else {
        return;
    };
    let primary = if bold { &fonts.bold } else { &fonts.regular };
    let mut chosen = primary;
    if primary.glyph_id(c).0 == 0 {
        if let Some(fallback) = fonts.fallbacks.iter().find(|f| f.glyph_id(c).0 != 0) {
            chosen = fallback;
        } else {
            return;
        }
    }
    let scaled = chosen.as_scaled(fonts.scale);
    let id = chosen.glyph_id(c);
    let advance = scaled.h_advance(id);
    // Centre glyphs from fallback fonts whose advance differs from the grid.
    let offset = (fonts.cell_w as f32 * symbol_cells(symbol) - advance) / 2.0;
    let g = id.with_scale_and_position(fonts.scale, ab_glyph::point(x + offset.max(0.0), y + fonts.baseline));
    if let Some(outlined) = chosen.outline_glyph(g) {
        let bounds = outlined.px_bounds();
        outlined.draw(|gx, gy, coverage| {
            blend(
                img,
                bounds.min.x as i64 + i64::from(gx),
                bounds.min.y as i64 + i64::from(gy),
                fg,
                coverage,
            );
        });
    }
}

fn symbol_cells(symbol: &str) -> f32 {
    unicode_width::UnicodeWidthStr::width(symbol).max(1) as f32
}

/// Rasterise `buf`; Reset colours fall back to `default_fg` / `default_bg`.
pub fn rasterize(buf: &Buffer, fonts: &Fonts, default_fg: [u8; 3], default_bg: [u8; 3]) -> RgbaImage {
    let (cw, ch) = (fonts.cell_w, fonts.cell_h);
    let area = buf.area;
    let mut img = RgbaImage::from_pixel(
        u32::from(area.width) * cw,
        u32::from(area.height) * ch,
        Rgba([default_bg[0], default_bg[1], default_bg[2], 255]),
    );
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buf[(area.x + x, area.y + y)];
            let (px, py) = (f32::from(x) * cw as f32, f32::from(y) * ch as f32);
            let bg = rgb(cell.bg, default_bg);
            rect(&mut img, px, py, px + cw as f32, py + ch as f32, bg);
        }
    }
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buf[(area.x + x, area.y + y)];
            let symbol = cell.symbol();
            if symbol.is_empty() || symbol == " " {
                continue;
            }
            let (px, py) = (f32::from(x) * cw as f32, f32::from(y) * ch as f32);
            let fg = rgb(cell.fg, default_fg);
            if geometric(&mut img, symbol, px, py, cw as f32, ch as f32, fg) {
                continue;
            }
            let bold = cell.modifier.contains(Modifier::BOLD);
            glyph(fonts, &mut img, symbol, px, py, bold, fg);
        }
    }
    img
}
