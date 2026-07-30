//! The interactive `explore` governance cockpit (ratatui).
//!
//! A human can supervise an agent's memory through this window. It renders an
//! evidence-aware browse of what the agent stored: the
//! store verdict up top, memory items by kind on the left each with its own
//! trust dot, and the selected item's detail (content, trust verdict, evidence)
//! on the right. The CLI builds a plain `Snapshot` and hands it here, so this
//! module stays decoupled from nahuali-core.
//!
//! The nahual mascot ([`crate::mascot`]) rides along quietly: a small axolotl in
//! the bottom-right corner during normal use, and the full sprite taking over
//! the otherwise-empty detail pane. Both mirror the live store verdict.

use std::io;
use std::time::{Duration, Instant};

use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::mascot::{self, Verdict};
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

/// The hash-chain state shown independently from content authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerStatus {
    /// The ledger has no records yet.
    Empty,
    /// Every record is chained and every link matches.
    Verified,
    /// The ledger contains compatible history from before hash chaining.
    Legacy,
    /// A recorded link does not match its predecessor.
    Broken,
    /// The binary was built without tamper evidence.
    Unavailable,
}

/// The store's recorded-history posture, shown independently from the content
/// authority verdict.
pub struct Integrity {
    pub records: usize,
    pub checksums_valid: bool,
    pub sequence_contiguous: bool,
    pub status: LedgerStatus,
    pub merkle_root: Option<String>,
}

/// External checkpoint posture, kept independent from both content authority
/// and the live ledger integrity verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorStatus {
    /// No checkpoint and external policy were supplied to the cockpit.
    NotChecked,
    /// The checkpoint authenticates the complete current ledger.
    TrustedCurrent,
    /// The checkpoint authenticates a historical prefix; later appends remain
    /// outside that checkpoint's coverage.
    TrustedHistorical,
    /// Verification completed and rejected the checkpoint under the policy.
    Untrusted,
    /// The checkpoint or policy could not be parsed or validated.
    Invalid,
    /// This build does not include external checkpoint verification.
    Unavailable,
}

/// Display-ready external checkpoint result prepared by the CLI layer.
pub struct Anchor {
    pub status: AnchorStatus,
    /// Number of later updates when the supplied proof covers an earlier state.
    pub newer_updates: u64,
}

/// A point-in-time view of a store for the cockpit.
pub struct Snapshot {
    pub store_trust_label: String,
    pub store_trust_color: Rgb,
    pub integrity: Integrity,
    pub anchor: Anchor,
    pub items: Vec<Item>,
    /// Store-level governance signals (health, review queue, provenance, …).
    pub signals: Vec<Signal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputMode {
    Browse,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppAction {
    Continue,
    Quit,
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
    input_mode: InputMode,
    search_query: String,
    mascot_images: Option<mascot::MascotImages>,
    mascot_tick: usize,
}

impl App {
    fn new(snapshot: Snapshot) -> Self {
        let verdict = Verdict::from_label(&snapshot.store_trust_label);
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
            input_mode: InputMode::Browse,
            search_query: String::new(),
            mascot_images: mascot::MascotImages::halfblocks(verdict).ok(),
            mascot_tick: 0,
        };
        app.recompute_visible();
        app
    }

    /// Rebuild the visible index set for the active filter and reset selection.
    fn recompute_visible(&mut self) {
        let query = self.search_query.to_lowercase();
        self.visible = self
            .snapshot
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                (self.filter == 0 || item.kind == self.kinds[self.filter - 1])
                    && item_matches_search(item, &query)
            })
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

    fn list_label(&self) -> String {
        let filter = self.filter_label();
        if self.search_query.is_empty() {
            filter
        } else {
            format!("{filter} · /{}", self.search_query)
        }
    }

    fn handle_key(&mut self, code: KeyCode) -> AppAction {
        if self.input_mode == InputMode::Search {
            match code {
                KeyCode::Esc | KeyCode::Enter => self.input_mode = InputMode::Browse,
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.recompute_visible();
                }
                KeyCode::Char(character) => {
                    self.search_query.push(character);
                    self.recompute_visible();
                }
                _ => {}
            }
            return AppAction::Continue;
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => AppAction::Quit,
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search;
                AppAction::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.step(1);
                AppAction::Continue
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.step(-1);
                AppAction::Continue
            }
            KeyCode::Tab => {
                self.cycle_filter(1);
                AppAction::Continue
            }
            KeyCode::BackTab => {
                self.cycle_filter(-1);
                AppAction::Continue
            }
            KeyCode::Home => {
                self.list.select((!self.visible.is_empty()).then_some(0));
                AppAction::Continue
            }
            _ => AppAction::Continue,
        }
    }

    fn on_tick(&mut self) {
        self.mascot_tick = self.mascot_tick.wrapping_add(1);
    }
}

fn item_matches_search(item: &Item, query: &str) -> bool {
    query.is_empty()
        || item.kind.to_lowercase().contains(query)
        || item.title.to_lowercase().contains(query)
        || item.detail.to_lowercase().contains(query)
        || (item.evidence.is_some() && "evidence linked".contains(query))
        || item.meta.iter().any(|(key, value)| {
            !is_internal_meta_key(key)
                && (key.to_lowercase().contains(query) || value.to_lowercase().contains(query))
        })
}

fn is_internal_meta_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized == "id"
        || normalized.ends_with("_id")
        || matches!(normalized.as_str(), "conf" | "scope")
}

const MASCOT_TICK_RATE: Duration = Duration::from_millis(240);

/// Run the cockpit against `snapshot`, taking over the terminal until the user
/// quits (`q`/`Esc`). Restores the terminal on the way out, including on error.
pub fn run(snapshot: Snapshot) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(snapshot);
    let verdict = Verdict::from_label(&app.snapshot.store_trust_label);
    if let Ok(images) = mascot::MascotImages::from_terminal(verdict) {
        app.mascot_images = Some(images);
    }
    let outcome = event_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    outcome
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|frame| draw(frame, app))?;
        let timeout = MASCOT_TICK_RATE.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && app.handle_key(key.code) == AppAction::Quit
        {
            return Ok(());
        }
        if last_tick.elapsed() >= MASCOT_TICK_RATE {
            app.on_tick();
            last_tick = Instant::now();
        }
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    let corner_mascot = app
        .mascot_images
        .as_ref()
        .and_then(mascot::MascotImages::corner);
    let footer_height = if corner_mascot.is_some() && frame.area().height >= 18 {
        mascot::MINI_SLOT_HEIGHT
    } else {
        1
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(footer_height),
        ])
        .split(frame.area());

    draw_header(frame, app, rows[0]);

    // The cockpit keeps a stable two-pane body. The full nahual owns only the
    // empty detail state; the always-visible mini version lives in the footer,
    // outside the operator's working panes.
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(rows[1]);

    // Build owned widgets from immutable data first, then take the &mut borrow
    // of the list state to render the stateful list. Titles are clipped to the
    // list column's real width (minus borders, the ▸ symbol, the dot, and the
    // kind column) so they end in an ellipsis instead of a raw terminal cut.
    let title_width = (body[0].width as usize).saturating_sub(16);
    let visible = app.visible_items();
    let list = item_list(&visible, title_width, &app.list_label());

    frame.render_stateful_widget(list, body[0], &mut app.list);
    draw_detail(frame, app, body[1]);

    draw_signals(frame, &app.snapshot.signals, rows[2]);
    let verdict = Verdict::from_label(&app.snapshot.store_trust_label);
    draw_footer(
        frame,
        rows[3],
        verdict,
        corner_mascot,
        app.mascot_tick,
        app.input_mode,
        &app.search_query,
    );
}

/// Draw the right-hand detail pane. With an item selected it shows that item's
/// content, trust, and evidence; with nothing selected (an empty store or an
/// empty filtered set) the pane becomes the nahual's home — see
/// [`draw_empty_detail`].
fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    match app.selected_item() {
        Some(item) => frame.render_widget(detail_paragraph(item), area),
        None => {
            let verdict = Verdict::from_label(&app.snapshot.store_trust_label);
            let empty_mascot = app.mascot_images.as_ref().map(mascot::MascotImages::empty);
            draw_empty_detail(frame, verdict, empty_mascot, area);
        }
    }
}

/// The empty-state detail pane. On a pane with room ([`mascot::EMPTY_STATE_MIN_COLS`]
/// × [`mascot::EMPTY_STATE_MIN_ROWS`]) the full nahual takes it over — sprite,
/// caption, and a dim hint — which is where the art lives now that it has no
/// panel; it costs no space while the operator is actually browsing. Below that
/// size the pane falls back to the plain placeholder line.
fn draw_empty_detail(
    frame: &mut Frame,
    verdict: Verdict,
    empty_mascot: Option<&mascot::RasterMascot>,
    area: Rect,
) {
    let block = detail_block();
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let roomy =
        area.width >= mascot::EMPTY_STATE_MIN_COLS && area.height >= mascot::EMPTY_STATE_MIN_ROWS;
    if !roomy || empty_mascot.is_none() {
        frame.render_widget(
            Paragraph::new(Line::styled(
                "No memory to show yet.",
                Style::default().fg(color(theme::INK_FAINT)),
            )),
            inner,
        );
        return;
    }

    let empty_mascot = empty_mascot.expect("the empty mascot was checked above");
    let image_size = empty_mascot.size();
    let total_h = image_size.height.saturating_add(3); // image, gap, caption, hint
    let top = inner.y + inner.height.saturating_sub(total_h) / 2;
    let image_x = inner.x + inner.width.saturating_sub(image_size.width) / 2;
    frame.render_widget(
        empty_mascot.frame(),
        Rect::new(image_x, top, image_size.width, image_size.height),
    );

    let caption = verdict.caption();
    let caption_y = top + image_size.height + 1;
    if caption_y < inner.bottom() {
        let caption_w = (caption.chars().count() as u16).min(inner.width);
        let caption_x = inner.x + inner.width.saturating_sub(caption_w) / 2;
        frame.render_widget(
            Paragraph::new(Line::styled(
                caption,
                Style::default().fg(color(verdict.accent())),
            )),
            Rect::new(caption_x, caption_y, caption_w, 1),
        );
    }

    let hint = "the nahual watches over your memory";
    let hint_y = caption_y + 1;
    if hint_y < inner.bottom() {
        let hint_w = (hint.chars().count() as u16).min(inner.width);
        let hint_x = inner.x + inner.width.saturating_sub(hint_w) / 2;
        frame.render_widget(
            Paragraph::new(Line::styled(
                hint,
                Style::default().fg(color(theme::INK_FAINT)),
            )),
            Rect::new(hint_x, hint_y, hint_w, 1),
        );
    }
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
                " at a glance ",
                Style::default()
                    .fg(color(theme::CLAY))
                    .add_modifier(Modifier::BOLD),
            )),
    );
    frame.render_widget(panel, area);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let trust = Line::from(vec![
        Span::styled(" MEMORY ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled(
            format!("{} ", app.snapshot.store_trust_label),
            Style::default()
                .fg(color(app.snapshot.store_trust_color))
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let title = vec![Span::styled(
        " Nahuali · Memory Explorer ",
        Style::default()
            .fg(color(theme::CLAY))
            .add_modifier(Modifier::BOLD),
    )];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color(theme::INK_FAINT)))
        .title(Line::from(title));
    let header = Paragraph::new(vec![
        trust,
        history_line(&app.snapshot.integrity),
        proof_line(&app.snapshot.anchor),
    ])
    .block(block);
    frame.render_widget(header, area);
}

fn proof_line(anchor: &Anchor) -> Line<'static> {
    let (label, label_color) = proof_badge(anchor.status);
    Line::from(vec![
        Span::styled(" EXTERNAL ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled(
            label,
            Style::default()
                .fg(color(label_color))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" · {}", proof_detail(anchor)),
            Style::default().fg(color(theme::INK_FAINT)),
        ),
    ])
}

fn proof_detail(anchor: &Anchor) -> String {
    match anchor.status {
        AnchorStatus::NotChecked => "optional external check not provided".to_string(),
        AnchorStatus::TrustedCurrent => "covers the memory shown now".to_string(),
        AnchorStatus::TrustedHistorical => format!(
            "{} newer {} not covered",
            anchor.newer_updates,
            if anchor.newer_updates == 1 {
                "update is"
            } else {
                "updates are"
            }
        ),
        AnchorStatus::Untrusted => "external check was not accepted".to_string(),
        AnchorStatus::Invalid => "external check could not be completed".to_string(),
        AnchorStatus::Unavailable => "external checks are unavailable in this build".to_string(),
    }
}

fn proof_badge(status: AnchorStatus) -> (&'static str, Rgb) {
    match status {
        AnchorStatus::NotChecked => ("o NOT PROVIDED", theme::INK_DIM),
        AnchorStatus::TrustedCurrent => ("\u{2713} CURRENT", theme::GREEN),
        AnchorStatus::TrustedHistorical => ("! EARLIER STATE", theme::AMBER),
        AnchorStatus::Untrusted => ("\u{2717} NOT ACCEPTED", theme::RED),
        AnchorStatus::Invalid => ("\u{2717} COULD NOT VERIFY", theme::RED),
        AnchorStatus::Unavailable => ("! UNAVAILABLE", theme::AMBER),
    }
}

/// The integrity line keeps a fully chained ledger, legacy unchained history,
/// a broken chain, and a build without tamper evidence visibly distinct.
fn history_line(integrity: &Integrity) -> Line<'static> {
    let (verdict, verdict_color) = history_badge(integrity);
    let mut spans = vec![
        Span::styled(" HISTORY ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled(
            verdict,
            Style::default()
                .fg(color(verdict_color))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" · {} updates · ", integrity.records),
            Style::default().fg(color(theme::INK_FAINT)),
        ),
    ];
    if !integrity.checksums_valid
        || !integrity.sequence_contiguous
        || (integrity.status == LedgerStatus::Verified
            && integrity.records > 0
            && integrity.merkle_root.is_none())
    {
        spans.push(Span::styled(
            "internal record checks failed",
            Style::default().fg(color(theme::RED)),
        ));
    } else {
        append_history_detail(&mut spans, integrity);
    }
    Line::from(spans)
}

fn history_badge(integrity: &Integrity) -> (&'static str, Rgb) {
    if !integrity.checksums_valid
        || !integrity.sequence_contiguous
        || (integrity.status == LedgerStatus::Verified
            && integrity.records > 0
            && integrity.merkle_root.is_none())
    {
        return ("\u{2717} PROBLEM", theme::RED);
    }

    match integrity.status {
        LedgerStatus::Empty => ("o EMPTY", theme::INK_DIM),
        LedgerStatus::Verified => ("\u{2713} CHECKS PASS", theme::GREEN),
        LedgerStatus::Legacy => ("! LIMITED", theme::AMBER),
        LedgerStatus::Broken => ("\u{2717} PROBLEM", theme::RED),
        LedgerStatus::Unavailable => ("! PARTIAL", theme::AMBER),
    }
}

fn append_history_detail(spans: &mut Vec<Span<'static>>, integrity: &Integrity) {
    match (integrity.status, &integrity.merkle_root) {
        (LedgerStatus::Empty, _) => {
            spans.push(Span::styled(
                "ready for the first memory",
                Style::default().fg(color(theme::INK_DIM)),
            ));
        }
        (LedgerStatus::Verified, Some(_)) => {
            spans.push(Span::styled(
                "internally consistent",
                Style::default()
                    .fg(color(theme::GREEN))
                    .add_modifier(Modifier::BOLD),
            ));
        }
        (LedgerStatus::Verified, None) => {
            spans.push(Span::styled(
                "some internal checks are unavailable",
                Style::default().fg(color(theme::RED)),
            ));
        }
        (LedgerStatus::Legacy, _) => {
            spans.push(Span::styled(
                "older records have fewer checks",
                Style::default().fg(color(theme::AMBER)),
            ));
        }
        (LedgerStatus::Broken, _) => {
            spans.push(Span::styled(
                "internal record checks failed",
                Style::default().fg(color(theme::RED)),
            ));
        }
        (LedgerStatus::Unavailable, _) => {
            spans.push(Span::styled(
                "some record checks are unavailable",
                Style::default().fg(color(theme::INK_DIM)),
            ));
        }
    }
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

/// The bordered memory-detail pane, shared by the item view and the empty
/// state so both frame the pane identically.
fn detail_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color(theme::INK_FAINT)))
        .title(Span::styled(
            " memory details ",
            Style::default()
                .fg(color(theme::CLAY))
                .add_modifier(Modifier::BOLD),
        ))
}

fn detail_paragraph(item: &Item) -> Paragraph<'static> {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        item.kind.clone(),
        Style::default()
            .fg(color(theme::CLAY))
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::raw(item.title.clone()));
    if item.detail.trim() != item.title.trim() {
        lines.push(Line::raw(""));
        for chunk in item.detail.lines() {
            lines.push(Line::styled(
                chunk.to_string(),
                Style::default().fg(color(theme::INK_DIM)),
            ));
        }
    }
    if let Some((label, trust_color)) = &item.trust {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("status  ", Style::default().fg(color(theme::INK_FAINT))),
            Span::styled(
                label.clone(),
                Style::default()
                    .fg(color(*trust_color))
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    if item.evidence.is_some() {
        lines.push(Line::from(vec![
            Span::styled("evidence ", Style::default().fg(color(theme::INK_FAINT))),
            Span::styled(
                "linked",
                Style::default()
                    .fg(color(theme::GREEN))
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    for (key, value) in &item.meta {
        if is_internal_meta_key(key) {
            continue;
        }
        lines.push(Line::from(vec![
            Span::styled(
                format!("{key:<12}"),
                Style::default().fg(color(theme::INK_FAINT)),
            ),
            Span::styled(value.clone(), Style::default().fg(color(theme::INK_DIM))),
        ]));
    }

    Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .block(detail_block())
}

fn draw_footer(
    frame: &mut Frame,
    area: Rect,
    verdict: Verdict,
    corner_mascot: Option<&mascot::RasterMascot>,
    mascot_tick: usize,
    input_mode: InputMode,
    search_query: &str,
) {
    let footer_width = area
        .width
        .saturating_sub(if corner_mascot.is_some() { 9 } else { 0 });
    let footer = Paragraph::new(footer_line(input_mode, search_query, footer_width));
    if corner_mascot.is_none()
        || area.height < mascot::MINI_SLOT_HEIGHT
        || area.width < mascot::MINI_WIDTH + 20
    {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(9)])
            .split(area);
        frame.render_widget(footer, columns[0]);
        frame.render_widget(
            Paragraph::new(Line::from(mascot::compact_mark(verdict))).alignment(Alignment::Right),
            columns[1],
        );
        return;
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(mascot::MINI_WIDTH)])
        .split(area);
    frame.render_widget(footer, columns[0]);
    let image_area = Rect::new(
        columns[1].x,
        columns[1].y + mascot::motion_offset(mascot_tick),
        mascot::MINI_WIDTH,
        mascot::MINI_HEIGHT,
    );
    frame.render_widget(
        corner_mascot
            .expect("the raster mascot was checked above")
            .frame(),
        image_area,
    );
}

fn footer_line(input_mode: InputMode, search_query: &str, width: u16) -> Line<'static> {
    if input_mode == InputMode::Search {
        let query_width = usize::from(width.saturating_sub(15));
        let query = clip(search_query, query_width);
        return Line::from(vec![
            Span::styled("  search ", Style::default().fg(color(theme::CLAY))),
            Span::styled(
                format!("{query}_ "),
                Style::default().fg(color(theme::INK_DIM)),
            ),
            Span::styled("Esc", Style::default().fg(color(theme::CLAY))),
        ]);
    }

    if width < 70 {
        return Line::from(vec![
            Span::styled("  ↑↓ ", Style::default().fg(color(theme::CLAY))),
            Span::styled("move  ", Style::default().fg(color(theme::INK_FAINT))),
            Span::styled("/ ", Style::default().fg(color(theme::CLAY))),
            Span::styled("search  ", Style::default().fg(color(theme::INK_FAINT))),
            Span::styled("q ", Style::default().fg(color(theme::CLAY))),
            Span::styled("quit", Style::default().fg(color(theme::INK_FAINT))),
        ]);
    }

    Line::from(vec![
        Span::styled("  ↑↓/jk ", Style::default().fg(color(theme::CLAY))),
        Span::styled("move   ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled("tab ", Style::default().fg(color(theme::CLAY))),
        Span::styled("filter   ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled("/ ", Style::default().fg(color(theme::CLAY))),
        Span::styled("search   ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled("q/Esc ", Style::default().fg(color(theme::CLAY))),
        Span::styled("quit", Style::default().fg(color(theme::INK_FAINT))),
    ])
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    use super::*;

    /// Render the whole cockpit into a `TestBackend` of the given size and hand
    /// back its buffer, so a test can inspect both the drawn text ([`text_of`])
    /// and the empty-state half-block art it contains ([`block_glyphs`]). Raster
    /// graphics are negotiated only by the real terminal, never TestBackend.
    fn render_cockpit(app: &mut App, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    /// Every cell symbol concatenated — enough to assert what text is drawn.
    fn text_of(buf: &Buffer) -> String {
        buf.content().iter().map(|cell| cell.symbol()).collect()
    }

    /// The count of half-block glyphs on screen. Only the full empty-state
    /// mascot paints these; the real corner mascot uses a terminal image.
    fn block_glyphs(buf: &Buffer) -> usize {
        buf.content()
            .iter()
            .filter(|cell| matches!(cell.symbol(), "\u{2580}" | "\u{2584}" | "\u{2588}"))
            .count()
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

    fn type_search(app: &mut App, query: &str) {
        assert_eq!(app.handle_key(KeyCode::Char('/')), AppAction::Continue);
        for character in query.chars() {
            assert_eq!(
                app.handle_key(KeyCode::Char(character)),
                AppAction::Continue
            );
        }
    }

    fn snapshot(n: usize) -> Snapshot {
        Snapshot {
            store_trust_label: if n == 0 {
                "EMPTY · ready for the first memory".to_string()
            } else {
                "CERTIFY · evidence checks passed".to_string()
            },
            store_trust_color: if n == 0 { theme::INK_DIM } else { theme::GREEN },
            integrity: Integrity {
                records: n,
                checksums_valid: true,
                sequence_contiguous: true,
                status: if n == 0 {
                    LedgerStatus::Empty
                } else {
                    LedgerStatus::Verified
                },
                merkle_root: (n > 0).then(|| "a".repeat(64)),
            },
            anchor: Anchor {
                status: AnchorStatus::NotChecked,
                newer_updates: 0,
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

    #[test]
    fn animation_tick_advances_without_input() {
        let mut app = App::new(snapshot(1));
        assert_eq!(app.mascot_tick, 0);
        app.on_tick();
        assert_eq!(app.mascot_tick, 1);
    }

    fn mixed() -> Snapshot {
        Snapshot {
            store_trust_label: "ADVISORY · use with context".to_string(),
            store_trust_color: theme::BLUE,
            integrity: Integrity {
                records: 4,
                checksums_valid: true,
                sequence_contiguous: true,
                status: LedgerStatus::Verified,
                merkle_root: Some("b".repeat(64)),
            },
            anchor: Anchor {
                status: AnchorStatus::NotChecked,
                newer_updates: 0,
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
    fn search_mode_treats_q_as_input_and_escape_as_a_mode_boundary() {
        let mut app = App::new(mixed());
        assert_eq!(app.input_mode, InputMode::Browse);

        assert_eq!(app.handle_key(KeyCode::Char('/')), AppAction::Continue);
        assert_eq!(app.input_mode, InputMode::Search);
        assert_eq!(app.handle_key(KeyCode::Char('q')), AppAction::Continue);
        assert_eq!(app.search_query, "q");

        assert_eq!(app.handle_key(KeyCode::Esc), AppAction::Continue);
        assert_eq!(app.input_mode, InputMode::Browse);
        assert_eq!(app.search_query, "q");
        assert_eq!(app.handle_key(KeyCode::Esc), AppAction::Quit);
    }

    #[test]
    fn search_and_kind_filter_are_combined_and_selection_remains_safe() {
        let mut app = App::new(mixed());
        type_search(&mut app, "episode");
        assert_eq!(app.visible.len(), 2);
        assert_eq!(app.list.selected(), Some(0));

        app.cycle_filter(2); // all -> episode -> claim
        assert_eq!(app.filter_label(), "claim");
        assert!(app.visible.is_empty());
        assert_eq!(app.list.selected(), None);
        assert!(app.selected_item().is_none());

        app.cycle_filter(-1);
        assert_eq!(app.filter_label(), "episode");
        assert_eq!(app.visible.len(), 2);
        assert_eq!(app.list.selected(), Some(0));
    }

    #[test]
    fn search_matches_every_local_item_field_case_insensitively() {
        let mut state = snapshot(0);
        state.items = vec![
            Item {
                kind: "claim".to_string(),
                title: "TitleNeedle".to_string(),
                detail: "DetailNeedle".to_string(),
                trust: None,
                evidence: Some("EvidenceNeedle".to_string()),
                meta: vec![("MetaKey".to_string(), "MetaValue".to_string())],
            },
            item("unrelated"),
        ];

        for query in [
            "CLAIM",
            "titleneedle",
            "detailneedle",
            "evidence",
            "metakey",
            "metavalue",
        ] {
            let mut app = App::new(Snapshot {
                store_trust_label: state.store_trust_label.clone(),
                store_trust_color: state.store_trust_color,
                integrity: Integrity {
                    records: state.integrity.records,
                    checksums_valid: state.integrity.checksums_valid,
                    sequence_contiguous: state.integrity.sequence_contiguous,
                    status: state.integrity.status,
                    merkle_root: state.integrity.merkle_root.clone(),
                },
                anchor: Anchor {
                    status: state.anchor.status,
                    newer_updates: state.anchor.newer_updates,
                },
                items: vec![
                    Item {
                        kind: "claim".to_string(),
                        title: "TitleNeedle".to_string(),
                        detail: "DetailNeedle".to_string(),
                        trust: None,
                        evidence: Some("EvidenceNeedle".to_string()),
                        meta: vec![("MetaKey".to_string(), "MetaValue".to_string())],
                    },
                    item("unrelated"),
                ],
                signals: Vec::new(),
            });
            type_search(&mut app, query);
            assert_eq!(app.visible, vec![0], "query {query} should match item 0");
        }
    }

    #[test]
    fn backspace_recovers_results_after_an_empty_search() {
        let mut app = App::new(mixed());
        type_search(&mut app, "episodex");
        assert!(app.visible.is_empty());
        assert_eq!(app.list.selected(), None);

        assert_eq!(app.handle_key(KeyCode::Backspace), AppAction::Continue);
        assert_eq!(app.search_query, "episode");
        assert_eq!(app.visible.len(), 2);
        assert_eq!(app.list.selected(), Some(0));
        assert_eq!(app.handle_key(KeyCode::Enter), AppAction::Continue);
        assert_eq!(app.input_mode, InputMode::Browse);
        assert_eq!(app.visible.len(), 2);
    }

    #[test]
    fn header_renders_memory_history_and_external_check_as_separate_states() {
        let mut state = snapshot(3);
        state.anchor = Anchor {
            status: AnchorStatus::TrustedCurrent,
            newer_updates: 0,
        };
        let mut app = App::new(state);
        let text = text_of(&render_cockpit(&mut app, 120, 40));

        assert!(text.contains("MEMORY"));
        assert!(text.contains("CERTIFY"));
        assert!(text.contains("HISTORY"));
        assert!(text.contains("CHECKS PASS"));
        assert!(text.contains("EXTERNAL"));
        assert!(text.contains("CURRENT"));
        assert!(text.contains("covers the memory shown now"));
    }

    #[test]
    fn default_tui_copy_hides_implementation_vocabulary() {
        let history_states = [
            LedgerStatus::Empty,
            LedgerStatus::Verified,
            LedgerStatus::Legacy,
            LedgerStatus::Broken,
            LedgerStatus::Unavailable,
        ];
        let proof_states = [
            AnchorStatus::NotChecked,
            AnchorStatus::TrustedCurrent,
            AnchorStatus::TrustedHistorical,
            AnchorStatus::Untrusted,
            AnchorStatus::Invalid,
            AnchorStatus::Unavailable,
        ];
        let forbidden = [
            "merkle",
            "ledger",
            "checksum",
            "sequence",
            "hash",
            "checkpoint",
            "policy",
            "signature",
            "tree size",
            "anchor",
            "score",
        ];

        for history_status in history_states {
            for proof_status in proof_states {
                let mut state = snapshot(3);
                state.integrity.status = history_status;
                state.anchor = Anchor {
                    status: proof_status,
                    newer_updates: 1,
                };
                let mut app = App::new(state);
                let text = text_of(&render_cockpit(&mut app, 120, 40)).to_lowercase();
                for term in forbidden {
                    assert!(
                        !text.contains(term),
                        "default TUI exposed implementation term {term:?}: {text}"
                    );
                }
            }
        }
    }

    #[test]
    fn proof_copy_explains_every_coverage_state() {
        let cases = [
            (
                AnchorStatus::NotChecked,
                0,
                "optional external check not provided",
            ),
            (
                AnchorStatus::TrustedCurrent,
                0,
                "covers the memory shown now",
            ),
            (
                AnchorStatus::TrustedHistorical,
                1,
                "1 newer update is not covered",
            ),
            (
                AnchorStatus::Untrusted,
                0,
                "external check was not accepted",
            ),
            (
                AnchorStatus::Invalid,
                0,
                "external check could not be completed",
            ),
            (
                AnchorStatus::Unavailable,
                0,
                "external checks are unavailable in this build",
            ),
        ];

        for (status, newer_updates, expected) in cases {
            let anchor = Anchor {
                status,
                newer_updates,
            };
            assert_eq!(proof_detail(&anchor), expected);
        }
    }

    #[test]
    fn default_item_details_hide_internal_identifiers() {
        let mut state = snapshot(1);
        state.items = vec![Item {
            kind: "claim".to_string(),
            title: "A supported decision".to_string(),
            detail: "The release owner is Lena.".to_string(),
            trust: Some(("with evidence".to_string(), theme::GREEN)),
            evidence: Some("episode_private_123".to_string()),
            meta: vec![
                ("id".to_string(), "claim_private_456".to_string()),
                (
                    "source_episode_id".to_string(),
                    "episode_private_123".to_string(),
                ),
                ("scope".to_string(), "Project(Private)".to_string()),
                ("conf".to_string(), "0.95".to_string()),
                ("context".to_string(), "Project(Nahuali)".to_string()),
            ],
        }];
        let mut app = App::new(state);
        let text = text_of(&render_cockpit(&mut app, 120, 40));

        assert!(text.contains("evidence linked"));
        assert!(text.contains("context"));
        assert!(!text.contains("episode_private_123"));
        assert!(!text.contains("claim_private_456"));
        assert!(!text.contains("Project(Private)"));

        type_search(&mut app, "private");
        assert!(app.visible.is_empty());
    }

    #[test]
    fn earlier_and_rejected_proof_badges_have_non_green_risk_colors() {
        let (historical_label, historical_color) = proof_badge(AnchorStatus::TrustedHistorical);
        assert_eq!(historical_label, "! EARLIER STATE");
        assert_eq!(historical_color, theme::AMBER);
        assert_ne!(historical_color, theme::GREEN);

        let (untrusted_label, untrusted_color) = proof_badge(AnchorStatus::Untrusted);
        assert_eq!(untrusted_label, "\u{2717} NOT ACCEPTED");
        assert_eq!(untrusted_color, theme::RED);
        assert_ne!(untrusted_color, theme::GREEN);

        let mut state = snapshot(3);
        state.anchor = Anchor {
            status: AnchorStatus::TrustedHistorical,
            newer_updates: 2,
        };
        let mut app = App::new(state);
        let text = text_of(&render_cockpit(&mut app, 120, 40));
        assert!(text.contains("EARLIER STATE"));
        assert!(text.contains("2 newer updates are not covered"));
    }

    #[test]
    fn narrow_search_render_keeps_all_governance_labels_and_compact_controls() {
        let mut state = snapshot(3);
        state.anchor = Anchor {
            status: AnchorStatus::Untrusted,
            newer_updates: 0,
        };
        let mut app = App::new(state);
        type_search(&mut app, "episode");
        let text = text_of(&render_cockpit(&mut app, 42, 30));

        assert!(text.contains("MEMORY"));
        assert!(text.contains("HISTORY"));
        assert!(text.contains("EXTERNAL"));
        assert!(text.contains("search"));
        assert!(text.contains("Esc"));
        assert!(text.contains("≋(•ᴗ•)≋"));
    }

    #[test]
    fn empty_store_shows_the_full_mascot_in_a_roomy_detail_pane() {
        // With nothing selected and room to spare, the nahual takes over the
        // detail pane: its EMPTY caption ("waiting") and a dense sprite appear.
        let mut app = App::new(snapshot(0));
        let buf = render_cockpit(&mut app, 120, 40);
        assert!(
            text_of(&buf).contains("waiting"),
            "EMPTY caption missing from the empty detail pane"
        );
        assert!(
            block_glyphs(&buf) > 40,
            "the full nahual sprite should fill the empty detail pane"
        );
    }

    #[test]
    fn empty_store_on_a_tiny_terminal_draws_the_compact_fallback() {
        // Below the empty-state threshold the pane falls back to the plain
        // placeholder. The corner mascot remains visible and nothing panics.
        let mut app = App::new(snapshot(0));
        let buf = render_cockpit(&mut app, 40, 24);
        let text = text_of(&buf);
        assert!(
            !text.contains("waiting"),
            "no mascot caption should appear on a tiny terminal"
        );
        assert_eq!(block_glyphs(&buf), 0);
        assert!(text.contains("≋(•ᴗ•)≋"), "compact mascot fallback missing");
        assert!(
            text.contains("No memory"),
            "the plain placeholder should show instead"
        );
    }

    #[test]
    fn non_empty_test_backend_shows_the_compact_corner_fallback() {
        // Unit tests have no terminal graphics negotiation. They prove the safe
        // fallback while live Ghostty QA proves the high-resolution raster path.
        let mut app = App::new(snapshot(3));
        let buf = render_cockpit(&mut app, 120, 40);
        let text = text_of(&buf);
        assert_eq!(block_glyphs(&buf), 0);
        assert!(text.contains("≋(•ᴗ•)≋"));
        assert!(!text.contains("ready · evidence checked"));
    }

    #[test]
    fn narrow_terminal_keeps_the_compact_mascot_and_layout() {
        // The fallback survives narrow layouts without colliding with useful
        // information.
        let mut app = App::new(snapshot(3));
        let buf = render_cockpit(&mut app, 42, 30);
        let text = text_of(&buf);
        assert_eq!(block_glyphs(&buf), 0);
        assert!(text.contains("≋(•ᴗ•)≋"));
        assert!(!text.contains("ready · evidence checked"));
        assert!(text.contains("Memory Explorer"), "header still renders");
        assert!(text.contains("memory"), "list pane still renders");
    }

    #[test]
    fn history_header_distinguishes_every_integrity_state() {
        let cases = [
            (LedgerStatus::Empty, "EMPTY"),
            (LedgerStatus::Verified, "CHECKS PASS"),
            (LedgerStatus::Legacy, "LIMITED"),
            (LedgerStatus::Broken, "PROBLEM"),
            (LedgerStatus::Unavailable, "PARTIAL"),
        ];

        for (status, expected) in cases {
            let mut state = snapshot(1);
            state.integrity.status = status;
            let mut app = App::new(state);
            let text = text_of(&render_cockpit(&mut app, 120, 40));
            assert!(
                text.contains(expected),
                "history status {status:?} must render as {expected}"
            );
        }
    }

    #[test]
    fn checksum_or_sequence_failure_dominates_a_legacy_chain_label() {
        for (checksums_valid, sequence_contiguous) in [(false, true), (true, false)] {
            let mut state = snapshot(1);
            state.integrity.status = LedgerStatus::Legacy;
            state.integrity.checksums_valid = checksums_valid;
            state.integrity.sequence_contiguous = sequence_contiguous;
            let mut app = App::new(state);
            let text = text_of(&render_cockpit(&mut app, 120, 40));
            assert!(text.contains("PROBLEM"));
            assert!(!text.contains("HISTORY ! LIMITED"));
        }
    }

    #[test]
    fn pose_tracks_the_verdict_in_both_the_mark_and_the_empty_state() {
        // Empty BLOCK store: the full mascot wears the paused pose, never the
        // ready one — the empty-state art is bound to the live verdict.
        let mut empty = snapshot(0);
        empty.store_trust_label = "BLOCK · do not use yet".to_string();
        empty.store_trust_color = theme::RED;
        let mut app = App::new(empty);
        let text = text_of(&render_cockpit(&mut app, 120, 40));
        assert!(
            text.contains("paused"),
            "empty BLOCK store shows the paused pose"
        );
        assert!(
            !text.contains("ready · evidence checked"),
            "must not show the ready caption for a BLOCK store"
        );

        // Non-empty BLOCK store without terminal graphics: the compact fallback
        // switches expression while the full half-block sprite remains absent.
        let mut full = snapshot(3);
        full.store_trust_label = "BLOCK · do not use yet".to_string();
        full.store_trust_color = theme::RED;
        let mut app = App::new(full);
        let buf = render_cockpit(&mut app, 120, 40);
        assert_eq!(block_glyphs(&buf), 0);
        assert!(text_of(&buf).contains("≋(—_—)≋"));
    }
}
