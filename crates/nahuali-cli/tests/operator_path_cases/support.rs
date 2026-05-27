use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn run_ok(store: &Path, args: &[&str]) -> String {
    let output = run(store, args);
    assert!(
        output.status.success(),
        "command failed\nargs: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("stdout is UTF-8")
}

pub fn run_ok_with_semantic_collection(store: &Path, args: &[&str], collection: &str) -> String {
    let output = run_with_semantic_collection(store, args, collection);
    assert!(
        output.status.success(),
        "command failed\nargs: {args:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("stdout is UTF-8")
}

pub fn run(store: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nahuali"));
    command.arg("--database").arg(store);
    command.args(args);
    command.output().expect("nahuali-cli runs")
}

pub fn run_with_semantic_collection(store: &Path, args: &[&str], collection: &str) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nahuali"));
    command
        .arg("--database")
        .arg(store)
        .env("NAHUALI_QDRANT_COLLECTION", collection);
    command.args(args);
    command.output().expect("nahuali-cli runs")
}

pub fn temp_store(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("nahuali-cli-{name}-{}-{nanos}", std::process::id()));
    let _ = fs::remove_file(&path);
    path
}

pub fn semantic_collection_name(name: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after epoch")
        .as_nanos();
    format!("nahuali_cli_{name}_{}_{nanos}", std::process::id())
}
