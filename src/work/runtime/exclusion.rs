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

/// The transition log's writer lock, which the runtime keeps at the root beside
/// `runtime/` rather than in it, so a reset that replaces `runtime/` keeps it.
const TRANSITION_LOG_LOCK: &str = "runtime.state-transitions.log.lock";

/// The one lock every metrics writer takes, inside the metrics directory.
const METRICS_LOCK: &str = "runtime/metrics/.lock";

/// The runtime's directory of per-run files inside a root.
const RUNTIME: &str = "runtime";

/// Where a root keeps the runtime's snapshot cache by default: one entry per
/// task, named after the task id.
const SNAPSHOT_CACHE: &str = ".rhei/cache/snapshots";

/// The index that makes a directory a workspace.
const INDEX: &str = "index.rhei.md";

/// The listing verb, and how long it gets: a local read.
const RUNS_VERB: [&str; 2] = ["runs", "--json"];

/// How the listing starts the line it prints on stderr for a run it could not
/// decide about, which goes on with that run's summary line.
const UNDECIDED: &str = "warning: could not check ";

/// What stands in a summary line for a run whose entry could not be read at
/// all, so it names no plan and could be any root's.
const UNREADABLE_ENTRY: &str = "(unreadable entry)";

/// The word a run's summary line puts before its count of parallel slots, two
/// spaces after which comes the run's plan path.
const PARALLEL: &str = " parallel ";

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
/// workspace its index's, each task file's, the transition log's and the
/// metrics', whether present or not, where the runtime puts each, and every
/// lock already under it outside the runtime's own directory — which is how a
/// lock the runtime adds for a file there later is held too. The runtime keeps
/// no lock for notes of its own, so none is made up; one present is held.
pub fn writer_locks(unit: &Unit) -> Vec<PathBuf> {
    let mut locks = Vec::new();
    match unit {
        Unit::File(file) => locks.push(plan::sidecar_of(file)),
        Unit::Workspace(dir) => {
            locks.push(plan::sidecar_of(&dir.join(INDEX)));
            locks.push(dir.join(TRANSITION_LOG_LOCK));
            locks.push(dir.join(METRICS_LOCK));
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

/// What a moved plan leaves in the root it shared (§FS-005-dispatch.36.5).
///
/// A single plan leaves its lock file; every file the runtime keeps for each
/// of its tickets there, found by the runtime's own naming of them — the
/// result, the logs, spawns and reports, the snapshot sessions, the worktree
/// reference, the exports, the accounting, and the snapshot cache; and every
/// file ephor's own states and drafts keep there for each ticket — its report,
/// verdict, failures and reply — and for the plan, by ephor's naming of them.
/// The lock file first, then the rest sorted.
///
/// A workspace is its own root, so it takes the runtime's files with it; what
/// it leaves is what ephor keeps for it in the root it was laid in, a
/// workflow's answer.
pub fn left_behind(unit: &Path, plan_id: &str) -> Vec<PathBuf> {
    let workspace = unit.is_dir();
    let mut found = Vec::new();
    if let Some(root) = unit.parent() {
        found.extend(
            super::results::kept_for_plan(root, plan_id)
                .into_iter()
                .filter(|path| fs::symlink_metadata(path).is_ok()),
        );
        let read = match workspace {
            true => None,
            false => plan::Plan::read(unit).ok().flatten(),
        };
        for ticket in read.iter().flat_map(|read| read.tickets()) {
            let task = format!("{plan_id}.{}", ticket.id);
            for (dir, named) in kept_for(root, &task) {
                let Ok(entries) = fs::read_dir(&dir) else {
                    continue;
                };
                found.extend(
                    entries
                        .flatten()
                        .filter(|entry| named.names(&entry.file_name().to_string_lossy()))
                        .map(|entry| entry.path()),
                );
            }
        }
    }
    found.sort();
    found.dedup();
    if workspace {
        return found;
    }
    let mut left = vec![plan::sidecar_of(unit)];
    left.extend(found);
    left
}

/// How the runtime names one task's entry in a directory.
enum Named {
    /// The one entry of this name.
    Exact(String),
    /// Every entry whose name starts with this.
    Prefixed(String),
}

impl Named {
    fn names(&self, name: &str) -> bool {
        match self {
            Named::Exact(exact) => name == exact,
            Named::Prefixed(prefix) => name.starts_with(prefix.as_str()),
        }
    }
}

/// Every place a file is kept for the task `task` in `root`, and how it is
/// named there: the runtime's own rules for what one task leaves in a root's
/// `runtime/`, its snapshot cache's default home and identity, and ephor's own
/// rule for what its states and drafts write for a task.
fn kept_for(root: &Path, task: &str) -> Vec<(PathBuf, Named)> {
    let runtime = root.join(RUNTIME);
    let accounting = accounting_segment(task);
    vec![
        (
            super::results::artifacts(root),
            Named::Prefixed(super::results::task_prefix(task)),
        ),
        (
            root.join(super::results::RESULTS),
            Named::Exact(format!("{task}.md")),
        ),
        (
            runtime.join("logs"),
            Named::Prefixed(format!("task-{task}-")),
        ),
        (
            runtime.join("spawns"),
            Named::Prefixed(format!("task-{task}-")),
        ),
        // A session report is named after the log it renders.
        (
            runtime.join("reports"),
            Named::Prefixed(format!("task-{task}-")),
        ),
        (
            runtime.join("snapshot-sessions"),
            Named::Prefixed(format!("{task}-")),
        ),
        (
            runtime.join("worktree-refs"),
            Named::Exact(format!("{task}.yaml")),
        ),
        (runtime.join("exports"), Named::Exact(task.to_string())),
        (
            runtime.join("accounting").join("captures"),
            Named::Prefixed(format!("{accounting}-")),
        ),
        (
            runtime.join("accounting").join("tasks"),
            Named::Exact(format!("{accounting}.json")),
        ),
        (root.join(SNAPSHOT_CACHE), Named::Exact(task.to_string())),
    ]
}

/// A task id as the runtime writes it into an accounting file name: letters,
/// digits, `.`, `_` and `-` kept, every other byte as `%XX`, and `task` for an
/// empty one.
fn accounting_segment(task: &str) -> String {
    let mut encoded = String::new();
    for byte in task.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-' => {
                encoded.push(byte as char)
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    match encoded.is_empty() {
        true => "task".to_string(),
        false => encoded,
    }
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
    for line in stderr.lines() {
        let root = match warned(line, roots) {
            Warned::Nothing => continue,
            Warned::Root(root) => root,
            Warned::Every => first,
        };
        return Some(Refused {
            root: root.clone(),
            says: format!(
                "{} cannot be checked for a live run: {}",
                root.display(),
                line.trim()
            ),
        });
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

/// Which of the roots one line the listing printed on stderr leaves
/// undecided (§FS-005-dispatch.36.3).
#[derive(Debug, PartialEq, Eq)]
enum Warned<'a> {
    /// None: the line is not about a run it could not decide, or its run's
    /// plan is in none of the roots.
    Nothing,
    /// The deepest root the undecided run's plan is in.
    Root(&'a PathBuf),
    /// Every root: the run names no plan that could be read, so it could be
    /// any root's.
    Every,
}

fn warned<'a>(line: &str, roots: &'a [PathBuf]) -> Warned<'a> {
    let Some(about) = line.trim().strip_prefix(UNDECIDED) else {
        return Warned::Nothing;
    };
    if about.starts_with(UNREADABLE_ENTRY) {
        return Warned::Every;
    }
    let Some(plan) = summary_plan(about) else {
        return Warned::Every;
    };
    roots
        .iter()
        .filter(|root| plan.starts_with(root))
        .max_by_key(|root| root.components().count())
        .map_or(Warned::Nothing, Warned::Root)
}

/// The plan path a run's summary line ends with, up to the `: ` the warning
/// puts before its reason. The runtime writes it absolute, so one that is not
/// is not a plan path read right.
fn summary_plan(about: &str) -> Option<PathBuf> {
    let (_, after) = about.split_once(PARALLEL)?;
    let (_, plan) = after.trim_start().split_once("  ")?;
    let plan = plan.split_once(": ").map_or(plan, |(plan, _)| plan);
    let plan = PathBuf::from(plan.trim());
    plan.is_absolute().then_some(plan)
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
        write(&workspace.join("runtime.notes.md.lock"), "");
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
                workspace.join("runtime.notes.md.lock"),
                workspace.join("runtime.state-transitions.log.lock"),
                workspace.join("tasks/01-fix.md.lock"),
            ],
            "the index's, the transition log's, the metrics' and each task's \
             lock whether present or not, and a lock already present (notes)"
        );

        // Taken: an absent lock is created empty; a present one keeps its bytes.
        write(&workspace.join("index.rhei.md.lock"), "kept");
        let roots = roots(&units);
        let held = take(&set, &roots, false).unwrap();
        assert_eq!(held.count(), set.len());
        for absent in [
            "tasks/01-fix.md.lock",
            "runtime.state-transitions.log.lock",
            "runtime/metrics/.lock",
        ] {
            let lock = workspace.join(absent);
            assert!(held.created.contains(&lock), "{absent} is created");
            assert_eq!(fs::read_to_string(&lock).unwrap(), "", "{absent} is empty");
        }
        assert!(!held
            .created
            .contains(&workspace.join("runtime.notes.md.lock")));
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

    /// A listing whose `runs --json` prints `warnings` on stderr and an empty
    /// list, from a stub runner.
    #[cfg(unix)]
    fn listing(dir: &Path, warnings: &[String]) -> WorkConfig {
        use std::os::unix::fs::PermissionsExt;
        let said: String = warnings
            .iter()
            .map(|line| format!("echo '{line}' >&2\n"))
            .collect();
        let runner = dir.join("runtime-stub");
        fs::write(&runner, format!("#!/bin/sh\n{said}echo '[]'\n")).unwrap();
        fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
        // Executed by path, so wait out `ETXTBSY` from another thread's fork.
        crate::seams::summons::settle_executable(&runner);
        WorkConfig {
            runner: Some(runner.display().to_string()),
            ..WorkConfig::default()
        }
    }

    /// A run the listing could not decide about is matched to a root by its
    /// plan's path, component by component: a root whose name is a prefix of
    /// the plan's directory is not its root (§FS-005-dispatch.36.3).
    #[cfg(unix)]
    #[test]
    fn an_undecided_run_is_matched_to_a_root_by_its_plans_path() {
        let tmp = tempfile::tempdir().unwrap();
        let panta = tmp.path().join("panta");
        let workspace = panta.join("w");
        let roots = vec![panta.clone(), workspace.clone()];
        let line = |plan: &Path| {
            format!(
                "warning: could not check r-1  running   headless   pid 41      \
                 parallel 1  {}: {}/.rhei/run.descriptor could not be read: denied",
                plan.display(),
                tmp.path().display()
            )
        };
        let neighbour = line(&tmp.path().join("panta2/p.rhei.md"));
        assert_eq!(warned(&neighbour, &roots), Warned::Nothing);
        let config = listing(tmp.path(), &[neighbour]);
        assert_eq!(listed_live(&config, &roots), None, "panta2 is not panta");

        let shared = line(&panta.join("p.rhei.md"));
        assert_eq!(warned(&shared, &roots), Warned::Root(&panta));
        let inner = line(&workspace.join("index.rhei.md"));
        assert_eq!(warned(&inner, &roots), Warned::Root(&workspace));
        let config = listing(tmp.path(), &[inner.clone()]);
        assert_eq!(
            listed_live(&config, &roots),
            Some(Refused {
                root: workspace.clone(),
                says: format!(
                    "{} cannot be checked for a live run: {inner}",
                    workspace.display()
                ),
            })
        );
    }

    /// A run whose registry entry could not be read names no plan, so it is
    /// undecided for every root and refuses, quoting the line
    /// (§FS-005-dispatch.36.3).
    #[cfg(unix)]
    #[test]
    fn an_unreadable_entry_is_undecided_for_every_root() {
        let tmp = tempfile::tempdir().unwrap();
        let panta = tmp.path().join("panta");
        let roots = vec![panta.clone()];
        let line = format!(
            "warning: could not check (unreadable entry)  {}/state/rhei/runs/r-2.json: \
             expected value at line 1 column 1",
            tmp.path().display()
        );
        assert_eq!(warned(&line, &roots), Warned::Every);
        let config = listing(tmp.path(), &[line.clone()]);
        let refused = listed_live(&config, &roots).expect("refused");
        assert_eq!(refused.root, panta);
        assert!(refused.says.ends_with(&line), "{}", refused.says);
    }

    /// A moved single plan's record names its lock file, every file the
    /// runtime keeps for each of its tickets in the shared root, by the
    /// runtime's own naming, and every file ephor's own states and drafts keep
    /// there for each ticket and for the plan, by ephor's — and nothing of a
    /// neighbour's, nor of a ticket whose id merely begins with one of its own
    /// (§FS-005-dispatch.36.5).
    #[test]
    fn a_single_plan_leaves_every_file_the_runtime_keeps_for_its_tickets() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("panta");
        let plan = root.join("p.rhei.md");
        write(&plan, "# Rhei: p\n\n### Task fix: x\n\n### Task fix-2: y\n");
        let ours = [
            "runtime/results/p.fix.md",
            "runtime/logs/task-p.fix-implementing.log",
            "runtime/spawns/task-p.fix-implementing.json",
            "runtime/reports/task-p.fix-implementing.md",
            "runtime/snapshot-sessions/p.fix-implementing-a-1/s.json",
            "runtime/worktree-refs/p.fix.yaml",
            "runtime/exports/p.fix/resolutions.md",
            "runtime/accounting/captures/p.fix-1.json",
            "runtime/accounting/tasks/p.fix.json",
            ".rhei/cache/snapshots/p.fix/review/fix/1/a/record.json",
            "runtime/results/p.fix-2.md",
            "runtime/ephor/p.fix.report.md",
            "runtime/ephor/p.fix.verdict.md",
            "runtime/ephor/p.fix.failures.md",
            "runtime/ephor/p.fix.reply.md",
            "runtime/ephor/p.fix-2.reply.posted.md",
            "runtime/ephor/p.reply.md",
            "runtime/ephor/p.reply.posted.md",
        ];
        let theirs = [
            "runtime/results/q.fix.md",
            "runtime/results/p.fixed.md",
            "runtime/logs/task-q.fix-implementing.log",
            "runtime/logs/task-p.fixed-implementing.log",
            ".rhei/cache/snapshots/q.fix/review/fix/1/a/record.json",
            ".rhei/cache/snapshots/p.fixed/review/fix/1/a/record.json",
            "runtime/ephor/q.fix.report.md",
            "runtime/ephor/p.fixed.report.md",
            "runtime/ephor/p.fix-20.verdict.md",
            "runtime/ephor/q.reply.md",
            "runtime/ephor/p-2.reply.md",
        ];
        for path in ours.iter().chain(theirs.iter()) {
            write(&root.join(path), "");
        }
        let mut expected: Vec<PathBuf> = [
            "runtime/results/p.fix.md",
            "runtime/logs/task-p.fix-implementing.log",
            "runtime/spawns/task-p.fix-implementing.json",
            "runtime/reports/task-p.fix-implementing.md",
            "runtime/snapshot-sessions/p.fix-implementing-a-1",
            "runtime/worktree-refs/p.fix.yaml",
            "runtime/exports/p.fix",
            "runtime/accounting/captures/p.fix-1.json",
            "runtime/accounting/tasks/p.fix.json",
            ".rhei/cache/snapshots/p.fix",
            "runtime/results/p.fix-2.md",
            "runtime/ephor/p.fix.report.md",
            "runtime/ephor/p.fix.verdict.md",
            "runtime/ephor/p.fix.failures.md",
            "runtime/ephor/p.fix.reply.md",
            "runtime/ephor/p.fix-2.reply.posted.md",
            "runtime/ephor/p.reply.md",
            "runtime/ephor/p.reply.posted.md",
        ]
        .iter()
        .map(|path| root.join(path))
        .collect();
        expected.sort();
        expected.insert(0, plan::sidecar_of(&plan));
        assert_eq!(left_behind(&plan, "p"), expected);
    }

    /// A workspace takes the runtime's files with it, so all it leaves in the
    /// root it was laid in is the workflow's answer ephor named there, as
    /// drafted and as posted — and nothing of a neighbour's
    /// (§FS-005-dispatch.36.5).
    #[test]
    fn a_workspace_leaves_only_the_answer_ephor_named_for_it_in_its_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("panta");
        let workspace = root.join("w");
        write(&workspace.join(INDEX), "# Rhei: w\n\n### Task fix: x\n");
        for path in [
            "runtime/ephor/w.workflow.reply.md",
            "runtime/ephor/w.workflow.reply.posted.md",
            "runtime/ephor/v.workflow.reply.md",
            "runtime/ephor/w-2.workflow.reply.md",
        ] {
            write(&root.join(path), "");
        }
        assert_eq!(
            left_behind(&workspace, "w"),
            vec![
                root.join("runtime/ephor/w.workflow.reply.md"),
                root.join("runtime/ephor/w.workflow.reply.posted.md"),
            ]
        );
    }

    /// The runtime's accounting file names keep a task id's safe bytes and
    /// percent-encode the rest: how `left` finds a task's accounting files
    /// (§FS-005-dispatch.36.5).
    #[test]
    fn an_accounting_name_is_the_task_id_as_the_runtime_encodes_it() {
        assert_eq!(accounting_segment("p.fix-1_a"), "p.fix-1_a");
        assert_eq!(accounting_segment("p/fix 1"), "p%2Ffix%201");
        assert_eq!(accounting_segment(""), "task");
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
