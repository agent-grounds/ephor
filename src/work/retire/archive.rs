//! The archive a retirement moves into, and the record that says what moved
//! (§FS-005-dispatch.36.4, §FS-005-dispatch.36.5).
//!
//! One retirement is one directory, `archive/<project>/<item-slug>/<stamp>/`,
//! under ephor's data directory: the record `retirement.json`, and one
//! directory per root something came from. The record is written `pending`
//! before the first move and flushed, so a retirement cut short is found and
//! finished or undone by the next one about the same matter.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::candidates::canonical;

/// The record's own name, and the version of what it holds.
pub const RECORD: &str = "retirement.json";
pub const SCHEMA: &str = "ephor-retirement/1";

pub const PENDING: &str = "pending";
pub const COMMITTED: &str = "committed";
pub const ROLLED_BACK: &str = "rolled-back";

/// Where every retirement is kept: ephor's data, never its state
/// (§FS-005-dispatch.36.4).
pub fn base() -> PathBuf {
    crate::paths::data_dir().join("archive")
}

/// The directory every retirement of one matter is kept under.
pub fn matter_dir(project: &str, item: &str) -> PathBuf {
    base().join(project).join(crate::slug::id_slug(item))
}

/// A fresh retirement directory under `matter`, stamped in UTC so a second
/// retirement after a reopen is kept apart from the first. Not created.
pub fn stamped(matter: &Path, at: DateTime<Utc>) -> PathBuf {
    let stamp = at.format("%Y%m%dT%H%M%S%.3fZ").to_string();
    let first = matter.join(&stamp);
    if fs::symlink_metadata(&first).is_err() {
        return first;
    }
    (2..)
        .map(|nth| matter.join(format!("{stamp}-{nth}")))
        .find(|path| fs::symlink_metadata(path).is_err())
        .unwrap_or(first)
}

/// The path as it will be once the parts that do not exist yet are made: the
/// canonical form of what does exist, with the rest joined on — so a data
/// directory reached through a symlink is compared where it really is
/// (§FS-005-dispatch.36.4).
pub fn resolved(path: &Path) -> PathBuf {
    canonical(path)
}

/// The first of `walked` the archive sits inside, where it sits inside one
/// (§FS-005-dispatch.36.4). Both sides are canonical already.
pub fn inside<'a>(archive: &Path, walked: &'a [PathBuf]) -> Option<&'a PathBuf> {
    walked
        .iter()
        .find(|place| !place.as_os_str().is_empty() && archive.starts_with(place))
}

/// The device a path's nearest existing ancestor is on, where that can be
/// said: a rename never crosses one (§FS-005-dispatch.36.4).
#[cfg(unix)]
pub fn device(path: &Path) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    let mut at = Some(path);
    while let Some(here) = at {
        if let Ok(meta) = fs::symlink_metadata(here) {
            return Some(meta.dev());
        }
        at = here.parent();
    }
    None
}

#[cfg(not(unix))]
pub fn device(_path: &Path) -> Option<u64> {
    None
}

/// One move, as the record lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Write the record, flushed to disk before anything relies on it
/// (§FS-005-dispatch.36.5).
pub fn write(path: &Path, record: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let mut file = File::create(&tmp)?;
        file.write_all(
            serde_json::to_string_pretty(record)
                .unwrap_or_default()
                .as_bytes(),
        )?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

/// Rewrite only the record's state.
pub fn mark(path: &Path, state: &str) -> std::io::Result<()> {
    let text = fs::read_to_string(path)?;
    let mut record: Value = serde_json::from_str(&text)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    record["state"] = Value::String(state.to_string());
    write(path, &record)
}

/// Make every move in order. On the first that fails, put back what moved,
/// in reverse, and say what failed and what could not be put back
/// (§FS-005-dispatch.36.5).
pub fn perform(moves: &[Move]) -> Result<(), (String, Vec<PathBuf>)> {
    for (done, step) in moves.iter().enumerate() {
        let made = step
            .to
            .parent()
            .map(fs::create_dir_all)
            .unwrap_or(Ok(()))
            .and_then(|_| match fs::symlink_metadata(&step.to) {
                Ok(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "the archive already holds this path",
                )),
                Err(_) => fs::rename(&step.from, &step.to),
            });
        if let Err(err) = made {
            let failed = format!(
                "moving {} to {} failed: {err}",
                step.from.display(),
                step.to.display()
            );
            return Err((failed, restore(&moves[..done])));
        }
    }
    Ok(())
}

/// Put every move back, last first. A move is restored only where its
/// original path is free, and one already back in its place counts as
/// restored; the paths that could not be restored are returned, and their
/// copies stay in the archive (§FS-005-dispatch.36.5).
pub fn restore(moves: &[Move]) -> Vec<PathBuf> {
    let mut not_restored = Vec::new();
    for step in moves.iter().rev() {
        let there = fs::symlink_metadata(&step.from).is_ok();
        let kept = fs::symlink_metadata(&step.to).is_ok();
        match (there, kept) {
            (true, false) => {}
            (false, true) => {
                let back = step
                    .from
                    .parent()
                    .map(fs::create_dir_all)
                    .unwrap_or(Ok(()))
                    .and_then(|_| fs::rename(&step.to, &step.from));
                if back.is_err() {
                    not_restored.push(step.from.clone());
                }
            }
            _ => not_restored.push(step.from.clone()),
        }
    }
    not_restored
}

/// Every record under one matter's directory still saying `pending`, oldest
/// first (§FS-005-dispatch.36.5).
pub fn pending(matter: &Path) -> Vec<(PathBuf, Value)> {
    let Ok(entries) = fs::read_dir(matter) else {
        return Vec::new();
    };
    let mut found: Vec<(PathBuf, Value)> = entries
        .flatten()
        .map(|entry| entry.path().join(RECORD))
        .filter_map(|path| {
            let record: Value = serde_json::from_str(&fs::read_to_string(&path).ok()?).ok()?;
            (record.get("state").and_then(Value::as_str) == Some(PENDING)).then_some((path, record))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// The moves a record lists.
pub fn moves_of(record: &Value) -> Vec<Move> {
    record
        .get("moves")
        .cloned()
        .and_then(|moves| serde_json::from_value(moves).ok())
        .unwrap_or_default()
}

/// The roots a record names, whose locks a recovery holds.
pub fn roots_of(record: &Value) -> Vec<PathBuf> {
    record
        .get("roots")
        .cloned()
        .and_then(|roots| serde_json::from_value(roots).ok())
        .unwrap_or_default()
}

/// Who and where, for the record: the host and the user, as the environment
/// says them.
pub fn host() -> Option<String> {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|name| !name.is_empty())
        .or_else(|| {
            fs::read_to_string("/etc/hostname")
                .ok()
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
        })
}

pub fn user() -> Option<String> {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §FS-005-dispatch.36.4: an archive reached through a symlink is
    /// compared where it really is, so an alias of a work root is inside it;
    /// a sibling that only shares a prefix of its name is not.
    #[cfg(unix)]
    #[test]
    fn the_archive_is_placed_by_its_canonical_path_through_aliases() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("forest");
        let root = real.join("panta");
        fs::create_dir_all(root.join("keep")).unwrap();
        fs::create_dir_all(tmp.path().join("panta-sibling")).unwrap();
        let alias = tmp.path().join("alias");
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let data = tmp.path().join("data");
        std::os::unix::fs::symlink(root.join("keep"), &data).unwrap();

        let walked = vec![resolved(&root)];
        let through_alias = resolved(&alias.join("panta/keep/ephor/archive/demo"));
        assert_eq!(inside(&through_alias, &walked), Some(&walked[0]));
        let through_data = resolved(&data.join("ephor/archive/demo/x"));
        assert_eq!(inside(&through_data, &walked), Some(&walked[0]));

        let apart = resolved(&tmp.path().join("panta-sibling/ephor/archive"));
        assert_eq!(
            inside(&apart, &walked),
            None,
            "a shared name prefix is not inside"
        );
        let elsewhere = resolved(&tmp.path().join("share/ephor/archive"));
        assert_eq!(inside(&elsewhere, &walked), None);
    }

    /// A restore never replaces a path that is occupied, and one already
    /// back counts as restored (§FS-005-dispatch.36.5).
    #[test]
    fn a_restore_puts_back_only_what_it_can_without_replacing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = |name: &str| tmp.path().join(name);
        fs::write(path("a.kept"), "a").unwrap();
        fs::write(path("b"), "b").unwrap();
        fs::write(path("c.kept"), "old").unwrap();
        fs::write(path("c"), "newer").unwrap();
        let moves = vec![
            Move {
                from: path("a"),
                to: path("a.kept"),
            },
            Move {
                from: path("b"),
                to: path("b.kept"),
            },
            Move {
                from: path("c"),
                to: path("c.kept"),
            },
        ];
        let not_restored = restore(&moves);
        assert_eq!(not_restored, vec![path("c")]);
        assert_eq!(fs::read_to_string(path("a")).unwrap(), "a");
        assert_eq!(fs::read_to_string(path("c")).unwrap(), "newer");
        assert_eq!(fs::read_to_string(path("c.kept")).unwrap(), "old");
    }
}
