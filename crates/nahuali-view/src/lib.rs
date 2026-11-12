//! Nahuali's renderer-agnostic view model.
//!
//! - [`model`]: the tree a renderer paints (`Screen`, `Node`, `Tone`, `MascotState`, ...).
//! - [`source`]: what the screens read, and the [`ViewSource`](source::ViewSource)
//!   trait whatever holds the memory implements.
//! - [`screens`]: builders for the Board and Lessons.
//! - [`keymap`]: one binding table per view, shared by key handling, footer
//!   hints and the help card.
//! - [`state`]: UI state and the pure transitions keys trigger.
//! - [`mock`] (feature `mock`): an invented memory for demos and tests.

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
