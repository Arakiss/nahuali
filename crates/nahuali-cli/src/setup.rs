//! `nahuali setup`: tell the tools that run agents on this machine about
//! Nahuali. It writes the one page an agent reads (the skill) and, with
//! `--hooks`, the two calls that bring the memory to an agent by itself.
//! `--remove` takes out exactly what it put in. `--dry-run` only says.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{Failure, parse, words};

const SKILL: &str = include_str!("../../../skills/nahuali/SKILL.md");

/// What marks a hook as ours in a settings file.
const MARK: &str = " hook ";

fn refused(e: impl std::fmt::Display) -> Failure {
    Failure::Refused(e.to_string())
}

fn ours(entry: &Value) -> bool {
    entry["hooks"].as_array().is_some_and(|hooks| {
        hooks.iter().any(|h| h["command"].as_str().is_some_and(|c| c.contains("nahuali") && c.contains(MARK)))
    })
}

/// A settings file with our two hooks in it (or out of it). Everything
/// else in the file is left as it was, in the order it was.
fn with_hooks(mut settings: Value, exe: &str, remove: bool) -> Value {
    if !settings.is_object() {
        settings = json!({});
    }
    for (event, call, matcher) in
        [("SessionStart", "session-start", None), ("PreToolUse", "pre-tool", Some("Bash"))]
    {
        let mut entries: Vec<Value> = settings["hooks"][event].as_array().cloned().unwrap_or_default();
        entries.retain(|entry| !ours(entry));
        if !remove {
            let mut entry = json!({ "hooks": [{ "type": "command", "command": format!("\"{exe}\" hook {call}"), "timeout": 5 }] });
            if let Some(matcher) = matcher {
                entry["matcher"] = json!(matcher);
            }
            entries.push(entry);
        }
        if entries.is_empty() {
            if let Some(hooks) = settings["hooks"].as_object_mut() {
                hooks.remove(event);
            }
        } else {
            settings["hooks"][event] = json!(entries);
        }
    }
    if settings["hooks"].as_object().is_some_and(serde_json::Map::is_empty)
        && let Some(all) = settings.as_object_mut()
    {
        all.remove("hooks");
    }
    settings
}

fn skill(dir: &Path, remove: bool, dry: bool, said: &mut Vec<String>) -> Result<(), Failure> {
    let file = dir.join("skills").join("nahuali").join("SKILL.md");
    if remove {
        if file.exists() {
            if !dry {
                std::fs::remove_file(&file).map_err(refused)?;
                let _ = std::fs::remove_dir(file.parent().expect("it has one"));
            }
            said.push(format!("removed  {}", file.display()));
        }
        return Ok(());
    }
    if std::fs::read_to_string(&file).is_ok_and(|now| now == SKILL) {
        said.push(format!("current  {}", file.display()));
        return Ok(());
    }
    if !dry {
        std::fs::create_dir_all(file.parent().expect("it has one")).map_err(refused)?;
        std::fs::write(&file, SKILL).map_err(refused)?;
    }
    said.push(format!("wrote    {}", file.display()));
    Ok(())
}

pub fn setup(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &["hooks", "remove", "dry-run"])?;
    let [] = words(&args, "setup takes no words")?;
    let (remove, dry) = (args.has("remove"), args.has("dry-run"));
    let home =
        PathBuf::from(std::env::var("HOME").map_err(|_| Failure::Refused("there is no HOME".to_string()))?);
    let mut said = Vec::new();

    let tools: Vec<PathBuf> =
        [".claude", ".codex"].iter().map(|d| home.join(d)).filter(|d| d.is_dir()).collect();
    if tools.is_empty() {
        return Ok(
            "no tool that runs agents was found in your home directory (looked for .claude and .codex)\n"
                .to_string(),
        );
    }
    for dir in &tools {
        skill(dir, remove, dry, &mut said)?;
    }

    let settings = home.join(".claude").join("settings.json");
    if (args.has("hooks") || remove) && home.join(".claude").is_dir() {
        let before = std::fs::read_to_string(&settings).unwrap_or_else(|_| "{}".to_string());
        let parsed: Value = serde_json::from_str(&before).map_err(|e| {
            Failure::Refused(format!("{} is not JSON and was left alone: {e}", settings.display()))
        })?;
        let exe = std::env::current_exe().map_err(refused)?;
        let after = with_hooks(parsed.clone(), &exe.to_string_lossy(), remove);
        if after == parsed {
            said.push(format!("current  {}", settings.display()));
        } else {
            if !dry {
                let backup = settings.with_extension("json.before-nahuali");
                if settings.exists() && !backup.exists() {
                    std::fs::copy(&settings, &backup).map_err(refused)?;
                }
                let text = serde_json::to_string_pretty(&after).map_err(refused)? + "\n";
                std::fs::write(&settings, text).map_err(refused)?;
            }
            let what = if remove { "took the hooks out of" } else { "put the hooks in" };
            said.push(format!("{what}  {}", settings.display()));
        }
    }
    let head = if dry { "would do (nothing was changed):\n" } else { "" };
    Ok(format!("{head}{}\n", said.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_go_in_once_and_come_out_leaving_the_rest_as_it_was() {
        let theirs = json!({
            "model": "x",
            "hooks": {
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "their-guard" }] }],
                "Stop": [{ "hooks": [{ "type": "command", "command": "their-stop" }] }],
            },
            "zeta": true,
        });
        let once = with_hooks(theirs.clone(), "/bin/nahuali", false);
        assert_eq!(once["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        assert_eq!(
            once["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "their-guard",
            "theirs stays first"
        );
        assert_eq!(once["hooks"]["PreToolUse"][1]["hooks"][0]["command"], "\"/bin/nahuali\" hook pre-tool");
        assert_eq!(
            once["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            "\"/bin/nahuali\" hook session-start"
        );
        assert_eq!(
            with_hooks(once.clone(), "/bin/nahuali", false),
            once,
            "putting them in twice changes nothing"
        );
        assert_eq!(
            with_hooks(once.clone(), "/other/nahuali", false)["hooks"]["PreToolUse"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            with_hooks(once, "/bin/nahuali", true),
            theirs,
            "taking them out leaves exactly what was there"
        );
        let keys: Vec<&String> = theirs.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["model", "hooks", "zeta"], "the order of their file is kept");
    }

    #[test]
    fn an_empty_file_gets_only_the_hooks_and_loses_them_whole() {
        let once = with_hooks(json!({}), "/bin/nahuali", false);
        assert!(once["hooks"]["SessionStart"].is_array());
        assert_eq!(with_hooks(once, "/bin/nahuali", true), json!({}));
    }
}
