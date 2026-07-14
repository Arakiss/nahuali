use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};

pub struct McpProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: Receiver<String>,
    home: PathBuf,
}

impl McpProcess {
    pub fn spawn(store: &PathBuf) -> Self {
        let home = std::env::temp_dir().join(format!("{}_home", store.display()));
        std::fs::create_dir_all(&home).expect("isolated MCP home is created");
        let mut child = Command::new(env!("CARGO_BIN_EXE_nahuali-mcp"))
            .arg("--database")
            .arg(store)
            .env("NAHUALI_HOME", &home)
            .env_remove("NAHUALI_DB_URL")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("nahuali-mcp starts");

        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        let (sender, receiver) = mpsc::channel();

        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            child,
            stdin,
            stdout: receiver,
            home,
        }
    }

    pub fn request(&mut self, message: Value) -> Value {
        self.write(message);
        self.read()
    }

    pub fn notify(&mut self, message: Value) {
        self.write(message);
    }

    pub fn read_json_resource(&mut self, id: u64, uri: &str) -> Value {
        let response = self.request(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "resources/read",
            "params": {
                "uri": uri
            }
        }));
        assert_eq!(response["result"]["contents"][0]["uri"], uri);
        assert_eq!(
            response["result"]["contents"][0]["mimeType"],
            "application/json"
        );
        let text = response["result"]["contents"][0]["text"]
            .as_str()
            .expect("resource content is text");
        serde_json::from_str(text).expect("resource text is JSON")
    }

    pub fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.home);
    }

    fn write(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").expect("message writes to server stdin");
        self.stdin.flush().expect("server stdin flushes");
    }

    fn read(&self) -> Value {
        let line = self
            .stdout
            // Six isolated MCP servers may initialize SurrealKV concurrently
            // in the workspace suite. Allow startup headroom on constrained CI
            // runners without weakening the protocol assertions themselves.
            .recv_timeout(Duration::from_secs(15))
            .expect("server writes a response");
        serde_json::from_str(&line).expect("server stdout is valid JSON")
    }
}

impl Drop for McpProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

pub fn temp_store(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    // The MCP server refuses a path-like --database name, so build a clean,
    // unique SurrealDB identifier (this store is only ever a database name).
    let sanitized: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect();
    PathBuf::from(format!(
        "nahuali_mcp_{sanitized}_{}_{nanos}",
        std::process::id()
    ))
}
