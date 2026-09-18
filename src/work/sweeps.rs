//! When each recipe last swept for its own matters (§FS-005-dispatch.31).
//!
//! Ephor's record of ephor's own activity, and never a claim about the work.
//! It is kept the way `burn`'s cursors are kept — a small file under ephor's
//! own state directory — rather than in the ledger, which answers one question
//! and goes on answering only that one (§FS-005-dispatch.4).
//!
//! The reading is deliberately forgiving in one direction. A record that is
//! missing, unreadable, or written by a version that spelled it differently
//! means **due now**: a reader who deletes this state loses a sweep's worth of
//! waiting rather than the sweep itself, and erring toward one early sweep is
//! the right direction where erring the other way is a queue that silently
//! stops being looked at (§REQ-001-boundary.1).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{EphorError, Result};

/// Every recipe's last sweep, keyed by project and recipe id.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Sweeps {
    #[serde(default = "version")]
    pub version: u32,
    /// Keyed `<project>/<recipe-id>`. Per project because that is what a
    /// recipe is: the same id resolves to a different recipe in a project that
    /// replaced it (§FS-005-dispatch.1), and a sweep narrowed to one project
    /// must not spend another project's clock.
    #[serde(default)]
    pub swept: BTreeMap<String, DateTime<Utc>>,
}

fn version() -> u32 {
    1
}

/// The one key a (project, recipe) pair is written under.
pub fn key(project: &str, recipe: &str) -> String {
    format!("{project}/{recipe}")
}

/// Where the record lives: beside the feed cache and the burn store, under
/// ephor's own state directory (§REQ-001-boundary.4).
pub fn path() -> PathBuf {
    crate::paths::state_dir().join("sweeps.json")
}

/// The record as it stands. Anything that cannot be read is an empty record,
/// which makes every recipe due — see the module note.
pub fn load() -> Sweeps {
    load_from(&path())
}

fn load_from(path: &Path) -> Sweeps {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write the record back, through a temporary file in the same directory so a
/// sweep interrupted half way leaves the previous record rather than a
/// truncated one — the same care `burn`'s cursors are written with.
pub fn store(sweeps: &Sweeps) -> Result<()> {
    store_at(&path(), sweeps)
}

fn store_at(path: &Path, sweeps: &Sweeps) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            EphorError::Command(format!("Cannot create {}: {err}", parent.display()))
        })?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(sweeps)
        .map_err(|err| EphorError::Command(format!("Cannot write the sweep record: {err}")))?;
    fs::write(&tmp, text)
        .map_err(|err| EphorError::Command(format!("Cannot write {}: {err}", tmp.display())))?;
    fs::rename(&tmp, path)
        .map_err(|err| EphorError::Command(format!("Cannot rename {}: {err}", tmp.display())))
}

impl Sweeps {
    /// When this recipe last swept in this project, where anything was
    /// recorded. `None` is due now.
    pub fn last(&self, project: &str, recipe: &str) -> Option<DateTime<Utc>> {
        self.swept.get(&key(project, recipe)).copied()
    }

    /// Record that it swept. Called for the sweep a timer ran *and* for one a
    /// reader typed: `ephor work dispatch` opens what a self-sweeping recipe
    /// would have opened, so a record that ignored it would send the timer to
    /// look again at a queue a person had just emptied by hand
    /// (§FS-005-dispatch.31).
    pub fn mark(&mut self, project: &str, recipe: &str, at: DateTime<Utc>) {
        self.swept.insert(key(project, recipe), at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minutes: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + minutes * 60, 0).unwrap()
    }

    #[test]
    fn a_recipe_nothing_was_recorded_about_has_no_last_sweep() {
        let sweeps = Sweeps::default();
        assert_eq!(sweeps.last("widget", "implement"), None);
    }

    #[test]
    fn the_clock_is_per_project_and_per_recipe() {
        let mut sweeps = Sweeps::default();
        sweeps.mark("widget", "implement", at(0));
        // The same id in another project is another recipe, and keeps its own
        // clock (§FS-005-dispatch.31).
        assert_eq!(sweeps.last("widget", "implement"), Some(at(0)));
        assert_eq!(sweeps.last("gadget", "implement"), None);
        assert_eq!(sweeps.last("widget", "fix-gate"), None);
    }

    #[test]
    fn a_later_sweep_replaces_the_earlier_one() {
        let mut sweeps = Sweeps::default();
        sweeps.mark("widget", "implement", at(0));
        sweeps.mark("widget", "implement", at(60));
        assert_eq!(sweeps.last("widget", "implement"), Some(at(60)));
    }

    #[test]
    fn a_record_round_trips_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sweeps.json");
        let mut sweeps = Sweeps::default();
        sweeps.mark("widget", "implement", at(0));
        store_at(&path, &sweeps).unwrap();
        assert_eq!(load_from(&path).last("widget", "implement"), Some(at(0)));
    }

    /// A record that is not there, and one that cannot be parsed, are the same
    /// thing and both mean *due now* (§FS-005-dispatch.31).
    #[test]
    fn a_missing_or_unreadable_record_is_an_empty_one() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("sweeps.json");
        assert!(load_from(&missing).swept.is_empty());

        let corrupt = dir.path().join("corrupt.json");
        fs::write(&corrupt, "{ this is not json").unwrap();
        assert!(load_from(&corrupt).swept.is_empty());

        // And one whose shape a later version changed reads as empty too,
        // rather than refusing and stopping the sweep it was written to pace.
        let alien = dir.path().join("alien.json");
        fs::write(&alien, r#"{"version": 99, "swept": "not a map"}"#).unwrap();
        assert!(load_from(&alien).swept.is_empty());
    }
}
