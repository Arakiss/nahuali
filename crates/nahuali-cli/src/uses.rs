//! What nahuali records about its own use, so that whoever keeps it can see
//! how it is really used and where it gets in the way, and repair it from
//! that instead of from a guess.
//!
//! One line of JSON for each command, each call over MCP and each hook, in
//! this machine's own folder of the memory (`<home>/local/usage/`). The verb,
//! the names of the flags, who called, the project, how long it took and how
//! it ended. Never the words of what was said, never the value of a flag.
//! The one text it keeps is friction: a line an agent writes on purpose,
//! about nahuali itself, with `nahuali usage --friction`.
//!
//! Nothing leaves the machine. Recording never fails a command and never
//! makes it wait: whatever goes wrong is dropped. `NAHUALI_USAGE=off` stops it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use nahuali_core::{Actor, Time};
use serde_json::{Map, Value, json};

use crate::{Failure, env, parse, usage, words};

/// The verbs a person or an agent is meant to use. One not used in a while
/// is a verb nobody needs, or one nobody finds.
const VERBS: [&str; 15] = [
    "status", "update", "ask", "answer", "learn", "check", "lessons", "retire", "note", "recall", "forget",
    "where", "doctor", "export", "board",
];

/// What runs for as long as someone looks at it: how long it took says nothing.
const LONG: [&str; 1] = ["board"];

/// The words of a command that has words of its own after it.
const UNDER: [(&str, &[&str]); 2] =
    [("team", &["init", "add", "join", "leave"]), ("node", &["listen", "join", "sync", "forget"])];

/// What a command found out about itself while it ran, for its line.
#[derive(Default)]
struct Measure {
    counts: Vec<(&'static str, u64)>,
    project: Option<String>,
    /// The command already wrote its own line.
    kept: bool,
}

thread_local! {
    static NOW: RefCell<Measure> = RefCell::default();
}

/// How many of something the command gave: things found, lessons told.
pub fn count(name: &'static str, n: usize) {
    NOW.with(|m| m.borrow_mut().counts.push((name, n as u64)));
}

/// The project the command spoke of, once it knows it.
pub fn seen_project(project: &str) {
    NOW.with(|m| m.borrow_mut().project = Some(project.to_string()));
}

fn take() -> Measure {
    NOW.with(|m| std::mem::take(&mut *m.borrow_mut()))
}

/// Whether use is recorded: unless `NAHUALI_USAGE` says off.
pub fn on() -> bool {
    let said = std::env::var("NAHUALI_USAGE").unwrap_or_default().trim().to_ascii_lowercase();
    !matches!(said.as_str(), "off" | "0" | "no" | "false")
}

pub fn dir(home: &Path) -> PathBuf {
    home.join("local").join("usage")
}

/// A name as typed, when it is a name. A sentence typed where a verb goes is
/// someone's words, and is not kept.
fn word(typed: &str) -> String {
    let fits = !typed.is_empty()
        && typed.len() <= 32
        && typed.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if fits { typed.to_ascii_lowercase() } else { "(not a word)".to_string() }
}

/// The names of the flags given, without their values, each once.
fn flags(argv: &[String]) -> Vec<String> {
    let mut names = Vec::new();
    for arg in argv {
        if arg == "--" {
            break;
        }
        if let Some(name) = arg.strip_prefix("--") {
            let name = word(name.split('=').next().unwrap_or_default());
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// The value given to a flag, read without knowing what the command takes.
fn value_of<'a>(argv: &'a [String], name: &str) -> Option<&'a str> {
    let long = format!("--{name}");
    let inline = format!("--{name}=");
    let mut rest = argv.iter();
    while let Some(arg) = rest.next() {
        if arg == "--" {
            return None;
        }
        if arg == &long {
            return rest.next().map(String::as_str);
        }
        if let Some(value) = arg.strip_prefix(&inline) {
            return Some(value);
        }
    }
    None
}

/// The first line of what a failure said, without what was quoted in it:
/// what is quoted is what someone typed.
fn said(text: &str) -> String {
    let first = text.lines().next().unwrap_or_default();
    let mut out = String::new();
    let mut quote: Option<char> = None;
    for c in first.chars() {
        match (quote, c) {
            (None, '`' | '"') => {
                quote = Some(c);
                out.push('…');
            }
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, c) => out.push(c),
        }
    }
    if let Some(home) = std::env::var("HOME").ok().filter(|h| h.len() > 1) {
        out = out.replace(&home, "~");
    }
    if out.chars().count() > 160 {
        out = out.chars().take(159).collect::<String>() + "…";
    }
    out
}

/// Who called: the tool or agent, or the person. Never the person's name.
fn caller(named: Option<&str>) -> String {
    match env::actor(named, None) {
        Ok(Actor::Person(_)) => "person".to_string(),
        Ok(Actor::Agent(name)) => name,
        Ok(Actor::Reported { by, .. }) => by,
        Err(_) if std::io::stdin().is_terminal() => "person".to_string(),
        Err(_) => "unattended".to_string(),
    }
}

/// How a call ended, as fields of its line.
fn ending(line: &mut Map<String, Value>, result: &Result<String, Failure>, typed: &str) {
    let (kind, unknown, text) = match result {
        Ok(_) => {
            line.insert("outcome".into(), json!("ok"));
            return;
        }
        Err(Failure::Usage(t)) if t.ends_with("is not something nahuali does") => {
            ("unknown-verb", Some(word(typed)), Some(t))
        }
        Err(Failure::Usage(t)) if t.contains(" is not something this command takes") => {
            let name = t.strip_prefix("--").and_then(|r| r.split(' ').next()).unwrap_or_default();
            ("unknown-flag", Some(format!("--{}", word(name))), Some(t))
        }
        Err(Failure::Usage(t)) if t.ends_with(" takes a value") => ("missing-value", None, Some(t)),
        Err(Failure::Usage(t)) => ("usage", None, Some(t)),
        Err(Failure::Refused(t)) => ("refused", None, Some(t)),
        // What doctor says when something is not right: its own report.
        Err(Failure::Told(_)) => ("told", None, None),
    };
    line.insert("outcome".into(), json!("error"));
    line.insert("kind".into(), json!(kind));
    if let Some(unknown) = unknown {
        line.insert("unknown".into(), json!(unknown));
    }
    if let Some(text) = text {
        line.insert("said".into(), json!(said(text)));
    }
}

/// Add one line to the month's file. Only in a memory that exists, unless
/// `make`: reading a memory that does not exist creates nothing.
fn keep(home: &Path, line: &Value, make: bool) -> std::io::Result<()> {
    if !make && !home.join("local").is_dir() {
        return Ok(());
    }
    let dir = dir(home);
    std::fs::create_dir_all(&dir)?;
    let month = line["at"].as_str().and_then(|at| at.get(..7)).unwrap_or("unknown");
    let mut file =
        std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{month}.jsonl")))?;
    // One write, so lines written at once by many do not mix.
    file.write_all(format!("{line}\n").as_bytes())
}

struct Call<'a> {
    surface: &'static str,
    verb: String,
    by: String,
    argv: &'a [String],
    took: Duration,
    typed: &'a str,
}

fn record(call: Call<'_>, result: &Result<String, Failure>) {
    let measure = take();
    let cold = crate::remote::STARTED.swap(false, Ordering::Relaxed);
    if measure.kept || !on() {
        return;
    }
    let Ok(home) = env::home(value_of(call.argv, "home")) else { return };
    let project = measure.project.or_else(|| env::project(value_of(call.argv, "project")).ok());
    let mut line = Map::new();
    line.insert("v".into(), json!(1));
    line.insert("at".into(), json!(Time::from_millis(env::now_ms()).rfc3339()));
    line.insert("surface".into(), json!(call.surface));
    line.insert("verb".into(), json!(call.verb));
    line.insert("by".into(), json!(call.by));
    if let Some(project) = project {
        line.insert("project".into(), json!(project));
    }
    let flags = flags(call.argv);
    if !flags.is_empty() {
        line.insert("flags".into(), json!(flags));
    }
    line.insert("ms".into(), json!(call.took.as_millis() as u64));
    ending(&mut line, result, call.typed);
    let mut counts: Map<String, Value> =
        measure.counts.into_iter().map(|(k, n)| (k.into(), json!(n))).collect();
    if call.surface == "hook"
        && let Ok(out) = result
    {
        counts.insert("said".into(), json!(u64::from(!out.trim().is_empty())));
    }
    if !counts.is_empty() {
        line.insert("counts".into(), Value::Object(counts));
    }
    if cold {
        line.insert("cold".into(), json!(true));
    }
    line.insert("version".into(), json!(env!("CARGO_PKG_VERSION")));
    let _ = keep(&home, &Value::Object(line), false);
}

/// After a command has run and said what it had to say.
pub fn after_command(argv: &[String], result: &Result<String, Failure>, took: Duration) {
    let first = argv.first().map(String::as_str);
    let looked = || if std::io::stdout().is_terminal() { "board" } else { "status" };
    let (verb, rest) = match first {
        None => (looked(), argv),
        Some(f) if f.starts_with("--") && !matches!(f, "--help" | "--version") => (looked(), argv),
        Some("help" | "--help" | "-h") => ("help", &argv[1..]),
        Some("--version" | "-V") => ("version", &argv[1..]),
        Some(f) => (f, &argv[1..]),
    };
    // A server that answers for hours: its calls are recorded one by one.
    if matches!(verb, "mcp" | "serve") {
        return;
    }
    let (surface, verb, by) = if verb == "hook" {
        let kind = word(rest.first().map(String::as_str).unwrap_or_default());
        let by = rest.get(1).map_or_else(|| caller(None), |tool| word(tool));
        ("hook", kind, by)
    } else {
        let under = UNDER
            .iter()
            .find(|(top, _)| *top == verb)
            .and_then(|(_, subs)| rest.first().filter(|s| subs.contains(&s.as_str())));
        let verb = match under {
            Some(sub) => format!("{verb} {sub}"),
            None => word(verb),
        };
        ("cli", verb, caller(value_of(argv, "as")))
    };
    let typed = first.unwrap_or_default();
    record(Call { surface, verb, by, argv, took, typed }, result);
}

/// After one call over the Model Context Protocol. `argv` is the command
/// line it stood for; with no tool of that name there is none.
pub fn after_mcp(tool: &str, argv: &[String], result: &Result<String, Failure>, took: Duration) {
    let by = caller(value_of(argv, "as"));
    let unknown;
    let result = if argv.is_empty() {
        unknown = Err(Failure::Usage(format!("`{tool}` is not something nahuali does")));
        &unknown
    } else {
        result
    };
    record(Call { surface: "mcp", verb: word(tool), by, argv, took, typed: tool }, result);
}

/// `nahuali usage [--since 24h|7d|30d|all] [--json]`, or `--friction "TEXT"`.
pub fn report(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &["since", "friction"], &["json"])?;
    let [] =
        words(&args, "usage takes no words; to say what got in your way: nahuali usage --friction \"…\"")?;
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    if let Some(text) = args.value("friction") {
        return friction(&home, text, args.value("as"), args.value("project"));
    }
    let since = window(args.value("since").unwrap_or("7d"))?;
    let now = Time::from_millis(env::now_ms());
    let from = since.as_ref().map(|(seconds, _)| Time(now.0.saturating_sub(*seconds)));
    let lines = read(&home, from);
    let tally = Tally::of(&lines);
    let span = since.map_or_else(|| "all that was kept".to_string(), |(_, says)| format!("the last {says}"));
    let recording = if on() {
        format!("recording is on · kept on this machine only, in {}", dir(&home).display())
    } else {
        "recording is off (NAHUALI_USAGE=off)".to_string()
    };
    if args.has("json") {
        return Ok(format!("{}\n", tally.json(&span, on(), &dir(&home))));
    }
    Ok(tally.text(&span, &recording, now))
}

/// A line an agent or the person writes about nahuali itself: what got in
/// the way. It is the one text this log keeps, and it is kept on purpose.
fn friction(home: &Path, text: &str, named: Option<&str>, project: Option<&str>) -> Result<String, Failure> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err(usage(
            "say what got in your way: nahuali usage --friction \"recall found nothing for …\"",
        ));
    }
    if !on() {
        return Err(Failure::Refused(
            "use is not recorded here (NAHUALI_USAGE=off), so the friction was not kept".to_string(),
        ));
    }
    let text: String = text.chars().take(500).collect();
    let mut line = json!({
        "v": 1, "at": Time::from_millis(env::now_ms()).rfc3339(), "surface": "cli", "verb": "usage",
        "by": caller(named), "friction": text, "version": env!("CARGO_PKG_VERSION"),
    });
    if let Ok(project) = env::project(project) {
        line["project"] = json!(project);
    }
    keep(home, &line, true).map_err(|e| Failure::Refused(format!("the friction could not be kept: {e}")))?;
    NOW.with(|m| m.borrow_mut().kept = true);
    Ok("friction kept · `nahuali usage` shows it to whoever keeps nahuali\n".to_string())
}

/// `24h`, `7d`, `30d` or `all`: how far back, and how to say it.
fn window(said: &str) -> Result<Option<(u64, String)>, Failure> {
    if said == "all" {
        return Ok(None);
    }
    let wrong = || usage("--since takes hours or days, as 24h or 7d, or all");
    let (n, unit) = said.split_at(said.len().saturating_sub(1));
    let n: u64 = n.parse().map_err(|_| wrong())?;
    match unit {
        "h" if n > 0 => Ok(Some((n * 3600, format!("{n} h")))),
        "d" if n == 1 => Ok(Some((86_400, "day".to_string()))),
        "d" if n > 0 => Ok(Some((n * 86_400, format!("{n} days")))),
        _ => Err(wrong()),
    }
}

/// Every line kept since `from`, in the order they were written.
fn read(home: &Path, from: Option<Time>) -> Vec<Value> {
    let month = from.map(|t| t.rfc3339()[..7].to_string());
    let Ok(entries) = std::fs::read_dir(dir(home)) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .filter(|p| {
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            month.as_ref().is_none_or(|m| stem.as_str() >= m.as_str())
        })
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .flat_map(|text| {
            text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).collect::<Vec<_>>()
        })
        .filter(|line| {
            let at = line["at"].as_str().and_then(Time::parse);
            at.is_some() && from.is_none_or(|from| at >= Some(from))
        })
        .collect()
}

/// The lines this month, for `doctor`.
pub fn this_month(home: &Path) -> usize {
    let month = &Time::from_millis(env::now_ms()).rfc3339()[..7];
    std::fs::read_to_string(dir(home).join(format!("{month}.jsonl"))).map_or(0, |t| t.lines().count())
}

/// The value at `p` (0 to 1) of what was measured, by nearest rank.
fn rank(sorted: &[u64], p: f64) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let i = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
    Some(sorted[i])
}

#[derive(Default)]
struct Verb {
    calls: u64,
    failed: u64,
    ms: Vec<u64>,
    last: Time,
    /// Each kind of failure: how many, and what one of them said.
    failures: BTreeMap<String, (u64, String)>,
}

#[derive(Default)]
struct Hook {
    runs: u64,
    ms: Vec<u64>,
    said: u64,
    told: u64,
    stopped: u64,
}

#[derive(Default)]
struct Tally {
    calls: u64,
    first: Option<Time>,
    last: Option<Time>,
    verbs: BTreeMap<String, Verb>,
    by: BTreeMap<String, u64>,
    surfaces: BTreeMap<String, u64>,
    unknown: BTreeMap<String, u64>,
    recalls: u64,
    empty: u64,
    hooks: BTreeMap<String, Hook>,
    friction: Vec<(Time, String, String, String)>,
}

fn n(line: &Value, path: &str) -> u64 {
    line.pointer(path).and_then(Value::as_u64).unwrap_or(0)
}

fn text_of(line: &Value, field: &str) -> String {
    line[field].as_str().unwrap_or("?").to_string()
}

impl Tally {
    fn of(lines: &[Value]) -> Self {
        let mut t = Self::default();
        for line in lines {
            let at = line["at"].as_str().and_then(Time::parse).unwrap_or_default();
            t.first = Some(t.first.map_or(at, |f| f.min(at)));
            t.last = Some(t.last.map_or(at, |l| l.max(at)));
            let verb = text_of(line, "verb");
            if let Some(said) = line["friction"].as_str() {
                let project = line["project"].as_str().unwrap_or("-").to_string();
                t.friction.push((at, text_of(line, "by"), project, said.to_string()));
                continue;
            }
            t.calls += 1;
            *t.by.entry(text_of(line, "by")).or_default() += 1;
            let surface = text_of(line, "surface");
            *t.surfaces.entry(surface.clone()).or_default() += 1;
            let ms = n(line, "/ms");
            if surface == "hook" {
                let hook = t.hooks.entry(verb).or_default();
                hook.runs += 1;
                hook.ms.push(ms);
                hook.said += n(line, "/counts/said");
                hook.told += n(line, "/counts/told");
                hook.stopped += n(line, "/counts/stopped");
                continue;
            }
            let failed = line["outcome"] == "error";
            if let Some(unknown) = line["unknown"].as_str() {
                let key = if line["kind"] == "unknown-verb" {
                    format!("nahuali {unknown}")
                } else {
                    format!("{unknown} on {verb}")
                };
                *t.unknown.entry(key).or_default() += 1;
            }
            if verb == "recall" && !failed {
                t.recalls += 1;
                if n(line, "/counts/found") + n(line, "/counts/near") == 0 {
                    t.empty += 1;
                }
            }
            // A verb nahuali does not have is counted where it was typed, not as a verb.
            if line["kind"] == "unknown-verb" {
                continue;
            }
            let v = t.verbs.entry(verb.clone()).or_default();
            v.calls += 1;
            v.last = v.last.max(at);
            if !LONG.contains(&verb.as_str()) {
                v.ms.push(ms);
            }
            if failed {
                v.failed += 1;
                let kind = text_of(line, "kind");
                let entry = v.failures.entry(kind).or_insert((0, String::new()));
                entry.0 += 1;
                if let Some(said) = line["said"].as_str() {
                    entry.1 = said.to_string();
                }
            }
        }
        for v in t.verbs.values_mut() {
            v.ms.sort_unstable();
        }
        for h in t.hooks.values_mut() {
            h.ms.sort_unstable();
        }
        t
    }

    fn unused(&self) -> Vec<&'static str> {
        VERBS.iter().copied().filter(|v| !self.verbs.contains_key(*v)).collect()
    }

    fn text(&self, span: &str, recording: &str, now: Time) -> String {
        if self.calls == 0 && self.friction.is_empty() {
            return format!("nahuali usage · nothing recorded in {span}\n  {recording}\n");
        }
        let ms = |v: Option<u64>| v.map_or_else(|| "-".to_string(), |v| format!("{v}ms"));
        let mut out = format!(
            "nahuali usage · {span} · {} call{}, the first {}, the last {}\n  {recording}\n",
            self.calls,
            if self.calls == 1 { "" } else { "s" },
            self.first.unwrap_or_default().ago(now),
            self.last.unwrap_or_default().ago(now),
        );
        if !self.verbs.is_empty() {
            out.push_str(&format!(
                "\nVERBS\n  {:<14} {:>6} {:>7} {:>8} {:>8}   last\n",
                "", "calls", "failed", "p50", "p95"
            ));
            let mut verbs: Vec<_> = self.verbs.iter().collect();
            verbs.sort_by(|a, b| b.1.calls.cmp(&a.1.calls).then(a.0.cmp(b.0)));
            for (name, v) in verbs {
                out.push_str(&format!(
                    "  {name:<14} {:>6} {:>7} {:>8} {:>8}   {}\n",
                    v.calls,
                    v.failed,
                    ms(rank(&v.ms, 0.5)),
                    ms(rank(&v.ms, 0.95)),
                    v.last.ago(now)
                ));
            }
        }
        let unused = self.unused();
        if !unused.is_empty() {
            out.push_str(&format!("\nNOT USED IN {}\n  {}\n", span.to_uppercase(), unused.join(" · ")));
        }
        if !self.by.is_empty() {
            let join = |m: &BTreeMap<String, u64>| {
                let mut all: Vec<_> = m.iter().collect();
                all.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                all.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(" · ")
            };
            out.push_str(&format!("\nWHO CALLS\n  {}\n  {}\n", join(&self.by), join(&self.surfaces)));
        }
        let failures: Vec<String> = self
            .verbs
            .iter()
            .flat_map(|(name, v)| {
                v.failures.iter().map(move |(kind, (n, said))| {
                    let said = if said.is_empty() { String::new() } else { format!("  {said}") };
                    format!("  {name:<14} {kind} ×{n}{said}\n")
                })
            })
            .collect();
        if !failures.is_empty() {
            out.push_str("\nWHAT FAILED\n");
            out.push_str(&failures.concat());
        }
        if !self.unknown.is_empty() {
            out.push_str("\nTYPED, BUT NOT SOMETHING NAHUALI TAKES\n");
            let mut all: Vec<_> = self.unknown.iter().collect();
            all.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            for (typed, n) in all {
                out.push_str(&format!("  {typed} ×{n}\n"));
            }
        }
        if self.recalls > 0 {
            out.push_str(&format!(
                "\nRECALL\n  {} asked · {} came back with nothing ({}%)\n",
                self.recalls,
                self.empty,
                self.empty * 100 / self.recalls
            ));
        }
        if !self.hooks.is_empty() {
            out.push_str("\nHOOKS\n");
            for (kind, h) in &self.hooks {
                let mut parts = vec![
                    format!("{} run{}", h.runs, if h.runs == 1 { "" } else { "s" }),
                    format!("p50 {}", ms(rank(&h.ms, 0.5))),
                    format!("p95 {}", ms(rank(&h.ms, 0.95))),
                    format!("said something {} time{}", h.said, if h.said == 1 { "" } else { "s" }),
                ];
                if h.told > 0 {
                    parts.push(format!("told {} lesson{}", h.told, if h.told == 1 { "" } else { "s" }));
                }
                if h.stopped > 0 {
                    parts.push(format!(
                        "stopped {} command{}",
                        h.stopped,
                        if h.stopped == 1 { "" } else { "s" }
                    ));
                }
                out.push_str(&format!("  {kind:<14} {}\n", parts.join(" · ")));
            }
        }
        if !self.friction.is_empty() {
            out.push_str("\nFRICTION (what got in the way, in the words of who met it)\n");
            for (at, by, project, said) in self.friction.iter().rev().take(20) {
                out.push_str(&format!("  {} · {by} · {project} · {said}\n", at.ago(now)));
            }
        }
        out
    }

    fn json(&self, span: &str, recording: bool, dir: &Path) -> Value {
        let verbs: Map<String, Value> = self
            .verbs
            .iter()
            .map(|(name, v)| {
                let failures: Map<String, Value> = v
                    .failures
                    .iter()
                    .map(|(kind, (n, said))| (kind.clone(), json!({ "n": n, "said": said })))
                    .collect();
                let value = json!({
                    "calls": v.calls, "failed": v.failed, "p50_ms": rank(&v.ms, 0.5), "p95_ms": rank(&v.ms, 0.95),
                    "last": v.last, "failures": failures,
                });
                (name.clone(), value)
            })
            .collect();
        let hooks: Map<String, Value> = self
            .hooks
            .iter()
            .map(|(kind, h)| {
                let value = json!({
                    "runs": h.runs, "p50_ms": rank(&h.ms, 0.5), "p95_ms": rank(&h.ms, 0.95),
                    "said": h.said, "told": h.told, "stopped": h.stopped,
                });
                (kind.clone(), value)
            })
            .collect();
        let friction: Vec<Value> = self
            .friction
            .iter()
            .map(|(at, by, project, said)| json!({ "at": at, "by": by, "project": project, "said": said }))
            .collect();
        json!({
            "window": span, "recording": recording, "dir": dir.display().to_string(),
            "calls": self.calls, "first": self.first, "last": self.last,
            "verbs": verbs, "unused": self.unused(), "by": self.by, "surfaces": self.surfaces,
            "unknown": self.unknown, "recall": { "asked": self.recalls, "empty": self.empty },
            "hooks": hooks, "friction": friction,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_string).collect()
    }

    #[test]
    fn only_the_names_of_flags_are_kept_never_their_values() {
        let given = argv("importer --line tests-green --verified=cargo-test --done a --done b -- --literal");
        assert_eq!(flags(&given), ["line", "verified", "done"]);
        assert_eq!(value_of(&argv("x --project atlas"), "project"), Some("atlas"));
        assert_eq!(value_of(&argv("x --project=atlas"), "project"), Some("atlas"));
        assert_eq!(value_of(&argv("x -- --project atlas"), "project"), None);
    }

    #[test]
    fn a_sentence_where_a_name_goes_is_not_kept() {
        assert_eq!(word("pending"), "pending");
        assert_eq!(word("Intention-Complete"), "intention-complete");
        assert_eq!(word("the audit is on the 20th"), "(not a word)");
        assert_eq!(word(""), "(not a word)");
    }

    #[test]
    fn what_a_failure_quotes_is_left_out() {
        assert_eq!(said("`Bad` is not a name for a project"), "… is not a name for a project");
        assert_eq!(said("no ask \"k3f9q2\" here\nand more"), "no ask … here");
        assert!(said(&"x".repeat(400)).chars().count() <= 160);
    }

    #[test]
    fn a_window_is_hours_or_days_or_all() {
        assert_eq!(window("24h").ok().flatten().map(|w| w.0), Some(86_400));
        assert_eq!(window("7d").ok().flatten().map(|w| w.0), Some(7 * 86_400));
        assert!(window("all").is_ok_and(|w| w.is_none()));
        for wrong in ["7", "d", "0d", "-1d", "7w", ""] {
            assert!(window(wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn the_rank_of_what_was_measured_is_the_nearest() {
        let ms: Vec<u64> = (1..=100).collect();
        assert_eq!(rank(&ms, 0.5), Some(50));
        assert_eq!(rank(&ms, 0.95), Some(95));
        assert_eq!(rank(&[7], 0.95), Some(7));
        assert_eq!(rank(&[], 0.5), None);
    }
}
