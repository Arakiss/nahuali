//! The lines of the log. One event is one line of JSON, and it carries the
//! four marks: who, how sure, as of when, and how far it reaches.
//!
//! A reader never refuses a log because of one line. A line it cannot read
//! is counted and skipped; a line written by a newer Nahuali is counted and
//! skipped; a kind of event it does not know is kept as [`What::Unknown`].
//! Two installations on different versions can therefore share a log.

use serde::{Deserialize, Deserializer, Serialize};

use crate::time::Time;

/// The version this code writes.
pub const VERSION: u32 = 1;

/// Who is speaking.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Actor {
    /// The person said it.
    Person(String),
    /// An agent saw it itself.
    Agent(String),
    /// `by` passes on what `from` told it.
    Reported { by: String, from: String },
}

impl Actor {
    /// The one who wrote the line.
    pub fn name(&self) -> &str {
        match self {
            Self::Person(name) | Self::Agent(name) => name,
            Self::Reported { by, .. } => by,
        }
    }

    pub fn is_person(&self) -> bool {
        matches!(self, Self::Person(_))
    }

    /// `ada`, `claude-code`, `claude-code (from reviewer)`.
    pub fn line(&self) -> String {
        match self {
            Self::Person(name) | Self::Agent(name) => name.clone(),
            Self::Reported { by, from } => format!("{by} (from {from})"),
        }
    }
}

/// How far a line can be trusted.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sure {
    /// Said, and checked by nothing.
    #[default]
    Claimed,
    /// Checked, and by what.
    Verified(String),
}

impl Sure {
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified(_))
    }

    pub fn line(&self) -> String {
        match self {
            Self::Claimed => "claimed".to_string(),
            Self::Verified(by) => format!("verified by {by}"),
        }
    }
}

/// Where a piece of work stands, as its card says. Waiting on the person is
/// not written here: it follows from an open ask (see `state::Standing`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Moving,
    /// Stopped, and by what.
    Blocked(String),
    NotStarted,
    Done,
}

impl Status {
    pub fn word(&self) -> &'static str {
        match self {
            Self::Moving => "moving",
            Self::Blocked(_) => "blocked",
            Self::NotStarted => "not started",
            Self::Done => "done",
        }
    }
}

/// Overwrite a work's card. A field left out keeps what it had.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkSet {
    pub work: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    /// The card in one line: what is true now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    /// Things finished since the last update. They add to the card.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub done: Vec<String>,
    /// What comes next. It replaces what the card said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Vec<String>>,
}

/// Put a question to the person.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskOpen {
    /// The short name it is called by.
    pub ask: String,
    pub question: String,
    /// What cannot go on until it is answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommends: Option<String>,
    /// The piece of work that waits on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work: Option<String>,
    /// Who is asked, when it is not the person the asker works for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
}

/// Close an ask with an answer: it becomes a decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub ask: String,
    pub text: String,
}

/// Close an ask without an answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Withdraw {
    pub ask: String,
    pub why: String,
}

/// A rule learned from a failure, and the moment it applies.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Learn {
    /// The short name it is called by.
    pub lesson: String,
    pub rule: String,
    /// When it applies, in words: "before spawning an agent in a worktree".
    pub moment: String,
    /// What marks that moment in what an agent is about to do. Each entry
    /// is a few words that must all be there: "worktree add".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub when: Vec<String>,
    /// It applies to every project, not only the one it was learned in.
    #[serde(default, skip_serializing_if = "is_false")]
    pub everywhere: bool,
    /// Serious enough to stop the act once, not only to be told.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stop: bool,
}

/// A lesson that stopped being true.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retire {
    pub lesson: String,
    pub why: String,
}

/// Something worth knowing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteNew {
    pub note: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The piece of work it is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
}

/// A note that went stale or was wrong.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forget {
    pub note: String,
    pub why: String,
}

/// Lessons were shown to someone about to act. Written by the machine.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shown {
    pub lessons: Vec<String>,
    /// What they were about to do.
    pub doing: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The act was stopped once, not only spoken to.
    #[serde(default, skip_serializing_if = "is_false")]
    pub stopped: bool,
}

/// What happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum What {
    Work(WorkSet),
    Ask(AskOpen),
    Answer(Answer),
    Withdraw(Withdraw),
    Learn(Learn),
    Retire(Retire),
    Note(NoteNew),
    Forget(Forget),
    Shown(Shown),
    /// A kind this version does not know. Read, never written.
    #[serde(skip_serializing)]
    Unknown,
}

fn lenient<'de, D: Deserializer<'de>>(d: D) -> Result<What, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).unwrap_or(What::Unknown))
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// One line of the log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub v: u32,
    /// Sorts by the moment it was made (see [`crate::id::ulid`]).
    pub id: String,
    pub at: Time,
    pub by: Actor,
    /// The person the speaker works for: themselves, when a person speaks.
    /// Absent in lines written before people were told apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<String>,
    /// Only that person and their agents see it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub mine: bool,
    pub project: String,
    /// Not to be offered to work on another project.
    #[serde(default, skip_serializing_if = "is_false")]
    pub private: bool,
    #[serde(default)]
    pub sure: Sure,
    /// For anything about code: the commit it was true at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(deserialize_with = "lenient")]
    pub what: What,
}

/// What one line of a log turned out to be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Line {
    Event(Box<Event>),
    /// Written by a newer version than this code.
    Newer,
    /// Not a line this code or any version of it wrote: torn by a crash, or
    /// edited by hand.
    Damaged,
}

impl Event {
    /// The line as it is appended, without its newline.
    pub fn line(&self) -> String {
        serde_json::to_string(self).expect("an event is plain data")
    }

    pub fn read(line: &str) -> Line {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else { return Line::Damaged };
        match value.get("v").and_then(serde_json::Value::as_u64) {
            None => Line::Damaged,
            Some(v) if v > u64::from(VERSION) => Line::Newer,
            Some(_) => match serde_json::from_value::<Event>(value) {
                Ok(mut event) => {
                    // A line is one person's. If it says another person
                    // spoke, it is read as what it is: someone working for
                    // the line's own person, saying so.
                    if let (Actor::Person(said), Some(owner)) = (&event.by, &event.person)
                        && said != owner
                    {
                        event.by = Actor::Reported { by: owner.clone(), from: said.clone() };
                    }
                    Line::Event(Box::new(event))
                }
                Err(_) => Line::Damaged,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(what: What) -> Event {
        Event {
            v: VERSION,
            id: crate::id::ulid(1_794_496_811_000, 7),
            at: Time(1_794_496_811),
            by: Actor::Agent("claude-code".into()),
            person: None,
            mine: false,
            project: "atlas".into(),
            private: false,
            sure: Sure::Verified("cargo test".into()),
            commit: Some("abc1234".into()),
            what,
        }
    }

    #[test]
    fn a_line_is_plain_enough_to_read_without_nahuali() {
        let e = event(What::Work(WorkSet {
            work: "importer".into(),
            status: Some(Status::Blocked("review".into())),
            line: Some("tests green, review left".into()),
            ..WorkSet::default()
        }));
        let line = e.line();
        assert!(line.contains(r#""at":"2026-11-12T"#), "{line}");
        assert!(line.contains(r#""by":{"agent":"claude-code"}"#), "{line}");
        assert!(line.contains(r#""sure":{"verified":"cargo test"}"#), "{line}");
        assert!(line.contains(r#""status":{"blocked":"review"}"#), "{line}");
        assert!(!line.contains("private"), "what is not set is not written: {line}");
        assert!(!line.contains('\n'));
        assert_eq!(Event::read(&line), Line::Event(Box::new(e)));
    }

    #[test]
    fn a_kind_from_the_future_is_kept_as_unknown() {
        let line = r#"{"v":1,"id":"01","at":"2026-11-12T15:20:11Z","by":{"agent":"x"},"project":"atlas","what":{"dream":{"of":"sheep"}}}"#;
        let Line::Event(e) = Event::read(line) else { panic!("it is a readable line") };
        assert_eq!(e.what, What::Unknown);
        assert_eq!(e.sure, Sure::Claimed, "no mark of how sure means only claimed");
    }

    #[test]
    fn a_field_from_the_future_is_ignored() {
        let line = r#"{"v":1,"id":"01","at":"2026-11-12T15:20:11Z","by":{"person":"ada"},"project":"atlas","mood":"fine","what":{"answer":{"ask":"k3f9q2","text":"yes","tone":"firm"}}}"#;
        let Line::Event(e) = Event::read(line) else { panic!("it is a readable line") };
        assert_eq!(e.what, What::Answer(Answer { ask: "k3f9q2".into(), text: "yes".into() }));
    }

    #[test]
    fn lines_that_are_not_ours_are_told_apart() {
        assert_eq!(Event::read(r#"{"v":2,"anything":true}"#), Line::Newer);
        assert_eq!(Event::read(r#"{"v":1,"id":"01","at":"2026-11-1"#), Line::Damaged);
        assert_eq!(Event::read("hello"), Line::Damaged);
        assert_eq!(Event::read(r#"{"id":"01"}"#), Line::Damaged);
    }

    #[test]
    fn a_line_cannot_speak_as_a_person_it_does_not_belong_to() {
        let line = r#"{"v":1,"id":"01","at":"2026-11-12T15:20:11Z","by":{"person":"ada"},"person":"bob","project":"atlas","what":{"answer":{"ask":"k3f9q2","text":"yes"}}}"#;
        let Line::Event(e) = Event::read(line) else { panic!("it is a readable line") };
        assert_eq!(e.by, Actor::Reported { by: "bob".into(), from: "ada".into() });
        assert!(!e.by.is_person());
        let own = line.replace(r#""person":"bob""#, r#""person":"ada""#);
        let Line::Event(e) = Event::read(&own) else { panic!("it is a readable line") };
        assert_eq!(e.by, Actor::Person("ada".into()));
    }
}
