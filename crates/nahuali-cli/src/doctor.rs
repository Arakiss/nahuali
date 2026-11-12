//! `nahuali doctor`: what would otherwise only be found by reading files.
//! One line for each part, and for each that is not right, what to do.
//!
//! It exits 0 when everything is right and 1 when something needs a look.

use std::path::Path;
use std::time::SystemTime;

use serde_json::{Value, json};

use crate::remote::Remote;
use crate::{Failure, env, node, parse, team, words};

struct Line {
    fine: bool,
    part: &'static str,
    says: String,
}

fn fine(part: &'static str, says: impl Into<String>) -> Line {
    Line { fine: true, part, says: says.into() }
}

fn look(part: &'static str, says: impl Into<String>) -> Line {
    Line { fine: false, part, says: says.into() }
}

fn ago(seconds: u64) -> String {
    match seconds {
        s if s < 90 => "moments ago".to_string(),
        s if s < 5400 => format!("{} min ago", s / 60),
        s if s < 172_800 => format!("{} h ago", s / 3600),
        s => format!("{} d ago", s / 86_400),
    }
}

/// When a tool last called one of our hooks, as the hook itself left it.
fn last_called(home: &Path, tool: &str) -> Option<u64> {
    let touched =
        std::fs::metadata(home.join("local").join(format!("called-{tool}"))).ok()?.modified().ok()?;
    SystemTime::now().duration_since(touched).ok().map(|d| d.as_secs())
}

/// The lines of the keeper's own log that say it fell over, among the last.
fn fell_over(home: &Path) -> usize {
    let log = std::fs::read_to_string(home.join("local").join("keeper.log")).unwrap_or_default();
    let recent: Vec<&str> = log.lines().rev().take(200).collect();
    recent
        .iter()
        .filter(|l| {
            l.contains("overflowed its stack") || l.contains("panicked") || l.contains("fatal runtime error")
        })
        .count()
}

fn memory(home: &Path, events: Option<usize>) -> Line {
    let whose = match (team::Server::read(home), node::Node::read(home)) {
        (Some(server), _) => format!("the team's, at {} (you are {})", server.url, server.name),
        (None, Some(n)) if n.listen.is_some() || !n.peers.is_empty() => {
            let answers = n.listen.map_or(String::new(), |a| format!("answers at {a}"));
            let goes =
                if n.peers.is_empty() { String::new() } else { format!("goes to {}", n.peers.join(", ")) };
            [answers, goes].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ")
        }
        _ => "on this machine alone".to_string(),
    };
    match events {
        Some(n) => fine("memory", format!("{} · {n} events · {whose}", home.display())),
        None if !Remote::at(home).exists() => {
            fine("memory", format!("{} · nothing written yet", home.display()))
        }
        None => look("memory", format!("{} · could not be read", home.display())),
    }
}

fn keeper(home: &Path, health: &Result<Value, String>) -> Line {
    let fell = fell_over(home);
    let log = home.join("local").join("keeper.log");
    match health {
        Err(why) => look("keeper", format!("does not answer: {why}. What it said is in {}", log.display())),
        Ok(_) if fell > 0 => look(
            "keeper",
            format!(
                "answering, but it fell over {fell} time(s) lately: the last words are in {}",
                log.display()
            ),
        ),
        Ok(health) => {
            fine("keeper", format!("answering · started {}", ago(health["up"].as_u64().unwrap_or(0))))
        }
    }
}

fn meaning(home: &Path, health: &Result<Value, String>) -> Line {
    let installed = nahuali_embed::spec(nahuali_embed::DEFAULT_MODEL)
        .is_ok_and(|spec| nahuali_embed::installed(&home.join("models"), spec));
    if !installed {
        return fine(
            "meaning",
            "off: words only. `nahuali setup --meaning` installs the model (220 MB, runs here)",
        );
    }
    match health {
        Ok(health) if health["meaning"] == true => match health["lacking"].as_u64().unwrap_or(0) {
            0 => fine(
                "meaning",
                format!("on · every one of {} things has its vector", health["things"].as_u64().unwrap_or(0)),
            ),
            n => fine(
                "meaning",
                format!(
                    "on · {n} things still wait for their vector; they are found by their words meanwhile"
                ),
            ),
        },
        Ok(_) => look(
            "meaning",
            "the model is installed but the keeper does not use it: `nahuali stop`, and the next command starts one that does",
        ),
        Err(_) => look("meaning", "installed, but the keeper does not answer"),
    }
}

/// One tool that runs agents: does it know about nahuali, and does it call.
fn tool(home: &Path, user: &Path, dir: &str, file: &str, name: &'static str) -> Option<Line> {
    let root = user.join(dir);
    if !root.is_dir() {
        return None;
    }
    let skill = std::fs::read_to_string(root.join("skills").join("nahuali").join("SKILL.md")).ok();
    if skill.is_none() {
        return Some(look(name, "does not know about nahuali: `nahuali setup --hooks`"));
    }
    let settings: Value = std::fs::read_to_string(root.join(file))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(json!({}));
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let calls: Vec<String> = settings["hooks"]
        .as_object()
        .into_iter()
        .flat_map(|events| events.values())
        .flat_map(|entries| entries.as_array().into_iter().flatten())
        .flat_map(|entry| entry["hooks"].as_array().into_iter().flatten())
        .filter_map(|hook| hook["command"].as_str())
        .filter(|command| command.contains("nahuali") && command.contains(" hook "))
        .map(str::to_string)
        .collect();
    if calls.is_empty() {
        return Some(fine(
            name,
            "has the page about nahuali · no hooks: an agent has to ask. `nahuali setup --hooks` brings the memory to it",
        ));
    }
    if calls.iter().any(|command| !command.contains(&exe)) {
        return Some(look(
            name,
            "its hooks call another copy of nahuali than this one: `nahuali setup --hooks` points them here",
        ));
    }
    if calls.len() < 3 {
        return Some(look(
            name,
            "has some of the hooks and not all: `nahuali setup --hooks` puts in the rest",
        ));
    }
    Some(match last_called(home, name) {
        Some(seconds) => fine(name, format!("knows nahuali · hooks in place · last called {}", ago(seconds))),
        None if name == "codex" => look(
            name,
            "hooks in place but never called: Codex runs a new hook only after you review it. Open Codex and type /hooks",
        ),
        None => {
            look(name, "hooks in place but never called: start a new session, the open ones do not see them")
        }
    })
}

pub fn doctor(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &["json"])?;
    let [] = words(&args, "doctor takes no words")?;
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    let remote = Remote::at(&home);
    let health = if remote.exists() {
        remote.call("health", json!({})).map_err(|e| e.to_string())
    } else {
        Err("nothing to keep yet".to_string())
    };
    let events = health.as_ref().ok().and_then(|h| h["events"].as_u64()).map(|n| n as usize);

    let mut lines = vec![memory(&home, events)];
    if remote.exists() {
        lines.push(keeper(&home, &health));
        lines.push(meaning(&home, &health));
    }
    if let Ok(user) = std::env::var("HOME") {
        let user = Path::new(&user);
        lines.extend(tool(&home, user, ".claude", "settings.json", "claude-code"));
        lines.extend(tool(&home, user, ".codex", "hooks.json", "codex"));
    }
    let who = env::actor(args.value("as"), None).map_or_else(|_| "?".to_string(), |a| a.line());
    lines.push(fine("you", format!("{} is the person · {who} is speaking", crate::me_at(&home))));

    let wrong = lines.iter().filter(|l| !l.fine).count();
    if args.has("json") {
        let parts: Vec<Value> =
            lines.iter().map(|l| json!({ "part": l.part, "fine": l.fine, "says": l.says })).collect();
        return Ok(format!("{}\n", json!({ "fine": wrong == 0, "parts": parts })));
    }
    let mut out = String::new();
    for line in &lines {
        out.push_str(&format!(
            "  {}  {:<11}  {}\n",
            if line.fine { "ok" } else { "!!" },
            line.part,
            line.says
        ));
    }
    out.push_str(&match wrong {
        0 => "everything is right\n".to_string(),
        1 => "1 thing to look at\n".to_string(),
        n => format!("{n} things to look at\n"),
    });
    if wrong == 0 { Ok(out) } else { Err(Failure::Told(out)) }
}
