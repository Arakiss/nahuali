//! Nahuali's portable terminal UI: paints a [`nahuali_view::Screen`] with
//! ratatui + crossterm, with the animated axolotl through Kitty or iTerm2
//! images and a quantised half-block fallback.
//!
//! Entry point: [`run`] with any [`nahuali_view::ViewSource`].

pub mod anim;
pub mod app;
pub mod chrome;
pub mod mascot;
pub mod render;
pub mod text;
pub mod theme;

pub use app::{App, Options, run};
