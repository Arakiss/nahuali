//! One person's machines, each with its own memory, brought together when
//! they can see each other.
//!
//! Every machine keeps a whole memory and works with no network at all. A
//! memory is a set of events that never change, so bringing two of them
//! together is each taking from the other the events it lacks: there is
//! nothing to merge and nothing to decide. A machine that listens (a
//! computer left on at home, say) is where the others meet; two laptops
//! that only ever talk to it end up holding the same.
//!
//! ```text
//! nahuali node listen [ADDR]     this machine answers the others; prints how to join it
//! nahuali node join HOST:PORT    bring this memory together with that one (NAHUALI_KEY)
//! nahuali node sync              now, instead of at the next half minute
//! nahuali node                   who this machine talks to, and how the last time went
//! nahuali node forget            keep to this machine again
//! ```
//!
//! Machines that share a memory share one key, drawn once. Everything they
//! say to each other is sealed with it: someone else on the same network
//! can neither read it nor put anything in.

use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::remote::Remote;
use crate::{Failure, env, parse, usage, words};

/// Where a machine listens when it is not told where.
const DEFAULT: &str = "0.0.0.0:8746";
/// No message between machines is longer than this.
const LARGEST: usize = 256 * 1024 * 1024;

/// Which machines this one talks to. Kept in `<home>/local/node.json`,
/// readable by this user alone.
#[derive(Clone, Debug, Default)]
pub struct Node {
    /// The key every machine of this memory shares, as 64 hex digits.
    pub key: String,
    /// Where this machine answers the others, when it does.
    pub listen: Option<String>,
    /// The machines this one goes to.
    pub peers: Vec<String>,
}

fn file(home: &Path) -> PathBuf {
    home.join("local").join("node.json")
}

impl Node {
    pub fn read(home: &Path) -> Option<Self> {
        let value: Value = serde_json::from_str(&std::fs::read_to_string(file(home)).ok()?).ok()?;
        Some(Self {
            key: value["key"].as_str()?.to_string(),
            listen: value["listen"].as_str().map(str::to_string),
            peers: value["peers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| p.as_str().map(str::to_string))
                .collect(),
        })
    }

    fn write(&self, home: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::create_dir_all(home.join("local"))?;
        let path = file(home);
        let _ = std::fs::remove_file(&path);
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
        let text = json!({ "key": self.key, "listen": self.listen, "peers": self.peers });
        std::io::Write::write_all(&mut out, format!("{text}\n").as_bytes())
    }

    pub fn cipher(&self) -> Result<XChaCha20Poly1305, String> {
        let bytes = unhex(&self.key).ok_or("the key of this memory is not 64 hex digits")?;
        XChaCha20Poly1305::new_from_slice(&bytes)
            .map_err(|_| "the key of this memory is not 32 bytes".to_string())
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() != 64 {
        return None;
    }
    (0..32).map(|i| u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()).collect()
}

/// Seal one message and send it: its length, a number used once, and the
/// message, unreadable and unchangeable without the key.
pub async fn send(stream: &mut TcpStream, cipher: &XChaCha20Poly1305, message: &Value) -> Result<(), String> {
    let mut once = [0u8; 24];
    getrandom::fill(&mut once).map_err(|e| e.to_string())?;
    let nonce = XNonce::try_from(&once[..]).map_err(|_| "a nonce is 24 bytes".to_string())?;
    let sealed = cipher
        .encrypt(&nonce, message.to_string().as_bytes())
        .map_err(|_| "the message could not be sealed".to_string())?;
    let length =
        u32::try_from(once.len() + sealed.len()).map_err(|_| "the message is too long".to_string())?;
    let mut frame = Vec::with_capacity(4 + once.len() + sealed.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&once);
    frame.extend_from_slice(&sealed);
    stream.write_all(&frame).await.map_err(|e| e.to_string())
}

/// Receive one message. Anything not sealed with this memory's key is not
/// a message.
pub async fn receive(stream: &mut TcpStream, cipher: &XChaCha20Poly1305) -> Result<Value, String> {
    let mut length = [0u8; 4];
    stream.read_exact(&mut length).await.map_err(|e| e.to_string())?;
    let length = u32::from_be_bytes(length) as usize;
    if !(24 + 16..=LARGEST).contains(&length) {
        return Err("that is not a message of this memory".to_string());
    }
    let mut frame = vec![0u8; length];
    stream.read_exact(&mut frame).await.map_err(|e| e.to_string())?;
    let nonce = XNonce::try_from(&frame[..24]).map_err(|_| "a nonce is 24 bytes".to_string())?;
    let opened = cipher
        .decrypt(&nonce, &frame[24..])
        .map_err(|_| "it was not sealed with this memory's key".to_string())?;
    serde_json::from_slice(&opened).map_err(|e| e.to_string())
}

fn refused(e: impl std::fmt::Display) -> Failure {
    Failure::Refused(e.to_string())
}

/// The name others reach this machine by, as far as it knows.
fn hostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "THIS-MACHINE".to_string())
}

/// How the last meeting with each machine went, as the keeper tells it.
fn told(answer: &Value) -> String {
    let mut out = String::new();
    for peer in answer["peers"].as_array().into_iter().flatten() {
        let how = match (peer["took"].as_u64(), peer["gave"].as_u64(), peer["error"].as_str()) {
            (Some(took), Some(gave), _) => format!("took {took}, gave {gave}"),
            (_, _, Some(why)) => format!("does not answer ({why}); it is tried again every half minute"),
            _ => "not tried yet".to_string(),
        };
        out.push_str(&format!("  {} · {how}\n", peer["addr"].as_str().unwrap_or("?")));
    }
    out
}

pub fn node(argv: &[String]) -> Result<String, Failure> {
    let args = parse(argv, &[], &[])?;
    let home = env::home(args.value("home")).map_err(Failure::Usage)?;
    let config = Node::read(&home);
    if crate::team::Server::read(&home).is_some() && args.words.first().is_some_and(|w| w != "forget") {
        return Err(Failure::Refused(
            "this memory is on a team's server, which already brings its machines together".to_string(),
        ));
    }
    match args.words.first().map(String::as_str) {
        None => {
            let Some(config) = config else { return Ok("this memory keeps to this machine\n".to_string()) };
            let mut out = match &config.listen {
                Some(addr) => format!("this machine answers the others at {addr}\n"),
                None => "this machine goes to the others; it does not answer them\n".to_string(),
            };
            if !config.peers.is_empty() {
                let answer = Remote::at(&home).call("node", json!({})).map_err(refused)?;
                out.push_str("it goes to\n");
                out.push_str(&told(&answer));
            }
            Ok(out)
        }
        Some("listen") => {
            let addr = match args.words.as_slice() {
                [_] => DEFAULT.to_string(),
                [_, addr] => addr.clone(),
                _ => {
                    return Err(usage(
                        "node listen takes at most where to listen: nahuali node listen 0.0.0.0:8746",
                    ));
                }
            };
            addr.parse::<std::net::SocketAddr>()
                .map_err(|_| usage(format!("`{addr}` is not an address and a port, like 0.0.0.0:8746")))?;
            let mut config = config.unwrap_or_default();
            if config.key.is_empty() {
                let mut key = [0u8; 32];
                getrandom::fill(&mut key).map_err(refused)?;
                config.key = hex(&key);
            }
            config.listen = Some(addr.clone());
            config.write(&home).map_err(refused)?;
            Remote::at(&home).stop();
            // The keeper is what listens: start it, and see that it could.
            Remote::at(&home).call("ping", json!({})).map_err(refused)?;
            let port = addr.rsplit(':').next().unwrap_or("8746");
            Ok(format!(
                "this machine now answers the others at {addr}, for as long as nahuali runs on it\n\
                 on each of your other machines, once:\n\n  NAHUALI_KEY={} nahuali node join {}:{port}\n\n\
                 To keep it answering with nobody using it, run `nahuali serve --stay` as a service.\n",
                config.key,
                hostname()
            ))
        }
        Some("join") => {
            let [_, addr] = words(
                &args,
                "node join takes the other machine's name and port: nahuali node join pc.local:8746",
            )?;
            let key = std::env::var("NAHUALI_KEY")
                .map_err(|_| usage("set NAHUALI_KEY to the key the other machine printed"))?;
            if unhex(&key).is_none() {
                return Err(usage("NAHUALI_KEY is 64 hex digits, as the other machine printed it"));
            }
            let mut config = config.unwrap_or_default();
            if !config.key.is_empty() && config.key != key {
                return Err(Failure::Refused(
                    "this memory already shares another key with other machines; `nahuali node forget` first"
                        .to_string(),
                ));
            }
            config.key = key;
            if !config.peers.contains(&addr) {
                config.peers.push(addr.clone());
            }
            config.write(&home).map_err(refused)?;
            Remote::at(&home).stop();
            let answer = Remote::at(&home).call("consolidate", json!({})).map_err(refused)?;
            Ok(format!(
                "this memory and the one at {addr} are brought together whenever they see each other\n{}",
                told(&answer)
            ))
        }
        Some("sync") => {
            let [_] = words(&args, "node sync takes no words")?;
            if config.is_none_or(|c| c.peers.is_empty()) {
                return Ok("this machine goes to no other\n".to_string());
            }
            let answer = Remote::at(&home).call("consolidate", json!({})).map_err(refused)?;
            Ok(told(&answer))
        }
        Some("forget") => {
            let [_] = words(&args, "node forget takes no words")?;
            if config.is_none() {
                return Ok("this memory keeps to this machine\n".to_string());
            }
            std::fs::remove_file(file(&home)).map_err(refused)?;
            Remote::at(&home).stop();
            Ok("this memory keeps to this machine again. What it took from the others stays\n".to_string())
        }
        Some(other) => {
            Err(usage(format!("`{other}` is not something node does: listen, join, sync or forget")))
        }
    }
}
