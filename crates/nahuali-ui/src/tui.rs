//! The interactive `explore` governance cockpit (ratatui).
//!
//! Nahuali is agent-first, but a human supervises the agent's memory — this is
//! that window. It renders a trust-first browse of what the agent stored: the
//! store verdict up top, memory items by kind on the left each with its own
//! trust dot, and the selected item's detail (content, trust verdict, evidence)
//! on the right. The CLI builds a plain `Snapshot` and hands it here, so this
//! module stays decoupled from nahuali-core.

use std::io;

use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::theme::{self, Rgb};

/// One memory item to browse, already reduced to display strings by the caller.
pub struct Item {
    pub kind: String,
    pub title: String,
    pub detail: String,
    /// `(label, color)` of the item's trust verdict, if it carries one.
    pub trust: Option<(String, Rgb)>,
    pub evidence: Option<String>,
}

/// A point-in-time view of a store for the cockpit.
pub struct Snapshot {
    pub database: String,
    pub store_trust_label: String,
    pub store_trust_color: Rgb,
    pub store_trust_score: f32,
    pub items: Vec<Item>,
}

fn color(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

struct App {
    snapshot: Snapshot,
    list: ListState,
}

impl App {
    fn new(snapshot: Snapshot) -> Self {
        let mut list = ListState::default();
        if !snapshot.items.is_empty() {
            list.select(Some(0));
        }
        Self { snapshot, list }
    }

    /// Move the selection by `delta`, wrapping around the ends.
    fn step(&mut self, delta: isize) {
        let len = self.snapshot.items.len();
        if len == 0 {
            return;
        }
        let current = self.list.selected().unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(len as isize) as usize;
        self.list.select(Some(next));
    }
}

/// Run the cockpit against `snapshot`, taking over the terminal until the user
/// quits (`q`/`Esc`). Restores the terminal on the way out, including on error.
pub fn run(snapshot: Snapshot) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(snapshot);
    let outcome = event_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    outcome
}

fn event_loop<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, app))?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Down | KeyCode::Char('j') => app.step(1),
                KeyCode::Up | KeyCode::Char('k') => app.step(-1),
                KeyCode::Home => app.list.select(Some(0)),
                _ => {}
            }
        }
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_header(frame, app, rows[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(rows[1]);

    // Build owned widgets from immutable data first, then take the &mut borrow
    // of the list state to render the stateful list.
    let detail = detail_paragraph(app.list.selected().and_then(|i| app.snapshot.items.get(i)));
    let list = item_list(&app.snapshot.items);

    frame.render_stateful_widget(list, body[0], &mut app.list);
    frame.render_widget(detail, body[1]);

    draw_footer(frame, rows[2]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", app.snapshot.store_trust_label),
            Style::default()
                .fg(color(app.snapshot.store_trust_color))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("· score {:.2}", app.snapshot.store_trust_score),
            Style::default().fg(color(theme::INK_FAINT)),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color(theme::INK_FAINT)))
            .title(Span::styled(
                format!(" nahuali explore · {} ", app.snapshot.database),
                Style::default()
                    .fg(color(theme::CLAY))
                    .add_modifier(Modifier::BOLD),
            )),
    );
    frame.render_widget(header, area);
}

fn item_list(items: &[Item]) -> List<'static> {
    let rows: Vec<ListItem> = items
        .iter()
        .map(|item| {
            let dot_color = item.trust.as_ref().map_or(theme::INK_FAINT, |(_, c)| *c);
            ListItem::new(Line::from(vec![
                Span::styled("● ", Style::default().fg(color(dot_color))),
                Span::styled(
                    format!("{:<10}", item.kind),
                    Style::default().fg(color(theme::INK_FAINT)),
                ),
                Span::raw(item.title.clone()),
            ]))
        })
        .collect();

    List::new(rows)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color(theme::INK_FAINT)))
                .title(Span::styled(
                    " memory ",
                    Style::default()
                        .fg(color(theme::CLAY))
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(
            Style::default()
                .fg(color(theme::CLAY))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ")
}

fn detail_paragraph(item: Option<&Item>) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    match item {
        None => lines.push(Line::styled(
            "No memory to show yet.",
            Style::default().fg(color(theme::INK_FAINT)),
        )),
        Some(item) => {
            lines.push(Line::from(Span::styled(
                item.kind.clone(),
                Style::default()
                    .fg(color(theme::CLAY))
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::raw(item.title.clone()));
            lines.push(Line::raw(""));
            for chunk in item.detail.lines() {
                lines.push(Line::styled(
                    chunk.to_string(),
                    Style::default().fg(color(theme::INK_DIM)),
                ));
            }
            if let Some((label, trust_color)) = &item.trust {
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::styled("trust  ", Style::default().fg(color(theme::INK_FAINT))),
                    Span::styled(
                        label.clone(),
                        Style::default()
                            .fg(color(*trust_color))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            }
            if let Some(evidence) = &item.evidence {
                lines.push(Line::from(vec![
                    Span::styled("source ", Style::default().fg(color(theme::INK_FAINT))),
                    Span::styled(
                        evidence.clone(),
                        Style::default().fg(color(theme::INK_FAINT)),
                    ),
                ]));
            }
        }
    }

    Paragraph::new(lines).wrap(Wrap { trim: true }).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color(theme::INK_FAINT)))
            .title(Span::styled(
                " detail ",
                Style::default()
                    .fg(color(theme::CLAY))
                    .add_modifier(Modifier::BOLD),
            )),
    )
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("  ↑↓/jk ", Style::default().fg(color(theme::CLAY))),
        Span::styled("move   ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled("q/Esc ", Style::default().fg(color(theme::CLAY))),
        Span::styled("quit", Style::default().fg(color(theme::INK_FAINT))),
    ]));
    frame.render_widget(footer, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: &str) -> Item {
        Item {
            kind: kind.to_string(),
            title: format!("{kind} title"),
            detail: "detail".to_string(),
            trust: None,
            evidence: None,
        }
    }

    fn snapshot(n: usize) -> Snapshot {
        Snapshot {
            database: "memory".to_string(),
            store_trust_label: "CERTIFY".to_string(),
            store_trust_color: theme::GREEN,
            store_trust_score: 1.0,
            items: (0..n).map(|i| item(&format!("episode{i}"))).collect(),
        }
    }

    #[test]
    fn selection_wraps_both_directions() {
        let mut app = App::new(snapshot(3));
        assert_eq!(app.list.selected(), Some(0));
        app.step(1);
        assert_eq!(app.list.selected(), Some(1));
        app.step(-1);
        app.step(-1);
        assert_eq!(app.list.selected(), Some(2)); // wrapped past 0
        app.step(1);
        assert_eq!(app.list.selected(), Some(0)); // wrapped past end
    }

    #[test]
    fn empty_snapshot_has_no_selection_and_does_not_panic() {
        let mut app = App::new(snapshot(0));
        assert_eq!(app.list.selected(), None);
        app.step(1);
        assert_eq!(app.list.selected(), None);
    }
}
