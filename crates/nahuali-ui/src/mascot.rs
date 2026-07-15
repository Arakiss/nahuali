//! The nahual — an ajolote (axolotl) spirit that watches over the `explore`
//! cockpit and mirrors the store's trust verdict back to the human supervisor.
//!
//! The ajolote is on-brand: Nahuali already speaks in Mictlán/Tonalli terms, and
//! the axolotl is the creature of that world. It is drawn as compact pixel art —
//! each terminal row packs two pixel rows via the Unicode upper-half block
//! (`▀`), painting the top pixel in the cell's foreground and the bottom pixel
//! in its background — so a full-body 28×~30 sprite (head, gill fans, belly,
//! hands, feet, dark outline) fits in ~15 rows with no external assets or image
//! crates. The palette is a leucistic axolotl (pale rose body, coral gills)
//! harmonized with the clay-on-coffee identity in [`crate::theme`].
//!
//! Four poses bind to the live store verdict: CERTIFY is serene and luminous,
//! ADVISORY curious with perked gills, WARN alert with its gills flared upward,
//! and BLOCK guarded with its gills tucked protectively down the body. Each
//! carries a one-line caption. On non-truecolor terminals (or `NO_COLOR`) the
//! sprite renders as a single-color silhouette so the shape still reads.
//!
//! The nahual has no column of its own — a dedicated panel read as too loud. It
//! lives in two subtle places instead: full-size in the detail pane's empty
//! state (with its caption and a dim hint) when there is no item to show, and
//! always-on as a compact gill-and-face mark in the header border. The mark puts
//! the recognisable gill fans first, so it survives title clipping on narrow
//! terminals without taking a row from the operator. The cockpit host
//! ([`crate::tui`]) owns placement; this module owns the art, palette, and
//! [`Verdict`] binding, and exposes [`compact_mark`], [`Verdict::accent`], and
//! [`Mascot::block_height`] for composition.
//!
//! No animation: the cockpit's event loop blocks on `event::read`, so there is
//! no tick to drive one — the mascot is deliberately still.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

use crate::theme::{self, Rgb};

/// The full nahual takes over the detail pane's empty state only when that pane
/// is at least this wide; narrower, the host draws the plain placeholder line so
/// the sprite (28 columns) is never clipped or centered awkwardly.
pub const EMPTY_STATE_MIN_COLS: u16 = 34;
/// …and at least this tall. Sized for the tallest pose: the 16-cell WARN sprite
/// is an 18-row block ([`Mascot::block_height`]), plus one hint line, needs 19
/// inner rows, i.e. a 21-row pane once the detail border is subtracted.
pub const EMPTY_STATE_MIN_ROWS: u16 = 21;

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
    /// Parse a verdict from a store-trust label such as `"CERTIFY · trustworthy"`.
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

    /// The verdict accent color, matching the cockpit's severity coding. Public
    /// so the header cue can tint its pose caption without duplicating the map.
    pub fn accent(self) -> Rgb {
        match self {
            Self::Certify => theme::GREEN,
            Self::Advisory => theme::BLUE,
            Self::Warn => theme::AMBER,
            Self::Block => theme::RED,
        }
    }

    /// A brightness factor on the body: CERTIFY glows, BLOCK dims and guards.
    fn body_factor(self) -> f32 {
        match self {
            Self::Certify => 1.05,
            Self::Block => 0.86,
            _ => 1.0,
        }
    }

    /// The one-line caption shown under the sprite.
    pub fn caption(self) -> &'static str {
        match self {
            Self::Certify => "calm · certified",
            Self::Advisory => "curious · advisory",
            Self::Warn => "alert · warn",
            Self::Block => "guarded · blocked",
        }
    }

    /// The expression used by the compact always-visible mark. Gill fans and
    /// head shape remain stable while the mouth/eyes mirror the trust verdict.
    fn compact_expression(self) -> &'static str {
        match self {
            Self::Certify => "•ᴗ•",
            Self::Advisory => "•o•",
            Self::Warn => "•△•",
            Self::Block => "—_—",
        }
    }

    /// The pixel-art rows for this pose. Each byte is a palette key (see
    /// [`pixel_rgb`]); rows may be ragged — missing trailing cells are
    /// transparent. ASCII-only, so byte indexing equals column indexing.
    fn sprite(self) -> &'static [&'static str] {
        match self {
            Self::Certify => &CERTIFY,
            Self::Advisory => &ADVISORY,
            Self::Warn => &WARN,
            Self::Block => &BLOCK,
        }
    }
}

// ---- Pixel-art poses -------------------------------------------------------
// Legend: '.'/' ' transparent · 'k' outline · 'o' body · 'l' body light ·
// 'd' body shade · 'w' belly · 'e' eye · 'x' eye sparkle · 'b' blush ·
// 'm' mouth · 'O' open mouth · 'g' gill frond · 'h' gill light · 'q' gill
// base · 'G' gill tip (verdict-tinted) · '*' spark (verdict accent).

/// CERTIFY — serene: relaxed fan of gills, happy closed eyes, a wide smile,
/// blush, and two aura sparks. Luminous.
const CERTIFY: [&str; 29] = [
    "                         *",
    "  G*         kkk        ***G",
    "  ***    kkkkklkkkkk     *h",
    "   *g  kkklllllllllkkk   gg",
    "    ggkklllllllllllllkk gg",
    "     qqllllllooollllllkqq",
    "     kkoooooooooooooookk",
    "hg   koooooooooooooooook   g",
    "hggq koooeeeoooooeeeoook qgg",
    "  gqqkoooeoeoooooeoeoookqqg",
    "    qkoooooooooooooooookq",
    "     kobbooooooooooobbok",
    " GhhqqkooooomooomoooookqqhhG",
    " GhhqqkddddddmmmddddddkqqhhG",
    "      kkdddddddddddddkk",
    "       kkkdddddddddkkk",
    "        kkdddddddddkk",
    "        kdddddddddddk",
    "       kkdddddwdddddkk",
    "       kddddwwwwwddddk",
    "     kkkdddwwwwwwwdddkkk",
    "     kdddddwwwwwwwdddddk",
    "     kkdkddwwwwwwwddkdkk",
    "      kkkdddwwwwwdddkkk",
    "        kkddddwddddkk",
    "        kkkkdddddkkkk",
    "        kddddkkkddddk",
    "        kkddkk kkddkk",
    "         kkkk   kkkk",
];
/// ADVISORY — curious: perked fan, big sparkling eyes, a small open mouth,
/// blush, one questioning spark.
const ADVISORY: [&str; 29] = [
    "   GG                    *G",
    "   hh        kkk        ***",
    "   hh    kkkkklkkkkk     *h",
    "    gg kkklllllllllkkk  gg",
    "     qqklllllllllllllkkqq",
    "     qqllllllooollllllkqq",
    "h    kkoooooooooooooookk",
    "hhg  koooooooooooooooook  gh",
    " hgq koooxeeoooooxeeoook qgh",
    "  gqqkoooeeeoooooeeeoookqqg",
    "    qkoooeeeoooooeeeoookq",
    "     kobbooooooooooobbok",
    " GhhqqkoooooooOOooooookqqhhG",
    " GhhqqkdddddddOOddddddkqqhhG",
    "      kkdddddddddddddkk",
    "       kkkdddddddddkkk",
    "        kkdddddddddkk",
    "        kdddddddddddk",
    "       kkdddddwdddddkk",
    "       kddddwwwwwddddk",
    "     kkkdddwwwwwwwdddkkk",
    "     kdddddwwwwwwwdddddk",
    "     kkdkddwwwwwwwddkdkk",
    "      kkkdddwwwwwdddkkk",
    "        kkddddwddddkk",
    "        kkkkdddddkkkk",
    "        kddddkkkddddk",
    "        kkddkk kkddkk",
    "         kkkk   kkkk",
];
/// WARN — alert: gills flared high, raised brows, wide eyes, open mouth. Tense.
const WARN: [&str; 31] = [
    "    GG                  GG",
    "    hh                  hh",
    "    hh                  hh",
    "     gg      kkk       gg",
    "     gg  kkkkklkkkkk   gg",
    "GG   qqkkklllllllllkkk qq",
    "hhh  qqklllllllllllllkkqq  h",
    "  gg  qqlllllooollllllqq  gg",
    "  gg kkookkkoooookkkookk  gg",
    "   qqkoooooooooooooooook qq",
    "    qqoooexeoooooexeoookqq",
    "     koooeeeoooooeeeoook",
    " Gh  koooeeeoooooeeeoook  hG",
    " GhgqkoooooooooooooooookqghG",
    "   gqqkooooooOOOooooookqqg",
    "     qkddddddOOOddddddkq",
    "      kkddddddOddddddkk",
    "       kkkdddddddddkkk",
    "        kkdddddddddkk",
    "        kdddddddddddk",
    "       kkdddddwdddddkk",
    "       kddddwwwwwddddk",
    "     kkkdddwwwwwwwdddkkk",
    "     kdddddwwwwwwwdddddk",
    "     kkdkddwwwwwwwddkdkk",
    "      kkkdddwwwwwdddkkk",
    "        kkddddwddddkk",
    "        kkkkdddddkkkk",
    "        kddddkkkddddk",
    "        kkddkk kkddkk",
    "         kkkk   kkkk",
];
/// BLOCK — guarded: hunched, gills drooped and tucked against the body,
/// eyes closed flat, a set mouth. Withdrawn.
const BLOCK: [&str; 27] = [
    "             kkk",
    "         kkkkklkkkkk",
    "       kkklllllllllkkk",
    "      kklllllllllllllkk",
    "      kllllllooollllllk",
    "     kkoooooooooooooookk",
    "     koooooooooooooooook",
    "    qkoooooooooooooooookq",
    " hgqqkoooeeeoooooeeeoookqqgh",
    "GGhqqkoooooooooooooooookqqhG",
    "GGhhqkoooooooooooooooookqhhG",
    "  hhqqkooooommmmmoooookqqhh",
    "     qkdddddddddddddddkq",
    "      kkdddddddddddddkk",
    "       kkkdddddddddkkk",
    "         kkdddddddkk",
    "        kkdddddddddkk",
    "       kkdddddddddddkk",
    "       kddddwwwwwddddk",
    "     kkkdddwwwwwwwdddkkk",
    "     kdddddwwwwwwwdddddk",
    "     kkdkddwwwwwwwddkdkk",
    "      kkkkddwwwwwddkkkk",
    "        kkkdddddddkkk",
    "        kddddkdkddddk",
    "        kkddkkkkkddkk",
    "         kkkk   kkkk",
];

// ---- Palette ---------------------------------------------------------------

/// Leucistic axolotl base colors, harmonized with the clay-on-coffee palette.
const OUTLINE: Rgb = Rgb(62, 44, 48); // dark warm silhouette line
const BODY: Rgb = Rgb(240, 205, 208); // pale rose flesh
const BODY_LIGHT: Rgb = Rgb(250, 228, 230); // lit crown
const BODY_SHADE: Rgb = Rgb(210, 165, 175); // chin / underside
const BELLY: Rgb = Rgb(252, 240, 240); // belly patch
const EYE: Rgb = Rgb(45, 32, 36); // warm near-black
const EYE_SPARK: Rgb = Rgb(255, 250, 250); // catchlight
const BLUSH: Rgb = Rgb(246, 158, 158); // cheek spots
const MOUTH: Rgb = Rgb(196, 110, 112); // rose smile
const MOUTH_OPEN: Rgb = Rgb(150, 74, 80); // deeper rose (open)
const GILL: Rgb = Rgb(232, 128, 118); // coral frond
const GILL_LIGHT: Rgb = Rgb(246, 172, 160); // frond towards the tip
const GILL_DARK: Rgb = Rgb(196, 96, 92); // frond root

/// A seven-column axolotl mark for the cockpit header: coral gill fans around a
/// pale head, with a verdict-tinted expression. Unlike the old pair of colored
/// half blocks, the silhouette is readable without its caption and remains
/// recognizable in monochrome. It deliberately begins with the left gill so a
/// very narrow title still carries the brand mark before trailing text clips.
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

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let a = f32::from(a);
    let b = f32::from(b);
    (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

/// Blend `a` toward `b` by `t` in `0.0..=1.0`.
fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    Rgb(lerp(a.0, b.0, t), lerp(a.1, b.1, t), lerp(a.2, b.2, t))
}

/// Scale each channel of `a` by `f`, clamped to a valid color.
fn scale(a: Rgb, f: f32) -> Rgb {
    let c = |v: u8| (f32::from(v) * f).round().clamp(0.0, 255.0) as u8;
    Rgb(c(a.0), c(a.1), c(a.2))
}

/// Resolve a palette key to a truecolor pixel, or `None` if transparent.
/// The gills stay recognizably coral — only their bright tips (and the sparks)
/// take the full verdict accent, so the crown signals the store's mood without
/// muddying the pink; the body brightens (CERTIFY) or dims (BLOCK) with it.
fn pixel_rgb(key: u8, verdict: Verdict) -> Option<Rgb> {
    let accent = verdict.accent();
    let factor = verdict.body_factor();
    let rgb = match key {
        b'.' | b' ' => return None,
        b'k' => OUTLINE,
        b'o' => scale(BODY, factor),
        b'l' => scale(BODY_LIGHT, factor),
        b'd' => scale(BODY_SHADE, factor),
        b'w' => scale(BELLY, factor),
        b'e' => EYE,
        b'x' => EYE_SPARK,
        b'b' => BLUSH,
        b'm' => MOUTH,
        b'O' => MOUTH_OPEN,
        b'g' => GILL,
        b'h' => GILL_LIGHT,
        b'q' => GILL_DARK,
        b'G' => blend(GILL_LIGHT, accent, 0.55),
        b'*' => accent,
        _ => return None,
    };
    Some(rgb)
}

fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.0, rgb.1, rgb.2)
}

/// Is a palette key drawn (non-transparent)?
fn is_ink(key: u8) -> bool {
    !matches!(key, b'.' | b' ')
}

// ---- Widget ----------------------------------------------------------------

/// The nahual mascot as a `ratatui` widget. Paints its pixel-art pose centered
/// in the given area with a caption beneath it.
#[derive(Clone, Copy)]
pub struct Mascot {
    verdict: Verdict,
    /// Render as a single-color silhouette (non-truecolor / `NO_COLOR`).
    mono: bool,
}

impl Mascot {
    /// Build a mascot for `verdict`, choosing truecolor vs. monochrome from the
    /// terminal's advertised capabilities.
    pub fn new(verdict: Verdict) -> Self {
        let mono = !(theme::colors_enabled() && theme::truecolor_enabled());
        Self { verdict, mono }
    }

    /// Force the color mode (used by tests to exercise both paths).
    #[cfg(test)]
    fn with_mono(verdict: Verdict, mono: bool) -> Self {
        Self { verdict, mono }
    }

    /// The height in terminal rows this mascot draws: the half-block sprite plus
    /// a blank line and the caption (this is the `block_h` [`render`](Mascot::render)
    /// centers). The empty-state host uses it to place a hint line directly under
    /// the caption and to size the render area so the caption is never clipped.
    pub fn block_height(self) -> u16 {
        let sprite = self.verdict.sprite();
        let sprite_h = sprite.len().div_ceil(2) as u16; // 2 pixel rows per cell
        sprite_h + 2 // sprite + blank line + caption
    }
}

impl Widget for Mascot {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let sprite = self.verdict.sprite();
        let sprite_w = sprite.iter().map(|r| r.len()).max().unwrap_or(0) as u16;
        let sprite_h = sprite.len().div_ceil(2) as u16; // 2 pixel rows per cell
        let block_h = sprite_h + 2; // sprite + blank line + caption

        // Center the sprite within the area (it "breathes" in its panel).
        let ox = area.x + area.width.saturating_sub(sprite_w) / 2;
        let oy = area.y + area.height.saturating_sub(block_h) / 2;

        let mono_fg = Color::Reset;
        for (cy, pair) in sprite.chunks(2).enumerate() {
            let y = oy + cy as u16;
            if y >= area.bottom() {
                break;
            }
            let top = pair[0].as_bytes();
            let bot = pair.get(1).map_or(&b""[..], |r| r.as_bytes());
            for x in 0..sprite_w {
                let cx = ox + x;
                if cx >= area.right() {
                    break;
                }
                let tk = top.get(x as usize).copied().unwrap_or(b'.');
                let bk = bot.get(x as usize).copied().unwrap_or(b'.');
                let (top_set, bot_set) = (is_ink(tk), is_ink(bk));
                if !top_set && !bot_set {
                    continue;
                }
                let Some(cell) = buf.cell_mut((cx, y)) else {
                    continue;
                };
                if self.mono {
                    let sym = match (top_set, bot_set) {
                        (true, true) => "\u{2588}",  // █
                        (true, false) => "\u{2580}", // ▀
                        _ => "\u{2584}",             // ▄
                    };
                    cell.set_symbol(sym).set_fg(mono_fg);
                    continue;
                }
                let top_rgb = pixel_rgb(tk, self.verdict).unwrap_or(BODY);
                let bot_rgb = pixel_rgb(bk, self.verdict).unwrap_or(BODY);
                match (top_set, bot_set) {
                    (true, true) => {
                        cell.set_symbol("\u{2580}");
                        cell.set_fg(to_color(top_rgb));
                        cell.set_bg(to_color(bot_rgb));
                    }
                    (true, false) => {
                        cell.set_symbol("\u{2580}");
                        cell.set_fg(to_color(top_rgb));
                    }
                    _ => {
                        cell.set_symbol("\u{2584}");
                        cell.set_fg(to_color(bot_rgb));
                    }
                }
            }
        }

        // Caption, centered under the sprite.
        let caption = self.verdict.caption();
        let cap_y = oy + sprite_h + 1;
        if cap_y < area.bottom() {
            let cap_w = caption.len() as u16;
            let cap_x = area.x + area.width.saturating_sub(cap_w) / 2;
            let fg = if self.mono {
                mono_fg
            } else {
                to_color(self.verdict.accent())
            };
            buf.set_string(
                cap_x,
                cap_y,
                caption,
                ratatui::style::Style::default().fg(fg),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(mascot: Mascot, w: u16, h: u16) -> Buffer {
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        mascot.render(area, &mut buf);
        buf
    }

    /// The bytes of every cell symbol concatenated — a crude snapshot signature.
    fn signature(buf: &Buffer) -> Vec<String> {
        buf.content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect()
    }

    fn painted_cells(buf: &Buffer) -> usize {
        buf.content()
            .iter()
            .filter(|c| c.symbol() != " " && !c.symbol().is_empty())
            .count()
    }

    #[test]
    fn verdict_parses_from_store_labels() {
        assert_eq!(
            Verdict::from_label("CERTIFY · trustworthy"),
            Verdict::Certify
        );
        assert_eq!(
            Verdict::from_label("ADVISORY · use with judgment"),
            Verdict::Advisory
        );
        assert_eq!(
            Verdict::from_label("WARN · verify before use"),
            Verdict::Warn
        );
        assert_eq!(
            Verdict::from_label("BLOCK · not yet trustworthy"),
            Verdict::Block
        );
        // Unknown label falls back to the neutral pose, never panics.
        assert_eq!(Verdict::from_label(""), Verdict::Advisory);
        assert_eq!(Verdict::from_label("mystery"), Verdict::Advisory);
    }

    #[test]
    fn compact_mark_keeps_the_axolotl_shape_and_tracks_every_verdict() {
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
    fn certify_renders_a_painted_sprite_and_caption() {
        let buf = render(Mascot::with_mono(Verdict::Certify, false), 34, 20);
        // The sprite paints a substantial number of cells…
        assert!(painted_cells(&buf) > 40, "sprite should be dense");
        // …and the caption's marker word is present in the buffer.
        let text: String = signature(&buf).concat();
        assert!(text.contains("calm"), "CERTIFY caption missing: {text:?}");
    }

    #[test]
    fn advisory_renders_with_its_own_caption() {
        let buf = render(Mascot::with_mono(Verdict::Advisory, false), 34, 20);
        assert!(painted_cells(&buf) > 40);
        assert!(signature(&buf).concat().contains("curious"));
    }

    #[test]
    fn warn_renders_with_its_own_caption() {
        let buf = render(Mascot::with_mono(Verdict::Warn, false), 34, 20);
        assert!(painted_cells(&buf) > 40);
        assert!(signature(&buf).concat().contains("alert"));
    }

    #[test]
    fn block_renders_with_its_own_caption() {
        let buf = render(Mascot::with_mono(Verdict::Block, false), 34, 20);
        assert!(painted_cells(&buf) > 40);
        assert!(signature(&buf).concat().contains("guarded"));
    }

    #[test]
    fn the_four_poses_have_distinct_silhouettes() {
        let sigs: Vec<Vec<String>> = [
            Verdict::Certify,
            Verdict::Advisory,
            Verdict::Warn,
            Verdict::Block,
        ]
        .into_iter()
        .map(|v| signature(&render(Mascot::with_mono(v, false), 34, 20)))
        .collect();
        // Every pair of poses differs in at least one cell — no two are the same
        // drawing. (Compares symbols only, which already captures the silhouette.)
        for i in 0..sigs.len() {
            for j in (i + 1)..sigs.len() {
                assert_ne!(sigs[i], sigs[j], "poses {i} and {j} render identically");
            }
        }
    }

    #[test]
    fn truecolor_stacks_two_colors_in_a_half_block() {
        let buf = render(Mascot::with_mono(Verdict::Certify, false), 34, 20);
        // At least one cell uses the upper-half block with distinct fg/bg — the
        // two-pixels-per-cell technique that gives the sprite its resolution.
        let stacked = buf
            .content()
            .iter()
            .any(|c| c.symbol() == "\u{2580}" && c.fg != c.bg && c.bg != Color::Reset);
        assert!(
            stacked,
            "expected a two-color half-block cell in truecolor mode"
        );
    }

    #[test]
    fn monochrome_fallback_uses_default_fg_and_no_background() {
        let buf = render(Mascot::with_mono(Verdict::Warn, true), 34, 20);
        assert!(painted_cells(&buf) > 40, "silhouette should still render");
        // No cell paints an RGB color in mono mode; blocks use the default fg
        // (Color::Reset) so NO_COLOR / non-truecolor terminals stay clean.
        for cell in buf.content() {
            let sym = cell.symbol();
            if sym == "\u{2588}" || sym == "\u{2580}" || sym == "\u{2584}" {
                assert_eq!(cell.fg, Color::Reset, "mono block must use default fg");
                assert_eq!(cell.bg, Color::Reset, "mono block must not set a bg");
            }
        }
    }

    #[test]
    fn renders_into_a_tiny_area_without_panicking() {
        // Clipping at the panel edge must never index out of bounds.
        for w in 1..=6 {
            for h in 1..=4 {
                let _ = render(Mascot::with_mono(Verdict::Block, false), w, h);
            }
        }
    }
}
