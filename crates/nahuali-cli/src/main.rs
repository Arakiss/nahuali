//! `nahuali`: the verbs for agents and the board for a person.
//!
//! Exit status: 0 when it did what was asked, 1 when the memory refused
//! (and says why), 2 when the command line made no sense.

mod args;
mod env;
mod live;
mod text;

use std::io::IsTerminal;
use std::process::ExitCode;

use args::Args;
use nahuali_core::event::WorkSet;
use nahuali_core::{FileJournal, Mind, NewAsk, Scope, Signature, Status, Sure, Time};
use nahuali_tui::anim::Motion;
use nahuali_tui::theme::ColorMode;

const HELP: &str = "\
nahuali: a working memory for a person and their agents

  nahuali                     the board (at a terminal), or status (anywhere else)
  nahuali status              where this project stands: open asks, the cards, what was decided
  nahuali update WORK …       overwrite a work's card; the first update starts the work
  nahuali ask \"QUESTION\" …    put a question to the person; it stays in sight until closed
  nahuali answer ASK \"TEXT\"   close an ask; it becomes a decision
  nahuali where               where the memory lives, which project this is, who you are

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

answer ASK \"TEXT\"
  --withdraw                  the ask no longer needs an answer; TEXT says why

everywhere
  --project NAME              default: the repository, or this directory
  --as NAME                   which agent is speaking; default: the tool that runs it
  --from NAME                 you pass on what NAME told you
  --private                   not to be offered to work on another project
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
    let flags: Vec<&str> = ["private"].iter().chain(flags).copied().collect();
    Args::parse(argv, &valued, &flags).map_err(Failure::Usage)
}

fn open(args: &Args) -> Result<Mind<FileJournal>, Failure> {
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    Ok(Mind::open(FileJournal::at(home))?)
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
    })
}

fn words<const N: usize>(args: &Args, what: &str) -> Result<[String; N], Failure> {
    <[String; N]>::try_from(args.words.clone()).map_err(|_| usage(what))
}

fn run(argv: &[String]) -> Result<String, Failure> {
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
    let args = parse(argv, &["blocks", "recommends", "work"], &[])?;
    let [question] = words(&args, "ask takes one question, in quotes: nahuali ask \"Ship behind a flag?\"")?;
    let new = NewAsk {
        question,
        blocks: args.value("blocks").map(str::to_string),
        recommends: args.value("recommends").map(str::to_string),
        work: args.value("work").map(str::to_string),
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
    })
}

fn place(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &[])?;
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    let project = env::project(args.value("project")).map_err(Failure::Usage)?;
    let who = env::actor(args.value("as"), args.value("from")).map_err(Failure::Usage)?;
    let kind = if who.is_person() { "the person" } else { "an agent" };
    let commit = env::commit(&project).unwrap_or_else(|| "none".to_string());
    Ok(format!(
        "memory   {}\nproject  {project}\nyou      {} ({kind})\ncommit   {commit}\n",
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
    let src = live::Live::new(open(&args)?, env::person());
    nahuali_tui::run(&src, options)
        .map_err(|e| Failure::Refused(format!("the terminal could not be drawn: {e}")))?;
    Ok(String::new())
}
