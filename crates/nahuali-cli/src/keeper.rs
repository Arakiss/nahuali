//! The keeper: the one process that has the database open (`nahuali serve`).
//!
//! It holds an embedded SurrealDB under `<home>/db`, answers the commands
//! of this user over a socket, and leaves when nobody has spoken to it for
//! a while.
//!
//! Two tables. `event` is the memory: every event, added and never changed,
//! kept whole as the line it was written as and with its marks as fields.
//! `thing` is derived from it and can be thrown away: one record per card,
//! ask, decision, lesson and note as they stand now, with a full-text index
//! over their words. It is what `recall` searches.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use nahuali_core::State;
use nahuali_core::event::{Event, Line, What};
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, Notify};

use crate::remote;

/// How long the keeper stays with nobody speaking to it.
const IDLE: u64 = 600;

/// Words are split where letters end, lower-cased, stripped of accents and
/// cut to their English root, so `worktrees` finds `worktree` and
/// `decision` finds `decisión`.
const SCHEMA: &str = "
DEFINE TABLE IF NOT EXISTS event SCHEMALESS;
DEFINE INDEX IF NOT EXISTS event_project ON event FIELDS project;
DEFINE INDEX IF NOT EXISTS event_at ON event FIELDS at;
DEFINE ANALYZER IF NOT EXISTS words TOKENIZERS class FILTERS lowercase, ascii, snowball(english);
DEFINE TABLE IF NOT EXISTS thing SCHEMALESS;
DEFINE INDEX IF NOT EXISTS thing_key ON thing FIELDS key;
DEFINE INDEX IF NOT EXISTS thing_text ON thing FIELDS text FULLTEXT ANALYZER words BM25;
DELETE thing;
";

/// What the keeper knows beyond the database: the events in memory, and
/// which things it has indexed and in what shape.
#[derive(Default)]
struct Held {
    events: Vec<Event>,
    indexed: HashMap<String, u64>,
}

struct Keeper {
    db: Surreal<Any>,
    held: Mutex<Held>,
    started: Instant,
    /// Seconds after `started` when someone last spoke.
    spoken: AtomicU64,
    leave: Notify,
    stamp: u64,
}

fn text(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Keeper {
    /// Keep one event. Keeping it twice changes nothing.
    async fn keep(&self, line: &str) -> Result<bool, String> {
        let Line::Event(event) = Event::read(line) else {
            return Err("that is not a line of a Nahuali log".to_string());
        };
        let kind = match &event.what {
            What::Work(_) => "work",
            What::Ask(_) => "ask",
            What::Answer(_) => "answer",
            What::Withdraw(_) => "withdraw",
            What::Learn(_) => "learn",
            What::Retire(_) => "retire",
            What::Note(_) => "note",
            What::Forget(_) => "forget",
            What::Shown(_) => "shown",
            What::Unknown => "unknown",
        };
        let mut held = self.held.lock().await;
        let mut answer = self
            .db
            .query(
                "INSERT IGNORE INTO event { id: $id, at: <datetime> $at, project: $project, by: $by, \
                 person: $person, verified: $verified, private: $private, kind: $kind, line: $line } RETURN VALUE id",
            )
            .bind(("id", event.id.clone()))
            .bind(("at", event.at.rfc3339()))
            .bind(("project", event.project.clone()))
            .bind(("by", event.by.name().to_string()))
            .bind(("person", event.by.is_person()))
            .bind(("verified", event.sure.is_verified()))
            .bind(("private", event.private))
            .bind(("kind", kind))
            .bind(("line", line.to_string()))
            .await
            .map_err(text)?;
        let kept: Vec<Value> = answer.take(0).map_err(text)?;
        if kept.is_empty() {
            return Ok(false);
        }
        held.events.push(*event);
        self.index(&mut held).await?;
        Ok(true)
    }

    /// Bring `thing` in line with the events: write what changed, remove
    /// what is no longer held.
    async fn index(&self, held: &mut Held) -> Result<(), String> {
        let things = State::fold(&held.events).things();
        let mut seen = HashMap::with_capacity(things.len());
        for thing in &things {
            let words = thing.text();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (&words, &thing.marks.project, thing.marks.private, thing.everywhere, thing.marks.at.0)
                .hash(&mut hasher);
            let shape = hasher.finish();
            seen.insert(thing.key.clone(), shape);
            if held.indexed.get(&thing.key) == Some(&shape) {
                continue;
            }
            self.db
                .query(
                    "DELETE thing WHERE key = $key; \
                     CREATE thing CONTENT { key: $key, kind: $kind, text: $text, project: $project, \
                     private: $private, everywhere: $everywhere, at: <datetime> $at }",
                )
                .bind(("key", thing.key.clone()))
                .bind(("kind", thing.kind))
                .bind(("text", words))
                .bind(("project", thing.marks.project.clone()))
                .bind(("private", thing.marks.private))
                .bind(("everywhere", thing.everywhere))
                .bind(("at", thing.marks.at.rfc3339()))
                .await
                .map_err(text)?
                .check()
                .map_err(text)?;
        }
        for gone in held.indexed.keys().filter(|key| !seen.contains_key(*key)) {
            self.db.query("DELETE thing WHERE key = $key").bind(("key", gone.clone())).await.map_err(text)?;
        }
        held.indexed = seen;
        Ok(())
    }

    async fn lines(&self) -> Result<Vec<String>, String> {
        let mut answer = self.db.query("SELECT VALUE line FROM event ORDER BY id ASC").await.map_err(text)?;
        answer.take(0).map_err(text)
    }

    /// The keys of the things that hold every word asked for, the best
    /// match first. What the asker may not see is never among them.
    async fn search(&self, request: &Value) -> Result<Vec<String>, String> {
        let words = request["q"].as_str().unwrap_or_default().trim().to_string();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let reach = match request["scope"].as_str() {
            Some("everything") => "true",
            Some("all") => "(private = false OR project = $project)",
            _ => "(project = $project OR (everywhere = true AND private = false))",
        };
        let query = format!(
            "SELECT key, search::score(0) AS score, at FROM thing WHERE text @0@ $q AND {reach} \
             ORDER BY score DESC, at DESC LIMIT $limit"
        );
        let mut answer = self
            .db
            .query(query)
            .bind(("q", words))
            .bind(("project", request["project"].as_str().unwrap_or_default().to_string()))
            .bind(("limit", request["limit"].as_u64().unwrap_or(8).min(50)))
            .await
            .map_err(text)?;
        let rows: Vec<Value> = answer.take(0).map_err(text)?;
        Ok(rows.iter().filter_map(|row| row["key"].as_str().map(str::to_string)).collect())
    }

    async fn answer(&self, request: &Value) -> Value {
        if request["stamp"].as_u64() != Some(self.stamp) {
            // A newer build is speaking: step aside and let it start its own.
            self.leave.notify_one();
            return json!({ "ok": false, "stale": true });
        }
        let done = match request["op"].as_str() {
            Some("read") => self.lines().await.map(|lines| json!({ "ok": true, "lines": lines })),
            Some("append") => match request["line"].as_str() {
                Some(line) => self.keep(line).await.map(|new| json!({ "ok": true, "new": new })),
                None => Err("append takes a line".to_string()),
            },
            Some("search") => self.search(request).await.map(|keys| json!({ "ok": true, "keys": keys })),
            Some("ping") => Ok(json!({ "ok": true })),
            Some("stop") => {
                self.leave.notify_one();
                Ok(json!({ "ok": true }))
            }
            _ => Err("the keeper does not know that".to_string()),
        };
        done.unwrap_or_else(|error| json!({ "ok": false, "error": error }))
    }

    async fn talk(self: Arc<Self>, stream: UnixStream) {
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            self.spoken.store(self.started.elapsed().as_secs(), Ordering::Relaxed);
            let answer = match serde_json::from_str::<Value>(&line) {
                Ok(request) => self.answer(&request).await,
                Err(_) => json!({ "ok": false, "error": "a request is one line of JSON" }),
            };
            if write.write_all(format!("{answer}\n").as_bytes()).await.is_err() {
                break;
            }
        }
    }
}

/// Lines from an export or from another machine: everything under
/// `<home>/log` is taken in, once per event.
async fn take_in(keeper: &Keeper, home: &Path) {
    let Ok(entries) = std::fs::read_dir(home.join("log")) else { return };
    let (mut new, mut bad) = (0, 0);
    for path in entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
    {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        for line in String::from_utf8_lossy(&bytes).lines().filter(|l| !l.trim().is_empty()) {
            match keeper.keep(line).await {
                Ok(true) => new += 1,
                Ok(false) => {}
                Err(_) => bad += 1,
            }
        }
    }
    if new > 0 || bad > 0 {
        eprintln!("took in {new} new line(s) from log/, left out {bad} it could not read");
    }
}

async fn run(home: &Path) -> Result<(), String> {
    std::fs::create_dir_all(home.join("local")).map_err(text)?;
    // Opening the database is what makes this the only keeper: a second
    // one is refused here and leaves.
    let db = surrealdb::engine::any::connect(format!("surrealkv://{}", home.join("db").display()))
        .await
        .map_err(text)?;
    db.use_ns("nahuali").use_db("memory").await.map_err(text)?;
    db.query(SCHEMA).await.map_err(text)?.check().map_err(text)?;
    let keeper = Arc::new(Keeper {
        db,
        held: Mutex::new(Held::default()),
        started: Instant::now(),
        spoken: AtomicU64::new(0),
        leave: Notify::new(),
        stamp: remote::stamp(),
    });
    {
        let mut held = keeper.held.lock().await;
        held.events = keeper
            .lines()
            .await?
            .iter()
            .filter_map(|line| match Event::read(line) {
                Line::Event(event) => Some(*event),
                _ => None,
            })
            .collect();
        keeper.index(&mut held).await?;
    }
    take_in(&keeper, home).await;

    let socket = remote::socket(home);
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).map_err(text)?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).map_err(text)?;
    }
    let idle = std::env::var("NAHUALI_IDLE_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(IDLE);
    eprintln!("keeping {} (leaves after {idle} s of silence)", home.display());
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                if let Ok((stream, _)) = accepted {
                    tokio::spawn(keeper.clone().talk(stream));
                }
            }
            () = keeper.leave.notified() => break,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                if keeper.started.elapsed().as_secs().saturating_sub(keeper.spoken.load(Ordering::Relaxed)) >= idle {
                    break;
                }
            }
        }
    }
    let _ = std::fs::remove_file(&socket);
    // Let the answer to whoever said "stop" reach them.
    tokio::time::sleep(Duration::from_millis(50)).await;
    eprintln!("left");
    Ok(())
}

pub fn serve(home: &Path) -> Result<(), String> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(text)?.block_on(run(home))
}
