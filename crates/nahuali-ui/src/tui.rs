//! The interactive `explore` governance cockpit (ratatui).
//!
//! Nahuali is agent-first, but a human supervises the agent's memory — this is
//! that window. It renders a trust-first browse of what the agent stored: the
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

/// The ledger's tamper-evidence posture — Nahuali's core differentiator,
/// surfaced independently from the content authority verdict.
pub struct Integrity {
    pub records: usize,
    pub checksums_valid: bool,
    pub sequence_contiguous: bool,
    pub status: LedgerStatus,
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
            mascot_images: mascot::MascotImages::halfblocks(verdict).ok(),
            mascot_tick: 0,
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

    fn on_tick(&mut self) {
        self.mascot_tick = self.mascot_tick.wrapping_add(1);
    }
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
            Constraint::Length(4),
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
    let list = item_list(&visible, title_width, &app.filter_label());

    frame.render_stateful_widget(list, body[0], &mut app.list);
    draw_detail(frame, app, body[1]);

    draw_signals(frame, &app.snapshot.signals, rows[2]);
    let verdict = Verdict::from_label(&app.snapshot.store_trust_label);
    draw_footer(frame, rows[3], verdict, corner_mascot, app.mascot_tick);
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

    let hint = "the nahual mirrors store trust";
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
    let title = vec![Span::styled(
        format!(" nahuali explore · {} ", app.snapshot.database),
        Style::default()
            .fg(color(theme::CLAY))
            .add_modifier(Modifier::BOLD),
    )];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color(theme::INK_FAINT)))
        .title(Line::from(title));
    let header = Paragraph::new(vec![trust, integrity_line(&app.snapshot.integrity)]).block(block);
    frame.render_widget(header, area);
}

/// The integrity line keeps a fully chained ledger, legacy unchained history,
/// a broken chain, and a build without tamper evidence visibly distinct.
fn integrity_line(integrity: &Integrity) -> Line<'static> {
    let (verdict, verdict_color) = ledger_badge(integrity);
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
    if !integrity.checksums_valid {
        spans.push(Span::styled(
            "checksum mismatch",
            Style::default().fg(color(theme::RED)),
        ));
    } else if !integrity.sequence_contiguous {
        spans.push(Span::styled(
            "sequence gap or reordering",
            Style::default().fg(color(theme::RED)),
        ));
    } else if integrity.status == LedgerStatus::Verified
        && integrity.records > 0
        && integrity.merkle_root.is_none()
    {
        spans.push(Span::styled(
            "Merkle commitment unavailable",
            Style::default().fg(color(theme::RED)),
        ));
    } else {
        append_integrity_detail(&mut spans, integrity);
    }
    Line::from(spans)
}

fn ledger_badge(integrity: &Integrity) -> (&'static str, Rgb) {
    if !integrity.checksums_valid
        || !integrity.sequence_contiguous
        || (integrity.status == LedgerStatus::Verified
            && integrity.records > 0
            && integrity.merkle_root.is_none())
    {
        return ("\u{2717} FAILED", theme::RED);
    }

    match integrity.status {
        LedgerStatus::Empty => ("o EMPTY", theme::INK_DIM),
        LedgerStatus::Verified => ("\u{2713} VERIFIED", theme::GREEN),
        LedgerStatus::Legacy => ("! LEGACY", theme::AMBER),
        LedgerStatus::Broken => ("\u{2717} BROKEN", theme::RED),
        LedgerStatus::Unavailable => ("! CHECKSUMMED", theme::AMBER),
    }
}

fn append_integrity_detail(spans: &mut Vec<Span<'static>>, integrity: &Integrity) {
    match (integrity.status, &integrity.merkle_root) {
        (LedgerStatus::Empty, _) => {
            spans.push(Span::styled(
                "no records · hash chain ready",
                Style::default().fg(color(theme::INK_DIM)),
            ));
        }
        (LedgerStatus::Verified, Some(root)) => {
            let short: String = root.chars().take(10).collect();
            spans.push(Span::styled(
                "tamper-evident",
                Style::default()
                    .fg(color(theme::GREEN))
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                format!(" · merkle {short}\u{2026}"),
                Style::default().fg(color(theme::INK_FAINT)),
            ));
        }
        (LedgerStatus::Verified, None) => {
            spans.push(Span::styled(
                "empty · hash chain ready",
                Style::default().fg(color(theme::INK_DIM)),
            ));
        }
        (LedgerStatus::Legacy, _) => {
            spans.push(Span::styled(
                "checksummed · unchained history",
                Style::default().fg(color(theme::AMBER)),
            ));
        }
        (LedgerStatus::Broken, _) => {
            spans.push(Span::styled(
                "hash chain mismatch",
                Style::default().fg(color(theme::RED)),
            ));
        }
        (LedgerStatus::Unavailable, _) => {
            spans.push(Span::styled(
                "per-record integrity",
                Style::default().fg(color(theme::INK_DIM)),
            ));
            spans.push(Span::styled(
                " · hash chain off",
                Style::default().fg(color(theme::AMBER)),
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

/// The bordered `detail` pane block, shared by the item view and the empty
/// state so both frame the pane identically.
fn detail_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color(theme::INK_FAINT)))
        .title(Span::styled(
            " detail ",
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
) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("  ↑↓/jk ", Style::default().fg(color(theme::CLAY))),
        Span::styled("move   ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled("tab ", Style::default().fg(color(theme::CLAY))),
        Span::styled("filter   ", Style::default().fg(color(theme::INK_FAINT))),
        Span::styled("q/Esc ", Style::default().fg(color(theme::CLAY))),
        Span::styled("quit", Style::default().fg(color(theme::INK_FAINT))),
    ]));
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

    fn snapshot(n: usize) -> Snapshot {
        Snapshot {
            database: "memory".to_string(),
            store_trust_label: "CERTIFY".to_string(),
            store_trust_color: theme::GREEN,
            store_trust_score: 1.0,
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
            database: "memory".to_string(),
            store_trust_label: "ADVISORY".to_string(),
            store_trust_color: theme::BLUE,
            store_trust_score: 0.75,
            integrity: Integrity {
                records: 4,
                checksums_valid: true,
                sequence_contiguous: true,
                status: LedgerStatus::Verified,
                merkle_root: Some("b".repeat(64)),
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
    fn empty_store_shows_the_full_mascot_in_a_roomy_detail_pane() {
        // With nothing selected and room to spare, the nahual takes over the
        // detail pane: its CERTIFY caption ("calm") and a dense sprite appear.
        let mut app = App::new(snapshot(0));
        let buf = render_cockpit(&mut app, 120, 40);
        assert!(
            text_of(&buf).contains("calm"),
            "CERTIFY caption missing from the empty detail pane"
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
            !text.contains("calm"),
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
        assert!(!text.contains("calm"));
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
        assert!(!text.contains("calm"));
        assert!(text.contains("nahuali explore"), "header still renders");
        assert!(text.contains("memory"), "list pane still renders");
    }

    #[test]
    fn ledger_header_distinguishes_every_ledger_state() {
        let cases = [
            (LedgerStatus::Empty, "EMPTY"),
            (LedgerStatus::Verified, "VERIFIED"),
            (LedgerStatus::Legacy, "LEGACY"),
            (LedgerStatus::Broken, "BROKEN"),
            (LedgerStatus::Unavailable, "CHECKSUMMED"),
        ];

        for (status, expected) in cases {
            let mut state = snapshot(1);
            state.integrity.status = status;
            let mut app = App::new(state);
            let text = text_of(&render_cockpit(&mut app, 120, 40));
            assert!(
                text.contains(expected),
                "ledger status {status:?} must render as {expected}"
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
            assert!(text.contains("FAILED"));
            assert!(!text.contains("Ledger ! LEGACY"));
        }
    }

    #[test]
    fn pose_tracks_the_verdict_in_both_the_mark_and_the_empty_state() {
        // Empty BLOCK store: the full mascot wears the guarded pose, never the
        // CERTIFY one — the empty-state art is bound to the live verdict.
        let mut empty = snapshot(0);
        empty.store_trust_label = "BLOCK · not yet trustworthy".to_string();
        empty.store_trust_color = theme::RED;
        let mut app = App::new(empty);
        let text = text_of(&render_cockpit(&mut app, 120, 40));
        assert!(
            text.contains("guarded"),
            "empty BLOCK store shows the guarded pose"
        );
        assert!(
            !text.contains("calm"),
            "must not show the CERTIFY caption for a BLOCK store"
        );

        // Non-empty BLOCK store without terminal graphics: the compact fallback
        // switches expression while the full half-block sprite remains absent.
        let mut full = snapshot(3);
        full.store_trust_label = "BLOCK · not yet trustworthy".to_string();
        full.store_trust_color = theme::RED;
        let mut app = App::new(full);
        let buf = render_cockpit(&mut app, 120, 40);
        assert_eq!(block_glyphs(&buf), 0);
        assert!(text_of(&buf).contains("≋(—_—)≋"));
    }
}
