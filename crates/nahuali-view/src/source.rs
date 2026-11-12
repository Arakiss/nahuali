//! What the screens need from a mind: the seam between a store and the UI.
//!
//! Everything that crosses it is a view model owned by this crate: what a
//! screen shows, not how a store keeps it. A store adapter decodes its own
//! rows into these types; the UI never sees a table, a column name or a JSON
//! map, so a new record model only changes the adapter.
//!
//! The seam has two parts:
//!
//! - [`ViewSource`], the core every installation has: who the session is
//!   for ([`Principal`]), today's briefing, the journal, open work, search, a
//!   record with its history, and the state of verification.
//! - Optional extensions, reached through [`ViewSource::loops`],
//!   [`ViewSource::candidates`] and [`ViewSource::updates`]. A source without
//!   one returns `None`, and the UI then shows no tab, no row, no key and no
//!   card for it ([`Extensions`]).
//!
//! All stamps are local time. The UI never reads the clock itself; it asks
//! [`ViewSource::now`], so tests and the mock are deterministic.

use crate::date::{Date, Stamp};

// ---------------------------------------------------------------------------
// Who the session is for

/// What kind of party a [`Principal`] is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PrincipalKind {
    #[default]
    Person,
    Team,
    Organization,
    /// An agent acting on its own account, not for a person or a team.
    Agent,
}

impl PrincipalKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::Team => "team",
            Self::Organization => "organization",
            Self::Agent => "agent",
        }
    }
}

/// The party this session is, or acts for. The UI names it and measures
/// "since your last visit" from its own activity; which actors write on its
/// behalf is the source's business.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Principal {
    /// How it is shown: a handle or a display name.
    pub name: String,
    pub kind: PrincipalKind,
}

impl Principal {
    pub fn new(name: impl Into<String>, kind: PrincipalKind) -> Self {
        Self { name: name.into(), kind }
    }

    pub fn person(name: impl Into<String>) -> Self {
        Self::new(name, PrincipalKind::Person)
    }

    /// `ada` for a person, `infra · team` for anything else.
    pub fn badge(&self) -> String {
        match self.kind {
            PrincipalKind::Person => self.name.clone(),
            kind => format!("{} · {}", self.name, kind.label()),
        }
    }
}

// ---------------------------------------------------------------------------
// Records

/// `public | normal | private | intimate`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sensitivity {
    Public,
    #[default]
    Normal,
    Private,
    Intimate,
}

impl Sensitivity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Normal => "normal",
            Self::Private => "private",
            Self::Intimate => "intimate",
        }
    }

    /// Above `normal`: shown with a mark, never on a shared screen by default.
    pub fn is_private(self) -> bool {
        self >= Self::Private
    }
}

/// What a record is, as the UI names it. Kinds the UI does not know yet
/// still render, under their own label.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum RecordKind {
    /// Something that happened.
    #[default]
    Episode,
    /// Something true.
    Fact,
    /// A choice, with what it replaced.
    Decision,
    /// Something to do.
    Intention,
    Other(String),
}

impl RecordKind {
    pub fn parse(label: &str) -> Self {
        match label {
            "episode" => Self::Episode,
            "fact" => Self::Fact,
            "decision" => Self::Decision,
            "intention" => Self::Intention,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Episode => "episode",
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::Intention => "intention",
            Self::Other(label) => label,
        }
    }
}

/// One record in a list: a line of the journal, of Today or of a detail.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub kind: RecordKind,
    /// What it says, in one piece of text (a fact as `subject predicate object`).
    pub text: String,
    pub at: Stamp,
    /// Who wrote it, when the source knows.
    pub actor: Option<String>,
    pub project: Option<String>,
    pub sensitivity: Sensitivity,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IntentionStatus {
    #[default]
    Open,
    Done,
    Dropped,
}

/// Work to do, open or recently closed.
#[derive(Clone, Debug, PartialEq)]
pub struct Intention {
    pub id: String,
    pub text: String,
    pub priority: Priority,
    pub due: Option<Date>,
    pub project: Option<String>,
    pub status: IntentionStatus,
    /// When it was closed or dropped.
    pub closed_at: Option<Stamp>,
}

/// A decision, with what the screens show of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub id: String,
    pub question: String,
    pub choice: String,
    pub decided_at: Stamp,
    pub review_on: Option<Date>,
    pub project: Option<String>,
}

/// One change in a record's history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub at: Stamp,
    pub actor: String,
    /// In words: `recorded`, `closed`, `refiled to infra`.
    pub what: String,
    /// Its content was forgotten; the change itself stays in the chain.
    pub redacted: bool,
}

/// How one record points at another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// `replaces`, `replaced by`, `mentions`, `evidence`…
    pub relation: String,
    pub id: String,
}

/// One record in full: what `nahuali show` prints.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordDetail {
    pub entry: Entry,
    /// What only some kinds carry, already in words (`choice`, `due`, `status`…).
    pub details: Vec<(String, String)>,
    /// Oldest first.
    pub history: Vec<Change>,
    pub sources: Vec<String>,
    pub links: Vec<Link>,
}

// ---------------------------------------------------------------------------
// Today

/// What others wrote since the principal last left a mark.
#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    /// The principal's latest own write.
    pub since: Stamp,
    pub sessions: u32,
    pub episodes: u32,
    pub facts: u32,
    pub decisions_to_review: u32,
}

/// A project with recent movement.
#[derive(Clone, Debug, PartialEq)]
pub struct Thread {
    pub project: String,
    pub title: String,
    pub last_touch: Stamp,
    pub actor: Option<String>,
}

/// What Today and the mascot need.
#[derive(Clone, Debug, PartialEq)]
pub struct Briefing {
    /// What happened the previous day, oldest first.
    pub recent: Vec<Entry>,
    pub open: Vec<Intention>,
    pub stale: Vec<Intention>,
    pub decisions_due: Vec<Decision>,
    pub health: HealthLine,
    /// `None` when the principal never wrote: there is no last visit.
    pub activity: Option<Activity>,
    pub threads: Vec<Thread>,
}

// ---------------------------------------------------------------------------
// Verification and health

/// The health line every screen carries.
#[derive(Clone, Debug, PartialEq)]
pub struct HealthLine {
    pub chain: ChainStatus,
    pub events: u64,
    pub last_event_at: Option<Stamp>,
    pub checkpoint: Option<JobRun>,
    pub export: Option<JobRun>,
    /// Latest write from any actor other than the principal, if tracked.
    pub ingest_last: Option<Stamp>,
    /// Things that stopped working: stalled ingest, failed or old export.
    pub alarms: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainStatus {
    Verified { to_seq: u64 },
    Broken { at_seq: u64 },
    Unknown,
}

/// The outcome of a background job (checkpoint, export, backup).
#[derive(Clone, Debug, PartialEq)]
pub struct JobRun {
    pub at: Stamp,
    pub ok: bool,
    pub detail: String,
}

/// One actor that writes into the mind.
#[derive(Clone, Debug, PartialEq)]
pub struct Feed {
    pub client: String,
    pub last_seen: Option<Stamp>,
    pub sessions_today: u32,
}

/// Everything the Health screen shows.
#[derive(Clone, Debug, PartialEq)]
pub struct HealthReport {
    pub line: HealthLine,
    pub store_version: String,
    pub store_addr: String,
    pub feeds: Vec<Feed>,
    /// Writes per hour for the last 24 hours, oldest first.
    pub heartbeat: Vec<u32>,
    pub backups: Vec<(String, JobRun)>,
}

// ---------------------------------------------------------------------------
// Journal

/// One day of the journal, for the day list.
#[derive(Clone, Debug, PartialEq)]
pub struct DaySummary {
    pub date: Date,
    pub episodes: u32,
    pub decisions: u32,
    pub sessions: u32,
    /// Someone other than the principal wrote that day.
    pub by_agents: bool,
    /// A record above `normal` was written that day.
    pub private: bool,
}

/// Work on one project during a day.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectSessions {
    pub project: Option<String>,
    /// `3 sessions · 12 agent runs · 2 corrections`.
    pub summary: String,
}

/// One day of the journal, in full.
#[derive(Clone, Debug, PartialEq)]
pub struct JournalDay {
    pub date: Date,
    pub sessions: Vec<ProjectSessions>,
    pub happened: Vec<Entry>,
    pub decisions: Vec<Decision>,
    pub opened: Vec<Intention>,
    pub closed: Vec<Intention>,
    pub learned: Vec<Entry>,
}

// ---------------------------------------------------------------------------
// Search

/// Why a result ranks where it does.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Breakdown {
    pub lexical: f32,
    pub recency: f32,
    pub graph: f32,
}

/// One search result.
#[derive(Clone, Debug, PartialEq)]
pub struct RecallItem {
    pub id: String,
    pub kind: RecordKind,
    pub text: String,
    pub project: Option<String>,
    pub occurred_at: Stamp,
    pub actor: Option<String>,
    pub score: f32,
    pub breakdown: Breakdown,
    pub superseded_by: Option<String>,
    pub sources: Vec<String>,
    /// Matched query terms, for the "why" panel.
    pub matched: Vec<String>,
}

/// A search run.
#[derive(Clone, Debug, PartialEq)]
pub struct RecallPage {
    pub query: String,
    pub items: Vec<RecallItem>,
    pub took_ms: u32,
    pub searched_events: u64,
}

// ---------------------------------------------------------------------------
// The core

/// The read and minimal write surface every installation gives the UI.
pub trait ViewSource {
    /// Who this session is, or acts for.
    fn principal(&self) -> Principal;
    /// Local wall-clock time.
    fn now(&self) -> Stamp;
    /// What Today needs.
    fn briefing(&self) -> Briefing;
    /// The days of `month` (any date inside it) that have entries, newest
    /// first; the list may reach into the month before.
    fn journal_days(&self, month: Date) -> Vec<DaySummary>;
    /// One day in full; `None` when nothing was recorded.
    fn journal_day(&self, date: Date) -> Option<JournalDay>;
    /// Open, stale and recently closed intentions.
    fn intentions(&self) -> Vec<Intention>;
    /// Search, best first, with the reason for each result.
    fn recall(&self, query: &str, limit: usize) -> RecallPage;
    /// One record with its history; `None` when it does not exist or is not
    /// visible to this session.
    fn record(&self, id: &str) -> Option<RecordDetail>;
    /// Verification, jobs and feeds.
    fn health(&self) -> HealthReport;
    /// Close an intention as done; the error is a line to show.
    fn close_intention(&self, id: &str) -> Result<(), String>;

    /// The improvement loops, when this installation runs them.
    fn loops(&self) -> Option<&dyn Loops> {
        None
    }
    /// Decision candidates, when something proposes them.
    fn candidates(&self) -> Option<&dyn Candidates> {
        None
    }
    /// A staged update the UI can install, when this installation stages them.
    fn updates(&self) -> Option<&dyn SelfUpdate> {
        None
    }
}

/// Which extensions a source has, read once per screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extensions {
    pub loops: bool,
    pub candidates: bool,
    pub updates: bool,
}

impl Extensions {
    /// Every extension: what the mock and the owner's install have.
    pub const ALL: Self = Self { loops: true, candidates: true, updates: true };
    /// The core alone.
    pub const NONE: Self = Self { loops: false, candidates: false, updates: false };

    pub fn of(src: &dyn ViewSource) -> Self {
        Self {
            loops: src.loops().is_some(),
            candidates: src.candidates().is_some(),
            updates: src.updates().is_some(),
        }
    }
}

// ---------------------------------------------------------------------------
// Extension: improvement loops

/// One gate a proposal went through.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposalGate {
    pub gate: String,
    pub passed: bool,
    pub evidence: String,
}

/// A motivating event, as the reviewer sees it (kind, class, subject,
/// pointer; never transcript text).
#[derive(Clone, Debug, PartialEq)]
pub struct Motive {
    pub id: String,
    pub kind: String,
    pub detail: String,
}

/// A proposed change to the system.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub id: String,
    /// pending, approved, rejected, deferred, applied or reverted.
    pub status: String,
    pub loop_name: String,
    pub target: String,
    pub branch: String,
    pub rev: String,
    pub hypothesis: String,
    pub behaviour: String,
    pub diff_summary: String,
    pub lines_added: u64,
    pub lines_removed: u64,
    /// unchecked, passed or failed.
    pub frozen: String,
    pub gates: Vec<ProposalGate>,
    pub cost: Option<String>,
    pub created_at: Stamp,
}

impl Proposal {
    /// Whether it can still be approved, rejected or deferred.
    pub fn is_open(&self) -> bool {
        matches!(self.status.as_str(), "pending" | "deferred")
    }

    /// Whether every gate it went through passed, the frozen one included.
    pub fn gates_ok(&self) -> bool {
        self.frozen == "passed" && self.gates.iter().all(|g| g.passed)
    }
}

/// A proposal with what the detail pane shows.
#[derive(Clone, Debug, PartialEq)]
pub struct ProposalDetail {
    pub proposal: Proposal,
    pub motives: Vec<Motive>,
    /// The literal diff (possibly truncated by the recorder).
    pub diff: String,
}

/// One loop's health.
#[derive(Clone, Debug, PartialEq)]
pub struct LoopHealthLine {
    pub name: String,
    pub runs: u64,
    pub last_run_at: Option<Stamp>,
    pub approved: u64,
    pub measured: u64,
    /// The kill-criterion counter.
    pub runs_without_win: u64,
    pub retire_at: u64,
}

/// The loops card of Health.
#[derive(Clone, Debug, PartialEq)]
pub struct LoopReport {
    pub baseline_at: Option<Stamp>,
    pub baseline_rev: Option<String>,
    pub pending: u64,
    pub this_week: u64,
    pub weekly_cap: u64,
    pub loops: Vec<LoopHealthLine>,
    /// Latest score per suite: (suite, "14/15").
    pub evals: Vec<(String, String)>,
    pub recall_used: u64,
    pub recall_used_failures: u64,
}

/// A reviewer's answer to a proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Approve,
    Reject,
    Defer,
}

impl Verdict {
    /// The recorded value.
    pub fn decision(self) -> &'static str {
        match self {
            Self::Approve => "approved",
            Self::Reject => "rejected",
            Self::Defer => "deferred",
        }
    }
}

/// Proposals to review: the Proposals view and the loops card of Health.
pub trait Loops {
    /// Newest first.
    fn proposals(&self) -> Vec<Proposal>;
    fn proposal(&self, id: &str) -> Option<ProposalDetail>;
    /// Record the principal's verdict; the error is a line to show.
    fn decide(&self, id: &str, verdict: Verdict, reason: &str) -> Result<(), String>;
    /// The loops' own health, when it can be read.
    fn report(&self) -> Option<LoopReport>;
}

// ---------------------------------------------------------------------------
// Extension: decision candidates

/// A question answered with options, not yet kept as a decision or dismissed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionCandidate {
    pub id: String,
    pub question: String,
    pub chosen: String,
}

/// Decisions something noticed and nobody recorded yet: rows on Today.
pub trait Candidates {
    /// Oldest first.
    fn pending(&self) -> Vec<DecisionCandidate>;
    /// Record it as a decision; returns the decision's id.
    fn accept(&self, id: &str) -> Result<String, String>;
    fn dismiss(&self, id: &str) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// Extension: self-update

/// An update staged for this installation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingUpdate {
    /// What it carries, in one line.
    pub headline: String,
    /// The commit it was built from, if any.
    pub commit: Option<String>,
}

/// Updates the UI can install: a row on Today and its key.
pub trait SelfUpdate {
    fn staged(&self) -> Option<PendingUpdate>;
    /// Called with the terminal handed back to the shell, so any prompt can
    /// be answered there; returns the line to show.
    fn install(&self) -> Result<String, String>;
}
