//! A watched branch's gate, as `github-ci` reads it: the newest verdict of
//! each workflow, from the branch's own runs, at one listing per branch
//! (§FS-001-forge-interface.1, §FS-001-forge-interface.8.4).

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use super::common::{assert_golden, ephor_cmd, make_executable, tempdir};
use super::{feed_env, write_feed_fixture, FAKE_GH};

/// GitHub's run listing, served from a fixture and filtered the way the forge
/// filters it: `branch` matches the run's head branch, whoever's head that is,
/// which is how a fork's pull request from its own `main` lands under `main`.
/// Every call is logged with what it asked for. `gh run list` and `gh run view`
/// are refused: they spend two and three requests on what one `gh api` call
/// answers (§FS-001-forge-interface.8.4).
const GH: &str = r#"#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

fixture_path = Path(os.environ['BRANCH_RUNS_FIXTURE'])
fixture = json.loads(fixture_path.read_text())
args = sys.argv[1:]
call = {'args': args}
try:
    if args[:2] == ['pr', 'list']:
        print('[]')
        sys.exit(0)
    if args[:1] == ['run']:
        sys.exit('gh run ' + ' '.join(args[1:2]) + ' was asked; one gh api request answers it')
    if args[:1] != ['api']:
        print('[]')
        sys.exit(0)
    path = next(arg for arg in args[1:] if 'actions/runs' in arg)
    url = urlsplit(path)
    query = {key: values[-1] for key, values in parse_qs(url.query).items()}
    for flag, value in zip(args, args[1:]):
        if flag in ('-f', '-F', '--field', '--raw-field'):
            key, _, text = value.partition('=')
            query[key] = text
    parts = url.path.strip('/').split('/')
    repo = '/'.join(parts[1:3])
    call.update(repo=repo, query=query)
    if parts[-1] == 'jobs':
        call['jobs'] = parts[-2]
        jobs = fixture.get('jobs', {}).get(parts[-2], [])
        print(json.dumps({'total_count': len(jobs), 'jobs': jobs}))
    else:
        call['listing'] = query.get('branch')
        runs = [run for run in fixture['runs'].get(repo, [])
                if 'branch' not in query or run['head_branch'] == query['branch']]
        print(json.dumps({'total_count': len(runs), 'workflow_runs': runs}))
finally:
    with open(fixture_path.with_suffix('.calls'), 'a') as log:
        log.write(json.dumps(call) + '\n')
"#;

const SHA: &str = "aa4484657d3fb2c3e8f459ba715f2d0df5d2e3f6";

/// One completed push run on `main` of `acme/widget`. `changes` is laid over
/// it, for the run that differs in its event, its head, its status.
fn run(id: u64, workflow: (u64, &str), number: u64, created: &str, conclusion: &str) -> Value {
    json!({
        "id": id,
        "name": workflow.1,
        "display_title": "Widen the retry window",
        "workflow_id": workflow.0,
        "run_number": number,
        "run_attempt": 1,
        "event": "push",
        "status": "completed",
        "conclusion": conclusion,
        "head_branch": "main",
        "head_sha": SHA,
        "html_url": format!("https://github.com/acme/widget/actions/runs/{id}"),
        "created_at": created,
        "run_started_at": created,
        "updated_at": created,
        "head_repository": { "full_name": "acme/widget" },
        "repository": { "full_name": "acme/widget" }
    })
}

fn with(mut run: Value, changes: Value) -> Value {
    for (key, value) in changes.as_object().expect("changes are an object") {
        run[key] = value.clone();
    }
    run
}

const CI: (u64, &str) = (11, "CI");
const DOCS: (u64, &str) = (12, "Docs");
const ROW: &str = "github-ci:acme/widget@main";

/// The jobs under every run a fixture marks red: one failed, one passed.
fn jobs(ids: &[u64]) -> Value {
    let mut jobs = serde_json::Map::new();
    for id in ids {
        jobs.insert(
            id.to_string(),
            json!([
                { "id": id * 10, "name": "cargo test (ubuntu-latest)", "status": "completed",
                  "conclusion": "failure",
                  "html_url": format!("https://github.com/acme/widget/actions/runs/{id}/job/1") },
                { "id": id * 10 + 1, "name": "cargo fmt", "status": "completed",
                  "conclusion": "success",
                  "html_url": format!("https://github.com/acme/widget/actions/runs/{id}/job/2") }
            ]),
        );
    }
    Value::Object(jobs)
}

/// What one refresh of a `github-ci` source watching `branches` left behind:
/// the source's matters, and every call it made of `gh`.
struct Refreshed {
    matters: Vec<Value>,
    calls: Vec<Value>,
}

impl Refreshed {
    fn ci(&self) -> Vec<&Value> {
        self.matters.iter().filter(|m| m["kind"] == "ci").collect()
    }

    fn row(&self, key: &str) -> Option<&Value> {
        self.matters.iter().find(|m| m["key"] == key)
    }

    fn listings(&self) -> Vec<&Value> {
        self.calls
            .iter()
            .filter(|c| c.get("listing").is_some())
            .collect()
    }

    fn jobs(&self) -> Vec<&str> {
        self.calls
            .iter()
            .filter_map(|c| c["jobs"].as_str())
            .collect()
    }
}

fn refresh(branches: &[&str], runs: Value, jobs: Value) -> Refreshed {
    let tmp = tempdir();
    let tmp = tmp.path();
    write_feed_fixture(tmp);
    fs::write(
        tmp.join("status.json"),
        serde_json::to_string_pretty(&json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10, "github_user": "tester" },
            "projects": { "demo": { "providers": [
                { "provider": "github-ci", "repos": ["acme/widget"], "branches": branches }
            ] } }
        }))
        .unwrap(),
    )
    .unwrap();
    let fixture = tmp.join("runs.json");
    fs::write(
        &fixture,
        json!({ "runs": { "acme/widget": runs }, "jobs": jobs }).to_string(),
    )
    .unwrap();
    let fake_bin = tmp.join("fakebin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("gh"), GH);

    let mut cmd = ephor_cmd();
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            fake_bin.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    )
    .env("BRANCH_RUNS_FIXTURE", &fixture);
    for (key, value) in feed_env(tmp) {
        cmd.env(key, value);
    }
    cmd.args(["refresh", "demo"]).assert().success();

    let cache: Value =
        serde_json::from_str(&fs::read_to_string(tmp.join("state/ephor/feed/demo.json")).unwrap())
            .unwrap();
    let calls = fs::read_to_string(tmp.join("runs.calls"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    Refreshed {
        matters: cache["providers"]["github-ci"]["matters"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        calls,
    }
}

/// A red push run on `main` with no pull request open is one `ci` row keyed by
/// the branch, waiting on the reader, and naming the run, the commit and the
/// job that failed (§FS-001-forge-interface.1, §FS-007-matters.1).
#[test]
#[ignore = "red until #216: github-ci refuses `branches` and never reads a branch's runs; implement removes this"]
fn a_red_push_run_on_main_is_one_row_keyed_by_the_branch() {
    let seen = refresh(
        &["main"],
        json!([run(901, CI, 40, "2026-10-14T15:02:00Z", "failure")]),
        jobs(&[901]),
    );
    assert_eq!(
        seen.ci().len(),
        1,
        "one row for the branch: {:?}",
        seen.matters
    );
    let row = seen
        .row(ROW)
        .expect("the row is keyed by the repository and the branch");
    assert_eq!(row["needs_response"], true);
    assert_eq!(row["raw"]["repo"], "acme/widget");
    assert_eq!(row["raw"]["branch"], "main");
    // The subject is marked a branch rather than inferred from `branch`, which
    // a pull request's report carries too (§FS-007-matters.1).
    assert_eq!(row["raw"]["watched_branch"], true);
    assert_eq!(row["raw"]["gate"]["repos"][0]["failed"], 1);
    assert_eq!(row["raw"]["gate"]["repos"][0]["passed"], 1);
    let said = row.to_string();
    for named in ["CI", "901", "aa44846", "cargo test (ubuntu-latest)"] {
        assert!(said.contains(named), "the row names {named}: {row}");
    }
}

/// A second red run updates the row rather than adding one, and the first
/// green run of that workflow clears it (§FS-001-forge-interface.3).
#[test]
#[ignore = "red until #216: github-ci refuses `branches` and never reads a branch's runs; implement removes this"]
fn a_newer_red_run_is_the_same_row_and_a_newer_green_run_clears_it() {
    let red = run(901, CI, 40, "2026-10-14T15:02:00Z", "failure");
    let redder = run(902, CI, 41, "2026-10-14T16:02:00Z", "failure");
    let seen = refresh(
        &["main"],
        json!([redder.clone(), red.clone()]),
        jobs(&[902]),
    );
    assert_eq!(seen.ci().len(), 1, "{:?}", seen.matters);
    assert!(seen
        .row(ROW)
        .expect("the same key")
        .to_string()
        .contains("902"));

    let green = run(903, CI, 42, "2026-10-14T17:02:00Z", "success");
    let seen = refresh(&["main"], json!([green, redder, red]), jobs(&[]));
    assert!(seen.ci().is_empty(), "green clears it: {:?}", seen.matters);
    assert!(!seen.listings().is_empty(), "the branch was asked about");
}

/// A fork's pull request from its own `main` is listed under `main`, and so is
/// a run it triggers; neither is the branch's own run, and a stranger's red
/// change must not turn the watched branch red (§FS-001-forge-interface.1).
#[test]
#[ignore = "red until #216: github-ci refuses `branches` and never reads a branch's runs; implement removes this"]
fn runs_the_branch_did_not_make_do_not_turn_it_red() {
    let own = run(901, CI, 40, "2026-10-14T15:02:00Z", "success");
    let fork = with(
        run(902, CI, 41, "2026-10-14T16:00:00Z", "failure"),
        json!({ "event": "pull_request",
                "head_repository": { "full_name": "stranger/widget" } }),
    );
    let triggered = with(
        run(903, (13, "Report"), 7, "2026-10-14T16:10:00Z", "failure"),
        json!({ "event": "workflow_run",
                "head_repository": { "full_name": "stranger/widget" } }),
    );
    // A pull request from this repository's own `main` into another branch:
    // its head is here, and still a pull request's event started it.
    let inward = with(
        run(904, CI, 42, "2026-10-14T16:20:00Z", "failure"),
        json!({ "event": "pull_request" }),
    );
    let seen = refresh(
        &["main"],
        json!([inward, triggered, fork, own]),
        jobs(&[902, 903, 904]),
    );
    assert!(
        seen.ci().is_empty(),
        "only the branch's own runs count: {:?}",
        seen.matters
    );
    assert_eq!(
        seen.listings().len(),
        1,
        "the branch was asked about: {:?}",
        seen.calls
    );
}

/// Newest is by run number or creation time, never by the last update: an old
/// red run rerun until it went green does not clear a newer red one.
#[test]
#[ignore = "red until #216: github-ci refuses `branches` and never reads a branch's runs; implement removes this"]
fn an_old_run_rerun_to_green_does_not_clear_a_newer_red_one() {
    let rerun = with(
        run(901, CI, 40, "2026-10-14T10:00:00Z", "success"),
        json!({ "run_attempt": 2, "updated_at": "2026-10-14T13:00:00Z" }),
    );
    let newer = with(
        run(902, CI, 41, "2026-10-14T11:00:00Z", "failure"),
        json!({ "updated_at": "2026-10-14T11:30:00Z" }),
    );
    // Listed most recently updated first, so neither the list's order nor the
    // last update picks the run that decides.
    let seen = refresh(&["main"], json!([rerun, newer]), jobs(&[902]));
    let row = seen.row(ROW).expect("the newer red run still stands");
    assert!(row.to_string().contains("902"), "{row}");
}

/// The verdict is keyed by the workflow's id, not its name: two workflows both
/// called CI are two verdicts, and the green one does not hide the red one.
#[test]
#[ignore = "red until #216: github-ci refuses `branches` and never reads a branch's runs; implement removes this"]
fn two_workflows_with_one_name_are_two_verdicts() {
    let red = run(901, (11, "CI"), 40, "2026-10-14T10:00:00Z", "failure");
    let green = run(902, (12, "CI"), 41, "2026-10-14T11:00:00Z", "success");
    let seen = refresh(&["main"], json!([green, red]), jobs(&[901]));
    assert_eq!(seen.ci().len(), 1, "{:?}", seen.matters);
    assert!(seen
        .row(ROW)
        .expect("workflow 11 is red")
        .to_string()
        .contains("901"));
}

/// A run still going, or one that ended cancelled, has no verdict: it leaves
/// the one before it standing, red or green.
#[test]
#[ignore = "red until #216: github-ci refuses `branches` and never reads a branch's runs; implement removes this"]
fn a_run_without_a_verdict_leaves_the_earlier_one_standing() {
    let cancelled =
        |id, number, created| with(run(id, CI, number, created, "cancelled"), json!({}));
    let going = |id, number, created| {
        with(
            run(id, CI, number, created, "success"),
            json!({ "status": "in_progress", "conclusion": null }),
        )
    };
    let red = run(901, CI, 40, "2026-10-14T10:00:00Z", "failure");
    let seen = refresh(
        &["main"],
        json!([
            going(903, 42, "2026-10-14T12:00:00Z"),
            cancelled(902, 41, "2026-10-14T11:00:00Z"),
            red
        ]),
        jobs(&[901]),
    );
    assert!(
        seen.row(ROW)
            .expect("still red")
            .to_string()
            .contains("901"),
        "{:?}",
        seen.matters
    );

    let green = run(901, CI, 40, "2026-10-14T10:00:00Z", "success");
    let seen = refresh(
        &["main"],
        json!([
            going(903, 42, "2026-10-14T12:00:00Z"),
            cancelled(902, 41, "2026-10-14T11:00:00Z"),
            green
        ]),
        jobs(&[]),
    );
    assert!(seen.ci().is_empty(), "still green: {:?}", seen.matters);
    assert_eq!(seen.listings().len(), 1, "the branch was asked about");
}

/// One push ran a red `ci.yml` and a green `docs.yml` that finished later: one
/// red row, naming the workflow that is red and not the one that is not.
#[test]
#[ignore = "red until #216: github-ci refuses `branches` and never reads a branch's runs; implement removes this"]
fn a_green_workflow_finishing_later_does_not_hide_a_red_one() {
    let red = run(901, CI, 40, "2026-10-14T15:02:00Z", "failure");
    let green = with(
        run(902, DOCS, 9, "2026-10-14T15:02:05Z", "success"),
        json!({ "updated_at": "2026-10-14T15:30:00Z" }),
    );
    let seen = refresh(&["main"], json!([green, red]), jobs(&[901]));
    assert_eq!(seen.ci().len(), 1, "{:?}", seen.matters);
    let title = seen.row(ROW).expect("red")["title"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(title.contains("CI") && !title.contains("Docs"), "{title}");
}

/// A scheduled run on the branch counts like a pushed one, and the row says
/// which workflow is red, so a weekly Auto bump does not read as CI.
#[test]
#[ignore = "red until #216: github-ci refuses `branches` and never reads a branch's runs; implement removes this"]
fn a_red_scheduled_run_raises_the_row_and_the_row_names_it() {
    let ci = run(901, CI, 40, "2026-10-12T09:00:00Z", "success");
    let bump = with(
        run(902, (14, "Auto bump"), 9, "2026-10-13T06:00:00Z", "failure"),
        json!({ "event": "schedule" }),
    );
    let seen = refresh(&["main"], json!([bump, ci]), jobs(&[902]));
    let title = seen.row(ROW).expect("scheduled runs count")["title"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert!(title.contains("Auto bump"), "{title}");
}

/// The cost is one listing per repository and branch, carrying the branch and
/// a page of a hundred, plus one jobs listing per red run and none for a green
/// one (§FS-001-forge-interface.8.4).
#[test]
#[ignore = "red until #216: github-ci refuses `branches` and never reads a branch's runs; implement removes this"]
fn a_refresh_asks_one_listing_per_branch_and_one_jobs_list_per_red_run() {
    let red = run(901, CI, 40, "2026-10-14T15:02:00Z", "failure");
    let green = with(
        run(902, CI, 12, "2026-10-14T15:05:00Z", "success"),
        json!({ "head_branch": "release/1" }),
    );
    let seen = refresh(&["main", "release/1"], json!([green, red]), jobs(&[901]));
    let mut asked: Vec<&str> = seen
        .listings()
        .iter()
        .map(|call| call["listing"].as_str().unwrap_or("(no branch)"))
        .collect();
    asked.sort();
    assert_eq!(asked, ["main", "release/1"], "{:?}", seen.calls);
    for listing in seen.listings() {
        assert_eq!(listing["query"]["per_page"], "100", "{listing}");
    }
    assert_eq!(seen.jobs(), ["901"], "jobs only under the red run");
    assert!(seen.row(ROW).is_some());
    assert!(seen.row("github-ci:acme/widget@release/1").is_none());
}

/// A source that names no branch asks what it asked before and nothing more,
/// and the feed it produces is the recorded one byte for byte.
#[test]
fn without_branches_no_run_is_listed_and_the_feed_is_unchanged() {
    let tmp = tempdir();
    let tmp = tmp.path();
    write_feed_fixture(tmp);
    let fake_bin = tmp.join("fakebin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("gh-canned"), FAKE_GH);
    make_executable(
        &fake_bin.join("gh"),
        "#!/usr/bin/env bash\nhere=\"$(dirname \"$0\")\"\n\
         printf '%s\\n' \"$*\" >> \"$here/../gh.calls\"\nexec \"$here/gh-canned\" \"$@\"\n",
    );
    let mut cmd = ephor_cmd();
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            fake_bin.display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    for (key, value) in feed_env(tmp) {
        cmd.env(key, value);
    }
    cmd.args(["refresh", "demo"]).assert().success();

    assert_golden("github-feed.json", &tmp.join("state/ephor/feed/demo.json"));
    let calls = read(&tmp.join("gh.calls"));
    assert!(
        calls.contains("pr checks"),
        "the log is the source's: {calls}"
    );
    for asked in ["actions/runs", "run list", "run view"] {
        assert!(
            !calls.contains(asked),
            "asked {asked} with no branch named:\n{calls}"
        );
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}
