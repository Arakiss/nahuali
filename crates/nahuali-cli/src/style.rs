//! Human-facing rendering of Nahuali's trust/authority layer.
//!
//! Nahuali's value is its governed/auditable trust layer, so the human
//! (non-`--json`) output renders authority modes as a plain-language label plus
//! a short gloss, colored by severity in the shared clay-on-coffee palette.
//! Generic styling lives in `nahuali-ui`; this module only maps Nahuali's core
//! trust types onto it. Color is emitted only when stdout is a TTY and
//! `NO_COLOR` is unset; `--json` output is never touched.

use nahuali_core::{AuthorityMode, RecallResultTrustMode};
use nahuali_ui::style;
use nahuali_ui::theme::{self, Rgb};

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

/// Palette color for an authority mode, by severity.
fn authority_color(mode: &AuthorityMode) -> Rgb {
    match mode {
        AuthorityMode::Certify => theme::GREEN,
        AuthorityMode::Advisory => theme::BLUE,
        AuthorityMode::Warn => theme::AMBER,
        AuthorityMode::Block => theme::RED,
    }
}

/// Palette color for a per-result trust mode, by severity.
fn trust_color(mode: &RecallResultTrustMode) -> Rgb {
    match mode {
        RecallResultTrustMode::Certify => theme::GREEN,
        RecallResultTrustMode::Advisory => theme::BLUE,
        RecallResultTrustMode::Warn => theme::AMBER,
        RecallResultTrustMode::Block => theme::RED,
    }
}

/// Colored plain-language badge for a store-level authority mode.
pub(crate) fn authority_badge(mode: &AuthorityMode) -> String {
    style::badge(authority_label(mode), authority_color(mode))
}

/// Colored plain-language badge for a per-result trust mode.
pub(crate) fn trust_badge(mode: &RecallResultTrustMode) -> String {
    style::badge(trust_label(mode), trust_color(mode))
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

/// A styled write confirmation for the human (non-JSON) path: a green check, the
/// action plus a short human summary, and the record id beneath. The `--json`
/// path is untouched, so machine output stays clean — this only makes the human
/// read say what happened instead of echoing an opaque id.
pub(crate) fn confirm(action: &str, summary: &str, id: &str) -> String {
    let check = style::badge("\u{2713}", theme::GREEN);
    if summary.is_empty() {
        format!("{check} {action}  {}", style::dim(id))
    } else {
        format!("{check} {action} · {summary}\n  {}", style::dim(id))
    }
}

/// A bold clay section heading for human-readable reports.
pub(crate) fn heading(text: &str) -> String {
    style::heading(text)
}

/// A dimmed (faint) string for secondary detail such as ids and counts.
pub(crate) fn dim(text: &str) -> String {
    style::dim(text)
}
