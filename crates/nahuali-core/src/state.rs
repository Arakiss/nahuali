//! What is true now: the log folded in order.
//!
//! The order is the moment of each event and then its id, never the order
//! of the files or of the lines in them. Two copies of the same lines, read
//! in any order, give the same state: that is what lets two machines share
//! a log by copying files.

use std::collections::BTreeMap;

use crate::event::{Actor, Event, Status, Sure, What};
use crate::id;
use crate::time::Time;

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
}

/// Why a name did not lead to one ask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoAsk {
    Unknown,
    /// Several asks start like that.
    Ambiguous(Vec<String>),
}

/// A finished piece of work leaves the board a day later.
const DONE_STAYS: u64 = 86_400;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    works: BTreeMap<(String, String), Work>,
    asks: BTreeMap<String, Ask>,
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
        self.works.is_empty() && self.asks.is_empty()
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
        let name = name.strip_prefix("ask:").unwrap_or(name).to_ascii_lowercase();
        if let Some(ask) = self.asks.get(&name) {
            return Ok(ask);
        }
        if name.is_empty() {
            return Err(NoAsk::Unknown);
        }
        let mut found = self.asks.range(name.clone()..).take_while(|(id, _)| id.starts_with(&name));
        match (found.next(), found.next()) {
            (Some((_, ask)), None) => Ok(ask),
            (Some((a, _)), Some((b, _))) => Err(NoAsk::Ambiguous(vec![a.clone(), b.clone()])),
            _ => Err(NoAsk::Unknown),
        }
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
