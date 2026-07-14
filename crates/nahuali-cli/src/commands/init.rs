//! `nahuali init` — turn "installed" into "my agent uses Nahuali".
//!
//! Installs the bundled Claude Code skill into the user's harness and prints the
//! MCP server config and the cross-harness protocol, so a freshly installed
//! binary wires the transition in one command. It only ever *adds* files (never
//! overwrites without `--force`) and never edits the user's existing config.

use std::path::{Path, PathBuf};

use anyhow::Context;

/// The Claude Code skill, bundled into the binary so `init` needs no network.
const SKILL: &str = include_str!("../../../../skills/nahuali/SKILL.md");

const MCP_SNIPPET: &str = "{\n  \"mcpServers\": {\n    \"nahuali\": { \"command\": \"nahuali-mcp\", \"args\": [\"--database\", \"memory\"] }\n  }\n}";

struct Style {
    bold: &'static str,
    dim: &'static str,
    green: &'static str,
    accent: &'static str,
    reset: &'static str,
}

impl Style {
    fn detect() -> Self {
        use std::io::IsTerminal;
        let on = std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal();
        if on {
            Self {
                bold: "\x1b[1m",
                dim: "\x1b[2m",
                green: "\x1b[32m",
                accent: "\x1b[36m",
                reset: "\x1b[0m",
            }
        } else {
            Self {
                bold: "",
                dim: "",
                green: "",
                accent: "",
                reset: "",
            }
        }
    }
}

/// Wire the current agent harness to use Nahuali.
pub(crate) fn init(dry_run: bool, force: bool) -> anyhow::Result<()> {
    let s = Style::detect();

    println!(
        "{}Setting up Nahuali as your agent's memory protocol.{}",
        s.bold, s.reset
    );
    if dry_run {
        println!("{}(dry run — nothing will be written){}", s.dim, s.reset);
    }
    println!();

    // 1 — the Claude Code skill (the concrete, safe, additive transition).
    println!("{}1 · Claude Code skill{}", s.bold, s.reset);
    match claude_skill_target() {
        Some(target) => install_skill(&target, dry_run, force, &s)?,
        None => println!(
            "    Claude Code not detected ({}~/.claude{}); the skill ships in the repo at {}skills/nahuali/{}.",
            s.dim, s.reset, s.dim, s.reset
        ),
    }
    println!();

    // 2 — the MCP server (printed, never auto-merged into the user's config).
    println!("{}2 · Add Nahuali as an MCP server{}", s.bold, s.reset);
    println!(
        "    Paste into your client's MCP config (e.g. {}.mcp.json{}):",
        s.dim, s.reset
    );
    for line in MCP_SNIPPET.lines() {
        println!("    {line}");
    }
    println!();

    // 3 — the cross-harness protocol.
    println!("{}3 · Make it binding for any harness{}", s.bold, s.reset);
    println!(
        "    Copy the protocol into your {}AGENTS.md{} (or system prompt) so the agent",
        s.accent, s.reset
    );
    println!("    recalls before assuming, records evidence, and reads the trust decision.");
    println!(
        "    Protocol: {}https://github.com/Arakiss/nahuali/blob/main/skills/nahuali/protocol.md{}",
        s.dim, s.reset
    );
    println!();
    println!(
        "{}Done.{} Try it: {}nahuali demo{}",
        s.green, s.reset, s.accent, s.reset
    );

    Ok(())
}

/// The Claude Code skill destination, when a `~/.claude` harness is present.
fn claude_skill_target() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let claude = PathBuf::from(home).join(".claude");
    claude
        .is_dir()
        .then(|| claude.join("skills").join("nahuali").join("SKILL.md"))
}

fn install_skill(target: &Path, dry_run: bool, force: bool, s: &Style) -> anyhow::Result<()> {
    if target.exists() && !force {
        println!(
            "    already installed at {} {}(use --force to overwrite){}",
            target.display(),
            s.dim,
            s.reset
        );
        return Ok(());
    }
    if dry_run {
        println!(
            "    would install the Nahuali skill to {}",
            target.display()
        );
        return Ok(());
    }
    let parent = target
        .parent()
        .context("skill target has no parent directory")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    std::fs::write(target, SKILL)
        .with_context(|| format!("failed to write {}", target.display()))?;
    println!(
        "    {}installed{} the Nahuali skill to {}",
        s.green,
        s.reset,
        target.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::MCP_SNIPPET;

    #[test]
    fn generated_mcp_config_uses_a_valid_database_identifier() {
        let snippet: serde_json::Value =
            serde_json::from_str(MCP_SNIPPET).expect("MCP snippet is valid JSON");
        let database = snippet["mcpServers"]["nahuali"]["args"][1]
            .as_str()
            .expect("database argument is a string");
        assert_eq!(
            nahuali_core::validate_database_name(database).expect("valid database"),
            database
        );
        assert!(MCP_SNIPPET.contains("[\"--database\", \"memory\"]"));
        assert!(!MCP_SNIPPET.contains("./memory"));
    }
}
