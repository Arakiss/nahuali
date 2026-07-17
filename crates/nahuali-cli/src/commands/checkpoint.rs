use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail, ensure};
use nahuali_core::{
    CheckpointMatchMode, CheckpointTrustPolicyV2, CheckpointVerificationOptionsV2, MemoryEngine,
    SignedLedgerCheckpointV2, add_checkpoint_signature_v2, checkpoint_policy_key_v2,
    sign_checkpoint_v2,
};

const MAX_INPUT_BYTES: u64 = 64 * 1024;

pub(crate) fn policy_init(
    memory: &mut MemoryEngine,
    origin: &str,
    key_ids: &[String],
    key_files: &[PathBuf],
    minimum_signatures: u32,
    output: &Path,
) -> anyhow::Result<()> {
    validate_key_pairs(key_ids, key_files)?;
    let keys = key_ids
        .iter()
        .zip(key_files)
        .map(|(key_id, key_file)| {
            let seed = read_bounded_text(key_file, "checkpoint signing key")?;
            checkpoint_policy_key_v2(key_id, seed.trim()).with_context(|| {
                format!(
                    "checkpoint signing key file '{}' is invalid for key id '{key_id}'",
                    key_file.display()
                )
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let policy = memory
        .create_checkpoint_policy_v2(origin, minimum_signatures, keys)
        .context("create checkpoint trust policy")?;
    write_json_new(output, &policy, "checkpoint trust policy")?;

    println!(
        "Created checkpoint trust policy for origin '{}' with {} active key(s) and a threshold of {} at {}",
        policy.expected_origin,
        policy.keys.len(),
        policy.minimum_signatures,
        output.display()
    );
    Ok(())
}

pub(crate) fn sign(
    memory: &mut MemoryEngine,
    policy_path: &Path,
    key_ids: &[String],
    key_files: &[PathBuf],
    output: &Path,
) -> anyhow::Result<()> {
    validate_key_pairs(key_ids, key_files)?;
    let policy: CheckpointTrustPolicyV2 = read_json_bounded(policy_path, "checkpoint policy")?;
    policy
        .validate()
        .context("validate external checkpoint policy")?;

    let latest_event_ms = memory
        .events()
        .iter()
        .map(|event| event.timestamp_ms)
        .max()
        .unwrap_or_default();
    let generated_at_ms = now_ms()?.max(latest_event_ms);
    let checkpoint = memory
        .create_checkpoint_v2(&policy.expected_origin, generated_at_ms)
        .context("create current ledger checkpoint")?;

    let mut pairs = key_ids.iter().zip(key_files);
    let (first_key_id, first_key_file) = pairs
        .next()
        .context("at least one checkpoint signing key is required")?;
    let first_seed = read_bounded_text(first_key_file, "checkpoint signing key")?;
    let mut signed = sign_checkpoint_v2(checkpoint, &policy, first_key_id, first_seed.trim())
        .with_context(|| {
            format!(
                "sign checkpoint with authorized key id '{first_key_id}' from '{}'",
                first_key_file.display()
            )
        })?;

    for (key_id, key_file) in pairs {
        let seed = read_bounded_text(key_file, "checkpoint signing key")?;
        add_checkpoint_signature_v2(&mut signed, &policy, key_id, seed.trim()).with_context(
            || {
                format!(
                    "add checkpoint signature for authorized key id '{key_id}' from '{}'",
                    key_file.display()
                )
            },
        )?;
    }

    let verdict = memory
        .verify_checkpoint_v2(
            &signed,
            &policy,
            CheckpointVerificationOptionsV2::current(now_ms()?),
        )
        .context("verify signed checkpoint before writing it")?;
    ensure!(
        verdict.trusted,
        "refusing to write an untrusted checkpoint: {}",
        verdict.reasons.join("; ")
    );

    write_json_new(output, &signed, "signed ledger checkpoint")?;
    println!(
        "Created trusted checkpoint covering {} event(s) with {} accepted signature(s) at {}",
        verdict.checkpoint_tree_size,
        verdict.accepted_signature_count,
        output.display()
    );
    Ok(())
}

pub(crate) fn verify(
    memory: &mut MemoryEngine,
    checkpoint_path: &Path,
    policy_path: &Path,
    mode: CheckpointMatchMode,
    json: bool,
) -> anyhow::Result<()> {
    let signed: SignedLedgerCheckpointV2 =
        read_json_bounded(checkpoint_path, "signed ledger checkpoint")?;
    let policy: CheckpointTrustPolicyV2 = read_json_bounded(policy_path, "checkpoint policy")?;
    policy
        .validate()
        .context("validate external checkpoint policy")?;
    let options = match mode {
        CheckpointMatchMode::Current => CheckpointVerificationOptionsV2::current(now_ms()?),
        CheckpointMatchMode::Historical => CheckpointVerificationOptionsV2::historical(now_ms()?),
    };
    let verdict = memory
        .verify_checkpoint_v2(&signed, &policy, options)
        .context("verify signed ledger checkpoint")?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&verdict)
                .context("serialize checkpoint verification verdict")?
        );
    } else if verdict.trusted {
        match mode {
            CheckpointMatchMode::Current => {
                println!("TRUSTED CURRENT CHECKPOINT");
                println!(
                    "Checkpoint covers all {} current ledger event(s).",
                    verdict.checkpoint_tree_size
                );
            }
            CheckpointMatchMode::Historical => {
                println!("TRUSTED HISTORICAL CHECKPOINT");
                println!(
                    "Checkpoint covers {} event(s); {} event(s) were appended after it.",
                    verdict.checkpoint_tree_size, verdict.appended_event_count
                );
            }
        }
        println!(
            "Accepted signatures: {}/{}",
            verdict.accepted_signature_count, verdict.minimum_signature_count
        );
    } else {
        println!("UNTRUSTED CHECKPOINT");
        println!(
            "Checkpoint covers {} event(s); {} event(s) were appended after it.",
            verdict.checkpoint_tree_size, verdict.appended_event_count
        );
        for reason in &verdict.reasons {
            println!("- {reason}");
        }
    }

    ensure!(
        verdict.trusted,
        "checkpoint is not trusted under the supplied external policy and match mode"
    );
    Ok(())
}

fn validate_key_pairs(key_ids: &[String], key_files: &[PathBuf]) -> anyhow::Result<()> {
    ensure!(
        !key_ids.is_empty(),
        "at least one --key-id/--key-file pair is required"
    );
    ensure!(
        key_ids.len() == key_files.len(),
        "--key-id and --key-file must be repeated the same number of times (got {} key id(s) and {} key file(s))",
        key_ids.len(),
        key_files.len()
    );
    Ok(())
}

fn read_json_bounded<T: serde::de::DeserializeOwned>(
    path: &Path,
    label: &str,
) -> anyhow::Result<T> {
    let text = read_bounded_text(path, label)?;
    serde_json::from_str(&text).with_context(|| format!("parse {label} from '{}'", path.display()))
}

fn read_bounded_text(path: &Path, label: &str) -> anyhow::Result<String> {
    let file = File::open(path).with_context(|| format!("open {label} at '{}'", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("read {label} from '{}'", path.display()))?;
    ensure!(
        bytes.len() as u64 <= MAX_INPUT_BYTES,
        "{label} at '{}' exceeds the 64 KiB input limit",
        path.display()
    );
    String::from_utf8(bytes)
        .with_context(|| format!("{label} at '{}' is not valid UTF-8", path.display()))
}

fn write_json_new<T: serde::Serialize>(
    output: &Path,
    value: &T,
    label: &str,
) -> anyhow::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .with_context(|| format!("serialize {label} for '{}'", output.display()))?;
    bytes.push(b'\n');
    write_atomic_new(output, &bytes, label)
}

fn write_atomic_new(output: &Path, bytes: &[u8], label: &str) -> anyhow::Result<()> {
    ensure!(
        !output.exists(),
        "refusing to overwrite existing {label} at '{}'",
        output.display()
    );
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    ensure!(
        parent.is_dir(),
        "parent directory '{}' does not exist",
        parent.display()
    );

    let file_name = output
        .file_name()
        .context("checkpoint output path must name a file")?;
    let mut last_collision = None;
    for attempt in 0..100_u32 {
        let temp_name = format!(
            ".{}.nahuali-tmp-{}-{}-{attempt}",
            file_name.to_string_lossy(),
            std::process::id(),
            now_nanos()?
        );
        let temp_path = parent.join(temp_name);
        let mut temp = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                last_collision = Some(error);
                continue;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("create temporary {label} beside '{}'", output.display())
                });
            }
        };

        let write_result = (|| -> anyhow::Result<()> {
            temp.write_all(bytes)
                .with_context(|| format!("write temporary {label} for '{}'", output.display()))?;
            temp.sync_all()
                .with_context(|| format!("sync temporary {label} for '{}'", output.display()))?;
            drop(temp);
            match fs::hard_link(&temp_path, output) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    bail!(
                        "refusing to overwrite existing {label} at '{}'",
                        output.display()
                    );
                }
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "atomically publish {label} without replacing '{}'",
                            output.display()
                        )
                    });
                }
            }
            fs::remove_file(&temp_path).with_context(|| {
                format!(
                    "remove temporary {label} after publishing '{}'",
                    output.display()
                )
            })?;
            Ok(())
        })();

        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        return write_result;
    }

    bail!(
        "could not allocate a temporary file for {label} beside '{}': {}",
        output.display(),
        last_collision
            .map(|error| error.to_string())
            .unwrap_or_else(|| "temporary name space exhausted".to_string())
    )
}

fn now_ms() -> anyhow::Result<u64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_millis();
    u64::try_from(millis).context("system clock exceeds checkpoint timestamp range")
}

fn now_nanos() -> anyhow::Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos())
}
