//! E2E-036-the-minting-stops-at-one-generation: a recipe over a project's own
//! tasks that mints a checkout each cannot feed itself.
//!
//! The scenario is the one #120 shipped `{id_slug}` for, run twice. A project
//! keeps its work in its own checkout, and a recipe selects that source and
//! says each task belongs in a workspace of its own —
//! `sources: ["rhei"]`, `needs_checkout: true`, `"branch": "task/{id_slug}"`
//! (§FS-005-dispatch.25). Every piece of that is legal and stays legal.
//!
//! What closed over it was the reading half. The workspace the mint makes gets
//! a task store of its own, the dispatch writes its plan inside that store, and
//! the next read offered those plans back as fresh task matters of the same
//! project — matched by the same recipe, minting again, each generation twice
//! the size of the last. The ledger could not stop it: it keys work per item,
//! and every turn's items were new.
//!
//! So the bound is at the seam rather than in the recipe: a plan ephor caused
//! to exist is not yielded as a matter (§FS-006-project-interface.7). This case
//! holds ephor to one generation and to what that costs — the two tasks are
//! still matters, the two plans the dispatch wrote are not, a second sweep is
//! offered nothing, and ephor's own plans are still on disk and still watched
//! where work is watched (§FS-005-dispatch.15).

#[path = "../support.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;

use support::*;

/// The project's own task store, as the project itself writes one: two open
/// tasks, no state line ephor put there, and a file that exists whether or not
/// ephor ever runs (§FS-006-project-interface.7).
const TWO_TASKS: &str = "# Rhei: the retry window\n\n\
## Tasks\n\n\
### Task 1: Widen the retry window\n**State:** pending\n\n\
The window resets per attempt, which is not what the docs say.\n\n\
### Task 2: Shorten the reset\n**State:** pending\n\n\
A second, unrelated task.\n";

/// The two matters the store holds, before anything is dispatched.
const FIRST_TASK: &str = "rhei:window.1";
const SECOND_TASK: &str = "rhei:window.2";

/// A runtime that answers the two verbs this scenario asks of it and carries no
/// workflow: the recipe hands work over as a ticket, so nothing here renders.
/// Asked to make a store it does nothing, and ephor writes the one it can.
const ACME_RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
verb="$1"; shift
case "$verb" in
  templates) printf '[]' ;;
  init) exit 0 ;;
  *) echo "unknown verb $verb" >&2; exit 1 ;;
esac
"#;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

/// The project before anything is dispatched: an origin, the main branch
/// checked out at `<root>/main`, and the store in that checkout. Every other
/// branch is a workspace beside it, and none exists yet.
fn project_with_its_own_tasks(world: &World) {
    let origin = world.path().join("origin");
    std::fs::create_dir_all(&origin).expect("the remote");
    git(&origin, &["init", "-q", "--initial-branch=main"]);
    std::fs::write(origin.join("README.md"), "the project\n").expect("a file");
    git(&origin, &["add", "README.md"]);
    git(&origin, &["commit", "-q", "-m", "the project"]);

    let main = world.forest().join("main");
    std::fs::create_dir_all(main.parent().expect("a parent")).expect("the project root");
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(&origin)
        .arg(&main)
        .status()
        .expect("git clones");
    assert!(status.success());
    git(&main, &["config", "user.email", "t@example.com"]);
    git(&main, &["config", "user.name", "t"]);
    world.file("main/panta/window.rhei.md", TWO_TASKS);
}

/// A world watching that project, with one recipe over its own tasks that says
/// each of them belongs in a checkout of its own.
fn watching_its_own_tasks() -> World {
    let world = World::new();
    project_with_its_own_tasks(&world);
    world.stub("acme-runtime", ACME_RUNTIME);

    world.configure(json!({
        "projects": { PROJECT: {
            "providers": [],
            "work": { "recipes": [ {
                "id": "task-work",
                "icon": "⛬",
                "description": "work a task in its own checkout",
                "when": { "sources": ["rhei"] },
                "needs_checkout": true,
                "branch": "task/{id_slug}",
                // A state ephor's own machine declares, so nothing is refused
                // before the branch is ever rendered.
                "state": "fix",
                "brief": "Work {title}."
            } ] }
        } },
        "work": { "runner": "acme-runtime" }
    }));
    world.register(json!({
        "branches": [],
        "branch_root_template": "{project_root}/{branch}"
    }));
    world
}

/// Every matter the tasks seam yields for this project, by id, after a fresh
/// read of what is on disk.
fn tasks_after_a_read(world: &World) -> Vec<String> {
    world.ephor().args(["refresh", PROJECT]).assert().success();
    let mut ids: Vec<String> = world
        .matters()
        .iter()
        .filter(|matter| matter["source"] == json!("rhei"))
        .map(|matter| {
            matter["key"]
                .as_str()
                .expect("each matter has a key")
                .to_string()
        })
        .collect();
    ids.sort();
    ids
}

/// Every plan file under the project on disk, relative to the forest root, so
/// a case can say what ephor wrote as well as what it read back.
fn plans_on_disk(world: &World) -> Vec<String> {
    fn visit(root: &Path, dir: &Path, found: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.filter_map(|entry| entry.ok()) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == ".git" {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, found);
            } else if name.ends_with(".rhei.md") {
                found.push(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    let mut found = Vec::new();
    visit(&world.forest(), &world.forest(), &mut found);
    found.sort();
    found
}

/// What one sweep would open, as its own report says it: the matters it names,
/// so a case can assert on the offer rather than on the prose.
fn would_open(world: &World) -> Vec<String> {
    let said = world
        .ephor()
        .args([
            "work",
            "dispatch",
            "--project",
            PROJECT,
            "--dry-run",
            "--json",
        ])
        .assert()
        .success();
    let report = json_of(said.get_output());
    let mut items: Vec<String> = report["items"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter(|row| {
                    row["outcome"]
                        .as_str()
                        .is_some_and(|outcome| outcome.starts_with("would-open"))
                })
                .map(|row| row["item"].as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default();
    items.sort();
    items
}

/// The two workspaces the two tasks mint, by the ids those tasks read down to
/// (§FS-005-dispatch.2).
fn minted(world: &World) -> (PathBuf, PathBuf) {
    (
        world.forest().join("task/rhei-window-1-d8a9c768"),
        world.forest().join("task/rhei-window-2-dba9cc21"),
    )
}

/// The ticket's own case, end to end. One dispatch mints one tree per task and
/// writes its plan inside the store that tree got — and the next read is
/// offered the two tasks it was offered before, and nothing else. The minting
/// stops at one generation (§FS-005-dispatch.25,
/// §FS-006-project-interface.7).
#[test]
fn what_a_dispatch_wrote_is_not_offered_back_as_work() {
    let world = watching_its_own_tasks();

    // Turn one: the store holds two open tasks, and both are matters.
    assert_eq!(
        tasks_after_a_read(&world),
        vec![FIRST_TASK.to_string(), SECOND_TASK.to_string()]
    );
    assert_eq!(
        would_open(&world),
        vec![FIRST_TASK.to_string(), SECOND_TASK.to_string()]
    );

    // The sweep, for real: two worktrees, each with a store of its own and the
    // plan about its own task inside it.
    world
        .ephor()
        .args(["work", "dispatch", "--project", PROJECT])
        .assert()
        .success();
    let (first, second) = minted(&world);
    for workspace in [&first, &second] {
        assert!(
            workspace.join("panta/states.yaml").is_file(),
            "{} got no store",
            workspace.display()
        );
    }
    // Four plans on disk: the project's own, and the two ephor wrote inside the
    // two stores its own mint made.
    assert_eq!(
        plans_on_disk(&world),
        vec![
            "main/panta/window.rhei.md".to_string(),
            "task/rhei-window-1-d8a9c768/panta/rhei-window-1-d8a9c768.rhei.md".to_string(),
            "task/rhei-window-2-dba9cc21/panta/rhei-window-2-dba9cc21.rhei.md".to_string(),
        ],
        "the dispatch did not write what this case is about"
    );

    // Turn two, and the whole point: the same two tasks, and not one matter
    // more. The plans ephor wrote are its own filing about work it was told to
    // do, never a second piece of the project's work
    // (§FS-006-project-interface.7).
    assert_eq!(
        tasks_after_a_read(&world),
        vec![FIRST_TASK.to_string(), SECOND_TASK.to_string()],
        "a plan ephor wrote came back as a task of the project"
    );

    // And the sweep that follows is offered nothing: the two tasks are held by
    // the ledger, and there is no third and no fourth matter for the recipe to
    // match. Nothing is minted, so the forest stays at one generation.
    assert_eq!(would_open(&world), Vec::<String>::new());
    world
        .ephor()
        .args(["work", "dispatch", "--project", PROJECT])
        .assert()
        .success();
    assert_eq!(
        plans_on_disk(&world),
        vec![
            "main/panta/window.rhei.md".to_string(),
            "task/rhei-window-1-d8a9c768/panta/rhei-window-1-d8a9c768.rhei.md".to_string(),
            "task/rhei-window-2-dba9cc21/panta/rhei-window-2-dba9cc21.rhei.md".to_string(),
        ],
        "a second generation was minted"
    );
}

/// What the bound costs, and what it does not. Ephor's own plans leave the
/// Tasks row and stay exactly as visible everywhere work is watched: every plan
/// a work root holds is still on the work screen whoever wrote it
/// (§FS-005-dispatch.15). One directory, two readers.
#[test]
fn the_work_ephor_wrote_is_still_watched_where_work_is_watched() {
    let world = watching_its_own_tasks();
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
        .ephor()
        .args(["work", "dispatch", "--project", PROJECT])
        .assert()
        .success();

    let said = world
        .ephor()
        .args(["work", "list", "--project", PROJECT, "--json"])
        .assert()
        .success();
    let listed = json_of(said.get_output()).to_string();
    for plan in ["rhei-window-1", "rhei-window-2"] {
        assert!(
            listed.contains(plan),
            "the work screen lost the plan {plan}: {listed}"
        );
    }
}
