//! Clay-on-coffee terminal presentation layer for Nahuali.
//!
//! Rust has strong à-la-carte terminal crates (comfy-table, ratatui, …) but no
//! single cohesive suite the way Go has Charm. This crate is that cohesion for
//! Nahuali: one palette and one set of primitives, the same clay-on-coffee
//! identity as the project's HTML artifacts, so the CLI reports and the
//! interactive `explore` TUI speak the same visual language.
//!
//! It is a thin layer that *composes* the good libraries rather than
//! reinventing terminal rendering. Everything degrades to plain text when
//! stdout is not a terminal or `NO_COLOR` is set, so agent, hook, and `--json`
//! output stays clean — Nahuali is agent-first, and this layer is the human
//! supervisor's window onto what the agent stored and how much to trust it.

pub mod render;
pub mod style;
pub mod theme;
