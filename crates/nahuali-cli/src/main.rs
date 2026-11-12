//! `nahuali`: the verbs for agents and the board for a person.
//!
//! Exit status: 0 when it did what was asked, 1 when the memory refused
//! (and says why), 2 when the command line made no sense.

mod args;
mod env;
mod hook;
mod keeper;
mod live;
mod mcp;
mod more;
mod remote;
mod setup;
mod team;
mod text;

use std::io::IsTerminal;
use std::process::ExitCode;

use args::Args;
use nahuali_core::event::WorkSet;
use nahuali_core::{Mind, NewAsk, Scope, Signature, Status, Sure, Time};
use nahuali_tui::anim::Motion;
use nahuali_tui::theme::ColorMode;
use remote::Remote;

const HELP: &str = "\
nahuali: a working memory for a person and their agents

  nahuali                     the board (at a terminal), or status (anywhere else)
  nahuali status              where this project stands: open asks, the cards, what was decided
  nahuali update WORK …       overwrite a work's card; the first update starts the work
  nahuali ask \"QUESTION\" …    put a question to the person; it stays in sight until closed
  nahuali answer ASK \"TEXT\"   close an ask; it becomes a decision
  nahuali learn \"RULE\" …      record a lesson and the moment it applies
  nahuali check \"DOING\"       say what you are about to do; get the lessons that apply
  nahuali note \"TEXT\" …       keep something worth knowing
  nahuali recall \"ABOUT\"      what is known about something; may be nothing
  nahuali where               where the memory lives, which project this is, who you are
  nahuali export              every event as one line of JSON, to read or keep without nahuali

status
  --all                       every project (nothing private to another project)
  --json                      one JSON object

update WORK
  --line \"TEXT\"               the card in one line: what is true now
  --status moving|not-started|done
  --blocked \"BY WHAT\"         stopped, and by what
  --done \"TEXT\"               something finished (repeatable; adds to the card)
  --next \"TEXT\"               what comes next (repeatable; replaces what the card said)
  --title \"TEXT\"  --goal \"TEXT\"
  --verified \"BY WHAT\"        how this was checked; without it the update is only claimed

ask \"QUESTION\"
  --blocks \"WHAT\"             what cannot go on until it is answered
  --recommends \"ANSWER\"       the answer you would give
  --work WORK                 the work that waits on it
  --to PERSON                 who is asked, when it is not the person you work for

answer ASK \"TEXT\"
  --withdraw                  the ask no longer needs an answer; TEXT says why

learn \"RULE\"
  --moment \"WHEN\"             when it applies, in words: \"before spawning an agent in a worktree\"
  --when \"WORDS\"              words that mark that moment in a command, all of them: \"worktree add\" (repeatable)
  --everywhere                it applies to every project
  --stop                      serious enough to stop the command once, not only to be told
  nahuali lessons             every lesson in force here
  nahuali retire LESSON \"WHY\" it stopped being true

note \"TEXT\"
  --tag WORD                  a word to find it by (repeatable)
  --about WORK                the work it is about
  nahuali forget NOTE \"WHY\"   it went stale or was wrong

recall \"ABOUT\"
  --all                       every project (nothing private to another project)
  --json  --limit N

other tools
  nahuali setup [--hooks]     tell Claude Code and Codex about nahuali; --hooks brings the memory to
                              an agent by itself; --remove takes it out; --dry-run only says
  nahuali setup --meaning     install the model that finds what is said in other words (220 MB, once,
                              runs on this machine; nothing leaves it)
  nahuali mcp                 the same verbs over the Model Context Protocol, on standard input

a team
  nahuali team                where this memory is kept, and who you are on it
  nahuali team init URL       make a SurrealDB server ready to hold a team's memory
  nahuali team add NAME URL   a person, and the passphrase they join with
  nahuali team join URL --name NAME    keep this memory on the team's server (NAHUALI_PASS)
  nahuali team leave          back to the memory on this machine

everywhere
  --project NAME              default: the repository, or this directory
  --as NAME                   which agent is speaking; default: the tool that runs it
  --from NAME                 you pass on what NAME told you
  --private                   not to be offered to work on another project
  --mine                      only the person you work for, and their agents, see it
  --home DIR                  where the memory lives; default: NAHUALI_HOME, or your data directory

The commit, the time and the project are recorded for you.
";

const EVERYWHERE: [&str; 4] = ["project", "as", "from", "home"];

enum Failure {
    /// The command line made no sense.
    Usage(String),
    /// The memory refused.
    Refused(String),
}

impl From<nahuali_core::Error> for Failure {
    fn from(e: nahuali_core::Error) -> Self {
        Self::Refused(e.to_string())
    }
}

fn usage(text: impl Into<String>) -> Failure {
    Failure::Usage(text.into())
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(Failure::Usage(text)) => {
            eprintln!("nahuali: {text}\n         `nahuali help` lists what it takes");
            ExitCode::from(2)
        }
        Err(Failure::Refused(text)) => {
            eprintln!("nahuali: {text}");
            ExitCode::from(1)
        }
    }
}

fn parse(argv: &[String], valued: &[&str], flags: &[&str]) -> Result<Args, Failure> {
    let valued: Vec<&str> = EVERYWHERE.iter().chain(valued).copied().collect();
    let flags: Vec<&str> = ["private", "mine"].iter().chain(flags).copied().collect();
    Args::parse(argv, &valued, &flags).map_err(Failure::Usage)
}

fn open(args: &Args) -> Result<Mind<Remote>, Failure> {
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    let me = me_at(&home);
    Ok(Mind::open_as(Remote::at(home), Some(me))?)
}

/// The person this installation works for: who it is on its team's server,
/// or else whoever owns this machine's memory.
pub(crate) fn me_at(home: &std::path::Path) -> String {
    team::Server::read(home).map_or_else(env::person, |server| server.name)
}

fn me(args: &Args) -> String {
    env::home(args.value("home")).map_or_else(|_| env::person(), |home| me_at(&home))
}

fn sign(args: &Args) -> Result<Signature, Failure> {
    let project = env::project(args.value("project")).map_err(Failure::Usage)?;
    Ok(Signature {
        who: env::actor(args.value("as"), args.value("from")).map_err(Failure::Usage)?,
        at_ms: env::now_ms(),
        entropy: env::entropy(),
        commit: env::commit(&project),
        project,
        private: args.has("private"),
        sure: args.value("verified").map_or(Sure::Claimed, |by| Sure::Verified(by.to_string())),
        person: Some(me(args)),
        mine: args.has("mine"),
    })
}

fn words<const N: usize>(args: &Args, what: &str) -> Result<[String; N], Failure> {
    <[String; N]>::try_from(args.words.clone()).map_err(|_| usage(what))
}

pub(crate) fn run(argv: &[String]) -> Result<String, Failure> {
    let (command, rest) = match argv.first().map(String::as_str) {
        None => (if std::io::stdout().is_terminal() { "board" } else { "status" }, argv),
        Some(first) if first.starts_with("--") && !matches!(first, "--help" | "--version") => {
            (if std::io::stdout().is_terminal() { "board" } else { "status" }, argv)
        }
        Some(first) => (first, &argv[1..]),
    };
    match command {
        "help" | "--help" | "-h" => Ok(HELP.to_string()),
        "--version" | "-V" => Ok(format!("nahuali {}\n", env!("CARGO_PKG_VERSION"))),
        "status" => status(rest),
        "update" => update(rest),
        "ask" => ask(rest),
        "answer" => answer(rest),
        "where" => place(rest),
        "board" => board(rest),
        "export" => export(rest),
        "learn" => more::learn(rest),
        "check" => more::check(rest),
        "lessons" => more::lessons(rest),
        "retire" => more::retire(rest),
        "note" => more::note(rest),
        "recall" => more::recall(rest),
        "forget" => more::forget(rest),
        "hook" => hook::hook(rest),
        "mcp" => mcp::mcp(rest),
        "setup" => setup::setup(rest),
        "team" => team::team(rest),
        "serve" => serve(rest),
        "stop" => stop(rest),
        other => Err(usage(format!("`{other}` is not something nahuali does"))),
    }
}

fn status(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &["all", "json"])?;
    let [] = words(&args, "status takes no words")?;
    let mind = open(&args)?;
    let project = env::project(args.value("project")).map_err(Failure::Usage)?;
    let report = text::Report {
        state: mind.state(),
        scope: if args.has("all") { Scope::AllFrom(&project) } else { Scope::Project(&project) },
        project: &project,
        now: Time::from_millis(env::now_ms()),
        damaged: mind.damaged,
        newer: mind.newer,
        me: &me(&args),
    };
    Ok(if args.has("json") { format!("{}\n", report.json()) } else { report.text() })
}

fn update(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &["line", "status", "blocked", "done", "next", "title", "goal", "verified"], &[])?;
    let [work] =
        words(&args, "update takes the name of one piece of work: nahuali update importer --line \"…\"")?;
    let status = match (args.value("blocked"), args.value("status")) {
        (Some(_), Some(_)) => return Err(usage("say --blocked or --status, not both")),
        (Some(why), None) => Some(Status::Blocked(why.to_string())),
        (None, Some("moving")) => Some(Status::Moving),
        (None, Some("not-started")) => Some(Status::NotStarted),
        (None, Some("done")) => Some(Status::Done),
        (None, Some("blocked")) => return Err(usage("say what blocks it: --blocked \"by what\"")),
        (None, Some(other)) => {
            return Err(usage(format!("`{other}` is not a status: moving, not-started or done")));
        }
        (None, None) => None,
    };
    let next = args.values("next");
    let set = WorkSet {
        work,
        title: args.value("title").map(str::to_string),
        goal: args.value("goal").map(str::to_string),
        status,
        line: args.value("line").map(str::to_string),
        done: args.values("done"),
        next: (!next.is_empty()).then_some(next),
    };
    let mut mind = open(&args)?;
    let work = mind.update(sign(&args)?, set)?;
    Ok(format!("{}\n", text::work_line(mind.state(), &work)))
}

fn ask(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &["blocks", "recommends", "work", "to"], &[])?;
    let [question] = words(&args, "ask takes one question, in quotes: nahuali ask \"Ship behind a flag?\"")?;
    let new = NewAsk {
        question,
        blocks: args.value("blocks").map(str::to_string),
        recommends: args.value("recommends").map(str::to_string),
        work: args.value("work").map(str::to_string),
        to: args.value("to").map(str::to_string),
    };
    let mut mind = open(&args)?;
    let ask = mind.ask(sign(&args)?, new)?;
    Ok(format!(
        "ask:{} · open in {} · it stays on the person's board; `nahuali status` shows the answer when there is one\n",
        ask.id, ask.marks.project
    ))
}

fn answer(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &["withdraw"])?;
    let [ask, said] = words(
        &args,
        "answer takes an ask and the answer, in quotes: nahuali answer k3f9q2 \"Behind a flag\"",
    )?;
    let mut mind = open(&args)?;
    // The ask says which project it belongs to; the directory need not.
    let sig = sign_anywhere(&args)?;
    if args.has("withdraw") {
        let ask = mind.withdraw(sig, &ask, &said)?;
        Ok(format!("ask:{} · withdrawn · {}\n", ask.id, ask.question))
    } else {
        let ask = mind.answer(sig, &ask, &said)?;
        Ok(format!(
            "ask:{} · answered, and kept as a decision in {} · {}\n",
            ask.id, ask.marks.project, ask.question
        ))
    }
}

/// A signature for what does not depend on where it is said from.
fn sign_anywhere(args: &Args) -> Result<Signature, Failure> {
    let project = env::project(args.value("project")).unwrap_or_else(|_| "anywhere".to_string());
    Ok(Signature {
        who: env::actor(args.value("as"), args.value("from")).map_err(Failure::Usage)?,
        at_ms: env::now_ms(),
        entropy: env::entropy(),
        project,
        commit: None,
        private: args.has("private"),
        sure: Sure::Claimed,
        person: Some(me(args)),
        mine: args.has("mine"),
    })
}

fn place(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &[])?;
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    let project = env::project(args.value("project")).map_err(Failure::Usage)?;
    let who = env::actor(args.value("as"), args.value("from")).map_err(Failure::Usage)?;
    let kind = if who.is_person() { "the person" } else { "an agent" };
    let commit = env::commit(&project).unwrap_or_else(|| "none".to_string());
    let meaning = match nahuali_embed::spec(nahuali_embed::DEFAULT_MODEL) {
        Ok(spec) if nahuali_embed::installed(&home.join("models"), spec) => format!("on ({})", spec.id),
        _ => "off: words only. `nahuali setup --meaning` installs the model".to_string(),
    };
    Ok(format!(
        "memory   {}\nproject  {project}\nyou      {} ({kind})\ncommit   {commit}\nmeaning  {meaning}\n",
        home.display(),
        who.line()
    ))
}

fn board(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &["motion", "fps"], &["halfblocks", "256", "truecolor"])?;
    let [] = words(&args, "the board takes no words")?;
    let mut options = nahuali_tui::Options { force_halfblocks: args.has("halfblocks"), ..Default::default() };
    if args.has("256") {
        options.color = ColorMode::Ansi256;
    }
    if args.has("truecolor") {
        options.color = ColorMode::TrueColor;
    }
    if let Some(motion) = args.value("motion") {
        options.motion =
            Motion::parse(motion).ok_or_else(|| usage("--motion takes full, reduced or none"))?;
    }
    if let Some(fps) = args.value("fps") {
        options.fps = fps.parse().map_err(|_| usage("--fps takes a number"))?;
    }
    let mind = open(&args)?;
    let journal = mind.journal().clone();
    let src = live::Live::new(mind, me(&args)).finding(Box::new(move |about| {
        let asked = nahuali_core::words::plain(about).join(" ");
        let keys = if asked.is_empty() {
            Vec::new()
        } else {
            journal.search(&asked, "everything", "", 12).unwrap_or_default()
        };
        let near =
            journal.near(about, "everything", "", None, 5, None).map(|(near, _)| near).unwrap_or_default();
        (keys, near)
    }));
    nahuali_tui::run(&src, options)
        .map_err(|e| Failure::Refused(format!("the terminal could not be drawn: {e}")))?;
    Ok(String::new())
}

/// Every event, as the lines a person can read and another memory can take in.
fn export(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &[])?;
    let [] = words(&args, "export takes no words")?;
    let mind = open(&args)?;
    Ok(mind.events().iter().map(|e| e.line() + "\n").collect())
}

/// Be the keeper of a memory. Commands start one when they need it.
fn serve(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &[])?;
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    keeper::serve(&home).map_err(Failure::Refused)?;
    Ok(String::new())
}

/// Tell the keeper of a memory to leave. The next command starts another.
fn stop(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &[])?;
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    Ok(if Remote::at(home).stop() {
        "the keeper left\n".to_string()
    } else {
        "there was no keeper\n".to_string()
    })
}
