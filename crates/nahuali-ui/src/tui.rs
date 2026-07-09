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

use crate::mascot::{self, Mascot, Verdict};
use crate::theme::{self, Rgb};

/// One memory item to browse, already reduced to display strings by the caller.
pub struct Item {
    pub kind: String,
    pub title: String,
    pub detail: String,
    /// `(label, color)` of the item's trust verdict, if it carries one.
    pub trust: Option<(String, Rgb)>,
    pub evidence: Option<String>,
    /// Extra key/value detail lines (scope, confidence, id, …).
    pub meta: Vec<(String, String)>,
}

/// A store-level governance signal shown in the cockpit's bottom bar.
pub struct Signal {
    pub label: String,
    pub value: String,
    pub color: Rgb,
}

/// The ledger's tamper-evidence posture — Nahuali's core differentiator,
/// surfaced honestly: `chain_intact` + a `merkle_root` only when the binary was
/// built with the hash-chain feature; otherwise it is an append-only,
/// checksummed, replay-verified ledger without the cryptographic chain.
pub struct Integrity {
    pub verified: bool,
    pub records: usize,
    pub chain_intact: bool,
    pub merkle_root: Option<String>,
}

/// A point-in-time view of a store for the cockpit.
pub struct Snapshot {
    pub database: String,
    pub store_trust_label: String,
    pub store_trust_color: Rgb,
    pub store_trust_score: f32,
    pub integrity: Integrity,
    pub items: Vec<Item>,
    /// Store-level governance signals (health, review queue, provenance, …).
    pub signals: Vec<Signal>,
}

fn color(c: Rgb) -> Color {
    Color::Rgb(c.0, c.1, c.2)
}

struct App {
    snapshot: Snapshot,
    /// Distinct kinds present, in first-seen order; the filter cycles All → each.
    kinds: Vec<String>,
    /// 0 = all kinds; otherwise `kinds[filter - 1]`.
    filter: usize,
    /// Indices into `snapshot.items` matching the active filter.
    visible: Vec<usize>,
    list: ListState,
}

impl App {
    fn new(snapshot: Snapshot) -> Self {
        let mut kinds: Vec<String> = Vec::new();
        for item in &snapshot.items {
            if !kinds.iter().any(|kind| kind == &item.kind) {
                kinds.push(item.kind.clone());
            }
        }
        let mut app = Self {
            snapshot,
            kinds,
            filter: 0,
            visible: Vec::new(),
            list: ListState::default(),
        };
        app.recompute_visible();
        app
    }

    /// Rebuild the visible index set for the active filter and reset selection.
    fn recompute_visible(&mut self) {
        self.visible = self
            .snapshot
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.filter == 0 || item.kind == self.kinds[self.filter - 1])
            .map(|(index, _)| index)
            .collect();
        self.list.select((!self.visible.is_empty()).then_some(0));
    }

    /// Cycle the kind filter by `dir`, wrapping through All and each kind.
    fn cycle_filter(&mut self, dir: isize) {
        let slots = self.kinds.len() as isize + 1;
        self.filter = (self.filter as isize + dir).rem_euclid(slots) as usize;
        self.recompute_visible();
    }

    /// Move the selection by `delta` within the visible set, wrapping.
    fn step(&mut self, delta: isize) {
        let len = self.visible.len();
        if len == 0 {
            return;
        }
        let current = self.list.selected().unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(len as isize) as usize;
        self.list.select(Some(next));
    }

    fn selected_item(&self) -> Option<&Item> {
        self.list
            .selected()
            .and_then(|row| self.visible.get(row))
            .and_then(|&index| self.snapshot.items.get(index))
    }

    fn visible_items(&self) -> Vec<&Item> {
        self.visible
            .iter()
            .filter_map(|&index| self.snapshot.items.get(index))
            .collect()
    }

    fn filter_label(&self) -> String {
        if self.filter == 0 {
            "all".to_string()
        } else {
            self.kinds[self.filter - 1].clone()
        }
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
                KeyCode::Tab => app.cycle_filter(1),
                KeyCode::BackTab => app.cycle_filter(-1),
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
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_header(frame, app, rows[0]);

    // The nahual mascot gets a dedicated third column, but only when the
    // terminal is wide and tall enough to give it room; otherwise the cockpit
    // keeps its two-pane layout and the mascot hides so nothing is cramped.
    let full = frame.area();
    let show_mascot = full.width >= mascot::MIN_COLS && full.height >= mascot::MIN_ROWS;
    let body = if show_mascot {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(34),
                Constraint::Min(0),
                Constraint::Length(mascot::PANEL_WIDTH),
            ])
            .split(rows[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
            .split(rows[1])
    };

    // Build owned widgets from immutable data first, then take the &mut borrow
    // of the list state to render the stateful list. Titles are clipped to the
    // list column's real width (minus borders, the ▸ symbol, the dot, and the
    // kind column) so they end in an ellipsis instead of a raw terminal cut.
    let title_width = (body[0].width as usize).saturating_sub(16);
    let detail = detail_paragraph(app.selected_item());
    let visible = app.visible_items();
    let list = item_list(&visible, title_width, &app.filter_label());

    frame.render_stateful_widget(list, body[0], &mut app.list);
    frame.render_widget(detail, body[1]);
    if show_mascot {
        draw_mascot(frame, &app.snapshot.store_trust_label, body[2]);
    }

    draw_signals(frame, &app.snapshot.signals, rows[2]);
    draw_footer(frame, rows[3]);
}

/// Draw the nahual mascot in its own bordered panel, its pose bound to the
/// store's trust verdict (parsed from the same label the header shows).
fn draw_mascot(frame: &mut Frame, trust_label: &str, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color(theme::INK_FAINT)))
        .title(Span::styled(
            " nahual ",
            Style::default()
                .fg(color(theme::CLAY))
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Mascot::new(Verdict::from_label(trust_label)), inner);
}

fn draw_signals(frame: &mut Frame, signals: &[Signal], area: Rect) {
    let mut spans: Vec<Span> = Vec::new();
    for signal in signals {
        if !spans.is_empty() {
            spans.push(Span::styled("    ", Style::default()));
        }
        spans.push(Span::styled(
            format!("{} ", signal.label),
            Style::default().fg(color(theme::INK_FAINT)),
        ));
        spans.push(Span::styled(
            signal.value.clone(),
            Style::default()
                .fg(color(signal.color))
                .add_modifier(Modifier::BOLD),
        ));
    }
    let panel = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color(theme::INK_FAINT)))
            .title(Span::styled(
                " signals ",
                Style::default()
                    .fg(color(theme::CLAY))
                    .add_modifier(Modifier::BOLD),
            )),
    );
    frame.render_widget(panel, area);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let trust = Line::from(vec![
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
    ]);
    let header = Paragraph::new(vec![trust, integrity_line(&app.snapshot.integrity)]).block(
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

/// The integrity line — Nahuali's differentiator, stated honestly: a green
/// `tamper-evident · merkle …` when the ledger is hash-chained, or an amber
/// `append-only · hash chain off` when this build lacks the chain feature.
fn integrity_line(integrity: &Integrity) -> Line<'static> {
    let (verdict, verdict_color) = if integrity.verified {
        ("\u{2713} verified", theme::GREEN)
    } else {
        ("\u{2717} unverified", theme::RED)
    };
    let mut spans = vec![
        Span::styled(" Ledger ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled(
            verdict,
            Style::default()
                .fg(color(verdict_color))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" · {} records · ", integrity.records),
            Style::default().fg(color(theme::INK_FAINT)),
        ),
    ];
    match &integrity.merkle_root {
        Some(root) => {
            let short: String = root.chars().take(10).collect();
            let chain_color = if integrity.chain_intact {
                theme::GREEN
            } else {
                theme::RED
            };
            spans.push(Span::styled(
                "tamper-evident",
                Style::default()
                    .fg(color(chain_color))
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" · merkle {short}\u{2026}"),
                Style::default().fg(color(theme::INK_FAINT)),
            ));
        }
        None => {
            spans.push(Span::styled(
                "append-only",
                Style::default().fg(color(theme::INK_DIM)),
            ));
            spans.push(Span::styled(
                " · hash chain off",
                Style::default().fg(color(theme::AMBER)),
            ));
        }
    }
    Line::from(spans)
}

fn item_list(items: &[&Item], title_width: usize, filter_label: &str) -> List<'static> {
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
                Span::raw(clip(&item.title, title_width)),
            ]))
        })
        .collect();

    List::new(rows)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color(theme::INK_FAINT)))
                .title(Span::styled(
                    format!(" memory · {} ({}) ", filter_label, items.len()),
                    Style::default()
                        .fg(color(theme::CLAY))
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_style(
            Style::default()
                .bg(color(theme::SURFACE))
                .fg(color(theme::CLAY))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ")
}

/// Clip `text` to at most `max` display columns, ending in an ellipsis when it
/// would otherwise be cut mid-word by the terminal at the panel edge.
fn clip(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if text.chars().count() <= max {
        return text.to_string();
    }
    let head: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}\u{2026}", head.trim_end())
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
            for (key, value) in &item.meta {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{key:<7}"),
                        Style::default().fg(color(theme::INK_FAINT)),
                    ),
                    Span::styled(value.clone(), Style::default().fg(color(theme::INK_DIM))),
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
        Span::styled("tab ", Style::default().fg(color(theme::CLAY))),
        Span::styled("filter   ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled("q/Esc ", Style::default().fg(color(theme::CLAY))),
        Span::styled("quit", Style::default().fg(color(theme::INK_FAINT))),
    ]));
    frame.render_widget(footer, area);
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;

    use super::*;

    /// Render the whole cockpit into a `TestBackend` of the given size and
    /// return every cell symbol concatenated — enough to assert what is drawn.
    fn cockpit_text(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn item(kind: &str) -> Item {
        Item {
            kind: kind.to_string(),
            title: format!("{kind} title"),
            detail: "detail".to_string(),
            trust: None,
            evidence: None,
            meta: Vec::new(),
        }
    }

    fn snapshot(n: usize) -> Snapshot {
        Snapshot {
            database: "memory".to_string(),
            store_trust_label: "CERTIFY".to_string(),
            store_trust_color: theme::GREEN,
            store_trust_score: 1.0,
            integrity: Integrity {
                verified: true,
                records: n,
                chain_intact: true,
                merkle_root: None,
            },
            items: (0..n).map(|i| item(&format!("episode{i}"))).collect(),
            signals: Vec::new(),
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

    fn mixed() -> Snapshot {
        Snapshot {
            database: "memory".to_string(),
            store_trust_label: "ADVISORY".to_string(),
            store_trust_color: theme::BLUE,
            store_trust_score: 0.75,
            integrity: Integrity {
                verified: true,
                records: 4,
                chain_intact: true,
                merkle_root: None,
            },
            items: vec![
                item("episode"),
                item("episode"),
                item("claim"),
                item("intention"),
            ],
            signals: Vec::new(),
        }
    }

    #[test]
    fn filter_cycles_through_kinds_and_narrows_the_visible_set() {
        let mut app = App::new(mixed());
        assert_eq!(app.kinds, vec!["episode", "claim", "intention"]);
        assert_eq!(app.filter_label(), "all");
        assert_eq!(app.visible.len(), 4);

        app.cycle_filter(1);
        assert_eq!(app.filter_label(), "episode");
        assert_eq!(app.visible.len(), 2);

        app.cycle_filter(1);
        assert_eq!(app.filter_label(), "claim");
        assert_eq!(app.visible.len(), 1);

        // Backward from claim returns to episode, then to all (with wrap intact).
        app.cycle_filter(-1);
        assert_eq!(app.filter_label(), "episode");
        app.cycle_filter(-1);
        assert_eq!(app.filter_label(), "all");
        assert_eq!(app.visible.len(), 4);
    }

    #[test]
    fn selected_item_follows_the_active_filter() {
        let mut app = App::new(mixed());
        app.cycle_filter(2); // all -> episode -> claim
        assert_eq!(
            app.selected_item().map(|item| item.kind.as_str()),
            Some("claim")
        );
    }

    #[test]
    fn mascot_shows_on_a_roomy_terminal() {
        // A wide, tall terminal gets the dedicated nahual panel; its CERTIFY
        // caption ("calm") appears nowhere else in the cockpit.
        let mut app = App::new(snapshot(3));
        let text = cockpit_text(&mut app, 120, 40);
        assert!(
            text.contains("calm"),
            "mascot caption missing on a roomy terminal"
        );
    }

    #[test]
    fn mascot_hides_on_a_narrow_terminal() {
        // Below the width threshold the cockpit keeps its two-pane layout and
        // draws no mascot — and must not panic building the smaller layout.
        let mut app = App::new(snapshot(3));
        let text = cockpit_text(&mut app, 90, 40);
        assert!(
            !text.contains("calm"),
            "mascot must hide when the terminal is too narrow"
        );
    }

    #[test]
    fn mascot_hides_on_a_short_terminal() {
        let mut app = App::new(snapshot(3));
        let text = cockpit_text(&mut app, 120, 24);
        assert!(
            !text.contains("calm"),
            "mascot must hide when the terminal is too short"
        );
    }

    #[test]
    fn mascot_pose_tracks_the_verdict() {
        // A BLOCK store renders the guarded pose (caption "guarded"), not the
        // CERTIFY one — the pose is bound to the live verdict.
        let mut block = snapshot(3);
        block.store_trust_label = "BLOCK · not yet trustworthy".to_string();
        let mut app = App::new(block);
        let text = cockpit_text(&mut app, 120, 40);
        assert!(
            text.contains("guarded"),
            "expected the guarded (BLOCK) pose"
        );
        assert!(
            !text.contains("calm"),
            "must not show the CERTIFY caption for a BLOCK store"
        );
    }
}
