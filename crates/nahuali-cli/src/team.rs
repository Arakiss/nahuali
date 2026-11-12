//! Several people on one memory.
//!
//! The events of a team live in a SurrealDB server everyone reaches, and
//! the server itself keeps each person to their reach: it lets a person
//! read everything shared and what is only theirs, write only in their own
//! name, and change or delete nothing. No copy of Nahuali has to be
//! trusted for that to hold.
//!
//! Each machine still runs its own keeper. It signs in to the server as
//! its person, reads and writes events there, and keeps what is derived
//! (the search index, the vectors) in its own database on its own disk.
//!
//! ```text
//! nahuali team init URL          once, by whoever runs the server
//! nahuali team add NAME URL      a person, and the passphrase they join with
//! nahuali team join URL --name NAME
//! nahuali team                   where this memory is kept
//! nahuali team leave             back to the memory on this machine
//! ```

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::{Record, Root};

use crate::remote::Remote;
use crate::{Failure, env, parse, usage, words};

pub const NAMESPACE: &str = "nahuali";
pub const DATABASE: &str = "memory";
const ACCESS: &str = "member";

/// What the server holds, and who may do what with it. A person reads the
/// events that are shared and those that are only theirs; writes only in
/// their own name; and changes or deletes nothing.
const SCHEMA: &str = "
DEFINE TABLE IF NOT EXISTS person SCHEMAFULL PERMISSIONS FOR select WHERE id = $auth.id;
DEFINE FIELD IF NOT EXISTS name ON person TYPE string;
DEFINE FIELD IF NOT EXISTS pass ON person TYPE string;
DEFINE INDEX IF NOT EXISTS person_name ON person FIELDS name UNIQUE;
DEFINE ACCESS IF NOT EXISTS member ON DATABASE TYPE RECORD
  SIGNIN ( SELECT * FROM person WHERE name = $name AND crypto::argon2::compare(pass, $pass) )
  DURATION FOR TOKEN 1h, FOR SESSION 30d;
DEFINE TABLE IF NOT EXISTS event SCHEMALESS PERMISSIONS
  FOR select WHERE mine = false OR person = $auth.name
  FOR create WHERE person = $auth.name
  FOR update, delete NONE;
DEFINE INDEX IF NOT EXISTS event_project ON event FIELDS project;
DEFINE INDEX IF NOT EXISTS event_at ON event FIELDS at;
";

/// Where a team's memory is, and who this installation is on it. Kept in
/// `<home>/local/server.json`, readable by this user alone.
#[derive(Clone, Debug)]
pub struct Server {
    pub url: String,
    pub name: String,
    pub pass: String,
}

fn file(home: &Path) -> PathBuf {
    home.join("local").join("server.json")
}

impl Server {
    pub fn read(home: &Path) -> Option<Self> {
        let value: Value = serde_json::from_str(&std::fs::read_to_string(file(home)).ok()?).ok()?;
        Some(Self {
            url: value["url"].as_str()?.to_string(),
            name: value["name"].as_str()?.to_string(),
            pass: value["pass"].as_str()?.to_string(),
        })
    }

    fn write(&self, home: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::create_dir_all(home.join("local"))?;
        let path = file(home);
        let _ = std::fs::remove_file(&path);
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
        std::io::Write::write_all(
            &mut out,
            format!("{}\n", json!({ "url": self.url, "name": self.name, "pass": self.pass })).as_bytes(),
        )
    }

    /// Sign in as this person.
    pub async fn connect(&self) -> Result<Surreal<Any>, String> {
        let db = surrealdb::engine::any::connect(self.url.as_str())
            .await
            .map_err(|e| format!("{} does not answer: {e}", self.url))?;
        db.signin(Record {
            namespace: NAMESPACE.to_string(),
            database: DATABASE.to_string(),
            access: ACCESS.to_string(),
            params: json!({ "name": self.name, "pass": self.pass }),
        })
        .await
        .map_err(|_| {
            format!(
                "{} did not let `{}` in: the name or the passphrase is not the one it has",
                self.url, self.name
            )
        })?;
        db.use_ns(NAMESPACE).use_db(DATABASE).await.map_err(|e| e.to_string())?;
        Ok(db)
    }
}

fn refused(e: impl std::fmt::Display) -> Failure {
    Failure::Refused(e.to_string())
}

fn runtime() -> Result<tokio::runtime::Runtime, Failure> {
    tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(refused)
}

/// Sign in as whoever runs the server. Their name and passphrase come from
/// the environment, never from the command line, where others could read them.
async fn owner(url: &str) -> Result<Surreal<Any>, Failure> {
    let var = |name: &str| {
        std::env::var(name)
            .map_err(|_| usage(format!("set {name}: the server's own user is needed for this")))
    };
    let (username, password) = (var("NAHUALI_ROOT_USER")?, var("NAHUALI_ROOT_PASS")?);
    let db = surrealdb::engine::any::connect(url)
        .await
        .map_err(|e| Failure::Refused(format!("{url} does not answer: {e}")))?;
    db.signin(Root { username, password })
        .await
        .map_err(|_| Failure::Refused(format!("{url} did not accept the server's user")))?;
    db.query(format!("DEFINE NAMESPACE IF NOT EXISTS {NAMESPACE}; USE NS {NAMESPACE}; DEFINE DATABASE IF NOT EXISTS {DATABASE};"))
        .await
        .map_err(refused)?
        .check()
        .map_err(refused)?;
    db.use_ns(NAMESPACE).use_db(DATABASE).await.map_err(refused)?;
    Ok(db)
}

fn name(given: &str) -> Result<String, Failure> {
    if nahuali_core::id::is_slug(given) {
        Ok(given.to_string())
    } else {
        Err(usage(format!("`{given}` is not a name for a person: lower-case letters, digits and dashes")))
    }
}

/// A passphrase nobody chose: twenty characters drawn at random.
fn passphrase() -> String {
    const LETTERS: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut bits = env::entropy();
    (0..20)
        .map(|i| {
            if i % 8 == 7 {
                bits = env::entropy();
            }
            let c = LETTERS[(bits % LETTERS.len() as u128) as usize] as char;
            bits /= LETTERS.len() as u128;
            c
        })
        .collect()
}

pub fn team(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &["name"], &[])?;
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    match args.words.first().map(String::as_str) {
        None => Ok(match Server::read(&home) {
            Some(server) => {
                format!("this memory is the team's, at {}\nyou are {} on it\n", server.url, server.name)
            }
            None => "this memory is on this machine alone\n".to_string(),
        }),
        Some("init") => {
            let [_, url] =
                words(&args, "team init takes the server's address: nahuali team init ws://host:8000")?;
            runtime()?.block_on(async {
                let db = owner(&url).await?;
                db.query(SCHEMA).await.map_err(refused)?.check().map_err(refused)?;
                Ok(format!(
                    "{url} is ready to hold a team's memory\nadd people with: nahuali team add NAME {url}\n"
                ))
            })
        }
        Some("add") => {
            let [_, who, url] = words(
                &args,
                "team add takes a person and the server's address: nahuali team add bob ws://host:8000",
            )?;
            let who = name(&who)?;
            let pass = passphrase();
            runtime()?.block_on(async {
                let db = owner(&url).await?;
                let mut made = db
                    .query("CREATE person SET name = $name, pass = crypto::argon2::generate($pass) RETURN VALUE name")
                    .bind(("name", who.clone()))
                    .bind(("pass", pass.clone()))
                    .await
                    .map_err(refused)?;
                let made: Result<Vec<String>, _> = made.take(0);
                match made {
                    Ok(names) if !names.is_empty() => Ok(format!(
                        "{who} can join. Give them this, once, by a way you trust:\n\n  NAHUALI_PASS={pass} nahuali team join {url} --name {who}\n\nIt is not kept anywhere else.\n"
                    )),
                    _ => Err(Failure::Refused(format!("{who} is already on this team"))),
                }
            })
        }
        Some("join") => {
            let [_, url] = words(
                &args,
                "team join takes the server's address: nahuali team join ws://host:8000 --name ada",
            )?;
            let who =
                name(args.value("name").ok_or_else(|| usage("say who you are on the team: --name ada"))?)?;
            let pass = std::env::var("NAHUALI_PASS")
                .map_err(|_| usage("set NAHUALI_PASS to the passphrase you were given"))?;
            let server = Server { url: url.clone(), name: who.clone(), pass };
            runtime()?.block_on(server.connect()).map_err(Failure::Refused)?;
            server.write(&home).map_err(refused)?;
            // The keeper signs in when it starts.
            Remote::at(&home).stop();
            Ok(format!(
                "this memory is now the team's, at {url}, and you are {who} on it\nwhat was on this machine is still here: `nahuali team leave` goes back to it\n"
            ))
        }
        Some("leave") => {
            let [_] = words(&args, "team leave takes no words")?;
            if Server::read(&home).is_none() {
                return Ok("this memory is on this machine alone\n".to_string());
            }
            std::fs::remove_file(file(&home)).map_err(refused)?;
            Remote::at(&home).stop();
            Ok("back to the memory on this machine. The team's is untouched\n".to_string())
        }
        Some(other) => Err(usage(format!("`{other}` is not something team does: init, add, join or leave"))),
    }
}
