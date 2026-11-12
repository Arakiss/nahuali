//! What is true now: the log folded in order.
//!
//! The order is the moment of each event and then its id, never the order
//! of the files or of the lines in them. Two copies of the same lines, read
//! in any order, give the same state: that is what lets two machines share
//! a log by copying files.

use std::collections::{BTreeMap, BTreeSet};

use crate::event::{Actor, Event, Status, Sure, What};
use crate::id;
use crate::time::Time;
use crate::words;

/// The four marks of an item: who, how sure, as of when, how far it reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marks {
    pub who: Actor,
    pub sure: Sure,
    pub at: Time,
    pub commit: Option<String>,
    pub project: String,
    pub private: bool,
}

impl Marks {
    fn of(event: &Event) -> Self {
        Self {
            who: event.by.clone(),
            sure: event.sure.clone(),
            at: event.at,
            commit: event.commit.clone(),
            project: event.project.clone(),
            private: event.private,
        }
    }
}

/// One step in the past of an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub at: Time,
    pub by: Actor,
    pub what: String,
}

/// A named piece of work and its one state card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Work {
    pub slug: String,
    pub title: String,
    pub goal: Option<String>,
    /// What the card says. What the board shows is [`State::standing`].
    pub status: Status,
    pub line: String,
    pub done: Vec<String>,
    pub next: Vec<String>,
    /// The marks of the last update.
    pub marks: Marks,
    pub since: Time,
    /// Oldest first.
    pub history: Vec<Change>,
}

/// How an ask ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Closed {
    /// It became a decision.
    Answered(String),
    Withdrawn(String),
}

/// A question for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    pub id: String,
    pub question: String,
    pub blocks: Option<String>,
    pub recommends: Option<String>,
    /// The piece of work that waits on it, in the same project.
    pub work: Option<String>,
    /// The marks of the asking.
    pub marks: Marks,
    pub closed: Option<(Closed, Marks)>,
}

impl Ask {
    pub fn is_open(&self) -> bool {
        self.closed.is_none()
    }
}

/// A rule learned from a failure, and the moment it applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lesson {
    pub id: String,
    pub rule: String,
    pub moment: String,
    pub when: Vec<String>,
    /// It applies to every project.
    pub everywhere: bool,
    /// It stops the act once, not only speaks to it.
    pub stop: bool,
    pub marks: Marks,
    pub retired: Option<(String, Marks)>,
    /// How many times it was shown to someone about to act, and when last.
    pub shown: u32,
    pub last_shown: Option<Time>,
    /// How many of those times it stopped the act.
    pub stopped: u32,
    /// The sessions it was shown in: once each is enough.
    pub sessions: BTreeSet<String>,
}

impl Lesson {
    pub fn in_force(&self) -> bool {
        self.retired.is_none()
    }
}

/// Something worth knowing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub text: String,
    pub tags: Vec<String>,
    pub about: Option<String>,
    pub marks: Marks,
    pub forgotten: Option<(String, Marks)>,
}

/// A lesson that applies to what is about to be done, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applies<'a> {
    pub lesson: &'a Lesson,
    /// The terms of the act that brought it.
    pub because: Vec<String>,
}

/// Anything held, as something that can be searched for: its words, its
/// marks, and where it reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thing {
    /// `work:atlas/importer`, `ask:k3f9q2`, `decision:k3f9q2`, `lesson:…`, `note:…`.
    pub key: String,
    pub kind: &'static str,
    /// The first line shown for it.
    pub title: String,
    /// Every other line worth showing.
    pub detail: Vec<String>,
    pub marks: Marks,
    pub everywhere: bool,
}

impl Thing {
    /// All of its words, for whatever indexes it.
    pub fn text(&self) -> String {
        let mut text = self.title.clone();
        for line in &self.detail {
            text.push('\n');
            text.push_str(line);
        }
        text
    }
}

/// Where a piece of work stands, counting what waits on the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Standing {
    Moving,
    Blocked(String),
    /// Stopped on these open asks.
    Waiting(Vec<String>),
    NotStarted,
    Done,
}

impl Standing {
    pub fn word(&self) -> &'static str {
        match self {
            Self::Moving => "moving",
            Self::Blocked(_) => "blocked",
            Self::Waiting(_) => "waiting on the person",
            Self::NotStarted => "not started",
            Self::Done => "done",
        }
    }

    /// What needs a person first comes first.
    fn rank(&self) -> u8 {
        match self {
            Self::Waiting(_) => 0,
            Self::Blocked(_) => 1,
            Self::Moving => 2,
            Self::NotStarted => 3,
            Self::Done => 4,
        }
    }
}

/// Who is looking, and so how far they see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope<'a> {
    /// One project, with what is private to it.
    Project(&'a str),
    /// Every project, from inside this one: what is private to another
    /// project stays out.
    AllFrom(&'a str),
    /// The person looking at their own memory.
    Everything,
}

impl Scope<'_> {
    pub fn sees(self, marks: &Marks) -> bool {
        match self {
            Self::Project(p) => marks.project == p,
            Self::AllFrom(p) => !marks.private || marks.project == p,
            Self::Everything => true,
        }
    }

    /// As [`Scope::sees`], for what may be meant for every project: it
    /// reaches a project it was not made in, unless it is private.
    pub fn reaches(self, marks: &Marks, everywhere: bool) -> bool {
        self.sees(marks) || (everywhere && !marks.private)
    }
}

/// Why a name did not lead to one thing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoAsk {
    Unknown,
    /// Several start like that.
    Ambiguous(Vec<String>),
}

/// What a name leads to in a map: its whole name, with or without its kind
/// in front, or the start of it when only one starts that way.
fn find<'a, T>(map: &'a BTreeMap<String, T>, kind: &str, name: &str) -> Result<&'a T, NoAsk> {
    let name = name.strip_prefix(kind).unwrap_or(name).to_ascii_lowercase();
    if let Some(found) = map.get(&name) {
        return Ok(found);
    }
    if name.is_empty() {
        return Err(NoAsk::Unknown);
    }
    let mut found = map.range(name.clone()..).take_while(|(id, _)| id.starts_with(&name));
    match (found.next(), found.next()) {
        (Some((_, one)), None) => Ok(one),
        (Some((a, _)), Some((b, _))) => Err(NoAsk::Ambiguous(vec![a.clone(), b.clone()])),
        _ => Err(NoAsk::Unknown),
    }
}

/// A finished piece of work leaves the board a day later.
const DONE_STAYS: u64 = 86_400;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    works: BTreeMap<(String, String), Work>,
    asks: BTreeMap<String, Ask>,
    lessons: BTreeMap<String, Lesson>,
    notes: BTreeMap<String, Note>,
    /// Events of a kind this version does not know.
    pub unknown: usize,
}

impl State {
    pub fn fold(events: &[Event]) -> Self {
        let mut ordered: Vec<&Event> = events.iter().collect();
        ordered.sort_by(|a, b| (a.at, &a.id).cmp(&(b.at, &b.id)));
        ordered.dedup_by(|a, b| a.id == b.id);
        let mut state = Self::default();
        for event in ordered {
            state.apply(event);
        }
        state
    }

    fn apply(&mut self, event: &Event) {
        match &event.what {
            What::Work(set) => {
                let key = (event.project.clone(), set.work.clone());
                let fresh = !self.works.contains_key(&key);
                let work = self.works.entry(key).or_insert_with(|| Work {
                    slug: set.work.clone(),
                    title: set.work.clone(),
                    goal: None,
                    status: if set.line.is_some() || !set.done.is_empty() {
                        Status::Moving
                    } else {
                        Status::NotStarted
                    },
                    line: String::new(),
                    done: Vec::new(),
                    next: Vec::new(),
                    marks: Marks::of(event),
                    since: event.at,
                    history: Vec::new(),
                });
                let before = work.status.clone();
                if let Some(title) = &set.title {
                    work.title = title.clone();
                }
                if let Some(goal) = &set.goal {
                    work.goal = Some(goal.clone());
                }
                if let Some(status) = &set.status {
                    work.status = status.clone();
                }
                if let Some(line) = &set.line {
                    work.line = line.clone();
                }
                work.done.extend(set.done.iter().cloned());
                if let Some(next) = &set.next {
                    work.next = next.clone();
                }
                let private = work.marks.private || event.private;
                work.marks = Marks { private, ..Marks::of(event) };
                let mut what = match &work.status {
                    Status::Blocked(why) => format!("blocked ({why})"),
                    other => other.word().to_string(),
                };
                if !fresh && before == work.status {
                    what = "updated".to_string();
                }
                if let Some(line) = &set.line {
                    what = format!("{what} · {line}");
                }
                work.history.push(Change { at: event.at, by: event.by.clone(), what });
            }
            What::Ask(open) => {
                // Two writers can draw the same short name. The later one
                // is then called by a longer piece of its own id.
                let name = if self.asks.contains_key(&open.ask) {
                    id::short(&event.id, 12)
                } else {
                    open.ask.clone()
                };
                let mut marks = Marks::of(event);
                if let Some(work) = &open.work
                    && let Some(work) = self.works.get(&(event.project.clone(), work.clone()))
                {
                    marks.private |= work.marks.private;
                }
                self.asks.entry(name.clone()).or_insert_with(|| Ask {
                    id: name,
                    question: open.question.clone(),
                    blocks: open.blocks.clone(),
                    recommends: open.recommends.clone(),
                    work: open.work.clone(),
                    marks,
                    closed: None,
                });
            }
            What::Answer(answer) => self.close(&answer.ask, Closed::Answered(answer.text.clone()), event),
            What::Withdraw(withdraw) => {
                self.close(&withdraw.ask, Closed::Withdrawn(withdraw.why.clone()), event)
            }
            What::Learn(learn) => {
                let name = if self.lessons.contains_key(&learn.lesson) {
                    id::short(&event.id, 12)
                } else {
                    learn.lesson.clone()
                };
                self.lessons.entry(name.clone()).or_insert_with(|| Lesson {
                    id: name,
                    rule: learn.rule.clone(),
                    moment: learn.moment.clone(),
                    when: learn.when.clone(),
                    everywhere: learn.everywhere,
                    stop: learn.stop,
                    marks: Marks::of(event),
                    retired: None,
                    shown: 0,
                    last_shown: None,
                    stopped: 0,
                    sessions: BTreeSet::new(),
                });
            }
            What::Retire(retire) => {
                if let Some(lesson) = self.lessons.get_mut(&retire.lesson)
                    && lesson.retired.is_none()
                {
                    lesson.retired = Some((retire.why.clone(), Marks::of(event)));
                }
            }
            What::Note(new) => {
                let name = if self.notes.contains_key(&new.note) {
                    id::short(&event.id, 12)
                } else {
                    new.note.clone()
                };
                let mut marks = Marks::of(event);
                if let Some(work) = &new.about
                    && let Some(work) = self.works.get(&(event.project.clone(), work.clone()))
                {
                    marks.private |= work.marks.private;
                }
                self.notes.entry(name.clone()).or_insert_with(|| Note {
                    id: name,
                    text: new.text.clone(),
                    tags: new.tags.clone(),
                    about: new.about.clone(),
                    marks,
                    forgotten: None,
                });
            }
            What::Forget(forget) => {
                if let Some(note) = self.notes.get_mut(&forget.note)
                    && note.forgotten.is_none()
                {
                    note.forgotten = Some((forget.why.clone(), Marks::of(event)));
                }
            }
            What::Shown(shown) => {
                for name in &shown.lessons {
                    if let Some(lesson) = self.lessons.get_mut(name) {
                        lesson.shown += 1;
                        lesson.stopped += u32::from(shown.stopped);
                        lesson.last_shown = Some(event.at);
                        if let Some(session) = &shown.session {
                            lesson.sessions.insert(session.clone());
                        }
                    }
                }
            }
            What::Unknown => self.unknown += 1,
        }
    }

    /// The first close of an ask stands. A second one, from a copy that had
    /// not seen the first, changes nothing.
    fn close(&mut self, ask: &str, how: Closed, event: &Event) {
        if let Some(ask) = self.asks.get_mut(ask)
            && ask.closed.is_none()
        {
            let private = ask.marks.private || event.private;
            ask.closed = Some((how, Marks { private, ..Marks::of(event) }));
        }
    }

    pub fn is_empty(&self) -> bool {
        self.works.is_empty() && self.asks.is_empty() && self.lessons.is_empty() && self.notes.is_empty()
    }

    pub fn lesson(&self, id: &str) -> Option<&Lesson> {
        self.lessons.get(id)
    }

    pub fn note(&self, id: &str) -> Option<&Note> {
        self.notes.get(id)
    }

    pub fn find_lesson(&self, name: &str) -> Result<&Lesson, NoAsk> {
        find(&self.lessons, "lesson:", name)
    }

    pub fn find_note(&self, name: &str) -> Result<&Note, NoAsk> {
        find(&self.notes, "note:", name)
    }

    /// The lessons in force that reach this scope, the newest first.
    pub fn lessons(&self, scope: Scope<'_>) -> Vec<&Lesson> {
        let mut lessons: Vec<&Lesson> =
            self.lessons.values().filter(|l| l.in_force() && scope.reaches(&l.marks, l.everywhere)).collect();
        lessons.sort_by_key(|l| (std::cmp::Reverse(l.marks.at), l.id.clone()));
        lessons
    }

    /// The lessons that apply to what is about to be done: those that stop
    /// the act first, then the newest. Nothing that does not clearly belong
    /// to the act is returned, and returning nothing is a correct answer.
    pub fn check(&self, scope: Scope<'_>, doing: &str) -> Vec<Applies<'_>> {
        let doing = words::terms(doing);
        let mut applies: Vec<Applies<'_>> = self
            .lessons(scope)
            .into_iter()
            .filter_map(|lesson| {
                words::marks(&lesson.moment, &lesson.when, &doing).map(|because| Applies { lesson, because })
            })
            .collect();
        applies.sort_by_key(|a| !a.lesson.stop);
        applies
    }

    /// The notes still kept that this scope sees, the newest first.
    pub fn notes(&self, scope: Scope<'_>) -> Vec<&Note> {
        let mut notes: Vec<&Note> =
            self.notes.values().filter(|n| n.forgotten.is_none() && scope.sees(&n.marks)).collect();
        notes.sort_by_key(|n| (std::cmp::Reverse(n.marks.at), n.id.clone()));
        notes
    }

    /// Everything held that can be searched for. What was withdrawn,
    /// retired or forgotten is not among it.
    pub fn things(&self) -> Vec<Thing> {
        let mut things = Vec::new();
        for work in self.works.values() {
            let mut detail = vec![format!("{} · {}", self.standing(work).word(), work.line)];
            detail.extend(work.goal.iter().map(|g| format!("goal: {g}")));
            detail.extend(work.done.iter().map(|d| format!("done: {d}")));
            detail.extend(work.next.iter().map(|n| format!("next: {n}")));
            things.push(Thing {
                key: format!("work:{}/{}", work.marks.project, work.slug),
                kind: "work",
                title: if work.title == work.slug {
                    work.slug.clone()
                } else {
                    format!("{} ({})", work.title, work.slug)
                },
                detail,
                marks: work.marks.clone(),
                everywhere: false,
            });
        }
        for ask in self.asks.values() {
            match &ask.closed {
                None => {
                    let mut detail = Vec::new();
                    detail.extend(ask.blocks.iter().map(|b| format!("blocks {b}")));
                    detail.extend(ask.recommends.iter().map(|r| format!("recommends {r}")));
                    things.push(Thing {
                        key: format!("ask:{}", ask.id),
                        kind: "ask",
                        title: ask.question.clone(),
                        detail,
                        marks: ask.marks.clone(),
                        everywhere: false,
                    });
                }
                Some((Closed::Answered(text), marks)) => things.push(Thing {
                    key: format!("decision:{}", ask.id),
                    kind: "decision",
                    title: ask.question.clone(),
                    detail: vec![format!("answer: {text}")],
                    marks: marks.clone(),
                    everywhere: false,
                }),
                Some((Closed::Withdrawn(_), _)) => {}
            }
        }
        for lesson in self.lessons.values().filter(|l| l.in_force()) {
            things.push(Thing {
                key: format!("lesson:{}", lesson.id),
                kind: "lesson",
                title: lesson.rule.clone(),
                detail: vec![format!("when: {}", lesson.moment)],
                marks: lesson.marks.clone(),
                everywhere: lesson.everywhere,
            });
        }
        for note in self.notes.values().filter(|n| n.forgotten.is_none()) {
            let mut detail = Vec::new();
            if !note.tags.is_empty() {
                detail.push(format!("tags: {}", note.tags.join(", ")));
            }
            detail.extend(note.about.iter().map(|a| format!("about {a}")));
            things.push(Thing {
                key: format!("note:{}", note.id),
                kind: "note",
                title: note.text.clone(),
                detail,
                marks: note.marks.clone(),
                everywhere: false,
            });
        }
        things
    }

    /// What is known about something, without an index: the things that
    /// hold every term asked for, the newest first. An empty answer is a
    /// correct one. Whoever has a database asks it instead.
    pub fn recall(&self, scope: Scope<'_>, about: &str) -> Vec<Thing> {
        let wanted = words::terms(about);
        if wanted.is_empty() {
            return Vec::new();
        }
        let mut found: Vec<Thing> = self
            .things()
            .into_iter()
            .filter(|t| scope.reaches(&t.marks, t.everywhere))
            .filter(|t| wanted.is_subset(&words::terms(&t.text())))
            .collect();
        found.sort_by_key(|t| (std::cmp::Reverse(t.marks.at), t.key.clone()));
        found
    }

    /// The past of a lesson, oldest first.
    pub fn lesson_history(&self, lesson: &Lesson) -> Vec<Change> {
        let mut past =
            vec![Change { at: lesson.marks.at, by: lesson.marks.who.clone(), what: "learned".to_string() }];
        if let Some(at) = lesson.last_shown {
            let stopped = if lesson.stopped > 0 {
                format!(", stopped the act {}", times(lesson.stopped))
            } else {
                String::new()
            };
            past.push(Change {
                at,
                by: lesson.marks.who.clone(),
                what: format!("shown {}{stopped}", times(lesson.shown)),
            });
        }
        if let Some((why, marks)) = &lesson.retired {
            past.push(Change { at: marks.at, by: marks.who.clone(), what: format!("retired · {why}") });
        }
        past
    }

    pub fn work(&self, project: &str, slug: &str) -> Option<&Work> {
        self.works.get(&(project.to_string(), slug.to_string()))
    }

    pub fn ask(&self, id: &str) -> Option<&Ask> {
        self.asks.get(id)
    }

    /// The ask a name leads to: its whole name, with or without `ask:`, or
    /// the start of it when only one ask starts that way.
    pub fn find_ask(&self, name: &str) -> Result<&Ask, NoAsk> {
        find(&self.asks, "ask:", name)
    }

    /// Where a piece of work stands: its card, unless an open ask holds it.
    pub fn standing(&self, work: &Work) -> Standing {
        if work.status == Status::Done {
            return Standing::Done;
        }
        let waiting: Vec<String> = self
            .asks
            .values()
            .filter(|a| {
                a.is_open() && a.marks.project == work.marks.project && a.work.as_deref() == Some(&work.slug)
            })
            .map(|a| a.id.clone())
            .collect();
        if !waiting.is_empty() {
            return Standing::Waiting(waiting);
        }
        match &work.status {
            Status::Moving => Standing::Moving,
            Status::Blocked(why) => Standing::Blocked(why.clone()),
            Status::NotStarted => Standing::NotStarted,
            Status::Done => Standing::Done,
        }
    }

    /// What waits on the person, the longest wait first.
    pub fn open_asks(&self, scope: Scope<'_>) -> Vec<&Ask> {
        let mut asks: Vec<&Ask> =
            self.asks.values().filter(|a| a.is_open() && scope.sees(&a.marks)).collect();
        asks.sort_by_key(|a| (a.marks.at, a.id.clone()));
        asks
    }

    /// The works on the board: what needs a person first, then the most
    /// recently touched. Finished work leaves a day after it was finished.
    pub fn works(&self, scope: Scope<'_>, now: Time) -> Vec<&Work> {
        let mut works: Vec<&Work> = self
            .works
            .values()
            .filter(|w| scope.sees(&w.marks))
            .filter(|w| w.status != Status::Done || w.marks.at.until(now) < DONE_STAYS)
            .collect();
        works.sort_by_key(|w| (self.standing(w).rank(), std::cmp::Reverse(w.marks.at), w.slug.clone()));
        works
    }

    /// Asks closed at or after `since`, the latest first.
    pub fn closed_asks(&self, scope: Scope<'_>, since: Time) -> Vec<&Ask> {
        let mut asks: Vec<&Ask> = self
            .asks
            .values()
            .filter(|a| scope.sees(&a.marks))
            .filter(|a| a.closed.as_ref().is_some_and(|(_, marks)| marks.at >= since))
            .collect();
        asks.sort_by_key(|a| std::cmp::Reverse(a.closed.as_ref().map(|(_, m)| m.at)));
        asks
    }

    /// The past of an ask, oldest first.
    pub fn ask_history(&self, ask: &Ask) -> Vec<Change> {
        let mut past =
            vec![Change { at: ask.marks.at, by: ask.marks.who.clone(), what: "asked".to_string() }];
        if let Some((how, marks)) = &ask.closed {
            let what = match how {
                Closed::Answered(text) => format!("answered · {text}"),
                Closed::Withdrawn(why) => format!("withdrawn · {why}"),
            };
            past.push(Change { at: marks.at, by: marks.who.clone(), what });
        }
        past
    }
}

fn times(n: u32) -> String {
    if n == 1 { "once".to_string() } else { format!("{n} times") }
}
