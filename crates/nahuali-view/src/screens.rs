//! Builders for the screens. Each takes the data source and the UI state and
//! returns a complete [`Screen`]; nothing here depends on terminal size.

use crate::date::Stamp;
use crate::keymap::{self, View};
use crate::model::{Icon, Item, MascotState, Node, Screen, Seg, Span, Tone};
use crate::source::{Ask, Board, Change, Found, Lesson, Marks, ViewSource, Work, WorkStatus};
use crate::state::{Picked, UiState, picked};

const TITLE: &str = "nahuali";
/// A question that has waited this long worries the axolotl.
const WORRY_AFTER_DAYS: i64 = 2;

/// Build the screen for `ui.view`.
pub fn build(src: &dyn ViewSource, ui: &UiState) -> Screen {
    match ui.view {
        View::Board => board(src, ui),
        View::Lessons => lessons(src, ui),
        View::Recall => recall(src, ui),
    }
}

fn screen(src: &dyn ViewSource, ui: &UiState, body: Node, status: Vec<Seg>) -> Screen {
    let mut status = status;
    if let Some(notice) = &ui.notice {
        status.insert(0, Seg::new(None, notice.clone(), Tone::Accent, 0));
    }
    let mut keys = keymap::footer_hints(ui.view, ui.demo);
    if let Some(p) = &ui.prompt {
        status.insert(0, Seg::new(None, format!("answer: {}▏", p.text), Tone::Warn, 0));
        keys = keymap::prompt_hints();
    }
    let who = src.person().name;
    Screen {
        title: format!("{TITLE} · {who} · {}", ui.view.title()),
        who,
        tabs: View::ALL.iter().map(|v| v.title().to_string()).collect(),
        active_tab: ui.view.index(),
        body,
        status,
        keys,
    }
}

// ---------------------------------------------------------------------------
// Which face the axolotl shows and why.

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

    /// `listening · 2 questions wait for you`.
    pub fn caption(&self) -> String {
        format!("{} · {}", self.state.name(), self.reason)
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn days(from: Stamp, now: Stamp) -> i64 {
    (now.minutes() - from.minutes()).max(0) / 1440
}

/// The mood the board alone justifies: what is owed to the person comes
/// before what is merely stuck, and a clear board is a happy one.
pub fn board_mood(board: &Board, now: Stamp) -> Mood {
    let oldest = board.asks.iter().map(|a| days(a.marks.as_of, now)).max();
    if let Some(d) = oldest.filter(|d| *d >= WORRY_AFTER_DAYS) {
        return Mood::new(MascotState::Worried, format!("a question has waited {d} days"));
    }
    if !board.asks.is_empty() {
        let n = board.asks.len();
        return Mood::new(
            MascotState::Listening,
            format!("{n} question{} wait{} for you", plural(n), if n == 1 { "s" } else { "" }),
        );
    }
    if let Some(w) = board.works.iter().find(|w| matches!(w.status, WorkStatus::Blocked(_))) {
        return Mood::new(MascotState::Thinking, format!("{} is blocked", w.marks.project));
    }
    if board.works.is_empty() {
        return Mood::new(MascotState::Idle, "resting");
    }
    if board.works.iter().all(|w| w.status == WorkStatus::Done) {
        return Mood::new(MascotState::Proud, "everything is done");
    }
    Mood::new(MascotState::Happy, "nothing waits on you")
}

fn resolve_mood(ui: &UiState, mood: Mood) -> Mood {
    match ui.mascot_override {
        Some(state) => Mood::new(state, "demo"),
        None => mood,
    }
}

fn mascot_seg(mood: &Mood) -> Seg {
    Seg::new(Some(Icon::Mascot(mood.state)), mood.caption(), Tone::Neutral, 0)
}

/// How much of what is shown was checked by something.
fn trust<'a>(marks: impl Iterator<Item = &'a Marks>) -> (usize, usize) {
    marks.fold((0, 0), |(v, c), m| if m.sure.is_verified() { (v + 1, c) } else { (v, c + 1) })
}

fn trust_seg(verified: usize, claimed: usize) -> Seg {
    match (verified, claimed) {
        (0, 0) => Seg::new(Some(Icon::Idle), "nothing held yet", Tone::Muted, 2),
        (_, 0) => Seg::new(Some(Icon::Ok), "all verified", Tone::Ok, 2),
        (_, c) => Seg::new(Some(Icon::Fail), format!("{c} only claimed"), Tone::Warn, 1),
    }
}

// ---------------------------------------------------------------------------
// Rows

fn ask_item(a: &Ask, now: Stamp) -> Item {
    let mut sub = Vec::new();
    if let Some(blocks) = &a.blocks {
        sub.push(Span::new(format!("blocks {blocks}"), Tone::Warn));
    }
    if let Some(rec) = &a.recommends {
        if !sub.is_empty() {
            sub.push(Span::muted(" · "));
        }
        sub.push(Span::muted(format!("recommended: {rec}")));
    }
    Item::new(vec![Span::strong(format!("{}  ", a.marks.project), Tone::Accent), Span::plain(&a.question)])
        .with_lead(Span::new("▸", Tone::Warn))
        .with_meta(vec![Span::muted(format!("{} ago", a.marks.as_of.age(now)))])
        .with_sub(sub)
}

fn status_lead(status: &WorkStatus) -> Span {
    match status {
        WorkStatus::Moving => Span::new("●", Tone::Ok),
        WorkStatus::Blocked(_) => Span::new("●", Tone::Danger),
        WorkStatus::Waiting => Span::new("●", Tone::Warn),
        WorkStatus::NotStarted => Span::muted("○"),
        WorkStatus::Done => Span::new("✓", Tone::Ok),
    }
}

fn work_item(w: &Work, now: Stamp) -> Item {
    let sub = match &w.status {
        WorkStatus::Blocked(by) => vec![Span::new(format!("blocked by {by}"), Tone::Danger)],
        WorkStatus::Waiting => vec![Span::new("waiting on you", Tone::Warn)],
        _ => Vec::new(),
    };
    Item::new(vec![
        Span::strong(format!("{}  ", w.marks.project), Tone::Accent),
        Span::plain(format!("{} · ", w.title)),
        Span::muted(&w.line),
    ])
    .with_lead(status_lead(&w.status))
    .with_meta(vec![Span::muted(format!("{} ago", w.marks.as_of.age(now)))])
    .with_sub(sub)
}

fn lesson_item(l: &Lesson) -> Item {
    Item::new(vec![Span::plain(&l.rule)])
        .with_lead(Span::new("!", Tone::Warn))
        .with_sub(vec![Span::muted(format!("when: {}", l.moment))])
}

fn history_node(history: &[Change], now: Stamp) -> Vec<Node> {
    if history.is_empty() {
        return Vec::new();
    }
    vec![
        Node::Text(vec![Span::strong("Past", Tone::Accent)]),
        Node::List {
            items: history
                .iter()
                .map(|c| {
                    Item::new(vec![Span::plain(&c.what)])
                        .with_lead(Span::muted(c.at.date.short()))
                        .with_meta(vec![Span::muted(format!("{} · {} ago", c.by, c.at.age(now)))])
                })
                .collect(),
            selected: None,
        },
    ]
}

fn marks_node(marks: &Marks, now: Stamp) -> Vec<Node> {
    vec![Node::Text(vec![Span::strong("Marks", Tone::Accent)]), Node::Kv(marks.rows(now))]
}

fn bullets(title: &str, lines: &[String], lead: &str, tone: Tone) -> Vec<Node> {
    if lines.is_empty() {
        return Vec::new();
    }
    vec![
        Node::Text(vec![Span::strong(title, Tone::Accent)]),
        Node::List {
            items: lines
                .iter()
                .map(|l| Item::new(vec![Span::plain(l)]).with_lead(Span::new(lead, tone)))
                .collect(),
            selected: None,
        },
    ]
}

// ---------------------------------------------------------------------------
// Open items

fn ask_card(a: &Ask, history: &[Change], now: Stamp) -> Node {
    let mut facts = Vec::new();
    if let Some(blocks) = &a.blocks {
        facts.push(("blocks".to_string(), blocks.clone()));
    }
    if let Some(rec) = &a.recommends {
        facts.push(("recommended".to_string(), rec.clone()));
    }
    let mut parts = vec![Node::Text(vec![Span::strong(&a.question, Tone::Neutral)])];
    if !facts.is_empty() {
        parts.push(Node::Kv(facts));
    }
    parts.extend(marks_node(&a.marks, now));
    parts.extend(history_node(history, now));
    Node::card(format!("Question · {}", a.marks.project), Tone::Warn, Node::Col(parts))
}

fn work_card(w: &Work, history: &[Change], now: Stamp) -> Node {
    let status = match &w.status {
        WorkStatus::Blocked(by) => format!("blocked by {by}"),
        other => other.word().to_string(),
    };
    let mut parts = vec![
        Node::Text(vec![Span::strong(&w.title, Tone::Neutral)]),
        Node::Kv(vec![("status".to_string(), status), ("now".to_string(), w.line.clone())]),
    ];
    parts.extend(bullets("Done", &w.done, "✓", Tone::Ok));
    parts.extend(bullets("Next", &w.next, "→", Tone::Accent));
    parts.extend(marks_node(&w.marks, now));
    parts.extend(history_node(history, now));
    Node::card(format!("Work · {}", w.marks.project), Tone::Accent, Node::Col(parts))
}

fn lesson_card(l: &Lesson, history: &[Change], now: Stamp) -> Node {
    let mut parts = vec![
        Node::Text(vec![Span::strong(&l.rule, Tone::Neutral)]),
        Node::Kv(vec![("when".to_string(), l.moment.clone())]),
    ];
    parts.extend(marks_node(&l.marks, now));
    parts.extend(history_node(history, now));
    Node::card(format!("Lesson · {}", l.marks.project), Tone::Warn, Node::Col(parts))
}

// ---------------------------------------------------------------------------
// Board: where things stand and what is owed.

fn board(src: &dyn ViewSource, ui: &UiState) -> Screen {
    let now = src.now();
    let b = src.board();
    let mood = resolve_mood(ui, board_mood(&b, now));
    let (verified, claimed) = trust(b.marks());

    let mut sidebar = vec![Node::Mascot { state: mood.state, caption: mood.caption() }];
    sidebar.push(Node::card(
        "How sure",
        if claimed == 0 { Tone::Ok } else { Tone::Warn },
        Node::Kv(vec![
            ("verified".to_string(), verified.to_string()),
            ("claimed".to_string(), claimed.to_string()),
        ]),
    ));
    let sidebar = Node::Col(sidebar);

    let index = ui.selected(View::Board);
    let chosen = picked(&b, index);
    let main = if let (true, Some(chosen)) = (ui.detail, chosen) {
        match chosen {
            Picked::Ask(a) => ask_card(a, &src.history(&a.id), now),
            Picked::Work(w) => work_card(w, &src.history(&w.id), now),
        }
    } else {
        let rows = b.asks.len() + b.works.len();
        let at = if rows == 0 { 0 } else { index.min(rows - 1) };
        let asks_node = if b.asks.is_empty() {
            Node::Empty {
                title: "Nothing waits on you".into(),
                line: "Questions for you show up here until you answer them.".into(),
            }
        } else {
            Node::List {
                items: b.asks.iter().map(|a| ask_item(a, now)).collect(),
                selected: (at < b.asks.len()).then_some(at),
            }
        };
        // What is finished is told apart from what is in motion: by the
        // end of a day there is more of it, and it needs nobody.
        let active = b.works.iter().take_while(|w| w.status != WorkStatus::Done).count();
        let in_works = at.checked_sub(b.asks.len());
        let works_node = if active == 0 {
            Node::Empty {
                title: "Nothing in motion".into(),
                line: "Work shows up here when it starts.".into(),
            }
        } else {
            Node::List {
                items: b.works[..active].iter().map(|w| work_item(w, now)).collect(),
                selected: in_works.filter(|i| *i < active),
            }
        };
        let lessons_node = if b.lessons.is_empty() {
            Node::Empty { title: "Nothing new".into(), line: "No lesson was recorded this week.".into() }
        } else {
            Node::List { items: b.lessons.iter().map(lesson_item).collect(), selected: None }
        };
        let mut cards = vec![
            Node::card(
                format!("Waiting on you · {}", b.asks.len()),
                if b.asks.is_empty() { Tone::Ok } else { Tone::Warn },
                asks_node,
            ),
            Node::card(format!("In motion · {active}"), Tone::Accent, works_node),
            Node::card(format!("Learned this week · {}", b.lessons.len()), Tone::Accent, lessons_node),
        ];
        if active < b.works.len() {
            cards.push(Node::card(
                format!("Done lately · {}", b.works.len() - active),
                Tone::Ok,
                Node::List {
                    items: b.works[active..].iter().map(|w| work_item(w, now)).collect(),
                    selected: in_works.filter(|i| *i >= active).map(|i| i - active),
                },
            ));
        }
        Node::Col(cards)
    };

    let status = vec![mascot_seg(&mood), trust_seg(verified, claimed)];
    screen(src, ui, Node::Row(vec![sidebar, main]), status)
}

// ---------------------------------------------------------------------------
// Lessons: what was learned, and when each one applies.

fn lessons(src: &dyn ViewSource, ui: &UiState) -> Screen {
    let now = src.now();
    let all = src.lessons();
    let mood = resolve_mood(ui, board_mood(&src.board(), now));
    let (verified, claimed) = trust(all.iter().map(|l| &l.marks));
    let body = if all.is_empty() {
        Node::card(
            "Lessons",
            Tone::Accent,
            Node::Empty {
                title: "Nothing learned yet".into(),
                line: "A lesson is a rule and the moment it applies.".into(),
            },
        )
    } else {
        let at = ui.selected(View::Lessons).min(all.len() - 1);
        if ui.detail {
            lesson_card(&all[at], &src.history(&all[at].id), now)
        } else {
            Node::card(
                format!("Lessons · {}", all.len()),
                Tone::Accent,
                Node::List {
                    items: all
                        .iter()
                        .map(|l| {
                            lesson_item(l).with_meta(vec![Span::muted(format!(
                                "{} · {} ago",
                                l.marks.project,
                                l.marks.as_of.age(now)
                            ))])
                        })
                        .collect(),
                    selected: Some(at),
                },
            )
        }
    };
    let status = vec![mascot_seg(&mood), trust_seg(verified, claimed)];
    screen(src, ui, body, status)
}

// ---------------------------------------------------------------------------
// Recall: ask the memory what it knows about something.

fn found_item(f: &Found, now: Stamp) -> Item {
    let (lead, tone) = match f.kind.as_str() {
        "ask" => ("▸", Tone::Warn),
        "decision" => ("✓", Tone::Ok),
        "lesson" => ("!", Tone::Warn),
        "note" => ("·", Tone::Accent),
        _ => ("●", Tone::Accent),
    };
    let mut meta = format!("{} · {} ago", f.kind, f.marks.as_of.age(now));
    if let Some(alike) = f.alike {
        meta = format!("0.{alike:02} alike · {meta}");
    }
    let mut item =
        Item::new(vec![Span::strong(format!("{}  ", f.marks.project), Tone::Accent), Span::plain(&f.title)])
            .with_lead(Span::new(lead, tone))
            .with_meta(vec![Span::muted(meta)]);
    if let Some(first) = f.detail.first() {
        item = item.with_sub(vec![Span::muted(first)]);
    }
    item
}

fn found_card(f: &Found, history: &[Change], now: Stamp) -> Node {
    let mut parts = vec![Node::Text(vec![Span::strong(&f.title, Tone::Neutral)])];
    if !f.detail.is_empty() {
        parts.push(Node::List {
            items: f.detail.iter().map(|d| Item::new(vec![Span::plain(d)])).collect(),
            selected: None,
        });
    }
    if let Some(alike) = f.alike {
        parts.push(Node::Text(vec![Span::new(
            format!("Found by its meaning, not by its words: 0.{alike:02} alike. Judge whether it is about what you asked."),
            Tone::Warn,
        )]));
    }
    parts.extend(marks_node(&f.marks, now));
    parts.extend(history_node(history, now));
    Node::card(format!("{} · {}", f.kind, f.marks.project), Tone::Accent, Node::Col(parts))
}

fn recall(src: &dyn ViewSource, ui: &UiState) -> Screen {
    let now = src.now();
    let asked = ui.query.trim();
    let found = if asked.is_empty() { Vec::new() } else { src.recall(asked) };
    let mood = if ui.searching {
        Mood::new(MascotState::Thinking, "looking")
    } else {
        resolve_mood(ui, board_mood(&src.board(), now))
    };
    let (verified, claimed) = trust(found.iter().map(|f| &f.marks));

    let question = Node::card(
        "Ask the memory",
        Tone::Accent,
        Node::Text(vec![Span::new("› ", Tone::Accent), Span::plain(&ui.query), Span::new("▏", Tone::Accent)]),
    );
    let body = if asked.is_empty() {
        Node::Col(vec![
            question,
            Node::card(
                "Recall",
                Tone::Accent,
                Node::Empty {
                    title: "Type what you want to know".into(),
                    line: "Cards, decisions, lessons and notes that hold every word you type.".into(),
                },
            ),
        ])
    } else if found.is_empty() {
        Node::Col(vec![
            question,
            Node::card(
                "Recall",
                Tone::Ok,
                Node::Empty {
                    title: "Nothing known about that".into(),
                    line: "An empty answer is a correct answer.".into(),
                },
            ),
        ])
    } else {
        let at = ui.selected(View::Recall).min(found.len() - 1);
        if ui.detail {
            found_card(&found[at], &src.history(&found[at].id), now)
        } else {
            // What the words found comes first; what only the meaning found
            // comes after, apart, and is named for what it is.
            let matches = found.iter().take_while(|f| f.alike.is_none()).count();
            let mut cards = vec![question];
            if matches > 0 {
                cards.push(Node::card(
                    format!("Found · {matches}"),
                    Tone::Accent,
                    Node::List {
                        items: found[..matches].iter().map(|f| found_item(f, now)).collect(),
                        selected: (at < matches).then_some(at),
                    },
                ));
            }
            if matches < found.len() {
                cards.push(Node::card(
                    format!("Close in meaning · {} (not the same words)", found.len() - matches),
                    Tone::Warn,
                    Node::List {
                        items: found[matches..].iter().map(|f| found_item(f, now)).collect(),
                        selected: (at >= matches).then(|| at - matches),
                    },
                ));
            }
            Node::Col(cards)
        }
    };
    let mut status = vec![mascot_seg(&mood)];
    if !found.is_empty() {
        status.push(trust_seg(verified, claimed));
    }
    screen(src, ui, body, status)
}
