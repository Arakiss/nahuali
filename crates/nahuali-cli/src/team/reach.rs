//! The team's rules, held by the server and by nothing else.
//!
//! Every way a client signed in to the server can reach an event or a
//! person is tried against a throwaway SurrealDB that holds [`SCHEMA`],
//! straight through the client library and never through Nahuali, as a
//! changed copy would. What the server holds afterwards, read as its own
//! user, is compared with the rules written out again here by hand, so the
//! schema is never its own oracle.
//!
//! Each plant takes one protection away from the schema, or adds one too
//! many, and must be missed by the rule it breaks and by no other: a matrix
//! that a schema refusing everything, or allowing everything, still
//! passes is not a check.

use serde_json::{Value, json};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::{Record, Root};

use super::{ACCESS, NAMESPACE, SCHEMA};

/// The rules of the README's table, and the one it leaves to the server's
/// own user: who is on the team.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    /// A person reads what is shared and what is only theirs; nobody else reads.
    Reads,
    /// A person writes only in their own name, and can write in it.
    OwnName,
    /// Nothing written is changed or deleted, by anyone signed in.
    Unchanged,
    /// A person sees their own record on the team, and nobody else's.
    People,
    /// Nobody but the server's user adds, changes or removes a person.
    Membership,
}

/// Who is on the other end: a person of the team, or a connection that
/// never signed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Who {
    Person(&'static str),
    Nobody,
}

impl Who {
    fn is(self, name: &str) -> bool {
        matches!(self, Who::Person(n) if n == name)
    }
}

const PEOPLE: [&str; 2] = ["ada", "bob"];
const ROOT: (&str, &str) = ("root", "test-root");

/// What came out different from what the rules say.
#[derive(Debug)]
struct Miss {
    rule: Rule,
    case: String,
    wanted: bool,
}

struct Matrix {
    root: Surreal<Any>,
    misses: Vec<Miss>,
    /// How many cases were tried.
    cases: usize,
    /// Keys of events and people made during the run, so no two collide.
    made: u32,
}

impl Matrix {
    fn expect(&mut self, rule: Rule, case: String, wanted: bool, got: bool) {
        self.cases += 1;
        if wanted != got {
            self.misses.push(Miss { rule, case, wanted });
        }
    }

    fn key(&mut self, what: &str) -> String {
        self.made += 1;
        format!("{what}_{}", self.made)
    }

    async fn rows(&self, query: &str) -> Vec<Value> {
        self.root.query(query).await.unwrap().take(0).unwrap()
    }

    /// An event, written as the server's user.
    async fn event(&mut self, person: &str, mine: bool) -> String {
        let key = self.key(&format!("{person}_{}", if mine { "mine" } else { "shared" }));
        self.root
            .query(format!(
                "CREATE event:{key} CONTENT {{ key: '{key}', person: '{person}', mine: {mine}, project: 'atlas', line: 'as written' }}"
            ))
            .await
            .unwrap()
            .check()
            .unwrap();
        key
    }

    async fn everything(&self) -> Value {
        json!({
            "event": self.rows("SELECT * FROM event ORDER BY key").await,
            "person": self.rows("SELECT * FROM person ORDER BY name").await,
        })
    }
}

/// A SurrealDB server in memory, gone when the test ends; `None` when no
/// `surreal` is installed here.
struct Server {
    child: std::process::Child,
    url: String,
}

impl Server {
    fn start() -> Option<Self> {
        let port = std::net::TcpListener::bind("127.0.0.1:0").ok()?.local_addr().ok()?.port();
        let child = std::process::Command::new("surreal")
            .args(["start", "memory", "--bind", &format!("127.0.0.1:{port}")])
            .args(["--user", ROOT.0, "--pass", ROOT.1, "--log", "error"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        let server = Self { child, url: format!("ws://127.0.0.1:{port}") };
        for _ in 0..100 {
            if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(300));
                return Some(server);
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        None
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn pass(name: &str) -> String {
    format!("{name}-passphrase")
}

/// A database of its own on `server`, holding `schema`, with ada and bob on it.
async fn team(server: &Server, database: &str, schema: &str) -> Surreal<Any> {
    let root = surrealdb::engine::any::connect(server.url.as_str()).await.unwrap();
    root.signin(Root { username: ROOT.0.to_string(), password: ROOT.1.to_string() }).await.unwrap();
    root.query(format!(
        "DEFINE NAMESPACE IF NOT EXISTS {NAMESPACE}; USE NS {NAMESPACE}; DEFINE DATABASE {database};"
    ))
    .await
    .unwrap()
    .check()
    .unwrap();
    root.use_ns(NAMESPACE).use_db(database).await.unwrap();
    root.query(schema).await.unwrap().check().unwrap();
    for name in PEOPLE {
        root.query("CREATE person SET name = $name, pass = crypto::argon2::generate($pass)")
            .bind(("name", name))
            .bind(("pass", pass(name)))
            .await
            .unwrap()
            .check()
            .unwrap();
    }
    root
}

async fn client(server: &Server, database: &str, who: Who) -> Surreal<Any> {
    let db = surrealdb::engine::any::connect(server.url.as_str()).await.unwrap();
    if let Who::Person(name) = who {
        db.signin(Record {
            namespace: NAMESPACE.to_string(),
            database: database.to_string(),
            access: ACCESS.to_string(),
            params: json!({ "name": name, "pass": pass(name) }),
        })
        .await
        .unwrap();
    }
    db.use_ns(NAMESPACE).use_db(database).await.unwrap();
    db
}

/// What `who` is given back, or nothing when the server refuses outright.
async fn given(db: &Surreal<Any>, query: &str) -> Vec<Value> {
    match db.query(query).await {
        Ok(mut answer) => answer.take(0).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Run every case against the team on `database` and return what the
/// rules did not get.
async fn run(server: &Server, database: &str, root: Surreal<Any>) -> Vec<Miss> {
    let mut m = Matrix { root, misses: Vec::new(), cases: 0, made: 0 };
    let everyone = [Who::Person("ada"), Who::Person("bob"), Who::Nobody];

    // What each reads. Reads go first, before any write could have changed
    // what there is to read.
    let mut seeded = Vec::new();
    for person in PEOPLE {
        for mine in [false, true] {
            seeded.push((m.event(person, mine).await, person, mine));
        }
    }
    for who in everyone {
        let db = client(server, database, who).await;
        let listed: Vec<String> = given(&db, "SELECT VALUE key FROM event")
            .await
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        let counted = given(&db, "SELECT VALUE count() FROM event GROUP ALL").await;
        let counted = counted.first().and_then(Value::as_u64).unwrap_or(0);
        let mut seen = 0;
        for (key, author, mine) in &seeded {
            let wanted = matches!(who, Who::Person(_)) && (!mine || who.is(author));
            seen += u64::from(wanted);
            let what = format!("{who:?} reads {author}'s {} event", if *mine { "own" } else { "shared" });
            m.expect(Rule::Reads, format!("{what}, listing them all"), wanted, listed.contains(key));
            let by_id = !given(&db, &format!("SELECT * FROM event:{key}")).await.is_empty();
            m.expect(Rule::Reads, format!("{what}, by its id"), wanted, by_id);
        }
        m.expect(Rule::Reads, format!("{who:?} counts {seen} events"), true, counted == seen);
        for name in PEOPLE {
            let listed = given(&db, "SELECT VALUE name FROM person").await.contains(&json!(name));
            let by_id = !given(&db, &format!("SELECT * FROM person WHERE name = '{name}'")).await.is_empty();
            let wanted = who.is(name);
            m.expect(Rule::People, format!("{who:?} sees {name}'s record, listing them"), wanted, listed);
            m.expect(Rule::People, format!("{who:?} sees {name}'s record, by name"), wanted, by_id);
        }
    }

    // What each writes, by every way there is to write a new event.
    for who in everyone {
        let db = client(server, database, who).await;
        for person in PEOPLE {
            for mine in [false, true] {
                for (how, template) in [
                    ("CREATE", "CREATE event:{k} CONTENT {c}"),
                    ("INSERT", "INSERT INTO event {c}"),
                    ("INSERT IGNORE", "INSERT IGNORE INTO event {c}"),
                    ("UPSERT", "UPSERT event:{k} CONTENT {c}"),
                ] {
                    let k = m.key("new");
                    let c = format!(
                        "{{ id: event:{k}, key: '{k}', person: '{person}', mine: {mine}, project: 'atlas', line: 'new' }}"
                    );
                    let c = if how.starts_with("INSERT") {
                        c
                    } else {
                        c.replacen(&format!("id: event:{k}, "), "", 1)
                    };
                    let _ = db.query(template.replace("{k}", &k).replace("{c}", &c)).await;
                    let kept = !m.rows(&format!("SELECT * FROM event:{k}")).await.is_empty();
                    let wanted = who.is(person);
                    // UPSERT asks for the right to update as well as to
                    // create, and that is nobody's: refusing it even in one's
                    // own name breaks no rule, and Nahuali writes with INSERT.
                    if how == "UPSERT" && wanted && !kept {
                        continue;
                    }
                    m.expect(
                        Rule::OwnName,
                        format!("{who:?} writes as {person} (mine: {mine}) with {how}"),
                        wanted,
                        kept,
                    );
                }
            }
        }
        // An event that names nobody is nobody's to write.
        let k = m.key("nameless");
        let _ = db
            .query(format!("CREATE event:{k} CONTENT {{ key: '{k}', mine: false, line: 'from no one' }}"))
            .await;
        let kept = !m.rows(&format!("SELECT * FROM event:{k}")).await.is_empty();
        m.expect(Rule::OwnName, format!("{who:?} writes an event in nobody's name"), false, kept);
    }

    // What each changes: every event they can or cannot see, every way.
    for who in everyone {
        let db = client(server, database, who).await;
        for person in PEOPLE {
            for mine in [false, true] {
                for (how, template) in [
                    ("UPDATE its line", "UPDATE event:{k} SET line = 'changed'"),
                    ("UPDATE whose it is", "UPDATE event:{k} SET mine = !mine"),
                    ("UPDATE who wrote it", "UPDATE event:{k} SET person = 'someone-else'"),
                    ("MERGE", "UPDATE event:{k} MERGE { line: 'merged' }"),
                    ("UPSERT", "UPSERT event:{k} SET line = 'upserted'"),
                    (
                        "INSERT over it",
                        "INSERT INTO event { id: event:{k}, line: 'over' } ON DUPLICATE KEY UPDATE line = 'over'",
                    ),
                    ("DELETE", "DELETE event:{k}"),
                ] {
                    let k = m.event(person, mine).await;
                    let before = m.rows(&format!("SELECT * FROM event:{k}")).await;
                    let _ = db.query(template.replace("{k}", &k)).await;
                    let after = m.rows(&format!("SELECT * FROM event:{k}")).await;
                    let what = format!("{who:?} changes {person}'s event (mine: {mine}) with {how}");
                    m.expect(Rule::Unchanged, what, true, before == after);
                }
            }
        }
        let before = m.everything().await;
        for all in ["UPDATE event SET line = 'all changed'", "DELETE event", "REMOVE TABLE event"] {
            let _ = db.query(all).await;
            let after = m.everything().await;
            m.expect(
                Rule::Unchanged,
                format!("{who:?} runs `{all}`"),
                true,
                before["event"] == after["event"],
            );
        }
    }

    // Who is on the team: only the server's user says.
    for who in everyone {
        let db = client(server, database, who).await;
        for name in PEOPLE {
            for (how, query) in [
                // The passphrase first: a rename that got through would
                // leave nothing by that name for the next case to try.
                ("sets the passphrase of", format!("UPDATE person SET pass = 'known' WHERE name = '{name}'")),
                ("renames", format!("UPDATE person SET name = '{name}-renamed' WHERE name = '{name}'")),
                ("removes", format!("DELETE person WHERE name = '{name}'")),
            ] {
                let before = m.everything().await;
                let _ = db.query(query).await;
                let after = m.everything().await;
                m.expect(
                    Rule::Membership,
                    format!("{who:?} {how} {name}"),
                    true,
                    before["person"] == after["person"],
                );
            }
        }
        let k = m.key("carol");
        let before = m.everything().await;
        let _ = db
            .query(format!("CREATE person SET name = '{k}', pass = crypto::argon2::generate('known')"))
            .await;
        let signed_up = surrealdb::engine::any::connect(server.url.as_str()).await.unwrap();
        let _ = signed_up
            .signup(Record {
                namespace: NAMESPACE.to_string(),
                database: database.to_string(),
                access: ACCESS.to_string(),
                params: json!({ "name": format!("{k}-signed-up"), "pass": "known" }),
            })
            .await;
        let after = m.everything().await;
        m.expect(
            Rule::Membership,
            format!("{who:?} adds a person, or signs one up"),
            true,
            before["person"] == after["person"],
        );
    }
    eprintln!("{database}: {} cases, {} missed", m.cases, m.misses.len());
    m.misses
}

fn told(misses: &[Miss]) -> String {
    misses
        .iter()
        .map(|miss| {
            format!(
                "  {:?}: {} should {}",
                miss.rule,
                miss.case,
                if miss.wanted { "succeed" } else { "fail" }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Each plant: what it does to the schema, the text it changes, what it
/// changes it to, and the one rule that must miss it.
fn plants() -> Vec<(&'static str, &'static str, &'static str, Rule)> {
    let signup: &str =
        "SIGNUP ( CREATE person SET name = $name, pass = crypto::argon2::generate($pass) )\n  SIGNIN (";
    vec![
        (
            "anyone reads everything",
            "FOR select WHERE mine = false OR person = $auth.name",
            "FOR select FULL",
            Rule::Reads,
        ),
        (
            "only one's own is read",
            "WHERE mine = false OR person",
            "WHERE mine = false AND person",
            Rule::Reads,
        ),
        (
            "shared is read by all, own by nobody",
            "mine = false OR person = $auth.name",
            "mine = false",
            Rule::Reads,
        ),
        (
            "anyone writes in any name",
            "FOR create WHERE person = $auth.name",
            "FOR create FULL",
            Rule::OwnName,
        ),
        ("nobody writes at all", "FOR create WHERE person = $auth.name", "FOR create NONE", Rule::OwnName),
        (
            "what is written may change",
            "FOR update, delete NONE",
            "FOR update, delete WHERE person = $auth.name",
            Rule::Unchanged,
        ),
        (
            "everyone's record is seen",
            "PERMISSIONS FOR select WHERE id = $auth.id",
            "PERMISSIONS FOR select FULL",
            Rule::People,
        ),
        (
            "a person changes their own record",
            "PERMISSIONS FOR select WHERE id = $auth.id",
            "PERMISSIONS FOR select, update WHERE id = $auth.id",
            Rule::Membership,
        ),
        ("anyone signs up", "SIGNIN (", signup, Rule::Membership),
    ]
}

fn planted(what: &str, from: &str, to: &str) -> String {
    assert_eq!(SCHEMA.matches(from).count(), 1, "plant `{what}` no longer finds `{from}` in the schema");
    SCHEMA.replacen(from, to, 1)
}

#[tokio::test(flavor = "multi_thread")]
async fn the_server_keeps_every_rule() {
    let Some(server) = Server::start() else {
        assert!(
            std::env::var_os("CI").is_none(),
            "no `surreal` on PATH, and in CI the team's tests may not be skipped"
        );
        eprintln!("no `surreal` on PATH: the team's rules were not checked");
        return;
    };
    let misses = run(&server, "kept", team(&server, "kept", SCHEMA).await).await;
    assert!(misses.is_empty(), "the schema does not keep the rules:\n{}", told(&misses));
}

#[tokio::test(flavor = "multi_thread")]
async fn each_protection_taken_away_is_missed_by_its_rule_alone() {
    let Some(server) = Server::start() else {
        assert!(
            std::env::var_os("CI").is_none(),
            "no `surreal` on PATH, and in CI the team's tests may not be skipped"
        );
        eprintln!("no `surreal` on PATH: the team's rules were not checked");
        return;
    };
    for (n, (what, from, to, rule)) in plants().into_iter().enumerate() {
        let database = format!("plant{n}");
        let misses = run(&server, &database, team(&server, &database, &planted(what, from, to)).await).await;
        assert!(!misses.is_empty(), "plant `{what}` went unseen");
        assert!(
            misses.iter().all(|miss| miss.rule == rule),
            "plant `{what}` should be missed by {rule:?} alone:\n{}",
            told(&misses)
        );
    }
}

/// `team init` run again on a server that holds other rules, from an older
/// build or set by hand, leaves it holding these: with what was written
/// still there, and the people who were signed in still signed in.
#[tokio::test(flavor = "multi_thread")]
async fn init_again_brings_a_server_to_these_rules() {
    let Some(server) = Server::start() else {
        assert!(
            std::env::var_os("CI").is_none(),
            "no `surreal` on PATH, and in CI the team's tests may not be skipped"
        );
        eprintln!("no `surreal` on PATH: the team's rules were not checked");
        return;
    };
    for (n, (what, from, to, _)) in plants().into_iter().enumerate() {
        let database = format!("again{n}");
        let root = team(&server, &database, &planted(what, from, to)).await;
        let ada = client(&server, &database, Who::Person("ada")).await;
        root.query("CREATE event:before CONTENT { key: 'before', person: 'ada', mine: true, line: 'kept' }")
            .await
            .unwrap()
            .check()
            .unwrap();
        root.query(SCHEMA).await.unwrap().check().unwrap();
        let kept = given(&ada, "SELECT VALUE line FROM event:before").await;
        assert_eq!(
            kept,
            vec![json!("kept")],
            "after `{what}`, what was written is there, and ada, signed in before, reads it"
        );
        root.query("DELETE event:before").await.unwrap().check().unwrap();
        let misses = run(&server, &database, root).await;
        assert!(misses.is_empty(), "init over `{what}` left:\n{}", told(&misses));
    }
}
