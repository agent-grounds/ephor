//! Standing scheduling and screen startup (§AR-004-forest.2,
//! §AR-009-surfaces.2), exercised through the user's commands and screen
//! (§FS-011-command-line.2). Git runs in isolated child environments; no test
//! changes the process-wide PATH or configuration.

mod common;
mod standing_fixture;

use serde_json::{json, Value};
use standing_fixture::*;
use std::fs;
use std::path::Path;
use std::process::Command;

const BRANCHES: [&str; 4] = ["main", "feature-1", "feature-2", "feature-3"];

fn git(dir: &Path, args: &[&str]) {
    let output = common::git_command()
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}: {output:?}");
}

fn fixture() -> World {
    let world = World::new();
    world.register(json!({
        "branch_root_template": "{project_root}/{branch}",
        "branches": BRANCHES.into_iter().chain(["missing"]).map(|branch| {
            json!({"id": branch, "branch": branch, "active": true})
        }).collect::<Vec<_>>()
    }));
    for branch in BRANCHES {
        let repo = world.forest().join(branch);
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "--initial-branch=main"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "base"]);
        git(&repo, &["branch", "base"]);
        for _ in 0..2 {
            git(
                &repo,
                &["commit", "-q", "--allow-empty", "-m", "main moves"],
            );
        }
        git(&repo, &["remote", "add", "origin", "./unreachable-remote"]);
        git(&repo, &["update-ref", "refs/remotes/origin/main", "main"]);
        if branch != "main" {
            git(&repo, &["checkout", "-q", "-b", branch, "base"]);
            if branch != "feature-3" {
                let published = format!("refs/remotes/origin/{branch}");
                let revision = if branch == "feature-1" {
                    "main"
                } else {
                    "base"
                };
                git(&repo, &["update-ref", &published, revision]);
            }
        }
    }
    world.configure(json!({"projects": {PROJECT: {"providers": [{
        "provider": "custom-status", "command": "cp \"$HOME/answer.json\" \"$EPHOR_ANSWER\"",
        "format": "answer"
    }]}}}));
    answer(&world, "Startup223");
    // Populate the real cache through the normal provider seam before logging.
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert_eq!(
        world.matters().len(),
        1,
        "the cached fixture must contain its matter"
    );
    let real_git = Command::new("which").arg("git").output().unwrap();
    assert!(real_git.status.success());
    fs::write(world.path().join("real-git"), real_git.stdout).unwrap();
    world.stub("git", include_str!("standing_git.py"));
    world
}

fn answer(world: &World, title: &str) {
    fs::write(
        world.path().join("answer.json"),
        json!({
            "v": 1, "matters": [{"key": "startup", "kind": "status", "title": title,
            "state": "waiting", "terminal": false, "time": "2026-10-20T00:00:00Z"}]
        })
        .to_string(),
    )
    .unwrap();
}

fn events(world: &World) -> Vec<Value> {
    fs::read_to_string(world.path().join("git-events.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn measurements(events: &[Value]) -> Vec<&Value> {
    assert!(
        !events.iter().any(|event| event["args"]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "fetch")),
        "a reading fetched: {events:#?}"
    );
    events
        .iter()
        .filter(|event| event["probe"] == true)
        .collect()
}

fn save_artifacts(world: &World, name: &str) {
    if let Some(root) = std::env::var_os("EPHOR_223_ARTIFACTS") {
        let directory = Path::new(&root).join(name);
        fs::create_dir_all(&directory).unwrap();
        for file in [
            "git-events.jsonl",
            "terminal.log",
            "workspaces.json",
            "status.json",
        ] {
            let source = world.path().join(file);
            if source.exists() {
                fs::copy(source, directory.join(file)).unwrap();
            }
        }
    }
}

fn assert_readings(rows: &[Value]) {
    assert_eq!(
        rows.iter()
            .map(|row| row["branch"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["main", "feature-1", "feature-2", "feature-3", "missing"]
    );
    for row in &rows[..4] {
        assert_eq!(
            row["behind"]["behind"],
            if row["branch"] == "main" { 0 } else { 2 }
        );
    }
    assert_eq!(rows[1]["behind_upstream"]["behind"], 2);
    assert_eq!(rows[1]["published"], "origin/feature-1");
    assert_eq!(rows[2]["behind_upstream"]["behind"], 0);
    assert!(
        rows[3].get("behind_upstream").is_none(),
        "unpublished: {:?}",
        rows[3]
    );
    assert_eq!(rows[4]["checked_out"], false);
    assert!(rows[4].get("behind").is_none());
}

#[test]
fn issue_223_independent_standing_probes_overlap() {
    let world = fixture();
    let output = world
        .ephor()
        .args(["branches", PROJECT, "--json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_readings(&rows);
    let log = events(&world);
    save_artifacts(&world, "overlap");
    let probes = measurements(&log);
    assert_eq!(probes.len(), 8, "one start/end per checkout: {probes:#?}");
    let mut active = 0;
    let mut peak = 0;
    for event in probes {
        if event["event"] == "start" {
            active += 1;
        } else {
            active -= 1;
        }
        peak = peak.max(active);
    }
    assert_eq!(active, 0);
    assert!(peak >= 2, "independent checkout standing probes never overlapped (peak={peak}); expected at least two measurements in flight");
}

#[test]
fn issue_223_first_screen_uses_one_sweep_and_refresh_still_reloads() {
    let world = fixture();
    let mut driver = Command::new("python3");
    driver.arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/integration/standing_tui.py"));
    driver
        .arg(assert_cmd::cargo::cargo_bin("ephor"))
        .arg(world.path());
    // Apply the same isolated child environment the CLI cases use, removals
    // included, so the `ephor` the driver starts inherits no `GIT_*`
    // (§REQ-003-test-isolation.3.3).
    for (key, value) in world.ephor_raw().get_envs() {
        match value {
            Some(value) => driver.env(key, value),
            None => driver.env_remove(key),
        };
    }
    let output = driver.output().expect("the PTY driver runs");
    save_artifacts(&world, "startup");
    assert!(output.status.success(), "PTY driver failed: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    println!("PTY observations: {report}");
    assert_eq!(report["first_frame"], true, "cached matter must be drawn");
    assert_eq!(
        report["refreshed_frame"], true,
        "refresh must rebuild the view"
    );
    assert_eq!(
        report["refresh_sweeps"],
        json!([1, 1, 1, 1]),
        "later refresh must still measure every checkout"
    );
    let log = events(&world);
    measurements(&log);
    assert_eq!(report["startup_sweeps"], json!([1, 1, 1, 1]),
        "first drawn cached screen repeated the standing sweep; expected one measurement per checkout before presentation");
}
