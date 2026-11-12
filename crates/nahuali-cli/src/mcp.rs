//! The same verbs over the Model Context Protocol (`nahuali mcp`), so a
//! tool that cannot run a command line can still use the memory. One
//! request per line on standard input, one answer per line on standard
//! output. Every tool runs the very code its command runs.

use std::io::{BufRead, Write};
use std::sync::OnceLock;

use serde_json::{Value, json};

use crate::Failure;

/// How a tool's argument reaches the command line.
enum Kind {
    /// A word, in order.
    Word,
    /// `--name value`.
    Value,
    /// `--name value`, as many times as given.
    Each,
    /// `--name`, when true.
    Flag,
}

struct Tool {
    name: &'static str,
    says: &'static str,
    takes: &'static [(&'static str, Kind, &'static str)],
}

const EVERYWHERE: &[(&str, Kind, &str)] = &[
    ("project", Kind::Value, "The project. Default: the repository or directory the server runs in."),
    ("as", Kind::Value, "Which agent is speaking. Default: the client's name."),
    ("private", Kind::Flag, "Not to be offered to work on another project."),
];

const TOOLS: &[Tool] = &[
    Tool {
        name: "status",
        says: "Where this project stands: questions waiting on the person, each piece of work with its one-line card, and what was decided lately. Read it before starting.",
        takes: &[("all", Kind::Flag, "Every project, without what is private to another one.")],
    },
    Tool {
        name: "update",
        says: "Overwrite a work's card when its state changes. The first update of a name starts the work.",
        takes: &[
            ("work", Kind::Word, "The work's name: lower-case letters, digits and dashes."),
            ("line", Kind::Value, "The card in one line: what is true now."),
            ("status", Kind::Value, "moving, not-started or done."),
            ("blocked", Kind::Value, "It cannot go on, and by what."),
            ("done", Kind::Each, "Things finished. They add to the card."),
            ("next", Kind::Each, "What comes next. It replaces what the card said."),
            ("title", Kind::Value, "A title."),
            ("goal", Kind::Value, "What the work is for."),
            (
                "verified",
                Kind::Value,
                "How you checked this yourself, just now. Without it the update is only claimed.",
            ),
        ],
    },
    Tool {
        name: "ask",
        says: "Put a question to the person. It stays on their board until answered; find the answer in status. Do not repeat it in chat.",
        takes: &[
            ("question", Kind::Word, "The question."),
            ("recommends", Kind::Value, "The answer you would give."),
            ("blocks", Kind::Value, "What cannot go on until it is answered."),
            ("work", Kind::Value, "The work that waits on it."),
        ],
    },
    Tool {
        name: "answer",
        says: "Close an ask; it becomes a decision. Answers are the person's to give: use it only to record what they told you.",
        takes: &[
            ("ask", Kind::Word, "The ask's name, or the start of it."),
            ("text", Kind::Word, "The answer, or with withdraw the reason."),
            ("withdraw", Kind::Flag, "The ask no longer needs an answer."),
        ],
    },
    Tool {
        name: "learn",
        says: "Record a lesson from a failure and the moment it applies, so whoever is about to repeat it is told then.",
        takes: &[
            ("rule", Kind::Word, "The lesson, as a rule."),
            ("moment", Kind::Value, "When it applies, in words: before spawning an agent in a worktree."),
            (
                "when",
                Kind::Each,
                "Words that mark that moment in a command, all of which must be there: worktree add.",
            ),
            ("everywhere", Kind::Flag, "It applies to every project."),
            ("stop", Kind::Flag, "Serious enough to stop the command once."),
        ],
    },
    Tool {
        name: "check",
        says: "Say what you are about to do and get the lessons that apply. No lesson applies is a normal answer.",
        takes: &[("doing", Kind::Word, "What you are about to do: the command, or a sentence.")],
    },
    Tool {
        name: "note",
        says: "Keep something worth knowing that is not the state of a piece of work.",
        takes: &[
            ("text", Kind::Word, "The note, in one or two sentences."),
            ("tag", Kind::Each, "Words to find it by."),
            ("about", Kind::Value, "The work it is about."),
        ],
    },
    Tool {
        name: "recall",
        says: "What is known about something: cards, decisions, lessons and notes that hold every word you ask for. Nothing known is a normal answer.",
        takes: &[
            ("about", Kind::Word, "A few words. Every one of them has to be there."),
            ("all", Kind::Flag, "Every project, without what is private to another one."),
        ],
    },
];

/// The client's name, as it gave it: who is speaking when a call does not say.
static CLIENT: OnceLock<String> = OnceLock::new();

fn schema(tool: &Tool) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for (name, kind, says) in tool.takes.iter().chain(EVERYWHERE) {
        let shape = match kind {
            Kind::Word | Kind::Value => json!({ "type": "string", "description": says }),
            Kind::Each => json!({ "type": "array", "items": { "type": "string" }, "description": says }),
            Kind::Flag => json!({ "type": "boolean", "description": says }),
        };
        properties.insert((*name).to_string(), shape);
        if matches!(kind, Kind::Word) {
            required.push(*name);
        }
    }
    json!({ "type": "object", "properties": properties, "required": required })
}

/// The command line a call stands for: names first, then `--`, then words.
fn argv(tool: &Tool, given: &Value) -> Vec<String> {
    let mut line = vec![tool.name.to_string()];
    let mut words = Vec::new();
    let text = |v: &Value| v.as_str().map_or_else(|| v.to_string(), str::to_string);
    for (name, kind, _) in tool.takes.iter().chain(EVERYWHERE) {
        let value = &given[*name];
        match kind {
            _ if value.is_null() => {}
            Kind::Word => words.push(text(value)),
            Kind::Value => line.extend([format!("--{name}"), text(value)]),
            Kind::Each => {
                for one in value.as_array().into_iter().flatten() {
                    line.extend([format!("--{name}"), text(one)]);
                }
            }
            Kind::Flag if value == true => line.push(format!("--{name}")),
            Kind::Flag => {}
        }
    }
    if given["as"].is_null()
        && let Some(client) = CLIENT.get()
    {
        line.extend(["--as".to_string(), client.clone()]);
    }
    line.push("--".to_string());
    line.extend(words);
    line
}

fn call(params: &Value) -> Result<Value, String> {
    let name = params["name"].as_str().unwrap_or_default();
    let tool = TOOLS
        .iter()
        .find(|t| t.name == name)
        .ok_or_else(|| format!("nahuali has no tool called `{name}`"))?;
    let (said, failed) = match crate::run(&argv(tool, &params["arguments"])) {
        Ok(out) => (out, false),
        Err(Failure::Usage(why) | Failure::Refused(why)) => (why, true),
    };
    Ok(json!({ "content": [{ "type": "text", "text": said }], "isError": failed }))
}

/// The answer to one request, or nothing for a notification.
pub fn handle(request: &Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    let result = match request["method"].as_str().unwrap_or_default() {
        "initialize" => {
            if let Some(client) = request["params"]["clientInfo"]["name"].as_str() {
                let _ = CLIENT.set(client.to_string());
            }
            let version = request["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18");
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "nahuali", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "A working memory shared with the person and other agents. Read `status` before starting; `update` a card when its state changes; `ask` when the person has to decide; `check` before something that went wrong before.",
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({
            "tools": TOOLS.iter().map(|t| json!({ "name": t.name, "description": t.says, "inputSchema": schema(t) })).collect::<Vec<_>>(),
        })),
        "tools/call" => call(&request["params"]),
        other => Err(format!("nahuali does not know `{other}`")),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(why) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": why } }),
    })
}

pub fn mcp(_argv: &[String]) -> Result<String, Failure> {
    let (stdin, mut stdout) = (std::io::stdin(), std::io::stdout());
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let answer = match serde_json::from_str::<Value>(&line) {
            Ok(request) => handle(&request),
            Err(_) => Some(
                json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "a request is one line of JSON" } }),
            ),
        };
        if let Some(answer) = answer
            && (writeln!(stdout, "{answer}").is_err() || stdout.flush().is_err())
        {
            break;
        }
    }
    Ok(String::new())
}
