//! The command as an agent meets it: run for real, on a memory of its own.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A memory and a working directory of their own, gone when the test ends.
struct Desk {
    root: PathBuf,
    /// Whether its keeper tells meaning, with a stand-in for the model in
    /// which a few words are known to say the same.
    meaning: bool,
}

impl Desk {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("nahuali-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("work")).unwrap();
        Self { root, meaning: false }
    }

    fn with_meaning(name: &str) -> Self {
        let mut desk = Self::new(name);
        desk.meaning = true;
        desk
    }

    fn home(&self) -> PathBuf {
        self.root.join("memory")
    }

    fn command(&self, dir: &Path) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_nahuali"));
        // A keeper nobody stops leaves by itself soon after the test.
        c.current_dir(dir).env("NAHUALI_HOME", self.home()).env("NAHUALI_IDLE_SECS", "20");
        c.env("NAHUALI_MEANING", if self.meaning { "test" } else { "off" }).env("NAHUALI_NEAR", "0.3");
        for marker in ["CLAUDECODE", "CODEX_SANDBOX", "CODEX_THREAD_ID", "CURSOR_AGENT", "CURSOR_TRACE_ID"] {
            c.env_remove(marker);
        }
        c.env_remove("NAHUALI_ACTOR").env_remove("NAHUALI_PROJECT").env_remove("NAHUALI_PERSON");
        c.env_remove("NAHUALI_USAGE");
        c.stdin(std::process::Stdio::null());
        c
    }

    /// Run with `input` on standard input, as a tool that runs agents would.
    fn feed(&self, args: &[&str], input: &str) -> Said {
        use std::io::Write;
        let mut child = self
            .command(&self.root.join("work"))
            .env("NAHUALI_ACTOR", "builder")
            .env("NAHUALI_PROJECT", "atlas")
            .env("HOME", &self.root)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        Said::of(child.wait_with_output().unwrap())
    }

    /// What a tool says before a command runs, and what the hook answers.
    fn before(&self, session: &str, command: &str) -> serde_json::Value {
        let input = serde_json::json!({
            "session_id": session, "cwd": self.root.join("work"), "tool_name": "Bash", "tool_input": { "command": command },
        });
        let out = self.feed(&["hook", "pre-tool"], &input.to_string()).ok();
        if out.trim().is_empty() { serde_json::Value::Null } else { serde_json::from_str(&out).unwrap() }
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
        // A keeper that fell over is started again by the next command, so
        // nothing else would ever show it. Its own words do.
        let said = std::fs::read_to_string(self.home().join("local").join("keeper.log")).unwrap_or_default();
        let fell = said.contains("overflowed its stack")
            || said.contains("panicked")
            || said.contains("fatal runtime error");
        if fell && !std::thread::panicking() {
            let _ = std::fs::remove_dir_all(&self.root);
            panic!("a keeper fell over during this test:\n{said}");
        }
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

#[test]
fn a_lesson_is_told_when_the_command_is_its_moment_once_per_session() {
    let desk = Desk::new("lesson");
    let said = desk
        .run(&[
            "learn",
            "Agent worktrees start from the default branch",
            "--moment",
            "before spawning an agent in its own worktree",
            "--when",
            "worktree add",
        ])
        .ok();
    assert!(
        said.starts_with("lesson:")
            && said.contains("is told when what is about to be done has \"worktree add\""),
        "{said}"
    );

    let asked = desk.run(&["check", "git worktree add ../side -b side"]).ok();
    assert!(asked.starts_with("1 lesson applies to this\n"), "{asked}");
    assert!(asked.contains("matched: add, worktree"), "{asked}");
    assert_eq!(desk.run(&["check", "cargo test"]).ok(), "no lesson applies\n");

    let told = desk.before("s1", "git worktree add ../x");
    let context = told["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert_eq!(told["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert!(
        told["hookSpecificOutput"]["permissionDecision"].is_null(),
        "a lesson that is told does not stop the command"
    );
    assert!(context.contains("Agent worktrees start from the default branch"), "{context}");
    assert!(desk.before("s1", "git worktree add ../y").is_null(), "once per session is enough");
    assert!(!desk.before("s2", "git worktree add ../z").is_null(), "another session has not heard it");
    assert!(desk.before("s3", "git worktree list").is_null(), "half the moment is not the moment");
    assert!(
        desk.before("s3", "nahuali check \"git worktree add\"").is_null(),
        "talking to the memory is nobody's moment"
    );
    assert!(desk.run(&["lessons"]).ok().contains("shown 3 times"), "every showing is recorded");
}

#[test]
fn a_lesson_that_stops_refuses_the_command_once_and_then_lets_it_through() {
    let desk = Desk::new("stop");
    desk.run(&[
        "learn",
        "Back up the database before migrating",
        "--moment",
        "before a migration",
        "--when",
        "migrate",
        "--stop",
        "--everywhere",
    ])
    .ok();
    let stopped = desk.before("s1", "./migrate up");
    assert_eq!(stopped["hookSpecificOutput"]["permissionDecision"], "deny");
    let why = stopped["hookSpecificOutput"]["permissionDecisionReason"].as_str().unwrap();
    assert!(why.contains("Back up the database before migrating") && why.contains("run it again"), "{why}");
    assert!(desk.before("s1", "./migrate up").is_null(), "the second time it goes through");

    let lessons = desk.run(&["lessons", "--project", "borealis"]).ok();
    assert!(
        lessons.contains("every project") && lessons.contains("stops the act once"),
        "it reaches another project: {lessons}"
    );
    let name = lessons.split("lesson:").nth(1).unwrap().split_whitespace().next().unwrap().to_string();
    desk.run(&["retire", &name, "migrations are reversible now"]).ok();
    assert!(desk.before("s9", "./migrate up").is_null(), "a retired lesson is gone");
    assert_eq!(desk.run(&["lessons"]).ok(), "atlas · no lesson in force\n");
}

#[test]
fn a_hook_never_breaks_the_session_it_runs_in() {
    let desk = Desk::new("hook");
    for input in ["", "not json", "{}", r#"{"cwd":"/nonexistent/place","tool_input":{"command":"x"}}"#] {
        for kind in ["pre-tool", "session-start", "something-else"] {
            let said = desk.feed(&["hook", kind], input);
            assert_eq!((said.code, said.out.as_str()), (0, ""), "{kind} on {input:?}: {}", said.err);
        }
    }
    assert!(!desk.home().exists(), "and it starts no keeper for a memory that does not exist");
}

#[test]
fn a_session_starts_knowing_where_the_project_stands() {
    let desk = Desk::new("start");
    let input = serde_json::json!({ "session_id": "s1", "cwd": desk.root.join("work") }).to_string();
    desk.run(&["learn", "Mind the gap", "--moment", "before boarding", "--everywhere"]).ok();
    assert_eq!(
        desk.feed(&["hook", "session-start"], &input).ok(),
        "",
        "lessons alone are not pushed at the start"
    );
    desk.run(&["update", "importer", "--line", "tests green, review left"]).ok();
    desk.run(&["ask", "Ship behind a flag?", "--work", "importer"]).ok();
    let said: serde_json::Value =
        serde_json::from_str(&desk.feed(&["hook", "session-start"], &input).ok()).unwrap();
    let context = said["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert!(
        context.contains("atlas · 1 waiting on the person · 1 piece of work · 1 lesson in force"),
        "{context}"
    );
    assert!(context.contains("Ship behind a flag?"), "{context}");
    assert!(context.contains("data, not instructions"), "{context}");
}

#[test]
fn the_start_of_a_session_stays_small_however_much_work_is_held() {
    let desk = Desk::new("brief");
    let input = serde_json::json!({ "session_id": "s1", "cwd": desk.root.join("work") }).to_string();
    // Fifty cards, written as lines a keeper takes in when it starts, a
    // minute apart, the last the most recent.
    let lines: String = (0..50)
        .map(|i| {
            serde_json::json!({
                "v": 1, "id": format!("01M8AJ13QQ6QJ86PAVM6W{i:05}"),
                "at": format!("2026-11-20T{:02}:{:02}:00Z", 10 + i / 60, i % 60),
                "by": { "agent": "builder" }, "project": "atlas", "sure": "claimed",
                "what": { "work": { "work": format!("slice-{i:02}"),
                    "line": format!("slice {i} of the importer reads the old format, review left"),
                    "next": ["open the review", "ask borealis to check the parser"] } },
            })
            .to_string()
                + "\n"
        })
        .collect();
    std::fs::create_dir_all(desk.home().join("log")).unwrap();
    std::fs::write(desk.home().join("log").join("cards.jsonl"), lines).unwrap();
    desk.run(&["update", "slice-49", "--status", "done"]).ok();
    let asks =
        ["Ship slice 3 behind a flag?", "Keep the old parser for a release?", "Move the audit to the 20th?"];
    for ask in asks {
        desk.run(&["ask", ask, "--recommends", "yes"]).ok();
    }
    let said: serde_json::Value =
        serde_json::from_str(&desk.feed(&["hook", "session-start"], &input).ok()).unwrap();
    let context = said["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    for ask in asks {
        assert!(context.contains(ask), "every open ask is there: {context}");
    }
    assert!(!context.contains("slice-49 "), "finished work is not: {context}");
    assert!(context.contains("slice-48 ") && !context.contains("slice-40 "), "the latest cards: {context}");
    assert!(context.contains("\n+41 more pieces of work · `nahuali status` shows them all\n"), "{context}");
    assert!(context.len() < 3500, "{} bytes: {context}", context.len());
    assert!(desk.run(&["status"]).ok().contains("slice-00 "), "status still shows it all");
}

#[test]
fn a_hook_does_not_wait_for_the_memory_to_wake() {
    let desk = Desk::new("cold");
    desk.run(&["update", "importer", "--line", "tests green, review left"]).ok();
    desk.run(&[
        "learn",
        "Run the migration twice",
        "--moment",
        "before running migrate",
        "--when",
        "migrate",
    ])
    .ok();
    // A keeper that takes seconds to start: it has lines to take in.
    let lines: String = (0..100)
        .map(|i| {
            serde_json::json!({
                "v": 1, "id": format!("01M8AJ13QQ6QJ86PAVM6W{i:05}"), "at": "2026-11-20T10:00:00Z",
                "by": { "agent": "builder" }, "project": "atlas", "sure": "claimed",
                "what": { "note": { "note": format!("n{i:05}"), "text": format!("the audit of slice {i} is on the 20th") } },
            })
            .to_string()
                + "\n"
        })
        .collect();
    std::fs::create_dir_all(desk.home().join("log")).unwrap();
    std::fs::write(desk.home().join("log").join("notes.jsonl"), lines).unwrap();
    let cwd = desk.root.join("work");
    for kind in ["session-start", "pre-tool", "prompt"] {
        desk.run(&["stop"]).ok();
        let input = serde_json::json!({
            "session_id": "s1", "cwd": cwd, "tool_name": "Bash", "tool_input": { "command": "bin/migrate up" },
        });
        let began = std::time::Instant::now();
        let said = desk.feed(&["hook", kind], &input.to_string()).ok();
        let took = began.elapsed();
        assert!(took < std::time::Duration::from_millis(1000), "{kind} waited {took:?}");
        if kind == "session-start" {
            assert!(said.contains("waking up") && said.contains("nahuali status"), "{said}");
        } else {
            assert_eq!(said, "", "{kind}");
        }
        // The keeper it started is there for whoever comes next.
        let began = std::time::Instant::now();
        assert!(desk.run(&["status"]).ok().contains("importer"));
        assert!(began.elapsed() > took, "{kind}: the memory really was asleep");
    }
    let told = desk.before("s2", "bin/migrate up");
    assert!(told.to_string().contains("Run the migration twice"), "once awake, a hook tells: {told}");
}

#[test]
fn recall_finds_what_holds_every_word_and_says_so_when_nothing_does() {
    let desk = Desk::new("recall");
    desk.run(&["update", "importer", "--line", "reads the three export formats"]).ok();
    desk.run(&[
        "note",
        "The audit is on the 20th, with the decisión about the importer",
        "--tag",
        "dates",
        "--about",
        "importer",
    ])
    .ok();
    desk.run(&["note", "The audit fee is 4000", "--private"]).ok();
    desk.run(&["learn", "Exports from 2019 lack a header", "--moment", "before parsing old exports"]).ok();

    let audit = desk.run(&["recall", "audit"]).ok();
    assert!(audit.starts_with("atlas · 2 things known about \"audit\"\n"), "{audit}");
    let roots = desk.run(&["recall", "exporting formats"]).ok();
    assert!(roots.contains("work:atlas/importer"), "a word finds its other forms: {roots}");
    assert!(desk.run(&["recall", "decision"]).ok().contains("decisión"), "and its accents");
    assert_eq!(
        desk.run(&["recall", "audit kubernetes"]).ok(),
        "atlas · nothing known about \"audit kubernetes\"\n"
    );
    assert_eq!(desk.run(&["recall", "the of"]).ok(), "atlas · nothing known about \"the of\"\n");

    let elsewhere = desk.run(&["recall", "audit", "--all", "--project", "borealis"]).ok();
    assert!(
        elsewhere.contains("on the 20th") && !elsewhere.contains("4000"),
        "nothing private is offered to another project: {elsewhere}"
    );
    assert_eq!(
        desk.run(&["recall", "audit", "--project", "borealis"]).ok(),
        "borealis · nothing known about \"audit\"\n"
    );

    let found: serde_json::Value =
        serde_json::from_str(&desk.run(&["recall", "fee", "--json"]).ok()).unwrap();
    let note = found["found"][0]["key"].as_str().unwrap().to_string();
    assert_eq!(found["found"][0]["marks"]["private"], true);
    desk.run(&["forget", &note, "it changed"]).ok();
    assert_eq!(
        desk.run(&["recall", "fee"]).ok(),
        "atlas · nothing known about \"fee\"\n",
        "what was forgotten is not recalled"
    );

    // What can be searched is rebuilt from the events when a keeper starts.
    desk.run(&["stop"]).ok();
    assert!(
        desk.run(&["recall", "audit"]).ok().starts_with("atlas · 1 thing known"),
        "the private note is gone, the other is found again"
    );
}

#[test]
fn the_same_verbs_answer_over_the_model_context_protocol() {
    let desk = Desk::new("mcp");
    let requests = [
        serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "clientInfo": { "name": "Some Tool" } } }),
        serde_json::json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        serde_json::json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "update", "arguments": { "work": "pricing", "line": "drafted", "done": ["table", "copy"], "project": "borealis" } } }),
        serde_json::json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "update", "arguments": { "work": "Bad Name" } } }),
        serde_json::json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "status", "arguments": { "project": "borealis" } } }),
        serde_json::json!({ "jsonrpc": "2.0", "id": 6, "method": "nope" }),
    ];
    let input: String = requests.iter().map(|r| format!("{r}\n")).collect();
    let mut child = {
        use std::io::Write;
        let mut c = desk.command(&desk.root.join("work"));
        let mut child = c
            .arg("mcp")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        child
    };
    let mut out = String::new();
    std::io::Read::read_to_string(child.stdout.as_mut().unwrap(), &mut out).unwrap();
    assert!(child.wait().unwrap().success());
    let answers: Vec<serde_json::Value> = out.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(answers.len(), 6, "a notification gets no answer: {out}");
    assert_eq!(answers[0]["result"]["serverInfo"]["name"], "nahuali");
    let tools: Vec<&str> = answers[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(tools, ["status", "update", "ask", "answer", "learn", "check", "note", "recall"]);
    assert_eq!(answers[1]["result"]["tools"][1]["inputSchema"]["required"], serde_json::json!(["work"]));
    assert_eq!(answers[2]["result"]["isError"], false);
    assert_eq!(answers[2]["result"]["content"][0]["text"], "borealis/pricing · moving · drafted\n");
    assert_eq!(answers[3]["result"]["isError"], true);
    let status = answers[4]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(status.contains("some-tool · claimed"), "the client is who spoke: {status}");
    assert!(answers[5]["error"]["message"].as_str().unwrap().contains("nope"));
    let calls: Vec<_> = uses(&desk).into_iter().filter(|l| l["surface"] == "mcp").collect();
    let verbs: Vec<&str> = calls.iter().map(|l| l["verb"].as_str().unwrap()).collect();
    assert_eq!(verbs, ["update", "update", "status"], "each call is kept as its own: {calls:?}");
    assert_eq!(calls[0]["by"], "some-tool");
    assert_eq!(calls[1]["outcome"], "error");
}

#[test]
fn setup_tells_the_tools_and_takes_it_back_leaving_their_files_as_they_were() {
    let desk = Desk::new("setup");
    std::fs::create_dir_all(desk.root.join(".claude")).unwrap();
    std::fs::create_dir_all(desk.root.join(".codex")).unwrap();
    let settings = desk.root.join(".claude").join("settings.json");
    let codex = desk.root.join(".codex").join("hooks.json");
    let theirs = "{\n  \"model\": \"x\",\n  \"hooks\": {\n    \"Stop\": [\n      {\n        \"hooks\": [\n          {\n            \"type\": \"command\",\n            \"command\": \"their-stop\"\n          }\n        ]\n      }\n    ]\n  },\n  \"zeta\": true\n}\n";
    std::fs::write(&settings, theirs).unwrap();

    let dry = desk.feed(&["setup", "--hooks", "--dry-run"], "").ok();
    assert!(dry.starts_with("would do (nothing was changed):"), "{dry}");
    assert_eq!(std::fs::read_to_string(&settings).unwrap(), theirs);
    assert!(!desk.root.join(".claude/skills").exists() && !codex.exists());

    let done = desk.feed(&["setup", "--hooks"], "").ok();
    assert!(done.contains("Codex runs a new hook only after you review it"), "{done}");
    for tool in [".claude", ".codex"] {
        let skill = std::fs::read_to_string(desk.root.join(tool).join("skills/nahuali/SKILL.md")).unwrap();
        assert!(skill.starts_with("---\nname: nahuali\n"), "{skill}");
    }
    let with: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert!(
        with["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("hook pre-tool claude-code")
    );
    assert_eq!(with["hooks"]["Stop"][0]["hooks"][0]["command"], "their-stop");
    let with: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&codex).unwrap()).unwrap();
    assert!(
        with["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("hook pre-tool codex")
    );
    assert_eq!(with["hooks"]["PreToolUse"][0]["matcher"], "Bash");
    assert!(
        with["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("hook session-start codex")
    );
    let again = desk.feed(&["setup", "--hooks"], "").ok();
    assert!(again.lines().all(|l| l.starts_with("current")), "a second run changes nothing: {again}");

    desk.feed(&["setup", "--remove"], "").ok();
    assert_eq!(std::fs::read_to_string(&settings).unwrap(), theirs, "their file is exactly as it was");
    assert!(!codex.exists(), "a file that held only our hooks is gone");
    assert!(!desk.root.join(".claude/skills/nahuali").exists());
}

#[test]
fn the_tool_that_calls_a_hook_is_who_showed_the_lesson() {
    let desk = Desk::new("hook-who");
    desk.run(&["learn", "Back up first", "--moment", "before a migration", "--when", "migrate up"]).ok();
    let input = serde_json::json!({ "session_id": "c1", "cwd": desk.root.join("work"), "tool_name": "Bash", "tool_input": { "command": "./migrate up" } });
    let said = Said::of({
        use std::io::Write;
        let mut child = desk
            .command(&desk.root.join("work"))
            .env("NAHUALI_PROJECT", "atlas")
            .args(["hook", "pre-tool", "codex"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input.to_string().as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    })
    .ok();
    assert!(said.contains("Back up first"), "Codex takes the same answer as Claude Code: {said}");
    let events = desk.run(&["export"]).ok();
    let shown = events.lines().last().unwrap();
    assert!(shown.contains(r#""by":{"agent":"codex"}"#) && shown.contains(r#""shown""#), "{shown}");
}

#[test]
fn the_database_itself_keeps_reach_and_forgets_what_is_gone() {
    let desk = Desk::new("index");
    desk.run(&["note", "The audit is on the 20th"]).ok();
    desk.run(&["note", "The audit fee is 4000", "--private"]).ok();
    desk.run(&["learn", "Audit the exports first", "--moment", "before an audit"]).ok();
    let keys =
        |args: &[&str]| desk.run(&[&["recall", "audit", "--keys"], args].concat()).ok().lines().count();
    assert_eq!(keys(&[]), 3);
    assert_eq!(keys(&["--all", "--project", "borealis"]), 2, "the private note never leaves the database");
    assert_eq!(keys(&["--project", "borealis"]), 0, "and nothing of atlas reaches borealis unasked");

    let fee = desk.run(&["recall", "fee", "--keys"]).ok();
    desk.run(&["forget", fee.trim(), "it changed"]).ok();
    assert_eq!(desk.run(&["recall", "fee", "--keys"]).ok(), "", "what was forgotten is no longer indexed");
    assert_eq!(keys(&[]), 2);
}

#[test]
fn the_names_of_the_works_share_one_column() {
    let desk = Desk::new("column");
    desk.run(&["update", "a", "--line", "short name"]).ok();
    desk.run(&["update", "a-much-longer-name", "--line", "long name"]).ok();
    let status = desk.run(&["status"]).ok();
    let column = |needle: &str| status.lines().find(|l| l.contains(needle)).unwrap().find("moving").unwrap();
    assert_eq!(column("short name"), column("long name"), "{status}");
}

#[test]
fn what_is_said_in_other_words_is_found_and_said_to_be_a_likeness() {
    let desk = Desk::with_meaning("near");
    desk.run(&["note", "The audit is on the 20th"]).ok();
    desk.run(&["note", "The audit fee is private", "--private"]).ok();
    desk.run(&["note", "Lunch is at noon"]).ok();

    let other_words = desk.run(&["recall", "inspection"]).ok();
    assert!(
        other_words
            .starts_with("atlas · nothing holds the words \"inspection\"; 2 things are close in meaning\n"),
        "{other_words}"
    );
    assert!(other_words.contains("CLOSE IN MEANING (not the same words; judge them)"), "{other_words}");
    assert!(other_words.contains("alike") && !other_words.contains("Lunch"), "{other_words}");

    let same_words = desk.run(&["recall", "audit"]).ok();
    assert!(same_words.starts_with("atlas · 2 things known about \"audit\"\n"), "{same_words}");
    assert!(
        !same_words.contains("CLOSE IN MEANING"),
        "what the words found is not offered twice: {same_words}"
    );

    let elsewhere = desk.run(&["recall", "inspection", "--all", "--project", "borealis"]).ok();
    assert!(
        elsewhere.contains("on the 20th") && !elsewhere.contains("fee"),
        "a likeness keeps to its reach too: {elsewhere}"
    );
    assert_eq!(desk.run(&["recall", "kubernetes"]).ok(), "atlas · nothing known about \"kubernetes\"\n");

    // What the database itself answers, before the command looks at it.
    let from_elsewhere = desk.run(&["recall", "inspection", "--keys", "--all", "--project", "borealis"]).ok();
    assert_eq!(
        from_elsewhere.lines().count(),
        1,
        "the private note never leaves the database: {from_elsewhere}"
    );
    assert!(from_elsewhere.starts_with("near note:"), "{from_elsewhere}");

    let json: serde_json::Value =
        serde_json::from_str(&desk.run(&["recall", "inspection", "--json"]).ok()).unwrap();
    assert_eq!(json["found"].as_array().unwrap().len(), 0);
    assert!(json["near"][0]["alike"].as_f64().unwrap() > 0.3);

    // The vectors are kept: a keeper that starts again finds the same.
    desk.run(&["stop"]).ok();
    assert_eq!(desk.run(&["recall", "inspection"]).ok(), other_words);
}

#[test]
fn a_likeness_is_for_whoever_asks_and_unasked_only_the_words_speak() {
    let desk = Desk::with_meaning("near-lesson");
    desk.run(&["learn", "Back up first", "--moment", "before a migration", "--when", "migrate up", "--stop"])
        .ok();

    let asked = desk.run(&["check", "alembic upgrade"]).ok();
    assert!(asked.starts_with("1 lesson applies to this\n") && asked.contains("close in meaning"), "{asked}");
    assert_eq!(desk.run(&["check", "cargo test"]).ok(), "no lesson applies\n");

    assert!(desk.before("s1", "alembic upgrade").is_null(), "before a command, a likeness says nothing");
    assert_eq!(
        desk.before("s2", "./migrate up")["hookSpecificOutput"]["permissionDecision"],
        "deny",
        "its words do"
    );
    assert!(desk.before("s3", "ls -la").is_null());
}

#[test]
fn without_a_model_there_are_words_and_nothing_pretends_otherwise() {
    let desk = Desk::new("no-model");
    desk.run(&["note", "The audit is on the 20th"]).ok();
    assert_eq!(desk.run(&["recall", "inspection"]).ok(), "atlas · nothing known about \"inspection\"\n");
    assert!(desk.run(&["recall", "audit"]).ok().starts_with("atlas · 1 thing known"));
    assert!(desk.run(&["where"]).ok().contains("meaning  off: words only"));
}

#[test]
fn a_long_script_that_holds_the_words_somewhere_is_nobodys_moment() {
    let desk = Desk::new("far");
    desk.run(&[
        "learn",
        "Announce a change of foundations first",
        "--moment",
        "before replacing the database",
        "--when",
        "replace database",
    ])
    .ok();
    assert!(!desk.before("s1", "replace the database with plain files").is_null());
    let script = format!(
        "python3 - <<EOF\ns = s.replace(a, b)\n{}print('the database is fine')\nEOF",
        "x = x + 1  # and so on, line after line\n".repeat(8)
    );
    assert!(desk.before("s2", &script).is_null(), "the words are there, far apart, and mean nothing");
    // Right next to each other, but inside what the command writes.
    let written = "cat > notes.txt <<'EOF'\nreplace database\nEOF";
    assert!(desk.before("s3", written).is_null(), "what a command writes is not what it does");
    let near = "cat schema.sql | grep replace && echo the whole of the old database is fine";
    assert!(desk.before("s4", near).is_null(), "nor words that are near and not a phrase");
}

/// A SurrealDB server in memory, gone when the test ends. `None` when no
/// `surreal` is installed here: the team's tests are then skipped, and say so.
struct TeamServer {
    child: std::process::Child,
    url: String,
}

impl TeamServer {
    fn start() -> Option<Self> {
        let port = std::net::TcpListener::bind("127.0.0.1:0").ok()?.local_addr().ok()?.port();
        let child = Command::new("surreal")
            .args([
                "start",
                "memory",
                "--bind",
                &format!("127.0.0.1:{port}"),
                "--user",
                "root",
                "--pass",
                "test-root",
                "--log",
                "error",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        let server = Self { child, url: format!("ws://127.0.0.1:{port}") };
        for _ in 0..100 {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(300));
                return Some(server);
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        None
    }

    /// Run as whoever runs the server.
    fn owner(&self, desk: &Desk, args: &[&str]) -> Said {
        Said::of(
            desk.command(&desk.root)
                .env("NAHUALI_ROOT_USER", "root")
                .env("NAHUALI_ROOT_PASS", "test-root")
                .args(args)
                .output()
                .unwrap(),
        )
    }

    /// Add a person and put `desk` on the team as them.
    fn join(&self, desk: &Desk, name: &str) {
        let said = self.owner(desk, &["team", "add", name, &self.url]).ok();
        let pass = said.split("NAHUALI_PASS=").nth(1).unwrap().split_whitespace().next().unwrap().to_string();
        let joined = Said::of(
            desk.command(&desk.root)
                .env("NAHUALI_PASS", pass)
                .args(["team", "join", &self.url, "--name", name])
                .output()
                .unwrap(),
        );
        assert!(joined.ok().contains(&format!("you are {name} on it")));
    }
}

impl Drop for TeamServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn a_team_shares_one_memory_and_the_server_keeps_each_to_their_reach() {
    let Some(server) = TeamServer::start() else {
        assert!(
            std::env::var_os("CI").is_none(),
            "no `surreal` on PATH, and in CI the team's tests may not be skipped"
        );
        eprintln!("no `surreal` on PATH: the team's test was skipped");
        return;
    };
    let (ada, bob) = (Desk::new("team-ada"), Desk::new("team-bob"));
    ada.run(&["update", "notebook", "--line", "written before joining"]).ok();
    assert!(
        server.owner(&ada, &["team", "init", &server.url]).ok().contains("is ready to hold a team's memory")
    );
    server.join(&ada, "ada");
    server.join(&bob, "bob");
    assert_eq!(server.owner(&ada, &["team", "add", "ada", &server.url]).code, 1, "a name is one person");
    let wrong = Said::of(
        bob.command(&bob.root)
            .env("NAHUALI_PASS", "not-it")
            .args(["team", "join", &server.url, "--name", "ada"])
            .output()
            .unwrap(),
    );
    assert_eq!(wrong.code, 1, "a wrong passphrase lets nobody in: {}", wrong.err);

    ada.run(&["update", "importer", "--line", "tests green", "--verified", "cargo test"]).ok();
    ada.run(&["note", "The audit is on the 20th"]).ok();
    ada.run(&["note", "My own reminder about salaries", "--mine"]).ok();
    ada.run(&["ask", "Which logo goes on the cover?", "--to", "bob"]).ok();

    // Bob has written nothing yet, and sees what is shared.
    let seen = bob.run(&["status"]).ok();
    assert!(
        seen.contains("importer  moving · tests green") && seen.contains("Which logo goes on the cover?"),
        "{seen}"
    );
    assert!(!seen.contains("notebook"), "what ada wrote before joining stayed on her machine: {seen}");
    assert!(bob.run(&["recall", "audit"]).ok().starts_with("atlas · 1 thing known"));
    assert_eq!(
        bob.run(&["recall", "salaries"]).ok(),
        "atlas · nothing known about \"salaries\"\n",
        "what is only ada's never reaches bob"
    );
    assert!(
        ada.run(&["recall", "salaries"]).ok().contains("only")
            || ada.run(&["recall", "salaries"]).ok().starts_with("atlas · 1 thing known")
    );
    assert_eq!(bob.run(&["recall", "salaries", "--keys"]).ok(), "", "not even into the index on his machine");
    let hers = ada.run(&["status"]).ok();
    assert!(hers.contains("for bob"), "ada sees that the question waits on bob: {hers}");

    // Bob answers; ada finds the decision.
    let ask = first_ask(&bob);
    bob.run(&["answer", &ask, "The blue one."]).ok();
    assert!(ada.run(&["status"]).ok().contains("answer: The blue one."));

    // A line that carries ada's name, sent from bob's machine, as a changed
    // copy of nahuali would send it: the server refuses it.
    bob.run(&["stop"]).ok();
    std::fs::create_dir_all(bob.home().join("log")).unwrap();
    let forged = r#"{"v":1,"id":"01M7ZZZZZZZZZZZZZZZZFORGED","at":"2026-11-12T10:00:00Z","by":{"person":"ada"},"person":"ada","project":"atlas","sure":"claimed","what":{"work":{"work":"forged","line":"in ada's name"}}}"#;
    // And one in bob's own name that says ada spoke: it arrives, and is read as bob saying so.
    let pretended = r#"{"v":1,"id":"01M7ZZZZZZZZZZZZZZZPRETEND","at":"2026-11-12T10:00:01Z","by":{"person":"ada"},"person":"bob","project":"atlas","sure":"claimed","what":{"work":{"work":"pretended","line":"bob says ada said"}}}"#;
    std::fs::write(bob.home().join("log").join("sent.jsonl"), format!("{forged}\n{pretended}\n")).unwrap();
    bob.run(&["status"]).ok();
    let after = ada.run(&["status"]).ok();
    assert!(!after.contains("forged"), "{after}");
    assert!(after.contains("pretended") && after.contains("bob (from ada)"), "{after}");

    // Leaving goes back to what was on the machine.
    assert!(
        Said::of(ada.command(&ada.root).args(["team", "leave"]).output().unwrap())
            .ok()
            .starts_with("back to the memory on this machine")
    );
    let alone = ada.run(&["status"]).ok();
    assert!(alone.contains("notebook") && !alone.contains("importer"), "{alone}");
}

#[test]
fn only_yours_and_asked_of_someone_work_on_one_machine_too() {
    let desk = Desk::new("people");
    let as_person = |person: &str, args: &[&str]| {
        Said::of(
            desk.command(&desk.root.join("work"))
                .env("NAHUALI_PERSON", person)
                .env("NAHUALI_ACTOR", "builder")
                .env("NAHUALI_PROJECT", "atlas")
                .args(args)
                .output()
                .unwrap(),
        )
        .ok()
    };
    as_person("ada", &["note", "Shared between us"]);
    as_person("ada", &["note", "Only for ada", "--mine"]);
    as_person("ada", &["ask", "Bob, which logo?", "--to", "bob"]);
    assert_eq!(as_person("ada", &["recall", "ada"]).lines().count(), 3);
    assert_eq!(as_person("bob", &["recall", "ada"]), "atlas · nothing known about \"ada\"\n");
    assert!(as_person("ada", &["status"]).contains("for bob"));
    assert!(!as_person("bob", &["status"]).contains("for bob"), "bob is not told it is for someone else");
    let json: serde_json::Value = serde_json::from_str(&as_person("bob", &["status", "--json"])).unwrap();
    assert_eq!(json["asks"][0]["to"], "bob");
    assert_eq!(json["asks"][0]["marks"]["person"], "ada");
}

/// Make `desk` answer other machines on a port of its own; returns where,
/// and the key the others join with.
fn listening(desk: &Desk) -> (String, String) {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let addr = format!("127.0.0.1:{port}");
    let said = desk.run(&["node", "listen", &addr]).ok();
    let key = said.split("NAHUALI_KEY=").nth(1).unwrap().split_whitespace().next().unwrap().to_string();
    assert_eq!(key.len(), 64, "{said}");
    (addr, key)
}

fn joining(desk: &Desk, addr: &str, key: &str) -> Said {
    Said::of(
        desk.command(&desk.root.join("work"))
            .env("NAHUALI_ACTOR", "builder")
            .env("NAHUALI_PROJECT", "atlas")
            .env("NAHUALI_KEY", key)
            .args(["node", "join", addr])
            .output()
            .unwrap(),
    )
}

#[test]
fn machines_that_worked_alone_hold_the_same_once_they_meet() {
    let (pc, laptop, mini) = (Desk::new("node-pc"), Desk::new("node-laptop"), Desk::new("node-mini"));
    assert_eq!(pc.run(&["node"]).ok(), "this memory keeps to this machine\n");
    pc.run(&["update", "server-room", "--line", "written on the pc"]).ok();
    laptop.run(&["update", "importer", "--line", "written on a train"]).ok();
    laptop.run(&["note", "The audit is on the 20th"]).ok();
    mini.run(&["learn", "Back up before migrating", "--moment", "before a migration", "--when", "migrate"])
        .ok();

    let (addr, key) = listening(&pc);
    assert!(joining(&laptop, &addr, &key).ok().contains("took 1, gave 2"));
    assert!(joining(&mini, &addr, &key).ok().contains("took 3, gave 1"));
    // The laptop never talks to the mini: it finds its lesson on the pc.
    assert!(laptop.run(&["node", "sync"]).ok().contains("took 1, gave 0"));
    let held: Vec<String> = [&pc, &laptop, &mini].iter().map(|d| d.run(&["export"]).ok()).collect();
    assert_eq!(held[0].lines().count(), 4);
    assert!(held[0] == held[1] && held[1] == held[2], "every machine holds the same events");
    assert!(
        laptop.run(&["check", "./migrate up"]).ok().starts_with("1 lesson applies"),
        "and what one learned, all know"
    );
    assert!(
        laptop.run(&["recall", "server room"]).ok().contains("written on the pc"),
        "the index follows what arrives"
    );
    assert!(mini.run(&["node", "sync"]).ok().contains("took 0, gave 0"), "meeting again exchanges nothing");

    // With the pc off, the laptop works on, and says the pc does not answer.
    pc.run(&["stop"]).ok();
    laptop.run(&["update", "importer", "--line", "more work, with the pc off"]).ok();
    assert!(laptop.run(&["node", "sync"]).ok().contains("does not answer"));
    pc.run(&["status"]).ok();
    // Told to, or by itself at the next half minute: either way it arrives.
    assert!(!laptop.run(&["node", "sync"]).ok().contains("does not answer"));
    assert!(pc.run(&["status", "--all"]).ok().contains("more work, with the pc off"));

    assert!(laptop.run(&["node", "forget"]).ok().starts_with("this memory keeps to this machine again"));
    assert!(laptop.run(&["status", "--all"]).ok().contains("server-room"), "what it took stays");
}

#[test]
fn a_machine_without_the_key_takes_nothing_and_gives_nothing() {
    let (pc, stranger) = (Desk::new("node-home"), Desk::new("node-stranger"));
    pc.run(&["note", "Something only this house knows"]).ok();
    stranger.run(&["note", "Something planted from outside"]).ok();
    let (addr, key) = listening(&pc);
    let said = joining(&stranger, &addr, &"7".repeat(64)).ok();
    assert!(said.contains("does not share this memory's key"), "{said}");
    assert_eq!(stranger.run(&["export"]).ok().lines().count(), 1, "it took nothing");
    pc.run(&["status"]).ok();
    assert_eq!(pc.run(&["export"]).ok().lines().count(), 1, "and nothing of its own got in");
    assert_eq!(joining(&stranger, &addr, "not-a-key").code, 2);
    assert_eq!(
        joining(&stranger, &addr, &key).code,
        1,
        "a memory shares one key: the other has to be forgotten first"
    );

    // The key file is for this user alone.
    use std::os::unix::fs::PermissionsExt;
    let mode =
        std::fs::metadata(pc.home().join("local").join("node.json")).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn doctor_says_what_is_not_right_and_what_to_do_about_it() {
    let desk = Desk::new("doctor");
    let nothing = desk.feed(&["doctor"], "");
    assert_eq!(nothing.code, 0, "{}", nothing.out);
    assert!(
        nothing.out.contains("nothing written yet") && nothing.out.ends_with("everything is right\n"),
        "{}",
        nothing.out
    );

    // A tool that runs agents and was never told.
    std::fs::create_dir_all(desk.root.join(".claude")).unwrap();
    desk.run(&["update", "importer", "--line", "started"]).ok();
    let untold = desk.feed(&["doctor"], "");
    assert_eq!(untold.code, 1);
    assert!(
        untold.out.contains("!!  claude-code") && untold.out.contains("does not know about nahuali"),
        "{}",
        untold.out
    );
    assert!(untold.out.contains("ok  keeper") && untold.out.contains("1 events"), "{}", untold.out);
    assert!(untold.out.ends_with("1 thing to look at\n"), "{}", untold.out);

    // Told, with hooks, but never called: hooks in place are not hooks that run.
    desk.feed(&["setup", "--hooks"], "").ok();
    let silent = desk.feed(&["doctor"], "");
    assert!(silent.out.contains("hooks in place but never called"), "{}", silent.out);
    desk.feed(&["hook", "session-start", "claude-code"], "{}").ok();
    let right = desk.feed(&["doctor"], "");
    assert_eq!(right.code, 0, "{}", right.out);
    assert!(right.out.contains("last called moments ago"), "{}", right.out);
    let json: serde_json::Value = serde_json::from_str(&desk.feed(&["doctor", "--json"], "").ok()).unwrap();
    assert_eq!(json["fine"], true);

    // A keeper that fell over is started again by the next command: only its own words show it.
    let log = desk.home().join("local").join("keeper.log");
    let before = std::fs::read_to_string(&log).unwrap();
    std::fs::write(&log, format!("{before}thread 'x' has overflowed its stack\n")).unwrap();
    let fell = desk.feed(&["doctor"], "");
    assert_eq!(fell.code, 1);
    assert!(fell.out.contains("!!  keeper") && fell.out.contains("fell over 1 time"), "{}", fell.out);
    std::fs::write(&log, before).unwrap();
}

#[test]
fn a_card_older_than_the_commits_is_remembered_once_when_the_person_speaks() {
    let desk = Desk::new("remind");
    let repo = desk.root.join("atlas");
    std::fs::create_dir_all(&repo).unwrap();
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(&repo)
            .args(["-c", "user.name=ada", "-c", "user.email=ada@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    let nahuali = |args: &[&str]| {
        Said::of(desk.command(&repo).env("NAHUALI_ACTOR", "builder").args(args).output().unwrap()).ok()
    };
    let spoken = || {
        use std::io::Write;
        let mut child = desk
            .command(&repo)
            .args(["hook", "prompt", "claude-code"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let input = serde_json::json!({ "session_id": "s1", "cwd": repo, "prompt": "go on" }).to_string();
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout).to_string()
    };
    git(&["init", "-q"]);
    git(&["commit", "-q", "--allow-empty", "--no-verify", "-m", "first"]);
    assert_eq!(spoken(), "", "no card, nothing to remember");

    nahuali(&["update", "importer", "--line", "tests green, review left"]);
    assert_eq!(spoken(), "", "the card was written at this commit");

    // The clock of a commit counts whole seconds: let one pass.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    git(&["commit", "-q", "--allow-empty", "--no-verify", "-m", "second"]);
    let said: serde_json::Value = serde_json::from_str(&spoken()).unwrap();
    let context = said["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
    assert_eq!(said["hookSpecificOutput"]["hookEventName"], "UserPromptSubmit");
    assert!(
        context.contains("the card `importer` of atlas") && context.contains("there is 1 commit since"),
        "{context}"
    );
    assert!(context.contains("ignore this"), "it asks for nothing that is not there: {context}");
    assert_eq!(spoken(), "", "once for each commit");

    nahuali(&["update", "importer", "--line", "merged"]);
    assert_eq!(spoken(), "", "a card written after the commit is not old");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    git(&["commit", "-q", "--allow-empty", "--no-verify", "-m", "third"]);
    nahuali(&["update", "importer", "--status", "done"]);
    assert_eq!(spoken(), "", "finished work is not asked about");
}

/// Every line nahuali kept about its own use in this memory.
fn uses(desk: &Desk) -> Vec<serde_json::Value> {
    let dir = desk.home().join("local").join("usage");
    let mut lines = Vec::new();
    for file in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let text = std::fs::read_to_string(file.path()).unwrap();
        lines.extend(text.lines().map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()));
    }
    lines
}

#[test]
fn its_own_use_is_kept_without_a_word_of_what_was_said() {
    let desk = Desk::new("uses");
    desk.run(&["status"]).ok();
    assert!(!desk.home().exists(), "a memory that does not exist is not created to count its use");

    desk.run(&["update", "importer", "--line", "salaries of cobalt", "--verified", "cargo test"]).ok();
    desk.run(&["recall", "lighthouse"]).ok();
    desk.run(&["recall", "salaries"]).ok();
    assert_eq!(desk.run(&["pending"]).code, 2);
    assert_eq!(desk.run(&["status", "--since", "7d"]).code, 2);
    assert_eq!(desk.run(&["the audit is on the 20th"]).code, 2);
    desk.before("s1", "git push");

    let lines = uses(&desk);
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    for said in ["salaries of cobalt", "cargo test", "lighthouse", "audit is on"] {
        assert!(!text.contains(said), "what was said is never kept ({said}): {text}");
    }
    let update = lines.iter().find(|l| l["verb"] == "update").unwrap();
    assert_eq!(update["flags"], serde_json::json!(["line", "verified"]));
    assert_eq!((update["outcome"].as_str(), update["by"].as_str()), (Some("ok"), Some("builder")));
    assert_eq!(update["project"], "atlas");
    assert_eq!(update["surface"], "cli");
    assert!(update["ms"].is_u64() && update["version"].is_string(), "{update}");
    let empty = lines.iter().find(|l| l["verb"] == "recall").unwrap();
    assert_eq!(empty["counts"], serde_json::json!({ "found": 0, "near": 0 }));
    let unknown: Vec<_> = lines.iter().filter(|l| l["kind"] == "unknown-verb").collect();
    assert_eq!(unknown[0]["unknown"], "pending");
    assert_eq!(unknown[1]["unknown"], "(not a word)", "a sentence typed as a verb is not kept");
    let flag = lines.iter().find(|l| l["kind"] == "unknown-flag").unwrap();
    assert_eq!((flag["verb"].as_str(), flag["unknown"].as_str()), (Some("status"), Some("--since")));
    let hook = lines.iter().find(|l| l["surface"] == "hook").unwrap();
    assert_eq!((hook["verb"].as_str(), hook["counts"]["said"].as_u64()), (Some("pre-tool"), Some(0)));

    let report = desk.run(&["usage"]).ok();
    assert!(report.starts_with("nahuali usage · the last 7 days · "), "{report}");
    for part in [
        "VERBS",
        "update",
        "NOT USED IN THE LAST 7 DAYS",
        "nahuali pending ×1",
        "--since on status ×1",
        "unknown-flag ×1",
        "2 asked · 1 came back with nothing (50%)",
        "pre-tool",
    ] {
        assert!(report.contains(part), "{part}: {report}");
    }
    let json: serde_json::Value =
        serde_json::from_str(&desk.run(&["usage", "--json", "--since", "all"]).ok()).unwrap();
    assert_eq!(json["recall"], serde_json::json!({ "asked": 2, "empty": 1 }));
    assert_eq!(json["verbs"]["update"]["calls"], 1);
    assert_eq!(json["unknown"]["nahuali pending"], 1);
    assert_eq!(desk.run(&["usage", "--since", "7w"]).code, 2);
}

#[test]
fn friction_is_said_on_purpose_and_shown_to_whoever_keeps_nahuali() {
    let desk = Desk::new("friction");
    let said =
        desk.run(&["usage", "--friction", "recall needs every word;  I had to guess three times"]).ok();
    assert!(said.starts_with("friction kept"), "{said}");
    let report = desk.run(&["usage"]).ok();
    assert!(report.contains("FRICTION"), "{report}");
    assert!(
        report.contains("builder · atlas · recall needs every word; I had to guess three times"),
        "{report}"
    );
    assert_eq!(uses(&desk).iter().filter(|l| l["friction"].is_string()).count(), 1, "kept once");
    assert_eq!(desk.run(&["usage", "--friction", "  "]).code, 2);
}

#[test]
fn when_use_is_not_to_be_recorded_nothing_is() {
    let desk = Desk::new("uses-off");
    desk.run(&["update", "importer", "--line", "started"]).ok();
    let before = uses(&desk).len();
    let off = |args: &[&str]| {
        let out = desk
            .command(&desk.root.join("work"))
            .env("NAHUALI_ACTOR", "builder")
            .env("NAHUALI_PROJECT", "atlas")
            .env("NAHUALI_USAGE", "off")
            .env("HOME", &desk.root)
            .args(args)
            .output()
            .unwrap();
        (out.status.code(), String::from_utf8_lossy(&out.stdout).to_string())
    };
    assert_eq!(off(&["status"]).0, Some(0));
    assert_eq!(off(&["nope"]).0, Some(2));
    assert_eq!(
        off(&["usage", "--friction", "slow"]).0,
        Some(1),
        "friction that cannot be kept is not said to be"
    );
    assert_eq!(uses(&desk).len(), before, "nothing was added");
    assert!(off(&["doctor"]).1.contains("not recorded: NAHUALI_USAGE is off"));
    assert!(desk.feed(&["doctor"], "").ok().contains("lines this month"));
}

#[test]
fn every_verb_says_what_it_takes_when_asked_for_help() {
    let desk = Desk::new("help");
    desk.run(&["update", "importer", "--line", "started"]).ok();
    let events = desk.run(&["export"]).ok();
    // Before, a verb refused `--help` as a flag it does not take.
    let setup = desk.run(&["setup", "--help"]).ok();
    assert!(setup.contains("nahuali setup [--hooks]") && setup.contains("--meaning"), "{setup}");
    assert!(setup.contains("\neverywhere\n"), "what every command takes goes with it: {setup}");
    assert!(!setup.contains("nahuali recall"), "only that verb: {setup}");
    assert_eq!(desk.run(&["setup", "-h"]).ok(), setup);
    assert_eq!(desk.run(&["help", "setup"]).ok(), setup);

    let update = desk.run(&["update", "importer", "--line", "x", "--help"]).ok();
    assert!(update.contains("\nupdate WORK\n  --line") && update.contains("--verified"), "{update}");
    assert_eq!(desk.run(&["export"]).ok(), events, "asking for help wrote nothing");

    let all = desk.run(&["help"]).ok();
    assert!(all.starts_with("nahuali: a working memory") && all.contains("\na team\n"), "{all}");
    assert_eq!(desk.run(&["--help"]).ok(), all);

    let nope = desk.run(&["nope", "--help"]);
    assert_eq!(nope.code, 2);
    assert!(nope.err.contains("`nope` is not something nahuali does"), "{}", nope.err);
    assert_eq!(desk.run(&["help", "nope"]).code, 2);

    let lines = uses(&desk);
    let helped: Vec<_> =
        lines.iter().filter_map(|l| l["verb"].as_str()).filter(|v| v.starts_with("help")).collect();
    assert_eq!(
        helped,
        ["help setup", "help setup", "help setup", "help update", "help", "help", "help nope", "help nope"]
    );
    let unknown: Vec<_> = lines.iter().filter(|l| l["kind"] == "unknown-verb").collect();
    assert!(unknown.iter().all(|l| l["unknown"] == "nope"), "{unknown:?}");
}

#[test]
fn a_verb_of_the_old_nahuali_is_refused_with_what_to_use_now() {
    let desk = Desk::new("old-verbs");
    desk.run(&["update", "importer", "--line", "started"]).ok();
    let pending = desk.run(&["pending"]);
    assert_eq!(pending.code, 2);
    assert!(
        pending.err.contains(
            "`pending` was a verb of the old nahuali; what waits on the person and the work in motion is `nahuali status`"
        ),
        "{}",
        pending.err
    );
    let done = desk.run(&["intention-complete", "importer"]);
    assert_eq!(done.code, 2);
    assert!(done.err.contains("`nahuali update WORK --status done`"), "{}", done.err);
    for (old, now) in
        [("remember", "nahuali note"), ("procedure", "nahuali learn"), ("backup", "nahuali export")]
    {
        let said = desk.run(&[old, "x"]);
        assert_eq!(said.code, 2);
        assert!(said.err.contains(now), "{old}: {}", said.err);
    }

    let lines = uses(&desk);
    let unknown: Vec<_> = lines.iter().filter(|l| l["kind"] == "unknown-verb").collect();
    assert_eq!(unknown.len(), 5, "still a verb nahuali does not have: {lines:?}");
    assert_eq!(unknown[0]["unknown"], "pending");
    let report = desk.run(&["usage"]).ok();
    assert!(
        report.contains("nahuali pending ×1") && report.contains("nahuali intention-complete ×1"),
        "{report}"
    );
}

#[test]
fn what_doctor_finds_to_look_at_is_not_counted_as_a_failure() {
    let desk = Desk::new("told");
    desk.run(&["update", "importer", "--line", "started"]).ok();
    // A tool that runs agents and was never told: doctor has something to say.
    std::fs::create_dir_all(desk.root.join(".claude")).unwrap();
    assert_eq!(desk.feed(&["doctor"], "").code, 1, "its exit status says so, as before");
    assert_eq!(desk.run(&["status", "--since", "7d"]).code, 2);
    let told = uses(&desk).into_iter().find(|l| l["verb"] == "doctor").unwrap();
    assert_eq!((told["outcome"].as_str(), told.get("kind")), (Some("told"), None), "{told}");

    // A line written before `told` was an outcome of its own still reads.
    let month = told["at"].as_str().unwrap()[..7].to_string();
    let old = serde_json::json!({
        "v": 1, "at": told["at"], "surface": "cli", "verb": "doctor", "by": "builder", "ms": 5,
        "outcome": "error", "kind": "told", "version": "0.1.0",
    });
    let file = desk.home().join("local").join("usage").join(format!("{month}.jsonl"));
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str(&format!("{old}\n"));
    std::fs::write(&file, text).unwrap();

    let json: serde_json::Value = serde_json::from_str(&desk.run(&["usage", "--json"]).ok()).unwrap();
    let doctor = &json["verbs"]["doctor"];
    assert_eq!(
        (doctor["calls"].as_u64(), doctor["failed"].as_u64(), doctor["told"].as_u64()),
        (Some(2), Some(0), Some(2))
    );
    assert_eq!(json["verbs"]["status"]["failed"], 1, "a real failure is still one");
    let report = desk.run(&["usage"]).ok();
    let row = report.lines().find(|l| l.trim_start().starts_with("doctor")).unwrap();
    assert_eq!(row.split_whitespace().take(4).collect::<Vec<_>>(), ["doctor", "2", "0", "2"], "{report}");
    assert!(!report.contains(" told ×"), "not among what failed: {report}");
}
