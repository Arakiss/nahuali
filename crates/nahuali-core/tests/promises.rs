//! The core held to what `docs/product.md` promises, and to what a log kept
//! in plain files has to survive: several writers, crashes, other versions.

use std::path::PathBuf;

use nahuali_core::event::WorkSet;
use nahuali_core::{
    Actor, Closed, Error, FileJournal, Journal, MemJournal, Mind, NewAsk, Scope, Signature, Standing, Status,
    Sure, Time,
};

const T0: u64 = 1_794_496_800_000;
const MIN: u64 = 60_000;
const DAY: u64 = 86_400_000;

fn agent(minute: u64, project: &str) -> Signature {
    Signature {
        who: Actor::Agent("claude-code".into()),
        at_ms: T0 + minute * MIN,
        entropy: u128::from(minute) * 0x9e37_79b9_7f4a_7c15 + 1,
        project: project.into(),
        commit: Some("abc1234".into()),
        private: false,
        sure: Sure::Claimed,
    }
}

fn ada(minute: u64) -> Signature {
    Signature { who: Actor::Person("ada".into()), commit: None, ..agent(minute, "atlas") }
}

fn card(work: &str, line: &str) -> WorkSet {
    WorkSet { work: work.into(), line: Some(line.into()), ..WorkSet::default() }
}

fn question(text: &str, work: Option<&str>) -> NewAsk {
    NewAsk { question: text.into(), work: work.map(str::to_string), ..NewAsk::default() }
}

fn mind() -> Mind<MemJournal> {
    Mind::open(MemJournal::default()).unwrap()
}

/// A folder of its own under the system's temporary directory.
struct Home(PathBuf);

impl Home {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nahuali-core-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_card_is_overwritten_and_its_past_stays_behind_it() {
    let mut m = mind();
    m.update(agent(0, "atlas"), card("importer", "parsing the old format")).unwrap();
    let set = WorkSet {
        status: Some(Status::Blocked("review".into())),
        done: vec!["parser".into()],
        next: Some(vec!["open the review".into()]),
        ..card("importer", "tests green, review left")
    };
    let work =
        m.update(Signature { sure: Sure::Verified("cargo test".into()), ..agent(5, "atlas") }, set).unwrap();

    assert_eq!(work.line, "tests green, review left", "the card says only what is true now");
    assert_eq!(work.status, Status::Blocked("review".into()));
    assert_eq!(work.done, ["parser"]);
    assert_eq!(work.next, ["open the review"]);
    assert_eq!(
        work.marks.sure,
        Sure::Verified("cargo test".into()),
        "the marks are those of the last update"
    );
    assert_eq!(work.marks.commit.as_deref(), Some("abc1234"));
    let past: Vec<&str> = work.history.iter().map(|c| c.what.as_str()).collect();
    assert_eq!(past, ["moving · parsing the old format", "blocked (review) · tests green, review left"]);
}

#[test]
fn a_field_left_out_keeps_what_it_had() {
    let mut m = mind();
    let first = WorkSet {
        title: Some("The importer".into()),
        goal: Some("Read the old files".into()),
        ..card("importer", "started")
    };
    m.update(agent(0, "atlas"), first).unwrap();
    let work = m
        .update(
            agent(1, "atlas"),
            WorkSet { work: "importer".into(), done: vec!["a".into()], ..WorkSet::default() },
        )
        .unwrap();
    assert_eq!(work.title, "The importer");
    assert_eq!(work.goal.as_deref(), Some("Read the old files"));
    assert_eq!(work.line, "started");
    assert_eq!(work.history.last().unwrap().what, "updated");
}

#[test]
fn an_open_ask_stays_in_sight_until_someone_closes_it() {
    let mut m = mind();
    m.update(agent(0, "atlas"), card("importer", "ready")).unwrap();
    let ask = m.ask(agent(1, "atlas"), question("Ship behind a flag?", Some("importer"))).unwrap();
    let much_later = Time::from_millis(T0 + 400 * DAY);

    assert_eq!(m.state().open_asks(Scope::Everything).len(), 1, "an ask does not expire");
    let work = m.state().work("atlas", "importer").unwrap().clone();
    assert_eq!(
        m.state().standing(&work),
        Standing::Waiting(vec![ask.id.clone()]),
        "the work it holds waits on the person"
    );
    assert!(m.state().works(Scope::Everything, much_later).iter().any(|w| w.slug == "importer"));

    let closed = m.answer(ada(9), &ask.id, "Behind a flag.").unwrap();
    let (how, marks) = closed.closed.unwrap();
    assert_eq!(how, Closed::Answered("Behind a flag.".into()));
    assert_eq!(marks.who, Actor::Person("ada".into()), "a decision says who decided");
    assert!(m.state().open_asks(Scope::Everything).is_empty());
    assert_eq!(m.state().standing(&work), Standing::Moving, "and the work goes back to what its card says");
    assert_eq!(
        m.state().closed_asks(Scope::Project("atlas"), Time::from_millis(T0)).len(),
        1,
        "whoever asked finds the answer"
    );
}

#[test]
fn an_ask_is_closed_once() {
    let mut m = mind();
    let ask = m.ask(agent(0, "atlas"), question("Which logo?", None)).unwrap();
    m.withdraw(agent(1, "atlas"), &ask.id, "the cover has no logo any more").unwrap();
    let again = m.answer(ada(2), &ask.id, "The blue one").unwrap_err();
    assert!(matches!(again, Error::AlreadyClosed { .. }), "{again}");
    assert!(again.to_string().contains("withdrawn by claude-code"), "{again}");
}

#[test]
fn an_ask_is_called_by_a_short_name_or_the_start_of_it() {
    let mut m = mind();
    let a = m.ask(agent(0, "atlas"), question("One?", None)).unwrap();
    let b = m.ask(agent(1, "atlas"), question("Two?", None)).unwrap();
    assert_eq!(a.id.len(), 6);
    assert_ne!(a.id, b.id);
    assert_eq!(m.state().find_ask(&format!("ask:{}", a.id)).unwrap().question, "One?");
    assert_eq!(m.state().find_ask(&a.id.to_uppercase()).unwrap().question, "One?");
    let start = &b.id[..3];
    if !a.id.starts_with(start) {
        assert_eq!(m.state().find_ask(start).unwrap().question, "Two?");
    }
    assert!(matches!(m.answer(ada(2), "zzzzzz", "x").unwrap_err(), Error::NoSuchAsk(_)));
}

#[test]
fn nothing_private_is_offered_to_another_project() {
    let mut m = mind();
    m.update(Signature { private: true, ..agent(0, "atlas") }, card("salaries", "drafted")).unwrap();
    m.update(agent(1, "atlas"), card("importer", "ready")).unwrap();
    m.update(agent(2, "borealis"), card("cover", "two candidates")).unwrap();
    // An ask about private work is private too, whatever its asker said.
    m.ask(agent(3, "atlas"), question("Round up or down?", Some("salaries"))).unwrap();
    let now = Time::from_millis(T0 + 10 * MIN);
    let slugs = |m: &Mind<MemJournal>, scope| {
        m.state().works(scope, now).iter().map(|w| w.slug.clone()).collect::<Vec<_>>()
    };

    let mut from_borealis = slugs(&m, Scope::AllFrom("borealis"));
    from_borealis.sort();
    assert_eq!(from_borealis, ["cover", "importer"]);
    assert!(m.state().open_asks(Scope::AllFrom("borealis")).is_empty());
    assert_eq!(slugs(&m, Scope::Project("borealis")), ["cover"]);
    assert_eq!(slugs(&m, Scope::Project("atlas")).len(), 2, "inside its own project it is there");
    assert_eq!(m.state().open_asks(Scope::Project("atlas")).len(), 1);
    assert_eq!(slugs(&m, Scope::Everything).len(), 3, "and the person sees all of it");

    // Once private, a later update that forgets to say so does not open it.
    m.update(agent(4, "atlas"), card("salaries", "sent")).unwrap();
    assert!(!slugs(&m, Scope::AllFrom("borealis")).contains(&"salaries".to_string()));
}

#[test]
fn finished_work_leaves_the_board_a_day_later() {
    let mut m = mind();
    m.update(agent(0, "atlas"), WorkSet { status: Some(Status::Done), ..card("importer", "shipped") })
        .unwrap();
    let on_board = |at_ms| !m.state().works(Scope::Everything, Time::from_millis(at_ms)).is_empty();
    assert!(on_board(T0 + 60 * MIN));
    assert!(!on_board(T0 + DAY + MIN));
    assert!(m.state().work("atlas", "importer").is_some(), "it is still held");
}

#[test]
fn what_needs_a_person_comes_first() {
    let mut m = mind();
    m.update(agent(0, "atlas"), card("a-moving", "going")).unwrap();
    m.update(agent(1, "atlas"), WorkSet { status: Some(Status::NotStarted), ..card("b-idle", "later") })
        .unwrap();
    m.update(
        agent(2, "atlas"),
        WorkSet { status: Some(Status::Blocked("ci".into())), ..card("c-blocked", "red") },
    )
    .unwrap();
    m.update(agent(3, "atlas"), card("d-waiting", "needs a call")).unwrap();
    m.ask(agent(4, "atlas"), question("Which?", Some("d-waiting"))).unwrap();
    let order: Vec<&str> = m
        .state()
        .works(Scope::Everything, Time::from_millis(T0 + 5 * MIN))
        .iter()
        .map(|w| w.slug.as_str())
        .collect();
    assert_eq!(order, ["d-waiting", "c-blocked", "a-moving", "b-idle"]);
}

#[test]
fn words_are_required_where_words_are_the_point() {
    let mut m = mind();
    assert!(matches!(m.ask(agent(0, "atlas"), question("   ", None)).unwrap_err(), Error::Empty(_)));
    assert!(matches!(m.update(agent(0, "atlas"), card("Importer", "x")).unwrap_err(), Error::BadName { .. }));
    assert!(matches!(m.update(agent(0, "Atlas"), card("importer", "x")).unwrap_err(), Error::BadName { .. }));
    let long = "x".repeat(281);
    assert!(matches!(
        m.update(agent(0, "atlas"), card("importer", &long)).unwrap_err(),
        Error::TooLong { .. }
    ));
    assert!(matches!(
        m.ask(agent(0, "atlas"), question("Which?", Some("ghost"))).unwrap_err(),
        Error::NoSuchWork(_)
    ));
    m.update(agent(0, "atlas"), card("importer", "one\n  line")).unwrap();
    assert_eq!(m.state().work("atlas", "importer").unwrap().line, "one line");
    let nothing = WorkSet { work: "importer".into(), ..WorkSet::default() };
    assert!(matches!(m.update(agent(1, "atlas"), nothing).unwrap_err(), Error::Nothing));
    assert_eq!(m.events().len(), 1, "a refused line is never written");
}

#[test]
fn the_same_lines_in_any_order_give_the_same_state() {
    let mut m = mind();
    m.update(agent(0, "atlas"), card("importer", "one")).unwrap();
    let ask = m.ask(agent(1, "atlas"), question("Flag?", Some("importer"))).unwrap();
    m.update(agent(2, "atlas"), card("importer", "two")).unwrap();
    m.answer(ada(3), &ask.id, "Yes").unwrap();
    m.update(agent(4, "borealis"), card("cover", "three")).unwrap();

    let lines = m.journal().0.clone();
    let mut reversed = lines.clone();
    reversed.reverse();
    let mut doubled = lines.clone();
    doubled.extend(lines.iter().cloned());
    for other in [reversed, doubled] {
        let other = Mind::open(MemJournal(other)).unwrap();
        assert_eq!(other.state(), m.state(), "copies merged twice or read backwards change nothing");
    }
}

#[test]
fn two_copies_that_closed_the_same_ask_agree_on_the_first() {
    let mut shared = mind();
    let ask = shared.ask(agent(0, "atlas"), question("Flag?", None)).unwrap();
    let lines = shared.journal().0.clone();

    let mut laptop = Mind::open(MemJournal(lines.clone())).unwrap();
    let mut server = Mind::open(MemJournal(lines)).unwrap();
    laptop.answer(ada(5), &ask.id, "Yes").unwrap();
    server.withdraw(agent(9, "atlas"), &ask.id, "no longer needed").unwrap();

    let mut merged = laptop.journal().0.clone();
    merged.extend(server.journal().0.iter().cloned());
    let merged = Mind::open(MemJournal(merged)).unwrap();
    let (how, _) = merged.state().ask(&ask.id).unwrap().closed.clone().unwrap();
    assert_eq!(how, Closed::Answered("Yes".into()), "the earlier close stands on both machines");
}

#[test]
fn a_log_with_lines_it_cannot_read_is_still_a_log() {
    let mut m = mind();
    m.update(agent(0, "atlas"), card("importer", "one")).unwrap();
    let mut lines = m.journal().0.clone();
    lines.push(r#"{"v":1,"id":"01","at":"2026-11-1"#.into());
    lines.push(r#"{"v":7,"shape":"unknown"}"#.into());
    lines.push(
        r#"{"v":1,"id":"01ZZ","at":"2026-11-12T16:00:00Z","by":{"agent":"x"},"project":"atlas","what":{"dream":{}}}"#.into(),
    );
    let m = Mind::open(MemJournal(lines)).unwrap();
    assert_eq!((m.damaged, m.newer, m.state().unknown), (1, 1, 1));
    assert_eq!(m.state().work("atlas", "importer").unwrap().line, "one");
}

#[test]
fn files_keep_what_was_written_and_say_what_they_are() {
    let home = Home::new("files");
    let mut m = Mind::open(FileJournal::at(&home.0)).unwrap();
    assert!(m.state().is_empty(), "a memory that does not exist yet is an empty one");
    m.update(agent(0, "atlas"), card("importer", "one")).unwrap();
    m.ask(agent(1, "atlas"), question("Flag?", Some("importer"))).unwrap();

    let again = Mind::open(FileJournal::at(&home.0)).unwrap();
    assert_eq!(again.state(), m.state());
    assert!(
        std::fs::read_to_string(home.0.join("README.txt")).unwrap().contains("To read it without Nahuali")
    );
    let logs: Vec<_> = std::fs::read_dir(home.0.join("log")).unwrap().collect();
    assert_eq!(logs.len(), 1, "one installation writes one file");
}

#[test]
fn a_line_torn_by_a_crash_spoils_only_itself() {
    let home = Home::new("torn");
    let mut journal = FileJournal::at(&home.0);
    let mut m = Mind::open(journal.clone()).unwrap();
    m.update(agent(0, "atlas"), card("importer", "one")).unwrap();
    let log = std::fs::read_dir(home.0.join("log")).unwrap().next().unwrap().unwrap().path();
    // Power is lost half way through a line: no newline at the end.
    let mut bytes = std::fs::read(&log).unwrap();
    bytes.extend_from_slice(br#"{"v":1,"id":"01M","at":"2026-11-12T15:2"#);
    std::fs::write(&log, bytes).unwrap();

    let mut m = Mind::open(journal.clone()).unwrap();
    assert_eq!(m.damaged, 1);
    m.update(agent(1, "atlas"), card("importer", "two")).unwrap();
    let m = Mind::open(journal.clone()).unwrap();
    assert_eq!(m.damaged, 1, "the torn line is still the only one lost");
    assert_eq!(m.state().work("atlas", "importer").unwrap().line, "two");
    assert_eq!(journal.read().unwrap().len(), 3);
    journal.append("plain").unwrap();
    assert_eq!(journal.read().unwrap().len(), 4);
}

#[test]
fn many_writers_at_once_lose_nothing() {
    let home = Home::new("many");
    let threads: Vec<_> = (0..8u64)
        .map(|t| {
            let path = home.0.clone();
            std::thread::spawn(move || {
                for i in 0..25u64 {
                    let mut m = Mind::open(FileJournal::at(&path)).unwrap();
                    let n = t * 100 + i;
                    m.update(
                        agent(n, "atlas"),
                        card(&format!("work-{n}"), "a line long enough to be worth tearing"),
                    )
                    .unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let m = Mind::open(FileJournal::at(&home.0)).unwrap();
    assert_eq!(m.damaged, 0);
    assert_eq!(m.events().len(), 200);
    assert_eq!(m.state().works(Scope::Everything, Time::from_millis(T0)).len(), 200);
    assert_eq!(
        std::fs::read_dir(home.0.join("log")).unwrap().count(),
        1,
        "one machine, one file, however many agents"
    );
}

#[test]
fn two_machines_share_a_memory_by_copying_files() {
    let (laptop, server) = (Home::new("laptop"), Home::new("server"));
    let mut a = Mind::open(FileJournal::at(&laptop.0)).unwrap();
    let mut b = Mind::open(FileJournal::at(&server.0)).unwrap();
    a.update(agent(0, "atlas"), card("importer", "from the laptop")).unwrap();
    b.update(agent(1, "borealis"), card("cover", "from the server")).unwrap();
    let ask = b.ask(agent(2, "borealis"), question("Which logo?", Some("cover"))).unwrap();

    // Copy log/ both ways, as any file-copying tool would.
    for (from, to) in [(&laptop.0, &server.0), (&server.0, &laptop.0)] {
        for entry in std::fs::read_dir(from.join("log")).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), to.join("log").join(entry.file_name())).unwrap();
        }
    }
    let mut a = Mind::open(FileJournal::at(&laptop.0)).unwrap();
    let b = Mind::open(FileJournal::at(&server.0)).unwrap();
    assert_eq!(a.state(), b.state());
    assert_eq!(a.state().works(Scope::Everything, Time::from_millis(T0)).len(), 2);

    // The person answers on the laptop, in the laptop's own file.
    a.answer(ada(3), &ask.id, "The blue one").unwrap();
    assert_eq!(std::fs::read_dir(laptop.0.join("log")).unwrap().count(), 2);
}
