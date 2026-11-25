//! The monitors store on disk (§FS-021-monitors.9.1): under the state
//! directory, one directory per project holding `monitors.json` and one
//! `<id>.jsonl` history per monitor.
//!
//! Every change is made under the project's lock (§FS-021-monitors.6.9),
//! over the store as it is then rather than as the run first read it, and
//! `monitors.json` is rewritten whole and renamed into place, so a reader
//! never sees half of it (§FS-021-monitors.9.1). A history is appended to; a
//! reader takes every line it can read and passes over a last line a crash
//! left half-written, and a change that fails takes back what it appended,
//! so the records and the histories never disagree.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::error::{EphorError, Result};
use crate::monitors::history::Line;
use crate::monitors::state::Document;

/// The file that keeps every monitor's record (§FS-021-monitors.9.1).
pub const FILE: &str = "monitors.json";

/// Where every project's store lives: `<state dir>/monitors`
/// (§FS-021-monitors.9.1).
pub fn dir() -> PathBuf {
    crate::paths::state_dir().join("monitors")
}

/// One project's store.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    /// The project's store under `root`, which is [`dir`] outside a test.
    pub fn at(root: &Path, project: &str) -> Store {
        Store {
            dir: root.join(project),
        }
    }

    /// The project's store where ephor keeps it.
    pub fn of(project: &str) -> Store {
        Store::at(&dir(), project)
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join(FILE)
    }

    /// The history of one monitor (§FS-021-monitors.9.1).
    pub fn history_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.jsonl"))
    }

    /// The store as it is now, without the lock: a project with no store
    /// yet reads empty. A store that is there and does not read is the
    /// machinery failing, which fails the slot (§FS-021-monitors.17.3).
    pub fn load(&self) -> Result<Document> {
        let path = self.path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Document::default())
            }
            Err(err) => return Err(failed("read", &path, err)),
        };
        serde_json::from_str(&text).map_err(|err| {
            EphorError::Command(format!(
                "The monitor store {} does not read: {err}",
                path.display()
            ))
        })
    }

    /// Changes the store under the project's lock (§FS-021-monitors.6.9):
    /// the store is read again once the lock is held, `change` makes its
    /// change, histories included, and `monitors.json` is written whole and
    /// renamed into place before the lock is let go. A `change` that fails
    /// writes nothing of `monitors.json`.
    pub fn update<T>(&self, change: impl FnOnce(&mut Document) -> Result<T>) -> Result<T> {
        fs::create_dir_all(&self.dir).map_err(|err| failed("create", &self.dir, err))?;
        let lock_path = self.dir.join(format!("{FILE}.lock"));
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|err| failed("open", &lock_path, err))?;
        lock.lock().map_err(|err| failed("lock", &lock_path, err))?;
        let mut document = self.load()?;
        let changed = change(&mut document)?;
        self.write(&document)?;
        drop(lock);
        Ok(changed)
    }

    fn write(&self, document: &Document) -> Result<()> {
        let path = self.path();
        let text = serde_json::to_string_pretty(document).map_err(|err| {
            EphorError::Command(format!("Cannot encode the monitor store: {err}"))
        })?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text).map_err(|err| failed("write", &tmp, err))?;
        fs::rename(&tmp, &path).map_err(|err| failed("rename", &tmp, err))
    }

    /// Every line of a monitor's history that reads, oldest first. A line
    /// that does not read — the half-written last line of a crashed append —
    /// is passed over rather than costing the history (§FS-021-monitors.9.1).
    pub fn history(&self, id: &str) -> Result<Vec<Line>> {
        let path = self.history_path(id);
        let text = match fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(failed("read", &path, err)),
        };
        Ok(text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }

    /// Appends lines to a monitor's history, under the lock. A last line a
    /// crash left without its newline is cut first, so the next line starts
    /// on a line of its own.
    pub fn append(&self, id: &str, lines: &[Line]) -> Result<()> {
        self.appended(id, lines).map(|_| ())
    }

    /// [`Store::append`], giving back where the history ended before it, so
    /// the append can be taken back; none where nothing was appended.
    fn appended(&self, id: &str, lines: &[Line]) -> Result<Option<u64>> {
        if lines.is_empty() {
            return Ok(None);
        }
        let path = self.history_path(id);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|err| failed("open", &path, err))?;
        let whole = whole_lines(&mut file).map_err(|err| failed("read", &path, err))?;
        file.set_len(whole)
            .and_then(|()| file.seek(SeekFrom::End(0)).map(|_| ()))
            .map_err(|err| failed("write", &path, err))?;
        if let Err(err) = file.write_all(encode(lines)?.as_bytes()) {
            let _ = file.set_len(whole);
            return Err(failed("write", &path, err));
        }
        Ok(Some(whole))
    }

    /// Appends that are taken back together where the change they belong to
    /// fails, so a history never keeps a reading its record does not
    /// (§FS-021-monitors.9.1).
    pub fn appends(&self) -> Appends<'_> {
        Appends {
            store: self,
            made: Vec::new(),
        }
    }

    /// Writes a monitor's history anew, as thinning keeps it
    /// (§FS-021-monitors.9.3), renamed into place like `monitors.json`.
    pub fn rewrite(&self, id: &str, lines: &[Line]) -> Result<()> {
        let path = self.history_path(id);
        let tmp = path.with_extension("jsonl.tmp");
        fs::write(&tmp, encode(lines)?).map_err(|err| failed("write", &tmp, err))?;
        fs::rename(&tmp, &path).map_err(|err| failed("rename", &tmp, err))
    }

    /// Lets a monitor's history go: a kind changed under it
    /// (§FS-021-monitors.4.4), or the store forgot it (§FS-021-monitors.5.6).
    pub fn remove(&self, id: &str) -> Result<()> {
        let path = self.history_path(id);
        match fs::remove_file(&path) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                Err(failed("remove", &path, err))
            }
            _ => Ok(()),
        }
    }
}

/// The appends one change made, each with where its history ended before.
#[derive(Debug)]
pub struct Appends<'a> {
    store: &'a Store,
    made: Vec<(PathBuf, u64)>,
}

impl Appends<'_> {
    /// Appends lines to a monitor's history, remembering where it ended.
    pub fn append(&mut self, id: &str, lines: &[Line]) -> Result<()> {
        if let Some(before) = self.store.appended(id, lines)? {
            self.made.push((self.store.history_path(id), before));
        }
        Ok(())
    }

    /// Takes back every append made, newest first. What cannot be taken
    /// back is left: the reader passes over nothing it cannot read, and the
    /// change failing already says why.
    pub fn undo(self) {
        for (path, before) in self.made.into_iter().rev() {
            if let Ok(file) = OpenOptions::new().write(true).open(&path) {
                let _ = file.set_len(before);
            }
        }
    }
}

/// The length of a file up to the end of its last whole line.
fn whole_lines(file: &mut File) -> std::io::Result<u64> {
    let mut bytes = Vec::new();
    file.seek(SeekFrom::Start(0))?;
    file.read_to_end(&mut bytes)?;
    Ok(bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |at| at as u64 + 1))
}

fn encode(lines: &[Line]) -> Result<String> {
    let mut text = String::new();
    for line in lines {
        text.push_str(&serde_json::to_string(line).map_err(|err| {
            EphorError::Command(format!("Cannot encode a monitor's history: {err}"))
        })?);
        text.push('\n');
    }
    Ok(text)
}

fn failed(what: &str, path: &Path, err: std::io::Error) -> EphorError {
    EphorError::Command(format!("Cannot {what} {}: {err}", path.display()))
}

/// How big the store is, which the doctor reports (§FS-021-monitors.20.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Size {
    pub path: PathBuf,
    pub bytes: u64,
    pub files: usize,
}

/// The size of every project's store under `root`, its lock files included:
/// they are on disk too.
pub fn size(root: &Path) -> Size {
    let mut size = Size {
        path: root.to_path_buf(),
        bytes: 0,
        files: 0,
    };
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                size.bytes += meta.len();
                size.files += 1;
            }
        }
    }
    size
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
