//! What the screens read: the things Nahuali holds (`docs/product.md`) and
//! the four marks every one of them carries. Whatever stores them implements
//! [`ViewSource`]; nothing here knows how.

use crate::date::Stamp;

/// Who the session is for, as the header names them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    pub name: String,
}

/// Where an item came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Who {
    /// The person said it.
    Person(String),
    /// An agent saw it itself.
    Agent(String),
    /// An agent passes on what another agent told it.
    Reported(String),
}

impl Who {
    pub fn line(&self) -> String {
        match self {
            Self::Person(name) => format!("said by {name}"),
            Self::Agent(name) => format!("observed by {name}"),
            Self::Reported(name) => format!("reported by {name}"),
        }
    }
}

/// How far an item can be trusted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sure {
    /// Checked, and by what.
    Verified(String),
    /// Said, and checked by nothing.
    Claimed,
}

impl Sure {
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified(_))
    }

    pub fn line(&self) -> String {
        match self {
            Self::Verified(by) => format!("verified · {by}"),
            Self::Claimed => "claimed, not verified".to_string(),
        }
    }
}

/// The four marks: who, how sure, as of when, and how far it reaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marks {
    pub who: Who,
    pub sure: Sure,
    pub as_of: Stamp,
    /// For anything about code: the commit it was true at.
    pub commit: Option<String>,
    pub project: String,
    pub private: bool,
}

impl Marks {
    /// The marks as rows, in the order every screen shows them.
    pub fn rows(&self, now: Stamp) -> Vec<(String, String)> {
        let as_of = match &self.commit {
            Some(commit) => format!("{} ago · commit {commit}", self.as_of.age(now)),
            None => format!("{} ago", self.as_of.age(now)),
        };
        let reach = if self.private { format!("{} · private", self.project) } else { self.project.clone() };
        vec![
            ("who".to_string(), self.who.line()),
            ("how sure".to_string(), self.sure.line()),
            ("as of".to_string(), as_of),
            ("reach".to_string(), reach),
        ]
    }
}

/// Where a piece of work stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkStatus {
    Moving,
    /// Stopped, and by what.
    Blocked(String),
    /// Stopped on an answer from the person.
    Waiting,
    NotStarted,
    Done,
}

impl WorkStatus {
    pub fn word(&self) -> &'static str {
        match self {
            Self::Moving => "moving",
            Self::Blocked(_) => "blocked",
            Self::Waiting => "waiting on you",
            Self::NotStarted => "not started",
            Self::Done => "done",
        }
    }
}

/// A named piece of work and its one state card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Work {
    pub id: String,
    pub title: String,
    pub status: WorkStatus,
    /// The card in one line: what is true now.
    pub line: String,
    pub done: Vec<String>,
    pub next: Vec<String>,
    pub marks: Marks,
}

/// A question for the person. It was asked at `marks.as_of`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    pub id: String,
    pub question: String,
    /// What cannot go on until it is answered.
    pub blocks: Option<String>,
    pub recommends: Option<String>,
    pub marks: Marks,
}

/// A rule learned from a failure, and the moment it applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lesson {
    pub id: String,
    pub rule: String,
    /// When an agent is told: "before spawning agents in worktrees".
    pub moment: String,
    pub marks: Marks,
}

/// One step in the past of an item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub at: Stamp,
    pub by: String,
    pub what: String,
}

/// The first screen's data: what waits on the person, what is in motion and
/// what was learned lately.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Board {
    pub asks: Vec<Ask>,
    pub works: Vec<Work>,
    /// Lessons of the last seven days.
    pub lessons: Vec<Lesson>,
}

impl Board {
    /// Every mark on the board, to count what is verified.
    pub fn marks(&self) -> impl Iterator<Item = &Marks> {
        self.asks
            .iter()
            .map(|a| &a.marks)
            .chain(self.works.iter().map(|w| &w.marks))
            .chain(self.lessons.iter().map(|l| &l.marks))
    }
}

/// What a screen can ask of whatever holds the memory.
pub trait ViewSource {
    fn person(&self) -> Person;
    /// The local time the screens count ages from.
    fn now(&self) -> Stamp;
    fn board(&self) -> Board;
    /// Every lesson still in force, newest first.
    fn lessons(&self) -> Vec<Lesson>;
    /// The past of one item, newest first.
    fn history(&self, id: &str) -> Vec<Change>;
    /// Answer an ask: it leaves the board and becomes a decision.
    fn answer(&self, ask: &str, text: &str) -> Result<(), String>;
}
