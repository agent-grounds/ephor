//! E2E-059-a-red-branch-is-a-row: a red `main` reaches the feed with no pull
//! request open, and leaves it on the first green run.
//!
//! The scenario is the one a hand sweep catches first: a push to `main` fails
//! its required job, nobody has a pull request open, and the feed reads *No
//! feed items* while the branch is red for hours. With `branches` named on the
//! `github-ci` source, the branch's gate is asked for
//! (§FS-001-forge-interface.1), and while it is red the feed holds one `ci` row
//! keyed by the repository and the branch rather than by a run
//! (§FS-007-matters.1). A second red run is the same row, and a green run of the
//! workflow that was red clears it (§FS-001-forge-interface.3). The row is
//! offered what every red gate is offered: what failed, restart what failed,
//! restart the whole gate (§FS-004-quick-actions.4, §FS-004-quick-actions.9).
//!
//! The forge is a stand-in for `gh` that serves GitHub's run listing from a
//! file the case rewrites between refreshes, the way new runs land between two
//! refreshes of a real watch.

#[path = "../support.rs"]
mod support;

use predicates::prelude::*;
use serde_json::{json, Value};

use support::*;

/// `gh`, as far as `github-ci` needs it here: no open pull request, and the
/// run listing and the jobs under a run from `runs.json` beside the stubs.
const GH: &str = r#"#!/usr/bin/env python3
import json, sys
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

fixture = json.loads((Path(__file__).resolve().parent.parent / 'runs.json').read_text())
args = sys.argv[1:]
if args[:1] == ['api']:
    path = next(arg for arg in args[1:] if 'actions/runs' in arg)
    url = urlsplit(path)
    query = {key: values[-1] for key, values in parse_qs(url.query).items()}
    for flag, value in zip(args, args[1:]):
        if flag in ('-f', '-F', '--field', '--raw-field'):
            key, _, text = value.partition('=')
            query[key] = text
    parts = url.path.strip('/').split('/')
    if parts[-1] == 'jobs':
        jobs = fixture['jobs'].get(parts[-2], [])
        print(json.dumps({'total_count': len(jobs), 'jobs': jobs}))
    else:
        runs = [run for run in fixture['runs']
                if 'branch' not in query or run['head_branch'] == query['branch']]
        print(json.dumps({'total_count': len(runs), 'workflow_runs': runs}))
else:
    print('[]')
"#;

const ROW: &str = "github-ci:acme/widget@main";
const SHA: &str = "aa4484657d3fb2c3e8f459ba715f2d0df5d2e3f6";

/// One completed push run of the CI workflow on `main`.
fn run(id: u64, number: u64, created: &str, conclusion: &str) -> Value {
    json!({
        "id": id, "name": "CI", "workflow_id": 11, "run_number": number, "run_attempt": 1,
        "event": "push", "status": "completed", "conclusion": conclusion,
        "head_branch": "main", "head_sha": SHA,
        "html_url": format!("https://github.com/acme/widget/actions/runs/{id}"),
        "created_at": created, "run_started_at": created, "updated_at": created,
        "head_repository": { "full_name": "acme/widget" },
        "repository": { "full_name": "acme/widget" }
    })
}

/// The runs on the forge now, newest first, and the jobs under the red ones.
fn on_the_forge(world: &World, runs: Value, red: &[u64]) {
    let mut jobs = serde_json::Map::new();
    for id in red {
        jobs.insert(
            id.to_string(),
            json!([
                { "name": "cargo test (ubuntu-latest)", "status": "completed", "conclusion": "failure" },
                { "name": "cargo fmt", "status": "completed", "conclusion": "success" }
            ]),
        );
    }
    write_json(
        &world.path().join("runs.json"),
        &json!({ "runs": runs, "jobs": jobs }),
    );
}

fn watching(world: &World) {
    world.stub("gh", GH);
    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "github-ci", "repos": ["acme/widget"], "branches": ["main"] }
        ] } }
    }));
}

fn ci_rows(world: &World) -> Vec<Value> {
    world
        .matters()
        .into_iter()
        .filter(|matter| matter["kind"] == "ci")
        .collect()
}

/// The menu on the branch's row, by entry id, with each entry's command.
fn offers(world: &World) -> Vec<(String, String)> {
    let listing = world
        .ephor()
        .args(["actions", "--item", ROW, "--json"])
        .output()
        .expect("the menu");
    shaped("actions", &listing)["offers"]
        .as_array()
        .expect("a menu")
        .iter()
        .map(|offer| {
            (
                offer["id"].as_str().unwrap_or_default().to_string(),
                offer["command"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn command<'a>(menu: &'a [(String, String)], id: &str) -> &'a str {
    menu.iter()
        .find(|(offered, _)| offered == id)
        .map(|(_, command)| command.as_str())
        .unwrap_or_else(|| panic!("no `{id}` on the branch's row: {menu:?}"))
}

#[test]
#[ignore = "red until #216: github-ci refuses `branches`, so a red main is never a row; implement removes this"]
fn a_red_main_is_one_row_until_a_green_run_clears_it() {
    let world = World::new();
    watching(&world);

    // A push to main fails its required job, with no pull request open.
    on_the_forge(
        &world,
        json!([run(901, 40, "2026-10-14T15:02:00Z", "failure")]),
        &[901],
    );
    world.ephor().args(["refresh", PROJECT]).assert().success();
    let rows = ci_rows(&world);
    assert_eq!(rows.len(), 1, "one row for the branch: {rows:?}");
    let row = world.matter(ROW);
    assert_eq!(row["needs_response"], true);
    let title = row["title"].as_str().unwrap_or_default();
    assert!(
        title.contains("CI"),
        "the row names the red workflow: {title}"
    );
    world
        .ephor()
        .args(["feed", "--kind", "ci"])
        .assert()
        .success()
        .stdout(predicate::str::contains("CI"));

    // What every red gate is offered, each naming the red run the row carries
    // so that it never looks the run up again.
    let menu = offers(&world);
    for id in ["ci-failures", "restart-failed", "restart-all"] {
        assert!(
            command(&menu, id).contains("901"),
            "`{id}` names run 901: {menu:?}"
        );
    }

    // A second red run is the same row, not a second one.
    on_the_forge(
        &world,
        json!([
            run(902, 41, "2026-10-14T16:02:00Z", "failure"),
            run(901, 40, "2026-10-14T15:02:00Z", "failure")
        ]),
        &[902],
    );
    world.ephor().args(["refresh", PROJECT]).assert().success();
    let rows = ci_rows(&world);
    assert_eq!(rows.len(), 1, "still one row: {rows:?}");
    assert!(command(&offers(&world), "ci-failures").contains("902"));

    // The first green run of that workflow clears it.
    on_the_forge(
        &world,
        json!([
            run(903, 42, "2026-10-14T20:15:00Z", "success"),
            run(902, 41, "2026-10-14T16:02:00Z", "failure"),
            run(901, 40, "2026-10-14T15:02:00Z", "failure")
        ]),
        &[],
    );
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert!(!world.has_matter(ROW), "green clears the row");
    world
        .ephor()
        .args(["feed", "--kind", "ci"])
        .assert()
        .success()
        .stdout(predicate::str::contains("No feed items"));
}
