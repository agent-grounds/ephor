//! E2E-004-ticket-store: the tasks a project keeps in its own checkout become
//! matters.
//!
//! The scenario is a project that tracks its work in files rather than on a
//! forge: a plan directory in the checkout, kept for the project's own sake and
//! existing whether or not ephor ever runs (§FS-006-project-interface.7). ephor
//! recognizes the task store by its own name, reads it where it lives, and the
//! open tasks arrive in the feed as matters under the store's own ids —
//! nothing is renamed, nothing is written back, and no configuration was needed
//! to say the tasks belong to this project: they are in its checkout
//! (§FS-007-matters.1, §FS-008-attribution.2).
//!
//! The case keeps its id, which is stable; what it is about is the project's
//! own **tasks** — a ticket is what a remote tracker keys, an issue is what a
//! forge files, and these are neither (§FS-003-feed-categories.1).

#[path = "../support.rs"]
mod support;

use ephor::capabilities::{Bindings, CapabilitySet, Rung};

use predicates::prelude::*;
use serde_json::json;

use support::*;

/// A plan as the store itself writes one: tasks are task headings, and the
/// state line under each is the store's, not ephor's.
const PLAN: &str = "# Rhei: the retry window\n\n\
## Tasks\n\n\
### Task 1: Widen the retry window\n**State:** pending\n\n\
The window resets per attempt, which is not what the docs say.\n\n\
### Task 2: Document the reset\n**State:** completed\n\n\
Done.\n";

#[test]
fn a_store_in_the_checkout_is_read_where_it_lives_and_keeps_its_own_ids() {
    let world = World::new();
    // The probed convention: a directory the project keeps for itself, found
    // by its own name (§REQ-001-boundary.2).
    world.file("panta/window.rhei.md", PLAN);

    world.ephor().args(["refresh", PROJECT]).assert().success();

    // The open task is a matter, keyed as the store keys it: the store named
    // it and ephor does not get to rename it.
    let open = world.matter("rhei:window.1");
    assert_eq!(open["title"], "Widen the retry window");
    assert_eq!(open["state"], "pending");
    // The project's own task, and so its own row (§FS-003-feed-categories.1):
    // never an issue, which is what a forge files.
    assert_eq!(open["kind"], "task");
    // Attribution is the checkout's project: a store in a checkout is about
    // that checkout, and nothing has to guess (§FS-008-attribution.2).
    assert_eq!(open["placement"]["on"]["project"], PROJECT);
    // A task waits on whoever keeps the store; nothing about it says someone
    // is waiting on an answer.
    assert_eq!(open["needs_response"], false);

    // And the finished one is not a matter at all: it is history the store
    // keeps, not news the feed carries (§FS-006-project-interface.7). This
    // store declares no machine of its own, so what counts as finished is the
    // runtime's built-in default — `completed`, and final.
    assert!(!world.has_matter("rhei:window.2"), "{:#?}", world.matters());

    // And it is in the feed a person reads, beside whatever the forges said.
    world
        .ephor()
        .args(["feed"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Widen the retry window"));
}

/// Which of a store's tasks are over is the store's own machine to say, never
/// a list of spellings ephor carries (§FS-006-project-interface.7): a store
/// declaring `verified` final keeps its verified work to itself, and a state
/// that machine never heard of is as open as anything else.
#[test]
fn the_stores_own_machine_says_which_of_its_tasks_are_over() {
    let world = World::new();
    world.file(
        "panta/states.yaml",
        "name: custom\nstates:\n  todo:\n  verified:\n    final: true\n",
    );
    world.file(
        "panta/window.rhei.md",
        "# Rhei: the retry window\n\n\
## Tasks\n\n\
### Task 1: Widen the retry window\n**State:** todo\n\n\
The window resets per attempt.\n\n\
### Task 2: Document the reset\n**State:** verified\n\n\
Done.\n\n\
### Task 3: Retire the flag\n**State:** completed\n\n\
A word this machine never heard of.\n",
    );

    world.ephor().args(["refresh", PROJECT]).assert().success();

    assert_eq!(
        world.matter("rhei:window.1")["title"],
        "Widen the retry window"
    );
    // Final by this store's own machine, so it is the store's record and not
    // the feed's news.
    assert!(!world.has_matter("rhei:window.2"), "{:#?}", world.matters());
    // Not a state this machine declares at all, so nothing said the work was
    // over — ephor does not read `completed` as final on a store that never
    // declared it.
    assert_eq!(world.matter("rhei:window.3")["title"], "Retire the flag");
}

/// A store contains both its flat plans and the direct directory workspaces
/// the runtime renders. Each workspace is a plan of its own, so its directory
/// supplies the plan id, its `tasks/*.md` supply the tasks, and its local
/// machine — when it declares one — supplies their state semantics
/// (§FS-006-project-interface.7, §AR-007-runtime.1).
#[test]
fn directory_workspaces_are_read_with_their_own_machines_and_stable_ids() {
    let world = World::new();
    world.file(
        "panta/states.yaml",
        "name: root\nstates:\n  root-open:\n  local-open:\n    final: true\n  local-final:\n",
    );
    // The two existing flat spellings remain controls, under their existing
    // ids, while directory workspaces are added to the same ordinary feed.
    world.file(
        "panta/flat.rhei.md",
        "# Rhei: flat\n\n## Tasks\n\n### Task 1: Flat Rhei\n**State:** root-open\n",
    );
    world.file(
        "panta/legacy.panta.md",
        "# Panta: legacy\n\n## Tasks\n\n### Task 1: Flat Panta\n**State:** root-open\n",
    );

    world.file(
        "panta/alpha/index.rhei.md",
        "# Rhei: alpha\n**States:** alpha\n",
    );
    world.file(
        "panta/alpha/states.yaml",
        "name: alpha\nstates:\n  local-open:\n    gating: true\n  local-final:\n    final: true\n",
    );
    world.file(
        "panta/alpha/tasks/01-shared.md",
        "### Task shared: Alpha waits locally\n**State:** local-open\n",
    );
    world.file(
        "panta/alpha/tasks/02-finished.md",
        "### Task finished: Alpha is finished locally\n**State:** local-final\n",
    );

    // With no local machine beta falls back to the store root. Its task id is
    // deliberately the same as alpha's: the workspace id keeps them apart.
    world.file(
        "panta/beta/index.rhei.md",
        "# Rhei: beta\n**States:** root\n",
    );
    world.file(
        "panta/beta/tasks/01-shared.md",
        "### Task shared: Beta uses the root\n**State:** root-open\n",
    );

    world.ephor().args(["refresh", PROJECT]).assert().success();

    assert_eq!(world.matter("rhei:flat.1")["title"], "Flat Rhei");
    assert_eq!(world.matter("rhei:legacy.1")["title"], "Flat Panta");
    let alpha = world.matter("rhei:alpha.shared");
    let beta = world.matter("rhei:beta.shared");
    assert_eq!(alpha["title"], "Alpha waits locally");
    assert_eq!(alpha["state"], "local-open");
    assert_eq!(beta["title"], "Beta uses the root");
    assert!(
        !world.has_matter("rhei:alpha.finished"),
        "{:#?}",
        world.matters()
    );
    assert_eq!(
        alpha["raw"]["plan"],
        json!(world
            .forest()
            .join("panta/alpha/index.rhei.md")
            .to_string_lossy())
    );
    assert_eq!(
        beta["raw"]["plan"],
        json!(world
            .forest()
            .join("panta/beta/index.rhei.md")
            .to_string_lossy())
    );

    // IDs and provenance are readings of the layout, not minted per refresh.
    let first = (
        alpha["key"].clone(),
        alpha["raw"].clone(),
        beta["key"].clone(),
        beta["raw"].clone(),
    );
    world.ephor().args(["refresh", PROJECT]).assert().success();
    let alpha = world.matter("rhei:alpha.shared");
    let beta = world.matter("rhei:beta.shared");
    assert_eq!(
        first,
        (
            alpha["key"].clone(),
            alpha["raw"].clone(),
            beta["key"].clone(),
            beta["raw"].clone()
        )
    );

    // And with neither a workspace nor root declaration, the runtime's
    // default remains applicable: pending is open and completed is final.
    let defaulted = World::new();
    defaulted.file(
        "panta/gamma/index.rhei.md",
        "# Rhei: gamma\n**States:** rhei\n",
    );
    defaulted.file(
        "panta/gamma/tasks/01-open.md",
        "### Task open: Gamma is pending\n**State:** pending\n",
    );
    defaulted.file(
        "panta/gamma/tasks/02-finished.md",
        "### Task finished: Gamma is complete\n**State:** completed\n",
    );
    defaulted
        .ephor()
        .args(["refresh", PROJECT])
        .assert()
        .success();
    assert_eq!(
        defaulted.matter("rhei:gamma.open")["title"],
        "Gamma is pending"
    );
    assert!(!defaulted.has_matter("rhei:gamma.finished"));
}

/// A workspace that declares a machine has made it authoritative. If that
/// document will not read, the store reports that it did not answer instead
/// of silently judging the tasks by the readable root machine
/// (§FS-006-project-interface.7, §FS-005-dispatch.6).
#[test]
fn directory_workspaces_with_an_unreadable_machine_fail_the_store_read() {
    let world = World::new();
    world.file("panta/states.yaml", "name: root\nstates:\n  root-open:\n");
    world.file(
        "panta/broken/index.rhei.md",
        "# Rhei: broken\n**States:** broken\n",
    );
    world.file("panta/broken/states.yaml", "states:\n  root-open:\n");
    world.file(
        "panta/broken/tasks/01-work.md",
        "### Task work: Must not borrow root semantics\n**State:** root-open\n",
    );

    world.ephor().args(["refresh", PROJECT]).assert().code(4);

    let slot = &world.feed()["providers"]["rhei"];
    assert_eq!(slot["ok"], false, "{slot:#?}");
    let error = slot["error"].as_str().unwrap_or_default();
    assert!(error.contains("broken/states.yaml"), "{error}");
    assert!(error.contains("declares no state machine name"), "{error}");
    assert!(
        slot["matters"].as_array().is_some_and(Vec::is_empty),
        "{slot:#?}"
    );
}

fn directory_workspace(world: &World, name: &str) {
    world.file(&format!("panta/{name}/index.rhei.md"), "# Rhei: work\n");
    world.file(
        &format!("panta/{name}/tasks/01-work.md"),
        "### Task work: Workspace work\n**State:** pending\n",
    );
}

fn failed_task_store(world: &World, path: &str) {
    let slot = &world.feed()["providers"]["rhei"];
    assert_eq!(slot["ok"], false, "{slot:#?}");
    let error = slot["error"].as_str().unwrap_or_default();
    assert!(error.contains(path), "{error}");
    assert!(
        slot["matters"].as_array().is_some_and(Vec::is_empty),
        "{slot:#?}"
    );
}

/// Local authority does not depend on an unused root being readable; a plan
/// that needs the root still fails the whole source (§FS-006-project-interface.7).
#[test]
fn directory_workspaces_only_resolve_an_applicable_root_machine() {
    for needs_root in ["neither", "flat", "workspace"] {
        let world = World::new();
        world.file("panta/states.yaml", "states:\n  pending:\n");
        directory_workspace(&world, "alpha");
        world.file(
            "panta/alpha/states.yaml",
            "name: alpha\nstates:\n  pending:\n",
        );

        // Either shape requires root semantics only when no local machine answers.
        match needs_root {
            "flat" => {
                world.file("panta/flat.rhei.md", PLAN);
            }
            "workspace" => directory_workspace(&world, "beta"),
            _ => {
                world.ephor().args(["refresh", PROJECT]).assert().success();
                assert_eq!(world.feed()["providers"]["rhei"]["ok"], true);
                assert_eq!(world.matter("rhei:alpha.work")["state"], "pending");
                continue;
            }
        }
        world.ephor().args(["refresh", PROJECT]).assert().code(4);
        failed_task_store(&world, "panta/states.yaml");
    }
}

/// A task document that cannot be decoded is a source failure even under a
/// privileged user who can bypass mode bits (§FS-006-project-interface.7).
#[test]
fn directory_workspaces_with_unreadable_task_text_fail_the_store_read() {
    let world = World::new();
    directory_workspace(&world, "alpha");
    let path = world.forest().join("panta/alpha/tasks/01-work.md");
    std::fs::write(&path, [0xff, 0xfe]).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );

    world.ephor().args(["refresh", PROJECT]).assert().code(4);
    failed_task_store(&world, "alpha/tasks/01-work.md");
}

/// Permission loss must not become a healthy empty store (§FS-006-project-interface.7).
#[cfg(unix)]
#[test]
fn directory_workspaces_with_a_permission_denied_task_fail_the_store_read() {
    use std::os::unix::fs::PermissionsExt;

    let world = World::new();
    directory_workspace(&world, "alpha");
    let path = world.forest().join("panta/alpha/tasks/01-work.md");
    let original = std::fs::metadata(&path).unwrap().permissions();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let probe = std::fs::read_to_string(&path);
    let refresh = world.ephor().args(["refresh", PROJECT]).assert();
    std::fs::set_permissions(&path, original).unwrap();

    assert_eq!(
        probe
            .expect_err("run this permission regression as an unprivileged user")
            .kind(),
        std::io::ErrorKind::PermissionDenied
    );
    refresh.code(4);
    failed_task_store(&world, "alpha/tasks/01-work.md");
}

/// An absent or empty task collection is valid; an existing collection that
/// cannot be enumerated is a source failure (§FS-006-project-interface.7).
#[test]
fn directory_workspaces_distinguish_empty_tasks_from_enumeration_failure() {
    let world = World::new();
    world.file("panta/absent/index.rhei.md", "# Rhei: absent\n");
    world.file("panta/empty/index.rhei.md", "# Rhei: empty\n");
    std::fs::create_dir(world.forest().join("panta/empty/tasks")).unwrap();
    world.ephor().args(["refresh", PROJECT]).assert().success();
    let slot = &world.feed()["providers"]["rhei"];
    assert_eq!(slot["ok"], true, "{slot:#?}");
    assert!(slot["matters"].as_array().unwrap().is_empty());

    // A file in place of the task directory makes read_dir fail on any user account.
    world.file("panta/absent/tasks", "not a directory\n");
    world.ephor().args(["refresh", PROJECT]).assert().code(4);
    failed_task_store(&world, "absent/tasks");
}

/// Each task's activity follows its own file, while identity and plan
/// provenance stay fixed (§FS-006-project-interface.7, §FS-003-feed-categories.2).
#[test]
fn directory_workspaces_use_each_task_files_activity() {
    let world = World::new();
    directory_workspace(&world, "alpha");
    world.file(
        "panta/alpha/tasks/02-next.md",
        "### Task next: Next work\n**State:** pending\n",
    );
    world.file("panta/flat.rhei.md", PLAN);
    let set_modified = |path: &str, stamp: &str| {
        let stamp: chrono::DateTime<chrono::Utc> = stamp.parse().unwrap();
        std::fs::File::options()
            .write(true)
            .open(world.forest().join(path))
            .unwrap()
            .set_modified(stamp.into())
            .unwrap();
    };
    set_modified("panta/alpha/index.rhei.md", "2020-01-01T00:00:00Z");
    set_modified("panta/alpha/tasks/01-work.md", "2021-01-01T00:00:00Z");
    set_modified("panta/alpha/tasks/02-next.md", "2022-01-01T00:00:00Z");
    set_modified("panta/flat.rhei.md", "2023-01-01T00:00:00Z");

    world.ephor().args(["refresh", PROJECT]).assert().success();
    let work = world.matter("rhei:alpha.work");
    assert_eq!(work["updated_at"], "2021-01-01T00:00:00Z");
    assert_eq!(
        world.matter("rhei:alpha.next")["updated_at"],
        "2022-01-01T00:00:00Z"
    );
    assert_eq!(
        world.matter("rhei:flat.1")["updated_at"],
        "2023-01-01T00:00:00Z"
    );
    assert_eq!(
        work["raw"]["plan"],
        json!(world
            .forest()
            .join("panta/alpha/index.rhei.md")
            .to_string_lossy())
    );

    world.file(
        "panta/alpha/tasks/01-work.md",
        "### Task work: Updated work\n**State:** pending\n",
    );
    set_modified("panta/alpha/tasks/01-work.md", "2024-01-01T00:00:00Z");
    world.ephor().args(["refresh", PROJECT]).assert().success();
    let updated = world.matter("rhei:alpha.work");
    assert_eq!(updated["title"], "Updated work");
    assert_eq!(updated["updated_at"], "2024-01-01T00:00:00Z");
    assert_eq!(updated["key"], work["key"]);
    assert_eq!(updated["raw"], work["raw"]);
    assert_eq!(
        world.matter("rhei:alpha.next")["updated_at"],
        "2022-01-01T00:00:00Z"
    );
}

/// A project that keeps its store somewhere else says so in its manifest under
/// `tasks`, and declaring one does not hide the other — a project may keep two
/// (§FS-006-project-interface.7).
#[test]
fn a_declared_store_and_a_probed_one_are_both_read() {
    let world = World::new();
    world.file("panta/window.rhei.md", PLAN);
    world.file(
        "docs/plans/release.rhei.md",
        "# Rhei: the release\n\n## Tasks\n\n### Task 1: Cut the tag\n**State:** pending\n",
    );
    world.manifest(serde_json::json!({
        "tasks": [{ "kind": "rhei", "path": "docs/plans" }]
    }));

    world.ephor().args(["refresh", PROJECT]).assert().success();

    assert_eq!(
        world.matter("rhei:window.1")["title"],
        "Widen the retry window"
    );
    assert_eq!(world.matter("rhei:release.1")["title"], "Cut the tag");
}

/// `tickets` was that key's name before these were called what they are, and a
/// manifest somebody already wrote goes on meaning what it meant: the interface
/// evolves by addition (§FS-006-project-interface.11).
#[test]
fn a_manifest_written_the_older_way_is_still_read() {
    let world = World::new();
    world.file(
        "docs/plans/release.rhei.md",
        "# Rhei: the release\n\n## Tasks\n\n### Task 1: Cut the tag\n**State:** pending\n",
    );
    world.manifest(serde_json::json!({
        "tickets": [{ "kind": "rhei", "path": "docs/plans" }]
    }));

    world.ephor().args(["refresh", PROJECT]).assert().success();

    assert_eq!(world.matter("rhei:release.1")["title"], "Cut the tag");
    assert_eq!(world.matter("rhei:release.1")["kind"], "task");
}

/// Work about a change belongs in that change's working tree
/// (§FS-005-dispatch.3), so a project whose branches have workspaces of their
/// own keeps a store per workspace rather than one at the forest root — and
/// both places are read (§FS-006-project-interface.7).
///
/// Looking only at the root is how such a project came to write its plans into
/// a place nothing read again: dispatch put them in the branch workspace,
/// where the default work root is, and the feed showed none of them.
#[test]
fn a_store_in_a_branch_workspace_is_read_as_readily_as_one_at_the_root() {
    let world = World::new();
    let workspace = world.path().join("trees/you-ABC-42");
    std::fs::create_dir_all(&workspace).expect("the branch workspace");
    // The row names one branch. The disk has two, and the one it does not name
    // is where most of the work is — which is the situation this exists for.
    world.register(serde_json::json!({
        "branch_root_template": world.path().join("trees/{branch}").to_string_lossy(),
        "branches": [{ "id": "current", "branch": "you-ABC-42", "active": true }]
    }));
    std::fs::create_dir_all(workspace.join("panta")).expect("the store");
    std::fs::write(workspace.join("panta/window.rhei.md"), PLAN).expect("a plan in the branch");

    let unnamed = world.path().join("trees/you-XYZ-9/panta");
    std::fs::create_dir_all(&unnamed).expect("a store on a branch the row never named");
    std::fs::write(
        unnamed.join("old.rhei.md"),
        "# Rhei: old\n\n## Tasks\n\n### Task 1: Something else\n**State:** pending\n",
    )
    .expect("a plan on a branch the row never named");

    world.ephor().args(["refresh", PROJECT]).assert().success();

    // Both are read: the stores are verified on disk, because the row names
    // the branches somebody wrote down and the work is wherever branches were
    // actually checked out (§FS-006-project-interface.7).
    assert_eq!(
        world.matter("rhei:window.1")["title"],
        "Widen the retry window"
    );
    assert_eq!(world.matter("rhei:old.1")["title"], "Something else");

    // And the rung holds on the strength of the workspace, with nothing at the
    // forest root at all.
    let placement = ephor::branches::Placement::load(&world.registry_doc(), PROJECT)
        .expect("the registry describes the project");
    let set = CapabilitySet::resolve(
        PROJECT,
        Some(&placement),
        &Bindings {
            sources: 1,
            ..Bindings::default()
        },
    );
    assert!(set.holds(Rung::Tasks));
}

/// Finding a store is a capability, never an obligation
/// (§FS-006-project-interface.10): a project without one is watched exactly as
/// before, and the rung says in one sentence what it looked for.
#[test]
fn a_project_without_a_store_is_watched_all_the_same_and_says_what_it_looked_for() {
    let world = World::new();
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert!(world.matters().is_empty());

    let placement = ephor::branches::Placement::load(&world.registry_doc(), PROJECT)
        .expect("the registry describes the project");
    let bare = CapabilitySet::resolve(
        PROJECT,
        Some(&placement),
        &Bindings {
            sources: 1,
            ..Bindings::default()
        },
    );
    assert!(!bare.holds(Rung::Tasks));
    let reason = bare.reason(Rung::Tasks).expect("a missing rung says why");
    // Every place a store may live is named, not just the forest root: a
    // branch-addressable project keeps one per workspace
    // (§FS-006-project-interface.7), and a reason that named only the root
    // would send a reader to look in the wrong place.
    assert!(reason.contains("keeps no tasks of its own"), "{reason}");
    assert!(
        reason.contains(&world.forest().display().to_string()),
        "{reason}"
    );

    // The store appears, and so does the rung — resolved from the world as it
    // is now rather than from anything written down (§AR-005-capabilities.1).
    world.file("panta/window.rhei.md", PLAN);
    let with_store = CapabilitySet::resolve(
        PROJECT,
        Some(&placement),
        &Bindings {
            sources: 1,
            ..Bindings::default()
        },
    );
    assert!(with_store.holds(Rung::Tasks));
}

/// A project's own task carries no role at all (§FS-003-feed-categories.1),
/// so a `roles` selector — non-empty by definition once written — refuses
/// every one of them, correctly: role-less matches only an empty `roles`. But
/// a recipe covering issues and pull requests with `roles: [author]` used to
/// silently never fire on a task too, and `ephor work offers` said only
/// "nothing matches this matter" with no way to tell that from nothing being
/// configured at all. It now names the recipe and the field that refused it,
/// in the same words on both readings (§FS-005-dispatch.27, §REQ-002-parity.3).
#[test]
fn a_role_less_task_names_the_recipe_a_roles_selector_excluded() {
    let world = World::new();
    world.file("panta/window.rhei.md", PLAN);
    world.configure(json!({
        "projects": { PROJECT: { "work": { "recipes": [ {
            "id": "task-work",
            "description": "handle a task",
            "when": { "kinds": ["task"], "roles": ["author"] },
            "brief": "Handle {title}."
        } ] } } }
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();

    let view = shaped(
        "work",
        &world
            .ephor()
            .args(["work", "offers", "--item", "rhei:window.1", "--json"])
            .output()
            .expect("the work reading"),
    );
    assert_eq!(view["offers"], json!([]), "{view}");
    let excluded = view["excluded"].as_array().expect("the exclusions");
    assert_eq!(excluded.len(), 1, "{view}");
    assert_eq!(excluded[0]["recipe"], "task-work");
    let reason = excluded[0]["reason"].as_str().unwrap_or_default();
    assert!(reason.contains("carries no role"), "{reason}");
    assert!(reason.contains("`author`"), "{reason}");

    // The prose form says the same, under the same "nothing matches this
    // matter" a reader already looks under — neither reading knows something
    // the other does not.
    world
        .ephor()
        .args(["work", "offers", "--item", "rhei:window.1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("nothing matches this matter"))
        .stdout(predicate::str::contains("task-work"))
        .stdout(predicate::str::contains("carries no role"));
}
