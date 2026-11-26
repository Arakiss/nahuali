//! The way to the keeper: the one process that has the database open.
//!
//! A database on disk is opened by one process at a time, and a memory is
//! used by several at once: the board stays open while agents write. So the
//! first command that needs the memory starts a keeper (`nahuali serve`),
//! every command talks to it over a socket only this user can reach, and
//! the keeper leaves when nobody has spoken to it for a while. Nothing is
//! installed and nothing asks for a password.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, UNIX_EPOCH};

use nahuali_core::Journal;
use serde_json::{Value, json};

/// How long a command waits for a keeper to answer before giving up.
const PATIENCE: Duration = Duration::from_secs(20);
const STEP: Duration = Duration::from_millis(50);
/// Steps between two starts of a keeper by one command.
const RESTART: u32 = 10;

/// Where the keeper of a memory listens. Socket paths are short by law, so
/// it is not inside the memory: it is named after it, in the user's own
/// temporary directory.
pub fn socket(home: &Path) -> PathBuf {
    let full = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
    // FNV-1a: the same in every build, which a keeper and a newer command need.
    let hash = full
        .to_string_lossy()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    std::env::temp_dir().join(format!("nahuali-{hash:016x}.sock"))
}

/// Whether this process had to start a keeper: the first command after a
/// quiet while pays for opening the database, and its use says so.
pub static STARTED: AtomicBool = AtomicBool::new(false);

/// Which build this is: a keeper started by an older build steps aside.
pub fn stamp() -> u64 {
    std::env::current_exe()
        .and_then(std::fs::metadata)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

#[derive(Clone, Debug)]
pub struct Remote {
    home: PathBuf,
    patience: Duration,
    hasty: bool,
}

impl Remote {
    pub fn at(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into(), patience: PATIENCE, hasty: false }
    }

    /// Never wait for a keeper to start: when none answers, start one and
    /// give up at once, so that the next call finds it. For a hook, which
    /// runs before what an agent does and must not hold it up.
    pub fn hasty(mut self) -> Self {
        self.hasty = true;
        self
    }

    /// Wait at most this long for a keeper: for callers that must not hold
    /// anyone up.
    pub fn patient(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }

    /// The keys of what holds every word of `words`, the best match first,
    /// as far as `scope` (`project`, `all` or `everything`) reaches.
    pub fn search(&self, words: &str, scope: &str, project: &str, limit: u64) -> io::Result<Vec<String>> {
        if !self.exists() {
            return Ok(Vec::new());
        }
        let answer =
            self.call("search", json!({ "q": words, "scope": scope, "project": project, "limit": limit }))?;
        Ok(answer["keys"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|k| k.as_str().map(str::to_string))
            .collect())
    }

    /// What is closest in meaning to `asked`, each with how alike it is,
    /// and whether this memory has a model to tell at all.
    pub fn near(
        &self,
        asked: &str,
        scope: &str,
        project: &str,
        kind: Option<&str>,
        limit: u64,
        line: Option<f64>,
    ) -> io::Result<(Vec<(String, f64)>, bool)> {
        if !self.exists() {
            return Ok((Vec::new(), false));
        }
        let mut request = json!({ "q": asked, "scope": scope, "project": project, "limit": limit });
        if let Some(kind) = kind {
            request["kind"] = json!(kind);
        }
        if let Some(line) = line {
            request["line"] = json!(line);
        }
        let answer = self.call("near", request)?;
        let near = answer["near"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|pair| Some((pair[0].as_str()?.to_string(), pair[1].as_f64()?)))
            .collect();
        Ok((near, answer["meaning"] == true))
    }

    /// A memory nobody has written to has no database and needs no keeper.
    pub fn exists(&self) -> bool {
        // A memory kept on a team's server exists even before this machine wrote to it.
        self.home.join("db").exists()
            || self.home.join("log").exists()
            || self.home.join("local").join("server.json").exists()
    }

    fn start(&self) -> io::Result<Child> {
        STARTED.store(true, Ordering::Relaxed);
        std::fs::create_dir_all(self.home.join("local"))?;
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.home.join("local").join("keeper.log"))?;
        Command::new(std::env::current_exe()?)
            .arg("serve")
            .arg("--home")
            .arg(&self.home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            // Its own group: closing the command that started it does not stop it.
            .process_group(0)
            .spawn()
    }

    fn once(&self, request: &Value) -> io::Result<Value> {
        let mut stream = UnixStream::connect(socket(&self.home))?;
        stream.set_read_timeout(Some(self.patience))?;
        stream.write_all(format!("{request}\n").as_bytes())?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line)?;
        serde_json::from_str(&line)
            .map_err(|e| io::Error::other(format!("the keeper answered something else: {e}")))
    }

    /// Ask the keeper, starting one if none answers.
    pub fn call(&self, op: &str, mut request: Value) -> io::Result<Value> {
        request["op"] = json!(op);
        request["stamp"] = json!(stamp());
        let tries = (self.patience.as_millis() / STEP.as_millis()).max(1) as u32;
        let mut started: Option<(Child, u32)> = None;
        for attempt in 0..tries {
            match self.once(&request) {
                // An older keeper is leaving; a hook does not wait for it.
                Ok(answer) if answer["stale"] == true && self.hasty => return Err(waking()),
                Ok(answer) if answer["stale"] == true => {}
                Ok(answer) if answer["ok"] == true => return Ok(answer),
                Ok(answer) => {
                    return Err(io::Error::other(
                        answer["error"].as_str().unwrap_or("the keeper refused").to_string(),
                    ));
                }
                // Nobody there, or it is still opening the database. The
                // keeper started here is started again once it has left,
                // as it does when the database is still held by one that is
                // leaving, and at most every half second.
                Err(_) => {
                    let again = started.as_mut().is_none_or(|(child, at)| {
                        attempt - *at >= RESTART && child.try_wait().ok().flatten().is_some()
                    });
                    if again {
                        started = Some((self.start()?, attempt));
                    }
                    if self.hasty {
                        return Err(waking());
                    }
                }
            }
            std::thread::sleep(STEP);
        }
        Err(io::Error::other(format!(
            "no keeper answered for this memory; what it said is in {}",
            self.home.join("local").join("keeper.log").display()
        )))
    }

    /// Tell the keeper to leave, if there is one, and wait until it has.
    /// It answers before it goes: without the wait, what is asked next can
    /// reach the keeper that is leaving, which still reads the memory as it
    /// was set up before, instead of a new one.
    pub fn stop(&self) -> bool {
        if self.once(&json!({ "op": "stop", "stamp": stamp() })).is_err() {
            return false;
        }
        let deadline = Instant::now() + self.patience;
        while UnixStream::connect(socket(&self.home)).is_ok() && Instant::now() < deadline {
            std::thread::sleep(STEP);
        }
        true
    }
}

/// What a hasty call says when it started a keeper instead of waiting.
fn waking() -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, "the keeper is starting")
}

/// Whether a call gave up because the keeper was starting.
pub fn is_waking(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock
}

impl Journal for Remote {
    fn read(&self) -> io::Result<Vec<String>> {
        if !self.exists() {
            return Ok(Vec::new());
        }
        let answer = self.call("read", json!({}))?;
        let lines = answer["lines"].as_array().ok_or_else(|| io::Error::other("the keeper sent no lines"))?;
        Ok(lines.iter().filter_map(|l| l.as_str().map(str::to_string)).collect())
    }

    fn append(&mut self, line: &str) -> io::Result<()> {
        self.call("append", json!({ "line": line })).map(|_| ())
    }
}
