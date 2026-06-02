//! Human-facing rendering of Nahuali's trust/authority layer.
//!
//! Nahuali's value is its governed/auditable trust layer, so the human
//! (non-`--json`) output renders authority modes as a plain-language label
//! plus a short gloss, colored by severity. Color is emitted only when stdout
//! is a TTY and `NO_COLOR` is unset; `--json` output is never touched.

use std::io::IsTerminal;

use nahuali_core::{AuthorityMode, RecallResultTrustMode};

const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

/// Whether colored output should be emitted on stdout.
///
/// Honors `NO_COLOR` (any value disables color) and requires stdout to be a
/// terminal, so piped or redirected output stays plain for scripts and tests.
fn color_enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    std::io::stdout().is_terminal()
}

/// Plain-language label plus a short gloss for an authority mode, e.g.
/// `CERTIFY · trustworthy`. Keeps the English mode token plus a short gloss.
fn authority_label(mode: &AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::Certify => "CERTIFY · trustworthy",
        AuthorityMode::Advisory => "ADVISORY · use with judgment",
        AuthorityMode::Warn => "WARN · verify before use",
        AuthorityMode::Block => "BLOCK · not yet trustworthy",
    }
}

/// Plain-language label plus a short gloss for a per-result trust mode.
fn trust_label(mode: &RecallResultTrustMode) -> &'static str {
    match mode {
        RecallResultTrustMode::Certify => "CERTIFY · trustworthy",
        RecallResultTrustMode::Advisory => "ADVISORY · use with judgment",
        RecallResultTrustMode::Warn => "WARN · verify before use",
        RecallResultTrustMode::Block => "BLOCK · not yet trustworthy",
    }
}

fn authority_color(mode: &AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::Certify => GREEN,
        AuthorityMode::Advisory => CYAN,
        AuthorityMode::Warn => YELLOW,
        AuthorityMode::Block => RED,
    }
}

fn trust_color(mode: &RecallResultTrustMode) -> &'static str {
    match mode {
        RecallResultTrustMode::Certify => GREEN,
        RecallResultTrustMode::Advisory => CYAN,
        RecallResultTrustMode::Warn => YELLOW,
        RecallResultTrustMode::Block => RED,
    }
}

fn paint(text: &str, color: &str) -> String {
    if color_enabled() {
        format!("{color}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// Colored plain-language badge for a store-level authority mode.
pub(crate) fn authority_badge(mode: &AuthorityMode) -> String {
    paint(authority_label(mode), authority_color(mode))
}

/// Colored plain-language badge for a per-result trust mode.
pub(crate) fn trust_badge(mode: &RecallResultTrustMode) -> String {
    paint(trust_label(mode), trust_color(mode))
}

/// Render the canonical store-trust line, e.g.
/// `Store trust: CERTIFY · trustworthy (score 1.00)`.
pub(crate) fn store_trust_line(mode: &AuthorityMode, score: f32) -> String {
    format!(
        "Store trust: {} (score {:.2})",
        authority_badge(mode),
        score
    )
}
