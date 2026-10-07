//! Operation-bound outcome evidence, reserved before delivery and read through
//! the row store on every surface (§FS-005-dispatch.13, §FS-011-command-line.4).
//! This closes primary replacement failures, not loss of all outcome evidence.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::storage::{digest, error, io_error, Record, Status};
use crate::error::Result;

/// Real reservation and outcome writer boundaries (§FS-005-dispatch.13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidencePhase {
    ReserveWrite,
    ReserveFileSync,
    ReserveDirectorySync,
    OutcomeWrite,
    OutcomeFileSync,
}

/// The already opened append inode survives loss of directory write permission.
/// Its own write/sync can still fail (§FS-005-dispatch.13).
pub struct Receipt {
    file: File,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reservation {
    version: u32,
    operation: String,
    row: String,
    payload: Value,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Unknown {
    unknown: String,
}

/// Mutable status/note are excluded; all prepared context, target, words and
/// draft identity belong to the immutable operation (§FS-005-dispatch.13,
/// §FS-001-forge-interface.9).
fn payload(record: &Record) -> Result<Value> {
    let intent = record
        .intent
        .as_ref()
        .ok_or_else(|| error("Outcome evidence has no saved operation"))?;
    Ok(json!({
        "binding": intent.binding,
        "text": intent.text,
        "request": intent.request,
        "reconciliation": intent.reconciliation,
        "draft": intent.draft,
    }))
}

fn path(dir: &Path, record: &Record) -> Result<PathBuf> {
    let operation = record
        .evidence
        .as_deref()
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 100
                && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        })
        .ok_or_else(|| error("Saved reply evidence identity is invalid"))?;
    Ok(dir.join(format!("{}.{}.receipt", digest(&record.row.id), operation)))
}

/// Only a complete reservation, optionally followed by one complete unknown,
/// is readable. Missing, torn, conflicting or mismatched evidence refuses;
/// it never grants replay (§FS-005-dispatch.13).
fn read(dir: &Path, record: &Record) -> Result<Option<String>> {
    let bytes = fs::read(path(dir, record)?).map_err(io_error)?;
    let lines: Vec<_> = bytes.split(|byte| *byte == b'\n').collect();
    if !matches!(lines.len(), 2 | 3) || !bytes.ends_with(b"\n") {
        return Err(error(
            "Invalid saved reply evidence: incomplete or extra outcome",
        ));
    }
    let reservation: Reservation = serde_json::from_slice(lines[0])
        .map_err(|err| error(format!("Invalid saved reply evidence: {err}")))?;
    if reservation.version != 1
        || Some(&reservation.operation) != record.evidence.as_ref()
        || reservation.row != record.row.id
        || reservation.payload != payload(record)?
    {
        return Err(error(
            "Saved reply evidence does not match this operation/payload/context",
        ));
    }
    if lines.len() == 2 {
        return Ok(None);
    }
    let unknown: Unknown = serde_json::from_slice(lines[1])
        .map_err(|err| error(format!("Invalid saved reply evidence: {err}")))?;
    Ok(Some(unknown.unknown))
}

/// The shared effective status also governs enumeration and local decisions.
/// A successfully replaced checked decision supersedes this operation's hold;
/// the immutable evidence stays available for validation (§FS-005-dispatch.13).
pub(super) fn project(dir: &Path, record: &mut Record) -> Result<()> {
    if record.evidence.is_none() {
        return Ok(());
    }
    if let Some(note) = read(dir, record)? {
        let intent = record.intent.as_mut().unwrap();
        if intent.note.as_ref().is_some_and(|saved| saved != &note) {
            return Err(error(
                "Saved reply and outcome evidence have conflicting notes",
            ));
        }
        if !intent.unresolved() && intent.note.as_ref() != Some(&note) {
            return Err(error(
                "Saved resolution conflicts with the unknown outcome evidence",
            ));
        }
        if intent.unresolved() {
            intent.status = Status::Held;
            intent.note = Some(note);
        }
    }
    Ok(())
}

/// Synchronize the reservation inode and its name before saving the referring
/// version-2 row and invoking. Old readers reject version 2 (§FS-005-dispatch.13).
pub(super) fn reserve(
    dir: &Path,
    record: &mut Record,
    mut before: impl FnMut(EvidencePhase) -> io::Result<()>,
) -> Result<Receipt> {
    if record
        .intent
        .as_ref()
        .is_none_or(|intent| intent.status != Status::Pending)
    {
        return Err(error(
            "Only a pending operation can reserve outcome evidence",
        ));
    }
    if record.evidence.is_some() {
        if read(dir, record)?.is_some() {
            return Err(error("An explicit unknown cannot reserve a posting retry"));
        }
        let file = OpenOptions::new()
            .append(true)
            .open(path(dir, record)?)
            .map_err(io_error)?;
        return Ok(Receipt { file });
    }
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| error(err.to_string()))?
        .as_nanos();
    let operation = format!(
        "{nonce:x}-{:x}-{:x}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    );
    let reservation = Reservation {
        version: 1,
        operation: operation.clone(),
        row: record.row.id.clone(),
        payload: payload(record)?,
    };
    let receipt_path = dir.join(format!("{}.{}.receipt", digest(&record.row.id), operation));
    let mut created = false;
    let write = || -> io::Result<File> {
        before(EvidencePhase::ReserveWrite)?;
        let mut file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(&receipt_path)?;
        created = true;
        let mut bytes = serde_json::to_vec(&reservation).map_err(io::Error::other)?;
        bytes.push(b'\n');
        file.write_all(&bytes)?;
        before(EvidencePhase::ReserveFileSync)?;
        file.sync_all()?;
        before(EvidencePhase::ReserveDirectorySync)?;
        File::open(dir)?.sync_all()?;
        if let Some(parent) = dir.parent() {
            File::open(parent)?.sync_all()?;
        }
        Ok(file)
    };
    let mut write = write;
    let file = match write() {
        Ok(file) => file,
        Err(err) => {
            if created {
                let _ = fs::remove_file(receipt_path);
            }
            return Err(io_error(err));
        }
    };
    record.evidence = Some(operation);
    record.version = 2;
    Ok(Receipt { file })
}

impl Receipt {
    /// Append then fsync before attempting Held row replacement. A partial write
    /// refuses on read; a failed first write or pre-write death may leave only
    /// the reservation, indistinguishable from lost acknowledgement. This is
    /// the remaining author-decision boundary (§FS-005-dispatch.13).
    pub fn hold_with(
        &mut self,
        note: &str,
        mut before: impl FnMut(EvidencePhase) -> io::Result<()>,
    ) -> Result<()> {
        let mut bytes = serde_json::to_vec(&Unknown {
            unknown: note.into(),
        })
        .map_err(|err| error(err.to_string()))?;
        bytes.push(b'\n');
        before(EvidencePhase::OutcomeWrite).map_err(io_error)?;
        self.file.write_all(&bytes).map_err(io_error)?;
        before(EvidencePhase::OutcomeFileSync).map_err(io_error)?;
        self.file.sync_all().map_err(io_error)
    }
}
