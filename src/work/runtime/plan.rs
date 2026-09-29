//! The plan file: one rhei per item, one ticket per dispatch
//! (§FS-005-dispatch.3).
//!
//! ephor writes a plain-text plan in the runtime's language and reads the
//! state back out of it. Nothing else is stored about how the work is going:
//! the runtime owns that, the plan is where it writes it, and a second copy in
//! ephor's ledger would be a watch reporting on itself
//! (§FS-005-dispatch.4).

use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::error::{EphorError, Result};
// The bound's own measures live beside the model they hold
// (§FS-005-dispatch.8), so the reader that reports a drop and the accessor that
// guarantees the bound measure with one ruler rather than two that can drift.
use crate::feed::model::{is_nameable, META_VALUE_CAP};

/// The state machine ephor installs into a work root that has none.
pub const SHIPPED_STATES: &str = include_str!("../../../assets/ephor-work.states.yaml");

/// The plan language's **abandonment state**: the one final state that
/// satisfies no `**Prior:**`, and where a ticket the reader takes back goes
/// (§FS-005-dispatch.16). The name is the runtime's — its readiness rule
/// turns on this spelling and no other — so it is spelled here and nowhere
/// above this module (§REQ-001-boundary.5); a surface asks a [`WorkRoot`]
/// whether its machine declares it and a [`PlanTicket`] whether it sits in it.
pub const CANCELLED: &str = "cancelled";

/// The runtime's **built-in default machine**: what a project that declares no
/// `states.yaml` of its own runs its plans under — `pending`, and `completed`
/// final — as the runtime itself resolves it. The names and the shape are the
/// runtime's, so they are spelled here and nowhere above this module
/// (§REQ-001-boundary.5); a caller asks [`WorkRoot::in_force`] for the machine
/// a store's tasks actually run under (§FS-006-project-interface.7).
pub const DEFAULT_STATES: &str = "name: rhei\nstates:\n  pending:\n  completed:\n    final: true\n";

/// What surrounds the dossier, so a later sync can rewrite exactly that much
/// and leave every `**State:**` line the runtime owns untouched.
const DOSSIER_OPEN: &str = "<!-- ephor:dossier -->";
const DOSSIER_CLOSE: &str = "<!-- /ephor:dossier -->";

const TASKS_HEADING: &str = "## Tasks";

/// The fence around a plan's frontmatter, which [`Plan::set_metadata`] writes
/// between the title and the dossier block. The runtime's own grammar, so it is
/// spelled here and nowhere above this module (§REQ-001-boundary.5).
const FRONTMATTER_FENCE: &str = "---";

/// The directory a runtime project lives in, under the work root template
/// (§DA-001-runtime-bound-default). Part of the coupling, and so part of this
/// module (§REQ-001-boundary.5).
pub const PROJECT_DIR: &str = "panta";

/// The file whose presence makes a directory a runtime project — the runner's
/// own manifest, and so the one thing that answers "is there already a project
/// here" without asking the runner and reading its prose. Part of the
/// coupling, and so part of this module (§REQ-001-boundary.5).
pub const MANIFEST: &str = "index.panta.md";

/// The state machine file a runtime project may provide.
pub const STATES: &str = "states.yaml";

/// The ignore file ephor writes around its own work root.
pub const IGNORE: &str = ".gitignore";

/// Ephor's own hidden corner inside a work root: `.ephor/<plan id>/`, holding
/// the dossier, item and values ephor carries for a plan it asked the runtime
/// to render. A dotted name is not a plan, so enumerating the root steps over
/// it (§FS-005-dispatch.15) — and its presence is what names such a plan as one
/// ephor caused to exist (§FS-006-project-interface.7). One home for the name,
/// here with the rest of the work root's grammar, so the writer and the reader
/// cannot drift apart.
pub const CARRIED: &str = ".ephor";

/// What a plan file is called: `<plan id>` and this. The runtime's own word,
/// so it is spelled here and referred to by this name everywhere else
/// (§REQ-001-boundary.5) — including by a test that has to name the file a
/// carry-over moved (§FS-005-dispatch.3.1).
pub(crate) const PLAN_SUFFIX: &str = ".rhei.md";

/// The older flat-plan spelling accepted by the task-store reader. It lives
/// here with the rest of the binding's grammar rather than in the caller
/// (§AR-007-runtime.1).
const COMPAT_PLAN_SUFFIX: &str = ".panta.md";

/// The one plan file of a plan rendered as a directory: the index that names
/// it. Part of the coupling, and so part of this module
/// (§REQ-001-boundary.5).
const INDEX: &str = "index.rhei.md";

/// Where such a plan keeps its tasks: one file per task beside the index,
/// each written in the same header grammar a task inside a plan file is
/// (§FS-005-dispatch.28).
const TASKS_DIR: &str = "tasks";

/// The rule that makes a work root invisible to the repository it sits in
/// (§FS-006-project-interface.7). Ephor's promise, kept whatever else is in
/// that file.
const SELF_IGNORE: &str = "# ephor work — planning state, not repository content\n*\n";

/// Whether `dir` already holds a runtime project (§FS-006-project-interface.7).
/// Asked before the runner's own `init` is, so that a directory that has one is
/// left alone rather than being offered to the runner for it to refuse.
pub fn is_project(dir: &Path) -> bool {
    dir.join(MANIFEST).is_file()
}

/// A directory holding an item's plans: a rhei project, with the state machine
/// its tickets run under.
pub struct WorkRoot {
    pub dir: PathBuf,
    /// The machine's name, as its `states.yaml` declares it.
    pub machine: String,
    /// The states that machine declares, for refusing a recipe that names one
    /// it does not have (§FS-005-dispatch.6), and for telling a ticket that is
    /// still being worked from one that is over.
    states: Vec<StateInfo>,
}

pub struct StateInfo {
    pub name: String,
    pub is_final: bool,
    /// The runtime will not leave this state on its own: it is where work
    /// waits for a person (§FS-005-dispatch.9).
    pub is_gating: bool,
    /// The state declares `inputs:` — files an earlier state was supposed to
    /// write. A fresh ticket has no earlier state, so this is where one may
    /// not start (§FS-005-dispatch.6).
    pub needs_input: bool,
    /// The state's `poll:` declares `waiting_on:` — what this wait is for is
    /// a person's answer, not a machine's (§FS-005-dispatch.24). The other
    /// half of [`StateInfo::is_gating`]: both are work stopped until somebody
    /// answers, and they differ only in who resumes it.
    pub waits_on_person: bool,
}

impl WorkRoot {
    /// Prepare `dir` to hold tickets: create the project manifest when it is
    /// not there, install `states_yaml` when the directory has no machine of
    /// its own, and read back whichever machine is in force. An existing
    /// machine is never replaced — a reader who edited it meant it.
    ///
    /// A project that already holds plans of its own and declares no machine
    /// is refused rather than filled in (§FS-005-dispatch.6): `states.yaml` is
    /// how every plan in a project resolves its states, so dropping one in
    /// changes what those plans run under. A project with no plans in it —
    /// which is exactly what the runtime's own `init` leaves behind — has
    /// nothing to disturb, and ephor moves in beside it.
    pub fn ensure(dir: &Path, states_yaml: &str) -> Result<WorkRoot> {
        create_dir(dir)?;
        let manifest = dir.join(MANIFEST);
        let states = dir.join("states.yaml");
        let ours = !manifest.exists();
        if ours {
            let title = dir
                .parent()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "work".to_string());
            write(&manifest, &format!("# Panta: {title}\n"))?;
        }
        // A directory that ignores itself needs no entry in the repository's
        // own .gitignore: work about a branch would otherwise show up as a
        // change to that branch. The runner's own `init` writes rules of its
        // own here (§FS-006-project-interface.7) and none of them is this one —
        // it never promised the checkout would stay clean and ephor did — so
        // the line is added to what is there rather than only written where
        // nothing is.
        let ignore = dir.join(".gitignore");
        let held = fs::read_to_string(&ignore).unwrap_or_default();
        if !held.lines().any(|line| line.trim() == "*") {
            let lead = match held.is_empty() || held.ends_with('\n') {
                true => held,
                false => format!("{held}\n"),
            };
            write(&ignore, &format!("{lead}{SELF_IGNORE}"))?;
        }
        if !states.exists() {
            if !ours && holds_plans(dir) {
                return Err(EphorError::Command(format!(
                    "{} already holds plans of its own and declares no state machine — \
                     writing one there would change what they run under. Either install \
                     ephor's (`ephor work states > {}`) or give ephor a directory of its own \
                     with `work.root`.",
                    dir.display(),
                    states.display()
                )));
            }
            write(&states, states_yaml)?;
        }
        Self::read_root(dir, &states)
    }

    /// The machine in force in a work root, without creating anything. None
    /// when the directory holds no machine — a plan can still be read there,
    /// it is only finality that cannot be judged.
    pub fn open(dir: &Path) -> Result<Option<WorkRoot>> {
        let states = dir.join("states.yaml");
        if !states.is_file() {
            return Ok(None);
        }
        Self::read_root(dir, &states).map(Some)
    }

    /// The machine a directory's plans actually run under: the one it declares,
    /// or — where it declares none — the runtime's built-in default
    /// ([`DEFAULT_STATES`]), which is what the runtime resolves such a project
    /// to. This is the question a reader of somebody else's store asks, since a
    /// task's state means whatever the machine in force says it means
    /// (§FS-006-project-interface.7). [`open`](Self::open) keeps its own
    /// meaning: a surface that must withhold judgment where nothing is declared
    /// asks that one instead.
    pub fn in_force(dir: &Path) -> Result<WorkRoot> {
        match Self::open(dir)? {
            Some(root) => Ok(root),
            None => Self::from_states(dir, DEFAULT_STATES, "the runtime's default state machine"),
        }
    }

    /// The machine a work root that is not there yet would run under: the
    /// `states_yaml` [`ensure`](Self::ensure) would install into it. Nothing
    /// is read and nothing is written, which is the point — a caller that has
    /// to refuse before it makes the directory cannot ask
    /// [`open`](Self::open), because there is nothing there to open yet
    /// (§FS-005-dispatch.25).
    pub fn proposed(dir: &Path, states_yaml: &str) -> Result<WorkRoot> {
        Self::from_states(dir, states_yaml, "the state machine ephor would install")
    }

    fn read_root(dir: &Path, states: &Path) -> Result<WorkRoot> {
        let text = read(states)?;
        Self::from_states(dir, &text, &states.display().to_string())
    }

    /// A root from a states document, whatever it was read from — `origin`
    /// names that source in the one error this can raise.
    fn from_states(dir: &Path, text: &str, origin: &str) -> Result<WorkRoot> {
        Ok(WorkRoot {
            dir: dir.to_path_buf(),
            machine: machine_name(text).ok_or_else(|| {
                EphorError::Command(format!(
                    "{origin} declares no state machine name; ephor cannot write tickets that \
                     name one."
                ))
            })?,
            states: state_infos(text),
        })
    }

    pub fn plan_path(&self, plan_id: &str) -> PathBuf {
        plan_path_in(&self.dir, plan_id)
    }

    /// Whether the machine in force declares a state, so a recipe pointing at
    /// one it does not have is refused rather than dispatched.
    pub fn declares(&self, state: &str) -> bool {
        self.states.iter().any(|info| info.name == state)
    }

    /// Whether a state is one the work does not leave.
    pub fn is_final(&self, state: &str) -> bool {
        self.flag(state, |info| info.is_final)
    }

    /// The abandonment state this machine declares, where it declares one
    /// (§FS-005-dispatch.16): [`CANCELLED`], and final — a state under that
    /// name the work would leave again is not one a ticket can be taken back
    /// into. None is a refusal's cue, never a state to write.
    pub fn cancel_state(&self) -> Option<&str> {
        self.states
            .iter()
            .find(|info| info.name == CANCELLED && info.is_final)
            .map(|info| info.name.as_str())
    }

    /// Whether a state is one the runtime will not leave on its own — work
    /// parked there is waiting for a person (§FS-005-dispatch.9).
    pub fn is_gating(&self, state: &str) -> bool {
        self.flag(state, |info| info.is_gating)
    }

    /// Whether a state's poll says that what it waits for is a person
    /// (§FS-005-dispatch.24). A gating state must be moved by hand; this one
    /// moves itself the moment the answer lands — which is why the machine
    /// has to say so, and why a reading that only knew `gating` would call
    /// six hours of waiting on an author live work.
    pub fn waits_on_person(&self, state: &str) -> bool {
        self.flag(state, |info| info.waits_on_person)
    }

    fn flag(&self, state: &str, of: impl Fn(&StateInfo) -> bool) -> bool {
        self.states
            .iter()
            .find(|info| info.name == state)
            .map(of)
            .unwrap_or(false)
    }

    pub fn state_names(&self) -> Vec<String> {
        self.states.iter().map(|info| info.name.clone()).collect()
    }

    /// Whether a state expects files an earlier state writes, which is what
    /// makes it somewhere a fresh ticket cannot start
    /// (§FS-005-dispatch.6).
    pub fn needs_input(&self, state: &str) -> bool {
        self.flag(state, |info| info.needs_input)
    }

    /// The states a ticket could start in: declared, not already over, and
    /// waiting on nothing an earlier state was to write. What a refusal offers
    /// instead of the state it refused (§FS-005-dispatch.6).
    pub fn openable_states(&self) -> Vec<String> {
        self.states
            .iter()
            .filter(|info| !info.is_final && !info.needs_input)
            .map(|info| info.name.clone())
            .collect()
    }
}

/// The plan file for an id inside a work root. The same path
/// [`WorkRoot::plan_path`] resolves, for callers that have the directory
/// before they have the root — a dry run promising where work would go
/// (§FS-005-dispatch.5).
pub fn plan_path_in(dir: &Path, plan_id: &str) -> PathBuf {
    dir.join(format!("{plan_id}{PLAN_SUFFIX}"))
}

/// Whether a runtime project has any plans in it: a `*.rhei.md` file, or a
/// directory workspace, among its direct non-hidden children — which is where
/// the runtime looks for them.
fn holds_plans(dir: &Path) -> bool {
    !plans_in(dir).is_empty()
}

/// One plan found in a work root: its id — the name the runtime knows it
/// by — and the file the floor reads (§AR-007-runtime.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundPlan {
    pub plan_id: String,
    pub path: PathBuf,
}

/// Discover the plans in a runtime work root. A probing caller deliberately
/// receives an empty answer when the directory cannot be read; the task-store
/// reader uses [`task_store_plans_in`] when a recognized source must instead
/// report that it did not answer (§FS-006-project-interface.7).
fn discover_plans(dir: &Path, task_store: bool) -> Result<Vec<FoundPlan>> {
    let entries = fs::read_dir(dir).map_err(|err| {
        EphorError::Command(format!(
            "Cannot read plan directory {}: {err}",
            dir.display()
        ))
    })?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            // General runtime discovery is a probe and has always skipped an
            // unreadable entry. A recognized task store is a source already,
            // so partial discovery would falsely answer that work is absent
            // (§FS-006-project-interface.7).
            Err(err) if task_store => {
                return Err(EphorError::Command(format!(
                    "Cannot read an entry in plan directory {}: {err}",
                    dir.display()
                )))
            }
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name == "runtime" {
            continue;
        }
        let primary = name.strip_suffix(PLAN_SUFFIX);
        let compatible = task_store
            .then(|| name.strip_suffix(COMPAT_PLAN_SUFFIX))
            .flatten();
        if let Some(plan_id) = primary.or(compatible) {
            // The feed historically used everything before the first dot as
            // the identity of a flat plan. Keep that identity for task-store
            // callers while the runtime's ordinary grammar retains the whole
            // stem (§FS-006-project-interface.7).
            let plan_id = if task_store {
                name.split('.').next().unwrap_or(plan_id)
            } else {
                plan_id
            };
            found.push(FoundPlan {
                plan_id: plan_id.to_string(),
                path: entry.path(),
            });
            continue;
        }
        let index = entry.path().join(INDEX);
        let is_index = if task_store {
            let workspace = match fs::metadata(entry.path()) {
                Ok(metadata) => metadata,
                // A dangling entry is not a directory workspace. Other
                // inspection failures keep the recognized source honest.
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
                Err(err) => {
                    return Err(EphorError::Command(format!(
                        "Cannot inspect task-store entry {}: {err}",
                        entry.path().display()
                    )))
                }
            };
            if !workspace.is_dir() {
                continue;
            }
            match fs::metadata(&index) {
                Ok(metadata) => metadata.is_file(),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
                Err(err) => {
                    return Err(EphorError::Command(format!(
                        "Cannot inspect directory-workspace plan {}: {err}",
                        index.display()
                    )))
                }
            }
        } else {
            index.is_file()
        };
        if is_index {
            found.push(FoundPlan {
                // A directory workspace's directory is its stable plan id
                // (§FS-006-project-interface.7, §AR-007-runtime.1).
                plan_id: name,
                path: index,
            });
        }
    }
    Ok(found)
}

/// The name ephor's hidden corner for a plan is keyed by: the plan's own name
/// as the writer knew it — the directory of a rendered workspace, the file stem
/// of a flat plan. Deliberately not [`FoundPlan::plan_id`], which the
/// task-store reader truncates at the first dot for the feed's sake: the corner
/// was written under the whole name, so a flat plan laid as `a.b.rhei.md` must
/// be looked up as `a.b` rather than as `a` (§FS-006-project-interface.7). For
/// a rendered directory workspace the two agree.
fn carried_key(root: &Path, path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    if path.parent() == Some(root) {
        let stem = name
            .strip_suffix(PLAN_SUFFIX)
            .or_else(|| name.strip_suffix(COMPAT_PLAN_SUFFIX))?;
        return Some(stem.to_string());
    }
    Some(path.parent()?.file_name()?.to_string_lossy().into_owned())
}

/// Whether the plan file carries ephor's dossier **block** — the mark every
/// plan ephor **authored** bears, written by [`Plan::create`] and by no other
/// writer, so the test never has to enumerate the verbs that author one
/// (§FS-006-project-interface.7). The mark is the block and not one of its
/// markers: both the opening and the closing marker, in that order, because a
/// plan that merely quotes the opening marker in its prose is a plan somebody
/// wrote about ephor rather than one ephor wrote (§FS-006-project-interface.7).
///
/// Only the **head** of the file is read, and the head is what ephor writes
/// above the dossier and nothing else: the title, the declarations beside it,
/// and the frontmatter [`Plan::set_metadata`] puts between them and the block.
/// The first line that is none of those ends the head, so a plan of the
/// project's own costs a handful of lines however long it is and whether or not
/// it has a tasks heading at all — a directory workspace's index has none. Only
/// once the head has borne the opening marker does the reading go on, and then
/// to the close and nothing else: the dossier is ephor's verbatim copy of the
/// item, so a heading that would end the head may perfectly well be inside it.
///
/// A candidate that is not a readable file is not a plan bearing a mark: the
/// reader skips it exactly as [`Plan::read`] does, so a dangling entry named
/// like a plan, or one that vanishes between the listing and this read, no
/// longer takes its whole store down with it. A file that is there and cannot
/// be read is still a source that did not answer
/// (§FS-006-project-interface.7).
///
/// The skip is asked twice, before the open and after a failure, because the
/// two see different things and neither covers the other: asking first is the
/// only thing that stops a candidate whose `open(2)` never returns — a named
/// pipe with no writer — from hanging the sweep, since nothing has failed for
/// the classification to read; and asking after is the only thing that sees a
/// plan taken between the two, or a directory, which answers the open on Linux
/// and fails the line read instead (§FS-006-project-interface.7).
fn carries_dossier_block(path: &Path) -> Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    match head_carries_dossier_block(path) {
        Ok(found) => Ok(found),
        // Ask the filesystem only once a read has failed, and ask it about the
        // path as it is now: what failed is either no plan file at all — a
        // directory answers the open and not the read — or one that has gone.
        Err(_) if !path.is_file() => Ok(false),
        Err(err) => Err(EphorError::Command(format!(
            "Cannot read plan {}: {err}",
            path.display()
        ))),
    }
}

/// The scan itself, reporting the filesystem's own failure so that
/// [`carries_dossier_block`] is the one place that decides what a failure means.
fn head_carries_dossier_block(path: &Path) -> std::io::Result<bool> {
    let file = fs::File::open(path)?;
    // Where the scan is: in the head above the block, inside the plan's
    // frontmatter, or past the opening marker and looking for its close.
    let mut in_frontmatter = false;
    let mut opened = false;
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        let line = line.trim();
        // Past the opening marker only its close settles the block, and
        // nothing inside the dossier may end the scan: the dossier is ephor's
        // verbatim copy of the item, so a tasks heading may well be in it.
        if opened {
            if line == DOSSIER_CLOSE {
                return Ok(true);
            }
            continue;
        }
        if in_frontmatter {
            in_frontmatter = line != FRONTMATTER_FENCE;
            continue;
        }
        if line == DOSSIER_OPEN {
            opened = true;
        } else if line == FRONTMATTER_FENCE {
            in_frontmatter = true;
        } else if !(line.is_empty() || line.starts_with("# ") || line.starts_with("**")) {
            return Ok(false);
        }
    }
    Ok(false)
}

/// Whether ephor caused this plan to exist, read from what ephor wrote on disk
/// beside it and never from the ledger (§FS-006-project-interface.7,
/// §FS-005-dispatch.4). Two marks, because ephor stands in two relations to the
/// two plan shapes: the dossier block in a plan it authored, and its own hidden
/// corner beside a plan the runtime rendered for it, which is never ephor's to
/// write (§REQ-001-boundary.1). A plan the project wrote that a dispatch merely
/// appended a ticket to bears neither — appending does not rewrite a plan that
/// has no block — and so stays the project's own (§FS-006-project-interface.7).
fn ephor_caused(root: &Path, path: &Path) -> Result<bool> {
    if let Some(key) = carried_key(root, path) {
        if root.join(CARRIED).join(key).exists() {
            return Ok(true);
        }
    }
    carries_dossier_block(path)
}

/// Every plan a work root holds, whoever wrote it (§FS-005-dispatch.15): the
/// `*.rhei.md` files and the directory workspaces among its direct non-hidden
/// children, exactly where the runtime looks for them. One directory listing,
/// no file read, no runner asked — recognizing a plan is this module's
/// grammar (§AR-007-runtime.1), and the callers get ids and paths, never the
/// suffix. Ephor's own plans are among them: the work screen and the operations
/// board watch every plan a work root holds, and only the feed-facing
/// [`task_store_plans_in`] declines the ones ephor caused to exist.
pub fn plans_in(dir: &Path) -> Vec<FoundPlan> {
    let mut found = discover_plans(dir, false).unwrap_or_default();
    found.sort_by(|a, b| a.plan_id.cmp(&b.plan_id));
    found
}

/// Every plan a recognized Rhei task store contributes to the feed: the
/// runtime's flat and direct directory-workspace shapes, plus the established
/// flat compatibility spelling. Unlike probing discovery, failure to list a
/// recognized store remains a source failure (§FS-006-project-interface.7,
/// §AR-007-runtime.1).
///
/// **A plan ephor caused to exist is not among them**
/// (§FS-006-project-interface.7): the store holds the project's work, and a
/// plan ephor put there to do some of that work is ephor's own filing rather
/// than a second piece of it. Without the test a recipe selecting this source
/// and minting a checkout per task is offered its own plans back on the next
/// read and mints again (§FS-005-dispatch.25). Unlike [`plans_in`] this reads
/// the head of each candidate, which is what the mark costs; the seam reads the
/// whole plan a moment later anyway.
pub fn task_store_plans_in(dir: &Path) -> Result<Vec<FoundPlan>> {
    let mut found = Vec::new();
    for plan in discover_plans(dir, true)? {
        if !ephor_caused(dir, &plan.path)? {
            found.push(plan);
        }
    }
    // This is the flat reader's existing order, retained while directory
    // workspaces join it.
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

/// Write a plan such as a **project** writes for itself: a title, one open task
/// under the runtime's built-in default machine, and no dossier block — so the
/// task-store reader takes it for what it is rather than declining it as one
/// ephor caused to exist (§FS-006-project-interface.7). Both the file's name
/// and its grammar are the runtime's, so they are spelled here and nowhere
/// above this module (§REQ-001-boundary.5). The self-pass uses it to put
/// something in a store that is the project's own work.
///
/// **It is deliberately the one writer of a plan file in ephor that leaves no
/// mark, and it may never be used on a real project's store.** The rule tests
/// the mark rather than enumerating the verbs that author a plan
/// (§FS-006-project-interface.7), which holds because every verb that authors
/// one authors it through [`Plan::create`]; a plan this wrote would read as the
/// project's own however it got there, so it stays inside this crate and is for
/// standing in for a project in the self-pass's own throwaway site
/// (§FS-010-doctor.4).
pub(crate) fn write_project_plan(dir: &Path, plan_id: &str, task: &str) -> Result<PathBuf> {
    let path = dir.join(format!("{plan_id}{PLAN_SUFFIX}"));
    write(
        &path,
        &format!(
            "# Rhei: {}\n\n{TASKS_HEADING}\n\n### Task 1: {}\n**State:** pending\n",
            one_line(plan_id),
            one_line(task),
        ),
    )?;
    Ok(path)
}

/// The `name:` of a states document — the shallowest one, so a state called
/// `name` cannot be mistaken for it.
fn machine_name(yaml: &str) -> Option<String> {
    yaml.lines()
        .find_map(|line| line.strip_prefix("name:"))
        .map(|value| {
            value
                .trim()
                .trim_matches(['"', '\''].as_slice())
                .to_string()
        })
        .filter(|name| !name.is_empty())
}

/// The keys under `states:`, and which of them the work does not leave. A
/// line-scan rather than a YAML parse: this only has to be right enough to
/// refuse a recipe naming a state that is not there and to tell a ticket in
/// flight from a finished one, and the runtime's own validation is the
/// authority on everything else.
fn state_infos(yaml: &str) -> Vec<StateInfo> {
    let mut states: Vec<StateInfo> = Vec::new();
    let mut inside = false;
    // Whether the lines being read hang under the current state's `poll:`.
    // Only they may say who the poll waits on (§FS-005-dispatch.24).
    let mut in_poll = false;
    for line in yaml.lines() {
        if line.starts_with("states:") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if !line.starts_with(' ') && !line.trim().is_empty() {
            break;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        // Anything at the state's own key level or above has closed the poll
        // block, whatever comes next.
        if indent <= 4 {
            in_poll = false;
        }
        if indent == 2 && trimmed.ends_with(':') {
            states.push(StateInfo {
                name: trimmed.trim_end_matches(':').to_string(),
                is_final: false,
                is_gating: false,
                needs_input: false,
                waits_on_person: false,
            });
        } else if indent > 2 {
            let flag = |prefix: &str| {
                trimmed
                    .strip_prefix(prefix)
                    .map(|value| value.trim().eq_ignore_ascii_case("true"))
            };
            // `poll:` written as a block opens on its own line and is read on
            // the ones beneath it; written as a flow mapping it says
            // everything on this one.
            if indent == 4 {
                if let Some(rest) = trimmed.strip_prefix("poll:") {
                    in_poll = rest.trim().is_empty();
                }
            }
            let person_wait = waits_on_someone(trimmed)
                && match indent {
                    4 => trimmed.starts_with("poll:"),
                    _ => in_poll,
                };
            if let Some(last) = states.last_mut() {
                if let Some(value) = flag("final:") {
                    last.is_final = value;
                }
                if let Some(value) = flag("gating:") {
                    last.is_gating = value;
                }
                // The key alone, at the state's own level: what hangs under it
                // is the runtime's to read, and this only has to know that
                // something does (§FS-005-dispatch.6).
                if indent == 4 && trimmed == "inputs:" {
                    last.needs_input = true;
                }
                if person_wait {
                    last.waits_on_person = true;
                }
            }
        }
    }
    states
}

/// Whether a line declaring `waiting_on:` names somebody: the label after
/// the key, cut where a flow mapping ends the value and unquoted
/// (§FS-005-dispatch.24). A blank label is not a declaration — the runtime
/// refuses one, and reading it as a person wait would hand out capacity on a
/// machine that will not run at all.
fn waits_on_someone(line: &str) -> bool {
    let Some((_, after)) = line.split_once("waiting_on:") else {
        return false;
    };
    !after
        .split([',', '}'])
        .next()
        .unwrap_or(after)
        .trim()
        .trim_matches(['"', '\''].as_slice())
        .trim()
        .is_empty()
}

/// One ticket, as it is written into a plan.
pub struct Ticket {
    pub id: String,
    pub title: String,
    pub state: String,
    /// The ticket this one follows, so a reopened item's work stays ordered
    /// (§FS-005-dispatch.5).
    pub prior: Option<String>,
    pub target: Option<String>,
    pub model: Option<String>,
    pub body: String,
}

impl Ticket {
    fn render(&self) -> String {
        let mut out = format!("### Task {}: {}\n", self.id, one_line(&self.title));
        out.push_str(&format!("**State:** {}\n", self.state));
        if let Some(prior) = &self.prior {
            out.push_str(&format!("**Prior:** Task {prior}\n"));
        }
        // Mutually exclusive in the runtime's language: a target carries a
        // model already, and declaring both is a validation error there.
        match (&self.target, &self.model) {
            (Some(target), _) => out.push_str(&format!("**Target:** {target}\n")),
            (None, Some(model)) => out.push_str(&format!("**Model:** {model}\n")),
            (None, None) => {}
        }
        out.push('\n');
        out.push_str(self.body.trim_end());
        out.push('\n');
        out
    }
}

/// The execution line a ticket carries, where it carries one
/// (§FS-005-dispatch.14). The two rank differently against a run's flags: a
/// full line is resolved on its own, with the flags invisible to it, while a
/// model line takes its carrier from them — so only the latter keeps flags
/// off a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
    /// `**Target:**` — the full execution identity, the ticket's alone.
    Target,
    /// `**Model:**` — a model with no carrier of its own.
    Model,
}

/// A ticket as the plan currently has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanTicket {
    pub id: String,
    pub title: String,
    pub state: Option<String>,
    /// Who claimed the ticket, where anyone has. A claim makes the runtime
    /// skip the ticket — it is never a liveness signal (§FS-005-dispatch.15).
    pub assignee: Option<String>,
    /// The execution line the ticket carries, where it carries one
    /// (§FS-005-dispatch.14).
    pub pinned: Option<Pin>,
    /// The tickets this one is ordered after — its `**Prior:**` list, ids
    /// only, the kind word dropped as the runtime's own readers drop it. What
    /// a cancel names as left waiting (§FS-005-dispatch.16).
    pub prior: Vec<String>,
}

impl PlanTicket {
    /// Whether the ticket sits in the abandonment state (§FS-005-dispatch.16).
    pub fn cancelled(&self) -> bool {
        self.state.as_deref() == Some(CANCELLED)
    }
}

pub struct Plan {
    pub path: PathBuf,
    text: String,
    /// A plan rendered as a directory keeps its tasks in files beside its
    /// index, and those are as much this plan's tasks as one written into it
    /// (§FS-005-dispatch.28). Read when the plan is, so
    /// [`tickets`](Plan::tickets) answers for the whole plan whichever shape
    /// the runtime gave it. Empty for a plan written as one file — and for
    /// every plan ephor writes itself, which is always one.
    parts: Vec<PlanPart>,
}

/// A task file retains its origin for file-backed activity (§FS-006-project-interface.7).
struct PlanPart {
    path: PathBuf,
    text: String,
}

/// What a store said about one of its own tasks, once the bound has been
/// applied (§FS-005-dispatch.8): the keys that survived, and one line per key
/// that did not, so the drop can be said out loud where the store's answer is
/// reported rather than going silently.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskMeta {
    /// The carried keys, in the spelling a matter publishes them: the scalar
    /// as the store wrote it, so a number is still a number.
    pub values: serde_json::Map<String, serde_json::Value>,
    /// Why each dropped key was dropped, ready to be qualified by the matter
    /// it was about.
    pub dropped: Vec<String>,
}

/// One entry of the store's task map, under a key it may have written as a
/// bare number as readily as a string.
fn entry<'a>(tasks: &'a serde_yaml::Value, key: &str) -> Option<&'a serde_yaml::Value> {
    tasks
        .as_mapping()?
        .iter()
        .find(|(written, _)| scalar_key(written).as_deref() == Some(key))
        .map(|(_, value)| value)
}

/// A mapping key as the name it stands for, or None where the store wrote
/// something that is no name — a list, a nested map.
fn scalar_key(key: &serde_yaml::Value) -> Option<String> {
    match key {
        serde_yaml::Value::String(text) => Some(text.clone()),
        serde_yaml::Value::Number(number) => Some(number.to_string()),
        serde_yaml::Value::Bool(yes) => Some(yes.to_string()),
        _ => None,
    }
}

/// What the store wrote as the JSON a matter carries: a scalar in the spelling
/// the store used, so a number is still a number. `None` where the store wrote
/// something that is no scalar at all — a list, a nested map, or a number JSON
/// has no room for, which `serde_json::Number::from_f64` refuses.
fn converted(value: &serde_yaml::Value) -> Option<serde_json::Value> {
    match value {
        serde_yaml::Value::String(text) => Some(serde_json::Value::String(text.clone())),
        serde_yaml::Value::Bool(yes) => Some(serde_json::Value::Bool(*yes)),
        serde_yaml::Value::Number(number) => {
            number.as_i64().map(serde_json::Value::from).or_else(|| {
                number
                    .as_f64()
                    .and_then(serde_json::Number::from_f64)
                    .map(serde_json::Value::Number)
            })
        }
        _ => None,
    }
}

/// One key of a store's block against §FS-005-dispatch.8's bound — a scalar, a
/// key a shell will take, and an identifier rather than prose. `Err` carries
/// which part of the bound broke, in the words the drop is reported in.
///
/// The verdict is `bounded_entry`'s and is not restated here: the bound sits on
/// the accessor every surface reads the map through, so the reader carries
/// exactly what `Item::meta` will answer with and the two cannot come apart
/// (§FS-005-dispatch.8). A second copy of the clauses that merely agreed today
/// is how a key carried here gets dropped there with nobody told, which is the
/// silent stop-matching that point forbids. What the clauses are still for is
/// the reason: a store's author reads *which* part of the bound they broke.
fn bounded(key: &str, value: &serde_yaml::Value) -> std::result::Result<serde_json::Value, String> {
    let carried = converted(value);
    match carried
        .as_ref()
        .filter(|carried| crate::feed::model::bounded_entry(key, carried).is_some())
    {
        Some(carried) => Ok(carried.clone()),
        None => Err(broke(key, carried.as_ref())),
    }
}

/// Which clause of §FS-005-dispatch.8's bound an entry the accessor refused
/// broke, in the words the drop is reported in.
///
/// Called only about an entry `bounded_entry` has already refused, so the last
/// line is a refusal no clause here accounts for: a reason rather than a panic,
/// because a drop reported vaguely is still reported, and the guarantee this
/// arrangement buys is that the accessor's verdict is the one that stands.
fn broke(key: &str, carried: Option<&serde_json::Value>) -> String {
    if !is_nameable(key) {
        return "the key is not one a shell will take as a variable name".to_string();
    }
    let Some(rendered) = carried.and_then(crate::feed::model::spelled) else {
        return "the value is not a scalar".to_string();
    };
    // *Identifiers only*: a value with a line break in it is the prose the
    // dossier is for, and it is neither a path segment, a variable a script
    // can read a line at a time, nor anything a selector compares
    // (§FS-005-dispatch.8).
    if rendered.contains('\n') {
        return "the value is prose rather than an identifier".to_string();
    }
    if rendered.len() > META_VALUE_CAP {
        return format!(
            "the value is {} bytes, and a carried value is at most {META_VALUE_CAP}",
            rendered.len()
        );
    }
    "the value is not one the bound carries".to_string()
}

/// The keys that carried but set no variable, because another carried key folds
/// to the same one (§FS-006-project-interface.3): one line per variable name
/// two or more keys claim, naming both the keys and the variable neither of
/// them sets.
///
/// Decidable here rather than at the summons, and reported here rather than
/// there, because the fold is a property of the carried key set alone and this
/// is where the drop list already is. `meta_variable` is called rather than
/// restated, so the key-to-variable rule keeps one spelling.
///
/// The set it is asked about is what `bounded` carried, which is what the
/// accessor will answer with — so a key the bound dropped can never be counted
/// as claiming a variable its surviving neighbour then goes on to set.
fn folded(values: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut by_variable: std::collections::BTreeMap<String, Vec<&str>> =
        std::collections::BTreeMap::new();
    for key in values.keys() {
        by_variable
            .entry(crate::seams::dossier::meta_variable(key))
            .or_default()
            .push(key.as_str());
    }
    by_variable
        .into_iter()
        .filter(|(_, keys)| keys.len() > 1)
        .map(|(variable, keys)| {
            format!(
                "'{}' fold to one name, so {variable} is set by neither",
                keys.join("' and '")
            )
        })
        .collect()
}

impl Plan {
    pub fn read(path: &Path) -> Result<Option<Plan>> {
        if !path.is_file() {
            return Ok(None);
        }
        Ok(Some(Plan {
            path: path.to_path_buf(),
            text: read(path)?,
            parts: task_files(path)?,
        }))
    }

    /// A fresh plan for one item: its title, the machine its tickets run
    /// under, the dossier, and the first ticket.
    pub fn create(path: &Path, machine: &str, title: &str, dossier: &str, ticket: &Ticket) -> Plan {
        let text = format!(
            "# Rhei: {}\n**States:** {machine}\n\n{DOSSIER_OPEN}\n{}\n{DOSSIER_CLOSE}\n\n\
             {TASKS_HEADING}\n\n{}",
            one_line(title),
            dossier.trim_end(),
            ticket.render()
        );
        Plan {
            path: path.to_path_buf(),
            text,
            parts: Vec::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// The plan's own name: its leading heading, with the plan language's
    /// label stripped — what a board row can say about work ephor never
    /// dispatched, which has no matter to borrow a title from
    /// (§FS-005-dispatch.15). None where the file opens with no heading.
    pub fn title(&self) -> Option<String> {
        let heading = self
            .text
            .lines()
            .find_map(|line| line.trim().strip_prefix("# "))?;
        let title = heading.strip_prefix("Rhei:").unwrap_or(heading).trim();
        (!title.is_empty()).then(|| title.to_string())
    }

    /// Every ticket in the plan, in the order it was written — at every
    /// depth: the runtime nests a subtask one heading deeper per level, up to
    /// `######`, and a parked subtask is as much a ticket as its parent
    /// (§FS-005-dispatch.15). Fenced blocks
    /// are skipped: a dossier quotes conversations, and a conversation about a
    /// plan contains headings that are not this plan's. Metadata is read only
    /// from a ticket's header block — the `**Field:**` lines between its
    /// heading and its first content line, blank lines not closing it — which
    /// is exactly as far as the runtime reads them, so a dossier or a report
    /// quoted into a body cannot pin a ticket it merely mentions.
    pub fn tickets(&self) -> Vec<PlanTicket> {
        self.tickets_with_paths()
            .map(|(ticket, _)| ticket)
            .collect()
    }

    /// Tickets with the file containing each one, so task activity follows
    /// that file while plan provenance stays at the index (§FS-006-project-interface.7).
    /// Ticket ordering and parsing are the same as [`tickets`](Self::tickets).
    pub fn tickets_with_paths(&self) -> impl Iterator<Item = (PlanTicket, &Path)> {
        std::iter::once((self.path.as_path(), self.text.as_str()))
            .chain(
                self.parts
                    .iter()
                    .map(|part| (part.path.as_path(), part.text.as_str())),
            )
            .flat_map(|(path, text)| {
                tickets_in(text)
                    .into_iter()
                    .map(move |ticket| (ticket, path))
            })
    }

    /// The next id for a recipe's tickets on this plan: `answer-1`, then
    /// `answer-2`. Ids are per recipe so the file reads as what was asked for.
    pub fn next_ticket_id(&self, recipe: &str) -> String {
        let highest = self
            .tickets()
            .iter()
            .filter_map(|ticket| {
                ticket
                    .id
                    .strip_prefix(recipe)?
                    .strip_prefix('-')?
                    .parse::<u32>()
                    .ok()
            })
            .max()
            .unwrap_or(0);
        format!("{recipe}-{}", highest + 1)
    }

    /// The last top-level ticket in the plan that was not cancelled, which a
    /// new one follows: a new dispatch orders itself after the previous
    /// dispatch, never after a subtask the runtime nested under one — and
    /// never after an abandoned ticket, since the abandonment state satisfies
    /// no `**Prior:**` and a chain hung off one would never start
    /// (§FS-005-dispatch.16, §FS-005-dispatch.5).
    pub fn last_ticket(&self) -> Option<PlanTicket> {
        self.tickets()
            .into_iter()
            .filter(|ticket| !ticket.id.contains('.') && !ticket.cancelled())
            .next_back()
    }

    /// The last ticket nothing has cancelled **that is about `matter`**, which
    /// is what a reopened ticket is ordered after (§FS-005-dispatch.5).
    ///
    /// A plan holds one matter's work, so this is usually
    /// [`Plan::last_ticket`] itself. It is not always: an older ephor named two
    /// matters' plans alike and wrote both into one file, and a hand can write
    /// anything — and a ticket ordered after work about something else is held
    /// until that work finishes, which is the costly half of a shared name
    /// (§FS-005-dispatch.3). So the matter is read from the ticket rather than
    /// assumed of the plan.
    ///
    /// A ticket recording no matter at all stays eligible: what nobody wrote
    /// down is no evidence of another matter, and nothing that chains today
    /// stops chaining.
    pub fn last_ticket_about(&self, plan_id: &str, matter: &str) -> Option<PlanTicket> {
        self.tickets()
            .into_iter()
            .filter(|ticket| !ticket.id.contains('.') && !ticket.cancelled())
            .filter(|ticket| {
                self.matter_of(plan_id, &ticket.id)
                    .is_none_or(|about| about == matter)
            })
            .next_back()
    }

    /// The matter one ticket is about, as the ticket itself records it
    /// (§FS-005-dispatch.8): the `id` ephor wrote into this plan's own metadata
    /// block. None where the ticket records none.
    ///
    /// [`Plan::task_meta`] reads the same block for the store's words and
    /// subtracts every name ephor writes there; this reads one of those names
    /// and nothing else, which is why the two do not share a body.
    pub fn matter_of(&self, plan_id: &str, task_id: &str) -> Option<String> {
        let doc = serde_yaml::from_str::<serde_yaml::Value>(self.frontmatter_yaml()?).ok()?;
        let tasks = doc.get("metadata").and_then(|node| node.get("tasks"))?;
        // The bare id is canonical; the plan-qualified spelling is accepted for
        // the same reason it is there (§FS-006-project-interface.7).
        let said =
            entry(tasks, task_id).or_else(|| entry(tasks, &format!("{plan_id}.{task_id}")))?;
        match entry(said, "id")? {
            serde_yaml::Value::String(id) if !id.is_empty() => Some(id.clone()),
            _ => None,
        }
    }

    /// One ticket by id, as the plan has it.
    pub fn ticket(&self, id: &str) -> Option<PlanTicket> {
        self.tickets().into_iter().find(|ticket| ticket.id == id)
    }

    pub fn append(&mut self, ticket: &Ticket) {
        if !self.text.contains(TASKS_HEADING) {
            self.text.push_str(&format!("\n{TASKS_HEADING}\n"));
        }
        if !self.text.ends_with('\n') {
            self.text.push('\n');
        }
        self.text.push('\n');
        self.text.push_str(&ticket.render());
    }

    /// Record one ticket's item as structured metadata, where a program in the
    /// state machine can be handed it (§FS-005-dispatch.8).
    ///
    /// Merged, never rewritten: the runtime keeps its own per-task bookkeeping
    /// in this same block, and a ticket that replaced it would break every
    /// counted loop in the plan.
    pub fn set_metadata(&mut self, ticket: &str, values: &[(&str, String)]) {
        let entry = std::iter::once(format!("    {ticket}:\n"))
            .chain(
                values
                    .iter()
                    .filter(|(_, value)| !value.is_empty())
                    .map(|(key, value)| format!("      {key}: {}\n", yaml_string(value))),
            )
            .collect::<String>();

        let Some((open, close)) = self.frontmatter() else {
            // No block yet: one goes below the heading and its declaration,
            // which is where the runtime's language puts it.
            let body = format!("---\nmetadata:\n  tasks:\n{entry}---\n");
            let at = self.header_end();
            self.text.insert_str(at, &format!("\n{body}"));
            return;
        };
        let block = &self.text[open..close];
        let insert_at = |needle: &str| block.find(needle).map(|at| open + at + needle.len());
        match insert_at("\n  tasks:\n").or_else(|| insert_at("metadata:\n")) {
            // Under an existing `tasks:`, or as the first thing under
            // `metadata:` — YAML does not care about the order of keys.
            Some(at) if block.contains("\n  tasks:\n") => self.text.insert_str(at, &entry),
            Some(at) => self.text.insert_str(at, &format!("  tasks:\n{entry}")),
            None => self
                .text
                .insert_str(open, &format!("metadata:\n  tasks:\n{entry}")),
        }
    }

    /// The frontmatter body, as `(start, end)` byte offsets between its
    /// fences. None when the plan has none.
    fn frontmatter(&self) -> Option<(usize, usize)> {
        let fence = format!("\n{FRONTMATTER_FENCE}\n");
        let header_end = self.header_end();
        let rest = &self.text[header_end..];
        let open = rest.find(&fence)? + header_end + fence.len();
        // Only a block that opens before any content is this plan's
        // frontmatter; a horizontal rule further down is prose.
        if self.text[header_end..open].trim().len() > FRONTMATTER_FENCE.len() {
            return None;
        }
        let close = self.text[open..].find(&fence)? + open + 1;
        Some((open, close))
    }

    /// Just past the title and its `**States:**` declaration.
    fn header_end(&self) -> usize {
        let mut end = 0;
        for line in self.text.lines() {
            if line.starts_with("# ") || line.starts_with("**States:**") {
                end += line.len() + 1;
                continue;
            }
            break;
        }
        end
    }

    /// What the plan says about one of its own tasks
    /// (§FS-006-project-interface.7): the block the store keeps under this
    /// task's id in its own frontmatter, held to §FS-005-dispatch.8's bound
    /// and with the names ephor writes there itself subtracted.
    ///
    /// The documented key is the task's own id; `<plan id>.<task id>` is
    /// accepted as an alias, and the bare id wins where a plan writes both.
    /// `metadata.tasks` is the runtime's own grammar, so it is spelled here
    /// and nowhere above this module (§REQ-001-boundary.5).
    pub fn task_meta(&self, plan_id: &str, task_id: &str) -> TaskMeta {
        let Some(block) = self.frontmatter_yaml() else {
            return TaskMeta::default();
        };
        let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(block) else {
            return TaskMeta::default();
        };
        let Some(tasks) = doc.get("metadata").and_then(|node| node.get("tasks")) else {
            return TaskMeta::default();
        };
        // The bare id is canonical; the plan-qualified spelling is accepted so
        // that a store already writing it keeps working.
        let said = entry(tasks, task_id)
            .or_else(|| entry(tasks, &format!("{plan_id}.{task_id}")))
            .and_then(serde_yaml::Value::as_mapping);
        let Some(said) = said else {
            return TaskMeta::default();
        };
        let mut meta = TaskMeta::default();
        for (key, value) in said {
            let Some(key) = scalar_key(key) else {
                // A key that is not a scalar is no name at all, so there is
                // nothing to report it under.
                continue;
            };
            // The runtime keeps its own per-task bookkeeping in this same
            // namespace, and ephor lays its plans inside the directory it
            // reads as a store, so the names ephor writes here are subtracted
            // on the way in rather than handed back as though the store had
            // said them (§FS-005-dispatch.8). Declined and not dropped: such a
            // name was never the store's word, so there is nothing lost and
            // nobody to tell — unlike a bound drop, which is a thing the store
            // did say and has lost. The list is derived from the two functions
            // that write it, never copied.
            if crate::work::dossier::written_into_a_ticket(&key) {
                continue;
            }
            match bounded(&key, value) {
                Ok(value) => {
                    meta.values.insert(key, value);
                }
                Err(why) => meta.dropped.push(format!("dropped '{key}' — {why}")),
            }
        }
        meta.dropped.extend(folded(&meta.values));
        meta
    }

    /// The frontmatter body as YAML, between its fences. Accepted wherever the
    /// ticket parser accepts a heading — indentation and all — because a store
    /// writes these files by hand and the two readers of one file may not
    /// disagree about where its frontmatter is (§FS-006-project-interface.7).
    fn frontmatter_yaml(&self) -> Option<&str> {
        let rest = &self.text[self.header_end()..];
        let mut at = 0usize;
        let mut open: Option<usize> = None;
        for line in rest.lines() {
            let starts = at;
            at = (at + line.len() + 1).min(rest.len());
            if line.trim() != "---" {
                // Only a block that opens before any content is this plan's
                // frontmatter; a horizontal rule further down is prose.
                if open.is_none() && !line.trim().is_empty() {
                    return None;
                }
                continue;
            }
            match open {
                None => open = Some(at),
                Some(open) => return Some(&rest[open..starts]),
            }
        }
        None
    }

    /// Rewrite the dossier and nothing else. Tickets are appended, never
    /// rewritten: their `**State:**` lines belong to the runtime, which may be
    /// advancing one right now (§FS-005-dispatch.4).
    pub fn set_dossier(&mut self, dossier: &str) -> bool {
        let (Some(open), Some(close)) =
            (self.text.find(DOSSIER_OPEN), self.text.find(DOSSIER_CLOSE))
        else {
            return false;
        };
        if close < open {
            return false;
        }
        let replacement = format!("{DOSSIER_OPEN}\n{}\n", dossier.trim_end());
        self.text.replace_range(open..close, &replacement);
        true
    }

    pub fn save(&self) -> Result<()> {
        write(&self.path, &self.text)
    }
}

/// `Task fix-1: title` out of a node heading, ignoring headings that are not
/// tickets. The grammar is the runtime's own parser's: three to six hashes —
/// `###` is depth 1 — then a kind word (matched by shape alone: Title Case is
/// the runtime's convention, its matching is case-insensitive), then a dotted
/// id with exactly as many segments as the heading is deep, a colon, a title.
/// The depth match is what keeps a prose heading out of the tickets: the
/// runtime enforces it, so a heading that fails it is a ticket nowhere.
fn ticket_heading(line: &str) -> Option<(String, String)> {
    let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
    if !(3..=6).contains(&hashes) {
        return None;
    }
    let rest = &line[hashes..];
    let body = rest.trim_start();
    if body.len() == rest.len() {
        // Nothing separated the hashes from the words: not a heading.
        return None;
    }
    let (kind, rest) = body.split_once(char::is_whitespace)?;
    if !kind
        .bytes()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        || !kind
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return None;
    }
    let (id, title) = rest.trim_start().split_once(':')?;
    let id = id.trim_end();
    let depth = hashes - 2;
    if id.split('.').count() != depth || !id.split('.').all(id_segment) {
        return None;
    }
    Some((id.to_string(), title.trim().to_string()))
}

/// The ids a `**Prior:**` list names: `Task a-1, Task b-2` is `a-1`, `b-2`.
/// The kind word is decoration in the runtime's grammar — a reference
/// resolves on the id alone, and its own readers accept the bare form — so
/// both spellings read the same here.
fn prior_ids(list: &str) -> Vec<String> {
    list.split(',')
        .filter_map(|reference| {
            let reference = reference.trim();
            let id = match reference.split_once(char::is_whitespace) {
                Some((kind, id))
                    if kind
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') =>
                {
                    id.trim()
                }
                _ => reference,
            };
            (!id.is_empty()).then(|| id.to_string())
        })
        .collect()
}

/// One id segment, as the runtime's grammar has it: a name — a letter, then
/// letters, digits, `-`, `_` — or a canonical number, `0` or digits without a
/// leading zero, fitting in 32 bits.
fn id_segment(segment: &str) -> bool {
    match segment.bytes().next() {
        Some(first) if first.is_ascii_alphabetic() => segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'),
        Some(first) if first.is_ascii_digit() => {
            (segment.len() == 1 || first != b'0') && segment.parse::<u32>().is_ok()
        }
        _ => false,
    }
}

/// The store a plan's own tasks live in, where that is not the work root:
/// the directory a plan rendered as a directory keeps its index in
/// (§FS-005-dispatch.28). None for a plan the root holds directly — a
/// `*.rhei.md` file beside the root's own machine — whose store is the root.
///
/// Two things follow from the shape and both are read here: where its tasks
/// are, and which machine says what their states mean
/// (§FS-006-project-interface.7). So the shape is recognized in this one
/// place and every surface that reads a floor asks it (§AR-007-runtime.1).
pub fn own_store(plan: &Path) -> Option<&Path> {
    match plan.file_name().and_then(|name| name.to_str()) {
        Some(INDEX) => plan.parent(),
        _ => None,
    }
}

/// The machine that answers for a plan's tasks, resolved the way the runtime
/// itself resolves it (§FS-005-dispatch.28): the machine declared in the
/// plan's own store, where the plan is a store of its own and declares one.
///
/// `Ok(None)` is the other two shapes at once, because the answer is the same
/// for both: a plan the work root holds directly, and a store of its own that
/// declares nothing — whose `**States:**` names a machine the runtime resolves
/// from the project it sits in. In both, the root's own machine is the one in
/// force, and the caller supplies it. `Err` is a store that declares a machine
/// which will not read, where nothing there is judged at all rather than
/// judged by a machine that answers for other work (§FS-005-dispatch.15).
pub fn own_machine(plan: &Path) -> Result<Option<WorkRoot>> {
    match own_store(plan) {
        Some(dir) => WorkRoot::open(dir),
        None => Ok(None),
    }
}

/// The task files of a plan rendered as a directory: the direct children of
/// the `tasks/` directory beside its index, in name order, read as they
/// stand (§FS-005-dispatch.28). Empty for a plan written as one file, which
/// is every plan ephor writes itself, or an absent task directory. An existing
/// collection that cannot be read fails the plan (§FS-006-project-interface.7).
fn task_files(plan: &Path) -> Result<Vec<PlanPart>> {
    let Some(dir) = own_store(plan) else {
        return Ok(Vec::new());
    };
    let dir = dir.join(TASKS_DIR);
    match fs::symlink_metadata(&dir) {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(EphorError::Command(format!(
                "Cannot inspect {}: {err}",
                dir.display()
            )))
        }
    }
    let entries = fs::read_dir(&dir)
        .map_err(|err| EphorError::Command(format!("Cannot read {}: {err}", dir.display())))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| {
            EphorError::Command(format!("Cannot read an entry in {}: {err}", dir.display()))
        })?;
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "md") {
            continue;
        }
        let metadata = fs::metadata(&path).map_err(|err| {
            EphorError::Command(format!("Cannot inspect {}: {err}", path.display()))
        })?;
        if metadata.is_file() {
            files.push(path);
        }
    }
    files.sort();
    files
        .into_iter()
        .map(|path| {
            Ok(PlanPart {
                text: read(&path)?,
                path,
            })
        })
        .collect()
}

/// Every ticket one plan file holds, in the order it was written. The
/// grammar is the runtime's and is spelled here alone (§AR-007-runtime.1),
/// so a plan's index and each of its task files are read the same way.
fn tickets_in(text: &str) -> Vec<PlanTicket> {
    let mut tickets: Vec<PlanTicket> = Vec::new();
    let mut in_header = false;
    for line in unfenced(text) {
        let trimmed = line.trim();
        if trimmed.starts_with("###") {
            match ticket_heading(trimmed) {
                Some((id, title)) => {
                    tickets.push(PlanTicket {
                        id,
                        title,
                        state: None,
                        assignee: None,
                        pinned: None,
                        prior: Vec::new(),
                    });
                    in_header = true;
                }
                // A heading that is not a ticket is content, and content
                // closes the header block it lands in.
                None => in_header = false,
            }
            continue;
        }
        if !in_header {
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let Some(last) = tickets.last_mut() else {
            in_header = false;
            continue;
        };
        if let Some(state) = trimmed.strip_prefix("**State:**") {
            if last.state.is_none() {
                last.state = Some(state.trim().to_string());
            }
        } else if let Some(assignee) = trimmed.strip_prefix("**Assignee:**") {
            let assignee = assignee.trim();
            if last.assignee.is_none() && !assignee.is_empty() {
                last.assignee = Some(assignee.to_string());
            }
        } else if let Some(prior) = trimmed.strip_prefix("**Prior:**") {
            if last.prior.is_empty() {
                last.prior = prior_ids(prior);
            }
        } else if let Some(value) = trimmed.strip_prefix("**Target:**") {
            // The full line makes the ticket its own authority on who
            // runs it, whatever else it carries (§FS-005-dispatch.14).
            if !value.trim().is_empty() {
                last.pinned = Some(Pin::Target);
            }
        } else if let Some(value) = trimmed.strip_prefix("**Model:**") {
            if !value.trim().is_empty() && last.pinned.is_none() {
                last.pinned = Some(Pin::Model);
            }
        } else if !(trimmed.starts_with("**") && trimmed.contains(":**")) {
            // The first content line ends the header; any other
            // `**Field:**` line keeps it open, as the runtime reads it.
            in_header = false;
        }
    }
    tickets
}

/// The lines of a document that are not inside a fenced block.
fn unfenced(text: &str) -> impl Iterator<Item = &str> {
    let mut fence: Option<String> = None;
    text.lines().filter(move |line| {
        let trimmed = line.trim_start();
        let marker = trimmed
            .chars()
            .next()
            .filter(|ch| *ch == '`' || *ch == '~')
            .map(|ch| {
                trimmed
                    .chars()
                    .take_while(|candidate| *candidate == ch)
                    .collect::<String>()
            })
            .filter(|run| run.len() >= 3);
        match (&fence, marker) {
            (None, Some(open)) => {
                fence = Some(open);
                false
            }
            (Some(open), Some(close))
                if close.len() >= open.len() && close.starts_with(&open[..1]) =>
            {
                fence = None;
                false
            }
            (Some(_), _) => false,
            (None, None) => true,
        }
    })
}

/// A rhei id for an item: what `{id_slug}` renders for it, held additionally
/// to what the runtime's grammar allows for a file stem — never empty and never
/// leading with anything but an ASCII letter (§FS-005-dispatch.2).
///
/// One reduction, one digest, two grammars. The digest is [`crate::slug`]'s and
/// is the field's own, so the naming is injective: two matters whose ids read
/// down to one readable half stay two plans, where a stem without it is a name
/// both of them answer to and the second matter's work is written into the
/// first's record (§FS-005-dispatch.3). The guard below is this grammar's and
/// not that field's, and it is the whole of the difference between the two
/// strings.
pub fn plan_id(item_id: &str) -> String {
    file_stem(&crate::slug::id_slug(item_id))
}

/// A rhei id for one laying of a workflow about an item: the same grammar over
/// the **pair** of the matter's id and the entry that laid it
/// (§FS-005-dispatch.2, §FS-005-dispatch.19).
///
/// The pair and not the two joined: a reduction that collapses punctuation to a
/// `-` is not injective over a pair joined by one, so a digest taken over the
/// join would let two layings about two matters name one directory
/// (§FS-005-dispatch.3).
pub fn laid_plan_id(item_id: &str, entry_id: &str) -> String {
    file_stem(&crate::slug::pair_slug(item_id, entry_id))
}

/// A rendered name held to the runtime's grammar for a file stem, which refuses
/// one beginning with anything but an ASCII letter where neither git nor a
/// filesystem cares (§FS-005-dispatch.2). The guard is this module's word
/// rather than the field's, so it is spelled here (§REQ-001-boundary.5).
fn file_stem(slug: &str) -> String {
    match slug.chars().next() {
        Some(first) if first.is_ascii_alphabetic() => slug.to_string(),
        _ => format!("item-{slug}"),
    }
}

/// A value as a YAML scalar. Always quoted: a branch called `you/ABC-42` is a
/// string, a number like `24898` is a string too — a script comparing it
/// against what a forge printed should not have to care that YAML would have
/// made one of them an integer.
fn yaml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', r"\\").replace('"', "\\\""))
}

fn one_line(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() <= 120 {
        return joined;
    }
    joined.chars().take(117).collect::<String>() + "…"
}

pub(crate) fn create_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)
        .map_err(|err| EphorError::Command(format!("Cannot create {}: {err}", dir.display())))
}

fn read(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .map_err(|err| EphorError::Command(format!("Cannot read {}: {err}", path.display())))
}

/// Write through a temporary file: the runtime may be reading a plan while
/// this runs, and half a plan is a plan that does not parse.
fn write(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir(parent)?;
    }
    let tmp = path.with_extension("ephor-tmp");
    fs::write(&tmp, content)
        .map_err(|err| EphorError::Command(format!("Cannot write {}: {err}", tmp.display())))?;
    fs::rename(&tmp, path)
        .map_err(|err| EphorError::Command(format!("Cannot rename {}: {err}", tmp.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ticket(id: &str, state: &str, body: &str) -> Ticket {
        Ticket {
            id: id.to_string(),
            title: "fix the red gate".to_string(),
            state: state.to_string(),
            prior: None,
            target: None,
            model: None,
            body: body.to_string(),
        }
    }

    #[test]
    fn the_shipped_machine_declares_the_states_the_shipped_recipes_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = WorkRoot::ensure(tmp.path(), SHIPPED_STATES).unwrap();
        assert_eq!(root.machine, "ephor-work");
        assert!(root.declares("fix"), "{:?}", root.state_names());
        assert!(root.declares("review"));
        assert!(root.declares("done"));
        assert!(!root.declares("nonexistent"));
        // Finality is what tells work in flight from work that is over.
        assert!(root.is_final("done"));
        assert!(!root.is_final("fix"));
        assert!(!root.is_final("nonexistent"));

        // A machine with a state the runtime will not leave on its own: work
        // parked there is waiting for a person (§FS-005-dispatch.9).
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("states.yaml"),
            concat!(
                "name: m\n",
                "states:\n",
                "  fix:\n    agent: x\n",
                "  needs-human:\n    gating: true\n",
                "  done:\n    final: true\n",
            ),
        )
        .unwrap();
        let root = WorkRoot::ensure(tmp.path(), SHIPPED_STATES).unwrap();
        assert!(root.is_gating("needs-human"));
        assert!(!root.is_gating("fix"));
        assert!(!root.is_final("needs-human"));
        assert!(root.is_final("done"));
        // The project is set up, and ignores itself so the checkout stays clean.
        assert!(tmp.path().join("index.panta.md").is_file());
        assert!(fs::read_to_string(tmp.path().join(".gitignore"))
            .unwrap()
            .contains('*'));
    }

    /// A poll that says who it waits for is read out of the machine, and a
    /// poll that says nothing is machine backoff — which is the whole
    /// difference between a run spending tokens and a run waiting on a
    /// person (§FS-005-dispatch.24).
    #[test]
    fn a_poll_declaring_who_it_waits_on_is_read_as_a_person_wait() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("states.yaml"),
            concat!(
                "name: m\n",
                "states:\n",
                // The block form, as the runtime's own examples write it.
                "  plan-approval:\n",
                "    program: [\"bash\", \"await.sh\"]\n",
                "    poll:\n",
                "      interval: 15m\n",
                "      max_attempts: 96\n",
                "      waiting_on: author\n",
                // A poll waiting on a build says nothing, and is not this.
                "  ci-wait:\n",
                "    program: [\"bash\", \"ci.sh\"]\n",
                "    poll:\n",
                "      interval: 5m\n",
                "      max_attempts: 12\n",
                // The flow form says the same thing on one line.
                "  review-wait:\n",
                "    poll: { interval: 1h, waiting_on: \"reviewer\", max_attempts: 4 }\n",
                // A label that is blank is a machine the runtime refuses, not
                // a person wait: reading it as one would hand out a slot.
                "  blank-wait:\n",
                "    poll:\n",
                "      interval: 5m\n",
                "      waiting_on:\n",
                // The key outside a poll block is not the declaration, and
                // neither is one inside a comment or an agent's prose.
                "  implement:\n",
                "    agent: x\n",
                "    waiting_on: author\n",
                "    instructions: |\n",
                "      poll:\n",
                "        waiting_on: nobody\n",
                "  reviewed:\n",
                "    # waiting_on: author\n",
                "    agent: x\n",
                "  done:\n    final: true\n",
            ),
        )
        .unwrap();
        let root = WorkRoot::open(tmp.path()).unwrap().unwrap();
        assert!(root.waits_on_person("plan-approval"));
        assert!(root.waits_on_person("review-wait"));
        assert!(!root.waits_on_person("ci-wait"));
        assert!(!root.waits_on_person("blank-wait"));
        assert!(!root.waits_on_person("implement"));
        assert!(!root.waits_on_person("reviewed"));
        assert!(!root.waits_on_person("done"));
        // A state nobody declared is not a person wait either, and neither
        // are the machine's other facts about the same states.
        assert!(!root.waits_on_person("nonexistent"));
        assert!(!root.is_gating("plan-approval"));
        assert!(root.is_final("done"));
        // A person-waiting poll is not gating and the shipped machine has
        // neither: absence is the reading every machine written before this
        // field keeps.
        let shipped = WorkRoot::ensure(&tmp.path().join("shipped"), SHIPPED_STATES).unwrap();
        assert!(shipped
            .state_names()
            .iter()
            .all(|state| !shipped.waits_on_person(state)));
    }

    /// The machine in force is the declared one where there is one and the
    /// runtime's built-in default where there is not — which is what the
    /// runtime itself resolves an undeclared project to, and so what a reader
    /// of somebody else's store must judge its tasks by
    /// (§FS-006-project-interface.7). `open` is unchanged: it still answers
    /// None, for the surfaces that must withhold judgment.
    #[test]
    fn the_machine_in_force_falls_back_to_the_runtimes_default() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(WorkRoot::open(tmp.path()).unwrap().is_none());
        let root = WorkRoot::in_force(tmp.path()).unwrap();
        assert!(root.declares("pending"), "{:?}", root.state_names());
        assert!(root.is_final("completed"));
        assert!(!root.is_final("pending"));

        // A declared machine is the one in force, and nothing of the default
        // leaks into it: `completed` is not one of its states at all.
        fs::write(
            tmp.path().join("states.yaml"),
            "name: custom\nstates:\n  todo:\n  verified:\n    final: true\n",
        )
        .unwrap();
        let root = WorkRoot::in_force(tmp.path()).unwrap();
        assert_eq!(root.machine, "custom");
        assert!(root.is_final("verified"));
        assert!(!root.is_final("completed"));

        // A machine that is there and cannot be read is an error, never the
        // default quietly standing in for it.
        fs::write(tmp.path().join("states.yaml"), "states:\n  todo:\n").unwrap();
        assert!(WorkRoot::in_force(tmp.path()).is_err());
    }

    /// A state waiting on files an earlier one writes is not somewhere a fresh
    /// ticket can start, and the openable ones are what a refusal offers
    /// instead (§FS-005-dispatch.6). This is the shape of the machine that put
    /// two tickets into `fix` with the `collect` that feeds it never run.
    #[test]
    fn a_state_that_waits_on_an_earlier_ones_files_is_not_one_to_start_in() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("states.yaml"),
            r#"name: work
states:
  collect:
    program:
      command: ci.sh
    outputs:
      - name: failures
        path: "f.md"
  fix:
    agent: a
    inputs:
      - name: failures
        path: "f.md"
  done:
    final: true
"#,
        )
        .unwrap();
        let root = WorkRoot::ensure(tmp.path(), SHIPPED_STATES).unwrap();

        assert!(root.needs_input("fix"));
        // Producing files is not waiting on them, and neither is saying nothing.
        assert!(!root.needs_input("collect"));
        assert!(!root.needs_input("done"));
        // Final states are over, not open, so the offer is `collect` alone.
        assert_eq!(root.openable_states(), vec!["collect".to_string()]);
    }

    /// The shipped machine declares the abandonment state and it is final;
    /// a machine spelling the name over a state it would leave again declares
    /// none, and neither does one without it (§FS-005-dispatch.16).
    #[test]
    fn the_abandonment_state_is_the_final_one_under_the_runtimes_name() {
        let tmp = tempfile::tempdir().unwrap();
        let root = WorkRoot::ensure(tmp.path(), SHIPPED_STATES).unwrap();
        assert_eq!(root.cancel_state(), Some(CANCELLED));
        assert!(root.is_final(CANCELLED));

        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("states.yaml"),
            "name: m\nstates:\n  fix:\n    agent: x\n  cancelled:\n    agent: y\n  done:\n    final: true\n",
        )
        .unwrap();
        let root = WorkRoot::ensure(tmp.path(), SHIPPED_STATES).unwrap();
        assert_eq!(root.cancel_state(), None, "not final is not abandonment");

        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("states.yaml"),
            "name: m\nstates:\n  fix:\n    agent: x\n  done:\n    final: true\n",
        )
        .unwrap();
        let root = WorkRoot::ensure(tmp.path(), SHIPPED_STATES).unwrap();
        assert_eq!(root.cancel_state(), None);
    }

    /// A ticket's `**Prior:**` list is read as ids, kind word or not; a
    /// cancelled ticket reads as such; and the ticket a new one follows is
    /// the last that was not taken back, so ephor's own chain never hangs
    /// off abandoned work (§FS-005-dispatch.16, §FS-005-dispatch.5).
    #[test]
    fn priors_are_read_and_a_new_ticket_follows_the_last_that_was_not_cancelled() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("p.rhei.md");
        fs::write(
            &path,
            concat!(
                "# Rhei: p\n**States:** m\n\n## Tasks\n\n",
                "### Task fix-gate-1: one\n**State:** done\n\nbody\n\n",
                "### Task fix-gate-2: two\n**State:** cancelled\n**Prior:** Task fix-gate-1\n\nbody\n\n",
                "### Task fix-gate-3: three\n**State:** cancelled\n**Prior:** fix-gate-1, Task fix-gate-2\n\nbody\n\n",
                "#### Task fix-gate-3.1: sub\n**State:** fix\n\nbody\n",
            ),
        )
        .unwrap();
        let plan = Plan::read(&path).unwrap().unwrap();
        let tickets = plan.tickets();
        assert_eq!(tickets[1].prior, vec!["fix-gate-1"]);
        assert_eq!(tickets[2].prior, vec!["fix-gate-1", "fix-gate-2"]);
        assert!(tickets[1].cancelled());
        assert!(!tickets[0].cancelled());
        assert_eq!(
            plan.ticket("fix-gate-2").map(|t| t.id),
            Some("fix-gate-2".to_string())
        );
        assert!(plan.ticket("fix-gate-9").is_none());
        // Not the cancelled ones, and not the subtask: the finished one.
        assert_eq!(
            plan.last_ticket().map(|t| t.id),
            Some("fix-gate-1".to_string())
        );
        assert_eq!(
            prior_ids("Task a-1, Bug 2.3,  c-9 "),
            vec!["a-1", "2.3", "c-9"]
        );
        assert!(prior_ids("  ").is_empty());
    }

    #[test]
    fn an_existing_machine_is_read_and_never_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let mine = "name: mine\nversion: 2\nstates:\n  triage:\n    agent: x\n  shipped:\n    final: true\n";
        fs::write(tmp.path().join("states.yaml"), mine).unwrap();
        let root = WorkRoot::ensure(tmp.path(), SHIPPED_STATES).unwrap();
        assert_eq!(root.machine, "mine");
        assert!(root.declares("triage"));
        assert!(!root.declares("fix"));
        assert_eq!(
            fs::read_to_string(tmp.path().join("states.yaml")).unwrap(),
            mine
        );
    }

    /// Someone else's runtime project in the same checkout: filling in the
    /// machine it does not declare would change what its own plans run under
    /// (§FS-005-dispatch.6). An empty one — what the runtime's `init` leaves —
    /// has no plans to disturb, and is the common case in a checkout where a
    /// reader ran it once and never wrote a plan.
    #[test]
    fn only_a_project_with_plans_of_its_own_refuses_a_machine() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("index.panta.md"), "# Panta: theirs\n").unwrap();
        let root = WorkRoot::ensure(tmp.path(), SHIPPED_STATES)
            .unwrap_or_else(|err| panic!("an empty project has nothing to lose: {err}"));
        assert_eq!(root.machine, "ephor-work");

        // The same project once it holds a plan.
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("index.panta.md"), "# Panta: theirs\n").unwrap();
        fs::write(tmp.path().join("auth.rhei.md"), "# Rhei: Auth\n").unwrap();
        let Err(err) = WorkRoot::ensure(tmp.path(), SHIPPED_STATES) else {
            panic!("their plans, their machine");
        };
        assert!(err.to_string().contains("ephor work states"), "{err}");
        assert!(!tmp.path().join("states.yaml").exists());

        // A workspace-shaped rhei counts as a plan too.
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("index.panta.md"), "# Panta: theirs\n").unwrap();
        fs::create_dir(tmp.path().join("billing")).unwrap();
        fs::write(
            tmp.path().join("billing/index.rhei.md"),
            "# Rhei: Billing\n",
        )
        .unwrap();
        assert!(WorkRoot::ensure(tmp.path(), SHIPPED_STATES).is_err());
    }

    /// Every plan a work root holds is found whoever wrote it
    /// (§FS-005-dispatch.15): the plan files and the directory workspaces
    /// among the direct children, with ids and paths handed back so nothing
    /// above this module spells the suffix (§AR-007-runtime.1) — and nothing
    /// hidden, nothing under `runtime/`, and nothing that merely mentions a
    /// plan is one.
    #[test]
    fn every_plan_a_root_holds_is_found_with_its_id() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("zeta.rhei.md"), "# Rhei: Zeta\n").unwrap();
        fs::create_dir(dir.join("billing")).unwrap();
        fs::write(dir.join("billing/index.rhei.md"), "# Rhei: Billing\n").unwrap();
        // None of these is a plan: the manifest, a backup, a hidden file, the
        // runtime's own artifacts, and a bare directory.
        fs::write(dir.join("index.panta.md"), "# Panta: theirs\n").unwrap();
        fs::write(dir.join("zeta.rhei.md.bak"), "old").unwrap();
        fs::write(dir.join(".draft.rhei.md"), "hidden").unwrap();
        fs::create_dir_all(dir.join("runtime")).unwrap();
        fs::write(dir.join("runtime/echo.rhei.md"), "artifact").unwrap();
        fs::create_dir(dir.join("notes")).unwrap();

        let found = plans_in(dir);
        assert_eq!(
            found,
            vec![
                FoundPlan {
                    plan_id: "billing".to_string(),
                    path: dir.join("billing/index.rhei.md"),
                },
                FoundPlan {
                    plan_id: "zeta".to_string(),
                    path: dir.join("zeta.rhei.md"),
                },
            ]
        );
        // A directory that is not there answers empty, not an error: the
        // enumeration probes places that may hold nothing.
        assert!(plans_in(&dir.join("nowhere")).is_empty());
    }

    /// A work root holding one plan of each provenance, for the two readers
    /// below to disagree about: the project's own, the project's own with an
    /// ephor ticket appended to it, one ephor authored, and one ephor asked
    /// the runtime to render with ephor's hidden corner beside it.
    fn a_root_of_every_provenance(dir: &Path) {
        // The project's own plan: no block of ephor's, no corner.
        fs::write(
            dir.join("theirs.rhei.md"),
            "# Rhei: theirs\n\n## Tasks\n\n### Task 1: Widen the retry window\n\
             **State:** pending\n",
        )
        .unwrap();
        // The project's own plan a dispatch appended a ticket to. Appending
        // is not causing a plan to exist (§FS-006-project-interface.7).
        fs::write(
            dir.join("appended.rhei.md"),
            "# Rhei: theirs too\n\n## Tasks\n\n### Task 1: Shorten the reset\n\
             **State:** pending\n\n### Task fix-1: Fix the red gate\n**State:** fix\n",
        )
        .unwrap();
        // A plan ephor authored, exactly as `Plan::create` writes one.
        let authored = Plan::create(
            &dir.join("authored.rhei.md"),
            "ephor-work",
            "authored",
            "acmeforge:acme/widget#95",
            &ticket("fix-1", "fix", "Fix the red gate.\n"),
        );
        fs::write(&authored.path, authored.text()).unwrap();
        // A plan ephor asked the runtime to render, named by ephor's own
        // hidden corner in the same root, keyed by the plan's id.
        fs::create_dir_all(dir.join("laid")).unwrap();
        fs::write(
            dir.join("laid/index.rhei.md"),
            "# Rhei: laid\n\n## Tasks\n\n### Task 1: Do the issue\n**State:** pending\n",
        )
        .unwrap();
        fs::create_dir_all(dir.join(".ephor/laid")).unwrap();
        fs::write(dir.join(".ephor/laid/dossier.md"), "# Do the issue\n").unwrap();
    }

    fn ids(found: &[FoundPlan]) -> Vec<String> {
        let mut ids: Vec<String> = found.iter().map(|plan| plan.plan_id.clone()).collect();
        ids.sort();
        ids
    }

    /// The feed-facing reader declines a plan ephor caused to exist
    /// (§FS-006-project-interface.7). Two marks, because ephor stands in two
    /// relations to the two plan shapes: the dossier block it writes into a
    /// plan it authored, and its own hidden corner beside a plan the runtime
    /// rendered for it, which is never ephor's to write (§REQ-001-boundary.1).
    /// Without this a recipe over this source is offered its own filing back
    /// as fresh work and mints again (§FS-005-dispatch.25).
    #[test]
    fn the_task_store_reader_declines_a_plan_ephor_caused_to_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        a_root_of_every_provenance(dir);

        assert_eq!(
            ids(&task_store_plans_in(dir).unwrap()),
            vec!["appended".to_string(), "theirs".to_string()],
        );
    }

    /// And the probing reader behind the work screen and the operations board
    /// returns all four: every plan a work root holds is watched whoever wrote
    /// it (§FS-005-dispatch.15). One directory, two readers, and only the
    /// feed-facing one applies the test above.
    #[test]
    fn the_probing_reader_still_returns_every_plan_whoever_wrote_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        a_root_of_every_provenance(dir);

        assert_eq!(
            ids(&plans_in(dir)),
            vec![
                "appended".to_string(),
                "authored".to_string(),
                "laid".to_string(),
                "theirs".to_string(),
            ],
        );
    }

    /// Deleting the mark restores the matter, and that is the documented
    /// consequence of putting the fact on disk rather than in the ledger:
    /// everything ephor writes into a checkout must be deletable
    /// (§REQ-001-boundary.4), and the one store that could not be deleted that
    /// way is forbidden this job (§FS-005-dispatch.4). An authored plan
    /// carries its mark inside the plan file and cannot lose it without losing
    /// the plan, which is the shape the loop is made of.
    #[test]
    fn a_rendered_plan_whose_mark_was_deleted_is_read_again() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        a_root_of_every_provenance(dir);
        fs::remove_dir_all(dir.join(".ephor")).unwrap();

        assert_eq!(
            ids(&task_store_plans_in(dir).unwrap()),
            vec![
                "appended".to_string(),
                "laid".to_string(),
                "theirs".to_string(),
            ],
        );
    }

    /// A store that answered before the mark was read still answers. The
    /// mark's read opens each candidate, and `discover_plans` recognizes a flat
    /// plan by its name alone, so an entry named like a plan that is not a
    /// readable file — a dangling link, a directory carrying the name, a plan
    /// taken between the listing and the read — would fail the whole recognized
    /// source where [`Plan::read`] had always skipped it
    /// (§FS-006-project-interface.7). Both readers agree, and the store's own
    /// plan beside it is not lost with it.
    #[test]
    fn an_entry_named_like_a_plan_that_is_not_a_file_does_not_fail_the_store() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(
            dir.join("theirs.rhei.md"),
            "# Rhei: theirs\n\n## Tasks\n\n### Task 1: Widen the retry window\n\
             **State:** pending\n",
        )
        .unwrap();
        fs::create_dir(dir.join("odd.rhei.md")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("gone.rhei.md"), dir.join("stray.rhei.md")).unwrap();

        let read = task_store_plans_in(dir).expect("the store answers");
        assert!(
            ids(&read).contains(&"theirs".to_string()),
            "{:?}",
            ids(&read)
        );
        assert_eq!(ids(&read), ids(&plans_in(dir)));
    }

    /// And a plan that is a file and cannot be read is still a source that did
    /// not answer (§FS-006-project-interface.7): the skip above is about a
    /// candidate that is no plan file, never about hiding a read that failed.
    #[test]
    fn a_plan_that_is_a_file_and_cannot_be_read_still_fails_the_store() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(dir.join("broken.rhei.md"), b"# Rhei: \xff\xfe not text\n").unwrap();

        let err = task_store_plans_in(dir).expect_err("the source did not answer");
        assert!(err.to_string().contains("broken.rhei.md"), "{err}");
    }

    /// And a candidate whose open would never return is skipped before it is
    /// opened (§FS-006-project-interface.7). Classifying a failure cannot cover
    /// this one, because nothing fails: a named pipe named like a plan waits
    /// for a writer that never comes, and the sweep this machine runs
    /// unattended would wait with it where a stat had always skipped it.
    ///
    /// The reader is called on a worker thread and its answer taken with a
    /// bound, so a regression goes red rather than hanging the suite; the
    /// thread is left behind when it blocks, which costs nothing in a process
    /// that is ending either way.
    #[test]
    #[cfg(unix)]
    fn a_candidate_whose_open_would_block_does_not_hang_the_store() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        fs::write(
            dir.join("theirs.rhei.md"),
            "# Rhei: theirs\n\n## Tasks\n\n### Task 1: Widen the retry window\n\
             **State:** pending\n",
        )
        .unwrap();
        let made = std::process::Command::new("mkfifo")
            .arg(dir.join("pipe.rhei.md"))
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !made {
            // No `mkfifo` to be had: there is no pipe to skip, and a tool this
            // machine lacks is not a defect of the reader.
            return;
        }

        let (tell, hear) = std::sync::mpsc::channel();
        let read = dir.clone();
        std::thread::spawn(move || {
            let _ = tell.send(task_store_plans_in(&read).map(|found| ids(&found)));
        });
        let read = hear
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("the store answers rather than waiting on the pipe")
            .expect("the store answers");
        assert!(read.contains(&"theirs".to_string()), "{read:?}");
    }

    /// The mark is ephor's dossier **block** and it stands at the head of the
    /// plan (§FS-006-project-interface.7). So a plan of the project's own that
    /// quotes the opening marker in its prose keeps every one of its tasks —
    /// including a directory workspace, whose index has no tasks heading for a
    /// scan to stop at — and a block left unclosed is no block; while a plan
    /// ephor authored is still declined with the frontmatter
    /// [`Plan::set_metadata`] writes between its title and its block.
    #[test]
    fn the_mark_is_the_whole_block_and_only_at_the_head_of_the_plan() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::write(
            dir.join("quoted.rhei.md"),
            format!(
                "# Rhei: quoted\n\nWhen ephor dispatches it writes {DOSSIER_OPEN} into the\n\
                 plan, and {DOSSIER_CLOSE} after it.\n\n{TASKS_HEADING}\n\n\
                 ### Task 1: Say what the marker is\n**State:** pending\n"
            ),
        )
        .unwrap();
        // A directory workspace exactly as the runtime renders one: the tasks
        // are files under `tasks/` and the index has no tasks heading at all,
        // so nothing but the head of the file bounds the scan.
        fs::create_dir_all(dir.join("workspace/tasks")).unwrap();
        fs::write(
            dir.join("workspace/index.rhei.md"),
            format!("# Rhei: workspace\n\nEvery plan ephor authors carries {DOSSIER_OPEN}.\n"),
        )
        .unwrap();
        fs::write(
            dir.join("workspace/tasks/01-first.md"),
            "### Task first: Read the marker as prose\n**State:** pending\n",
        )
        .unwrap();
        fs::write(
            dir.join("unclosed.rhei.md"),
            format!(
                "# Rhei: unclosed\n\n{DOSSIER_OPEN}\n\n{TASKS_HEADING}\n\n\
                 ### Task 1: Open what is never closed\n**State:** pending\n"
            ),
        )
        .unwrap();
        // A plan ephor authored, as a dispatch leaves it: the metadata block
        // the runtime's language puts below the title sits above the dossier,
        // and the dossier itself quotes the item — whose body may carry any
        // heading at all, so nothing in there may end the scan.
        let mut authored = Plan::create(
            &dir.join("authored.rhei.md"),
            "ephor-work",
            "authored",
            "acmeforge:acme/widget#95\n\n## Tasks\n\nwhat the item's own body said",
            &ticket("fix-1", "fix", "Fix the red gate.\n"),
        );
        authored.set_metadata("fix-1", &[("item", "acmeforge:acme/widget#95".to_string())]);
        fs::write(&authored.path, authored.text()).unwrap();

        assert_eq!(
            ids(&task_store_plans_in(dir).unwrap()),
            vec![
                "quoted".to_string(),
                "unclosed".to_string(),
                "workspace".to_string(),
            ],
        );
        assert_eq!(
            ids(&plans_in(dir)),
            vec![
                "authored".to_string(),
                "quoted".to_string(),
                "unclosed".to_string(),
                "workspace".to_string(),
            ],
        );
    }

    /// Origin tracking preserves the shared reader's index-first, sorted
    /// task-file order and task metadata (§FS-006-project-interface.7, §FS-005-dispatch.28).
    #[test]
    fn directory_task_origins_preserve_ticket_order_and_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(INDEX);
        fs::write(
            &path,
            "# Rhei: work\n\n### Task first: In the index\n**State:** pending\n",
        )
        .unwrap();
        let tasks = tmp.path().join(TASKS_DIR);
        fs::create_dir(&tasks).unwrap();
        fs::write(
            tasks.join("02-last.md"),
            "### Task last: Last task\n**State:** completed\n",
        )
        .unwrap();
        fs::write(
            tasks.join("01-next.md"),
            "### Task next: Next task\n**State:** pending\n**Prior:** Task first\n\n\
             #### Step next.child: Nested task\n**State:** pending\n",
        )
        .unwrap();
        fs::write(tasks.join("notes.txt"), "### Task ignored: Notes\n").unwrap();
        fs::create_dir(tasks.join("nested.md")).unwrap();
        fs::write(
            tasks.join("nested.md/hidden.md"),
            "### Task hidden: Nested file\n",
        )
        .unwrap();

        let plan = Plan::read(&path).unwrap().unwrap();
        let origins: Vec<_> = plan
            .tickets_with_paths()
            .map(|(ticket, path)| (ticket.id, path.to_path_buf()))
            .collect();
        assert_eq!(
            origins,
            vec![
                ("first".to_string(), path.clone()),
                ("next".to_string(), tasks.join("01-next.md")),
                ("next.child".to_string(), tasks.join("01-next.md")),
                ("last".to_string(), tasks.join("02-last.md")),
            ]
        );
        let tickets = plan.tickets();
        assert_eq!(
            tickets
                .iter()
                .map(|ticket| ticket.id.as_str())
                .collect::<Vec<_>>(),
            origins
                .iter()
                .map(|(id, _)| id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(tickets[1].prior, vec!["first"]);
        assert_eq!(tickets[3].state.as_deref(), Some("completed"));
    }

    /// The block a store keeps about one of its own tasks, read where it is
    /// written (§FS-006-project-interface.7). The seam's own tests pin the
    /// publishing; these pin the reading, where each rule can be read on its
    /// own.
    ///
    /// The bodies are written flush with the left margin on purpose: this is a
    /// file a store wrote, and its frontmatter is at column zero.
    fn read_meta(tmp: &Path, body: &str, plan_id: &str, task_id: &str) -> TaskMeta {
        let path = tmp.join(format!("{plan_id}.rhei.md"));
        fs::write(&path, body).unwrap();
        Plan::read(&path)
            .unwrap()
            .unwrap()
            .task_meta(plan_id, task_id)
    }

    fn carried(meta: &TaskMeta) -> serde_json::Value {
        serde_json::Value::Object(meta.values.clone())
    }

    /// The documented key is the task's own id; the plan-qualified spelling is
    /// accepted as an alias so a store already writing it keeps working, and
    /// the bare id wins where a plan writes both
    /// (§FS-006-project-interface.7).
    #[test]
    fn a_task_block_is_found_under_its_own_id_or_the_plan_qualified_alias() {
        let tmp = tempfile::tempdir().unwrap();
        let bare = read_meta(
            tmp.path(),
            "# Rhei: work\n\n---\nmetadata:\n  tasks:\n    1:\n      context: acme-labs\n---\n",
            "work",
            "1",
        );
        assert_eq!(
            carried(&bare),
            serde_json::json!({ "context": "acme-labs" })
        );

        let aliased = read_meta(
            tmp.path(),
            "# Rhei: work\n\n---\nmetadata:\n  tasks:\n    work.1:\n      context: aliased\n---\n",
            "work",
            "1",
        );
        assert_eq!(
            carried(&aliased),
            serde_json::json!({ "context": "aliased" })
        );

        let both = read_meta(
            tmp.path(),
            "# Rhei: work\n\n---\nmetadata:\n  tasks:\n    work.1:\n      context: aliased\n    1:\n      context: canonical\n---\n",
            "work",
            "1",
        );
        assert_eq!(
            carried(&both),
            serde_json::json!({ "context": "canonical" })
        );

        // A task nobody said anything about said nothing: absent, never empty
        // (§AR-006-matters).
        let silent = read_meta(tmp.path(), "# Rhei: work\n\n## Tasks\n", "work", "1");
        assert_eq!(silent, TaskMeta::default());
    }

    /// The bound in isolation (§FS-005-dispatch.8): scalars, a key a shell
    /// will take, an identifier rather than prose, 1 KiB. The offending key
    /// goes, the rest is carried, and each drop says which part of the bound
    /// it broke.
    #[test]
    fn the_bound_drops_the_key_and_carries_the_rest() {
        let tmp = tempfile::tempdir().unwrap();
        let body = "\
# Rhei: work

---
metadata:
  tasks:
    1:
      context: acme-labs
      tier: 1
      live: true
      owners:
        - ana
      stateVisits:
        fix: 2
      essay: ESSAY
      'rollout pct': 50
      handover: |
        One line.

        And another.
---
"
        .replace("ESSAY", &"x".repeat(META_VALUE_CAP + 1));
        let read = read_meta(tmp.path(), &body, "work", "1");
        // A number and a boolean are identifiers, and they keep the spelling
        // the store wrote them in.
        assert_eq!(
            carried(&read),
            serde_json::json!({ "context": "acme-labs", "tier": 1, "live": true })
        );
        for (key, why) in [
            ("owners", "not a scalar"),
            ("stateVisits", "not a scalar"),
            ("essay", "at most"),
            ("rollout pct", "a shell will take"),
            ("handover", "prose"),
        ] {
            assert!(
                read.dropped
                    .iter()
                    .any(|drop| drop.contains(key) && drop.contains(why)),
                "'{key}' was not dropped for {why}:\n{}",
                read.dropped.join("\n")
            );
        }
    }

    /// The subtraction in isolation (§FS-005-dispatch.8): ephor lays its plans
    /// inside the directory it reads as a store and keeps its own per-task
    /// bookkeeping in this very namespace, so the names it writes there are
    /// taken back out rather than handed over as the store's words. Derived
    /// from the two lists that write them, so this holds for a name added to
    /// either.
    ///
    /// And taken out **silently**: such a name was never this store's word, so
    /// nothing was lost and there is nobody to tell. A work root of ephor's own
    /// is a store like any other, and a note per written name per open ticket
    /// per refresh would bury the drops that do mean something.
    #[test]
    fn the_names_the_runtime_writes_here_are_subtracted() {
        let tmp = tempfile::tempdir().unwrap();
        let written: String = crate::work::dossier::SUBJECT_METADATA
            .iter()
            .chain(crate::work::dossier::INSTRUCTION_METADATA.iter())
            .map(|key| format!("      {key}: mine\n"))
            .collect();
        let read = read_meta(
            tmp.path(),
            &format!(
                "# Rhei: work\n\n---\nmetadata:\n  tasks:\n    1:\n{written}      context: acme-labs\n---\n"
            ),
            "work",
            "1",
        );
        assert_eq!(
            carried(&read),
            serde_json::json!({ "context": "acme-labs" }),
            "the runtime's own bookkeeping came back as the store's words"
        );
        assert!(
            read.dropped.is_empty(),
            "a name ephor wrote here was reported as though the store had lost it:\n{}",
            read.dropped.join("\n")
        );
    }

    /// Two keys that fold to one variable name are reported like a dropped key
    /// (§FS-006-project-interface.3), naming both keys and the variable neither
    /// of them sets — while both keys stay in `meta`, where a selector and a
    /// template name them unambiguously.
    ///
    /// Reported here because the fold is a property of the carried key set
    /// alone, so the reader that holds the whole set can decide it; the summons
    /// that withholds the variable has no channel to say so on.
    #[test]
    fn two_keys_that_fold_to_one_name_are_reported_like_a_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let read = read_meta(
            tmp.path(),
            "# Rhei: work\n\n---\nmetadata:\n  tasks:\n    1:\n      roll-out: a\n      roll_out: b\n      context: acme-labs\n---\n",
            "work",
            "1",
        );
        // Both keys carried: the fold is about the variable, not about the map.
        assert_eq!(
            carried(&read),
            serde_json::json!({ "roll-out": "a", "roll_out": "b", "context": "acme-labs" })
        );
        assert_eq!(
            read.dropped,
            vec![
                "'roll-out' and 'roll_out' fold to one name, so EPHOR_META_ROLL_OUT is set by neither"
                    .to_string()
            ]
        );
    }

    /// A key that folds with nobody is reported to nobody: one carried key is
    /// the ordinary case and it sets its variable.
    #[test]
    fn a_key_that_folds_with_nobody_is_not_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let read = read_meta(
            tmp.path(),
            "# Rhei: work\n\n---\nmetadata:\n  tasks:\n    1:\n      roll-out: a\n      context: acme-labs\n---\n",
            "work",
            "1",
        );
        assert!(read.dropped.is_empty(), "{:?}", read.dropped);
    }

    /// A number JSON has no room for is a value the map cannot carry, and it is
    /// dropped *out loud* (§FS-005-dispatch.8): YAML admits `.inf` and `.nan`,
    /// and neither is a scalar the accessor every surface reads the map through
    /// will answer with. An exponent past the end of a double is not one of
    /// these, because serde_yaml reads `1.0e400` as a string — and a string is
    /// an identifier the map carries.
    ///
    /// Pinned because this is the one way the reader and the accessor could
    /// come apart without anybody seeing it: a value carried here and refused
    /// there reaches no selector, no template and no variable, while the store's
    /// author is told nothing — which is the silent stop-matching that point
    /// forbids.
    #[test]
    fn a_number_the_map_cannot_carry_is_dropped_and_said() {
        let tmp = tempfile::tempdir().unwrap();
        let read = read_meta(
            tmp.path(),
            "# Rhei: work\n\n---\nmetadata:\n  tasks:\n    1:\n      context: acme-labs\n      limit: .inf\n      score: .nan\n---\n",
            "work",
            "1",
        );
        assert_eq!(
            carried(&read),
            serde_json::json!({ "context": "acme-labs" }),
            "a number the accessor will refuse was carried anyway"
        );
        for key in ["limit", "score"] {
            assert!(
                read.dropped
                    .iter()
                    .any(|drop| drop.contains(key) && drop.contains("not a scalar")),
                "'{key}' was dropped without saying so:\n{}",
                read.dropped.join("\n")
            );
        }
    }

    /// A fold is only ever reported about keys the map actually carries
    /// (§FS-006-project-interface.3): where one of two keys that fold to the
    /// same variable is dropped by the bound, the survivor sets that variable
    /// and there is no fold to report.
    ///
    /// The other way round would have the refresh say `EPHOR_META_ROLL_OUT` is
    /// set by neither key while the summons set it from the one that carried —
    /// one report contradicting the thing it reports on.
    #[test]
    fn a_fold_whose_other_key_was_dropped_is_not_reported() {
        let tmp = tempfile::tempdir().unwrap();
        let read = read_meta(
            tmp.path(),
            "# Rhei: work\n\n---\nmetadata:\n  tasks:\n    1:\n      roll-out: .inf\n      roll_out: b\n---\n",
            "work",
            "1",
        );
        assert_eq!(carried(&read), serde_json::json!({ "roll_out": "b" }));
        assert_eq!(
            read.dropped,
            vec!["dropped 'roll-out' — the value is not a scalar".to_string()],
            "the fold was reported about a key the map does not carry"
        );
    }

    /// The agreement itself, at the seam it has to hold across: every key the
    /// reader carries comes back out of `Item::meta`, the accessor the selector,
    /// the template and the summons all read the map through
    /// (§FS-005-dispatch.8).
    ///
    /// The reader asks the accessor for its verdict rather than restating the
    /// bound, so this cannot fail while both sides are in one place — it is
    /// here for the day one of them moves, and for the seam in between, which
    /// publishes the carried map into `raw` under ephor's own name.
    #[test]
    fn what_the_reader_carries_is_what_the_accessor_answers_with() {
        let tmp = tempfile::tempdir().unwrap();
        let read = read_meta(
            tmp.path(),
            "# Rhei: work\n\n---\nmetadata:\n  tasks:\n    1:\n      context: acme-labs\n      tier: 1\n      live: true\n      ratio: 0.5\n      limit: .inf\n      'roll out': 50\n      handover: |\n        One line.\n\n        And another.\n---\n",
            "work",
            "1",
        );
        let item = crate::feed::model::Item {
            id: "rhei:work.1".to_string(),
            project: "demo".to_string(),
            source: "rhei".to_string(),
            kind: crate::feed::model::ItemKind::Task,
            role: None,
            title: "renew the staging certificate".to_string(),
            url: None,
            state: Some("open".to_string()),
            needs_response: false,
            updated_at: chrono::Utc::now(),
            raw: serde_json::json!({ crate::feed::model::META: carried(&read) }),
        };
        let answered: Vec<String> = item.meta().unwrap().into_keys().collect();
        let mut expected: Vec<String> = read.values.keys().cloned().collect();
        expected.sort();
        assert_eq!(
            answered, expected,
            "the reader carried a key the accessor would not answer with, or the other way round"
        );
    }

    /// A directory workspace keeps its frontmatter in its own index, and its
    /// tasks in files beside it (§FS-006-project-interface.7).
    #[test]
    fn a_directory_workspaces_block_is_read_from_its_index() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("alpha");
        fs::create_dir_all(workspace.join("tasks")).unwrap();
        fs::write(
            workspace.join("index.rhei.md"),
            "# Rhei: alpha\n\n---\nmetadata:\n  tasks:\n    shared:\n      context: acme-labs\n---\n",
        )
        .unwrap();
        fs::write(
            workspace.join("tasks/01-shared.md"),
            "### Task shared: Alpha waits\n**State:** pending\n",
        )
        .unwrap();
        let plan = Plan::read(&workspace.join("index.rhei.md"))
            .unwrap()
            .unwrap();
        assert_eq!(
            plan.task_meta("alpha", "shared").values["context"],
            serde_json::json!("acme-labs")
        );
    }

    /// A plan lends its own heading where nothing dispatched it — a foreign
    /// plan has no matter to borrow a title from (§FS-005-dispatch.15).
    #[test]
    fn a_plan_says_its_own_title() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("widget-42.rhei.md");
        let created = Plan::create(
            &path,
            "ephor-work",
            "Widen the retry window",
            "dossier",
            &ticket("fix-1", "fix", "work"),
        );
        assert_eq!(created.title().as_deref(), Some("Widen the retry window"));

        // A hand-written plan with a plain heading, below frontmatter.
        fs::write(
            &path,
            "---\nowner: luna\n---\n\n# Audit the retry paths\n\n## Tasks\n",
        )
        .unwrap();
        let plain = Plan::read(&path).unwrap().unwrap();
        assert_eq!(plain.title().as_deref(), Some("Audit the retry paths"));

        // No heading, no title — never a guess.
        fs::write(&path, "just notes\n").unwrap();
        let bare = Plan::read(&path).unwrap().unwrap();
        assert_eq!(bare.title(), None);
    }

    #[test]
    fn a_plan_holds_the_dossier_and_its_tickets_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("widget-42.rhei.md");
        let mut plan = Plan::create(
            &path,
            "ephor-work",
            "acme/widget#42 — Retry\nwindow",
            "## The item\n\n- **project** widget\n",
            &ticket("fix-gate-1", "fix", "make it green"),
        );
        assert_eq!(plan.next_ticket_id("fix-gate"), "fix-gate-2");

        plan.append(&Ticket {
            prior: Some("fix-gate-1".to_string()),
            target: Some("claude-code[yolo]:anthropic:sonnet".to_string()),
            ..ticket("fix-gate-2", "fix", "it changed")
        });
        plan.save().unwrap();

        let reread = Plan::read(&path).unwrap().unwrap();
        let tickets = reread.tickets();
        assert_eq!(tickets.len(), 2);
        assert_eq!(tickets[0].id, "fix-gate-1");
        assert_eq!(tickets[0].state.as_deref(), Some("fix"));
        assert_eq!(reread.last_ticket().unwrap().id, "fix-gate-2");
        assert!(reread.text().contains("**Prior:** Task fix-gate-1"));
        assert!(reread.text().contains("**Target:** claude-code[yolo]"));
        // The title survives as one line.
        assert!(reread
            .text()
            .starts_with("# Rhei: acme/widget#42 — Retry window\n"));
    }

    /// A program in the state machine is handed the item as `{meta.*}`, and
    /// the runtime keeps its own bookkeeping in the same block
    /// (§FS-005-dispatch.8).
    #[test]
    fn metadata_is_merged_into_the_frontmatter_the_runtime_shares() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plan.rhei.md");
        let mut plan = Plan::create(
            &path,
            "ephor-work",
            "acme/widget#42",
            "## The item\n",
            &ticket("fix-gate-1", "collect", "work"),
        );
        plan.set_metadata(
            "fix-gate-1",
            &[
                ("repo", "acme/widget".to_string()),
                ("number", "42".to_string()),
                ("branch", r#"you/"odd"\name"#.to_string()),
                ("empty", String::new()),
            ],
        );
        let text = plan.text().to_string();
        // Below the heading and its declaration, which is where the runtime's
        // language puts frontmatter.
        assert!(
            text.starts_with("# Rhei: acme/widget#42\n**States:** ephor-work\n\n---\nmetadata:\n  tasks:\n    fix-gate-1:\n"),
            "{text}"
        );
        assert!(text.contains(r#"      number: "42""#), "{text}");
        assert!(
            text.contains(r#"      branch: "you/\"odd\"\\name""#),
            "{text}"
        );
        // Nothing is said about a field the item does not have.
        assert!(!text.contains("empty:"), "{text}");
        // The dossier and the ticket still follow it.
        assert!(text.contains("\n---\n\n<!-- ephor:dossier -->"), "{text}");
        assert_eq!(plan.tickets().len(), 1);

        // The runtime has since written its own counter into the block; a
        // second ticket joins it rather than replacing it.
        plan.text = plan.text.replace(
            "  tasks:\n",
            "  tasks:\n    fix-gate-1:\n      stateVisits:\n        collect: 1\n",
        );
        plan.set_metadata("answer-1", &[("repo", "acme/widget".to_string())]);
        let text = plan.text().to_string();
        assert!(text.contains("stateVisits:"), "{text}");
        assert!(
            text.contains("    answer-1:\n      repo: \"acme/widget\""),
            "{text}"
        );
        assert_eq!(text.matches("metadata:").count(), 1, "{text}");
        assert_eq!(text.matches("  tasks:").count(), 1, "{text}");
    }

    /// A claim is read where the runtime wrote one, and an unclaimed ticket
    /// answers None rather than an empty word (§FS-005-dispatch.15).
    #[test]
    fn a_claim_is_read_beside_the_state() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plan.rhei.md");
        let mut plan = Plan::create(
            &path,
            "ephor-work",
            "t",
            "## The item\n",
            &ticket("fix-gate-1", "fix", "work"),
        );
        plan.append(&ticket("fix-gate-2", "fix", "more work"));
        // The runtime's `next` wrote the claim; ephor only reads it.
        plan.text = plan.text.replacen(
            "**State:** fix\n",
            "**State:** fix\n**Assignee:** luna\n",
            1,
        );
        let tickets = plan.tickets();
        assert_eq!(tickets[0].assignee.as_deref(), Some("luna"));
        assert_eq!(tickets[1].assignee, None);
    }

    /// An execution line is a ticket's own only in its header — the
    /// `**Field:**` lines between the heading and the first content line,
    /// which is as far as the runtime reads metadata (§FS-005-dispatch.14). A
    /// body that merely mentions one, quoting a report or a dossier, pins
    /// nothing.
    #[test]
    fn an_execution_line_pins_only_from_the_tickets_header() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plan.rhei.md");
        let mut plan = Plan::create(
            &path,
            "ephor-work",
            "t",
            "## The item\n",
            &Ticket {
                target: Some("codex[yolo]:openai:gpt-5".to_string()),
                ..ticket("fix-gate-1", "fix", "work")
            },
        );
        plan.append(&Ticket {
            model: Some("sonnet".to_string()),
            ..ticket("answer-1", "fix", "reply")
        });
        plan.append(&ticket(
            "review-1",
            "fix",
            "The report said:\n\n**Target:** codex[yolo]:openai:gpt-5\n\nand stopped there.",
        ));
        let tickets = plan.tickets();
        assert_eq!(tickets[0].pinned, Some(Pin::Target));
        assert_eq!(tickets[1].pinned, Some(Pin::Model));
        // The quote sits past the header — the blank line after the body's
        // first content line has long closed it — so it pins nothing, and the
        // state past it is not re-read either.
        assert_eq!(tickets[2].pinned, None);
        assert_eq!(tickets[2].state.as_deref(), Some("fix"));
    }

    /// The floor reads every depth the runtime's language nests
    /// (§FS-005-dispatch.15): a subtask is a heading one level deeper with a
    /// dotted id, one segment per level, numeric or named — the runtime's own
    /// parser is the authority on that grammar.
    #[test]
    fn a_subtask_is_a_ticket_at_every_depth() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plan.rhei.md");
        let mut plan = Plan::create(
            &path,
            "ephor-work",
            "t",
            "## The item\n",
            &ticket("fix-gate-1", "fix", "work"),
        );
        plan.text.push_str(concat!(
            "\n#### Task fix-gate-1.1: split off\n**State:** needs-human\n\nchild\n",
            "\n##### Task fix-gate-1.1.re-check: deeper\n**State:** fix\n\ngrandchild\n",
            "\n###### Task fix-gate-1.1.re-check.0: as deep as the language goes\n",
            "**State:** fix\n\nleaf\n",
        ));
        let tickets = plan.tickets();
        let ids: Vec<&str> = tickets.iter().map(|ticket| ticket.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "fix-gate-1",
                "fix-gate-1.1",
                "fix-gate-1.1.re-check",
                "fix-gate-1.1.re-check.0"
            ]
        );
        assert_eq!(tickets[1].state.as_deref(), Some("needs-human"));
        // A new dispatch follows the last dispatch, never a subtask of one.
        assert_eq!(plan.last_ticket().unwrap().id, "fix-gate-1");
    }

    /// What the runtime's parser refuses is not a ticket here either: the
    /// heading's depth must match the id's segment count, a segment is a name
    /// or a canonical number, and seven hashes is past the language. Kind
    /// matching is case-insensitive — Title Case is the runtime's convention,
    /// not its grammar.
    #[test]
    fn a_heading_the_runtime_would_refuse_is_not_a_ticket() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plan.rhei.md");
        let mut plan = Plan::create(
            &path,
            "ephor-work",
            "t",
            "## The item\n",
            &ticket("fix-gate-1", "fix", "work"),
        );
        plan.text.push_str(concat!(
            "\n#### Task fix-gate-1-a: one segment, two headings deep\n**State:** fix\n\nx\n",
            "\n### Task a.b: two segments, one heading deep\n**State:** fix\n\nx\n",
            "\n#### Task fix-gate-1.01: a leading zero is not canonical\n**State:** fix\n\nx\n",
            "\n#### Task fix-gate-1.: an empty segment\n**State:** fix\n\nx\n",
            "\n####### Task d.d.d.d.d: past the language\n**State:** fix\n\nx\n",
            "\n#### The plan: prose, not a node\n\nx\n",
            "\n### task 9: a lowercase kind is valid grammar\n**State:** fix\n\nx\n",
        ));
        let ids: Vec<String> = plan.tickets().into_iter().map(|ticket| ticket.id).collect();
        assert_eq!(ids, ["fix-gate-1", "9"]);
    }

    #[test]
    fn a_conversation_quoting_a_plan_is_not_read_as_one() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plan.rhei.md");
        let dossier =
            "## The conversation\n\n````\n### Task ghost: not a ticket\n**State:** done\n````\n";
        let plan = Plan::create(
            &path,
            "ephor-work",
            "t",
            dossier,
            &ticket("fix-gate-1", "fix", "real work"),
        );
        let tickets = plan.tickets();
        assert_eq!(tickets.len(), 1);
        assert_eq!(tickets[0].id, "fix-gate-1");
        assert_eq!(tickets[0].state.as_deref(), Some("fix"));
    }

    #[test]
    fn refreshing_the_dossier_leaves_every_ticket_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plan.rhei.md");
        let mut plan = Plan::create(
            &path,
            "ephor-work",
            "t",
            "## The item\n\n- **state** open\n",
            &ticket("fix-gate-1", "fix", "work"),
        );
        // The runtime has advanced the ticket since it was written.
        plan.text = plan.text.replace("**State:** fix", "**State:** review");
        assert!(plan.set_dossier("## The item\n\n- **state** merged\n"));
        assert!(plan.text().contains("- **state** merged"));
        assert!(!plan.text().contains("- **state** open"));
        assert_eq!(plan.tickets()[0].state.as_deref(), Some("review"));
        assert!(plan.text().contains(DOSSIER_CLOSE));
    }

    /// Two matters whose ids differ only in punctuation read down to one
    /// readable half and must still be two plans: a stem they shared would put
    /// the second matter's work into the first's record rather than merely
    /// misname a file (§FS-005-dispatch.3).
    ///
    /// The stems are pinned as literals rather than derived, and they are the
    /// literals §FS-005-dispatch.2 pins: a moved digit is a matter that every
    /// later dispatch resolves to a second plan, and it should be caught here
    /// rather than by a run that cannot find its own work.
    #[test]
    fn two_matters_that_read_down_to_one_slug_are_two_plans() {
        // The report's own pair. The readable half is the same string, which
        // is the whole reason the digest is not a tiebreaker.
        assert_eq!(
            crate::slug::readable("rhei:window.retry-1"),
            crate::slug::readable("rhei:window-retry.1")
        );
        assert_ne!(
            plan_id("rhei:window.retry-1"),
            plan_id("rhei:window-retry.1")
        );
        assert_eq!(
            plan_id("rhei:window.retry-1"),
            "rhei-window-retry-1-17bbeb3b"
        );
        assert_eq!(
            plan_id("rhei:window-retry.1"),
            "rhei-window-retry-1-5ff4987f"
        );
        // And the pair the collision was first found on, a forge matter whose
        // id differs from another's by one character's class.
        assert_ne!(
            plan_id("github-issues:acme/app#1"),
            plan_id("github-issues:acme/app-1")
        );
        assert_eq!(
            plan_id("github-issues:agent-grounds/ephor#127"),
            "github-issues-agent-grounds-ephor-127-c1c7a9e5"
        );
    }

    /// The stem *is* `{id_slug}`, held additionally to the runtime's file-stem
    /// grammar — one reduction and one digest under two grammars, which is what
    /// makes §FS-005-dispatch.2's claim about the two strings checkable rather
    /// than merely written.
    ///
    /// The grammar is the whole of the difference: where the field's value
    /// begins with an ASCII letter the two are one string, and where it does
    /// not the stem is that value with `item-` in front of it. No stem ever
    /// begins with anything else, whatever the id held.
    #[test]
    fn a_plan_stem_is_the_field_held_to_the_runtimes_grammar() {
        for id in [
            "rhei:window.1",
            "rhei:window.retry-1",
            "github-prs:acme/widget#42",
            "forge:repo/123",
            "2fa:acme/vault#3",
            "42",
            ":::",
            "///",
            "",
            "  spaced  out  ",
            "HEAD",
        ] {
            let field = crate::slug::id_slug(id);
            let stem = plan_id(id);
            let expected = match field.starts_with(|ch: char| ch.is_ascii_alphabetic()) {
                true => field.clone(),
                false => format!("item-{field}"),
            };
            assert_eq!(stem, expected, "{id}");
            // The grammar this file stem answers to, on every id: a name that
            // leads with a letter, and nothing in it a stem may not carry.
            assert!(
                stem.starts_with(|ch: char| ch.is_ascii_alphabetic()),
                "{id} gave the stem {stem}"
            );
            assert!(
                stem.chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
                "{id} gave the stem {stem}"
            );
        }
        // The four strings the earlier contract pinned, as they read now. They
        // are on disk in plans the runtime laid before the digest, which is
        // what §FS-005-dispatch.3.1 carries over rather than leaves behind.
        assert_eq!(
            plan_id("github-prs:acme/widget#42"),
            "github-prs-acme-widget-42-922ddbdc"
        );
        assert_eq!(plan_id("forge:repo/123"), "forge-repo-123-11912849");
        assert_eq!(plan_id("42"), "item-42-87e38583");
        assert_eq!(plan_id("///"), "item-1d37d324");
        // And the one row of §FS-005-dispatch.2's table where the two strings
        // are not the same string.
        assert_eq!(
            crate::slug::id_slug("2fa:acme/vault#3"),
            "2fa-acme-vault-3-b0ad6965"
        );
        assert_eq!(
            plan_id("2fa:acme/vault#3"),
            "item-2fa-acme-vault-3-b0ad6965"
        );
    }
}
