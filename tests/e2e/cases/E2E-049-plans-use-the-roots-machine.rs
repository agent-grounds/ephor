//! E2E-049-plans-use-the-roots-machine: fresh tickets rely on the neighboring machine.
//!
//! A project keeps a pending task in its checkout and hands it over with a
//! recipe. Dispatch and sync create fresh plans without the deprecated header
//! declaration (§FS-005-dispatch.6). The root receives the shipped machine or
//! keeps the user's existing one, which still decides where a ticket may start.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use predicates::prelude::*;
use serde_json::json;

use support::*;

const TASK: &str = "# Rhei: the retry window\n\n## Tasks\n\n\
### Task 1: Widen the retry window\n**State:** pending\n\n\
The window resets per attempt, which is not what the docs say.\n";

const CUSTOM: &str = "name: custom\nversion: 1\nstates:\n  triage:\n  shipped:\n    final: true\n";

fn configure(world: &World, root: &Path, state: &str) {
    world.configure(json!({
        "projects": { PROJECT: { "work": {
            "root": root,
            "recipes": [{ "id": "task-work", "icon": "T",
                "description": "work a task", "state": state,
                "needs_checkout": false, "when": { "sources": ["rhei"] },
                "brief": "Work the task." }]
        } } }
    }));
}

fn watching() -> (World, PathBuf) {
    let world = World::new();
    world.file("panta/window-retry.rhei.md", TASK);
    let root = world.path().join("work");
    configure(&world, &root, "fix");
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert!(world.has_matter("rhei:window-retry.1"));
    assert!(!root.exists(), "the work root must begin fresh");
    (world, root)
}

fn plan_in(root: &Path) -> PathBuf {
    let plans: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(".rhei.md"))
        .collect();
    assert_eq!(plans.len(), 1, "one plan must be laid: {plans:?}");
    plans[0].clone()
}

fn machine(root: &Path) -> String {
    let yaml = fs::read_to_string(root.join("states.yaml")).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
    doc["name"].as_str().unwrap().to_string()
}

fn dispatch(world: &World) {
    world
        .ephor()
        .args(["work", "dispatch", "--project", PROJECT])
        .assert()
        .success()
        .stdout(predicate::str::contains("1 ticket(s) opened"));
}

fn no_declaration(root: &Path) {
    let text = fs::read_to_string(plan_in(root)).unwrap();
    assert!(text.contains("### Task task-work-"), "{text}");
    assert!(
        !text.lines().any(|line| line.starts_with("**States:**")),
        "fresh plan must omit **States:**; neighboring states.yaml selects {}\n{text}",
        machine(root)
    );
    assert!(
        text.starts_with("# Rhei: Widen the retry window\n\n---\n"),
        "{text}"
    );
}

#[test]
fn dispatch_writes_no_declaration_in_a_fresh_default_root() {
    let (world, root) = watching();
    dispatch(&world);
    assert_eq!(machine(&root), "ephor-work");
    no_declaration(&root);
}

#[test]
fn dispatch_writes_no_declaration_beside_a_custom_machine() {
    let (world, root) = watching();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("states.yaml"), CUSTOM).unwrap();
    configure(&world, &root, "triage");
    dispatch(&world);
    assert_eq!(machine(&root), "custom");
    assert_eq!(
        fs::read_to_string(root.join("states.yaml")).unwrap(),
        CUSTOM
    );
    no_declaration(&root);
}

#[test]
fn sync_writes_no_declaration_when_reopening_into_a_fresh_root() {
    let (world, root) = watching();
    dispatch(&world);
    assert!(plan_in(&root).is_file());
    let fresh = world.path().join("new-work");
    configure(&world, &fresh, "fix");
    world.file(
        "panta/window-retry.rhei.md",
        &TASK.replace("Widen", "Extend"),
    );
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert!(!fresh.exists());
    world
        .ephor()
        .args(["work", "sync", "--project", PROJECT])
        .assert()
        .success();
    assert_eq!(machine(&fresh), "ephor-work");
    // This changed title demonstrates that sync created the new plan.
    let text = fs::read_to_string(plan_in(&fresh)).unwrap();
    assert!(
        text.starts_with("# Rhei: Extend the retry window\n"),
        "{text}"
    );
    assert!(
        !text.lines().any(|line| line.starts_with("**States:**")),
        "fresh sync plan must omit **States:**\n{text}"
    );
    assert!(
        text.starts_with("# Rhei: Extend the retry window\n\n---\n"),
        "{text}"
    );
}

/// Passing controls: removing a header must leave root ownership and state
/// validation intact (§FS-005-dispatch.6).
#[test]
fn default_and_custom_root_machines_still_govern_ticket_states() {
    let (world, root) = watching();
    dispatch(&world);
    assert_eq!(machine(&root), "ephor-work");
    assert!(fs::read_to_string(plan_in(&root))
        .unwrap()
        .contains("**State:** fix"));

    let (world, root) = watching();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("states.yaml"), CUSTOM).unwrap();
    configure(&world, &root, "triage");
    dispatch(&world);
    assert_eq!(machine(&root), "custom");
    let before = fs::read_to_string(plan_in(&root)).unwrap();
    assert!(before.contains("**State:** triage"), "{before}");
    configure(&world, &root, "fix");
    world
        .ephor()
        .args(["work", "dispatch", "--project", PROJECT, "--again"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "0 ticket(s) opened, 1 item(s) could not be",
        ))
        .stderr(predicate::str::contains("fix"))
        .stderr(predicate::str::contains("custom"));
    assert_eq!(fs::read_to_string(plan_in(&root)).unwrap(), before);
    assert_eq!(
        fs::read_to_string(root.join("states.yaml")).unwrap(),
        CUSTOM
    );
    println!("default root selects ephor-work/fix; custom root retains custom/triage and refuses fix without writing");
}
