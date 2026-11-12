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
}

#[test]
fn setup_tells_the_tools_and_takes_it_back_leaving_their_files_as_they_were() {
    let desk = Desk::new("setup");
    std::fs::create_dir_all(desk.root.join(".claude")).unwrap();
    std::fs::create_dir_all(desk.root.join(".codex")).unwrap();
    let settings = desk.root.join(".claude").join("settings.json");
    let theirs = "{\n  \"model\": \"x\",\n  \"hooks\": {\n    \"Stop\": [\n      {\n        \"hooks\": [\n          {\n            \"type\": \"command\",\n            \"command\": \"their-stop\"\n          }\n        ]\n      }\n    ]\n  },\n  \"zeta\": true\n}\n";
    std::fs::write(&settings, theirs).unwrap();

    let dry = desk.feed(&["setup", "--hooks", "--dry-run"], "").ok();
    assert!(dry.starts_with("would do (nothing was changed):"), "{dry}");
    assert_eq!(std::fs::read_to_string(&settings).unwrap(), theirs);
    assert!(!desk.root.join(".claude/skills").exists());

    let done = desk.feed(&["setup", "--hooks"], "").ok();
    assert_eq!(done.lines().count(), 3, "{done}");
    for tool in [".claude", ".codex"] {
        let skill = std::fs::read_to_string(desk.root.join(tool).join("skills/nahuali/SKILL.md")).unwrap();
        assert!(skill.starts_with("---\nname: nahuali\n"), "{skill}");
    }
    let with: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    assert!(
        with["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap().ends_with("hook pre-tool")
    );
    assert_eq!(with["hooks"]["Stop"][0]["hooks"][0]["command"], "their-stop");
    let again = desk.feed(&["setup", "--hooks"], "").ok();
    assert!(again.lines().all(|l| l.starts_with("current")), "a second run changes nothing: {again}");

    desk.feed(&["setup", "--remove"], "").ok();
    assert_eq!(std::fs::read_to_string(&settings).unwrap(), theirs, "their file is exactly as it was");
    assert!(!desk.root.join(".claude/skills/nahuali").exists());
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
