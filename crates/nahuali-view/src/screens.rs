//! Builders for the v0 screens. Each takes the data source and the UI state
//! and returns a complete [`Screen`]; nothing here depends on terminal size.

use crate::date::{Date, Stamp};
use crate::keymap::{self, View};
use crate::model::{Icon, Item, MascotState, Node, Screen, Seg, Span, Tone};
use crate::source::{
    Briefing, ChainStatus, Decision, DecisionCandidate, Entry, Extensions, HealthLine, Intention,
    IntentionStatus, JobRun, LoopReport, PendingUpdate, Priority, Proposal, RecordDetail, Verdict,
    ViewSource,
};
use crate::state::UiState;

const TITLE: &str = "nahuali";

/// Build the screen for `ui.view`; a view the source does not have shows Today.
pub fn build(src: &dyn ViewSource, ui: &UiState) -> Screen {
    if !ui.view.available(Extensions::of(src)) {
        return self::today(src, &ui.on(View::Today));
    }
    match ui.view {
        View::Today => today(src, ui),
        View::Journal => journal(src, ui),
        View::Intentions => intentions(src, ui),
        View::Health => health(src, ui),
        View::Search => search(src, ui),
        View::Proposals => proposals(src, ui),
    }
}

fn screen(src: &dyn ViewSource, ui: &UiState, body: Node, status: Vec<Seg>) -> Screen {
    let ext = Extensions::of(src);
    let mut status = status;
    if let Some(notice) = &ui.notice {
        status.insert(0, Seg::new(None, notice.clone(), Tone::Accent, 0));
    }
    let mut keys = keymap::footer_hints(ui.view, ui.demo, ext);
    if let Some(p) = &ui.prompt {
        let verb = match p.verdict {
            Verdict::Approve => "approve",
            Verdict::Reject => "reject",
            Verdict::Defer => "defer",
        };
        let need = if p.verdict == Verdict::Reject { "reason" } else { "reason (optional)" };
        status
            .insert(0, Seg::new(None, format!("{verb} {} · {need}: {}▏", p.target, p.reason), Tone::Warn, 0));
        keys = keymap::PROMPT
            .iter()
            .filter_map(|b| b.hint)
            .map(|(key, label)| crate::model::KeyHint { key: key.into(), label: label.into() })
            .collect();
    }
    let who = src.principal().badge();
    Screen {
        title: format!("{TITLE} · {who} · {}", ui.view.title()),
        who,
        tabs: View::shown(ext).iter().map(|v| v.title().to_string()).collect(),
        active_tab: ui.view.tab(ext),
        body,
        status,
        keys,
    }
}

// ---------------------------------------------------------------------------
// Mind state: which face the axolotl shows and why.

/// The axolotl's state and its one-line reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mood {
    pub state: MascotState,
    pub reason: String,
}

impl Mood {
    fn new(state: MascotState, reason: impl Into<String>) -> Self {
        Self { state, reason: reason.into() }
    }

    /// `worried · 2 overdue`.
    pub fn caption(&self) -> String {
        format!("{} · {}", self.state.name(), self.reason)
    }
}

/// Pick the highest-priority mood among the active signals.
pub fn arbitrate(signals: Vec<Mood>) -> Mood {
    signals
        .into_iter()
        .max_by_key(|mood| mood.state.priority())
        .unwrap_or_else(|| Mood::new(MascotState::Idle, "resting"))
}

fn overdue(intentions: &[Intention], today: Date) -> Vec<&Intention> {
    let mut list: Vec<&Intention> = intentions
        .iter()
        .filter(|i| i.status == IntentionStatus::Open && i.due.is_some_and(|d| d < today))
        .collect();
    list.sort_by_key(|i| i.due);
    list
}

/// The moods the briefing alone justifies.
pub fn briefing_moods(b: &Briefing, now: Stamp) -> Vec<Mood> {
    let mut moods = Vec::new();
    if let ChainStatus::Broken { at_seq } = b.health.chain {
        moods.push(Mood::new(MascotState::Alarmed, format!("chain breaks at seq {at_seq}")));
    }
    let all: Vec<Intention> = b.open.iter().chain(&b.stale).cloned().collect();
    let late = overdue(&all, now.date).len();
    // Stale (untouched for a month) and not already counted as overdue.
    let stale = b.stale.iter().filter(|i| !i.due.is_some_and(|d| d < now.date)).count();
    let reviews = b.decisions_due.len();
    // What stopped working comes first: it is the reason to look at all.
    let mut worries = b.health.alarms.clone();
    if late > 0 {
        worries.push(format!("{late} overdue"));
    }
    if stale > 0 {
        worries.push(format!("{stale} stale"));
    }
    match (worries.is_empty(), reviews) {
        (true, 0) => {}
        (true, r) => worries.push(format!("{r} decision{} to review", plural(r))),
        (false, 0) => {}
        (false, r) => worries.push(format!("{r} review")),
    }
    if !worries.is_empty() {
        moods.push(Mood::new(MascotState::Worried, worries.join(" · ")));
    }
    // An empty mind rests; any recorded event makes a clear day happy.
    if b.health.events > 0 {
        moods.push(Mood::new(MascotState::Happy, "all clear"));
    }
    moods
}

fn resolve_mood(ui: &UiState, signals: Vec<Mood>) -> Mood {
    match ui.mascot_override {
        Some(state) => Mood::new(state, "demo"),
        None => arbitrate(signals),
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn mascot_seg(mood: &Mood) -> Seg {
    Seg::new(Some(Icon::Mascot(mood.state)), mood.caption(), Tone::Neutral, 0)
}

fn job_seg(name: &str, job: Option<&JobRun>, priority: u8) -> Option<Seg> {
    let job = job?;
    Some(if job.ok {
        Seg::new(Some(Icon::Ok), format!("{name} {} ok", job.at.clock()), Tone::Neutral, priority)
    } else {
        Seg::new(Some(Icon::Fail), format!("{name} failed {}", job.at.clock()), Tone::Warn, 1)
    })
}

/// The health facts row shared by every screen.
fn health_segs(line: &HealthLine, now: Stamp) -> Vec<Seg> {
    let mut segs = Vec::new();
    segs.push(match line.chain {
        ChainStatus::Verified { .. } => Seg::new(Some(Icon::Ok), "chain ok", Tone::Neutral, 1),
        ChainStatus::Broken { at_seq } => {
            Seg::new(Some(Icon::Fail), format!("chain breaks at {at_seq}"), Tone::Danger, 0)
        }
        ChainStatus::Unknown => Seg::new(Some(Icon::Idle), "chain unchecked", Tone::Warn, 1),
    });
    segs.extend(job_seg("export", line.export.as_ref(), 3));
    if let Some(last) = line.ingest_last {
        let minutes = now.minutes() - last.minutes();
        segs.push(if minutes <= 10 {
            Seg::new(Some(Icon::Live), "ingest live", Tone::Neutral, 2)
        } else {
            Seg::new(
                Some(Icon::Idle),
                format!("ingest {} ago", last.age(now)),
                if minutes > 48 * 60 { Tone::Warn } else { Tone::Muted },
                2,
            )
        });
    }
    segs
}

// ---------------------------------------------------------------------------
// Today

/// What a "Needs you" row points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Need {
    Intention { id: String, text: String },
    Decision { id: String, decided_on: Date },
    Candidate { id: String, question: String },
    Update,
}

/// The rows of "Needs attention" for `src`, extensions included.
pub fn needs_of(src: &dyn ViewSource, now: Stamp) -> Vec<(Item, Need)> {
    let candidates = src.candidates().map(|c| c.pending()).unwrap_or_default();
    let update = src.updates().and_then(|u| u.staged());
    needs_you(&src.briefing(), &candidates, update.as_ref(), now)
}

/// The rows of "Needs attention", in display order, with what each points
/// at. `candidates` and `update` come from extensions and may be empty.
pub fn needs_you(
    b: &Briefing,
    candidates: &[DecisionCandidate],
    update: Option<&PendingUpdate>,
    now: Stamp,
) -> Vec<(Item, Need)> {
    let today = now.date;
    let all: Vec<Intention> = b.open.iter().chain(&b.stale).cloned().collect();
    let mut rows = Vec::new();
    if let Some(u) = update {
        rows.push((
            Item::new(vec![Span::plain(format!("Update ready: {}", u.headline))])
                .with_lead(Span::plain("↑"))
                .with_meta(vec![Span::new("u install", Tone::Accent)]),
            Need::Update,
        ));
    }
    for i in overdue(&all, today) {
        let late = i.due.map_or(0, |d| d.days_until(today));
        rows.push((
            Item::new(vec![Span::plain(&i.text)])
                .with_lead(Span::plain("□"))
                .with_meta(vec![Span::new(format!("{late}d overdue"), Tone::Warn)]),
            Need::Intention { id: i.id.clone(), text: i.text.clone() },
        ));
    }
    for c in candidates {
        rows.push((
            Item::new(vec![Span::plain(format!("Decided? {} → {}", c.question, c.chosen))])
                .with_lead(Span::plain("◇"))
                .with_meta(vec![Span::new("a accept · x dismiss", Tone::Accent)]),
            Need::Candidate { id: c.id.clone(), question: c.question.clone() },
        ));
    }
    for d in &b.decisions_due {
        rows.push((
            Item::new(vec![Span::plain(format!("Review: {}", d.question))])
                .with_lead(Span::plain("□"))
                .with_meta(vec![Span::new("review due", Tone::Warn)]),
            Need::Decision { id: d.id.clone(), decided_on: d.decided_at.date },
        ));
    }
    // Stale work the mascot is worried about: the most important few, so the
    // card explains the mood without flooding it.
    const STALE_SHOWN: usize = 3;
    let listed: Vec<String> = rows
        .iter()
        .filter_map(|(_, need)| match need {
            Need::Intention { id, .. } => Some(id.clone()),
            Need::Decision { .. } | Need::Candidate { .. } | Need::Update => None,
        })
        .collect();
    let rank = |p: Priority| match p {
        Priority::Critical => 0,
        Priority::High => 1,
        Priority::Normal => 2,
        Priority::Low => 3,
    };
    let mut stale: Vec<&Intention> = b
        .stale
        .iter()
        .filter(|i| {
            i.status == IntentionStatus::Open
                && matches!(i.priority, Priority::Critical | Priority::High)
                && !listed.contains(&i.id)
        })
        .collect();
    stale.sort_by_key(|i| rank(i.priority));
    for i in stale.into_iter().take(STALE_SHOWN) {
        rows.push((
            Item::new(vec![Span::plain(&i.text)])
                .with_lead(Span::plain("□"))
                .with_meta(vec![Span::new("stale", Tone::Warn)]),
            Need::Intention { id: i.id.clone(), text: i.text.clone() },
        ));
    }
    let mut soon: Vec<&Intention> = all
        .iter()
        .filter(|i| {
            i.status == IntentionStatus::Open && i.due.is_some_and(|d| d >= today && today.days_until(d) <= 7)
        })
        .collect();
    soon.sort_by_key(|i| i.due);
    for i in soon {
        let days = i.due.map_or(0, |d| today.days_until(d));
        let when = match days {
            0 => "today".to_string(),
            1 => "tomorrow".to_string(),
            n => format!("in {n}d"),
        };
        rows.push((
            Item::new(vec![Span::plain(&i.text)])
                .with_lead(Span::plain("□"))
                .with_meta(vec![Span::muted(when)]),
            Need::Intention { id: i.id.clone(), text: i.text.clone() },
        ));
    }
    rows
}

fn today(src: &dyn ViewSource, ui: &UiState) -> Screen {
    let now = src.now();
    let b = src.briefing();
    let mood = resolve_mood(ui, briefing_moods(&b, now));

    let mut sidebar = vec![Node::Mascot { state: mood.state, caption: mood.caption() }];
    if let Some(activity) = &b.activity {
        let mut since = Vec::new();
        if activity.sessions > 0 {
            since.push(Item::new(vec![Span::plain(format!(
                "{} session{}",
                activity.sessions,
                plural(activity.sessions as usize)
            ))]));
        }
        if activity.episodes + activity.facts > 0 {
            since.push(Item::new(vec![Span::plain(format!(
                "{} episode{} · {} fact{}",
                activity.episodes,
                plural(activity.episodes as usize),
                activity.facts,
                plural(activity.facts as usize)
            ))]));
        }
        if activity.decisions_to_review > 0 {
            since.push(Item::new(vec![Span::plain(format!(
                "{} decision{} to review",
                activity.decisions_to_review,
                plural(activity.decisions_to_review as usize)
            ))]));
        }
        let body = if since.is_empty() {
            Node::Text(vec![Span::muted("Nothing new from others.")])
        } else {
            Node::List { items: since, selected: None }
        };
        let title = format!("Since {}", last_visit(activity.since, now));
        sidebar.push(Node::card(title, Tone::Accent, body));
    }
    let sidebar = Node::Col(sidebar);

    let needs: Vec<Item> = needs_of(src, now).into_iter().map(|(item, _)| item).collect();
    let needs_node = if needs.is_empty() {
        Node::Empty {
            title: "Nothing needs attention".into(),
            line: "No overdue intentions or reviews.".into(),
        }
    } else {
        let selected = ui.selected(View::Today).min(needs.len() - 1);
        Node::List { items: needs, selected: Some(selected) }
    };

    let yesterday = now.date.add_days(-1);
    const SHOWN: usize = 4;
    let mut lines: Vec<Item> = b
        .recent
        .iter()
        .take(SHOWN)
        .map(|e| Item::new(vec![Span::plain(&e.text)]).with_lead(Span::plain("●")))
        .collect();
    if b.recent.len() > SHOWN {
        lines.push(Item::new(vec![Span::muted(format!(
            "and {} more in the journal",
            b.recent.len() - SHOWN
        ))]));
    }
    let yesterday_node = if lines.is_empty() {
        Node::Empty { title: "A quiet day".into(), line: "Nothing was recorded yesterday.".into() }
    } else {
        Node::List { items: lines, selected: None }
    };

    let threads: Vec<Item> = b
        .threads
        .iter()
        .map(|t| {
            let mut meta = vec![Span::muted(t.last_touch.age(now))];
            if let Some(actor) = &t.actor {
                meta.push(Span::muted(format!(" · {actor}")));
            }
            Item::new(vec![Span::plain(format!("{}: ", t.project)), Span::plain(&t.title)])
                .with_lead(Span::plain("○"))
                .with_meta(meta)
        })
        .collect();
    let threads_node = if threads.is_empty() {
        Node::Empty { title: "No open threads".into(), line: "Projects show up here when they move.".into() }
    } else {
        Node::List { items: threads, selected: None }
    };

    let main = Node::Col(vec![
        Node::card("Needs attention", Tone::Accent, needs_node),
        Node::card(format!("Yesterday · {}", yesterday.weekday_short()), Tone::Accent, yesterday_node),
        Node::card("Open threads", Tone::Accent, threads_node),
    ]);

    let status = health_segs(&b.health, now);
    screen(src, ui, Node::Row(vec![sidebar, main]), status)
}

/// `last visit · 14h ago`: when the principal last wrote.
fn last_visit(since: Stamp, now: Stamp) -> String {
    format!("last visit · {} ago", since.age(now))
}

// ---------------------------------------------------------------------------
// Journal

fn calendar(month: Date, selected: Date, marked: &[Date]) -> Vec<Span> {
    let mut spans = vec![Span::muted("Mo Tu We Th Fr Sa Su\n")];
    let first = month.first_of_month();
    let offset = usize::from(first.weekday());
    if offset > 0 {
        spans.push(Span::plain(" ".repeat(offset * 3)));
    }
    for day in 1..=first.days_in_month() {
        let date = Date::new(first.year, first.month, day);
        let column = (offset + usize::from(day) - 1) % 7;
        let text = format!("{day:>2}");
        let span = if date == selected {
            Span::strong(text, Tone::Accent)
        } else if marked.contains(&date) {
            Span::plain(text)
        } else {
            Span::muted(text)
        };
        spans.push(span);
        if column == 6 {
            spans.push(Span::plain("\n"));
        } else if day < first.days_in_month() {
            spans.push(Span::plain(" "));
        }
    }
    spans
}

fn journal(src: &dyn ViewSource, ui: &UiState) -> Screen {
    let now = src.now();
    let b = src.briefing();
    let mood = resolve_mood(ui, briefing_moods(&b, now));

    let days = src.journal_days(now.date);
    let selected_index = ui.selected(View::Journal).min(days.len().saturating_sub(1));
    let selected = days.get(selected_index).map_or(now.date, |d| d.date);
    let marked: Vec<Date> = days.iter().map(|d| d.date).collect();

    let day_items: Vec<Item> = days
        .iter()
        .map(|d| {
            let mut marks = String::new();
            marks.push(if d.by_agents { '●' } else { ' ' });
            marks.push(' ');
            marks.push(if d.decisions > 0 { '◆' } else { ' ' });
            marks.push(' ');
            marks.push(if d.private { '∗' } else { ' ' });
            Item::new(vec![
                Span::plain(format!("{:<3} {:>2}  ", d.date.weekday_name(), d.date.day)),
                Span::new(marks, Tone::Accent),
            ])
            .with_meta(vec![Span::muted(format!("{}", d.episodes))])
        })
        .collect();

    let left = Node::card(
        format!("{} {}", selected.month_long(), selected.year),
        Tone::Accent,
        Node::Col(vec![
            Node::Text(calendar(selected, selected, &marked)),
            Node::Text(vec![Span::muted("● agents  ◆ decision  ∗ private")]),
            if day_items.is_empty() {
                Node::Empty {
                    title: "No days yet".into(),
                    line: "Entries appear as they are written.".into(),
                }
            } else {
                Node::List { items: day_items, selected: Some(selected_index) }
            },
        ]),
    );

    let right_body = match src.journal_day(selected) {
        None => Node::Empty {
            title: "Nothing recorded".into(),
            line: format!("No entries on {}.", selected.weekday_short()),
        },
        Some(day) => {
            let mut parts = Vec::new();
            if !day.sessions.is_empty() {
                parts.push(Node::Text(vec![Span::strong("Sessions", Tone::Accent)]));
                parts.push(Node::List {
                    items: day
                        .sessions
                        .iter()
                        .map(|s| {
                            Item::new(vec![Span::plain(s.project.as_deref().unwrap_or("no project"))])
                                .with_lead(Span::muted("·"))
                                .with_meta(vec![Span::muted(&s.summary)])
                        })
                        .collect(),
                    selected: None,
                });
            }
            if !day.happened.is_empty() {
                parts.push(Node::Text(vec![Span::strong("What happened", Tone::Accent)]));
                parts.push(Node::List {
                    items: day.happened.iter().map(entry_item).collect(),
                    selected: None,
                });
            }
            if !day.decisions.is_empty() {
                parts.push(Node::Text(vec![Span::strong("Decisions", Tone::Accent)]));
                parts.push(Node::List {
                    items: day.decisions.iter().map(decision_item).collect(),
                    selected: None,
                });
            }
            if !(day.opened.is_empty() && day.closed.is_empty()) {
                parts.push(Node::Text(vec![Span::strong("Intentions", Tone::Accent)]));
                let opened = day
                    .opened
                    .iter()
                    .map(|i| Item::new(vec![Span::plain(&i.text)]).with_lead(Span::new("+", Tone::Accent)));
                let closed = day
                    .closed
                    .iter()
                    .map(|i| Item::new(vec![Span::plain(&i.text)]).with_lead(Span::new("✓", Tone::Ok)));
                parts.push(Node::List { items: opened.chain(closed).collect(), selected: None });
            }
            if !day.learned.is_empty() {
                parts.push(Node::Text(vec![Span::strong("Learned", Tone::Accent)]));
                parts.push(Node::List {
                    items: day
                        .learned
                        .iter()
                        .map(|f| Item::new(vec![Span::plain(&f.text)]).with_lead(Span::muted("·")))
                        .collect(),
                    selected: None,
                });
            }
            Node::Col(parts)
        }
    };
    let right = Node::card(selected.long(), Tone::Accent, right_body);

    // The list may reach into earlier months; the total is this month's.
    let total: u32 = days
        .iter()
        .filter(|d| d.date.year == now.date.year && d.date.month == now.date.month)
        .map(|d| d.episodes)
        .sum();
    let status =
        vec![mascot_seg(&mood), Seg::new(None, format!("{total} episodes this month"), Tone::Muted, 3)];
    screen(src, ui, Node::Row(vec![left, right]), status)
}

/// A journal line: the time, the text, who and where.
fn entry_item(e: &Entry) -> Item {
    let who = e.actor.as_deref().unwrap_or("unknown");
    let place = e.project.as_deref().map(|p| format!(" · {p}")).unwrap_or_default();
    Item::new(vec![Span::plain(&e.text)])
        .with_lead(Span::muted(e.at.clock()))
        .with_meta(vec![Span::muted(format!("{who}{place}"))])
}

fn decision_item(d: &Decision) -> Item {
    let mut item = Item::new(vec![Span::plain(format!("{} → {}", d.question, d.choice))])
        .with_lead(Span::new("◆", Tone::Accent));
    if let Some(review) = d.review_on {
        item = item.with_meta(vec![Span::muted(format!("review {}", review.short()))]);
    }
    item
}

// ---------------------------------------------------------------------------
// Intentions

/// The three intention columns: overdue, open and closed this week.
pub fn intention_columns(all: &[Intention], now: Stamp) -> [Vec<Intention>; 3] {
    let today = now.date;
    let late: Vec<Intention> = overdue(all, today).into_iter().cloned().collect();
    let mut open: Vec<Intention> = all
        .iter()
        .filter(|i| i.status == IntentionStatus::Open && !i.due.is_some_and(|d| d < today))
        .cloned()
        .collect();
    open.sort_by(|a, b| match (a.due, b.due) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.priority.cmp(&a.priority),
    });
    let mut closed: Vec<Intention> = all
        .iter()
        .filter(|i| {
            i.status != IntentionStatus::Open && i.closed_at.is_some_and(|c| c.date.days_until(today) <= 7)
        })
        .cloned()
        .collect();
    closed.sort_by_key(|i| std::cmp::Reverse(i.closed_at));
    [late, open, closed]
}

fn intention_item(i: &Intention, now: Stamp) -> Item {
    let project = i.project.as_deref().unwrap_or("no project");
    match i.status {
        IntentionStatus::Open => {
            let late = i.due.is_some_and(|d| d < now.date);
            let due = i.due.map_or("no date".to_string(), |d| format!("due {}", d.short()));
            Item::new(vec![Span::plain(&i.text)])
                .with_lead(if late { Span::strong("!", Tone::Warn) } else { Span::muted("·") })
                .with_sub(vec![Span::muted(format!("{due} · {project}"))])
        }
        IntentionStatus::Done | IntentionStatus::Dropped => {
            let done = i.status == IntentionStatus::Done;
            Item::new(vec![Span::new(&i.text, if done { Tone::Neutral } else { Tone::Muted })])
                .with_lead(if done { Span::new("✓", Tone::Ok) } else { Span::muted("×") })
                .with_meta(vec![Span::muted(i.closed_at.map(|c| c.date.short()).unwrap_or_default())])
        }
    }
}

fn intentions(src: &dyn ViewSource, ui: &UiState) -> Screen {
    let now = src.now();
    let b = src.briefing();
    let mood = resolve_mood(ui, briefing_moods(&b, now));
    let columns = intention_columns(&src.intentions(), now);
    let titles = ["Overdue", "Open", "Closed this week"];
    let tones = [Tone::Warn, Tone::Accent, Tone::Accent];
    let empty = [
        ("Nothing overdue", "Every dated intention is on time."),
        ("Nothing open", "Add one with `nahuali intend`."),
        ("Nothing closed", "Closed intentions stay here a week."),
    ];
    let focus = ui.column.min(2);
    let cards = columns
        .iter()
        .enumerate()
        .map(|(c, list)| {
            let body = if list.is_empty() {
                Node::Empty { title: empty[c].0.into(), line: empty[c].1.into() }
            } else {
                Node::List {
                    items: list.iter().map(|i| intention_item(i, now)).collect(),
                    selected: (c == focus).then(|| ui.selected(View::Intentions).min(list.len() - 1)),
                }
            };
            Node::card(
                format!("{} ({})", titles[c], list.len()),
                if c == focus { tones[c] } else { Tone::Muted },
                body,
            )
        })
        .collect();
    let status = vec![
        mascot_seg(&mood),
        Seg::new(None, format!("{} open", columns[0].len() + columns[1].len()), Tone::Muted, 2),
        Seg::new(None, format!("{} closed this week", columns[2].len()), Tone::Muted, 3),
    ];
    screen(src, ui, Node::Row(cards), status)
}

// ---------------------------------------------------------------------------
// Health

fn sparkline(values: &[u32]) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let max = values.iter().copied().max().unwrap_or(0).max(1);
    values.iter().map(|v| BARS[((*v as usize) * 7).div_ceil(max as usize).min(7)]).collect()
}

fn check_item(ok: bool, text: String, sub: Option<String>) -> Item {
    let item = Item::new(vec![Span::new(text, if ok { Tone::Neutral } else { Tone::Danger })])
        .with_lead(if ok { Span::new("✓", Tone::Ok) } else { Span::strong("✗", Tone::Danger) });
    match sub {
        Some(sub) => item.with_sub(vec![Span::muted(sub)]),
        None => item,
    }
}

fn health(src: &dyn ViewSource, ui: &UiState) -> Screen {
    let now = src.now();
    let report = src.health();
    let line = &report.line;
    let b = src.briefing();
    let mut moods = briefing_moods(&b, now);
    if let Some(cp) = &line.checkpoint
        && cp.ok
        && now.minutes() - cp.at.minutes() < 24 * 60
    {
        moods.push(Mood::new(MascotState::Proud, format!("checkpoint verified at {}", cp.at.clock())));
    }
    let mood = resolve_mood(ui, moods);
    let alarmed = mood.state == MascotState::Alarmed;

    let mut integrity = Vec::new();
    let mut integrity_tone = Tone::Accent;
    match line.chain {
        ChainStatus::Verified { to_seq } => {
            integrity.push(check_item(true, format!("event chain verified to seq {to_seq}"), None))
        }
        ChainStatus::Broken { at_seq } => {
            integrity_tone = Tone::Danger;
            integrity.push(check_item(
                false,
                format!("event chain breaks at seq {at_seq}"),
                Some(format!("nahuali verify --from {}", at_seq.saturating_sub(7))),
            ));
        }
        ChainStatus::Unknown => integrity.push(
            Item::new(vec![Span::new("event chain not verified yet", Tone::Warn)])
                .with_lead(Span::new("?", Tone::Warn)),
        ),
    }
    if let Some(cp) = &line.checkpoint {
        integrity.push(check_item(cp.ok, format!("checkpoint {} · {}", cp.at.clock(), cp.detail), None));
    }
    integrity.push(
        Item::new(vec![Span::plain(format!("{} events", group(line.events)))])
            .with_lead(Span::muted("·"))
            .with_meta(vec![Span::muted(
                line.last_event_at.map(|t| format!("last {} ago", t.age(now))).unwrap_or_default(),
            )]),
    );

    let feeds: Vec<Item> = report
        .feeds
        .iter()
        .map(|f| {
            let live = f.last_seen.is_some_and(|t| now.minutes() - t.minutes() <= 10);
            let seen = f.last_seen.map_or("never".to_string(), |t| format!("{} ago", t.age(now)));
            Item::new(vec![Span::plain(&f.client)])
                .with_lead(if live { Span::new("●", Tone::Ok) } else { Span::muted("○") })
                .with_meta(vec![Span::muted(format!("{seen} · {} today", f.sessions_today))])
        })
        .collect();
    let ingest = Node::Col(vec![
        Node::Text(vec![Span::muted("24h  "), Span::new(sparkline(&report.heartbeat), Tone::Accent)]),
        Node::List { items: feeds, selected: None },
    ]);

    let mut backups: Vec<Item> =
        line.alarms.iter().map(|alarm| check_item(false, alarm.clone(), None)).collect();
    backups.extend(
        report.backups.iter().map(|(name, job)| {
            check_item(job.ok, format!("{name} {} · {}", job.at.clock(), job.detail), None)
        }),
    );

    let store = Node::Kv(vec![
        ("engine".into(), report.store_version.clone()),
        ("address".into(), report.store_addr.clone()),
        ("events".into(), group(line.events)),
    ]);

    let integrity_card =
        Node::card("Integrity", integrity_tone, Node::List { items: integrity, selected: None });
    let backup_tone = if line.alarms.is_empty() { Tone::Accent } else { Tone::Warn };
    let backup_card =
        Node::card("Export & backup", backup_tone, Node::List { items: backups, selected: None });
    let mut left = vec![integrity_card, backup_card];
    // In an alarm the chain is all that matters; the loops wait.
    if !alarmed && let Some(report) = src.loops().and_then(|l| l.report()) {
        left.push(loops_card(&report, now));
    }
    let grid = Node::Row(vec![
        Node::Col(left),
        Node::Col(vec![Node::card("Ingest", Tone::Accent, ingest), Node::card("Store", Tone::Accent, store)]),
    ]);

    let body = if alarmed {
        Node::Col(vec![
            Node::card("", Tone::Neutral, Node::Mascot { state: mood.state, caption: mood.caption() }),
            grid,
        ])
    } else {
        grid
    };

    let mut status = vec![mascot_seg(&mood)];
    status.extend(
        health_segs(line, now)
            .into_iter()
            .filter(|seg| !(alarmed && seg.icon == Some(Icon::Fail) && seg.tone == Tone::Danger)),
    );
    screen(src, ui, body, status)
}

fn loops_card(r: &LoopReport, now: Stamp) -> Node {
    let mut items = Vec::new();
    items.push(match (r.baseline_at, &r.baseline_rev) {
        (Some(at), rev) => check_item(
            true,
            format!("baseline frozen {} ago", at.age(now)),
            rev.as_deref().map(|r| format!("at {}", r.get(..12).unwrap_or(r))),
        ),
        (None, _) => check_item(false, "no frozen baseline".into(), Some("nahuali loop freeze".into())),
    });
    items.push(
        Item::new(vec![Span::plain(format!("{} waiting for review", r.pending))])
            .with_lead(if r.pending > 0 { Span::new("●", Tone::Warn) } else { Span::muted("·") })
            .with_meta(vec![Span::muted(format!("{}/{} this week", r.this_week, r.weekly_cap))]),
    );
    for l in &r.loops {
        let near = l.runs_without_win + 1 >= l.retire_at;
        let last = l.last_run_at.map_or("never".to_string(), |t| format!("{} ago", t.age(now)));
        items.push(
            Item::new(vec![Span::plain(format!("{} · {} runs · {} approved", l.name, l.runs, l.approved))])
                .with_lead(Span::new(
                    if near { "!" } else { "·" },
                    if near { Tone::Warn } else { Tone::Muted },
                ))
                .with_meta(vec![Span::new(
                    format!("kill {}/{} · {last}", l.runs_without_win, l.retire_at),
                    if near { Tone::Warn } else { Tone::Muted },
                )]),
        );
    }
    let evals: Vec<String> = r.evals.iter().map(|(s, v)| format!("{s} {v}")).collect();
    if !evals.is_empty() {
        items.push(Item::new(vec![Span::plain(evals.join(" · "))]).with_lead(Span::muted("·")));
    }
    items.push(
        Item::new(vec![Span::plain(format!("{} recalls recorded", group(r.recall_used)))])
            .with_lead(if r.recall_used_failures > 0 {
                Span::new("!", Tone::Warn)
            } else {
                Span::muted("·")
            })
            .with_meta(vec![Span::muted(format!("{} write failures", r.recall_used_failures))]),
    );
    let tone = if r.baseline_at.is_none() { Tone::Warn } else { Tone::Accent };
    Node::card("Loops", tone, Node::List { items, selected: None })
}

fn group(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// Search

fn search(src: &dyn ViewSource, ui: &UiState) -> Screen {
    let now = src.now();
    let b = src.briefing();
    let mut moods = briefing_moods(&b, now);
    if ui.searching {
        moods.retain(|m| m.state == MascotState::Alarmed);
        moods.push(Mood::new(MascotState::Thinking, "hybrid recall"));
    }
    let mood = resolve_mood(ui, moods);

    let query_box = Node::card(
        "",
        Tone::Accent,
        Node::Text(vec![
            Span::muted("⌕  "),
            if ui.query.is_empty() { Span::muted("search your memory") } else { Span::plain(&ui.query) },
            Span::strong("▏", Tone::Accent),
        ]),
    );

    let mut status = vec![mascot_seg(&mood)];
    let body = if ui.query.trim().is_empty() {
        Node::Col(vec![
            query_box,
            Node::Empty {
                title: "Ask your memory anything".into(),
                line: "The same recall your agents run, with the reason for every result.".into(),
            },
        ])
    } else {
        let page = src.recall(&ui.query, 20);
        status.push(Seg::new(
            None,
            format!(
                "{} results · {} ms over {} events",
                page.items.len(),
                page.took_ms,
                group(page.searched_events)
            ),
            Tone::Muted,
            2,
        ));
        if page.items.is_empty() {
            Node::Col(vec![
                query_box,
                Node::Empty {
                    title: "No match".into(),
                    line: "Try fewer words, or another language.".into(),
                },
            ])
        } else {
            let selected = ui.selected(View::Search).min(page.items.len() - 1);
            let items = page
                .items
                .iter()
                .map(|r| {
                    let mut text =
                        vec![Span::muted(format!("{} · ", r.kind.label())), Span::plain(first_line(&r.text))];
                    if r.superseded_by.is_some() {
                        text.push(Span::muted(" (superseded)"));
                    }
                    Item::new(text)
                        .with_lead(Span::new(format!("{:.2}", r.score), Tone::Accent))
                        .with_meta(vec![Span::muted(r.occurred_at.date.short())])
                })
                .collect();
            let r = &page.items[selected];
            let mut why = vec![
                Node::Meter { label: "lexical".into(), value: r.breakdown.lexical },
                Node::Meter { label: "graph".into(), value: r.breakdown.graph },
                Node::Meter { label: "recency".into(), value: r.breakdown.recency },
            ];
            let mut kv = vec![(
                "when".to_string(),
                format!(
                    "{} {}{}",
                    r.occurred_at.date.short(),
                    r.occurred_at.clock(),
                    r.actor.as_deref().map(|a| format!(" · {a}")).unwrap_or_default()
                ),
            )];
            if !r.matched.is_empty() {
                kv.push(("matched".into(), r.matched.join(", ")));
            }
            if let Some(source) = r.sources.first() {
                kv.push(("source".into(), source.clone()));
            }
            if let Some(project) = &r.project {
                kv.push(("project".into(), project.clone()));
            }
            why.push(Node::Kv(kv));
            why.push(Node::Text(vec![Span::muted(format!(
                "nahuali recall \"{}\" --explain",
                ui.query.trim()
            ))]));
            let side = match ui.detail.then(|| src.record(&r.id)).flatten() {
                Some(detail) => record_card(&detail, now),
                None => Node::card("Why this result", Tone::Accent, Node::Col(why)),
            };
            Node::Col(vec![
                query_box,
                Node::Row(vec![
                    Node::card("Results", Tone::Accent, Node::List { items, selected: Some(selected) }),
                    side,
                ]),
            ])
        }
    };
    screen(src, ui, body, status)
}

/// A record in full, with its history: what `enter` opens on Search.
fn record_card(d: &RecordDetail, now: Stamp) -> Node {
    let e = &d.entry;
    let mut facts = vec![
        ("id".to_string(), e.id.clone()),
        (
            "when".to_string(),
            format!(
                "{} {}{}",
                e.at.date.short(),
                e.at.clock(),
                e.actor.as_deref().map(|a| format!(" · {a}")).unwrap_or_default()
            ),
        ),
    ];
    if let Some(project) = &e.project {
        facts.push(("project".into(), project.clone()));
    }
    if e.sensitivity.is_private() {
        facts.push(("visibility".into(), e.sensitivity.label().into()));
    }
    facts.extend(d.details.iter().cloned());
    for link in &d.links {
        facts.push((link.relation.clone(), link.id.clone()));
    }
    if let Some(source) = d.sources.first() {
        let more = if d.sources.len() > 1 { format!(" (+{})", d.sources.len() - 1) } else { String::new() };
        facts.push(("source".into(), format!("{source}{more}")));
    }
    let mut parts = vec![Node::Text(vec![Span::plain(&e.text)]), Node::Kv(facts)];
    if !d.history.is_empty() {
        parts.push(Node::Text(vec![Span::strong("History", Tone::Accent)]));
        parts.push(Node::List {
            items: d
                .history
                .iter()
                .map(|c| {
                    let what = if c.redacted { format!("{} (forgotten)", c.what) } else { c.what.clone() };
                    Item::new(vec![Span::new(what, if c.redacted { Tone::Muted } else { Tone::Neutral })])
                        .with_lead(Span::muted(c.at.date.short()))
                        .with_meta(vec![Span::muted(format!("{} · {} ago", c.actor, c.at.age(now)))])
                })
                .collect(),
            selected: None,
        });
    }
    Node::card(e.kind.label().to_string(), Tone::Accent, Node::Col(parts))
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

// ---------------------------------------------------------------------------
// Proposals

fn status_tone(status: &str) -> Tone {
    match status {
        "pending" => Tone::Warn,
        "approved" | "applied" => Tone::Ok,
        "rejected" | "reverted" => Tone::Danger,
        _ => Tone::Muted,
    }
}

fn proposal_item(p: &Proposal, now: Stamp) -> Item {
    let gates = if p.gates_ok() { Span::new("✓", Tone::Ok) } else { Span::strong("✗", Tone::Danger) };
    Item::new(vec![Span::plain(&p.target)])
        .with_lead(gates)
        .with_meta(vec![
            Span::new(&p.status, status_tone(&p.status)),
            Span::muted(format!(" · {} ago", p.created_at.age(now))),
        ])
        .with_sub(vec![Span::muted(first_line(&p.hypothesis).to_string())])
}

/// Diff lines with their tone, capped so a large diff cannot flood the pane.
fn diff_spans(diff: &str, max_lines: usize) -> Vec<Span> {
    let mut out = Vec::new();
    let total = diff.lines().count();
    for line in diff.lines().take(max_lines) {
        let tone = if line.starts_with("+++") || line.starts_with("---") || line.starts_with("diff ") {
            Tone::Muted
        } else if line.starts_with('+') {
            Tone::Ok
        } else if line.starts_with('-') {
            Tone::Danger
        } else if line.starts_with("@@") {
            Tone::Accent
        } else {
            Tone::Neutral
        };
        out.push(Span::new(format!("{line}\n"), tone));
    }
    if total > max_lines {
        out.push(Span::muted(format!("… {} more lines (nahuali loop show)", total - max_lines)));
    }
    out
}

fn proposals(src: &dyn ViewSource, ui: &UiState) -> Screen {
    let now = src.now();
    let b = src.briefing();
    let Some(loops) = src.loops() else {
        return today(src, &ui.on(View::Today));
    };
    let list = loops.proposals();
    let open = list.iter().filter(|p| p.is_open()).count();
    let mut moods = briefing_moods(&b, now);
    if open > 0 {
        moods.push(Mood::new(MascotState::Listening, format!("{open} proposal{} to review", plural(open))));
    }
    let mood = resolve_mood(ui, moods);
    if list.is_empty() {
        let body = Node::Col(vec![
            Node::card("", Tone::Neutral, Node::Mascot { state: mood.state, caption: mood.caption() }),
            Node::Empty {
                title: "No proposals".into(),
                line: "The loops propose at most 3 changes a week; each waits here for review.".into(),
            },
        ]);
        return screen(src, ui, body, vec![mascot_seg(&mood)]);
    }
    let selected = ui.selected(View::Proposals).min(list.len() - 1);
    let items: Vec<Item> = list.iter().map(|p| proposal_item(p, now)).collect();
    let list_card = Node::card(
        format!("Proposals ({open} open)"),
        Tone::Accent,
        Node::List { items, selected: Some(selected) },
    );
    let detail = loops.proposal(&list[selected].id);
    let p = detail.as_ref().map(|d| &d.proposal).unwrap_or(&list[selected]);
    let mut facts = vec![
        ("status".to_string(), p.status.clone()),
        ("loop".to_string(), p.loop_name.clone()),
        ("branch".to_string(), format!("{} @ {}", p.branch, p.rev.get(..12).unwrap_or(&p.rev))),
        ("diff".to_string(), format!("{} (+{} −{})", p.diff_summary, p.lines_added, p.lines_removed)),
    ];
    if let Some(cost) = &p.cost {
        facts.push(("cost".to_string(), cost.clone()));
    }
    let mut parts = vec![
        Node::Kv(facts),
        Node::Text(vec![Span::strong("Hypothesis  ", Tone::Accent), Span::plain(&p.hypothesis)]),
        Node::Text(vec![Span::strong("Changes  ", Tone::Accent), Span::plain(&p.behaviour)]),
    ];
    let mut gates: Vec<Item> =
        vec![check_item(p.frozen == "passed", format!("frozen-hashes · {}", p.frozen), None)];
    gates.extend(
        p.gates
            .iter()
            .filter(|g| g.gate != "frozen-hashes")
            .map(|g| check_item(g.passed, g.gate.clone(), Some(g.evidence.clone()))),
    );
    if let Some(frozen) = p.gates.iter().find(|g| g.gate == "frozen-hashes") {
        gates[0] = check_item(frozen.passed, "frozen-hashes".into(), Some(frozen.evidence.clone()));
    }
    parts.push(Node::card(
        "Gates",
        if p.gates_ok() { Tone::Ok } else { Tone::Danger },
        Node::List { items: gates, selected: None },
    ));
    if let Some(d) = &detail {
        let motives: Vec<Item> = d
            .motives
            .iter()
            .map(|m| {
                Item::new(vec![Span::plain(&m.kind)])
                    .with_lead(Span::muted("·"))
                    .with_meta(vec![Span::muted(&m.detail)])
            })
            .collect();
        if !motives.is_empty() {
            parts.push(Node::card(
                "Motivating events",
                Tone::Muted,
                Node::List { items: motives, selected: None },
            ));
        }
        if ui.detail {
            parts.push(Node::card("Diff", Tone::Accent, Node::Text(diff_spans(&d.diff, 400))));
        }
    }
    let body = if ui.detail {
        Node::Col(parts)
    } else {
        Node::Row(vec![list_card, Node::card(p.id.clone(), status_tone(&p.status), Node::Col(parts))])
    };
    let status = vec![
        mascot_seg(&mood),
        Seg::new(None, format!("{open} open · {} total", list.len()), Tone::Muted, 2),
    ];
    screen(src, ui, body, status)
}
