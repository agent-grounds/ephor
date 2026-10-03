//! E2E-045-a-registry-refused-whole: a registry written by hand from the schema
//! learns everything it still owes from one run, not one field per run.
//!
//! The scenario is §FS-006-project-interface.11.1 end to end, on the registry
//! agent-grounds/ephor#158 reports. A person building the smallest site they
//! can writes every top-level section and leaves out five required fields: the
//! organization's `name`, the project type's `layout` and its agents'
//! `template` and `structure_intro`, and the project's `display_name`. One
//! schema pass sees all five. Ephor named the first and dropped the rest, so
//! reaching a registry it accepts took five failing runs.
//!
//! What this case holds ephor to, on every surface that loads the registry —
//! `validate`, `validate --schema-only`, `validate --json` and `refresh`: the
//! one refusal names all five violations at their instance paths, counts them,
//! and points at `config/workspaces.example.json`; it is still a refusal, with
//! the exit code it had, and under `--json` the list rides in `says`.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

/// The issue's hand-written registry, rooted in this world rather than in a
/// scratch directory of the machine it was first written on.
fn hand_written(world: &World) -> Value {
    let root = world.path().join("site");
    json!({
        "organizations": [{"id": "demo", "root": root.to_string_lossy()}],
        "hook_sets": [],
        "project_types": [{"id": "repo",
            "repos": [{"id": "repo", "path": ".", "role": "the project",
                       "required": true, "update_mode": "skip"}],
            "agents": {"summary_template": "A demo project."}}],
        "projects": [{"id": "proj", "organization": "demo", "type": "repo",
            "root": root.join("proj").to_string_lossy(), "main_branch": "main",
            "clone_mode": "worktree", "branch_root_template": "{project_root}/{branch}"}]
    })
}

/// Every violation one schema pass sees: the place, and the field it owes.
const OWED: [(&str, &str); 5] = [
    ("/organizations/0", "name"),
    ("/project_types/0", "layout"),
    ("/project_types/0/agents", "template"),
    ("/project_types/0/agents", "structure_intro"),
    ("/projects/0", "display_name"),
];

/// The refusal names every owed field at its path, says how many there are,
/// and points at the shipped example.
fn assert_refused_whole(surface: &str, says: &str) {
    let missing: Vec<String> = OWED
        .iter()
        .filter(|(path, field)| {
            !says.lines().any(|line| {
                line.contains(path) && line.contains(&format!("\"{field}\" is a required property"))
            })
        })
        .map(|(path, field)| format!("{path}: {field}"))
        .collect();
    assert!(
        missing.is_empty(),
        "{surface}: one refusal should name all {} violations, each on a line with its path; \
         it does not name {missing:?}:\n{says}",
        OWED.len()
    );
    assert!(
        says.contains("5 violations"),
        "{surface}: the refusal should count what it names (\"5 violations\"):\n{says}"
    );
    assert!(
        says.contains("config/workspaces.example.json"),
        "{surface}: the refusal should name config/workspaces.example.json as a registry \
         to start from:\n{says}"
    );
}

fn stderr_of(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn validate_names_every_violation_in_one_run() {
    let world = World::new();
    write_json(&world.registry_path(), &hand_written(&world));

    for args in [&["validate"][..], &["validate", "--schema-only"][..]] {
        let output = world.ephor().args(args).output().expect("ephor runs");
        assert!(
            !output.status.success(),
            "{args:?} must still refuse the registry"
        );
        assert_refused_whole(&format!("ephor {}", args.join(" ")), &stderr_of(&output));
    }
}

#[test]
fn validate_json_carries_the_list_in_says() {
    let world = World::new();
    write_json(&world.registry_path(), &hand_written(&world));

    let output = world
        .ephor()
        .args(["validate", "--json"])
        .output()
        .expect("ephor runs");
    assert!(
        !output.status.success(),
        "--json must still refuse the registry"
    );
    let answer = json_of(&output);
    assert_eq!(answer["ok"], json!(false), "{answer}");
    let says = answer["says"]
        .as_str()
        .unwrap_or_else(|| panic!("a `says` string: {answer}"));
    assert_refused_whole("ephor validate --json", says);
}

#[test]
fn refresh_names_every_violation_in_one_run() {
    let world = World::new();
    write_json(&world.registry_path(), &hand_written(&world));
    world.configure(json!({ "projects": {} }));

    let output = world.ephor().arg("refresh").output().expect("ephor runs");
    assert!(
        !output.status.success(),
        "refresh must still refuse the registry"
    );
    assert_refused_whole("ephor refresh", &stderr_of(&output));
}
