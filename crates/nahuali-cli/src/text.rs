//! What `status` says: where things stand, in as few lines as say it, with
//! who said each thing, how sure they were and as of when. Written for an
//! agent to read at the start of its work, and short enough for a person.

use nahuali_core::{Ask, Closed, Marks, Scope, Standing, State, Time, Work};
use serde_json::{Value, json};

/// Decisions stay in `status` this long, so whoever asked finds the answer.
pub const DECIDED_FOR: u64 = 7 * 86_400;

pub struct Report<'a> {
    pub state: &'a State,
    pub scope: Scope<'a>,
    pub project: &'a str,
    pub now: Time,
    /// Lines of the log nobody could read, and lines from a newer version.
    pub damaged: usize,
    pub newer: usize,
}

impl Report<'_> {
    fn wide(&self) -> bool {
        !matches!(self.scope, Scope::Project(_))
    }

    fn asks(&self) -> Vec<&Ask> {
        self.state.open_asks(self.scope)
    }

    fn works(&self) -> Vec<&Work> {
        self.state.works(self.scope, self.now)
    }

    fn decided(&self) -> Vec<&Ask> {
        self.state.closed_asks(self.scope, Time(self.now.0.saturating_sub(DECIDED_FOR)))
    }

    /// `atlas/` in front of a name when more than one project is in sight.
    fn place(&self, marks: &Marks) -> String {
        if self.wide() { format!("{}/", marks.project) } else { String::new() }
    }

    /// Who and when, for what is a saying and not a claim about the world:
    /// an ask, an answer. An agent that decides is named as one.
    fn said(&self, marks: &Marks) -> String {
        let who =
            if marks.who.is_person() { marks.who.line() } else { format!("{}, an agent", marks.who.line()) };
        let private = if marks.private { " · private" } else { "" };
        format!("{who} · {}{private}", marks.at.ago(self.now))
    }

    fn marks(&self, marks: &Marks) -> String {
        let mut parts = vec![marks.who.line(), marks.sure.line(), marks.at.ago(self.now)];
        if let Some(commit) = &marks.commit {
            parts.push(format!("commit {commit}"));
        }
        if marks.private {
            parts.push("private".to_string());
        }
        parts.join(" · ")
    }

    pub fn text(&self) -> String {
        let (asks, works, decided) = (self.asks(), self.works(), self.decided());
        let whose = if self.wide() { "every project" } else { self.project };
        let mut out = String::new();
        if asks.is_empty() && works.is_empty() && decided.is_empty() {
            out.push_str(&format!("{whose} · nothing held yet\n"));
        } else {
            let count = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
            out.push_str(&format!(
                "{whose} · {} waiting on the person · {}\n",
                asks.len(),
                count(works.len(), "piece of work", "pieces of work")
            ));
        }
        if !asks.is_empty() {
            out.push_str("\nWAITING ON THE PERSON\n");
            for ask in &asks {
                let name = format!("{}ask:{}", self.place(&ask.marks), ask.id);
                let pad = " ".repeat(name.chars().count() + 4);
                out.push_str(&format!("  {name}  {}\n", ask.question));
                let mut about = Vec::new();
                if let Some(blocks) = &ask.blocks {
                    about.push(format!("blocks {blocks}"));
                }
                if let Some(work) = &ask.work {
                    about.push(format!("holds {work}"));
                }
                if let Some(recommends) = &ask.recommends {
                    about.push(format!("recommends {recommends}"));
                }
                if !about.is_empty() {
                    out.push_str(&format!("{pad}{}\n", about.join(" · ")));
                }
                out.push_str(&format!("{pad}asked by {}\n", self.said(&ask.marks)));
            }
        }
        if !works.is_empty() {
            out.push_str("\nWORK\n");
            for work in &works {
                let name = format!("{}{}", self.place(&work.marks), work.slug);
                let pad = " ".repeat(name.chars().count() + 4);
                let standing = match self.state.standing(work) {
                    Standing::Blocked(why) => format!("blocked by {why}"),
                    Standing::Waiting(asks) => format!("waiting on ask:{}", asks.join(", ask:")),
                    other => other.word().to_string(),
                };
                let line = if work.line.is_empty() { String::new() } else { format!(" · {}", work.line) };
                out.push_str(&format!("  {name}  {standing}{line}\n"));
                if let Some(goal) = &work.goal {
                    out.push_str(&format!("{pad}goal: {goal}\n"));
                }
                if let Some(last) = work.done.last() {
                    let more = if work.done.len() > 1 {
                        format!(" (and {} before)", work.done.len() - 1)
                    } else {
                        String::new()
                    };
                    out.push_str(&format!("{pad}done: {last}{more}\n"));
                }
                if !work.next.is_empty() {
                    out.push_str(&format!("{pad}next: {}\n", work.next.join("; ")));
                }
                out.push_str(&format!("{pad}{}\n", self.marks(&work.marks)));
            }
        }
        if !decided.is_empty() {
            out.push_str("\nDECIDED IN THE LAST 7 DAYS\n");
            for ask in &decided {
                let Some((how, marks)) = &ask.closed else { continue };
                let name = format!("{}ask:{}", self.place(&ask.marks), ask.id);
                let pad = " ".repeat(name.chars().count() + 4);
                out.push_str(&format!("  {name}  {}\n", ask.question));
                match how {
                    Closed::Answered(text) => {
                        out.push_str(&format!("{pad}answer: {text}\n{pad}decided by {}\n", self.said(marks)))
                    }
                    Closed::Withdrawn(why) => {
                        out.push_str(&format!("{pad}withdrawn: {why}\n{pad}by {}\n", self.said(marks)))
                    }
                }
            }
        }
        if self.damaged > 0 {
            out.push_str(&format!(
                "\nnote: {} of the log could not be read and was left out\n",
                lines(self.damaged)
            ));
        }
        if self.newer > 0 {
            out.push_str(&format!(
                "\nnote: {} of the log was written by a newer nahuali and was left out\n",
                lines(self.newer)
            ));
        }
        out
    }

    pub fn json(&self) -> Value {
        let marks = |m: &Marks| {
            json!({
                "who": m.who,
                "sure": m.sure,
                "at": m.at,
                "commit": m.commit,
                "project": m.project,
                "private": m.private,
            })
        };
        let asks: Vec<Value> = self
            .asks()
            .iter()
            .map(|a| {
                json!({
                    "ask": a.id, "question": a.question, "blocks": a.blocks,
                    "recommends": a.recommends, "work": a.work, "marks": marks(&a.marks),
                })
            })
            .collect();
        let works: Vec<Value> = self
            .works()
            .iter()
            .map(|w| {
                let (standing, why, waiting) = match self.state.standing(w) {
                    Standing::Blocked(why) => ("blocked", Some(why), Vec::new()),
                    Standing::Waiting(asks) => ("waiting", None, asks),
                    Standing::Moving => ("moving", None, Vec::new()),
                    Standing::NotStarted => ("not-started", None, Vec::new()),
                    Standing::Done => ("done", None, Vec::new()),
                };
                json!({
                    "work": w.slug, "title": w.title, "goal": w.goal, "standing": standing,
                    "blocked_by": why, "waiting_on": waiting, "line": w.line,
                    "done": w.done, "next": w.next, "marks": marks(&w.marks),
                })
            })
            .collect();
        let decided: Vec<Value> = self
            .decided()
            .iter()
            .filter_map(|a| {
                let (how, m) = a.closed.as_ref()?;
                let (kind, text) = match how {
                    Closed::Answered(text) => ("answered", text),
                    Closed::Withdrawn(why) => ("withdrawn", why),
                };
                Some(json!({ "ask": a.id, "question": a.question, "closed": kind, "text": text, "marks": marks(m) }))
            })
            .collect();
        json!({
            "project": self.project,
            "every_project": self.wide(),
            "asks": asks,
            "works": works,
            "decided": decided,
            "unreadable_lines": self.damaged,
            "newer_lines": self.newer,
        })
    }
}

fn lines(n: usize) -> String {
    if n == 1 { "1 line".to_string() } else { format!("{n} lines") }
}

/// One line that says where a piece of work stands after an update.
pub fn work_line(state: &State, work: &Work) -> String {
    let standing = match state.standing(work) {
        Standing::Blocked(why) => format!("blocked by {why}"),
        Standing::Waiting(asks) => format!("waiting on ask:{}", asks.join(", ask:")),
        other => other.word().to_string(),
    };
    let line = if work.line.is_empty() { String::new() } else { format!(" · {}", work.line) };
    format!("{}/{} · {standing}{line}", work.marks.project, work.slug)
}
