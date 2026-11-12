//! Cell-width aware text helpers. Every string that lands in a cell goes
//! through here: widths come from `unicode-width`, never from `len()`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn width(text: &str) -> u16 {
    u16::try_from(UnicodeWidthStr::width(text)).unwrap_or(u16::MAX)
}

/// Cut `text` to at most `max` cells, ending in `…` when it had to cut.
pub fn truncate(text: &str, max: u16) -> String {
    if width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0u16;
    for c in text.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0) as u16;
        if used + w > max - 1 {
            break;
        }
        out.push(c);
        used += w;
    }
    let trimmed = out.trim_end().to_string();
    trimmed + "…"
}

/// A styled run inside one line.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub text: String,
    pub style: Style,
}

/// Write runs on one row starting at `x`, clipped to `max_x`, truncating the
/// last visible run with an ellipsis. Returns the x after the last cell.
pub fn put_runs(buf: &mut Buffer, x: u16, y: u16, max_x: u16, runs: &[Run]) -> u16 {
    let total: u16 = runs.iter().map(|r| width(&r.text)).sum();
    let available = max_x.saturating_sub(x);
    let mut cursor = x;
    if total <= available {
        for run in runs {
            cursor = put(buf, cursor, y, &run.text, run.style, max_x);
        }
        return cursor;
    }
    let mut left = available;
    for run in runs {
        let w = width(&run.text);
        if w < left {
            cursor = put(buf, cursor, y, &run.text, run.style, max_x);
            left -= w;
        } else {
            let cut = truncate(&run.text, left);
            cursor = put(buf, cursor, y, &cut, run.style, max_x);
            break;
        }
    }
    cursor
}

/// Write one string, clipped at `max_x`. Returns the x after it.
pub fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style, max_x: u16) -> u16 {
    if x >= max_x || !buf.area.contains((x, y).into()) {
        return x;
    }
    let limit = (max_x - x) as usize;
    let (end, _) = buf.set_stringn(x, y, text, limit, style);
    end
}

/// Clear a rectangle to blanks in `style` (modifiers underneath are reset,
/// not inherited).
pub fn fill(buf: &mut Buffer, area: Rect, style: Style) {
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.reset();
                cell.set_style(style);
            }
        }
    }
}

/// Paint a background colour under existing text (keeps symbols and fg).
pub fn tint(buf: &mut Buffer, area: Rect, bg: ratatui::style::Color) {
    let area = area.intersection(buf.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(bg);
            }
        }
    }
}

/// Word-wrap runs into lines of at most `max` cells. `\n` breaks a line.
pub fn wrap(runs: &[Run], max: u16) -> Vec<Vec<Run>> {
    let max = max.max(1);
    let mut lines: Vec<Vec<Run>> = vec![Vec::new()];
    let mut used = 0u16;
    for run in runs {
        for (i, segment) in run.text.split('\n').enumerate() {
            if i > 0 {
                lines.push(Vec::new());
                used = 0;
            }
            for token in split_keep_spaces(segment) {
                let w = width(token);
                if used + w > max && used > 0 {
                    if token.trim().is_empty() {
                        continue;
                    }
                    lines.push(Vec::new());
                    used = 0;
                }
                let token = if w > max { truncate(token, max) } else { token.to_string() };
                used += width(&token);
                push_run(lines.last_mut().expect("one line"), token, run.style);
            }
        }
    }
    for line in &mut lines {
        if let Some(last) = line.last_mut() {
            let trimmed = last.text.trim_end().to_string();
            last.text = trimmed;
        }
    }
    lines
}

fn push_run(line: &mut Vec<Run>, text: String, style: Style) {
    match line.last_mut() {
        Some(last) if last.style == style => last.text.push_str(&text),
        _ => line.push(Run { text, style }),
    }
}

fn split_keep_spaces(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = None;
    for (i, c) in text.char_indices() {
        let space = c == ' ';
        match in_space {
            Some(prev) if prev != space => {
                out.push(&text[start..i]);
                start = i;
            }
            _ => {}
        }
        in_space = Some(space);
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> Run {
        Run { text: text.into(), style: Style::default() }
    }

    fn lines(runs: &[Run], max: u16) -> Vec<String> {
        wrap(runs, max).into_iter().map(|l| l.into_iter().map(|r| r.text).collect()).collect()
    }

    #[test]
    fn truncates_by_cells_not_bytes() {
        assert_eq!(truncate("añadir más", 6), "añadi…");
        assert_eq!(truncate("日本語テキスト", 7), "日本語…");
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(width("✓ ok"), 4);
    }

    #[test]
    fn wraps_on_words_and_newlines() {
        assert_eq!(lines(&[plain("one two three four")], 9), vec!["one two", "three", "four"]);
        assert_eq!(lines(&[plain("a\nb")], 9), vec!["a", "b"]);
        assert_eq!(lines(&[plain("Mo Tu\n 1  2")], 20), vec!["Mo Tu", " 1  2"]);
    }
}
