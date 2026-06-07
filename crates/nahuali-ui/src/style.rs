//! Styling primitives built on the palette. Each returns a `String` that is
//! either ANSI-styled (when colors are enabled) or the plain text unchanged.

use crate::theme::{self, Rgb};

const RESET: &str = "\x1b[0m";

/// Paint `text` with a 24-bit foreground color, optionally bold, when colors
/// are enabled for stdout; otherwise return `text` unchanged.
pub fn paint(text: &str, color: Rgb, bold: bool) -> String {
    if !theme::colors_enabled() {
        return text.to_string();
    }
    let Rgb(r, g, b) = color;
    let weight = if bold { "1;" } else { "" };
    format!("\x1b[{weight}38;2;{r};{g};{b}m{text}{RESET}")
}

/// A bold clay section heading.
pub fn heading(text: &str) -> String {
    paint(text, theme::CLAY, true)
}

/// An accent fragment in clay, not bold — for a highlighted word inline.
pub fn accent(text: &str) -> String {
    paint(text, theme::CLAY, false)
}

/// Secondary detail (ids, counts, hints) in faint ink.
pub fn dim(text: &str) -> String {
    paint(text, theme::INK_FAINT, false)
}

/// A bold colored badge, e.g. a trust verdict word.
pub fn badge(text: &str, color: Rgb) -> String {
    paint(text, color, true)
}

/// A horizontal rule of box-drawing dashes in faint ink, `width` columns wide.
pub fn rule(width: usize) -> String {
    paint(&"\u{2500}".repeat(width), theme::INK_FAINT, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degrades_to_plain_text_off_a_terminal() {
        // The test harness does not attach stdout to a tty, so colors are off
        // and every primitive must return its plain text verbatim.
        assert_eq!(heading("Session briefing"), "Session briefing");
        assert_eq!(dim("episode_1"), "episode_1");
        assert_eq!(accent("Nahuali"), "Nahuali");
        assert_eq!(rule(3), "\u{2500}\u{2500}\u{2500}");
    }
}
