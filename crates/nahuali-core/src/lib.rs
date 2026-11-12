//! What Nahuali holds and how it is kept.
//!
//! Everything that happens is one line appended to a log. What is true now
//! (the cards, the open asks, the decisions) is that log folded in order.
//! Nothing else is stored.
//!
//! This crate has no clock, no randomness, no network and no process of its
//! own. The caller says what time it is and who is speaking ([`Signature`]),
//! and where lines are kept ([`Journal`]). The same code therefore runs
//! behind a command line, inside a long-lived program, or on a machine with
//! no terminal at all.
//!
//! - [`event`]: the lines.
//! - [`state`]: the fold, and what can be read from it.
//! - [`journal`]: where lines are kept; files by default.
//! - [`mind`]: the verbs.

pub mod event;
pub mod id;
pub mod journal;
pub mod mind;
pub mod state;
pub mod time;

pub use event::{Actor, Event, Status, Sure, What};
pub use journal::{FileJournal, Journal, MemJournal};
pub use mind::{Error, Mind, NewAsk, Signature, Update};
pub use state::{Ask, Change, Closed, Marks, Scope, Standing, State, Work};
pub use time::Time;
