//! Downloading a model: an explicit act, never a side effect of a query.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::local::ModelSpec;
use crate::unigram::{INDEX_FILE, compile};

/// Whether every file of `spec` is in place.
pub fn installed(models_dir: &Path, spec: &ModelSpec) -> bool {
    let dir = models_dir.join(spec.id);
    dir.join(INDEX_FILE).is_file() && spec.files.iter().all(|(name, _)| dir.join(name).is_file())
}

fn sha256(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| Error::file(path, e))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 20];
    loop {
        let n = file.read(&mut buffer).map_err(|e| Error::file(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn fetch(url: &str, dest: &Path) -> Result<()> {
    let status = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", "--retry", "3", "--output"])
        .arg(dest)
        .arg(url)
        .stdin(Stdio::null())
        .status()
        .map_err(|e| Error::Download(format!("cannot run curl: {e}")))?;
    if status.success() { Ok(()) } else { Err(Error::Download(format!("curl failed on {url}"))) }
}

/// Download `spec` into `models_dir` unless it is already there. Every file
/// must match its pinned hash before it takes its final name. Returns the
/// model's directory and whether anything had to be written.
pub fn install(models_dir: &Path, spec: &ModelSpec) -> Result<(PathBuf, bool)> {
    let dir = models_dir.join(spec.id);
    fs::create_dir_all(&dir).map_err(|e| Error::file(&dir, e))?;
    let mut changed = false;
    for (name, hash) in spec.files {
        let path = dir.join(name);
        if path.is_file() && sha256(&path)? == *hash {
            continue;
        }
        let partial = dir.join(format!("{name}.partial"));
        let url = format!("https://huggingface.co/{}/resolve/{}/{name}", spec.repo, spec.revision);
        fetch(&url, &partial)?;
        let found = sha256(&partial)?;
        if found != *hash {
            let _ = fs::remove_file(&partial);
            return Err(Error::Download(format!("{name}: sha256 {found}, expected {hash}")));
        }
        fs::rename(&partial, &path).map_err(|e| Error::file(&path, e))?;
        changed = true;
    }
    let index = dir.join(INDEX_FILE);
    if changed || !index.is_file() {
        compile(&dir.join("tokenizer.json"), &index)?;
        changed = true;
    }
    Ok((dir, changed))
}
