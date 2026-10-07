//! Durable row records and nonblocking locks (§FS-005-dispatch.13).

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::Binding;
use crate::{
    error::{EphorError, Result},
    feed::model::Item,
    forge::Request,
};
use serde::{Deserialize, Serialize};

/// An uncertain implicit result is replayable only with original eligibility;
/// explicit unknown is always held (§FS-005-dispatch.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pending,
    Held,
    Sent,
    NotSent,
}

/// One operation, retained unchanged throughout its recovery (§FS-005-dispatch.13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub binding: Binding,
    pub text: String,
    pub request: Option<Request>,
    pub reconciliation: bool,
    pub draft: Option<PathBuf>,
    pub status: Status,
    pub note: Option<String>,
}

impl Intent {
    /// Pending and held operations block new sends (§FS-005-dispatch.13).
    pub fn unresolved(&self) -> bool {
        matches!(self.status, Status::Pending | Status::Held)
    }
    /// A process interruption must not turn native/legacy uncertainty into replay
    /// (§FS-005-dispatch.13).
    pub fn held(&self) -> bool {
        self.status == Status::Held || (self.status == Status::Pending && !self.reconciliation)
    }
}

/// Accepted thread generations survive subsequent intents (§FS-005-dispatch.13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Accepted {
    pub generation: u64,
    pub text: String,
    pub target: Option<serde_json::Value>,
}

/// Versioned state lives apart from work.json, with durable confirmed identities
/// so failed retirement cannot invite reposting (§FS-005-dispatch.13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub version: u32,
    pub row: Item,
    pub intent: Option<Intent>,
    pub accepted: BTreeMap<String, Accepted>,
    pub confirmed: BTreeSet<PathBuf>,
}

impl Record {
    /// An empty history for a never-sent row (§FS-005-dispatch.13).
    pub fn new(item: &Item) -> Self {
        Self {
            version: 1,
            row: item.clone(),
            intent: None,
            accepted: BTreeMap::new(),
            confirmed: BTreeSet::new(),
        }
    }
    /// A thread advances only after known acceptance or the person's sent decision
    /// (§FS-005-dispatch.13).
    pub fn generation(&self, binding: &Binding) -> u64 {
        self.accepted
            .get(&binding.key())
            .map(|accepted| accepted.generation)
            .unwrap_or(0)
    }
    /// Confirm before retiring a file; retain all earlier confirmations and
    /// generations when a later intent replaces this one (§FS-005-dispatch.13).
    pub fn confirm(&mut self) -> Result<()> {
        let intent = self
            .intent
            .as_ref()
            .ok_or_else(|| error("No saved send to confirm"))?;
        if !intent.unresolved() {
            return Err(error("This send is already resolved"));
        }
        let key = intent.binding.key();
        let generation = self
            .generation(&intent.binding)
            .checked_add(1)
            .ok_or_else(|| error("Reply generation exhausted"))?;
        self.accepted.insert(
            key,
            Accepted {
                generation,
                text: intent.text.clone(),
                target: intent.binding.target.clone(),
            },
        );
        if let Some(path) = &intent.draft {
            self.confirmed.insert(path.clone());
        }
        self.intent.as_mut().unwrap().status = Status::Sent;
        Ok(())
    }
    /// Not-sent releases a checked hold without advancing a thread or discarding
    /// the copyable draft (§FS-005-dispatch.13).
    pub fn release(&mut self) -> Result<()> {
        let intent = self
            .intent
            .as_mut()
            .ok_or_else(|| error("No held send to resolve"))?;
        if !intent.held() {
            return Err(error("Only a held outcome can be resolved"));
        }
        intent.status = Status::NotSent;
        Ok(())
    }
}

/// Fault injection points on the real save seam; callers can refuse each stage
/// before it executes (§FS-005-dispatch.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavePhase {
    Write,
    FileSync,
    Rename,
    DirectorySync,
}

/// The shared move uses the same persistence seam for pending, held, confirmed
/// and resolved records. Tests can wrap the real writer to inject save-stage
/// failures at either side of acknowledgement (§FS-005-dispatch.13).
pub trait ReplyStorage {
    fn read(&self) -> Result<Option<Record>>;
    fn save(&self, record: &Record) -> Result<()>;
}

impl ReplyStorage for Store {
    fn read(&self) -> Result<Option<Record>> {
        Store::read(self)
    }
    fn save(&self, record: &Record) -> Result<()> {
        Store::save(self, record)
    }
}

/// One row lock is held through capture, invocation, confirmation and resolution.
/// Lock files persist: removing one while held could create a second lock inode
/// (§FS-005-dispatch.13).
pub struct Store {
    dir: PathBuf,
    path: PathBuf,
    row: String,
    _lock: Option<File>,
    writable: bool,
}

/// Passive enumeration retains valid rows and names each invalid record
/// independently (§FS-005-dispatch.13, §FS-011-command-line.4).
#[derive(Debug, Default)]
pub struct Recovery {
    pub rows: Vec<Item>,
    pub diagnostics: Vec<String>,
}

#[cfg(test)]
thread_local! {
    static OPERATIONS: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) };
    static TEST_DIRECTORY: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn use_test_directory(path: PathBuf) -> TestDirectory {
    TEST_DIRECTORY.with(|directory| {
        assert!(directory.borrow().is_none());
        *directory.borrow_mut() = Some(path);
    });
    TestDirectory
}

#[cfg(test)]
pub(crate) struct TestDirectory;

#[cfg(test)]
impl Drop for TestDirectory {
    fn drop(&mut self) {
        TEST_DIRECTORY.with(|directory| *directory.borrow_mut() = None);
    }
}

fn reply_dir() -> PathBuf {
    #[cfg(test)]
    if let Some(path) = TEST_DIRECTORY.with(|directory| directory.borrow().clone()) {
        return path;
    }
    crate::paths::state_dir().join("replies")
}

#[cfg(test)]
pub(crate) fn take_operations() -> Vec<&'static str> {
    OPERATIONS.with(|operations| std::mem::take(&mut *operations.borrow_mut()))
}

impl Store {
    /// Atomic replacement permits passive readings without acquiring or creating
    /// a lock; moves re-read under their own lock (§FS-005-dispatch.13).
    pub fn inspect(row: &str) -> Result<Option<Record>> {
        let dir = reply_dir();
        Self {
            path: dir.join(format!("{}.json", digest(row))),
            dir,
            row: row.into(),
            _lock: None,
            writable: false,
        }
        .read()
    }
    /// Site-owned location (§FS-005-dispatch.13).
    pub fn site(row: &str, writing: bool) -> Result<Self> {
        Self::open(&reply_dir(), row, writing)
    }

    /// Read-only rehearsal probes an existing lock but creates no file or directory
    /// (§FS-011-command-line.4). Every writer acquires nonblocking before reading.
    pub fn open(dir: &Path, row: &str, writing: bool) -> Result<Self> {
        let name = digest(row);
        let lock_path = dir.join(format!("{name}.lock"));
        let lock = if writing {
            #[cfg(test)]
            OPERATIONS.with(|operations| operations.borrow_mut().push("create-directory/lock"));
            fs::create_dir_all(dir).map_err(io_error)?;
            Some(
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(lock_path)
                    .map_err(io_error)?,
            )
        } else {
            match OpenOptions::new().read(true).open(lock_path) {
                Ok(file) => Some(file),
                Err(err) if err.kind() == io::ErrorKind::NotFound => None,
                Err(err) => return Err(io_error(err)),
            }
        };
        if let Some(file) = &lock {
            file.try_lock()
                .map_err(|_| error("A reply move for this row is already in progress"))?;
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            path: dir.join(format!("{name}.json")),
            row: row.into(),
            _lock: lock,
            writable: writing,
        })
    }

    /// Unreadable, invalid or unknown-version records refuse rather than vanish
    /// (§FS-005-dispatch.13).
    pub fn read(&self) -> Result<Option<Record>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(io_error(err)),
        };
        let record: Record = serde_json::from_slice(&bytes)
            .map_err(|err| error(format!("Invalid saved reply: {err}")))?;
        if record.version != 1 || record.row.id != self.row {
            return Err(error("Saved reply version or row identity is invalid"));
        }
        if let Some(intent) = &record.intent {
            let carrier_valid = match (intent.binding.reply_target(), &intent.request) {
                (Some(crate::feed::reply::ReplyTarget::Forge { .. }), Some(request)) => {
                    intent.binding.matches_request(request)
                }
                (Some(crate::feed::reply::ReplyTarget::Native(_)), None) => !intent.reconciliation,
                _ => false,
            };
            if intent.binding.row != self.row
                || intent.binding.source != record.row.source
                || intent.text.trim().is_empty()
                || intent.binding.messages.is_empty()
                || !carrier_valid
            {
                return Err(error("Saved reply provenance or payload is invalid"));
            }
        }
        Ok(Some(record))
    }

    /// Atomic replacement with file sync before rename and directory sync before
    /// delivery. Synchronize the directory's parent as well on first creation
    /// (§FS-005-dispatch.13).
    pub fn save(&self, record: &Record) -> Result<()> {
        self.save_with(record, |_| Ok(()))
    }

    /// The production writer, with injectable failures at actual persistence
    /// boundaries (§FS-005-dispatch.13).
    pub fn save_with(
        &self,
        record: &Record,
        mut before: impl FnMut(SavePhase) -> io::Result<()>,
    ) -> Result<()> {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        if !self.writable || self._lock.is_none() {
            return Err(error("Read-only reply storage cannot save"));
        }
        let temporary = self.dir.join(format!(
            ".{}.{}.{}.tmp",
            digest(&self.row),
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let save = || -> io::Result<()> {
            before(SavePhase::Write)?;
            #[cfg(test)]
            OPERATIONS.with(|operations| operations.borrow_mut().push("create/write-temporary"));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            let bytes = serde_json::to_vec_pretty(record).map_err(io::Error::other)?;
            file.write_all(&bytes)?;
            before(SavePhase::FileSync)?;
            file.sync_all()?;
            before(SavePhase::Rename)?;
            fs::rename(&temporary, &self.path)?;
            before(SavePhase::DirectorySync)?;
            File::open(&self.dir)?.sync_all()?;
            if let Some(parent) = self.dir.parent() {
                File::open(parent)?.sync_all()?;
            }
            Ok(())
        };
        let mut save = save;
        let result = save().map_err(io_error);
        let _ = fs::remove_file(temporary);
        result
    }

    /// Saved unresolved rows are recoverable news even when a refresh drops them;
    /// enumerate only validated records, without creating locks (§FS-005-dispatch.13).
    pub fn pending_rows() -> Result<Vec<Item>> {
        Ok(Self::recovery().rows)
    }

    /// Invalid collection members are diagnostics, never permission to act
    /// (§FS-005-dispatch.13). Direct reads retain strict validation.
    pub fn recovery() -> Recovery {
        let dir = reply_dir();
        Self::recovery_in(&dir)
    }

    fn recovery_in(dir: &Path) -> Recovery {
        let mut recovery = Recovery::default();
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return recovery,
            Err(err) => {
                recovery.diagnostics.push(io_error(err).to_string());
                return recovery;
            }
        };
        for entry in entries {
            let path = match entry {
                Ok(entry) => entry.path(),
                Err(err) => {
                    recovery.diagnostics.push(io_error(err).to_string());
                    continue;
                }
            };
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let validate = || -> Result<Record> {
                let bytes = fs::read(&path).map_err(io_error)?;
                let record: Record = serde_json::from_slice(&bytes).map_err(|err| {
                    error(format!("Invalid saved reply {}: {err}", path.display()))
                })?;
                if path.file_name().and_then(|name| name.to_str())
                    != Some(format!("{}.json", digest(&record.row.id)).as_str())
                {
                    return Err(error("Saved reply filename does not match its row"));
                }
                let validated = Self {
                    dir: dir.to_path_buf(),
                    path: path.clone(),
                    row: record.row.id,
                    _lock: None,
                    writable: false,
                }
                .read()?
                .ok_or_else(|| error("Saved reply disappeared while reading"))?;
                Ok(validated)
            };
            match validate() {
                Ok(validated) if validated.intent.as_ref().is_some_and(Intent::unresolved) => {
                    recovery.rows.push(validated.row);
                }
                Ok(_) => {}
                Err(err) => recovery
                    .diagnostics
                    .push(format!("{}: {err}", path.display())),
            }
        }
        recovery.rows.sort_by(|a, b| a.id.cmp(&b.id));
        recovery.diagnostics.sort();
        recovery
    }
}

/// A stable 64-bit row digest with record identity validation (§FS-005-dispatch.13).
fn digest(row: &str) -> String {
    let hash = row.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

/// Persistence errors are refusals before any delivery (§FS-005-dispatch.13).
pub fn error(message: impl Into<String>) -> EphorError {
    EphorError::Command(message.into())
}
fn io_error(err: io::Error) -> EphorError {
    error(format!("Cannot save/read reply outcome: {err}"))
}

#[cfg(test)]
#[path = "tests.rs"]
pub(crate) mod tests;
