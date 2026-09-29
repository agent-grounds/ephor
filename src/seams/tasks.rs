//! The project's own tasks, read where they live (§FS-006-project-interface.7).
//!
//! A project may keep its own work in its checkout — a plan directory, a
//! git-backed issue store — and a task store ephor recognizes is read through
//! the store's own files, as matters with their discussions
//! (§FS-007-matters), into the same feed under the same rules as anything a
//! forge reported. The word is **task** and not ticket or issue: a ticket is
//! what a remote tracker keys, an issue is what a forge files, and these are
//! the project's own.
//!
//! Recognition is by probed convention or manifest declaration; attribution is
//! the checkout's own project; and the stores are project-native things that
//! exist without ephor — a store's presence is a capability rung, never an
//! obligation.

use std::path::{Path, PathBuf};

use crate::feed::model::{Item, ItemKind};
use crate::manifest::Manifest;

/// A store ephor has a reader for. The kind is the store's own name, which is
/// also what a manifest declares (§FS-006-project-interface.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A plan directory: markdown plans whose task headings are the tasks.
    Plans,
    /// A git-backed issue store.
    Beads,
}

impl Kind {
    /// What a manifest calls it, and the scheme its matters' keys carry.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Plans => "rhei",
            Kind::Beads => "beads",
        }
    }

    /// The directory probed at the forest root — a name the project carries
    /// for its own sake (§REQ-001-boundary.2).
    pub fn probed(self) -> &'static str {
        match self {
            Kind::Plans => "panta",
            Kind::Beads => ".beads",
        }
    }

    pub fn parse(name: &str) -> Option<Kind> {
        match name {
            "rhei" | "panta" | "plans" => Some(Kind::Plans),
            "beads" => Some(Kind::Beads),
            _ => None,
        }
    }

    pub fn all() -> [Kind; 2] {
        [Kind::Plans, Kind::Beads]
    }
}

/// A store found in a checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    pub kind: Kind,
    pub path: PathBuf,
}

/// Every task store this checkout holds: what the manifest declares, then
/// what the well-known names find. Declared and probed are not alternatives
/// here the way a verb's binding is — a project may keep two stores, and both
/// are read.
pub fn find(root: &Path, manifest: Option<&Manifest>) -> Vec<Store> {
    let mut found: Vec<Store> = Vec::new();
    let mut push = |kind: Kind, path: PathBuf| {
        if path.is_dir() && !found.iter().any(|store| store.path == path) {
            found.push(Store { kind, path });
        }
    };
    if let Some(manifest) = manifest {
        for declared in &manifest.tasks {
            if let Some(kind) = Kind::parse(&declared.kind) {
                push(kind, root.join(&declared.path));
            }
        }
    }
    for kind in Kind::all() {
        push(kind, root.join(kind.probed()));
    }
    found
}

/// Read one store's tasks as items of the project the checkout belongs to
/// (§FS-006-project-interface.7). Attribution is the checkout's project: a
/// store in a checkout is about that checkout, and nothing has to guess.
///
/// A store that could not be read is an error and not an empty answer: it is a
/// source like any other, and "no tasks" has to mean there are none rather
/// than that nobody could look (§FS-001-forge-interface.6).
pub fn read(store: &Store, project: &str) -> Result<Vec<Item>, String> {
    read_reporting(store, project).map(|(items, _)| items)
}

/// The same read, with what the bound dropped on the way
/// (§FS-005-dispatch.8): one line per key a plan wrote that a matter may not
/// carry, already naming the matter it was about. The caller says them where
/// it says the rest of this source's answer — the store has answered, so its
/// slot is not marked failed, and a drop nobody is told about is how a
/// selector silently stops matching.
pub fn read_reporting(store: &Store, project: &str) -> Result<(Vec<Item>, Vec<String>), String> {
    match store.kind {
        Kind::Plans => plans(store, project),
        // The beads reader is the second one; a store ephor recognizes but
        // cannot read yet reports nothing rather than pretending
        // (§RM-003-boundary).
        Kind::Beads => Ok((Vec::new(), Vec::new())),
    }
}

/// The plan reader. Every plan in the directory is one matter per open task,
/// keyed by the store's own id — `rhei:<plan>.<task>` — because the store
/// named it and ephor does not get to rename it (§FS-007-matters.1).
///
/// A task in a final state is not read (§FS-006-project-interface.7): the
/// machine applicable to its plan says which states those are. Neither is any
/// task of a plan ephor caused to exist — the reader declines those plans
/// outright, so the seam is never offered ephor's own filing back as the
/// project's work (§FS-006-project-interface.7, §FS-005-dispatch.25).
fn plans(store: &Store, project: &str) -> Result<(Vec<Item>, Vec<String>), String> {
    use crate::work::runtime::plan::{self, WorkRoot};

    // Resolve the root only when a plan needs it (§FS-006-project-interface.7).
    let mut root_machine = None;
    let plans = plan::task_store_plans_in(&store.path)
        .map_err(|err| format!("cannot read {}: {err}", store.path.display()))?;

    let mut items = Vec::new();
    let mut notes = Vec::new();
    for found in plans {
        let path = found.path;
        // A plan the store holds and ephor cannot read is the store failing to
        // answer, not a plan with no tasks in it.
        let plan = plan::Plan::read(&path)
            .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
        let Some(plan) = plan else {
            continue;
        };
        // A directory workspace is a plan store of its own. Its declared
        // machine answers for its tasks; only absence permits root/default
        // fallback (§FS-006-project-interface.7, §FS-005-dispatch.6).
        let own_machine = plan::own_machine(&path).map_err(|err| {
            format!(
                "cannot read the state machine for {}: {err}",
                path.display()
            )
        })?;
        let machine = match own_machine.as_ref() {
            Some(machine) => machine,
            None => match root_machine {
                Some(ref machine) => machine,
                None => root_machine.insert(WorkRoot::in_force(&store.path).map_err(|err| {
                    format!(
                        "cannot read the state machine in {}: {err}",
                        store.path.display()
                    )
                })?),
            },
        };
        for (task, task_path) in plan.tickets_with_paths() {
            let state = task.state.clone().unwrap_or_default();
            let id = format!("{}:{}.{}", store.kind.name(), found.plan_id, task.id);
            // A finished task is history the store keeps, not news the feed
            // carries: it has no activity time of its own beyond this file's,
            // so it would resurface every time the plan was touched
            // (§FS-006-project-interface.7).
            if machine.is_final(&state) {
                continue;
            }
            // What the store itself said about this task, bounded and
            // subtracted, under ephor's own name (§FS-006-project-interface.7).
            // Written only where the plan said something that survived: absent
            // rather than empty is how silence is told from an empty map, and
            // the selector's silence rule turns on it (§AR-006-matters).
            let said = plan.task_meta(&found.plan_id, &task.id);
            notes.extend(said.dropped.iter().map(|why| format!("{id}: {why}")));
            let mut raw = serde_json::json!({ "plan": path.to_string_lossy() });
            if !said.values.is_empty() {
                raw[crate::feed::model::META] = serde_json::Value::Object(said.values);
            }
            items.push(Item {
                id,
                project: project.to_string(),
                source: store.kind.name().to_string(),
                // The project's own task, and not an issue a forge filed
                // (§FS-003-feed-categories.1).
                kind: ItemKind::Task,
                role: None,
                title: task.title.clone(),
                url: None,
                state: (!state.is_empty()).then_some(state),
                // A task waits on whoever keeps the store; nothing about it
                // says anyone is waiting on an answer.
                needs_response: false,
                // Activity follows the task's file (§FS-006-project-interface.7).
                updated_at: modified(task_path),
                raw,
            });
        }
    }
    Ok((items, notes))
}

/// When the store last changed, which is the closest thing a file-backed
/// task has to a last-activity time.
fn modified(path: &Path) -> chrono::DateTime<chrono::Utc> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map(chrono::DateTime::<chrono::Utc>::from)
        .unwrap_or_else(|_| chrono::Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_dir(root: &Path, name: &str, body: &str) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("work.rhei.md"), body).unwrap();
        dir
    }

    #[test]
    fn a_well_known_directory_is_a_store_and_absence_is_a_complete_answer() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find(tmp.path(), None).is_empty());

        plan_dir(tmp.path(), "panta", "# Plan\n");
        let found = find(tmp.path(), None);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, Kind::Plans);
    }

    #[test]
    fn a_declared_store_is_read_where_the_project_keeps_it() {
        let tmp = tempfile::tempdir().unwrap();
        plan_dir(tmp.path(), "docs/plans", "# Plan\n");
        let manifest = crate::manifest::parse(
            r#"{"tasks": [{"kind": "rhei", "path": "docs/plans"}]}"#,
            "ephor.json",
        )
        .unwrap();
        let found = find(tmp.path(), Some(&manifest));
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("docs/plans"));
    }

    /// `tickets` was this key's name before these were called what they are,
    /// and a manifest somebody already wrote goes on meaning what it meant
    /// (§FS-006-project-interface.11).
    #[test]
    fn the_older_spelling_of_the_key_is_still_read() {
        let tmp = tempfile::tempdir().unwrap();
        plan_dir(tmp.path(), "docs/plans", "# Plan\n");
        let manifest = crate::manifest::parse(
            r#"{"tickets": [{"kind": "rhei", "path": "docs/plans"}]}"#,
            "ephor.json",
        )
        .unwrap();
        let found = find(tmp.path(), Some(&manifest));
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("docs/plans"));
    }

    /// A project may keep two stores, and both are read — declaring one does
    /// not hide the other.
    #[test]
    fn a_declared_store_and_a_probed_one_are_both_stores() {
        let tmp = tempfile::tempdir().unwrap();
        plan_dir(tmp.path(), "panta", "# Plan\n");
        plan_dir(tmp.path(), "elsewhere", "# Plan\n");
        let manifest = crate::manifest::parse(
            r#"{"tasks": [{"kind": "rhei", "path": "elsewhere"}]}"#,
            "ephor.json",
        )
        .unwrap();
        let found = find(tmp.path(), Some(&manifest));
        assert_eq!(found.len(), 2);
        // Declared first: the project said it, so it leads.
        assert!(found[0].path.ends_with("elsewhere"));
    }

    /// The store named its tasks; ephor does not get to rename them
    /// (§FS-007-matters.1). And a task in a final state is the store's record
    /// rather than the feed's news, so it is not read at all
    /// (§FS-006-project-interface.7) — final here by the runtime's built-in
    /// default machine, since this store declares none of its own.
    #[test]
    fn a_plans_store_reads_its_open_tasks_under_the_stores_own_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = plan_dir(
            tmp.path(),
            "panta",
            "# Rhei: work\n\n\
             ## Tasks\n\n\
             ### Task 1: Widen the retry window\n**State:** pending\n\n\
             Do the thing.\n\n\
             ### Task 2: And the other\n**State:** completed\n\n\
             Done it.\n",
        );
        let store = Store {
            kind: Kind::Plans,
            path: dir,
        };
        let items = read(&store, "widget").expect("the store answered");
        assert_eq!(items.len(), 1, "{items:#?}");
        assert_eq!(items[0].id, "rhei:work.1");
        assert_eq!(items[0].title, "Widen the retry window");
        assert_eq!(items[0].state.as_deref(), Some("pending"));
        // Attribution is the checkout's project: nothing has to guess.
        assert!(items.iter().all(|item| item.project == "widget"));
        assert!(items.iter().all(|item| item.source == "rhei"));
        // The project's own task, never an issue a forge filed
        // (§FS-003-feed-categories.1).
        assert!(items.iter().all(|item| item.kind == ItemKind::Task));
    }

    /// A plan says something about one of its own tasks, in its own
    /// frontmatter, and the fact arrives on that task's matter under ephor's
    /// own name (§FS-006-project-interface.7). The documented key is the task's
    /// own id; the sibling task's block is its own, and a matter carries only
    /// what was said about it.
    #[test]
    fn what_the_plan_says_about_one_task_arrives_on_that_task() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = plan_dir(
            tmp.path(),
            "panta",
            "# Rhei: work\n\n             ---\n             metadata:\n             \x20 tasks:\n             \x20   1:\n             \x20     context: acme-labs\n             \x20     tier: 1\n             \x20   2:\n             \x20     context: field-notes\n             ---\n\n             ## Tasks\n\n             ### Task 1: Widen the retry window\n**State:** pending\n\n             ### Task 2: And the other\n**State:** pending\n",
        );
        let items = read(
            &Store {
                kind: Kind::Plans,
                path: dir,
            },
            "widget",
        )
        .expect("the store answered");
        let meta = |id: &str| {
            items
                .iter()
                .find(|item| item.id == id)
                .unwrap_or_else(|| panic!("no {id} in {items:#?}"))
                .raw["meta"]
                .clone()
        };
        assert_eq!(
            meta("rhei:work.1"),
            serde_json::json!({ "context": "acme-labs", "tier": 1 })
        );
        assert_eq!(
            meta("rhei:work.2"),
            serde_json::json!({ "context": "field-notes" })
        );
    }

    /// The plan-qualified spelling is accepted as an alias, so a store already
    /// writing it keeps working; the bare id is the documented one and wins
    /// where a plan somehow writes both (§FS-006-project-interface.7).
    #[test]
    fn the_plan_qualified_spelling_is_accepted_and_the_bare_id_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = plan_dir(
            tmp.path(),
            "panta",
            "# Rhei: work\n\n             ---\n             metadata:\n             \x20 tasks:\n             \x20   work.1:\n             \x20     context: acme-labs\n             \x20   work.2:\n             \x20     context: aliased\n             \x20   2:\n             \x20     context: canonical\n             ---\n\n             ## Tasks\n\n             ### Task 1: Widen the retry window\n**State:** pending\n\n             ### Task 2: And the other\n**State:** pending\n",
        );
        let items = read(
            &Store {
                kind: Kind::Plans,
                path: dir,
            },
            "widget",
        )
        .expect("the store answered");
        let meta = |id: &str| {
            items
                .iter()
                .find(|item| item.id == id)
                .unwrap_or_else(|| panic!("no {id} in {items:#?}"))
                .raw["meta"]["context"]
                .clone()
        };
        assert_eq!(meta("rhei:work.1"), serde_json::json!("acme-labs"));
        assert_eq!(meta("rhei:work.2"), serde_json::json!("canonical"));
    }

    /// The bound: a value that is not a scalar, a value over the cap, and a key
    /// no shell will take are each dropped, and the rest of the map is carried
    /// (§FS-005-dispatch.8). The runtime's own counters are a nested map, so the
    /// bound drops them before any subtraction has to think about them.
    #[test]
    fn a_key_the_bound_refuses_is_dropped_and_the_rest_is_carried() {
        let tmp = tempfile::tempdir().unwrap();
        let long = "x".repeat(2000);
        let dir = plan_dir(
            tmp.path(),
            "panta",
            &format!(
                "# Rhei: work\n\n                 ---\n                 metadata:\n                 \x20 tasks:\n                 \x20   1:\n                 \x20     context: acme-labs\n                 \x20     owners:\n                 \x20       - ana\n                 \x20     stateVisits:\n                 \x20       fix: 2\n                 \x20     essay: {long}\n                 \x20     'rollout pct': 50\n                 ---\n\n                 ## Tasks\n\n                 ### Task 1: Widen the retry window\n**State:** pending\n"
            ),
        );
        let items = read(
            &Store {
                kind: Kind::Plans,
                path: dir,
            },
            "widget",
        )
        .expect("the store answered");
        assert_eq!(items.len(), 1, "{items:#?}");
        assert_eq!(
            items[0].raw["meta"],
            serde_json::json!({ "context": "acme-labs" }),
            "the bound kept the wrong keys: {:#?}",
            items[0].raw["meta"]
        );
    }

    /// Read-only inward: ephor lays its plans inside the directory it reads, so
    /// the names it writes into this same namespace are subtracted on the way in
    /// rather than handed back as though the store had said them
    /// (§FS-005-dispatch.8).
    #[test]
    fn the_names_ephor_writes_here_itself_do_not_come_back_out() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = plan_dir(
            tmp.path(),
            "panta",
            "# Rhei: work\n\n             ---\n             metadata:\n             \x20 tasks:\n             \x20   1:\n             \x20     id: \"rhei:elsewhere.9\"\n             \x20     state: \"fix\"\n             \x20     title: \"Something else entirely\"\n             \x20     instruction: \"/tmp/brief.md\"\n             \x20     instruction_sha256: \"deadbeef\"\n             \x20     context: acme-labs\n             ---\n\n             ## Tasks\n\n             ### Task 1: Widen the retry window\n**State:** pending\n",
        );
        let items = read(
            &Store {
                kind: Kind::Plans,
                path: dir,
            },
            "widget",
        )
        .expect("the store answered");
        assert_eq!(
            items[0].raw["meta"],
            serde_json::json!({ "context": "acme-labs" }),
            "ephor's own bookkeeping came back as the store's words: {:#?}",
            items[0].raw["meta"]
        );
        // And the matter is still the store's own: nothing a laid ticket wrote
        // displaces what the plan says about the task.
        assert_eq!(items[0].id, "rhei:work.1");
        assert_eq!(items[0].title, "Widen the retry window");
    }

    /// A directory workspace is a plan of its own, so its block is read from its
    /// own `index.rhei.md` and keyed by the task ids under `tasks/`
    /// (§FS-006-project-interface.7).
    #[test]
    fn a_directory_workspace_is_read_from_its_own_index() {
        let tmp = tempfile::tempdir().unwrap();
        let store = tmp.path().join("panta");
        let workspace = store.join("alpha");
        std::fs::create_dir_all(workspace.join("tasks")).unwrap();
        std::fs::write(
            workspace.join("index.rhei.md"),
            "# Rhei: alpha\n\n             ---\n             metadata:\n             \x20 tasks:\n             \x20   shared:\n             \x20     context: acme-labs\n             ---\n",
        )
        .unwrap();
        std::fs::write(
            workspace.join("tasks/01-shared.md"),
            "### Task shared: Alpha waits\n**State:** pending\n",
        )
        .unwrap();
        let items = read(
            &Store {
                kind: Kind::Plans,
                path: store,
            },
            "widget",
        )
        .expect("the store answered");
        assert_eq!(items.len(), 1, "{items:#?}");
        assert_eq!(items[0].id, "rhei:alpha.shared");
        assert_eq!(
            items[0].raw["meta"]["context"],
            serde_json::json!("acme-labs")
        );
    }

    /// Which states are final is the store's own machine to say, not a list of
    /// spellings ephor carries: a store declaring `verified` final keeps its
    /// verified work to itself, and its `completed` — a state its machine never
    /// heard of — is as open as anything else (§FS-006-project-interface.7).
    #[test]
    fn the_stores_own_machine_says_which_tasks_are_over() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = plan_dir(
            tmp.path(),
            "panta",
            "# Rhei: work\n\n\
             ## Tasks\n\n\
             ### Task 1: Widen the retry window\n**State:** todo\n\n\
             Do the thing.\n\n\
             ### Task 2: And the other\n**State:** verified\n\n\
             Done it.\n\n\
             ### Task 3: A third\n**State:** completed\n\n\
             Not a state this machine has.\n",
        );
        std::fs::write(
            dir.join("states.yaml"),
            "name: custom\nstates:\n  todo:\n  verified:\n    final: true\n",
        )
        .unwrap();
        let store = Store {
            kind: Kind::Plans,
            path: dir,
        };
        let items = read(&store, "widget").expect("the store answered");
        let ids: Vec<&str> = items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["rhei:work.1", "rhei:work.3"], "{items:#?}");
    }

    /// A task with no state at all is open: nothing said it was over.
    #[test]
    fn a_task_with_no_state_is_read() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = plan_dir(
            tmp.path(),
            "panta",
            "# Rhei: work\n\n## Tasks\n\n### Task 1: Nameless\n\nNo state line.\n",
        );
        let store = Store {
            kind: Kind::Plans,
            path: dir,
        };
        let items = read(&store, "widget").expect("the store answered");
        assert_eq!(items.len(), 1, "{items:#?}");
        assert_eq!(items[0].state, None);
    }

    /// A machine ephor cannot read is the store failing to answer, exactly like
    /// a plan it cannot read: "no tasks" has to mean there are none
    /// (§FS-001-forge-interface.6).
    #[test]
    fn a_store_whose_machine_cannot_be_read_says_so_rather_than_answering_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = plan_dir(
            tmp.path(),
            "panta",
            "# Rhei: work\n\n## Tasks\n\n### Task 1: Widen it\n**State:** pending\n",
        );
        std::fs::write(dir.join("states.yaml"), "states:\n  todo:\n").unwrap();
        let store = Store {
            kind: Kind::Plans,
            path: dir,
        };
        let err = read(&store, "widget").expect_err("the machine has no name");
        assert!(err.contains("state machine"), "{err}");
    }

    #[test]
    fn a_store_ephor_cannot_read_yet_reports_nothing_rather_than_pretending() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".beads")).unwrap();
        let found = find(tmp.path(), None);
        assert_eq!(found[0].kind, Kind::Beads);
        assert!(read(&found[0], "widget")
            .expect("declining to read is not failing to read")
            .is_empty());
    }

    /// A store that is there and cannot be read is a source that did not
    /// answer, never a store with nothing in it
    /// (§FS-001-forge-interface.6): an empty section has to mean "no tasks".
    #[test]
    fn a_store_that_cannot_be_read_says_so_rather_than_answering_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store {
            kind: Kind::Plans,
            path: tmp.path().join("gone"),
        };
        let err = read(&store, "widget").expect_err("the directory is not there");
        assert!(err.contains("gone"), "{err}");
    }
}
