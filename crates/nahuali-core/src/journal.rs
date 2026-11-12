//! Where lines are kept. A journal knows nothing of what a line means.
//!
//! [`FileJournal`] keeps them in plain files a person owns:
//!
//! ```text
//! <home>/README.txt            what these files are, for whoever finds them
//! <home>/log/<writer>.jsonl    one file per writer, appended to and never rewritten
//! <home>/local/writer          this installation's name; never copied elsewhere
//! ```
//!
//! Each installation appends only to its own file, so two machines never
//! write the same file and sharing a memory is copying `log/` both ways,
//! with any tool that copies files.

use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub trait Journal {
    /// Every line ever kept, in any order.
    fn read(&self) -> io::Result<Vec<String>>;
    /// Keep one more line. It is on stable storage when this returns.
    fn append(&mut self, line: &str) -> io::Result<()>;
}

/// Lines in memory: for tests, and for a program that keeps its own.
#[derive(Clone, Debug, Default)]
pub struct MemJournal(pub Vec<String>);

impl Journal for MemJournal {
    fn read(&self) -> io::Result<Vec<String>> {
        Ok(self.0.clone())
    }

    fn append(&mut self, line: &str) -> io::Result<()> {
        self.0.push(line.to_string());
        Ok(())
    }
}

const README: &str = "\
This folder is a Nahuali memory. It belongs to whoever owns the folder.

log/     Everything that happened, one line of JSON per event, one file per
         installation that wrote to it. Files are only ever appended to.
         Each line says who wrote it (by), when (at, in UTC), how sure they
         were (sure), which project it belongs to, and what happened (what).
         What is true now is these lines read in order of their time.
local/   Belongs to this machine alone. Do not copy it to another one.

To share this memory between machines, copy log/ both ways with any tool
that copies files. To read it without Nahuali, open the files in log/.
";

#[derive(Clone, Debug)]
pub struct FileJournal {
    home: PathBuf,
}

impl FileJournal {
    pub fn at(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    fn log(&self) -> PathBuf {
        self.home.join("log")
    }

    /// This installation's name, drawn once.
    fn writer(&self) -> io::Result<String> {
        let path = self.home.join("local").join("writer");
        if let Some(name) = read_name(&path)? {
            return Ok(name);
        }
        fs::create_dir_all(self.home.join("local"))?;
        let drawn = format!("{:012x}", draw() & 0xffff_ffff_ffff);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(format!("{drawn}\n").as_bytes())?;
                file.sync_all()?;
                Ok(drawn)
            }
            // Another process drew first: its name is the name.
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                for _ in 0..50 {
                    if let Some(name) = read_name(&path)? {
                        return Ok(name);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(io::Error::other(format!("{} is empty", path.display())))
            }
            Err(e) => Err(e),
        }
    }
}

fn read_name(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let name = text.trim();
            let fine = !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
            Ok(fine.then(|| name.to_string()))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Bits from the operating system, by way of the standard library's hasher
/// seeds. Enough to name an installation; not a secret.
fn draw() -> u64 {
    std::collections::hash_map::RandomState::new().build_hasher().finish()
}

impl Journal for FileJournal {
    fn read(&self) -> io::Result<Vec<String>> {
        let entries = match fs::read_dir(self.log()) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .collect();
        files.sort();
        let mut lines = Vec::new();
        for file in files {
            let bytes = fs::read(&file)?;
            lines.extend(
                String::from_utf8_lossy(&bytes).lines().filter(|l| !l.trim().is_empty()).map(str::to_string),
            );
        }
        Ok(lines)
    }

    fn append(&mut self, line: &str) -> io::Result<()> {
        debug_assert!(!line.contains('\n'), "one event is one line");
        let writer = self.writer()?;
        fs::create_dir_all(self.log())?;
        let readme = self.home.join("README.txt");
        if !readme.exists() {
            fs::write(&readme, README)?;
        }
        let mut file: File = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(self.log().join(format!("{writer}.jsonl")))?;
        // Several agents on one machine share this file: one at a time. On a
        // local disk an append of one line is whole even without the lock,
        // so no test here can show it missing; it is for the disks where it
        // is not, and for the look at the last byte just below.
        file.lock()?;
        let mut out = Vec::with_capacity(line.len() + 2);
        // A crash can leave half a line at the end. Close it, so that it
        // spoils itself and not the line that follows.
        if file.seek(SeekFrom::End(0))? > 0 {
            file.seek(SeekFrom::End(-1))?;
            let mut last = [0u8; 1];
            file.read_exact(&mut last)?;
            if last[0] != b'\n' {
                out.push(b'\n');
            }
        }
        out.extend_from_slice(line.as_bytes());
        out.push(b'\n');
        file.write_all(&out)?;
        file.sync_data()?;
        file.unlock()
    }
}
