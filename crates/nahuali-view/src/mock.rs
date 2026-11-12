//! A deterministic, invented memory for demos, tests and screenshots. The
//! content is plausible but fictional: its person is a made-up `ada`, and
//! the projects and people in it do not exist.

use std::cell::RefCell;

use crate::date::{Date, Stamp};
use crate::source::{
    Ask, Board, Change, Found, Lesson, Marks, Person, Sure, ViewSource, Who, Work, WorkStatus,
};

/// The mock's own person.
pub const OWNER: &str = "ada";

/// The mock's present: a Thursday morning.
const NOW: Stamp = Stamp::new(Date::new(2026, 11, 12), 10, 20);

fn nov(day: u8, hour: u8, minute: u8) -> Stamp {
    Stamp::new(Date::new(2026, 11, day), hour, minute)
}

fn marks(who: Who, sure: Sure, as_of: Stamp, project: &str) -> Marks {
    Marks { who, sure, as_of, commit: None, project: project.into(), private: false }
}

fn agent(name: &str) -> Who {
    Who::Agent(name.into())
}

fn verified(by: &str) -> Sure {
    Sure::Verified(by.into())
}

fn asks() -> Vec<Ask> {
    vec![
        Ask {
            id: "ask:importer-flag".into(),
            question: "Ship the importer behind a flag, or wait for the audit?".into(),
            blocks: Some("release 0.4".into()),
            recommends: Some("behind a flag".into()),
            marks: marks(agent("claude-code"), verified("the release checklist"), nov(10, 9, 5), "atlas"),
        },
        Ask {
            id: "ask:cover-logo".into(),
            question: "Which of the two logos goes on the cover?".into(),
            blocks: None,
            recommends: None,
            marks: marks(agent("codex"), Sure::Claimed, nov(12, 8, 40), "borealis"),
        },
    ]
}

fn works() -> Vec<Work> {
    vec![
        Work {
            id: "work:importer".into(),
            title: "importer".into(),
            status: WorkStatus::Moving,
            line: "tests green, review left".into(),
            done: vec!["reads the three export formats".into(), "refuses a file it cannot verify".into()],
            next: vec!["review by a second agent".into(), "the flag, once it is decided".into()],
            marks: Marks {
                commit: Some("4c1e9a2".into()),
                ..marks(agent("claude-code"), verified("cargo test, 212 passed"), nov(12, 10, 8), "atlas")
            },
        },
        Work {
            id: "work:cover".into(),
            title: "cover".into(),
            status: WorkStatus::Waiting,
            line: "two candidates drawn".into(),
            done: vec!["two logos, in the brand colours".into()],
            next: vec!["the chosen one at print size".into()],
            marks: marks(agent("codex"), Sure::Claimed, nov(12, 7, 15), "borealis"),
        },
        Work {
            id: "work:pricing-page".into(),
            title: "pricing page".into(),
            status: WorkStatus::NotStarted,
            line: "waits for the importer".into(),
            done: Vec::new(),
            next: vec!["three tiers from the notes of the last call".into()],
            marks: marks(Who::Person(OWNER.into()), verified("said in person"), nov(11, 16, 30), "cobalt"),
        },
    ]
}

fn lessons() -> Vec<Lesson> {
    vec![
        Lesson {
            id: "lesson:worktree-base".into(),
            rule: "Agent worktrees start from the default branch".into(),
            moment: "before spawning an agent in its own worktree".into(),
            marks: marks(
                agent("claude-code"),
                verified("git merge-base, 10 commits behind"),
                nov(11, 15, 25),
                "atlas",
            ),
        },
        Lesson {
            id: "lesson:shared-target".into(),
            rule: "Nine build directories filled the disk: share one".into(),
            moment: "before building in more than two worktrees".into(),
            marks: marks(agent("claude-code"), verified("df, 175 MB free"), nov(11, 17, 40), "atlas"),
        },
        Lesson {
            id: "lesson:logo-sizes".into(),
            rule: "The printer rejects logos under 300 dpi".into(),
            moment: "before sending artwork to print".into(),
            marks: marks(Who::Reported("the print shop".into()), Sure::Claimed, nov(3, 11, 0), "borealis"),
        },
    ]
}

/// The invented memory. Answering an ask removes it from the board.
#[derive(Debug, Default)]
pub struct MockSource {
    empty: bool,
    answered: RefCell<Vec<(String, String)>>,
}

impl MockSource {
    pub fn new() -> Self {
        Self::default()
    }

    /// A memory that holds nothing yet.
    pub fn empty() -> Self {
        Self { empty: true, ..Self::default() }
    }

    /// The answers recorded so far, as `(ask, text)`.
    pub fn answers(&self) -> Vec<(String, String)> {
        self.answered.borrow().clone()
    }

    fn is_answered(&self, ask: &str) -> bool {
        self.answered.borrow().iter().any(|(id, _)| id == ask)
    }
}

impl ViewSource for MockSource {
    fn person(&self) -> Person {
        Person { name: OWNER.into() }
    }

    fn now(&self) -> Stamp {
        NOW
    }

    fn board(&self) -> Board {
        if self.empty {
            return Board::default();
        }
        let asks: Vec<Ask> = asks().into_iter().filter(|a| !self.is_answered(&a.id)).collect();
        let mut works = works();
        // The cover waits on the logo: once that is answered, it moves.
        if self.is_answered("ask:cover-logo")
            && let Some(cover) = works.iter_mut().find(|w| w.id == "work:cover")
        {
            cover.status = WorkStatus::Moving;
        }
        let week_ago = NOW.date.add_days(-7);
        Board {
            asks,
            works,
            lessons: lessons().into_iter().filter(|l| l.marks.as_of.date > week_ago).collect(),
        }
    }

    fn lessons(&self) -> Vec<Lesson> {
        if self.empty { Vec::new() } else { lessons() }
    }

    fn history(&self, id: &str) -> Vec<Change> {
        let line = |at: Stamp, by: &str, what: &str| Change { at, by: by.into(), what: what.into() };
        match id {
            "work:importer" => vec![
                line(nov(12, 10, 8), "claude-code", "tests green, review left"),
                line(nov(12, 9, 2), "claude-code", "refuses a file it cannot verify"),
                line(nov(11, 18, 30), "claude-code", "reads the three export formats"),
                line(nov(10, 9, 0), OWNER, "started"),
            ],
            "ask:importer-flag" => vec![line(nov(10, 9, 5), "claude-code", "asked")],
            "ask:cover-logo" => vec![line(nov(12, 8, 40), "codex", "asked")],
            "work:cover" => vec![
                line(nov(12, 7, 15), "codex", "two candidates drawn"),
                line(nov(11, 12, 0), OWNER, "started"),
            ],
            "lesson:worktree-base" => {
                vec![line(nov(11, 15, 25), "claude-code", "learned after 21 files were edited on old code")]
            }
            _ => Vec::new(),
        }
    }

    /// What holds every word asked for, among what the board and the
    /// lessons show; and, for "release", one thing only alike in meaning.
    fn recall(&self, about: &str) -> Vec<Found> {
        let words: Vec<String> = about.to_lowercase().split_whitespace().map(str::to_string).collect();
        let holds = |text: &str| !words.is_empty() && words.iter().all(|w| text.to_lowercase().contains(w));
        let board = self.board();
        let mut found = Vec::new();
        for a in &board.asks {
            if holds(&format!("{} {}", a.question, a.blocks.as_deref().unwrap_or_default())) {
                let detail = a.recommends.iter().map(|r| format!("recommends {r}")).collect();
                found.push(Found {
                    id: a.id.clone(),
                    kind: "ask".into(),
                    title: a.question.clone(),
                    detail,
                    marks: a.marks.clone(),
                    alike: None,
                });
            }
        }
        for w in &board.works {
            if holds(&format!("{} {}", w.title, w.line)) {
                found.push(Found {
                    id: w.id.clone(),
                    kind: "work".into(),
                    title: w.title.clone(),
                    detail: vec![format!("{} · {}", w.status.word(), w.line)],
                    marks: w.marks.clone(),
                    alike: None,
                });
            }
        }
        for l in self.lessons() {
            if holds(&format!("{} {}", l.rule, l.moment)) {
                found.push(Found {
                    id: l.id.clone(),
                    kind: "lesson".into(),
                    title: l.rule.clone(),
                    detail: vec![format!("when: {}", l.moment)],
                    marks: l.marks.clone(),
                    alike: None,
                });
            }
        }
        if about.to_lowercase().contains("release")
            && let Some(w) = board.works.first().filter(|w| !found.iter().any(|f| f.id == w.id))
        {
            found.push(Found {
                id: w.id.clone(),
                kind: "work".into(),
                title: w.title.clone(),
                detail: vec![format!("{} · {}", w.status.word(), w.line)],
                marks: w.marks.clone(),
                alike: Some(71),
            });
        }
        found
    }

    fn answer(&self, ask: &str, text: &str) -> Result<(), String> {
        if self.is_answered(ask) || !asks().iter().any(|a| a.id == ask) {
            return Err(format!("{ask} is not open"));
        }
        self.answered.borrow_mut().push((ask.to_string(), text.to_string()));
        Ok(())
    }
}
