//! The keeper: the one process that has the database open (`nahuali serve`).
//!
//! It holds an embedded SurrealDB under `<home>/db`, answers the commands
//! of this user over a socket, and leaves when nobody has spoken to it for
//! a while. Every event is a record of the table `event`, kept whole as the
//! line it was written as and with its marks as fields to query by.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use nahuali_core::event::{Event, Line, What};
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Notify;

use crate::remote;

/// How long the keeper stays with nobody speaking to it.
const IDLE: u64 = 600;

const SCHEMA: &str = "
DEFINE TABLE IF NOT EXISTS event SCHEMALESS;
DEFINE INDEX IF NOT EXISTS event_project ON event FIELDS project;
DEFINE INDEX IF NOT EXISTS event_at ON event FIELDS at;
";

struct Keeper {
    db: Surreal<Any>,
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
            What::Unknown => "unknown",
        };
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
        Ok(!kept.is_empty())
    }

    async fn lines(&self) -> Result<Vec<String>, String> {
        let mut answer = self.db.query("SELECT VALUE line FROM event ORDER BY id ASC").await.map_err(text)?;
        answer.take(0).map_err(text)
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

/// Lines written before there was a database, or copied in from another
/// machine: everything under `<home>/log` is taken in, once per line.
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
        started: Instant::now(),
        spoken: AtomicU64::new(0),
        leave: Notify::new(),
        stamp: remote::stamp(),
    });
    take_in(&keeper, home).await;

    let socket = remote::socket(home);
    let _ = std::fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket).map_err(text)?;
    #[cfg(unix)]
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
