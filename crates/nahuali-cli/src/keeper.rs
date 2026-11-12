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
//! over their words. It is what `recall` searches. When a model is installed
//! each thing also carries a vector, so what is said in other words can be
//! found too. `vector` remembers the vectors already computed, by the text
//! they are of, so a keeper that starts again computes only what is new.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use nahuali_core::State;
use nahuali_core::event::{Event, Line, What};
use nahuali_embed::testing::WordEmbedder;
use nahuali_embed::{DEFAULT_MODEL, Embedder, LocalEmbedder};
use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, Notify};

use crate::node::{self, Node};
use crate::remote;
use crate::team::Server;

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
DEFINE TABLE IF NOT EXISTS vector SCHEMALESS;
DEFINE INDEX IF NOT EXISTS vector_of ON vector FIELDS of;
DELETE thing;
";

/// What the keeper knows beyond the database: the events in memory, and
/// which things it has indexed and in what shape.
#[derive(Default)]
struct Held {
    events: Vec<Event>,
    indexed: HashMap<String, u64>,
    /// Vectors already computed, by the model and the text they are of.
    vectors: HashMap<u64, Vec<f32>>,
    /// How many lines the events' database held at the last look.
    seen: usize,
}

fn read_events(lines: &[String]) -> Vec<Event> {
    lines
        .iter()
        .filter_map(|line| match Event::read(line) {
            Line::Event(event) => Some(*event),
            _ => None,
        })
        .collect()
}

/// How alike two texts have to be for one to be offered for the other.
/// Measured with `cargo run -p nahuali-embed --example near`: at this line
/// eight of nine things offered were answers, and half the answers that
/// shared no word were found. `NAHUALI_NEAR` moves it.
const NEAR: f64 = 0.65;

/// The model that turns text into vectors, when there is one.
fn meaning(home: &Path) -> Option<Arc<dyn Embedder>> {
    match std::env::var("NAHUALI_MEANING").as_deref() {
        Ok("off") => None,
        Ok("test") => Some(Arc::new(WordEmbedder)),
        _ => LocalEmbedder::open(&home.join("models"), DEFAULT_MODEL)
            .ok()
            .map(|m| Arc::new(m) as Arc<dyn Embedder>),
    }
}

/// A vector as SurrealQL writes it.
fn written(vector: &[f32]) -> String {
    let numbers: Vec<String> = vector.iter().map(|x| format!("{x:.6}")).collect();
    format!("[{}]", numbers.join(","))
}

fn of(model: &str, words: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (model, words).hash(&mut hasher);
    hasher.finish()
}

fn reach(scope: Option<&str>) -> &'static str {
    match scope {
        Some("everything") => "true",
        Some("all") => "(private = false OR project = $project)",
        _ => "(project = $project OR (everywhere = true AND private = false))",
    }
}

struct Keeper {
    /// The database on this machine: what is derived, and the events too
    /// unless a team's server holds them.
    db: Surreal<Any>,
    /// Where the events are: here, or on the team's server.
    events: Surreal<Any>,
    /// Who this installation is on the team's server, when there is one.
    member: Option<String>,
    held: Mutex<Held>,
    started: Instant,
    /// Seconds after `started` when someone last spoke.
    spoken: AtomicU64,
    leave: Notify,
    stamp: u64,
    meaning: Option<Arc<dyn Embedder>>,
    /// Rung when there may be vectors to compute.
    behind: Notify,
    /// The other machines of this memory, when there are any.
    node: Option<Node>,
    /// How the last meeting with each of them went.
    met: Mutex<HashMap<String, Met>>,
}

/// The stack of each of the keeper's threads.
const STACK: usize = 32 * 1024 * 1024;

/// How a meeting went: how many events were taken and given, or why not.
type Met = Result<(usize, usize), String>;

/// A meeting with another machine takes no longer than this.
const MEETING: Duration = Duration::from_secs(30);
/// How often this machine goes to the others.
const EVERY: u64 = 30;

/// The id an event line carries, without reading the rest of it.
fn id_of(line: &str) -> Option<String> {
    match Event::read(line) {
        Line::Event(event) => Some(event.id),
        _ => None,
    }
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
        // Whose it is: what the line says, or whoever brings in a line
        // from before people were told apart.
        let person = event.person.clone().or_else(|| self.member.clone()).unwrap_or_default();
        let mut answer = self
            .events
            .query(
                "INSERT IGNORE INTO event { id: $id, at: <datetime> $at, project: $project, by: $by, \
                 person: $person, mine: $mine, said_by_person: $said, verified: $verified, private: $private, \
                 kind: $kind, line: $line } RETURN VALUE id",
            )
            .bind(("id", event.id.clone()))
            .bind(("at", event.at.rfc3339()))
            .bind(("project", event.project.clone()))
            .bind(("by", event.by.name().to_string()))
            .bind(("person", person))
            .bind(("mine", event.mine))
            .bind(("said", event.by.is_person()))
            .bind(("verified", event.sure.is_verified()))
            .bind(("private", event.private))
            .bind(("kind", kind))
            .bind(("line", line.to_string()))
            .await
            .map_err(text)?;
        let kept: Vec<Value> = answer.take(0).map_err(text)?;
        if kept.is_empty() {
            // Already there, or the server would not have it.
            let mut there = self
                .events
                .query("SELECT VALUE id FROM event WHERE id = $id")
                .bind(("id", format!("event:{}", event.id)))
                .await
                .map_err(text)?;
            let there: Vec<Value> = there.take(0).unwrap_or_default();
            let mut by_key = self
                .events
                .query("SELECT VALUE line FROM event WHERE line = $line LIMIT 1")
                .bind(("line", line.to_string()))
                .await
                .map_err(text)?;
            let by_key: Vec<String> = by_key.take(0).unwrap_or_default();
            if there.is_empty() && by_key.is_empty() {
                let who = self.member.as_deref().unwrap_or("you");
                return Err(format!(
                    "the team's server refused it: on it you are {who}, and you write only in that name"
                ));
            }
            return Ok(false);
        }
        held.seen += 1;
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
            let vector = self
                .meaning
                .as_ref()
                .and_then(|m| held.vectors.get(&of(m.model(), &words)))
                .map_or_else(|| "NONE".to_string(), |v| written(v));
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (
                &words,
                &thing.marks.project,
                thing.marks.private,
                thing.everywhere,
                thing.marks.at.0,
                vector.len(),
            )
                .hash(&mut hasher);
            let shape = hasher.finish();
            seen.insert(thing.key.clone(), shape);
            if held.indexed.get(&thing.key) == Some(&shape) {
                continue;
            }
            self.db
                .query(format!(
                    "DELETE thing WHERE key = $key; \
                     CREATE thing CONTENT {{ key: $key, kind: $kind, text: $text, project: $project, \
                     private: $private, everywhere: $everywhere, at: <datetime> $at, vec: {vector} }}"
                ))
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
        if self.meaning.is_some() {
            self.behind.notify_one();
        }
        Ok(())
    }

    /// The texts held that have no vector yet, a few at a time.
    fn lacking(&self, held: &Held, most: usize) -> Vec<String> {
        let Some(model) = &self.meaning else { return Vec::new() };
        let mut lacking: Vec<String> = State::fold(&held.events)
            .things()
            .iter()
            .map(nahuali_core::Thing::text)
            .filter(|words| !held.vectors.contains_key(&of(model.model(), words)))
            .collect();
        lacking.sort();
        lacking.dedup();
        lacking.truncate(most);
        lacking
    }

    /// Compute the vectors still lacking, a few at a time, and bring the
    /// index up to them. The model runs away from whoever is waiting for
    /// an answer, and nobody is kept out for longer than one handful takes.
    /// Returns whether any is still lacking after `handfuls`.
    async fn catch_up(&self, handfuls: usize) -> Result<bool, String> {
        let Some(model) = self.meaning.clone() else { return Ok(false) };
        for _ in 0..handfuls {
            let texts = self.lacking(&*self.held.lock().await, 16);
            if texts.is_empty() {
                return Ok(false);
            }
            let (model_for, asked) = (model.clone(), texts.clone());
            let vectors = tokio::task::spawn_blocking(move || {
                let texts: Vec<&str> = asked.iter().map(String::as_str).collect();
                model_for.embed_passages(&texts).map_err(|e| e.to_string())
            })
            .await
            .map_err(text)??;
            let mut held = self.held.lock().await;
            for (words, vector) in texts.iter().zip(vectors) {
                let key = of(model.model(), words);
                self.db
                    .query(format!("CREATE vector CONTENT {{ of: $of, v: {} }}", written(&vector)))
                    .bind(("of", key.to_string()))
                    .await
                    .map_err(text)?
                    .check()
                    .map_err(text)?;
                held.vectors.insert(key, vector);
            }
            self.index(&mut held).await?;
        }
        Ok(!self.lacking(&*self.held.lock().await, 1).is_empty())
    }

    /// The things closest in meaning to what is asked, each with how alike
    /// it is, down to the line below which nothing is offered.
    async fn near(&self, request: &Value) -> Result<Vec<Value>, String> {
        let Some(model) = &self.meaning else { return Ok(Vec::new()) };
        // What was just written has to be findable by whoever asks next. A
        // memory that arrived all at once is caught up with behind.
        self.catch_up(4).await?;
        let asked = request["q"].as_str().unwrap_or_default().trim();
        if asked.is_empty() {
            return Ok(Vec::new());
        }
        let vector = model.embed_query(asked).map_err(text)?;
        // Whoever asks may ask for a higher line; the environment moves it for everyone.
        let line = std::env::var("NAHUALI_NEAR")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(request["line"].as_f64())
            .unwrap_or(NEAR);
        let kind = if request["kind"].is_string() { "AND kind = $kind" } else { "" };
        let query = format!(
            "SELECT key, vector::similarity::cosine(vec, {}) AS sim FROM thing WHERE vec != NONE AND {} {kind} \
             ORDER BY sim DESC LIMIT $limit",
            written(&vector),
            reach(request["scope"].as_str())
        );
        let mut answer = self
            .db
            .query(query)
            .bind(("project", request["project"].as_str().unwrap_or_default().to_string()))
            .bind(("kind", request["kind"].as_str().unwrap_or_default().to_string()))
            .bind(("limit", request["limit"].as_u64().unwrap_or(5).min(50)))
            .await
            .map_err(text)?;
        let rows: Vec<Value> = answer.take(0).map_err(text)?;
        Ok(rows
            .into_iter()
            .filter(|row| row["sim"].as_f64().is_some_and(|sim| sim >= line))
            .map(|row| json!([row["key"], row["sim"]]))
            .collect())
    }

    async fn lines(&self) -> Result<Vec<String>, String> {
        if self.member.is_none() {
            let mut answer =
                self.events.query("SELECT VALUE line FROM event ORDER BY id ASC").await.map_err(text)?;
            return answer.take(0).map_err(text);
        }
        // The server vouches for who wrote each record, not for what the
        // line inside it says. A line that names another person as its
        // own is not this team's line.
        let mut answer =
            self.events.query("SELECT person, line FROM event ORDER BY id ASC").await.map_err(text)?;
        let rows: Vec<Value> = answer.take(0).map_err(text)?;
        Ok(rows
            .iter()
            .filter_map(|row| {
                let (wrote, line) = (row["person"].as_str()?, row["line"].as_str()?);
                match Event::read(line) {
                    Line::Event(event) if event.person.as_deref().is_some_and(|said| said != wrote) => None,
                    _ => Some(line.to_string()),
                }
            })
            .collect())
    }

    /// Every event held, by its id.
    async fn by_id(&self) -> Result<HashMap<String, String>, String> {
        Ok(self.lines().await?.into_iter().filter_map(|line| id_of(&line).map(|id| (id, line))).collect())
    }

    /// Answer another machine of this memory: say which events are here,
    /// give the ones it asks for, take the ones it brings.
    async fn answer_peer(self: Arc<Self>, mut stream: tokio::net::TcpStream) {
        let Some(cipher) = self.node.as_ref().and_then(|n| n.cipher().ok()) else { return };
        // Whatever is not sealed with this memory's key ends the talk.
        while let Ok(request) = node::receive(&mut stream, &cipher).await {
            self.spoken.store(self.started.elapsed().as_secs(), Ordering::Relaxed);
            let answer = match request["op"].as_str() {
                Some("ids") => match self.by_id().await {
                    Ok(held) => json!({ "ids": held.keys().collect::<Vec<_>>() }),
                    Err(why) => json!({ "error": why }),
                },
                Some("give") => match self.by_id().await {
                    Ok(held) => {
                        let wanted =
                            request["ids"].as_array().into_iter().flatten().filter_map(Value::as_str);
                        json!({ "lines": wanted.filter_map(|id| held.get(id)).collect::<Vec<_>>() })
                    }
                    Err(why) => json!({ "error": why }),
                },
                Some("take") => {
                    let mut new = 0;
                    for line in request["lines"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                        new += usize::from(self.keep(line).await.unwrap_or(false));
                    }
                    json!({ "new": new })
                }
                _ => json!({ "error": "this machine does not know that" }),
            };
            if node::send(&mut stream, &cipher, &answer).await.is_err() {
                break;
            }
        }
    }

    /// Go to one machine: take the events it has and this one lacks, and
    /// give it the ones it lacks. Returns how many went each way.
    async fn meet(&self, addr: &str) -> Result<(usize, usize), String> {
        let cipher = self.node.as_ref().ok_or("this memory keeps to this machine")?.cipher()?;
        let mut stream = tokio::time::timeout(Duration::from_secs(3), tokio::net::TcpStream::connect(addr))
            .await
            .map_err(|_| "no answer in three seconds".to_string())?
            .map_err(|e| e.to_string())?;
        node::send(&mut stream, &cipher, &json!({ "op": "ids" })).await?;
        let theirs = node::receive(&mut stream, &cipher)
            .await
            .map_err(|_| "it does not share this memory's key".to_string())?;
        let theirs: std::collections::HashSet<String> = theirs["ids"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|id| id.as_str().map(str::to_string))
            .collect();
        let mine = self.by_id().await?;

        let wanted: Vec<&String> = theirs.iter().filter(|id| !mine.contains_key(*id)).collect();
        let mut took = 0;
        if !wanted.is_empty() {
            node::send(&mut stream, &cipher, &json!({ "op": "give", "ids": wanted })).await?;
            let given = node::receive(&mut stream, &cipher).await?;
            for line in given["lines"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                took += usize::from(self.keep(line).await.unwrap_or(false));
            }
        }
        let lacking: Vec<&String> =
            mine.iter().filter(|(id, _)| !theirs.contains(*id)).map(|(_, line)| line).collect();
        let mut gave = 0;
        if !lacking.is_empty() {
            node::send(&mut stream, &cipher, &json!({ "op": "take", "lines": lacking })).await?;
            gave = node::receive(&mut stream, &cipher).await?["new"].as_u64().unwrap_or(0) as usize;
        }
        Ok((took, gave))
    }

    /// Go to every machine this one knows, and remember how each went.
    async fn consolidate(&self) -> Value {
        let peers = self.node.as_ref().map(|n| n.peers.clone()).unwrap_or_default();
        for addr in &peers {
            let how = match tokio::time::timeout(MEETING, self.meet(addr)).await {
                Ok(how) => how,
                Err(_) => Err("it took too long".to_string()),
            };
            self.met.lock().await.insert(addr.clone(), how);
        }
        self.met_so_far().await
    }

    async fn met_so_far(&self) -> Value {
        let met = self.met.lock().await;
        let peers: Vec<Value> = self
            .node
            .iter()
            .flat_map(|n| &n.peers)
            .map(|addr| match met.get(addr) {
                Some(Ok((took, gave))) => json!({ "addr": addr, "took": took, "gave": gave }),
                Some(Err(why)) => json!({ "addr": addr, "error": why }),
                None => json!({ "addr": addr }),
            })
            .collect();
        json!({ "ok": true, "peers": peers })
    }

    /// Take what others wrote on the team's server since the last look.
    async fn sync(&self) -> Result<(), String> {
        let mut counted =
            self.events.query("SELECT VALUE count() FROM event GROUP ALL").await.map_err(text)?;
        let counted: Vec<u64> = counted.take(0).unwrap_or_default();
        let there = counted.first().copied().unwrap_or(0) as usize;
        let mut held = self.held.lock().await;
        if there == held.seen {
            return Ok(());
        }
        let lines = self.lines().await?;
        held.seen = lines.len();
        held.events = read_events(&lines);
        self.index(&mut held).await
    }

    /// The keys of the things that hold every word asked for, the best
    /// match first. What the asker may not see is never among them.
    async fn search(&self, request: &Value) -> Result<Vec<String>, String> {
        let words = request["q"].as_str().unwrap_or_default().trim().to_string();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let reach = reach(request["scope"].as_str());
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
            Some("near") => self
                .near(request)
                .await
                .map(|near| json!({ "ok": true, "near": near, "meaning": self.meaning.is_some() })),
            Some("consolidate") => Ok(self.consolidate().await),
            Some("node") => Ok(self.met_so_far().await),
            Some("health") => {
                let held = self.held.lock().await;
                Ok(json!({
                    "ok": true,
                    "events": held.events.len(),
                    "things": held.indexed.len(),
                    "lacking": self.lacking(&held, usize::MAX).len(),
                    "meaning": self.meaning.is_some(),
                    "up": self.started.elapsed().as_secs(),
                }))
            }
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

async fn run(home: &Path, stay: bool) -> Result<(), String> {
    std::fs::create_dir_all(home.join("local")).map_err(text)?;
    // Opening the database is what makes this the only keeper: a second
    // one is refused here and leaves.
    let db = surrealdb::engine::any::connect(format!("surrealkv://{}", home.join("db").display()))
        .await
        .map_err(text)?;
    db.use_ns("nahuali").use_db("memory").await.map_err(text)?;
    db.query(SCHEMA).await.map_err(text)?.check().map_err(text)?;
    // A team's events are on its server, signed in to as this person.
    let server = Server::read(home);
    let events = match &server {
        Some(server) => server.connect().await?,
        None => db.clone(),
    };
    let keeper = Arc::new(Keeper {
        db,
        events,
        member: server.as_ref().map(|s| s.name.clone()),
        held: Mutex::new(Held::default()),
        started: Instant::now(),
        spoken: AtomicU64::new(0),
        leave: Notify::new(),
        stamp: remote::stamp(),
        meaning: meaning(home),
        behind: Notify::new(),
        // A team's server already brings its machines together.
        node: if server.is_some() { None } else { Node::read(home) },
        met: Mutex::new(HashMap::new()),
    });
    {
        let mut held = keeper.held.lock().await;
        let mut kept = keeper.db.query("SELECT of, v FROM vector").await.map_err(text)?;
        let rows: Vec<Value> = kept.take(0).map_err(text)?;
        for row in rows {
            let (Some(of), Some(v)) =
                (row["of"].as_str().and_then(|s| s.parse::<u64>().ok()), row["v"].as_array())
            else {
                continue;
            };
            held.vectors.insert(of, v.iter().filter_map(|x| x.as_f64().map(|x| x as f32)).collect());
        }
        let lines = keeper.lines().await?;
        held.seen = lines.len();
        held.events = read_events(&lines);
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
    let idle = if stay {
        u64::MAX
    } else {
        std::env::var("NAHUALI_IDLE_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(IDLE)
    };
    if stay {
        eprintln!("keeping {} (stays until told to leave)", home.display());
    } else {
        eprintln!("keeping {} (leaves after {idle} s of silence)", home.display());
    }
    // The other machines of this memory: answer them here, and go to them.
    let answering = match keeper.node.as_ref().and_then(|n| n.listen.clone()) {
        Some(addr) => match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => {
                eprintln!("answering the other machines at {addr}");
                Some(listener)
            }
            Err(why) => {
                eprintln!("cannot answer the other machines at {addr}: {why}");
                None
            }
        },
        None => None,
    };
    if let Some(listener) = answering {
        let keeper = keeper.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(keeper.clone().answer_peer(stream));
            }
        });
    }
    // Vectors are computed behind whoever is being answered.
    if keeper.meaning.is_some() {
        let keeper = keeper.clone();
        tokio::spawn(async move {
            loop {
                keeper.behind.notified().await;
                loop {
                    match keeper.catch_up(8).await {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(why) => {
                            eprintln!("could not compute vectors: {why}");
                            break;
                        }
                    }
                }
            }
        });
    }
    let goes = keeper.node.as_ref().is_some_and(|n| !n.peers.is_empty());
    let mut seconds: u64 = EVERY - 1;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                if let Ok((stream, _)) = accepted {
                    tokio::spawn(keeper.clone().talk(stream));
                }
            }
            () = keeper.leave.notified() => break,
            () = tokio::time::sleep(Duration::from_secs(1)) => {
                if keeper.member.is_some()
                    && let Err(why) = keeper.sync().await
                {
                    eprintln!("could not look at the team's server: {why}");
                }
                seconds += 1;
                if goes && seconds.is_multiple_of(EVERY) {
                    let keeper = keeper.clone();
                    tokio::spawn(async move { keeper.consolidate().await });
                }
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

/// Be the keeper of the memory at `home`. With `stay` it never leaves by
/// itself: for a machine that is there for the others.
pub fn serve(home: &Path, stay: bool) -> Result<(), String> {
    // The database runs its queries deep: with the stack a thread gets by
    // default, an unoptimised build falls off the end of it.
    tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(STACK)
        .enable_all()
        .build()
        .map_err(text)?
        .block_on(run(home, stay))
}
