//! The board on real memory: what the screens ask, answered from the log.
//! The person looking at their own memory sees all of it.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use chrono::{Datelike, Local, TimeZone, Timelike};
use nahuali_core::{Actor, Journal, Marks, Mind, Scope, Signature, Standing, Sure, Time};
use nahuali_view::date::{Date, Stamp};
use nahuali_view::source::{self as view, ViewSource};

use crate::env;

/// How stale the board may be: agents write while the person looks.
const FRESH: Duration = Duration::from_secs(1);

/// What a database answers about a question: the keys of what holds its
/// words, and the keys of what is only close in meaning with how alike.
pub type Finder = Box<dyn Fn(&str) -> (Vec<String>, Vec<(String, f64)>)>;

pub struct Live<J> {
    finder: Option<Finder>,
    mind: RefCell<Mind<J>>,
    read: Cell<Instant>,
    person: String,
}

impl<J: Journal> Live<J> {
    pub fn new(mind: Mind<J>, person: String) -> Self {
        Self { finder: None, mind: RefCell::new(mind), read: Cell::new(Instant::now()), person }
    }

    /// Ask a database instead of looking through the events here.
    pub fn finding(mut self, finder: Finder) -> Self {
        self.finder = Some(finder);
        self
    }

    fn refresh(&self) {
        if self.read.get().elapsed() >= FRESH {
            // A read that fails keeps the last board; the next one tries again.
            let _ = self.mind.borrow_mut().reload();
            self.read.set(Instant::now());
        }
    }
}

/// A moment as the wall clock of this machine shows it.
fn local(at: Time) -> Stamp {
    match Local.timestamp_opt(at.0 as i64, 0).single() {
        Some(t) => {
            Stamp::new(Date::new(t.year(), t.month() as u8, t.day() as u8), t.hour() as u8, t.minute() as u8)
        }
        None => Stamp::new(Date::new(1970, 1, 1), 0, 0),
    }
}

fn who(actor: &Actor) -> view::Who {
    match actor {
        Actor::Person(name) => view::Who::Person(name.clone()),
        Actor::Agent(name) => view::Who::Agent(name.clone()),
        Actor::Reported { by, from } => view::Who::Reported(format!("{by}, from {from}")),
    }
}

fn marks(m: &Marks) -> view::Marks {
    view::Marks {
        who: who(&m.who),
        sure: match &m.sure {
            Sure::Verified(by) => view::Sure::Verified(by.clone()),
            Sure::Claimed => view::Sure::Claimed,
        },
        as_of: local(m.at),
        commit: m.commit.clone(),
        project: m.project.clone(),
        private: m.private,
    }
}

fn changes(past: &[nahuali_core::Change]) -> Vec<view::Change> {
    past.iter()
        .rev()
        .map(|c| view::Change { at: local(c.at), by: c.by.line(), what: c.what.clone() })
        .collect()
}

fn lesson(l: &nahuali_core::Lesson) -> view::Lesson {
    view::Lesson {
        id: format!("lesson:{}", l.id),
        rule: l.rule.clone(),
        moment: l.moment.clone(),
        marks: marks(&l.marks),
    }
}

impl<J: Journal> ViewSource for Live<J> {
    fn person(&self) -> view::Person {
        view::Person { name: self.person.clone() }
    }

    fn now(&self) -> Stamp {
        local(Time::from_millis(env::now_ms()))
    }

    fn board(&self) -> view::Board {
        self.refresh();
        let mind = self.mind.borrow();
        let state = mind.state();
        let asks = state
            .open_asks(Scope::Everything)
            .into_iter()
            .map(|a| view::Ask {
                id: format!("ask:{}", a.id),
                question: a.question.clone(),
                blocks: a.blocks.clone().or_else(|| a.work.clone()),
                recommends: a.recommends.clone(),
                marks: marks(&a.marks),
            })
            .collect();
        let works = state
            .works(Scope::Everything, Time::from_millis(env::now_ms()))
            .into_iter()
            .map(|w| view::Work {
                id: format!("work:{}/{}", w.marks.project, w.slug),
                title: w.title.clone(),
                status: match state.standing(w) {
                    Standing::Moving => view::WorkStatus::Moving,
                    Standing::Blocked(why) => view::WorkStatus::Blocked(why),
                    Standing::Waiting(_) => view::WorkStatus::Waiting,
                    Standing::NotStarted => view::WorkStatus::NotStarted,
                    Standing::Done => view::WorkStatus::Done,
                },
                line: w.line.clone(),
                done: w.done.clone(),
                next: w.next.clone(),
                marks: marks(&w.marks),
            })
            .collect();
        // Lessons of the last seven days.
        let week_ago = Time(Time::from_millis(env::now_ms()).0.saturating_sub(7 * 86_400));
        let lessons = state
            .lessons(Scope::Everything)
            .into_iter()
            .filter(|l| l.marks.at >= week_ago)
            .map(lesson)
            .collect();
        view::Board { asks, works, lessons }
    }

    fn lessons(&self) -> Vec<view::Lesson> {
        self.refresh();
        self.mind.borrow().state().lessons(Scope::Everything).into_iter().map(lesson).collect()
    }

    fn recall(&self, about: &str) -> Vec<view::Found> {
        self.refresh();
        let mind = self.mind.borrow();
        let state = mind.state();
        let found = |t: nahuali_core::Thing, alike: Option<f64>| view::Found {
            id: t.key.clone(),
            kind: t.kind.to_string(),
            title: t.title.clone(),
            detail: t.detail.clone(),
            marks: marks(&t.marks),
            alike: alike.map(|a| (a * 100.0).round().clamp(0.0, 99.0) as u8),
        };
        // The person looking at their own memory sees all of it.
        let Some(finder) = &self.finder else {
            return state.recall(Scope::Everything, about).into_iter().map(|t| found(t, None)).collect();
        };
        let (keys, near) = finder(about);
        let mut held: std::collections::HashMap<String, nahuali_core::Thing> =
            state.things().into_iter().map(|t| (t.key.clone(), t)).collect();
        let mut all: Vec<view::Found> =
            keys.iter().filter_map(|k| held.remove(k)).map(|t| found(t, None)).collect();
        all.extend(
            near.into_iter().filter_map(|(key, alike)| held.remove(&key).map(|t| found(t, Some(alike)))),
        );
        all
    }

    fn history(&self, id: &str) -> Vec<view::Change> {
        let mind = self.mind.borrow();
        let state = mind.state();
        if let Some(name) = id.strip_prefix("decision:") {
            return state.ask(name).map(|a| changes(&state.ask_history(a))).unwrap_or_default();
        }
        if let Some(name) = id.strip_prefix("ask:") {
            return state.ask(name).map(|a| changes(&state.ask_history(a))).unwrap_or_default();
        }
        if let Some(name) = id.strip_prefix("lesson:") {
            return state.lesson(name).map(|l| changes(&state.lesson_history(l))).unwrap_or_default();
        }
        id.strip_prefix("work:")
            .and_then(|rest| rest.split_once('/'))
            .and_then(|(project, slug)| state.work(project, slug))
            .map(|w| changes(&w.history))
            .unwrap_or_default()
    }

    fn answer(&self, ask: &str, text: &str) -> Result<(), String> {
        let mut mind = self.mind.borrow_mut();
        mind.reload().map_err(|e| e.to_string())?;
        let sig = Signature {
            who: Actor::Person(self.person.clone()),
            at_ms: env::now_ms(),
            entropy: env::entropy(),
            // The ask's own project and reach are kept by the core.
            project: "person".to_string(),
            commit: None,
            private: false,
            sure: Sure::Claimed,
        };
        mind.answer(sig, ask, text).map(|_| ()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use nahuali_core::event::WorkSet;
    use nahuali_core::{FileJournal, NewAsk};

    use super::*;

    /// One frame of the real interface, as text.
    fn draw(live: &Live<FileJournal>, w: u16, h: u16) -> String {
        use nahuali_tui::theme::{ColorMode, Theme};
        let theme = Theme::new(ColorMode::TrueColor);
        let sprites = nahuali_tui::mascot::Sprites::for_tests(ColorMode::TrueColor, theme.bg());
        let mut app = nahuali_tui::App::new(live, nahuali_view::UiState::new(false), sprites, theme);
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| app.render(frame.area(), frame.buffer_mut())).unwrap();
        let buf = terminal.backend().buffer();
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end().to_string()
                    + "\n"
            })
            .collect()
    }

    #[test]
    fn the_board_shows_real_work_and_answers_from_it() {
        let home = std::env::temp_dir().join(format!("nahuali-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let agent = |n: u64| Signature {
            who: Actor::Agent("builder".into()),
            at_ms: env::now_ms() - 600_000 + n,
            entropy: u128::from(n) + 1,
            project: "atlas".into(),
            commit: Some("abc1234".into()),
            private: false,
            sure: Sure::Verified("cargo test".into()),
        };
        let mut mind = Mind::open(FileJournal::at(&home)).unwrap();
        mind.update(
            agent(0),
            WorkSet { work: "importer".into(), line: Some("tests green".into()), ..WorkSet::default() },
        )
        .unwrap();
        let ask = mind
            .ask(
                agent(1),
                NewAsk {
                    question: "Ship behind a flag?".into(),
                    work: Some("importer".into()),
                    ..NewAsk::default()
                },
            )
            .unwrap();

        let live = Live::new(Mind::open(FileJournal::at(&home)).unwrap(), "ada".into());
        let screen = draw(&live, 110, 30);
        for said in [
            "Waiting on you · 1",
            "Ship behind a flag?",
            "In motion · 1",
            "importer",
            "waiting on you",
            "all verified",
        ] {
            assert!(screen.contains(said), "the screen does not say `{said}`:\n{screen}");
        }
        let board = live.board();
        assert_eq!(board.asks.len(), 1);
        assert_eq!(board.asks[0].id, format!("ask:{}", ask.id));
        assert_eq!(
            board.asks[0].blocks.as_deref(),
            Some("importer"),
            "with nothing else said, it blocks the work it holds"
        );
        assert_eq!(board.works[0].status, view::WorkStatus::Waiting);
        assert_eq!(board.works[0].marks.who, view::Who::Agent("builder".into()));
        assert!(board.works[0].marks.sure.is_verified());
        assert_eq!(live.history("work:atlas/importer").len(), 1);

        let found = live.recall("ship flag");
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].kind.as_str(), found[0].alike), ("ask", None));
        assert!(live.recall("kubernetes").is_empty(), "an empty answer is a correct answer");

        live.answer(&board.asks[0].id, "Behind a flag.").unwrap();
        let board = live.board();
        assert!(board.asks.is_empty(), "the answer is on the board at once, not a second later");
        assert_eq!(board.works[0].status, view::WorkStatus::Moving);
        let past = live.history(&format!("ask:{}", ask.id));
        assert_eq!(past[0].by, "ada", "newest first, and said by the person");
        assert!(past[0].what.starts_with("answered"));
        assert!(live.answer(&format!("ask:{}", ask.id), "again").is_err());
        let decided = live.recall("ship flag");
        assert_eq!(decided[0].kind, "decision", "the answered ask is found as a decision");
        assert_eq!(live.history(&decided[0].id).len(), 2, "with its past: asked, then answered");
        let _ = std::fs::remove_dir_all(&home);
    }
}
