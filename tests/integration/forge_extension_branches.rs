//! A watched branch's gate on a forge ephor never heard of: the extension
//! answers `branch-gates` with each branch's verdicts, workflow by workflow,
//! and the decision that a red one needs the reader is ephor's
//! (§FS-001-forge-interface.1, §FS-001-forge-interface.2,
//! §FS-001-forge-interface.3).

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use super::common::{ephor_cmd, make_executable, tempdir};
use super::{extension_env, write_fixture};

/// An extension whose `app` repository has three watched branches: `main` red
/// in CI and green in Docs, `release/2` green, `hotfix/2` with no verdict yet.
/// Every subcommand is logged with the request it was handed, one JSON line per
/// call, so a test reads what crossed the seam rather than inferring it.
/// `$BRANCH_GATE` turns the capability off, for the forge that does not watch
/// branches.
const BRANCH_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
request="$(cat)"
jq -cn --arg call "$1" --argjson request "$request" '{call: $call, request: $request}' \
  >> "$FORGE_CALLS"
case "${1:?subcommand}" in
  capabilities)
    printf '{"pull_requests":true,"branch_gate":%s,"failures":true,"restart":true}' \
      "${BRANCH_GATE:-true}"
    ;;
  branch-gates)
    printf '%s' '[
      { "repo": "app", "branch": "main",
        "url": "https://demo.example/app/tree/main",
        "head_sha": "aa4484657d3fb2c3e8f459ba715f2d0df5d2e3f6",
        "updated_at": "2026-10-14T15:02:00Z",
        "workflows": [
          { "id": "11", "name": "CI", "run": "901",
            "url": "https://demo.example/app/runs/901", "verdict": "red" },
          { "id": "12", "name": "Docs", "run": "902",
            "url": "https://demo.example/app/runs/902", "verdict": "green" } ] },
      { "repo": "app", "branch": "release/2",
        "updated_at": "2026-10-14T14:00:00Z",
        "workflows": [ { "id": "11", "name": "CI", "run": "880", "verdict": "green" } ] },
      { "repo": "app", "branch": "hotfix/2",
        "updated_at": "2026-10-14T13:00:00Z",
        "workflows": [] }
    ]'
    ;;
  failures)
    printf '%s' '[ { "job": "cargo test (ubuntu-latest)",
                     "url": "https://demo.example/app/runs/901/jobs/1",
                     "trace": "retry_window_test: expected 3 attempts, saw 1" } ]'
    ;;
  *) printf '[]' ;;
esac
"#;

/// The fixture `forge_extension_test` refreshes, with `branches` named on its
/// one source and the extension above on PATH.
fn watching(tmp: &Path) -> String {
    write_fixture(tmp);
    let status = tmp.join("status.json");
    let mut config: Value = serde_json::from_str(&fs::read_to_string(&status).unwrap()).unwrap();
    config["projects"]["demo"]["providers"][0]["branches"] =
        json!(["main", "release/2", "hotfix/2"]);
    fs::write(&status, serde_json::to_string_pretty(&config).unwrap()).unwrap();
    let fake_bin = tmp.join("fakebin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("ephor-forge-demoforge"), BRANCH_FORGE);
    format!(
        "{}:{}",
        fake_bin.to_string_lossy(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn ephor(tmp: &Path, path: &str) -> assert_cmd::Command {
    let mut cmd = ephor_cmd();
    cmd.env("PATH", path)
        .env("FORGE_CALLS", tmp.join("forge.calls"));
    for (key, value) in extension_env(tmp) {
        cmd.env(key, value);
    }
    cmd
}

/// Every call the extension was handed, in order.
fn calls(tmp: &Path) -> Vec<Value> {
    fs::read_to_string(tmp.join("forge.calls"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn matters(tmp: &Path) -> Vec<Value> {
    let cache: Value =
        serde_json::from_str(&fs::read_to_string(tmp.join("state/ephor/feed/demo.json")).unwrap())
            .unwrap();
    cache["providers"]["demoforge"]["matters"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// A red branch is one row, and a green branch, or one with no verdict yet, is
/// none: the extension reports the verdicts and ephor decides which need the
/// reader, as it does for every forge (§FS-001-forge-interface.3).
#[test]
fn a_red_branch_on_an_extension_is_one_row_and_a_green_one_none() {
    let tmp = tempdir();
    let path = watching(tmp.path());
    ephor(tmp.path(), &path)
        .args(["refresh", "demo"])
        .assert()
        .success();

    let asked: Vec<Value> = calls(tmp.path())
        .into_iter()
        .filter(|call| call["call"] == "branch-gates")
        .collect();
    assert_eq!(asked.len(), 1, "asked once per refresh: {asked:#?}");
    assert_eq!(
        asked[0]["request"]["config"]["branches"],
        json!(["main", "release/2", "hotfix/2"]),
        "the source's branches reach the extension as configured"
    );

    let rows: Vec<Value> = matters(tmp.path())
        .into_iter()
        .filter(|matter| matter["kind"] == "ci")
        .collect();
    assert_eq!(rows.len(), 1, "only the red branch: {rows:#?}");
    let row = &rows[0];
    assert_eq!(row["key"], "demoforge:app@main");
    assert_eq!(row["needs_response"], true);
    assert_eq!(row["raw"]["branch"], "main");
    assert_eq!(row["raw"]["watched_branch"], true);
    let title = row["title"].as_str().unwrap_or_default();
    assert!(
        title.contains("CI"),
        "the row names the red workflow: {title}"
    );
    assert!(!title.contains("Docs"), "and not the green one: {title}");
}

/// A forge that does not declare the capability is never asked, whatever its
/// configuration says: the question is the declaration's, not the key's
/// (§FS-001-forge-interface.2). Passes today, and must keep passing.
#[test]
fn an_extension_that_does_not_watch_branches_is_never_asked_for_one() {
    let tmp = tempdir();
    let path = watching(tmp.path());
    ephor(tmp.path(), &path)
        .env("BRANCH_GATE", "false")
        .args(["refresh", "demo"])
        .assert()
        .success();
    let asked: Vec<String> = calls(tmp.path())
        .iter()
        .map(|call| call["call"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(asked.contains(&"capabilities".to_string()), "{asked:?}");
    assert!(
        !asked.contains(&"branch-gates".to_string()),
        "a forge without `branch_gate` is not asked for one: {asked:?}"
    );
    assert!(
        matters(tmp.path())
            .iter()
            .all(|matter| matter["kind"] != "ci"),
        "and no branch row comes from it"
    );
}

/// What failed on a watched branch is asked by its repository and branch, with
/// the red runs its row carries and no number, and either spelling of the
/// subject asks the same thing (§FS-011-command-line.6, §FS-004-quick-actions.4).
#[test]
fn what_failed_on_a_watched_branch_is_asked_by_its_branch() {
    let tmp = tempdir();
    let path = watching(tmp.path());
    ephor(tmp.path(), &path)
        .args(["refresh", "demo"])
        .assert()
        .success();

    for subject in [
        &[
            "--project",
            "demo",
            "--source",
            "demoforge",
            "--repo",
            "app",
            "--branch",
            "main",
        ][..],
        &["--item", "demoforge:app@main"][..],
    ] {
        fs::remove_file(tmp.path().join("forge.calls")).ok();
        let output = ephor(tmp.path(), &path)
            .arg("failures")
            .args(subject)
            .arg("--json")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{subject:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(answer["item"], "demoforge:app@main", "{subject:?}");
        assert_eq!(
            answer["failures"][0]["job"], "cargo test (ubuntu-latest)",
            "{subject:?}"
        );

        let asked: Vec<Value> = calls(tmp.path())
            .into_iter()
            .filter(|call| call["call"] == "failures")
            .collect();
        assert_eq!(asked.len(), 1, "{subject:?}: {asked:#?}");
        let request = &asked[0]["request"];
        assert_eq!(request["repo"], "app", "{subject:?}");
        assert_eq!(request["branch"], "main", "{subject:?}");
        assert_eq!(
            request["runs"],
            json!(["901"]),
            "{subject:?}: only the red run"
        );
        assert!(
            request.get("number").is_none(),
            "{subject:?}: a branch has no number: {request}"
        );
    }
}
