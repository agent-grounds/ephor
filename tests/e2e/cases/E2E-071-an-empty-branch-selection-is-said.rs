//! An accepted scope with no managed branch entries is an outcome a person
//! can read (§FS-011-command-line.9.3), even though successful writes are quiet.
//! Port of ephor#265's intake reproducer: a worktree project declares no
//! branches while its main has fetched one upstream commit it has not merged.

#[path = "../support.rs"]
mod support;

use std::path::Path;
use std::process::Output;

use serde_json::json;
use support::*;

fn git(dir: &Path, args: &[&str]) -> String {
    let output = git_command()
        .arg("-C")
        .arg(dir)
        .args(["-c", "init.defaultBranch=main"])
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.com",
        ])
        .args(args)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout).expect("git stdout is UTF-8")
}

fn fixture() -> World {
    let world = World::new();
    world.register(json!({
        "clone_mode": "worktree",
        "branch_root_template": "{project_root}/{branch}",
        "branches": []
    }));
    world.organize("fixture-org", "Fixture Org");
    git(world.path(), &["init", "--bare", "origin.git"]);
    git(world.path(), &["clone", "origin.git", "seed"]);
    let seed = world.path().join("seed");
    git(&seed, &["commit", "--allow-empty", "-m", "one"]);
    git(&seed, &["push", "origin", "HEAD:main"]);
    git(
        &world.forest(),
        &[
            "clone",
            world.path().join("origin.git").to_str().unwrap(),
            "main",
        ],
    );
    git(&seed, &["commit", "--allow-empty", "-m", "two"]);
    git(&seed, &["push", "origin", "HEAD:main"]);
    git(&world.forest().join("main"), &["fetch", "origin"]);
    assert_behind(&world);

    // Refusing this fixture must never count as reporting an empty selection.
    let validation = run(&world, &["validate", "--schema-only"]);
    assert_eq!(validation.status.code(), Some(0), "{validation:?}");
    world
}

fn assert_behind(world: &World) {
    assert_eq!(
        git(
            &world.forest().join("main"),
            &["rev-list", "--count", "HEAD..origin/main"]
        )
        .trim(),
        "1",
        "main remains one fetched commit behind; it is not an implicit entry"
    );
}

fn run(world: &World, args: &[&str]) -> Output {
    world
        .ephor()
        .current_dir(world.path())
        .args(args)
        .output()
        .expect("ephor runs")
}

fn accepted(world: &World, args: &[&str]) -> Output {
    let output = run(world, args);
    assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
    assert_behind(world);
    output
}

fn assert_narration(command: &str, output: &Output) -> String {
    // Capture the streams separately, but allow the human note on either one.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = format!("{stdout}\n{stderr}").to_lowercase();
    let zero = ["0 ", "zero ", "no branch", "no managed"]
        .iter()
        .any(|phrase| text.contains(phrase));
    assert!(
        text.contains(command) && zero && text.contains("branch") && text.contains("select"),
        "{command}: expected a human explanation naming the command and zero selected managed branch entries; stdout={stdout:?}; stderr={stderr:?}"
    );
    assert!(
        text.contains("main")
            && ["implicit", "explicit", "declared"]
                .iter()
                .any(|word| text.contains(word)),
        "{command}: explain why project main is not implicitly managed: {text}"
    );
    assert!(
        text.contains("--all")
            && text.contains("project")
            && (text.contains("not") || text.contains("only")),
        "{command}: explain that --all widens branch entries, not projects: {text}"
    );
    text
}

fn empty_update(all: bool) {
    let world = fixture();
    let mut args = vec!["update", "--org", "fixture-org", "--skip-agents"];
    if all {
        args.push("--all");
    }
    let output = accepted(&world, &args);
    assert_narration("update", &output);
}

#[test]
fn empty_selection_update_names_zero_without_all() {
    empty_update(false);
}

#[test]
fn empty_selection_update_names_zero_with_all() {
    empty_update(true);
}

fn empty_agents(all: bool) {
    let world = fixture();
    let mut args = vec!["ensure-agents", "--org", "fixture-org"];
    if all {
        args.push("--all");
    }
    let output = accepted(&world, &args);
    assert!(!world.forest().join("main/AGENTS.md").exists());
    assert_narration("ensure-agents", &output);
}

#[test]
fn empty_selection_ensure_agents_names_zero_without_all() {
    empty_agents(false);
}

#[test]
fn empty_selection_ensure_agents_names_zero_with_all() {
    empty_agents(true);
}

fn declare_main(world: &World, active: bool) {
    let mut registry = world.registry_doc();
    registry["projects"][0]["branches"] = json!([
        {"id": "demo-main", "branch": "main", "active": active, "tags": ["kept"]}
    ]);
    write_json(&world.registry_path(), &registry);
}

fn excluded_entry(active: bool, filters: &[&str]) {
    let world = fixture();
    declare_main(&world, active);
    for command in ["update", "ensure-agents"] {
        let mut args = vec![command, "--org", "fixture-org"];
        if command == "update" {
            args.push("--skip-agents");
        }
        args.extend_from_slice(filters);
        // Establish the accepted zero result independently of the prose check.
        let mut json_args = args.clone();
        json_args.push("--json");
        let json_output = accepted(&world, &json_args);
        assert_eq!(json_of(&json_output)["count"], 0);
        let output = accepted(&world, &args);
        let text = assert_narration(command, &output);
        for false_claim in [
            "none declares",
            "no project declares",
            "no projects declare",
            "no branches declared",
            "no branch entries declared",
            "no branch declarations",
        ] {
            assert!(!text.contains(false_claim), "entries exist: {text}");
        }
    }
}

#[test]
fn empty_selection_inactive_entries_are_not_missing_declarations() {
    excluded_entry(false, &[]);
}

#[test]
fn empty_selection_filtered_entries_are_not_missing_declarations() {
    excluded_entry(true, &["--all", "--tag", "excluded"]);
}

// Preservation checks deliberately pass before implementation; they constrain
// the remedy, rather than claiming to reproduce the missing narration.
#[test]
fn json_zero_outcomes_keep_their_exact_shapes_and_streams() {
    let world = fixture();
    for (command, field) in [("update", "updated"), ("ensure-agents", "written")] {
        for all in [false, true] {
            let mut args = vec![command, "--org", "fixture-org", "--json"];
            if command == "update" {
                args.push("--skip-agents");
            }
            if all {
                args.push("--all");
            }
            let output = accepted(&world, &args);
            assert_eq!(json_of(&output), json!({"count": 0, field: []}));
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
}

#[test]
fn explicit_id_refusals_keep_their_reason_and_exit_code() {
    let world = fixture();
    for command in ["update", "ensure-agents"] {
        for id in ["missing", "demo"] {
            let output = run(&world, &[command, "--workspace", id]);
            assert_eq!(output.status.code(), Some(2), "{output:?}");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(&format!("Unknown projects or workspaces: {id}")),
                "{stderr}"
            );
            assert!(output.stdout.is_empty(), "{output:?}");
        }
    }
    assert_behind(&world);
}

#[test]
fn nonempty_successful_writes_remain_quiet() {
    let world = fixture();
    declare_main(&world, true);
    // Git's own fetch/checkout/pull progress is already visible. A skipped
    // repository isolates the command's prose outcome from that progress.
    let mut registry = world.registry_doc();
    registry["project_types"][0]["repos"][0]["update_mode"] = json!("skip");
    write_json(&world.registry_path(), &registry);
    let output = run(&world, &["update", "--org", "fixture-org", "--skip-agents"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "{output:?}"
    );
    assert_behind(&world);
    let json_output = accepted(
        &world,
        &["update", "--org", "fixture-org", "--skip-agents", "--json"],
    );
    assert_eq!(json_of(&json_output)["count"], 1);
    let output = run(&world, &["ensure-agents", "--org", "fixture-org"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        output.stdout.is_empty() && output.stderr.is_empty(),
        "{output:?}"
    );
    assert!(world.forest().join("main/AGENTS.md").exists());
}
