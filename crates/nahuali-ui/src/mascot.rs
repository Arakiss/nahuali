//! The nahual — an axolotl spirit that watches over the `explore` cockpit and
//! mirrors the store's trust verdict back to the human supervisor.
//!
//! Every illustrated mascot comes from the checked-in spritesheet. Terminals
//! with Kitty, Sixel, or iTerm2 support receive the raster image directly;
//! terminals without a graphics protocol receive a proportional half-block
//! rendering generated from the same frame. This keeps the empty-state hero and
//! the animated corner mascot visually consistent instead of maintaining a
//! second hand-drawn character in source code.

use image::imageops::FilterType;
use image::{DynamicImage, Rgba};
use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};

use crate::theme::{self, Rgb};

/// The full nahual takes over the detail pane only when the pane can fit the
/// image, its verdict caption, and the explanatory hint without clipping.
pub const EMPTY_STATE_MIN_COLS: u16 = 34;
pub const EMPTY_STATE_MIN_ROWS: u16 = 21;

/// Target size for the empty-state mascot. With the half-block picker's 1:2
/// cell ratio, 28×15 cells matches the source frame's near-square proportions.
pub const EMPTY_WIDTH: u16 = 28;
pub const EMPTY_HEIGHT: u16 = 15;

/// Width of the always-visible corner mascot in terminal cells.
pub const MINI_WIDTH: u16 = 10;
/// Height of the always-visible corner mascot in terminal rows.
pub const MINI_HEIGHT: u16 = 5;
/// The footer gives the five-row image one extra row in which to breathe.
pub const MINI_SLOT_HEIGHT: u16 = MINI_HEIGHT + 1;

const SPRITESHEET: &[u8] = include_bytes!("../../../assets/nahuali-mascot-spritesheet.png");
const FRAME_COLS: u32 = 4;
const FRAME_ROWS: u32 = 2;
const FRAME_COUNT: usize = (FRAME_COLS * FRAME_ROWS) as usize;
/// Mostly rests on the bottom edge, rising briefly every few seconds.
const MOTION_SEQUENCE: [u16; 12] = [1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1];

const BODY: Rgb = Rgb(240, 205, 208);
const GILL: Rgb = Rgb(232, 128, 118);

/// The store trust verdict the mascot binds to. Derived from the cockpit's
/// trust label so this module stays decoupled from `nahuali-core`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Certify,
    Advisory,
    Warn,
    Block,
}

impl Verdict {
    /// Parse a verdict from a store-trust label such as `CERTIFY · trustworthy`.
    /// Unrecognized labels fall back to the neutral `Advisory` pose.
    pub fn from_label(label: &str) -> Self {
        let token = label
            .split([' ', '\u{00b7}', '\t'])
            .find(|s| !s.is_empty())
            .unwrap_or("");
        match token.to_ascii_uppercase().as_str() {
            "CERTIFY" => Self::Certify,
            "WARN" => Self::Warn,
            "BLOCK" => Self::Block,
            _ => Self::Advisory,
        }
    }

    /// The verdict accent color, matching the cockpit's severity coding.
    pub fn accent(self) -> Rgb {
        match self {
            Self::Certify => theme::GREEN,
            Self::Advisory => theme::BLUE,
            Self::Warn => theme::AMBER,
            Self::Block => theme::RED,
        }
    }

    fn body_factor(self) -> f32 {
        match self {
            Self::Certify => 1.05,
            Self::Block => 0.86,
            _ => 1.0,
        }
    }

    /// The one-line caption shown under the empty-state mascot.
    pub fn caption(self) -> &'static str {
        match self {
            Self::Certify => "calm · certified",
            Self::Advisory => "curious · advisory",
            Self::Warn => "alert · warn",
            Self::Block => "guarded · blocked",
        }
    }

    fn compact_expression(self) -> &'static str {
        match self {
            Self::Certify => "•ᴗ•",
            Self::Advisory => "•o•",
            Self::Warn => "•△•",
            Self::Block => "—_—",
        }
    }
}

/// A compact textual fallback for spaces that cannot host the raster mascot.
pub fn compact_mark(verdict: Verdict) -> Vec<Span<'static>> {
    let colors = theme::colors_enabled();
    let gill = colors.then(|| to_color(blend(GILL, verdict.accent(), 0.35)));
    let body = colors.then(|| to_color(scale(BODY, verdict.body_factor())));
    let expression = colors.then(|| to_color(verdict.accent()));

    let styled = |text: &'static str, fg: Option<Color>, bold: bool| {
        let mut style = Style::default();
        if let Some(fg) = fg {
            style = style.fg(fg);
        }
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        Span::styled(text, style)
    };

    vec![
        styled("≋", gill, true),
        styled("(", body, false),
        styled(verdict.compact_expression(), expression, true),
        styled(")", body, false),
        styled("≋", gill, true),
    ]
}

/// Both illustrated placements, prepared from one spritesheet frame and one
/// terminal capability query. The corner image is omitted for half-block-only
/// terminals because its 10×5 slot is too small to preserve the character.
pub struct MascotImages {
    empty: RasterMascot,
    corner: Option<RasterMascot>,
}

impl MascotImages {
    /// Prepare the best renderers supported by the current terminal.
    pub fn from_terminal(verdict: Verdict) -> Result<Self, String> {
        if std::env::var_os("NAHUALI_TUI_FORCE_HALF_BLOCKS").is_some() {
            return Self::halfblocks(verdict);
        }
        let mut picker = Picker::from_query_stdio().map_err(|error| error.to_string())?;
        picker.set_background_color(Some([0, 0, 0, 0]));
        Self::from_picker(&picker, verdict)
    }

    /// Prepare a deterministic no-query fallback. This path is also what VHS
    /// and tests use, so the README capture exercises the real TUI renderer.
    pub fn halfblocks(verdict: Verdict) -> Result<Self, String> {
        let frame = pose_image(verdict)?;
        Ok(Self {
            empty: RasterMascot::from_halfblocks(frame, Size::new(EMPTY_WIDTH, EMPTY_HEIGHT)),
            corner: None,
        })
    }

    fn from_picker(picker: &Picker, verdict: Verdict) -> Result<Self, String> {
        if matches!(picker.protocol_type(), ProtocolType::Halfblocks) {
            return Self::halfblocks(verdict);
        }
        let frame = pose_image(verdict)?;
        let empty =
            RasterMascot::from_image(picker, frame.clone(), Size::new(EMPTY_WIDTH, EMPTY_HEIGHT))?;
        let corner = RasterMascot::from_image(picker, frame, Size::new(MINI_WIDTH, MINI_HEIGHT))?;
        Ok(Self {
            empty,
            corner: Some(corner),
        })
    }

    pub fn empty(&self) -> &RasterMascot {
        &self.empty
    }

    pub fn corner(&self) -> Option<&RasterMascot> {
        self.corner.as_ref()
    }
}

/// A fixed-size mascot encoded for one terminal graphics protocol.
pub struct RasterMascot {
    protocol: MascotProtocol,
}

impl RasterMascot {
    fn from_image(picker: &Picker, image: DynamicImage, size: Size) -> Result<Self, String> {
        let protocol = picker
            .new_protocol(image, size, Resize::Fit(None))
            .map_err(|error| error.to_string())?;
        Ok(Self {
            protocol: MascotProtocol::Raster(protocol),
        })
    }

    fn from_halfblocks(image: DynamicImage, size: Size) -> Self {
        Self {
            protocol: MascotProtocol::Halfblocks(TransparentHalfblocks::new(image, size)),
        }
    }

    pub fn size(&self) -> Size {
        match &self.protocol {
            MascotProtocol::Raster(protocol) => protocol.size(),
            MascotProtocol::Halfblocks(protocol) => protocol.size,
        }
    }

    pub fn frame(&self) -> RasterMascotFrame<'_> {
        RasterMascotFrame { mascot: self }
    }
}

pub struct RasterMascotFrame<'a> {
    mascot: &'a RasterMascot,
}

impl Widget for RasterMascotFrame<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        match &self.mascot.protocol {
            MascotProtocol::Raster(protocol) => Image::new(protocol).render(area, buffer),
            MascotProtocol::Halfblocks(protocol) => protocol.render(area, buffer),
        }
    }
}

enum MascotProtocol {
    Raster(Protocol),
    Halfblocks(TransparentHalfblocks),
}

struct TransparentHalfblocks {
    size: Size,
    cells: Vec<Option<HalfblockCell>>,
}

struct HalfblockCell {
    symbol: &'static str,
    foreground: Color,
    background: Color,
}

impl TransparentHalfblocks {
    fn new(image: DynamicImage, size: Size) -> Self {
        let pixels = image
            .resize_exact(
                u32::from(size.width),
                u32::from(size.height) * 2,
                FilterType::Triangle,
            )
            .to_rgba8();
        let mut cells = Vec::with_capacity(usize::from(size.width * size.height));
        for y in 0..size.height {
            for x in 0..size.width {
                let top = pixels.get_pixel(u32::from(x), u32::from(y) * 2);
                let bottom = pixels.get_pixel(u32::from(x), u32::from(y) * 2 + 1);
                cells.push(halfblock_cell(*top, *bottom));
            }
        }
        Self { size, cells }
    }

    fn render(&self, area: Rect, buffer: &mut Buffer) {
        for (index, cell) in self.cells.iter().enumerate() {
            let x = index as u16 % self.size.width;
            let y = index as u16 / self.size.width;
            if x >= area.width || y >= area.height {
                continue;
            }
            let Some(cell) = cell else {
                continue;
            };
            if let Some(target) = buffer.cell_mut((area.x + x, area.y + y)) {
                target
                    .set_symbol(cell.symbol)
                    .set_fg(cell.foreground)
                    .set_bg(cell.background);
            }
        }
    }
}

fn halfblock_cell(top: Rgba<u8>, bottom: Rgba<u8>) -> Option<HalfblockCell> {
    const VISIBLE_ALPHA: u8 = 24;
    let top_visible = top.0[3] >= VISIBLE_ALPHA;
    let bottom_visible = bottom.0[3] >= VISIBLE_ALPHA;
    let rgb = |pixel: Rgba<u8>| {
        let alpha = f32::from(pixel.0[3]) / 255.0;
        let background = theme::BACKGROUND;
        Color::Rgb(
            lerp(background.0, pixel.0[0], alpha),
            lerp(background.1, pixel.0[1], alpha),
            lerp(background.2, pixel.0[2], alpha),
        )
    };
    match (top_visible, bottom_visible) {
        (false, false) => None,
        (true, false) => Some(HalfblockCell {
            symbol: "▀",
            foreground: rgb(top),
            background: Color::Reset,
        }),
        (false, true) => Some(HalfblockCell {
            symbol: "▄",
            foreground: rgb(bottom),
            background: Color::Reset,
        }),
        (true, true) => Some(HalfblockCell {
            symbol: "▀",
            foreground: rgb(top),
            background: rgb(bottom),
        }),
    }
}

fn pose_image(verdict: Verdict) -> Result<DynamicImage, String> {
    let sheet = image::load_from_memory(SPRITESHEET).map_err(|error| error.to_string())?;
    split_frames(&sheet)
        .into_iter()
        .nth(pose_frame(verdict))
        .ok_or_else(|| "mascot spritesheet is incomplete".to_string())
}

fn split_frames(sheet: &DynamicImage) -> Vec<DynamicImage> {
    let mut frames = Vec::with_capacity(FRAME_COUNT);
    for row in 0..FRAME_ROWS {
        let top = row * sheet.height() / FRAME_ROWS;
        let bottom = (row + 1) * sheet.height() / FRAME_ROWS;
        for column in 0..FRAME_COLS {
            let left = column * sheet.width() / FRAME_COLS;
            let right = (column + 1) * sheet.width() / FRAME_COLS;
            frames.push(sheet.crop_imm(left, top, right - left, bottom - top));
        }
    }
    frames
}

fn pose_frame(verdict: Verdict) -> usize {
    match verdict {
        Verdict::Certify | Verdict::Advisory => 0,
        Verdict::Warn | Verdict::Block => 4,
    }
}

pub(crate) fn motion_offset(tick: usize) -> u16 {
    MOTION_SEQUENCE[tick % MOTION_SEQUENCE.len()]
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let a = f32::from(a);
    let b = f32::from(b);
    (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    Rgb(lerp(a.0, b.0, t), lerp(a.1, b.1, t), lerp(a.2, b.2, t))
}

fn scale(a: Rgb, factor: f32) -> Rgb {
    let channel = |value: u8| (f32::from(value) * factor).round().clamp(0.0, 255.0) as u8;
    Rgb(channel(a.0), channel(a.1), channel(a.2))
}

fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.0, rgb.1, rgb.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(mascot: &RasterMascot) -> Buffer {
        let size = mascot.size();
        let area = Rect::new(0, 0, size.width, size.height);
        let mut buffer = Buffer::empty(area);
        mascot.frame().render(area, &mut buffer);
        buffer
    }

    fn signature(buffer: &Buffer) -> Vec<(String, Color, Color)> {
        buffer
            .content()
            .iter()
            .map(|cell| (cell.symbol().to_string(), cell.fg, cell.bg))
            .collect()
    }

    #[test]
    fn verdict_parses_from_store_labels() {
        assert_eq!(
            Verdict::from_label("CERTIFY · trustworthy"),
            Verdict::Certify
        );
        assert_eq!(
            Verdict::from_label("WARN · verify before use"),
            Verdict::Warn
        );
        assert_eq!(
            Verdict::from_label("BLOCK · not yet trustworthy"),
            Verdict::Block
        );
        assert_eq!(Verdict::from_label(""), Verdict::Advisory);
        assert_eq!(Verdict::from_label("mystery"), Verdict::Advisory);
    }

    #[test]
    fn compact_mark_tracks_every_verdict() {
        let text = |verdict| {
            compact_mark(verdict)
                .into_iter()
                .map(|span| span.content.into_owned())
                .collect::<String>()
        };
        assert_eq!(text(Verdict::Certify), "≋(•ᴗ•)≋");
        assert_eq!(text(Verdict::Advisory), "≋(•o•)≋");
        assert_eq!(text(Verdict::Warn), "≋(•△•)≋");
        assert_eq!(text(Verdict::Block), "≋(—_—)≋");
    }

    #[test]
    fn spritesheet_decodes_into_eight_transparent_frames() {
        let sheet = image::load_from_memory(SPRITESHEET).expect("embedded PNG must decode");
        let frames = split_frames(&sheet);
        assert_eq!(frames.len(), FRAME_COUNT);
        assert!(frames.iter().all(|frame| frame.width() >= 429));
        assert!(frames.iter().all(|frame| frame.height() == 458));
        assert!(
            frames
                .iter()
                .all(|frame| { frame.to_rgba8().pixels().any(|pixel| pixel.0[3] == 0) })
        );
    }

    #[test]
    fn empty_halfblocks_preserve_source_aspect_ratio() {
        let images = MascotImages::halfblocks(Verdict::Block).unwrap();
        let size = images.empty().size();
        let source_ratio = 429.0 / 458.0;
        let rendered_ratio = f32::from(size.width) / (f32::from(size.height) * 2.0);
        assert!((source_ratio - rendered_ratio).abs() < 0.08);
        assert!(images.corner().is_none());
    }

    #[test]
    fn empty_halfblocks_are_generated_from_the_verdict_frames() {
        let calm = MascotImages::halfblocks(Verdict::Certify).unwrap();
        let guarded = MascotImages::halfblocks(Verdict::Block).unwrap();
        let calm_buffer = render(calm.empty());
        let guarded_buffer = render(guarded.empty());
        assert_ne!(signature(&calm_buffer), signature(&guarded_buffer));
        assert!(
            calm_buffer
                .content()
                .iter()
                .any(|cell| { matches!(cell.symbol(), "▀" | "▄" | "█") })
        );
        assert!(calm_buffer.content().iter().any(|cell| {
            matches!(cell.fg, Color::Rgb(red, green, blue) if red > 80 && green > 40 && blue > 40)
        }));
        let corner = &calm_buffer[(0, 0)];
        assert_eq!(corner.symbol(), " ");
        assert_eq!(corner.bg, Color::Reset);
    }

    #[test]
    fn motion_stays_subtle() {
        let offsets: Vec<_> = (0..MOTION_SEQUENCE.len()).map(motion_offset).collect();
        assert!(offsets.iter().all(|offset| *offset <= 1));
        assert!(offsets.iter().filter(|offset| **offset == 0).count() <= 2);
    }
}
