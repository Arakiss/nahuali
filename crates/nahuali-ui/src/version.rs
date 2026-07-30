//! Inline, animated identity card for `nahuali --version`.
//!
//! This is intentionally a human-only surface. Captured output remains the
//! conventional single-line version string in `nahuali-cli`.

use std::io;
use std::thread;
use std::time::Duration;

use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Padding, Paragraph};
use ratatui::{Frame, Terminal, TerminalOptions, Viewport};

use crate::mascot::{VERSION_HEIGHT, VERSION_WIDTH, VersionMascot};
use crate::theme;

const BANNER_HEIGHT: u16 = VERSION_HEIGHT + 2;
const FRAME_DELAY: Duration = Duration::from_millis(100);

/// Render the real mascot spritesheet as a short inline greeting.
///
/// The final frame remains in normal terminal history. The caller is expected
/// to use this only when stdout is an interactive terminal.
pub fn render(version: &str) -> io::Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(BANNER_HEIGHT),
        },
    )?;
    let mascot = VersionMascot::from_terminal()
        .or_else(|_| VersionMascot::halfblocks())
        .map_err(io::Error::other)?;

    for tick in 0..mascot.len() {
        terminal.draw(|frame| draw(frame, &mascot, tick, version))?;
        if tick + 1 < mascot.len() {
            thread::sleep(FRAME_DELAY);
        }
    }
    terminal.show_cursor()
}

fn draw(frame: &mut Frame<'_>, mascot: &VersionMascot, tick: usize, version: &str) {
    let area = frame.area();
    let [image_area, copy_area] =
        Layout::horizontal([Constraint::Length(VERSION_WIDTH + 2), Constraint::Min(12)])
            .areas(area);
    let image_area = centered_image_area(image_area);
    frame.render_widget(mascot.frame(tick), image_area);

    let text = Text::from(vec![
        Line::from("NAHUALI").style(
            Style::default()
                .fg(rgb(theme::CLAY))
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(format!("v{version}")).style(
            Style::default()
                .fg(rgb(theme::GREEN))
                .add_modifier(Modifier::BOLD),
        ),
        Line::default(),
        Line::from("governed memory").style(Style::default().fg(rgb(theme::INK))),
        Line::from("for AI agents").style(Style::default().fg(rgb(theme::INK_DIM))),
    ]);
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Left)
            .block(Block::new().padding(Padding::vertical(2))),
        copy_area,
    );
}

fn centered_image_area(area: Rect) -> Rect {
    Rect::new(
        area.x + 1,
        area.y + (area.height.saturating_sub(VERSION_HEIGHT) / 2),
        VERSION_WIDTH.min(area.width.saturating_sub(1)),
        VERSION_HEIGHT.min(area.height),
    )
}

const fn rgb(color: theme::Rgb) -> Color {
    Color::Rgb(color.0, color.1, color.2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn banner_renders_the_real_mascot_and_version_copy() {
        let mascot = VersionMascot::halfblocks().unwrap();
        let backend = TestBackend::new(48, BANNER_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| draw(frame, &mascot, 2, "1.2.3-beta.4"))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let copy = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(copy.contains("NAHUALI"));
        assert!(copy.contains("v1.2.3-beta.4"));
        assert!(
            buffer
                .content()
                .iter()
                .any(|cell| matches!(cell.symbol(), "▀" | "▄" | "█"))
        );
    }
}
