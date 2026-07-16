use std::path::Path;

use anyhow::Context;
use nahuali_core::{MemoryEngine, MemoryTrustReport, TrustReportOptions};

use crate::output;

/// Print a composed, non-mutating memory trust report. Exits non-zero when the
/// recorded history fails integrity verification, so it can gate scripts and CI;
/// the broader `trustworthy` verdict (which folds in conservative authority and
/// health signals) is reported, not gated. With `html`, also write a
/// self-contained HTML dossier of the report.
pub(crate) fn trust_report(
    memory: &MemoryEngine,
    options: TrustReportOptions,
    html: Option<&Path>,
    json: bool,
) -> anyhow::Result<()> {
    let report = memory.trust_report_with_options(options);

    if let Some(path) = html {
        std::fs::write(path, render_html(&report))
            .with_context(|| format!("failed to write trust report to {}", path.display()))?;
    }

    if json {
        output::print_json(&report)?;
    } else if let Some(path) = html {
        println!("Wrote trust report to {}", path.display());
    } else {
        print_human(&report);
    }

    if !report.integrity.ledger_verified {
        anyhow::bail!("memory trust report failed ledger integrity verification");
    }
    Ok(())
}

fn print_human(report: &MemoryTrustReport) {
    let knowledge = &report.knowledge;
    let integrity = &report.integrity;

    println!("Memory trust report");
    println!("Trustworthy: {}", yes_no(report.trustworthy));
    println!(
        "Knowledge: {} events ({} episodes, {} claims, {} links, {} procedures, {} intentions, {} sources, {} entities)",
        knowledge.event_count,
        knowledge.episode_count,
        knowledge.claim_count,
        knowledge.link_count,
        knowledge.procedure_count,
        knowledge.intention_count,
        knowledge.source_count,
        knowledge.entity_count
    );
    println!(
        "Authority: {:?} (score {:.2}, can_trust {})",
        report.authority.mode,
        report.authority.score,
        yes_no(report.authority.can_trust)
    );

    print!(
        "Integrity: {} (checksums {}, sequence {}",
        if integrity.ledger_verified {
            "verified"
        } else {
            "FAILED"
        },
        if integrity.checksums_valid {
            "ok"
        } else {
            "bad"
        },
        if integrity.sequence_contiguous {
            "contiguous"
        } else {
            "broken"
        }
    );
    #[cfg(feature = "tamper-evidence")]
    print!(
        ", chain {}",
        if integrity.chain_intact {
            "intact"
        } else {
            "broken"
        }
    );
    println!(")");
    #[cfg(feature = "tamper-evidence")]
    if let Some(tip) = &integrity.chain_tip {
        println!("Chain tip: {tip}");
    }
    #[cfg(feature = "tamper-evidence")]
    if let Some(root) = &integrity.merkle_root {
        println!("Merkle root: {root}");
    }

    println!(
        "Health: {} unsupported, {} conflicting, {} blind spots (avg confidence {:.2})",
        report.health.unsupported_fact_count,
        report.health.conflicting_fact_count,
        report.health.blind_spot_count,
        report.health.average_fact_confidence
    );

    #[cfg(feature = "attestation")]
    if let Some(verdict) = &report.attestation {
        println!(
            "Attestation: checkpoint at sequence {} {}",
            verdict.sequence,
            if verdict.anchored {
                "anchored"
            } else {
                "NOT anchored"
            }
        );
    }

    println!("Reasons:");
    for reason in &report.verdict_reasons {
        println!("- {reason}");
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

const HTML_STYLE: &str = r#":root{--bg:#16130f;--panel:#1f1a14;--line:#2e2820;--ink:#ece4d6;--ink-dim:#746b5b;--mut:#9b9078;--accent:#d9885a;--ok:#86a86b;--bad:#cf6b48}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--ink);font:15px/1.55 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;padding:48px 20px}
.wrap{max-width:820px;margin:0 auto}
.kicker,.q,.l{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;letter-spacing:.16em;text-transform:uppercase}
.kicker{font-size:11px;font-weight:600;color:var(--mut)}
h1{font-size:30px;margin:.3em 0 .1em;letter-spacing:-.01em}
.verdict{display:inline-block;margin-top:14px;padding:7px 14px;border-radius:999px;font:600 13px/1 ui-monospace,monospace;letter-spacing:.06em}
.verdict.ok{background:rgba(134,168,107,.14);color:var(--ok);border:1px solid rgba(134,168,107,.35)}
.verdict.bad{background:rgba(207,107,72,.14);color:var(--bad);border:1px solid rgba(207,107,72,.35)}
section{margin-top:34px}
.q{font-size:11px;font-weight:600;color:var(--accent);margin-bottom:12px}
.grid{display:grid;grid-template-columns:repeat(auto-fit,minmax(120px,1fr));gap:10px}
.card{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:14px 16px}
.card .n{font-size:22px;font-weight:600}
.card .l{font-size:10px;color:var(--mut);margin-top:4px;letter-spacing:.12em}
.rows{background:var(--panel);border:1px solid var(--line);border-radius:10px;padding:2px 16px}
.row{display:flex;justify-content:space-between;gap:16px;padding:9px 0;border-bottom:1px solid var(--line);font-size:14px}
.row:last-child{border-bottom:none}
.row .k{color:var(--mut)}
.row .v{font-family:ui-monospace,monospace}
.v.ok{color:var(--ok)}
.v.bad{color:var(--bad)}
.tip{font-family:ui-monospace,monospace;font-size:12px;color:var(--mut);word-break:break-all;padding:9px 0 4px}
ul.reasons{list-style:none;padding:0;margin:0;background:var(--panel);border:1px solid var(--line);border-radius:10px}
ul.reasons li{padding:9px 16px 9px 30px;position:relative;border-bottom:1px solid var(--line);font-size:14px}
ul.reasons li:before{content:"";position:absolute;left:16px;top:16px;width:6px;height:6px;border-radius:50%;background:var(--accent)}
ul.reasons li:last-child{border-bottom:none}
footer{margin-top:40px;color:var(--ink-dim);font:11px/1.6 ui-monospace,monospace;letter-spacing:.04em}"#;

/// Render the trust report as a self-contained HTML dossier: a single file with
/// inline styles, system fonts, and no network calls, so it renders offline.
fn render_html(report: &MemoryTrustReport) -> String {
    let knowledge = &report.knowledge;
    let integrity = &report.integrity;
    let health = &report.health;

    let knowledge_cards = [
        ("events", knowledge.event_count),
        ("episodes", knowledge.episode_count),
        ("claims", knowledge.claim_count),
        ("links", knowledge.link_count),
        ("procedures", knowledge.procedure_count),
        ("intentions", knowledge.intention_count),
        ("sources", knowledge.source_count),
        ("entities", knowledge.entity_count),
    ]
    .iter()
    .map(|(label, count)| card(&count.to_string(), label))
    .collect::<String>();

    let health_cards = [
        (
            "unsupported".to_string(),
            health.unsupported_fact_count.to_string(),
        ),
        (
            "conflicting".to_string(),
            health.conflicting_fact_count.to_string(),
        ),
        (
            "blind spots".to_string(),
            health.blind_spot_count.to_string(),
        ),
        (
            "avg confidence".to_string(),
            format!("{:.2}", health.average_fact_confidence),
        ),
    ]
    .iter()
    .map(|(label, value)| card(value, label))
    .collect::<String>();

    let mut trust_rows = String::new();
    trust_rows.push_str(&row(
        "Authority",
        &format!(
            "{:?} · {:.2}",
            report.authority.mode, report.authority.score
        ),
        None,
    ));
    trust_rows.push_str(&row(
        "Can trust",
        yes_no(report.authority.can_trust),
        Some(report.authority.can_trust),
    ));
    trust_rows.push_str(&row(
        "Ledger integrity",
        if integrity.ledger_verified {
            "verified"
        } else {
            "FAILED"
        },
        Some(integrity.ledger_verified),
    ));
    trust_rows.push_str(&row(
        "Checksums",
        if integrity.checksums_valid {
            "ok"
        } else {
            "bad"
        },
        Some(integrity.checksums_valid),
    ));
    trust_rows.push_str(&row(
        "Sequence",
        if integrity.sequence_contiguous {
            "contiguous"
        } else {
            "broken"
        },
        Some(integrity.sequence_contiguous),
    ));
    #[cfg(feature = "tamper-evidence")]
    trust_rows.push_str(&row(
        "Hash chain",
        if integrity.chain_intact {
            "intact"
        } else {
            "broken"
        },
        Some(integrity.chain_intact),
    ));

    let mut history_rows = String::new();
    history_rows.push_str(&row(
        "History verified",
        yes_no(integrity.ledger_verified),
        Some(integrity.ledger_verified),
    ));
    #[cfg(feature = "attestation")]
    if let Some(verdict) = &report.attestation {
        history_rows.push_str(&row(
            "Signed checkpoint",
            if verdict.anchored {
                "anchored"
            } else {
                "NOT anchored"
            },
            Some(verdict.anchored),
        ));
    }

    #[cfg(feature = "tamper-evidence")]
    let tip_html = integrity
        .chain_tip
        .as_ref()
        .map(|tip| format!("<div class=\"tip\">chain tip · {}</div>", escape(tip)))
        .unwrap_or_default();
    #[cfg(not(feature = "tamper-evidence"))]
    let tip_html = String::new();

    #[cfg(feature = "tamper-evidence")]
    let root_html = integrity
        .merkle_root
        .as_ref()
        .map(|root| format!("<div class=\"tip\">merkle root · {}</div>", escape(root)))
        .unwrap_or_default();
    #[cfg(not(feature = "tamper-evidence"))]
    let root_html = String::new();

    let reasons = report
        .verdict_reasons
        .iter()
        .map(|reason| format!("<li>{}</li>", escape(reason)))
        .collect::<String>();

    let verdict_class = if report.trustworthy { "ok" } else { "bad" };
    let verdict_text = if report.trustworthy { "YES" } else { "NO" };

    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Memory Trust Report</title><style>{style}</style></head><body><div class="wrap"><div class="kicker">Nahuali · memory trust report</div><h1>Memory Trust Report</h1><div class="verdict {verdict_class}">Trustworthy: {verdict_text}</div><section><div class="q">What do we know</div><div class="grid">{knowledge_cards}</div></section><section><div class="q">Why should we trust it</div><div class="rows">{trust_rows}</div></section><section><div class="q">What is missing or contradictory</div><div class="grid">{health_cards}</div></section><section><div class="q">Was the recorded history altered</div><div class="rows">{history_rows}</div>{tip_html}{root_html}</section><section><div class="q">Reasons</div><ul class="reasons">{reasons}</ul></section><footer>Non-mutating snapshot · report v{version} · generated at {generated} ms since the Unix epoch</footer></div></body></html>"#,
        style = HTML_STYLE,
        verdict_class = verdict_class,
        verdict_text = verdict_text,
        knowledge_cards = knowledge_cards,
        trust_rows = trust_rows,
        health_cards = health_cards,
        history_rows = history_rows,
        tip_html = tip_html,
        root_html = root_html,
        reasons = reasons,
        version = report.version,
        generated = report.generated_at_ms,
    )
}

fn card(value: &str, label: &str) -> String {
    format!(
        "<div class=\"card\"><div class=\"n\">{}</div><div class=\"l\">{label}</div></div>",
        escape(value)
    )
}

fn row(key: &str, value: &str, ok: Option<bool>) -> String {
    let class = match ok {
        Some(true) => " ok",
        Some(false) => " bad",
        None => "",
    };
    format!(
        "<div class=\"row\"><span class=\"k\">{key}</span><span class=\"v{class}\">{}</span></div>",
        escape(value)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Read and parse a signed attestation receipt to fold into the report.
#[cfg(feature = "attestation")]
pub(crate) fn read_attestation(
    path: &std::path::Path,
) -> anyhow::Result<nahuali_core::LedgerAttestation> {
    use anyhow::Context;

    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read attestation {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse attestation {}", path.display()))
}
