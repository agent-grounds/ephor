//! §AR-007-runtime.1, Holding: the binding's own exclusion, taken and held for
//! a move ephor makes on its files, and the knowledge of which of those files
//! are coordination state rather than work
//! (§FS-005-dispatch.36.3, §DA-011-retire-holds-the-runtimes-locks).
//!
//! Named `exclusion` rather than `hold` because `work::hold` is already the
//! hand on a ledger entry, and the two have nothing to do with each other.
//!
//! What goes up is a held set of locks, released when it is dropped, or a
//! refusal naming the root and what held it — never a lock path a surface has
//! to interpret. The board's probe in `watch` stays a probe that releases at
//! once; only the run lock's path is shared with it, declared here.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::plan;
use super::runner;
use crate::work::recipe::WorkConfig;

/// The lock a run holds on its execution root, under the root
/// (§AR-007-runtime.1). The board's liveness probe reads the same file.
pub(super) const RUN_LOCK: &str = ".rhei/run.lock";

/// The runtime's own directory inside a root: coordination state, never work.
const RUNTIME_DIR: &str = ".rhei";

/// The marker a forced recovery leaves in a root, and the one a pending
/// removal leaves at the project root (§FS-005-dispatch.36.3).
const FORCED_RECOVERY: &str = ".rhei/forced-recovery.json";
const PENDING_REMOVAL: &str = ".rhei/pending-removal.json";

/// The suffix of every lock file the runtime keeps beside the file it guards.
const LOCK_SUFFIX: &str = ".lock";

/// Where a shared root keeps the runtime's per-session snapshot cache: one
/// entry per task session, named after the task id
/// (`<task>-<state>-<slug>-<nonce>`).
const SNAPSHOT_SESSIONS: &str = "runtime/snapshot-sessions";

/// The index that makes a directory a workspace.
const INDEX: &str = "index.rhei.md";

/// The listing verb, and how long it gets: a local read.
const RUNS_VERB: [&str; 2] = ["runs", "--json"];

/// One unit a move takes, as the runtime sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unit {
    /// A single plan file in a root it shares.
    File(PathBuf),
    /// A directory workspace, whose root is itself.
    Workspace(PathBuf),
}

impl Unit {
    /// The unit a plan path belongs to: the directory of a workspace's index,
    /// or the file itself.
    pub fn of(plan: &Path) -> Unit {
        match plan::own_store(plan) {
            Some(dir) => Unit::Workspace(dir.to_path_buf()),
            None => Unit::File(plan.to_path_buf()),
        }
    }

    /// The path that moves: the file, or the workspace directory.
    pub fn path(&self) -> &Path {
        match self {
            Unit::File(path) | Unit::Workspace(path) => path,
        }
    }

    /// Its execution root (§FS-005-dispatch.36.3).
    pub fn root(&self) -> PathBuf {
        match self {
            Unit::Workspace(dir) => dir.clone(),
            Unit::File(file) => file.parent().map(Path::to_path_buf).unwrap_or_default(),
        }
    }
}

/// The plan file of a unit's path: a workspace's index, or the file itself.
pub fn plan_file(unit: &Path) -> PathBuf {
    match unit.is_dir() {
        true => unit.join(INDEX),
        false => unit.to_path_buf(),
    }
}

/// Every root a move of `units` reaches, in the runtime's canonical order —
/// sorted absolute paths (§FS-005-dispatch.36.3): each unit's execution root,
/// and for a single file's shared root every workspace it holds as well.
pub fn roots(units: &[Unit]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for unit in units {
        roots.push(unit.root());
        if let Unit::File(_) = unit {
            for found in plan::plans_in(&unit.root()) {
                if let Some(dir) = plan::own_store(&found.path) {
                    roots.push(dir.to_path_buf());
                }
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

/// Every writer lock guarding a file the move of `unit` takes, as a rule
/// rather than a list (§FS-005-dispatch.36.3). A single file's own lock; for a
/// workspace its index's and each task file's, whether present or not, and
/// every lock already under it outside the runtime's own directory — which is
/// how a lock the runtime adds for a file there later is held too.
pub fn writer_locks(unit: &Unit) -> Vec<PathBuf> {
    let mut locks = Vec::new();
    match unit {
        Unit::File(file) => locks.push(plan::sidecar_of(file)),
        Unit::Workspace(dir) => {
            locks.push(plan::sidecar_of(&dir.join(INDEX)));
            if let Ok(entries) = fs::read_dir(dir.join("tasks")) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && !is_lock(&path) {
                        locks.push(plan::sidecar_of(&path));
                    }
                }
            }
            existing_locks(dir, &mut locks);
        }
    }
    locks.sort();
    locks.dedup();
    locks
}

/// The run lock of each root, in the order given: what a recovery holds while
/// it puts a half-done move back (§FS-005-dispatch.36.5).
pub fn run_locks(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots.iter().map(|root| root.join(RUN_LOCK)).collect()
}

/// Every lock the move of `units` holds, in order: each root's run lock by
/// sorted root, then each unit's writer locks (§FS-005-dispatch.36.3).
pub fn lock_set(units: &[Unit]) -> Vec<PathBuf> {
    let mut set: Vec<PathBuf> = roots(units)
        .iter()
        .map(|root| root.join(RUN_LOCK))
        .collect();
    for unit in units {
        for lock in writer_locks(unit) {
            if !set.contains(&lock) {
                set.push(lock);
            }
        }
    }
    set
}

fn existing_locks(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() && entry.file_name() != RUNTIME_DIR {
            existing_locks(&path, into);
        } else if kind.is_file() && is_lock(&path) {
            into.push(path);
        }
    }
}

fn is_lock(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(LOCK_SUFFIX))
}

/// Whether a path inside a workspace is coordination state that stays where
/// it is (§FS-005-dispatch.36.4): the runtime's own directory, and every lock.
fn stays(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == RUNTIME_DIR) || is_lock(path)
}

/// What leaves a workspace when its content moves (§FS-005-dispatch.36.4):
/// every entry but what stays, where a directory holding something that stays
/// is entered and emptied of everything else rather than moved. Sorted, so a
/// move and its rollback walk the same order.
pub fn leaving(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    leaving_into(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn leaving_into(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if stays(&path) {
            continue;
        }
        if entry.file_type()?.is_dir() && holds_staying(&path)? {
            leaving_into(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn holds_staying(dir: &Path) -> std::io::Result<bool> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if stays(&path) || (entry.file_type()?.is_dir() && holds_staying(&path)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// What a moved single plan leaves in its shared root
/// (§FS-005-dispatch.36.5): its lock file, its files under the root's results
/// directory, and its entries in the runtime's snapshot-session cache.
pub fn left_behind(plan_file: &Path, plan_id: &str) -> Vec<PathBuf> {
    let mut left = vec![plan::sidecar_of(plan_file)];
    let Some(root) = plan_file.parent() else {
        return left;
    };
    let prefix = format!("{plan_id}.");
    for dir in [
        root.join(super::results::RESULTS),
        root.join(SNAPSHOT_SESSIONS),
    ] {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
            .map(|entry| entry.path())
            .collect();
        found.sort();
        left.extend(found);
    }
    left
}

/// Why a move cannot have a root (§FS-005-dispatch.36.3): the root, and one
/// sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub root: PathBuf,
    pub says: String,
}

fn held(root: &Path) -> Refused {
    Refused {
        root: root.to_path_buf(),
        says: format!(
            "{} is held by a live run; retire again when it ends",
            root.display()
        ),
    }
}

/// A marker the runtime left in one of `roots`, or at the project root above
/// a workspace (§FS-005-dispatch.36.3).
pub fn marker(roots: &[PathBuf]) -> Option<Refused> {
    for root in roots {
        let mut places = vec![root.clone()];
        if let Some(project) = root.parent().filter(|_| root.join(INDEX).exists()) {
            places.push(project.to_path_buf());
        }
        for place in places {
            for (name, what) in [
                (FORCED_RECOVERY, "a recovery the runtime forced"),
                (PENDING_REMOVAL, "a removal the runtime has pending"),
            ] {
                let marker = place.join(name);
                if fs::symlink_metadata(&marker).is_ok() {
                    return Some(Refused {
                        root: root.clone(),
                        says: format!(
                            "{} is held: {} marks {what}; retire again once it is resolved",
                            root.display(),
                            marker.display()
                        ),
                    });
                }
            }
        }
    }
    None
}

/// Whether the runtime's own listing of runs reports any of `roots` live, or
/// cannot decide about one (§FS-005-dispatch.36.3). A listing that fails, or
/// warns about a root, is a root nobody can vouch for.
pub fn listed_live(config: &WorkConfig, roots: &[PathBuf]) -> Option<Refused> {
    let first = roots.first()?;
    let output = match Command::new(runner(config)).args(RUNS_VERB).output() {
        Ok(output) => output,
        Err(err) => {
            return Some(Refused {
                root: first.clone(),
                says: format!(
                    "{} cannot be checked for a live run: {} {} failed: {err}",
                    first.display(),
                    runner(config),
                    RUNS_VERB.join(" ")
                ),
            })
        }
    };
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Some(Refused {
            root: first.clone(),
            says: format!(
                "{} cannot be checked for a live run: {} {} said: {}",
                first.display(),
                runner(config),
                RUNS_VERB.join(" "),
                stderr.trim()
            ),
        });
    }
    for root in roots {
        let named = root.to_string_lossy();
        if let Some(line) = stderr.lines().find(|line| line.contains(named.as_ref())) {
            return Some(Refused {
                root: root.clone(),
                says: format!(
                    "{} cannot be checked for a live run: {}",
                    root.display(),
                    line.trim()
                ),
            });
        }
    }
    let runs: Vec<serde_json::Value> = match serde_json::from_slice(&output.stdout) {
        Ok(runs) => runs,
        Err(err) => {
            return Some(Refused {
                root: first.clone(),
                says: format!(
                    "{} cannot be checked for a live run: the listing did not read: {err}",
                    first.display()
                ),
            })
        }
    };
    for run in &runs {
        let over = run
            .get("status")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|status| super::FINISHED.contains(&status));
        let Some(workspace) = run.get("workspace").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let workspace = fs::canonicalize(workspace).unwrap_or_else(|_| PathBuf::from(workspace));
        if let Some(root) = roots
            .iter()
            .find(|root| **root == workspace)
            .filter(|_| !over)
        {
            return Some(held(root));
        }
    }
    None
}

/// Locks held for a move, released when dropped (§FS-005-dispatch.36.3).
#[derive(Debug, Default)]
pub struct Held {
    files: Vec<File>,
    /// The lock files this hold created, absent before it.
    pub created: Vec<PathBuf>,
}

impl Held {
    /// How many locks are held.
    pub fn count(&self) -> usize {
        self.files.len()
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        for file in self.files.drain(..).rev() {
            let _ = file.unlock();
        }
    }
}

/// Take every lock in `locks`, in order, non-blocking and exclusive
/// (§FS-005-dispatch.36.3). An absent lock file is created empty, as the
/// runtime creates it, and never truncated. A lock already taken refuses,
/// naming the root it guards; what was taken so far is released.
///
/// `probe` is the dry run: an absent lock file is left absent, and each lock
/// is released as soon as it was taken.
pub fn take(locks: &[PathBuf], roots: &[PathBuf], probe: bool) -> Result<Held, Refused> {
    let mut held_set = Held::default();
    for lock in locks {
        let root = root_of(lock, roots);
        let exists = lock.exists();
        if probe && !exists {
            continue;
        }
        if !exists {
            if let Some(parent) = lock.parent() {
                fs::create_dir_all(parent).map_err(|err| cannot(&root, lock, &err))?;
            }
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(!probe)
            .truncate(false)
            .open(lock)
            .map_err(|err| cannot(&root, lock, &err))?;
        if !exists {
            held_set.created.push(lock.clone());
        }
        match file.try_lock() {
            Ok(()) if probe => {
                let _ = file.unlock();
            }
            Ok(()) => held_set.files.push(file),
            Err(fs::TryLockError::WouldBlock) => return Err(held(&root)),
            Err(fs::TryLockError::Error(err)) => return Err(cannot(&root, lock, &err)),
        }
    }
    Ok(held_set)
}

fn cannot(root: &Path, lock: &Path, err: &std::io::Error) -> Refused {
    Refused {
        root: root.to_path_buf(),
        says: format!(
            "{} cannot be locked: {}: {err}",
            root.display(),
            lock.display()
        ),
    }
}

/// The root a lock guards: the deepest of `roots` it sits under.
fn root_of(lock: &Path, roots: &[PathBuf]) -> PathBuf {
    roots
        .iter()
        .filter(|root| lock.starts_with(root))
        .max_by_key(|root| root.components().count())
        .cloned()
        .unwrap_or_else(|| lock.parent().map(Path::to_path_buf).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// The lock set and its order, for a single file and a directory, and a
    /// missing lock created empty and never truncated (§FS-005-dispatch.36.3).
    #[test]
    fn the_lock_set_is_run_locks_by_sorted_root_then_writer_locks() {
        let tmp = tempfile::tempdir().unwrap();
        let shared = tmp.path().join("panta");
        let single = shared.join("a-plan.rhei.md");
        write(&single, "# Rhei: a\n");
        let workspace = shared.join("b-work");
        write(&workspace.join(INDEX), "# Rhei: b\n");
        write(&workspace.join("tasks/01-fix.md"), "### Task fix: x\n");
        write(&workspace.join("runtime/metrics/.lock"), "");
        write(&workspace.join(".rhei/run.lock"), "");

        let units = vec![Unit::of(&single)];
        let set = lock_set(&units);
        assert_eq!(
            set,
            vec![
                shared.join(RUN_LOCK),
                workspace.join(RUN_LOCK),
                plan::sidecar_of(&single),
            ],
            "a single file locks its shared root and every workspace in it"
        );

        let units = vec![Unit::of(&workspace.join(INDEX))];
        let set = lock_set(&units);
        assert_eq!(
            set,
            vec![
                workspace.join(RUN_LOCK),
                workspace.join("index.rhei.md.lock"),
                workspace.join("runtime/metrics/.lock"),
                workspace.join("tasks/01-fix.md.lock"),
            ]
        );

        // Taken: an absent lock is created empty; a present one keeps its bytes.
        write(&workspace.join("index.rhei.md.lock"), "kept");
        let roots = roots(&units);
        let held = take(&set, &roots, false).unwrap();
        assert_eq!(held.count(), set.len());
        assert!(held
            .created
            .contains(&workspace.join("tasks/01-fix.md.lock")));
        assert_eq!(
            fs::read_to_string(workspace.join("index.rhei.md.lock")).unwrap(),
            "kept"
        );
        assert_eq!(
            fs::read_to_string(workspace.join("tasks/01-fix.md.lock")).unwrap(),
            ""
        );
        // While held, a second taker is refused naming the root.
        let refused = take(&set, &roots, true).unwrap_err();
        assert_eq!(refused.root, workspace);
        assert!(refused.says.contains("held by a live run"));
        drop(held);
        assert!(take(&set, &roots, true).is_ok());
    }

    #[test]
    fn a_probe_creates_no_lock_file() {
        let tmp = tempfile::tempdir().unwrap();
        let single = tmp.path().join("a.rhei.md");
        write(&single, "# Rhei: a\n");
        let units = vec![Unit::of(&single)];
        let held = take(&lock_set(&units), &roots(&units), true).unwrap();
        assert!(held.created.is_empty());
        assert!(!plan::sidecar_of(&single).exists());
        assert!(!tmp.path().join(RUN_LOCK).exists());
    }

    #[test]
    fn a_workspace_leaves_its_locks_and_runtime_directory_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("w");
        write(&dir.join(INDEX), "x");
        write(&dir.join("index.rhei.md.lock"), "");
        write(&dir.join("tasks/01.md"), "x");
        write(&dir.join("tasks/01.md.lock"), "");
        write(&dir.join("states.yaml"), "x");
        write(&dir.join(".rhei/run.lock"), "");
        write(&dir.join("notes/a.md"), "x");
        assert_eq!(
            leaving(&dir).unwrap(),
            vec![
                dir.join(INDEX),
                dir.join("notes"),
                dir.join("states.yaml"),
                dir.join("tasks/01.md"),
            ]
        );
    }
}
