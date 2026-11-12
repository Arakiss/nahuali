//! The axolotl as plain coloured text, for a greeting printed outside the
//! TUI: one line of half blocks per two pixel rows, no terminal query.

use image::imageops::{FilterType, resize};
use image::{Rgba, RgbaImage};
use nahuali_view::MascotState;

use super::atlas::Atlas;

const VISIBLE: u8 = 96;

/// How colours are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Palette {
    /// 24-bit colour.
    TrueColor,
    /// The 6x6x6 cube of 256-colour terminals.
    Cube,
}

impl Palette {
    /// What the terminal claims, from `COLORTERM`.
    pub fn from_env(colorterm: Option<&str>) -> Self {
        match colorterm {
            Some("truecolor" | "24bit") => Self::TrueColor,
            _ => Self::Cube,
        }
    }

    fn code(self, layer: u8, p: Rgba<u8>) -> String {
        let [r, g, b, _] = p.0;
        match self {
            Self::TrueColor => format!("\x1b[{layer};2;{r};{g};{b}m"),
            Self::Cube => {
                let step = |c: u8| (u16::from(c) * 5 / 255) as u8;
                format!("\x1b[{layer};5;{}m", 16 + 36 * step(r) + 6 * step(g) + step(b))
            }
        }
    }
}

/// `state`'s first frame, at most `width` cells wide.
pub fn art(state: MascotState, width: u32, palette: Palette) -> String {
    let atlas = Atlas::discover();
    let frame = &atlas.frames(state)[0].image;
    render(frame, width, palette)
}

fn render(frame: &RgbaImage, width: u32, palette: Palette) -> String {
    let height = (width * frame.height() / frame.width().max(1)).max(2);
    let small = resize(frame, width, height + height % 2, FilterType::Triangle);
    let mut out = String::new();
    for y in (0..small.height()).step_by(2) {
        let mut line = String::new();
        for x in 0..small.width() {
            let (top, bottom) = (*small.get_pixel(x, y), *small.get_pixel(x, y + 1));
            match (top.0[3] >= VISIBLE, bottom.0[3] >= VISIBLE) {
                (false, false) => line.push(' '),
                (true, false) => line.push_str(&format!("{}▀\x1b[0m", palette.code(38, top))),
                (false, true) => line.push_str(&format!("{}▄\x1b[0m", palette.code(38, bottom))),
                (true, true) => {
                    line.push_str(&format!("{}{}▀\x1b[0m", palette.code(38, top), palette.code(48, bottom)));
                }
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_greeting_is_coloured_text_of_the_asked_width() {
        let text = art(MascotState::Greeting, 24, Palette::TrueColor);
        assert!(text.contains("\x1b[38;2;") && text.contains('▀'));
        assert!(text.lines().count() >= 6);
        let cube = art(MascotState::Greeting, 24, Palette::Cube);
        assert!(cube.contains("\x1b[38;5;") && !cube.contains("\x1b[38;2;"));
    }

    #[test]
    fn colorterm_decides_the_palette() {
        assert_eq!(Palette::from_env(Some("truecolor")), Palette::TrueColor);
        assert_eq!(Palette::from_env(Some("24bit")), Palette::TrueColor);
        assert_eq!(Palette::from_env(None), Palette::Cube);
    }
}
