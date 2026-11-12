//! Nahuali's renderer-agnostic view model.
//!
//! - [`model`]: the frozen tree (`Screen`, `Node`, `Tone`, `MascotState`, ...).
//! - [`source`]: the [`ViewSource`](source::ViewSource) trait the store implements.
//! - [`screens`]: builders for Today, Journal, Intentions, Health and Search.
//! - [`keymap`]: one binding table per view, shared by key handling, footer
//!   hints and the help card.
//! - [`state`]: UI state and the pure transitions keys trigger.
//! - [`mock`] (feature `mock`): an invented mind for demos and tests.

pub mod date;
pub mod keymap;
#[cfg(any(test, feature = "mock"))]
pub mod mock;
pub mod model;
pub mod screens;
pub mod source;
pub mod state;

pub use date::{Date, Stamp};
pub use keymap::{Action, View};
pub use model::{Icon, Item, KeyHint, MascotState, Node, Screen, Seg, Span, Tone};
pub use screens::build;
pub use source::ViewSource;
pub use state::{Effect, UiState};
