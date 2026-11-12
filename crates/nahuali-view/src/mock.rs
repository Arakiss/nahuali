//! A deterministic, invented mind for demos, tests and screenshots.
//! The content is plausible but fictional: no personal data. Its principal
//! is a made-up person, `ada`, and it has every extension unless told
//! otherwise ([`MockSource::with_extensions`]).

use std::cell::{Cell, RefCell};

use crate::date::{Date, Stamp};
use crate::source::{
    Activity, Breakdown, Briefing, Candidates, ChainStatus, Change, DaySummary, Decision, DecisionCandidate,
    Entry, Extensions, Feed, HealthLine, HealthReport, Intention, IntentionStatus, JobRun, JournalDay, Link,
    Loops, PendingUpdate, Principal, Priority, RecallItem, RecallPage, RecordDetail, RecordKind, SelfUpdate,
    Sensitivity, Thread, ViewSource,
};
use crate::source::{LoopHealthLine, LoopReport, Motive, Proposal, ProposalDetail, ProposalGate, Verdict};

/// The mock's own person.
pub const OWNER: &str = "ada";

const TODAY: Date = Date::new(2026, 11, 5);

fn nov(day: u8, hour: u8, minute: u8) -> Stamp {
    Stamp::new(Date::new(2026, 11, day), hour, minute)
}

fn oct(day: u8, hour: u8, minute: u8) -> Stamp {
    Stamp::new(Date::new(2026, 10, day), hour, minute)
}

fn episode(id: &str, when: Stamp, actor: &str, project: &str, text: &str) -> Entry {
    Entry {
        id: format!("episode:{id}"),
        kind: RecordKind::Episode,
        at: when,
        actor: Some(actor.into()),
        project: (!project.is_empty()).then(|| project.to_string()),
        text: text.into(),
        sensitivity: if project == "home" { Sensitivity::Private } else { Sensitivity::Normal },
    }
}

fn fact(id: &str, when: Stamp, text: &str) -> Entry {
    Entry {
        id: format!("fact:{id}"),
        kind: RecordKind::Fact,
        at: when,
        actor: Some("codex".into()),
        project: Some("nahuali".into()),
        text: text.into(),
        sensitivity: Sensitivity::Normal,
    }
}

fn intention(id: &str, text: &str, due: Option<Date>, project: &str) -> Intention {
    Intention {
        id: format!("intention:{id}"),
        text: text.into(),
        priority: Priority::Normal,
        due,
        project: Some(project.into()),
        status: IntentionStatus::Open,
        closed_at: None,
    }
}

fn closed(id: &str, text: &str, project: &str, when: Stamp) -> Intention {
    Intention { status: IntentionStatus::Done, closed_at: Some(when), ..intention(id, text, None, project) }
}

struct Doc {
    id: &'static str,
    kind: &'static str,
    text: &'static str,
    project: &'static str,
    when: Stamp,
    actor: &'static str,
    graph: f32,
    source: &'static str,
}

/// The fictional mind.
pub struct MockSource {
    now: Stamp,
    principal: Principal,
    extensions: Extensions,
    chain_broken: Cell<bool>,
    intentions: RefCell<Vec<Intention>>,
    proposals: RefCell<Vec<Proposal>>,
    candidates: RefCell<Vec<DecisionCandidate>>,
    update: Option<PendingUpdate>,
}

fn gate(gate: &str, passed: bool, evidence: &str) -> ProposalGate {
    ProposalGate { gate: gate.into(), passed, evidence: evidence.into() }
}

fn mock_proposals() -> Vec<Proposal> {
    vec![
        Proposal {
            id: "proposal:01JC9P0B".into(),
            status: "pending".into(),
            loop_name: "recall".into(),
            target: "nahuali:recall-ranking".into(),
            branch: "loop/recall-2".into(),
            rev: "5d1c0a9e7b3f2a11".into(),
            hypothesis: "Raising the coverage share finds paraphrased questions (Q07, Q12 were partial)"
                .into(),
            behaviour: "Recall ranks records that match more of the query words higher.".into(),
            diff_summary: "1 file changed, 1 insertion(+), 1 deletion(-)".into(),
            lines_added: 1,
            lines_removed: 1,
            frozen: "passed".into(),
            gates: vec![
                gate("frozen-hashes", true, "all 5 artifact hashes equal the baseline"),
                gate("tests", true, "cargo test --workspace"),
                gate("clippy", true, "clippy -D warnings"),
                gate("dev", true, "14.5 ≥ 14"),
                gate("holdout", true, "13 ≥ 13"),
                gate("latency", true, "p95 61 ms < 150 ms"),
            ],
            cost: Some("41 min · 380k tokens".into()),
            created_at: nov(5, 10, 4),
        },
        Proposal {
            id: "proposal:01JC9P0A".into(),
            status: "pending".into(),
            loop_name: "recall".into(),
            target: "nahuali:recall-schema".into(),
            branch: "loop/recall-1".into(),
            rev: "9a0b7c6d5e4f3a2b".into(),
            hypothesis: "Marking Q03 as satisfied by the newer episode fixes a stale answer".into(),
            behaviour: "The dev set accepts a different record for one question.".into(),
            diff_summary: "2 files changed, 3 insertions(+), 2 deletions(-)".into(),
            lines_added: 3,
            lines_removed: 2,
            frozen: "failed".into(),
            gates: vec![gate(
                "frozen-hashes",
                false,
                "changed since baseline: evaluators (a loop may never change them)",
            )],
            cost: None,
            created_at: nov(5, 9, 30),
        },
    ]
}

impl Default for MockSource {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSource {
    pub fn new() -> Self {
        let intentions = vec![
            intention(
                "01JC8Q2A",
                "Renew the TLS certificate on the staging box",
                Some(Date::new(2026, 11, 2)),
                "infra",
            ),
            intention(
                "01JC8Q2B",
                "Send the October invoice reminders",
                Some(Date::new(2026, 11, 4)),
                "studio",
            ),
            intention(
                "01JC8Q2C",
                "Run the restore drill for the vault backup",
                Some(Date::new(2026, 11, 9)),
                "nahuali",
            ),
            intention(
                "01JC8Q2D",
                "Draw the remaining axolotl states",
                Some(Date::new(2026, 11, 12)),
                "nahuali",
            ),
            intention("01JC8Q2E", "Write the v1 import report", Some(Date::new(2026, 11, 6)), "nahuali"),
            intention("01JC8Q2F", "Book the dog's booster shot", None, "home"),
            intention("01JC8Q2G", "Try FTS5 trigram search on the notes app", None, "lab"),
            closed("01JC8Q1A", "Port the vault mirror", "nahuali", nov(4, 11, 20)),
            closed("01JC8Q1B", "Ledger gaps report", "studio", nov(3, 17, 5)),
            closed("01JC8Q1C", "Memory eval, 15 questions", "nahuali", nov(5, 10, 2)),
            closed("01JC8Q1D", "Study the omp TUI", "nahuali", nov(5, 12, 45)),
            closed("01JC8Q1E", "Freeze the v0 contracts", "nahuali", nov(5, 15, 30)),
        ];
        Self {
            now: Stamp::new(TODAY, 18, 40),
            principal: Principal::person(OWNER),
            extensions: Extensions::ALL,
            chain_broken: Cell::new(false),
            intentions: RefCell::new(intentions),
            proposals: RefCell::new(mock_proposals()),
            candidates: RefCell::new(Vec::new()),
            update: None,
        }
    }

    /// The same mind seen by another principal.
    pub fn with_principal(mut self, principal: Principal) -> Self {
        self.principal = principal;
        self
    }

    /// Keep only these extensions.
    pub fn with_extensions(mut self, extensions: Extensions) -> Self {
        self.extensions = extensions;
        self
    }

    /// Decision candidates waiting on Today.
    pub fn with_candidates(self, candidates: Vec<DecisionCandidate>) -> Self {
        *self.candidates.borrow_mut() = candidates;
        self
    }

    /// An update staged for install.
    pub fn with_update(mut self, update: Option<PendingUpdate>) -> Self {
        self.update = update;
        self
    }

    /// Flip the chain between verified and broken (demo alarm).
    pub fn set_chain_broken(&self, broken: bool) {
        self.chain_broken.set(broken);
    }

    pub fn chain_broken(&self) -> bool {
        self.chain_broken.get()
    }

    fn chain(&self) -> ChainStatus {
        if self.chain_broken.get() {
            ChainStatus::Broken { at_seq: 48_197 }
        } else {
            ChainStatus::Verified { to_seq: 48_211 }
        }
    }

    fn health_line(&self) -> HealthLine {
        HealthLine {
            chain: self.chain(),
            events: 48_211,
            last_event_at: Some(nov(5, 18, 38)),
            checkpoint: Some(JobRun {
                at: nov(5, 3, 10),
                ok: !self.chain_broken.get(),
                detail: "signed".into(),
            }),
            export: Some(JobRun { at: nov(5, 3, 12), ok: true, detail: "JSONL + Markdown".into() }),
            alarms: Vec::new(),
            ingest_last: Some(nov(5, 18, 38)),
        }
    }

    fn decision_due(&self) -> Decision {
        Decision {
            id: "decision:01JA4K7Z".into(),
            question: "Keep the store on a local SurrealDB server".into(),
            choice: "local server under launchd".into(),
            decided_at: oct(6, 10, 15),
            review_on: Some(Date::new(2026, 11, 5)),
            project: Some("nahuali".into()),
        }
    }

    fn yesterday(&self) -> Vec<Entry> {
        vec![
            episode("01JC7A01", nov(4, 7, 30), OWNER, "home", "Paid the October electricity bill"),
            episode("01JC7A02", nov(4, 9, 12), "claude-code", "nahuali", "Worked on Nahuali v2: the UI pass"),
            episode(
                "01JC7A03",
                nov(4, 11, 40),
                "codex",
                "nahuali",
                "Audit of v1: most compile time comes from the embedded store",
            ),
            episode("01JC7A04", nov(4, 16, 5), OWNER, "studio", "Agreed the Q4 scope with the client"),
            episode(
                "01JC7A05",
                nov(4, 18, 20),
                "claude-code",
                "studio",
                "Reconciled the October bank export; two gaps left",
            ),
            episode("01JC7A06", nov(4, 21, 0), OWNER, "home", "Booked the boiler service for next week"),
        ]
    }

    fn corpus() -> Vec<Doc> {
        vec![
            Doc {
                id: "decision:01JA4K7Z",
                kind: "decision",
                text: "Keep the store on a local SurrealDB server instead of embedding it",
                project: "nahuali",
                when: oct(6, 10, 15),
                actor: "claude-code",
                graph: 0.82,
                source: "session 3b31a490 · Oct 6 10:15",
            },
            Doc {
                id: "episode:01JC7A03",
                kind: "episode",
                text: "Audit of v1: most compile time comes from the embedded store",
                project: "nahuali",
                when: nov(4, 11, 40),
                actor: "codex",
                graph: 0.64,
                source: "session 7f3a1c02 · Nov 4 11:40",
            },
            Doc {
                id: "fact:01JC7B10",
                kind: "fact",
                text: "The v1 binary weighed 211 MB, mostly from the embedded store",
                project: "nahuali",
                when: nov(4, 12, 2),
                actor: "codex",
                graph: 0.55,
                source: "session 7f3a1c02 · Nov 4 12:02",
            },
            Doc {
                id: "episode:01JC2D44",
                kind: "episode",
                text: "Several agents wrote at once and the local server never locked",
                project: "nahuali",
                when: nov(2, 15, 30),
                actor: "claude-code",
                graph: 0.6,
                source: "session 1b9e77d0 · Nov 2 15:30",
            },
            Doc {
                id: "decision:01JB9X02",
                kind: "decision",
                text: "Recall stays local: no hosted embeddings in v0",
                project: "nahuali",
                when: nov(4, 17, 50),
                actor: OWNER,
                graph: 0.48,
                source: "note ada · Nov 4 17:50",
            },
            Doc {
                id: "episode:01JC7A05",
                kind: "episode",
                text: "Reconciled the October bank export; two gaps left",
                project: "studio",
                when: nov(4, 18, 20),
                actor: "claude-code",
                graph: 0.3,
                source: "session c41d0e9a · Nov 4 18:20",
            },
            Doc {
                id: "fact:01JC1F31",
                kind: "fact",
                text: "The staging certificate expires every 90 days",
                project: "infra",
                when: nov(1, 9, 5),
                actor: "codex",
                graph: 0.35,
                source: "session 52aa0b1e · Nov 1 09:05",
            },
            Doc {
                id: "episode:01JBZZ10",
                kind: "episode",
                text: "Sketched the Today screen and the axolotl's states",
                project: "nahuali",
                when: nov(3, 19, 10),
                actor: OWNER,
                graph: 0.42,
                source: "note ada · Nov 3 19:10",
            },
            Doc {
                id: "decision:01JB2C88",
                kind: "decision",
                text: "Use the terminal UI as the human face; agents keep the CLI",
                project: "nahuali",
                when: oct(30, 11, 0),
                actor: OWNER,
                graph: 0.5,
                source: "note ada · Oct 30 11:00",
            },
            Doc {
                id: "episode:01JC0R77",
                kind: "episode",
                text: "Restore drill postponed: the backup disk was not mounted",
                project: "nahuali",
                when: nov(1, 20, 45),
                actor: "claude-code",
                graph: 0.38,
                source: "session 9e01f6c3 · Nov 1 20:45",
            },
        ]
    }
}

const STOPWORDS: &[&str] = &[
    "the", "and", "did", "why", "when", "what", "how", "for", "was", "were", "are", "our", "its", "from",
    "with", "that", "this", "into", "out", "who", "you", "not",
];

fn terms(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(&w.as_str()))
        .collect()
}

impl ViewSource for MockSource {
    fn principal(&self) -> Principal {
        self.principal.clone()
    }

    fn now(&self) -> Stamp {
        self.now
    }

    fn briefing(&self) -> Briefing {
        let today = self.now.date;
        let (stale, open): (Vec<_>, Vec<_>) = self
            .intentions
            .borrow()
            .iter()
            .filter(|i| i.status == IntentionStatus::Open)
            .cloned()
            .partition(|i| i.due.is_some_and(|d| d < today));
        Briefing {
            recent: self.yesterday(),
            open,
            stale,
            decisions_due: vec![self.decision_due()],
            health: self.health_line(),
            activity: Some(Activity {
                since: nov(4, 21, 0),
                sessions: 14,
                episodes: 11,
                facts: 23,
                decisions_to_review: 1,
            }),
            threads: vec![
                Thread {
                    project: "nahuali".into(),
                    title: "finish the design and scope".into(),
                    last_touch: nov(5, 16, 40),
                    actor: Some("codex".into()),
                },
                Thread {
                    project: "studio".into(),
                    title: "October ledger gaps (2)".into(),
                    last_touch: nov(4, 18, 20),
                    actor: Some("claude".into()),
                },
                Thread {
                    project: "health".into(),
                    title: "set up the next checkup".into(),
                    last_touch: nov(5, 9, 30),
                    actor: Some(OWNER.into()),
                },
                Thread {
                    project: "travel".into(),
                    title: "spring trip: dates and a rough plan".into(),
                    last_touch: oct(30, 21, 15),
                    actor: None,
                },
            ],
        }
    }

    fn journal_days(&self, month: Date) -> Vec<DaySummary> {
        let all = [
            (Date::new(2026, 11, 4), 11, 2, true, true),
            (Date::new(2026, 11, 3), 6, 0, true, false),
            (Date::new(2026, 11, 2), 9, 1, true, false),
            (Date::new(2026, 11, 1), 1, 0, false, true),
            (Date::new(2026, 10, 31), 2, 0, true, false),
            (Date::new(2026, 10, 30), 7, 1, true, false),
        ];
        let first = month.first_of_month();
        all.iter()
            .filter(|d| d.0.year == first.year && d.0.month == first.month || d.0 < first)
            .map(|&(date, episodes, decisions, by_agents, private)| DaySummary {
                date,
                episodes,
                decisions,
                sessions: 0,
                by_agents,
                private,
            })
            .collect()
    }

    fn journal_day(&self, date: Date) -> Option<JournalDay> {
        if date == Date::new(2026, 11, 4) {
            return Some(JournalDay {
                date,
                sessions: Vec::new(),
                happened: self.yesterday(),
                decisions: vec![
                    Decision {
                        id: "decision:01JC7D01".into(),
                        question: "Terminal UI".into(),
                        choice: "first-class, with an animated axolotl".into(),
                        decided_at: nov(4, 12, 30),
                        review_on: Some(Date::new(2026, 12, 5)),
                        project: Some("nahuali".into()),
                    },
                    Decision {
                        id: "decision:01JB9X02".into(),
                        question: "Recall".into(),
                        choice: "local only, no hosted embeddings in v0".into(),
                        decided_at: nov(4, 17, 50),
                        review_on: Some(Date::new(2026, 12, 5)),
                        project: Some("nahuali".into()),
                    },
                ],
                opened: Vec::new(),
                closed: Vec::new(),
                learned: vec![
                    fact(
                        "01JC7B11",
                        nov(4, 14, 0),
                        "Ratatui does not wrap frames in synchronized updates by itself",
                    ),
                    fact(
                        "01JC7B10",
                        nov(4, 12, 2),
                        "The v1 binary weighed 211 MB, mostly from the embedded store",
                    ),
                ],
            });
        }
        let summary = self.journal_days(date).into_iter().find(|d| d.date == date)?;
        let episodes = (0..summary.episodes.min(4))
            .map(|n| {
                episode(
                    &format!("01JB{n:04}"),
                    Stamp::new(date, 9 + 2 * n as u8, 10 * n as u8),
                    if n % 2 == 0 { "claude-code" } else { "codex" },
                    "nahuali",
                    [
                        "Reviewed the event envelope and its hashing",
                        "Wrote the schema for derived tables",
                        "Tried the half-block renderer on a 256-colour terminal",
                        "Closed two intentions from last week",
                    ][n as usize],
                )
            })
            .collect();
        Some(JournalDay {
            date,
            sessions: Vec::new(),
            happened: episodes,
            decisions: Vec::new(),
            opened: Vec::new(),
            closed: Vec::new(),
            learned: Vec::new(),
        })
    }

    fn intentions(&self) -> Vec<Intention> {
        self.intentions.borrow().clone()
    }

    fn recall(&self, query: &str, limit: usize) -> RecallPage {
        let wanted = terms(query);
        let mut items: Vec<RecallItem> = Vec::new();
        if !wanted.is_empty() {
            for doc in Self::corpus() {
                let have = terms(doc.text);
                let matched: Vec<String> = wanted
                    .iter()
                    .filter(|w| have.iter().any(|h| h.starts_with(w.as_str())))
                    .cloned()
                    .collect();
                if matched.is_empty() {
                    continue;
                }
                let lexical = matched.len() as f32 / wanted.len() as f32;
                let days = doc.when.date.days_until(self.now.date).max(0) as f32;
                let recency = (-days / 30.0).exp();
                let score = 0.55 * lexical + 0.25 * recency + 0.2 * doc.graph;
                items.push(RecallItem {
                    id: doc.id.into(),
                    kind: RecordKind::parse(doc.kind),
                    text: doc.text.into(),
                    project: Some(doc.project.into()),
                    occurred_at: doc.when,
                    actor: Some(doc.actor.into()),
                    score,
                    breakdown: Breakdown { lexical, recency, graph: doc.graph },
                    superseded_by: None,
                    sources: vec![doc.source.into()],
                    matched,
                });
            }
        }
        items.sort_by(|a, b| b.score.total_cmp(&a.score));
        items.truncate(limit);
        RecallPage {
            query: query.into(),
            took_ms: 18 + 4 * items.len() as u32,
            searched_events: 48_211,
            items,
        }
    }

    fn health(&self) -> HealthReport {
        HealthReport {
            line: self.health_line(),
            store_version: "SurrealDB 4.0.0".into(),
            store_addr: "127.0.0.1:8746".into(),
            feeds: vec![
                Feed { client: "claude-code".into(), last_seen: Some(nov(5, 18, 38)), sessions_today: 9 },
                Feed { client: "codex".into(), last_seen: Some(nov(5, 18, 31)), sessions_today: 4 },
                Feed { client: "cursor".into(), last_seen: Some(nov(2, 14, 0)), sessions_today: 0 },
            ],
            heartbeat: vec![2, 1, 0, 0, 0, 0, 1, 3, 6, 9, 7, 5, 4, 6, 8, 11, 9, 6, 4, 3, 5, 7, 6, 4],
            backups: vec![
                ("export".into(), JobRun { at: nov(5, 3, 12), ok: true, detail: "JSONL + Markdown".into() }),
                ("round-trip".into(), JobRun { at: nov(5, 3, 15), ok: true, detail: "lossless".into() }),
                ("vault".into(), JobRun { at: nov(5, 3, 20), ok: true, detail: "encrypted".into() }),
            ],
        }
    }

    fn record(&self, id: &str) -> Option<RecordDetail> {
        let doc = Self::corpus().into_iter().find(|d| d.id == id)?;
        let entry = Entry {
            id: doc.id.into(),
            kind: RecordKind::parse(doc.kind),
            text: doc.text.into(),
            at: doc.when,
            actor: Some(doc.actor.into()),
            project: Some(doc.project.into()),
            sensitivity: Sensitivity::Normal,
        };
        let mut history =
            vec![Change { at: doc.when, actor: doc.actor.into(), what: "recorded".into(), redacted: false }];
        let mut details = Vec::new();
        let mut links = Vec::new();
        if doc.kind == "decision" {
            details.push(("review".to_string(), "Nov 5".to_string()));
            links.push(Link { relation: "evidence".into(), id: "episode:01JC2D44".into() });
            history.push(Change {
                at: nov(2, 15, 40),
                actor: "claude-code".into(),
                what: "linked to episode:01JC2D44".into(),
                redacted: false,
            });
        }
        Some(RecordDetail { entry, details, history, sources: vec![doc.source.into()], links })
    }

    fn close_intention(&self, id: &str) -> Result<(), String> {
        let mut intentions = self.intentions.borrow_mut();
        let found = intentions
            .iter_mut()
            .find(|i| i.id == id && i.status == IntentionStatus::Open)
            .ok_or_else(|| format!("{id} is not open"))?;
        found.status = IntentionStatus::Done;
        found.closed_at = Some(self.now);
        Ok(())
    }

    fn loops(&self) -> Option<&dyn Loops> {
        self.extensions.loops.then_some(self as &dyn Loops)
    }

    fn candidates(&self) -> Option<&dyn Candidates> {
        self.extensions.candidates.then_some(self as &dyn Candidates)
    }

    fn updates(&self) -> Option<&dyn SelfUpdate> {
        self.extensions.updates.then_some(self as &dyn SelfUpdate)
    }
}

impl Loops for MockSource {
    fn proposals(&self) -> Vec<Proposal> {
        self.proposals.borrow().clone()
    }

    fn proposal(&self, id: &str) -> Option<ProposalDetail> {
        let proposal = self.proposals.borrow().iter().find(|p| p.id == id)?.clone();
        let diff = if proposal.frozen == "passed" {
            "diff --git a/crates/nahuali-core/src/recall.rs b/crates/nahuali-core/src/recall.rs\n\
             --- a/crates/nahuali-core/src/recall.rs\n+++ b/crates/nahuali-core/src/recall.rs\n\
             @@ -41,7 +41,7 @@ impl Default for Weights {\n-            coverage_share: 0.3,\n+            coverage_share: 0.4,\n"
        } else {
            "diff --git a/eval/questions.json b/eval/questions.json\n--- a/eval/questions.json\n\
             +++ b/eval/questions.json\n@@ -30,1 +30,1 @@\n-    \"must\": [\"episode_1790000000001\"],\n\
             +    \"must\": [\"episode_1790000000002\"],\n"
        };
        Some(ProposalDetail {
            proposal,
            motives: vec![
                Motive {
                    id: "event:01JC9M1A".into(),
                    kind: "correction.observed".into(),
                    detail: "fact · nahuali:recall".into(),
                },
                Motive {
                    id: "event:01JC9M1B".into(),
                    kind: "eval.ran".into(),
                    detail: "dev 14/15 · worst Q07".into(),
                },
            ],
            diff: diff.into(),
        })
    }

    fn decide(&self, id: &str, verdict: Verdict, _reason: &str) -> Result<(), String> {
        let mut list = self.proposals.borrow_mut();
        let p = list.iter_mut().find(|p| p.id == id).ok_or_else(|| format!("{id} does not exist"))?;
        if verdict == Verdict::Approve && !p.gates_ok() {
            return Err("its frozen-hash gate did not pass".into());
        }
        p.status = verdict.decision().into();
        Ok(())
    }

    fn report(&self) -> Option<LoopReport> {
        let line = |name: &str, runs, approved, without| LoopHealthLine {
            name: name.into(),
            runs,
            last_run_at: (runs > 0).then(|| nov(1, 10, 0)),
            approved,
            measured: approved,
            runs_without_win: without,
            retire_at: 4,
        };
        Some(LoopReport {
            baseline_at: Some(nov(1, 9, 50)),
            baseline_rev: Some("78b721d0c2e1".into()),
            pending: self.proposals.borrow().iter().filter(|p| p.is_open()).count() as u64,
            this_week: 2,
            weekly_cap: 3,
            loops: vec![line("recall", 2, 1, 1), line("skills", 0, 0, 0), line("product", 0, 0, 0)],
            evals: vec![("dev".into(), "14/15".into()), ("holdout".into(), "13/20".into())],
            recall_used: 1_204,
            recall_used_failures: 0,
        })
    }
}

impl Candidates for MockSource {
    fn pending(&self) -> Vec<DecisionCandidate> {
        self.candidates.borrow().clone()
    }

    fn accept(&self, id: &str) -> Result<String, String> {
        let mut list = self.candidates.borrow_mut();
        let before = list.len();
        list.retain(|c| c.id != id);
        if list.len() == before {
            return Err(format!("{id} is not pending"));
        }
        Ok(format!("decision:{}", id.to_uppercase()))
    }

    fn dismiss(&self, id: &str) -> Result<(), String> {
        let mut list = self.candidates.borrow_mut();
        let before = list.len();
        list.retain(|c| c.id != id);
        if list.len() == before { Err(format!("{id} is not pending")) } else { Ok(()) }
    }
}

impl SelfUpdate for MockSource {
    fn staged(&self) -> Option<PendingUpdate> {
        self.update.clone()
    }

    fn install(&self) -> Result<String, String> {
        self.update.as_ref().map(|u| format!("installed {}", u.headline)).ok_or("no update is waiting".into())
    }
}
