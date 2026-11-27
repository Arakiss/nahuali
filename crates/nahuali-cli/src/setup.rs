//! `nahuali setup`: tell the tools that run agents on this machine about
//! Nahuali: Claude Code, Codex, Antigravity (`agy`) and Cursor. It writes
//! the one page an agent reads (the skill) and, with `--hooks`, the three
//! calls that bring the memory to an agent by itself. `--remove` takes out
//! exactly what it put in. `--dry-run` only says.

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
fn with_hooks(mut settings: Value, exe: &str, tool: &str, remove: bool) -> Value {
    if !settings.is_object() {
        settings = json!({});
    }
    for (event, call, matcher) in [
        ("SessionStart", "session-start", None),
        ("PreToolUse", "pre-tool", Some("Bash")),
        ("UserPromptSubmit", "prompt", None),
    ] {
        let mut entries: Vec<Value> = settings["hooks"][event].as_array().cloned().unwrap_or_default();
        entries.retain(|entry| !ours(entry));
        if !remove {
            let mut entry = json!({ "hooks": [{ "type": "command", "command": format!("\"{exe}\" hook {call} {tool}"), "timeout": 5 }] });
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

/// What Antigravity names a set of hooks by: ours, whole.
const ANTIGRAVITY_HOOK: &str = "nahuali";

/// Antigravity's `hooks.json`, with our named hook in it (or out of it).
/// Its calls before each call to the model carry `prompt`: the hook tells
/// the first of a turn from the rest.
fn with_antigravity_hooks(mut hooks: Value, exe: &str, remove: bool) -> Value {
    if !hooks.is_object() {
        hooks = json!({});
    }
    let all = hooks.as_object_mut().expect("it is one");
    all.remove(ANTIGRAVITY_HOOK);
    if !remove {
        let call = |what: &str| json!({ "type": "command", "command": format!("\"{exe}\" hook {what} antigravity"), "timeout": 5 });
        all.insert(
            ANTIGRAVITY_HOOK.to_string(),
            json!({
                "SessionStart": [call("session-start")],
                "PreToolUse": [{ "matcher": "run_command", "hooks": [call("pre-tool")] }],
                "PreInvocation": [call("prompt")],
            }),
        );
    }
    hooks
}

/// Cursor's `hooks.json`, with our three in it (or out of it). Cursor has
/// no words to add before a command it lets through: what a lesson tells is
/// said after it, at `postToolUse`.
fn with_cursor_hooks(mut hooks: Value, exe: &str, remove: bool) -> Value {
    if !hooks.is_object() {
        hooks = json!({});
    }
    let ours =
        |entry: &Value| entry["command"].as_str().is_some_and(|c| c.contains("nahuali") && c.contains(MARK));
    for (event, call, matcher) in [
        ("sessionStart", "session-start", None),
        ("preToolUse", "pre-tool", Some("Shell")),
        ("postToolUse", "post-tool", Some("Shell")),
    ] {
        let mut entries: Vec<Value> = hooks["hooks"][event].as_array().cloned().unwrap_or_default();
        entries.retain(|entry| !ours(entry));
        if !remove {
            let mut entry = json!({ "command": format!("\"{exe}\" hook {call} cursor"), "timeout": 5 });
            if let Some(matcher) = matcher {
                entry["matcher"] = json!(matcher);
            }
            entries.push(entry);
        }
        if entries.is_empty() {
            if let Some(all) = hooks["hooks"].as_object_mut() {
                all.remove(event);
            }
        } else {
            hooks["hooks"][event] = json!(entries);
        }
    }
    if hooks["hooks"].as_object().is_some_and(serde_json::Map::is_empty)
        && let Some(all) = hooks.as_object_mut()
    {
        all.remove("hooks");
    }
    let only_version = hooks.as_object().is_some_and(|all| all.keys().all(|k| k == "version"));
    if only_version {
        // Nothing of anyone's is left: the version was ours too.
        hooks = json!({});
    } else if hooks.get("version").is_none() {
        hooks["version"] = json!(1);
    }
    hooks
}

/// A tool that runs agents: where it lives, where its page goes, where its
/// hooks are and in what shape.
pub struct Target {
    name: &'static str,
    /// The folder that says the tool is on this machine.
    lives: &'static str,
    /// Where it looks for skills, or none when it reads another tool's.
    skills: Option<&'static str>,
    hooks: &'static str,
    shape: fn(Value, &str, &str, bool) -> Value,
}

const TARGETS: [Target; 4] = [
    Target {
        name: "claude-code",
        lives: ".claude",
        skills: Some(".claude/skills"),
        hooks: ".claude/settings.json",
        shape: |v, exe, tool, remove| with_hooks(v, exe, tool, remove),
    },
    Target {
        name: "codex",
        lives: ".codex",
        skills: Some(".codex/skills"),
        hooks: ".codex/hooks.json",
        shape: |v, exe, tool, remove| with_hooks(v, exe, tool, remove),
    },
    Target {
        name: "antigravity",
        lives: ".gemini/antigravity-cli",
        skills: Some(".gemini/config/skills"),
        hooks: ".gemini/config/hooks.json",
        shape: |v, exe, _, remove| with_antigravity_hooks(v, exe, remove),
    },
    // Cursor reads the skills of Claude Code and Codex as its own.
    Target {
        name: "cursor",
        lives: ".cursor",
        skills: None,
        hooks: ".cursor/hooks.json",
        shape: |v, exe, _, remove| with_cursor_hooks(v, exe, remove),
    },
];

/// The tools of [`TARGETS`] found in this home.
pub fn targets(home: &Path) -> impl Iterator<Item = &'static Target> + '_ {
    TARGETS.iter().filter(|t| home.join(t.lives).is_dir())
}

/// Where a tool's page about nahuali is, if it has its own.
pub fn page(home: &Path, target: &Target) -> Option<PathBuf> {
    target.skills.map(|dir| home.join(dir).join("nahuali").join("SKILL.md"))
}

/// Where a tool's hooks are.
pub fn hooks_file(home: &Path, target: &Target) -> PathBuf {
    home.join(target.hooks)
}

pub fn name(target: &Target) -> &'static str {
    target.name
}

fn skill(file: &Path, remove: bool, dry: bool, said: &mut Vec<String>) -> Result<(), Failure> {
    let file = file.to_path_buf();
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
    let args = parse(argv, &[], &["hooks", "remove", "dry-run", "meaning"])?;
    let [] = words(&args, "setup takes no words")?;
    let (remove, dry) = (args.has("remove"), args.has("dry-run"));
    let home =
        PathBuf::from(std::env::var("HOME").map_err(|_| Failure::Refused("there is no HOME".to_string()))?);
    let mut said = Vec::new();

    if args.has("meaning") && !remove {
        let memory = crate::env::home(args.value("home")).map_err(Failure::Usage)?;
        let spec = nahuali_embed::spec(nahuali_embed::DEFAULT_MODEL).map_err(refused)?;
        if nahuali_embed::installed(&memory.join("models"), spec) {
            said.push(format!("current  the model {}", spec.id));
        } else if dry {
            said.push(format!(
                "download the model {} (about 220 MB) into {}",
                spec.id,
                memory.join("models").display()
            ));
        } else {
            nahuali_embed::install(&memory.join("models"), spec).map_err(refused)?;
            // The keeper loads the model when it starts.
            crate::remote::Remote::at(&memory).stop();
            said.push(format!("installed the model {} in {}", spec.id, memory.join("models").display()));
        }
        if !args.has("hooks") {
            let head = if dry { "would do (nothing was changed):\n" } else { "" };
            return Ok(format!("{head}{}\n", said.join("\n")));
        }
    }

    let tools: Vec<&Target> = targets(&home).collect();
    if tools.is_empty() {
        return Ok("no tool that runs agents was found in your home directory \
             (looked for .claude, .codex, .gemini/antigravity-cli and .cursor)\n"
            .to_string());
    }
    for target in &tools {
        if let Some(file) = page(&home, target) {
            skill(&file, remove, dry, &mut said)?;
        }
    }

    let exe = std::env::current_exe().map_err(refused)?;
    for target in &tools {
        let tool = target.name;
        let settings = hooks_file(&home, target);
        if !(args.has("hooks") || remove) || (remove && !settings.exists()) {
            continue;
        }
        let before = std::fs::read_to_string(&settings).unwrap_or_else(|_| "{}".to_string());
        let parsed: Value = serde_json::from_str(&before).map_err(|e| {
            Failure::Refused(format!("{} is not JSON and was left alone: {e}", settings.display()))
        })?;
        let after = (target.shape)(parsed.clone(), &exe.to_string_lossy(), tool, remove);
        if after == parsed {
            said.push(format!("current  {}", settings.display()));
            continue;
        }
        if !dry {
            let backup = settings.with_extension("json.before-nahuali");
            if settings.exists() && !backup.exists() {
                std::fs::copy(&settings, &backup).map_err(refused)?;
            }
            if let Some(dir) = settings.parent() {
                std::fs::create_dir_all(dir).map_err(refused)?;
            }
            if after == json!({}) {
                // Nothing of anyone's is left in it: it was ours alone.
                std::fs::remove_file(&settings).map_err(refused)?;
            } else {
                let text = serde_json::to_string_pretty(&after).map_err(refused)? + "\n";
                std::fs::write(&settings, text).map_err(refused)?;
            }
        }
        let what = if remove { "took the hooks out of" } else { "put the hooks in" };
        said.push(format!("{what}  {}", settings.display()));
        if tool == "codex" && !remove {
            said.push(
                "         Codex runs a new hook only after you review it: open Codex and type /hooks, once"
                    .to_string(),
            );
        }
        if tool == "cursor"
            && !remove
            && !tools.iter().any(|t| t.skills.is_some() && t.lives != ".gemini/antigravity-cli")
        {
            said.push(
                "         Cursor reads its page from Claude Code's or Codex's skills, and neither is here"
                    .to_string(),
            );
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
        let once = with_hooks(theirs.clone(), "/bin/nahuali", "codex", false);
        assert_eq!(once["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
        assert_eq!(
            once["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "their-guard",
            "theirs stays first"
        );
        assert_eq!(
            once["hooks"]["PreToolUse"][1]["hooks"][0]["command"],
            "\"/bin/nahuali\" hook pre-tool codex"
        );
        assert_eq!(
            once["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            "\"/bin/nahuali\" hook session-start codex"
        );
        assert_eq!(
            with_hooks(once.clone(), "/bin/nahuali", "codex", false),
            once,
            "putting them in twice changes nothing"
        );
        assert_eq!(
            with_hooks(once.clone(), "/other/nahuali", "codex", false)["hooks"]["PreToolUse"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            with_hooks(once, "/bin/nahuali", "codex", true),
            theirs,
            "taking them out leaves exactly what was there"
        );
        let keys: Vec<&String> = theirs.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["model", "hooks", "zeta"], "the order of their file is kept");
    }

    #[test]
    fn an_empty_file_gets_only_the_hooks_and_loses_them_whole() {
        let once = with_hooks(json!({}), "/bin/nahuali", "codex", false);
        assert!(once["hooks"]["SessionStart"].is_array());
        assert_eq!(with_hooks(once, "/bin/nahuali", "codex", true), json!({}));
    }

    #[test]
    fn antigravity_gets_one_named_hook_and_loses_it_whole() {
        let theirs = json!({ "orca-status": { "enabled": false, "Stop": [{ "command": "their-stop" }] } });
        let once = with_antigravity_hooks(theirs.clone(), "/bin/nahuali", false);
        assert_eq!(once["orca-status"], theirs["orca-status"], "theirs is left as it was");
        assert_eq!(
            once["nahuali"]["SessionStart"][0]["command"],
            "\"/bin/nahuali\" hook session-start antigravity"
        );
        assert_eq!(once["nahuali"]["PreToolUse"][0]["matcher"], "run_command");
        assert_eq!(
            once["nahuali"]["PreInvocation"][0]["command"],
            "\"/bin/nahuali\" hook prompt antigravity"
        );
        assert_eq!(
            with_antigravity_hooks(once.clone(), "/bin/nahuali", false),
            once,
            "twice changes nothing"
        );
        assert_eq!(with_antigravity_hooks(once, "/bin/nahuali", true), theirs);
        assert_eq!(with_antigravity_hooks(json!({}), "/bin/nahuali", true), json!({}));
    }

    #[test]
    fn cursor_gets_three_hooks_and_a_version_and_loses_only_ours() {
        let once = with_cursor_hooks(json!({}), "/bin/nahuali", false);
        assert_eq!(once["version"], 1);
        assert_eq!(once["hooks"]["sessionStart"][0]["command"], "\"/bin/nahuali\" hook session-start cursor");
        assert_eq!(once["hooks"]["preToolUse"][0]["matcher"], "Shell");
        assert_eq!(once["hooks"]["postToolUse"][0]["command"], "\"/bin/nahuali\" hook post-tool cursor");
        assert_eq!(with_cursor_hooks(once.clone(), "/bin/nahuali", false), once, "twice changes nothing");
        assert_eq!(
            with_cursor_hooks(once, "/bin/nahuali", true),
            json!({}),
            "a file that was ours alone goes"
        );

        let theirs = json!({ "version": 1, "hooks": { "stop": [{ "command": "their-stop" }] } });
        let mixed = with_cursor_hooks(theirs.clone(), "/bin/nahuali", false);
        assert_eq!(mixed["hooks"]["stop"], theirs["hooks"]["stop"]);
        assert_eq!(with_cursor_hooks(mixed, "/bin/nahuali", true), theirs, "theirs stays, version and all");
    }
}
