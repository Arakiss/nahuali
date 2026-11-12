//! The command as an agent meets it: run for real, on a memory of its own.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A memory and a working directory of their own, gone when the test ends.
struct Desk {
    root: PathBuf,
}

impl Desk {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("nahuali-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("work")).unwrap();
        Self { root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("memory")
    }

    fn command(&self, dir: &Path) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_nahuali"));
        // A keeper nobody stops leaves by itself soon after the test.
        c.current_dir(dir).env("NAHUALI_HOME", self.home()).env("NAHUALI_IDLE_SECS", "20");
        for marker in ["CLAUDECODE", "CODEX_SANDBOX", "CODEX_THREAD_ID", "CURSOR_AGENT", "CURSOR_TRACE_ID"] {
            c.env_remove(marker);
        }
        c.env_remove("NAHUALI_ACTOR").env_remove("NAHUALI_PROJECT").env_remove("NAHUALI_PERSON");
        c.stdin(std::process::Stdio::null());
        c
    }

    /// Run as the agent `builder` on project `atlas`, outside any repository.
    fn run(&self, args: &[&str]) -> Said {
        let out = self
            .command(&self.root.join("work"))
            .env("NAHUALI_ACTOR", "builder")
            .env("NAHUALI_PROJECT", "atlas")
            .args(args)
            .output()
            .unwrap();
        Said::of(out)
    }
}

impl Drop for Desk {
    fn drop(&mut self) {
        let _ = self.command(&self.root).arg("stop").output();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct Said {
    out: String,
    err: String,
    code: i32,
}

impl Said {
    fn of(o: Output) -> Self {
        Self {
            out: String::from_utf8_lossy(&o.stdout).to_string(),
            err: String::from_utf8_lossy(&o.stderr).to_string(),
            code: o.status.code().unwrap_or(-1),
        }
    }

    fn ok(self) -> String {
        assert_eq!(self.code, 0, "stdout: {}\nstderr: {}", self.out, self.err);
        self.out
    }
}

/// The name of the first open ask, as `status --json` gives it.
fn first_ask(desk: &Desk) -> String {
    let status: serde_json::Value = serde_json::from_str(&desk.run(&["status", "--json"]).ok()).unwrap();
    status["asks"][0]["ask"].as_str().unwrap().to_string()
}

#[test]
fn reading_an_empty_memory_says_so_and_writes_nothing() {
    let desk = Desk::new("empty");
    assert_eq!(desk.run(&["status"]).ok(), "atlas · nothing held yet\n");
    assert!(!desk.home().exists(), "a read leaves no trace");
}

#[test]
fn a_question_holds_its_work_until_it_is_answered_and_the_answer_is_found() {
    let desk = Desk::new("flow");
    let said = desk
        .run(&[
            "update",
            "importer",
            "--line",
            "tests green",
            "--done",
            "parser",
            "--next",
            "review",
            "--verified",
            "cargo test",
        ])
        .ok();
    assert_eq!(said, "atlas/importer · moving · tests green\n");
    let said =
        desk.run(&["ask", "Ship behind a flag?", "--work", "importer", "--recommends", "behind a flag"]).ok();
    assert!(said.starts_with("ask:"), "{said}");
    let ask = first_ask(&desk);

    let status = desk.run(&["status"]).ok();
    assert!(status.starts_with("atlas · 1 waiting on the person · 1 piece of work\n"), "{status}");
    assert!(status.contains(&format!("importer  waiting on ask:{ask} · tests green")), "{status}");
    assert!(status.contains("holds importer · recommends behind a flag"), "{status}");
    assert!(status.contains("asked by builder, an agent · just now"), "{status}");
    assert!(status.contains("builder · verified by cargo test · just now"), "{status}");

    let said = desk.run(&["answer", &ask[..4], "Behind a flag.", "--as", "reviewer"]).ok();
    assert!(said.contains("answered, and kept as a decision in atlas"), "{said}");
    let status = desk.run(&["status"]).ok();
    assert!(status.contains("importer  moving · tests green"), "{status}");
    assert!(status.contains("answer: Behind a flag."), "{status}");
    assert!(
        status.contains("decided by reviewer, an agent"),
        "an agent that decides is named as one: {status}"
    );

    let again = desk.run(&["answer", &ask, "No."]);
    assert_eq!(again.code, 1);
    assert!(again.err.contains("was already answered by reviewer"), "{}", again.err);
}

#[test]
fn an_ask_can_be_withdrawn_with_its_reason() {
    let desk = Desk::new("withdraw");
    desk.run(&["ask", "Which logo?"]).ok();
    let ask = first_ask(&desk);
    desk.run(&["answer", &ask, "the cover has no logo now", "--withdraw"]).ok();
    let status = desk.run(&["status"]).ok();
    assert!(status.contains("withdrawn: the cover has no logo now"), "{status}");
    assert!(status.starts_with("atlas · 0 waiting on the person"), "{status}");
}

#[test]
fn status_as_json_carries_the_four_marks() {
    let desk = Desk::new("json");
    desk.run(&["update", "importer", "--line", "started", "--private"]).ok();
    let status: serde_json::Value = serde_json::from_str(&desk.run(&["status", "--json"]).ok()).unwrap();
    let marks = &status["works"][0]["marks"];
    assert_eq!(marks["who"]["agent"], "builder");
    assert_eq!(marks["sure"], "claimed");
    assert!(marks["at"].as_str().unwrap().ends_with('Z'));
    assert_eq!(marks["project"], "atlas");
    assert_eq!(marks["private"], true);
    assert_eq!(status["works"][0]["standing"], "moving");
}

#[test]
fn what_is_private_to_one_project_is_not_shown_from_another() {
    let desk = Desk::new("private");
    desk.run(&["update", "salaries", "--line", "drafted", "--private"]).ok();
    desk.run(&["update", "importer", "--line", "ready"]).ok();
    let elsewhere = desk.run(&["status", "--all", "--project", "borealis"]).ok();
    assert!(elsewhere.contains("atlas/importer"), "{elsewhere}");
    assert!(!elsewhere.contains("salaries"), "{elsewhere}");
    let inside = desk.run(&["status", "--all"]).ok();
    assert!(inside.contains("atlas/salaries"), "{inside}");
    assert!(inside.contains("· private"), "{inside}");
}

#[test]
fn a_mistake_on_the_command_line_is_told_apart_from_a_refusal() {
    let desk = Desk::new("errors");
    for args in [
        &["update"][..],
        &["nope"],
        &["status", "--wrong"],
        &["update", "x", "--status", "blocked"],
        &["ask"],
    ] {
        let said = desk.run(args);
        assert_eq!(said.code, 2, "{args:?}: {}", said.err);
        assert!(said.out.is_empty());
    }
    for args in [
        &["update", "Bad", "--line", "x"][..],
        &["answer", "zzzzzz", "yes"],
        &["ask", "Which?", "--work", "ghost"],
    ] {
        let said = desk.run(args);
        assert_eq!(said.code, 1, "{args:?}: {}", said.err);
    }
    assert!(!desk.home().exists(), "nothing refused was written, and no keeper was started for it");
}

#[test]
fn whoever_speaks_is_named_by_the_tool_that_runs_it() {
    let desk = Desk::new("who");
    let dir = desk.root.join("work");
    let you = |c: &mut Command, more: &[&str]| {
        let out = Said::of(c.args(["where", "--project", "atlas"]).args(more).output().unwrap()).ok();
        out.lines().find(|l| l.starts_with("you")).unwrap().to_string()
    };
    assert_eq!(you(desk.command(&dir).env("CLAUDECODE", "1"), &[]), "you      claude-code (an agent)");
    assert_eq!(
        you(desk.command(&dir).env("CLAUDECODE", "1"), &["--as", "Reviewer Two"]),
        "you      reviewer-two (an agent)"
    );
    assert_eq!(
        you(desk.command(&dir).env("NAHUALI_ACTOR", "lead"), &["--from", "scout"]),
        "you      lead (from scout) (an agent)"
    );
    // No tool, no name and nobody at a terminal: it is not the person.
    assert_eq!(you(&mut desk.command(&dir), &[]), "you      unattended (an agent)");
}

#[test]
fn a_worktree_belongs_to_its_repository_and_carries_its_commit() {
    let desk = Desk::new("worktree");
    let repo = desk.root.join("Atlas Project");
    let tree = desk.root.join("some-other-folder");
    std::fs::create_dir_all(&repo).unwrap();
    let git = |dir: &Path, args: &[&str]| {
        let out = Command::new("git")
            .current_dir(dir)
            .args(["-c", "user.name=ada", "-c", "user.email=ada@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&repo, &["init", "-q"]);
    git(&repo, &["commit", "-q", "--allow-empty", "--no-verify", "-m", "first"]);
    git(&repo, &["worktree", "add", "-q", tree.to_str().unwrap(), "-b", "side"]);

    let out =
        Said::of(desk.command(&tree).env("NAHUALI_ACTOR", "builder").arg("where").output().unwrap()).ok();
    assert!(out.contains("project  atlas-project\n"), "{out}");
    assert!(!out.contains("commit   none"), "{out}");
    // Speaking of another project from here: this repository's commit is not its mark.
    let out = Said::of(
        desk.command(&tree)
            .env("NAHUALI_ACTOR", "builder")
            .args(["where", "--project", "borealis"])
            .output()
            .unwrap(),
    )
    .ok();
    assert!(out.contains("commit   none"), "{out}");
}

#[test]
fn many_agents_write_at_once_while_another_reads_and_nothing_is_lost() {
    let desk = Desk::new("many");
    desk.run(&["update", "first", "--line", "starts the keeper"]).ok();
    let writers: Vec<_> = (0..12)
        .map(|i| {
            let mut c = desk.command(&desk.root.join("work"));
            c.env("NAHUALI_ACTOR", format!("agent-{i}")).env("NAHUALI_PROJECT", "atlas");
            c.args(["update", &format!("work-{i}"), "--line", "written while others write"]).spawn().unwrap()
        })
        .collect();
    let read_meanwhile = desk.run(&["status"]).ok();
    assert!(read_meanwhile.starts_with("atlas · "), "{read_meanwhile}");
    for mut writer in writers {
        assert!(writer.wait().unwrap().success());
    }
    let status = desk.run(&["status"]).ok();
    assert!(status.starts_with("atlas · 0 waiting on the person · 13 pieces of work\n"), "{status}");
    assert!(desk.home().join("db").exists(), "it is kept in the database");
}

#[test]
fn everything_comes_out_as_plain_lines_and_goes_into_another_memory() {
    let desk = Desk::new("export");
    desk.run(&["update", "importer", "--line", "ready", "--verified", "cargo test"]).ok();
    desk.run(&["ask", "Ship behind a flag?", "--work", "importer"]).ok();
    let lines = desk.run(&["export"]).ok();
    assert_eq!(lines.lines().count(), 2);
    for line in lines.lines() {
        let event: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(event["project"], "atlas");
        assert!(event["at"].as_str().unwrap().ends_with('Z'));
    }

    // Another memory takes the lines in from its log folder, once.
    let other = Desk::new("import");
    std::fs::create_dir_all(other.home().join("log")).unwrap();
    std::fs::write(
        other.home().join("log").join("from-elsewhere.jsonl"),
        format!("{lines}{lines}not a line\n"),
    )
    .unwrap();
    assert_eq!(other.run(&["status"]).ok(), desk.run(&["status"]).ok());
    assert_eq!(other.run(&["export"]).ok(), lines, "taken in once, however many times it is there");
}

#[test]
fn a_memory_has_one_keeper_and_it_comes_back_by_itself() {
    let desk = Desk::new("keeper");
    desk.run(&["update", "importer", "--line", "ready"]).ok();
    let second = Said::of(desk.command(&desk.root).arg("serve").output().unwrap());
    assert_eq!(second.code, 1, "a second keeper is refused: {}", second.err);
    assert_eq!(desk.run(&["stop"]).ok(), "the keeper left\n");
    assert_eq!(desk.run(&["stop"]).ok(), "there was no keeper\n");
    let status = desk.run(&["status"]).ok();
    assert!(
        status.contains("importer  moving · ready"),
        "what was kept is there when the next keeper starts: {status}"
    );
}
