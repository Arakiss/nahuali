//! Human-facing rendering of Nahuali's trust/authority layer.
//!
//! Nahuali's value is its governed/auditable trust layer, so the human
//! (non-`--json`) output renders authority modes as a plain-language label plus
//! a short gloss, colored by severity in the shared clay-on-coffee palette.
//! Generic styling lives in `nahuali-ui`; this module only maps Nahuali's core
//! trust types onto it. Color is emitted only when stdout is a TTY and
//! `NO_COLOR` is unset; `--json` output is never touched.

use nahuali_core::{
    AuthorityMode, HealthSeverity, HealthSignal, HealthSignalKind, RecallResultTrustMode,
};
use nahuali_ui::style;
use nahuali_ui::theme::{self, Rgb};

/// Plain-language label plus a short gloss for an authority mode, e.g.
/// `CERTIFY · evidence checks passed`. Keeps the stable mode token plus a
/// bounded description of what the engine actually established.
pub(crate) fn authority_label(mode: &AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::Certify => "CERTIFY · evidence checks passed",
        AuthorityMode::Advisory => "ADVISORY · use with context",
        AuthorityMode::Warn => "WARN · verify before use",
        AuthorityMode::Block => "BLOCK · do not use yet",
    }
}

/// Plain-language label plus a short gloss for a per-result trust mode.
fn trust_label(mode: &RecallResultTrustMode) -> &'static str {
    match mode {
        RecallResultTrustMode::Certify => "CERTIFY · evidence checks passed",
        RecallResultTrustMode::Advisory => "ADVISORY · use with context",
        RecallResultTrustMode::Warn => "WARN · verify before use",
        RecallResultTrustMode::Block => "BLOCK · do not use yet",
    }
}

/// Palette color for an authority mode, by severity.
pub(crate) fn authority_color(mode: &AuthorityMode) -> Rgb {
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
/// `Store trust: CERTIFY · evidence checks passed (score 1.00)`.
pub(crate) fn store_trust_line(mode: &AuthorityMode, score: f32) -> String {
    format!(
        "Store trust: {} (score {:.2})",
        authority_badge(mode),
        score
    )
}

/// Render the dominant reason behind a non-certified store-trust line, e.g.
/// `Why: unsupported_fact ×412 (medium, +3 other) · next: nahuali review --action capture-evidence`.
///
/// The dominant reason is the most frequent signal kind at the severity that
/// set the authority mode (ties go to the kind reported first), so the line
/// always names something that, once addressed, can lift the mode. Returns
/// `None` for a certified store.
pub(crate) fn store_trust_cause_line(
    mode: &AuthorityMode,
    signals: &[HealthSignal],
) -> Option<String> {
    let severity = match mode {
        AuthorityMode::Block => HealthSeverity::High,
        AuthorityMode::Warn => HealthSeverity::Medium,
        AuthorityMode::Advisory => HealthSeverity::Low,
        AuthorityMode::Certify => return None,
    };
    let mut counts: Vec<(&HealthSignalKind, usize)> = Vec::new();
    for signal in signals.iter().filter(|s| s.severity == severity) {
        match counts.iter_mut().find(|(kind, _)| *kind == &signal.kind) {
            Some((_, count)) => *count += 1,
            None => counts.push((&signal.kind, 1)),
        }
    }
    let total: usize = counts.iter().map(|(_, count)| count).sum();
    let (kind, count) = counts
        .iter()
        .rev()
        .max_by_key(|(_, count)| *count)
        .copied()?;
    let severity = match severity {
        HealthSeverity::High => "high",
        HealthSeverity::Medium => "medium",
        HealthSeverity::Low => "low",
    };
    let others = total - count;
    let scope = if others == 0 {
        severity.to_string()
    } else {
        format!("{severity}, +{others} other")
    };
    Some(format!(
        "Why: {} ×{count} ({scope}) · next: {}",
        health_signal_kind_name(kind),
        health_signal_next_command(kind)
    ))
}

/// Stable snake_case token for a health signal kind, matching the JSON output.
fn health_signal_kind_name(kind: &HealthSignalKind) -> &'static str {
    match kind {
        HealthSignalKind::NoEpisodes => "no_episodes",
        HealthSignalKind::UnsupportedFact => "unsupported_fact",
        HealthSignalKind::LowConfidenceFact => "low_confidence_fact",
        HealthSignalKind::ConflictingFact => "conflicting_fact",
        HealthSignalKind::StaleFact => "stale_fact",
        HealthSignalKind::StaleEpisode => "stale_episode",
        HealthSignalKind::SupersededFact => "superseded_fact",
        HealthSignalKind::IsolatedEntity => "isolated_entity",
    }
}

/// The command that addresses a health signal kind: the operator review queue
/// filtered to the action self-inspection assigns that kind, or `remember` when
/// the store has no episodes to review yet.
fn health_signal_next_command(kind: &HealthSignalKind) -> &'static str {
    match kind {
        HealthSignalKind::NoEpisodes => "nahuali remember \"<observation>\"",
        HealthSignalKind::UnsupportedFact | HealthSignalKind::LowConfidenceFact => {
            "nahuali review --action capture-evidence"
        }
        HealthSignalKind::ConflictingFact => "nahuali review --action resolve-contradiction",
        HealthSignalKind::StaleFact
        | HealthSignalKind::StaleEpisode
        | HealthSignalKind::SupersededFact => "nahuali review --action refresh-memory",
        HealthSignalKind::IsolatedEntity => "nahuali review --action link-memory",
    }
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

#[cfg(test)]
mod tests {
    use nahuali_core::{AuthorityMode, HealthSeverity, HealthSignal, HealthSignalKind};

    use super::store_trust_cause_line;

    fn signal(kind: HealthSignalKind, severity: HealthSeverity) -> HealthSignal {
        HealthSignal {
            kind,
            dimensions: Vec::new(),
            severity,
            message: String::new(),
            evidence_ids: Vec::new(),
        }
    }

    #[test]
    fn trust_cause_names_the_most_frequent_kind_at_the_deciding_severity() {
        let signals = vec![
            signal(HealthSignalKind::StaleFact, HealthSeverity::Medium),
            signal(HealthSignalKind::UnsupportedFact, HealthSeverity::Medium),
            signal(HealthSignalKind::UnsupportedFact, HealthSeverity::Medium),
            // More frequent, but low severity did not set the WARN mode.
            signal(HealthSignalKind::IsolatedEntity, HealthSeverity::Low),
            signal(HealthSignalKind::IsolatedEntity, HealthSeverity::Low),
            signal(HealthSignalKind::IsolatedEntity, HealthSeverity::Low),
        ];
        assert_eq!(
            store_trust_cause_line(&AuthorityMode::Warn, &signals).as_deref(),
            Some(
                "Why: unsupported_fact ×2 (medium, +1 other) · next: nahuali review --action capture-evidence"
            )
        );
    }

    #[test]
    fn trust_cause_breaks_ties_by_first_reported_kind() {
        let signals = vec![
            signal(HealthSignalKind::ConflictingFact, HealthSeverity::High),
            signal(HealthSignalKind::NoEpisodes, HealthSeverity::High),
        ];
        assert_eq!(
            store_trust_cause_line(&AuthorityMode::Block, &signals).as_deref(),
            Some(
                "Why: conflicting_fact ×1 (high, +1 other) · next: nahuali review --action resolve-contradiction"
            )
        );
    }

    #[test]
    fn trust_cause_is_absent_for_a_certified_store() {
        assert_eq!(store_trust_cause_line(&AuthorityMode::Certify, &[]), None);
    }
}
