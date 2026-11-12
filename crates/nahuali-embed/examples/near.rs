//! How close the model finds a set of questions to a set of things, to set
//! the line between "about this" and "about something else".
//!
//! ```text
//! cargo run --release -p nahuali-embed --example near -- MODELS_DIR
//! ```
//!
//! Each question is listed with every thing, nearest first, and a mark on
//! the ones a person would call an answer.

use std::time::Instant;

use nahuali_embed::{DEFAULT_MODEL, Embedder, LocalEmbedder};

/// Things a memory would hold. Invented.
const THINGS: &[&str] = &[
    "Agent worktrees start from the default branch, not from the branch you are on\nwhen: before spawning an agent in its own worktree",
    "Every worktree builds its own target directory: nine of them filled the disk\nwhen: before building Rust in more than two worktrees",
    "Back up the database before migrating\nwhen: before running a migration",
    "The audit is on the 20th; the client sends the files a week before\ntags: dates",
    "Ship the importer behind a flag?\nanswer: Behind a flag, until the audit.",
    "importer\nmoving · reads the three export formats\nnext: open the review",
    "The art sources and the brand live in a private repository\ntags: brand, art",
    "The database licence forbids offering it as a hosted database service\ntags: licence, cloud",
    "pricing-page\nnot started · waits for the importer",
    "Never force-push a shared branch\nwhen: before force pushing",
];

/// Questions, and which things answer them.
const ASKED: &[(&str, &[usize])] = &[
    ("when does the client review our work", &[3]),
    ("cuándo es la auditoría", &[3]),
    ("can we sell this as a managed service", &[7]),
    ("where are the logo files", &[6]),
    ("how do I schedule a meeting", &[]),
    ("what is the capital of France", &[]),
    ("python virtual environment", &[]),
    ("did we decide how to release the importer", &[4, 5]),
    ("git worktree add ../side -b side", &[0, 1]),
    ("git checkout -b feature in a new working tree for a subagent", &[0]),
    ("./migrate up --env production", &[2]),
    ("alembic upgrade head", &[2]),
    ("git push --force origin main", &[9]),
    ("git push -f", &[9]),
    ("cargo test", &[]),
    ("ls -la", &[]),
    ("npm install", &[]),
    ("git status", &[]),
    ("rm -rf target", &[]),
    ("cargo build --release in the fourth worktree", &[1]),
];

fn main() {
    let dir = std::env::args().nth(1).expect("the models directory");
    let started = Instant::now();
    let model = LocalEmbedder::open(dir.as_ref(), DEFAULT_MODEL).expect("the model is installed there");
    println!("loaded in {:?}, {} dimensions", started.elapsed(), model.dimension());
    let started = Instant::now();
    let things = model.embed_passages(THINGS).unwrap();
    println!("{} things in {:?}", THINGS.len(), started.elapsed());
    let (mut right, mut wrong) = (Vec::new(), Vec::new());
    for (question, answers) in ASKED {
        let started = Instant::now();
        let q = model.embed_query(question).unwrap();
        let took = started.elapsed();
        let mut near: Vec<(f32, usize)> =
            things.iter().enumerate().map(|(i, t)| (q.iter().zip(t).map(|(a, b)| a * b).sum(), i)).collect();
        near.sort_by(|a, b| b.0.total_cmp(&a.0));
        println!("\n{question}   ({took:?})");
        for (sim, i) in near.iter().take(3) {
            let mark = if answers.contains(i) { "✓" } else { " " };
            println!("  {mark} {sim:.3}  {}", THINGS[*i].lines().next().unwrap());
        }
        for (sim, i) in &near {
            if answers.contains(i) { right.push(*sim) } else { wrong.push(*sim) }
        }
    }
    right.sort_by(f32::total_cmp);
    wrong.sort_by(f32::total_cmp);
    println!("\nanswers:     lowest {:.3}, median {:.3}", right[0], right[right.len() / 2]);
    println!(
        "not answers: highest {:.3}, 99th of 100 {:.3}",
        wrong[wrong.len() - 1],
        wrong[wrong.len() * 99 / 100]
    );
    for line in [0.80_f32, 0.75, 0.70, 0.65, 0.60, 0.55] {
        let kept = right.iter().filter(|s| **s >= line).count();
        let let_in = wrong.iter().filter(|s| **s >= line).count();
        println!(
            "  at {line:.2}: {kept} of {} answers kept, {let_in} of {} others let in",
            right.len(),
            wrong.len()
        );
    }
}
