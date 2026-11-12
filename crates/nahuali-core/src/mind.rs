//! The verbs. Each one checks what it is given, appends one line and folds
//! it in. The caller brings the moment and the speaker: this code has no
//! clock and draws nothing at random.

use std::fmt;
use std::io;

use crate::event::{self, Actor, AskOpen, Event, Line, Status, Sure, VERSION, What, WorkSet};
use crate::id;
use crate::journal::Journal;
use crate::state::{Ask, Closed, NoAsk, State, Work};
use crate::time::Time;

/// Who writes a line, when, and with which marks.
#[derive(Clone, Debug)]
pub struct Signature {
    pub who: Actor,
    /// Milliseconds since 1970, UTC.
    pub at_ms: u64,
    /// Bits drawn at random by the caller, new for every line.
    pub entropy: u128,
    pub project: String,
    pub commit: Option<String>,
    pub private: bool,
    pub sure: Sure,
}

/// A card update: a field left out keeps what it had.
pub type Update = WorkSet;

#[derive(Clone, Debug, Default)]
pub struct NewAsk {
    pub question: String,
    pub blocks: Option<String>,
    pub recommends: Option<String>,
    pub work: Option<String>,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// A name that is not lower-case letters, digits and dashes.
    BadName {
        what: &'static str,
        given: String,
    },
    /// A text that has to say something and says nothing.
    Empty(&'static str),
    TooLong {
        what: &'static str,
        limit: usize,
    },
    /// An update that changes nothing.
    Nothing,
    NoSuchWork(String),
    NoSuchAsk(String),
    AmbiguousAsk {
        given: String,
        could_be: Vec<String>,
    },
    /// The ask was closed before, and how.
    AlreadyClosed {
        ask: String,
        how: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "the memory could not be read or written: {e}"),
            Self::BadName { what, given } => {
                write!(f, "`{given}` is not a name for {what}: use lower-case letters, digits and dashes")
            }
            Self::Empty(what) => write!(f, "{what} needs words"),
            Self::TooLong { what, limit } => write!(f, "{what} is one line of at most {limit} characters"),
            Self::Nothing => write!(
                f,
                "an update says at least one thing: a status, a line, something done or what is next"
            ),
            Self::NoSuchWork(work) => write!(f, "there is no work called `{work}` in this project"),
            Self::NoSuchAsk(ask) => write!(f, "there is no ask called `{ask}`"),
            Self::AmbiguousAsk { given, could_be } => {
                write!(f, "`{given}` could be ask:{}", could_be.join(" or ask:"))
            }
            Self::AlreadyClosed { ask, how } => write!(f, "ask:{ask} was already {how}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

const LINE: usize = 280;
const TEXT: usize = 600;

/// One line, with runs of white space made one space.
fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn said(what: &'static str, text: &str, limit: usize) -> Result<String, Error> {
    let text = clean(text);
    if text.is_empty() {
        return Err(Error::Empty(what));
    }
    if text.chars().count() > limit {
        return Err(Error::TooLong { what, limit });
    }
    Ok(text)
}

fn maybe(what: &'static str, text: &Option<String>, limit: usize) -> Result<Option<String>, Error> {
    text.as_deref().map(|t| said(what, t, limit)).transpose()
}

fn named(what: &'static str, given: &str) -> Result<(), Error> {
    if id::is_slug(given) { Ok(()) } else { Err(Error::BadName { what, given: given.to_string() }) }
}

pub struct Mind<J> {
    journal: J,
    events: Vec<Event>,
    state: State,
    /// Lines that could not be read: torn by a crash, or edited by hand.
    pub damaged: usize,
    /// Lines written by a newer version.
    pub newer: usize,
}

impl<J: Journal> Mind<J> {
    pub fn open(journal: J) -> Result<Self, Error> {
        let mut mind = Self { journal, events: Vec::new(), state: State::default(), damaged: 0, newer: 0 };
        mind.reload()?;
        Ok(mind)
    }

    /// Read the journal again: another writer may have added to it.
    pub fn reload(&mut self) -> Result<(), Error> {
        let (mut events, mut damaged, mut newer) = (Vec::new(), 0, 0);
        for line in self.journal.read()? {
            match Event::read(&line) {
                Line::Event(event) => events.push(*event),
                Line::Damaged => damaged += 1,
                Line::Newer => newer += 1,
            }
        }
        self.state = State::fold(&events);
        (self.events, self.damaged, self.newer) = (events, damaged, newer);
        Ok(())
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn journal(&self) -> &J {
        &self.journal
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    fn write(&mut self, sig: Signature, what: What) -> Result<String, Error> {
        named("a project", &sig.project)?;
        let event = Event {
            v: VERSION,
            id: id::ulid(sig.at_ms, sig.entropy),
            at: Time::from_millis(sig.at_ms),
            by: sig.who,
            project: sig.project,
            private: sig.private,
            sure: sig.sure,
            commit: sig.commit,
            what,
        };
        self.journal.append(&event.line())?;
        let id = event.id.clone();
        self.events.push(event);
        self.state = State::fold(&self.events);
        Ok(id)
    }

    /// Overwrite a work's card. The first update of a name starts the work.
    pub fn update(&mut self, sig: Signature, set: Update) -> Result<Work, Error> {
        named("a piece of work", &set.work)?;
        let status = match &set.status {
            Some(Status::Blocked(why)) => Some(Status::Blocked(said("what blocks it", why, LINE)?)),
            other => other.clone(),
        };
        let set = WorkSet {
            work: set.work.clone(),
            title: maybe("a title", &set.title, LINE)?,
            goal: maybe("a goal", &set.goal, TEXT)?,
            status,
            line: maybe("the card's line", &set.line, LINE)?,
            done: set.done.iter().map(|d| said("something done", d, LINE)).collect::<Result<_, _>>()?,
            next: set
                .next
                .as_ref()
                .map(|n| n.iter().map(|x| said("what is next", x, LINE)).collect::<Result<_, _>>())
                .transpose()?,
        };
        let says_something = set.title.is_some()
            || set.goal.is_some()
            || set.status.is_some()
            || set.line.is_some()
            || !set.done.is_empty()
            || set.next.is_some();
        let starts = self.state.work(&sig.project, &set.work).is_none();
        if !says_something && !starts {
            return Err(Error::Nothing);
        }
        let (project, slug) = (sig.project.clone(), set.work.clone());
        self.write(sig, What::Work(set))?;
        Ok(self.state.work(&project, &slug).expect("it was just written").clone())
    }

    /// Put a question to the person. It stays open, and in sight, until it
    /// is answered or withdrawn.
    pub fn ask(&mut self, sig: Signature, new: NewAsk) -> Result<Ask, Error> {
        let question = said("a question", &new.question, TEXT)?;
        let blocks = maybe("what it blocks", &new.blocks, LINE)?;
        let recommends = maybe("a recommendation", &new.recommends, TEXT)?;
        if let Some(work) = &new.work {
            named("a piece of work", work)?;
            if self.state.work(&sig.project, work).is_none() {
                return Err(Error::NoSuchWork(work.clone()));
            }
        }
        let id = id::ulid(sig.at_ms, sig.entropy);
        let name = [6, 8, 10, 12]
            .into_iter()
            .map(|len| id::short(&id, len))
            .find(|name| self.state.ask(name).is_none())
            .expect("twelve characters of an id are not taken");
        let open = AskOpen { ask: name.clone(), question, blocks, recommends, work: new.work };
        self.write(sig, What::Ask(open))?;
        Ok(self.state.ask(&name).expect("it was just written").clone())
    }

    fn open_ask(&self, name: &str) -> Result<Ask, Error> {
        let ask = self.state.find_ask(name).map_err(|e| match e {
            NoAsk::Unknown => Error::NoSuchAsk(name.to_string()),
            NoAsk::Ambiguous(could_be) => Error::AmbiguousAsk { given: name.to_string(), could_be },
        })?;
        if let Some((how, marks)) = &ask.closed {
            let how = match how {
                Closed::Answered(text) => format!("answered by {}: {text}", marks.who.line()),
                Closed::Withdrawn(why) => format!("withdrawn by {}: {why}", marks.who.line()),
            };
            return Err(Error::AlreadyClosed { ask: ask.id.clone(), how });
        }
        Ok(ask.clone())
    }

    fn close(&mut self, mut sig: Signature, ask: &Ask, what: What) -> Result<Ask, Error> {
        // The close belongs where the ask belongs, wherever it is said from.
        sig.project = ask.marks.project.clone();
        sig.private |= ask.marks.private;
        self.write(sig, what)?;
        Ok(self.state.ask(&ask.id).expect("it was there").clone())
    }

    /// Close an ask with an answer. It becomes a decision.
    pub fn answer(&mut self, sig: Signature, ask: &str, text: &str) -> Result<Ask, Error> {
        let text = said("an answer", text, TEXT)?;
        let ask = self.open_ask(ask)?;
        let what = What::Answer(event::Answer { ask: ask.id.clone(), text });
        self.close(sig, &ask, what)
    }

    /// Close an ask that no longer needs an answer.
    pub fn withdraw(&mut self, sig: Signature, ask: &str, why: &str) -> Result<Ask, Error> {
        let why = said("the reason", why, TEXT)?;
        let ask = self.open_ask(ask)?;
        let what = What::Withdraw(event::Withdraw { ask: ask.id.clone(), why });
        self.close(sig, &ask, what)
    }
}
