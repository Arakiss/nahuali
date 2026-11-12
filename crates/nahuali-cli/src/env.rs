//! What the machine knows without being told: where the memory lives, who
//! is at the keyboard, which project this is and which commit it is at.
//! An agent adds only judgement.

use std::hash::{BuildHasher, Hasher};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use nahuali_core::Actor;
use nahuali_core::id::slugify;

/// Where the memory lives: `--home`, then `NAHUALI_HOME`, then the user's
/// data directory.
pub fn home(flag: Option<&str>) -> Result<PathBuf, String> {
    if let Some(dir) = flag.map(str::to_string).or_else(|| var("NAHUALI_HOME")) {
        return Ok(PathBuf::from(dir));
    }
    if let Some(data) = var("XDG_DATA_HOME") {
        return Ok(Path::new(&data).join("nahuali"));
    }
    let home =
        var("HOME").ok_or("there is no HOME to keep the memory in; say where with --home or NAHUALI_HOME")?;
    Ok(Path::new(&home).join(".local").join("share").join("nahuali"))
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Bits from the operating system, by way of the standard library's hasher
/// seeds, mixed with the moment and the process.
pub fn entropy() -> u128 {
    let draw = |salt: u64| {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(salt);
        h.write_u64(now_ms());
        h.write_u32(std::process::id());
        h.finish()
    };
    (u128::from(draw(1)) << 64) | u128::from(draw(2))
}

/// The person this memory is for.
pub fn person() -> String {
    var("NAHUALI_PERSON")
        .or_else(|| var("USER"))
        .or_else(|| var("USERNAME"))
        .unwrap_or_else(|| "person".to_string())
}

/// Tools that run agents say so in the environment.
const HARNESSES: [(&str, &str); 5] = [
    ("CLAUDECODE", "claude-code"),
    ("CODEX_SANDBOX", "codex"),
    ("CODEX_THREAD_ID", "codex"),
    ("CURSOR_AGENT", "cursor"),
    ("CURSOR_TRACE_ID", "cursor"),
];

/// Who is speaking. An agent is whoever a tool says it is, or says itself
/// with `--as`; the person is someone at a terminal that no tool is driving.
/// It is what the speaker declares: nothing here proves it.
pub fn actor(named: Option<&str>, from: Option<&str>) -> Result<Actor, String> {
    let name = |text: &str, what: &str| {
        let slug = slugify(text);
        if slug.is_empty() { Err(format!("`{text}` is not a name for {what}")) } else { Ok(slug) }
    };
    let harness =
        HARNESSES.iter().find(|(marker, _)| var(marker).is_some()).map(|(_, name)| (*name).to_string());
    let me = match named.map(str::to_string).or_else(|| var("NAHUALI_ACTOR")) {
        Some(given) => Some(name(&given, "an agent")?),
        None => harness,
    };
    let me = match me {
        Some(agent) => agent,
        None if std::io::stdin().is_terminal() && from.is_none() => return Ok(Actor::Person(person())),
        None => "unattended".to_string(),
    };
    Ok(match from {
        Some(from) => Actor::Reported { by: me, from: name(from, "whoever reported it")? },
        None => Actor::Agent(me),
    })
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}

/// The project: `--project`, then `NAHUALI_PROJECT`, then the repository
/// this directory belongs to, then the directory itself. A worktree belongs
/// to its repository, whatever its own folder is called.
pub fn project(flag: Option<&str>) -> Result<String, String> {
    let given = flag.map(str::to_string).or_else(|| var("NAHUALI_PROJECT"));
    let name = match given {
        Some(name) => name,
        None => git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .and_then(|dir| repository_name(Path::new(&dir)))
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .and_then(|d| d.file_name().map(|n| n.to_string_lossy().to_string()))
            })
            .ok_or("this directory has no name to call the project by; say it with --project")?,
    };
    let slug = slugify(&name);
    if slug.is_empty() { Err(format!("`{name}` is not a name for a project")) } else { Ok(slug) }
}

/// `/x/atlas/.git` is `atlas`; a bare `/x/atlas.git` is `atlas` too.
fn repository_name(common_dir: &Path) -> Option<String> {
    let last = common_dir.file_name()?.to_string_lossy().to_string();
    if last == ".git" {
        return common_dir.parent()?.file_name().map(|n| n.to_string_lossy().to_string());
    }
    Some(last.strip_suffix(".git").unwrap_or(&last).to_string())
}

/// The commit the work is at, when this directory is a repository and the
/// project spoken of is that repository. A commit of some other project
/// would be a mark that says something false.
pub fn commit(project: &str) -> Option<String> {
    let here = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .and_then(|d| repository_name(Path::new(&d)))?;
    (slugify(&here) == project).then(|| git(&["rev-parse", "--short", "HEAD"])).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repository_gives_the_project_its_name() {
        assert_eq!(repository_name(Path::new("/x/atlas/.git")).as_deref(), Some("atlas"));
        assert_eq!(repository_name(Path::new("/x/atlas.git")).as_deref(), Some("atlas"));
    }
}
