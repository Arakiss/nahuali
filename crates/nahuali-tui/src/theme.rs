//! The warm coffee palette, in two layers: raw colours sampled from the
//! `today-terminal` sketch, and semantic roles the renderer uses. A tone from
//! the view model never names a colour; it goes through [`Theme::tone`].

use nahuali_view::Tone;
use ratatui::style::{Color, Modifier, Style};

/// How many colours the terminal can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    TrueColor,
    Ansi256,
}

impl ColorMode {
    /// `COLORTERM=truecolor|24bit` means 24-bit; anything else gets the
    /// 256-colour approximation (Apple Terminal still reports 256).
    pub fn detect() -> Self {
        let truecolor =
            std::env::var("COLORTERM").is_ok_and(|v| v.contains("truecolor") || v.contains("24bit"));
        if truecolor { Self::TrueColor } else { Self::Ansi256 }
    }
}

/// A 24-bit colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// Raw palette, sampled from `assets/screens/sketches/today-terminal.png`.
pub mod raw {
    use super::Rgb;
    pub const BG: Rgb = Rgb(27, 16, 12);
    pub const SURFACE: Rgb = Rgb(52, 31, 23);
    pub const SURFACE_SOFT: Rgb = Rgb(38, 24, 18);
    pub const BORDER: Rgb = Rgb(142, 104, 87);
    pub const BORDER_DIM: Rgb = Rgb(92, 68, 60);
    pub const RULE: Rgb = Rgb(160, 122, 102);
    pub const INK: Rgb = Rgb(246, 240, 232);
    pub const INK_DIM: Rgb = Rgb(201, 162, 140);
    pub const INK_FAINT: Rgb = Rgb(143, 116, 102);
    pub const ACCENT: Rgb = Rgb(247, 145, 61);
    pub const ACCENT_SOFT: Rgb = Rgb(216, 116, 71);
    pub const OK: Rgb = Rgb(156, 194, 126);
    pub const WARN: Rgb = Rgb(235, 180, 94);
    pub const DANGER: Rgb = Rgb(240, 88, 78);
    pub const DANGER_BORDER: Rgb = Rgb(210, 72, 63);
    pub const GILL: Rgb = Rgb(239, 111, 103);
}

/// Semantic colours for one colour mode.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub mode: ColorMode,
}

impl Theme {
    pub fn new(mode: ColorMode) -> Self {
        Self { mode }
    }

    pub fn color(&self, rgb: Rgb) -> Color {
        match self.mode {
            ColorMode::TrueColor => Color::Rgb(rgb.0, rgb.1, rgb.2),
            ColorMode::Ansi256 => Color::Indexed(token_xterm(rgb).unwrap_or_else(|| xterm_index(rgb))),
        }
    }

    pub fn bg(&self) -> Color {
        self.color(raw::BG)
    }

    pub fn base(&self) -> Style {
        Style::default().fg(self.color(raw::INK)).bg(self.bg())
    }

    pub fn tone_rgb(tone: Tone) -> Rgb {
        match tone {
            Tone::Neutral => raw::INK,
            Tone::Accent => raw::ACCENT,
            Tone::Ok => raw::OK,
            Tone::Warn => raw::WARN,
            Tone::Danger => raw::DANGER,
            Tone::Muted => raw::INK_DIM,
        }
    }

    /// Foreground style for text of `tone`.
    pub fn tone(&self, tone: Tone) -> Style {
        Style::default().fg(self.color(Self::tone_rgb(tone)))
    }

    pub fn text(&self, tone: Tone, strong: bool) -> Style {
        let style = self.tone(tone);
        if strong { style.add_modifier(Modifier::BOLD) } else { style }
    }

    pub fn faint(&self) -> Style {
        Style::default().fg(self.color(raw::INK_FAINT))
    }

    pub fn rule(&self) -> Style {
        Style::default().fg(self.color(raw::RULE))
    }

    pub fn divider(&self) -> Style {
        Style::default().fg(self.color(raw::BORDER_DIM))
    }

    /// Card border for `tone`.
    pub fn border(&self, tone: Tone) -> Style {
        let rgb = match tone {
            Tone::Danger => raw::DANGER_BORDER,
            Tone::Warn => raw::WARN,
            Tone::Muted => raw::BORDER_DIM,
            Tone::Ok => raw::OK,
            Tone::Neutral | Tone::Accent => raw::BORDER,
        };
        Style::default().fg(self.color(rgb))
    }

    /// Card title for `tone`.
    pub fn title(&self, tone: Tone) -> Style {
        let rgb = match tone {
            Tone::Neutral => raw::INK,
            Tone::Muted => raw::INK_DIM,
            other => Self::tone_rgb(other),
        };
        Style::default().fg(self.color(rgb)).add_modifier(Modifier::BOLD)
    }

    pub fn selection(&self) -> Color {
        self.color(raw::SURFACE)
    }

    pub fn surface_soft(&self) -> Color {
        self.color(raw::SURFACE_SOFT)
    }
}

/// Hand-picked xterm-256 stand-ins for the palette tokens. The cube has no
/// dark brown, so the coffee background becomes the darkest warm-neutral
/// grey and the browns lean on the 95/138/181 rose-greys instead of olive.
fn token_xterm(rgb: Rgb) -> Option<u8> {
    use raw::*;
    let table = [
        (BG, 234),
        (SURFACE, 237),
        (SURFACE_SOFT, 235),
        (BORDER, 138),
        (BORDER_DIM, 95),
        (RULE, 138),
        (INK, 255),
        (INK_DIM, 181),
        (INK_FAINT, 138),
        (ACCENT, 209),
        (ACCENT_SOFT, 173),
        (OK, 150),
        (WARN, 179),
        (DANGER, 203),
        (DANGER_BORDER, 167),
        (GILL, 203),
    ];
    table.iter().find(|(c, _)| *c == rgb).map(|(_, i)| *i)
}

/// Nearest xterm-256 index (6×6×6 cube or the grey ramp).
pub fn xterm_index(rgb: Rgb) -> u8 {
    const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest = |v: u8| -> (usize, u8) {
        STEPS
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| (i32::from(**s) - i32::from(v)).abs())
            .map(|(i, s)| (i, *s))
            .unwrap_or((0, 0))
    };
    let (ri, rv) = nearest(rgb.0);
    let (gi, gv) = nearest(rgb.1);
    let (bi, bv) = nearest(rgb.2);
    let cube = 16 + 36 * ri as u8 + 6 * gi as u8 + bi as u8;
    let dist = |a: (u8, u8, u8)| {
        let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2);
        d(a.0, rgb.0) + d(a.1, rgb.1) + d(a.2, rgb.2)
    };
    let avg = (u16::from(rgb.0) + u16::from(rgb.1) + u16::from(rgb.2)) / 3;
    let grey_index = ((avg.saturating_sub(8)) / 10).min(23) as u8;
    let grey = 8 + 10 * grey_index;
    if dist((grey, grey, grey)) < dist((rv, gv, bv)) { 232 + grey_index } else { cube }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palette::{FromColor, Srgb};

    fn luminance(rgb: Rgb) -> f32 {
        let lin = Srgb::new(rgb.0, rgb.1, rgb.2).into_format::<f32>().into_linear();
        let xyz = palette::Xyz::from_color(lin);
        xyz.y
    }

    fn contrast(a: Rgb, b: Rgb) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        let (hi, lo) = if x > y { (x, y) } else { (y, x) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn text_tones_are_readable_on_the_background() {
        for tone in [Tone::Neutral, Tone::Accent, Tone::Ok, Tone::Warn, Tone::Danger, Tone::Muted] {
            let ratio = contrast(Theme::tone_rgb(tone), raw::BG);
            assert!(ratio >= 4.5, "{tone:?} contrast {ratio:.2}");
            let on_selection = contrast(Theme::tone_rgb(tone), raw::SURFACE);
            assert!(on_selection >= 4.5, "{tone:?} on selection {on_selection:.2}");
        }
    }

    #[test]
    fn borders_and_faint_glyphs_reach_three_to_one() {
        for rgb in [raw::BORDER, raw::RULE, raw::INK_FAINT, raw::DANGER_BORDER] {
            assert!(contrast(rgb, raw::BG) >= 3.0, "{rgb:?}");
        }
    }

    #[test]
    fn xterm_mapping_is_sane() {
        assert_eq!(xterm_index(Rgb(0, 0, 0)), 16);
        assert_eq!(xterm_index(Rgb(255, 255, 255)), 231);
        assert_eq!(xterm_index(Rgb(128, 128, 128)), 244);
    }
}
