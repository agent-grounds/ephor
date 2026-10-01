//! One live run per checkout, end to end (§AR-007-runtime): what the sweep
//! does with two work roots over one working tree, and what the hand-started
//! key says when a run is already in that tree.
//!
//! A sibling of `work_capacity_test.rs` because the guard here is not a
//! ceiling: capacity is a budget the reader sets, and this is an invariant
//! about the tree the run edits.

mod common;

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use common::*;

/// A detached runner that holds every root it is given long enough for the
/// rest of the sweep to see the run, and logs what it was asked to start. Its
/// child redirects the inherited pipes so the launcher itself returns at once,
/// as a real detached launcher does.
fn holding_runner(tmp: &Path, log: &Path) {
    fs::create_dir_all(tmp.join("fakebin")).unwrap();
    make_executable(
        &tmp.join("fakebin/rhei"),
        &format!(
            "#!/usr/bin/env bash\n\
             case \"$*\" in\n\
             *--help*) printf 'Options:\\n      --headless  detach it\\n'; exit 0 ;;\n\
             *--headless*)\n\
               printf '%s\\n' \"$*\" >> {log}\n\
               root=\"$4\"\n\
               mkdir -p \"$root/.rhei\" \"$root/runtime\"\n\
               printf '{{\"id\":\"live-run\"}}\\n' > \"$root/runtime/run.json\"\n\
               ready=\"$root/.rhei/run-lock-ready\"\n\
               python -c 'import fcntl,pathlib,sys,time; lock=open(sys.argv[1],\"w\"); fcntl.flock(lock,fcntl.LOCK_EX); pathlib.Path(sys.argv[2]).touch(); time.sleep(20)' \"$root/.rhei/run.lock\" \"$ready\" >/dev/null 2>&1 &\n\
               for _ in {{1..200}}; do\n\
                 [[ -e \"$ready\" ]] && break\n\
                 sleep 0.01\n\
               done\n\
               [[ -e \"$ready\" ]] || exit 1\n\
               printf '{{\"id\":\"live-run\",\"status\":\"running\",\"exit_code\":null}}\\n'\n\
               exit 0 ;;\n\
             *) exit 0 ;;\n\
             esac\n",
            log = log.to_string_lossy(),
        ),
    );
}

/// A dispatched fixture with one autorun recipe whose site ceiling of zero
/// lets the ticket be written and starts nothing, so each case below decides
/// for itself what may run.
fn dispatched_but_unstarted(tmp: &Path, log: &Path) {
    fixture(
        tmp,
        json!({
            "max_concurrent": 0,
            "recipes": [{
                "id": "fix-gate",
                "icon": "🔧",
                "description": "fix the red gate",
                "brief": "fix {title}",
                "autorun": true,
                "when": { "kinds": ["pr"], "gate": "red" }
            }]
        }),
    );
    detaching_runner(tmp, log);
    ephor(tmp).args(["refresh", "demo"]).assert().success();
    ephor(tmp).args(["work", "dispatch"]).assert().success();
    let path = tmp.join("status.json");
    let mut config: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    config["work"]["max_concurrent"] = Value::Null;
    fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
}

/// A second work root beside the fixture's own, in the same checkout: two
/// pantas over one working tree.
fn second_root_in_the_same_checkout(tmp: &Path, item: &str) {
    duplicate_root_at(tmp, item, &tmp.join("demo"), "panta2", "second");
}

/// §FS-005-dispatch.24: a run live on one root stops the sweep starting
/// anything on another root over the same working tree. The live root holds no
/// due ticket of its own, which is exactly the case a per-root guard misses.
#[test]
fn a_sweep_starts_nothing_in_a_checkout_a_run_already_holds() {
    let tmp = tempdir();
    let log = tmp.path().join("runner.log");
    dispatched_but_unstarted(tmp.path(), &log);
    second_root_in_the_same_checkout(tmp.path(), "github-prs:acme/widget#second");
    let holder = hold(&tmp.path().join("demo/panta"));

    let output = ephor(tmp.path())
        .args(["work", "run", "--due", "--json"])
        .output()
        .unwrap();
    let reading: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        ephor::api::schema::holds("work-run", &reading).is_empty(),
        "the sweep must hold to the published work-run shape: {reading}"
    );
    assert!(
        output.status.success(),
        "passing a root over is not a failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(starts(&log), 0, "no run may be started in a busy checkout");
    // Nothing started, and the sweep says which run has the tree rather than
    // reading as a quiet machine. One row: the root holding the run itself is
    // said nothing about, because it has its run.
    let runs = reading["runs"].as_array().unwrap();
    assert_eq!(
        runs.len(),
        1,
        "one root is passed over, one is running: {reading}"
    );
    assert_eq!(runs[0]["outcome"], "passed-over", "{reading}");
    assert!(
        runs[0]["root"].as_str().unwrap().ends_with("demo/panta2"),
        "the root held back is the one beside the live run: {reading}"
    );
    assert!(
        runs[0]["reason"]
            .as_str()
            .unwrap()
            .contains("a run is live in this checkout"),
        "the reason is the tree, not a ceiling: {reading}"
    );
    drop(holder);
}

/// §FS-005-dispatch.24: one invocation, two work roots over one tree, nothing
/// live to begin with — the run this very command starts holds the tree for
/// the rest of it, so the second group is refused by the id of the first's
/// run rather than started beside it.
#[test]
fn one_command_starts_one_run_in_one_checkout() {
    let tmp = tempdir();
    let log = tmp.path().join("runner.log");
    dispatched_but_unstarted(tmp.path(), &log);
    second_root_in_the_same_checkout(tmp.path(), "github-prs:acme/widget#second");
    holding_runner(tmp.path(), &log);

    let output = ephor(tmp.path())
        .args(["work", "run", "--json"])
        .output()
        .unwrap();
    let reading: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        ephor::api::schema::holds("work-run", &reading).is_empty(),
        "a refusal must hold to the published work-run shape: {reading}"
    );
    assert!(
        !output.status.success(),
        "the reader asked for two runs and got one: {reading}"
    );
    assert_eq!(reading["refused"], 1, "{reading}");
    assert_eq!(reading["failed"], 0, "{reading}");
    let runs = reading["runs"].as_array().unwrap();
    let outcomes: Vec<&str> = runs
        .iter()
        .filter_map(|run| run["outcome"].as_str())
        .collect();
    assert_eq!(outcomes, vec!["started", "refused"], "{reading}");
    assert_eq!(
        runs[1]["says"], "a run is live in this checkout: live-run",
        "the refusal names the run this command just started: {reading}"
    );
    assert_eq!(starts(&log), 1, "one working tree takes one run");
}

/// §FS-005-dispatch.24: `--force` lifts that refusal too — the reader who
/// says they know what the other run is doing means the run made a second ago
/// as much as one that was already there.
#[test]
fn force_starts_both_groups_in_one_checkout() {
    let tmp = tempdir();
    let log = tmp.path().join("runner.log");
    dispatched_but_unstarted(tmp.path(), &log);
    second_root_in_the_same_checkout(tmp.path(), "github-prs:acme/widget#second");
    holding_runner(tmp.path(), &log);

    let output = ephor(tmp.path())
        .args(["work", "run", "--force", "--json"])
        .output()
        .unwrap();
    let reading: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        output.status.success(),
        "--force starts them anyway: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(reading["refused"], 0, "{reading}");
    let outcomes: Vec<&str> = reading["runs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|run| run["outcome"].as_str())
        .collect();
    assert_eq!(outcomes, vec!["started", "started"], "{reading}");
    assert_eq!(starts(&log), 2, "both roots were launched");
}

/// §FS-005-dispatch.24: two roots over one tree, both due, in one sweep. The
/// snapshot the due list was read from is older than the first launch, so the
/// second is passed over with the run that took the tree — a successful
/// non-launch outcome, not a failed start.
#[test]
fn one_sweep_starts_one_of_two_due_roots_over_one_checkout() {
    let tmp = tempdir();
    let log = tmp.path().join("runner.log");
    dispatched_but_unstarted(tmp.path(), &log);
    second_root_in_the_same_checkout(tmp.path(), "github-prs:acme/widget#second");
    holding_runner(tmp.path(), &log);

    let output = ephor(tmp.path())
        .args(["work", "run", "--due", "--json"])
        .output()
        .unwrap();
    let reading: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        ephor::api::schema::holds("work-run", &reading).is_empty(),
        "the sweep must hold to the published work-run shape: {reading}"
    );
    assert!(
        output.status.success(),
        "passing a root over is not a failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let runs = reading["runs"].as_array().unwrap();
    let outcomes: Vec<&str> = runs
        .iter()
        .filter_map(|run| run["outcome"].as_str())
        .collect();
    assert_eq!(
        outcomes.iter().filter(|of| **of == "started").count(),
        1,
        "one working tree takes one run: {reading}"
    );
    let over = runs
        .iter()
        .find(|run| run["outcome"] == "passed-over")
        .unwrap_or_else(|| panic!("{reading}"));
    assert!(
        over["reason"]
            .as_str()
            .unwrap()
            .contains("a run is live in this checkout: live-run"),
        "the reason names the run that took the tree: {reading}"
    );
    assert_eq!(reading["failed"], 0, "{reading}");
    assert_eq!(starts(&log), 1, "only one root may be launched");
}

/// §FS-005-dispatch.24: the key the reader presses is refused by name when a
/// run holds the plan's tree, and `--force` lifts exactly that refusal.
#[test]
fn a_named_plan_in_a_busy_checkout_is_refused_and_forced() {
    let tmp = tempdir();
    let log = tmp.path().join("runner.log");
    dispatched_but_unstarted(tmp.path(), &log);
    second_root_in_the_same_checkout(tmp.path(), "github-prs:acme/widget#second");
    // A run in the fixture's own root, with a descriptor naming it, so the
    // refusal has a run id to give rather than only a root.
    let root = tmp.path().join("demo/panta");
    fs::create_dir_all(root.join("runtime")).unwrap();
    fs::write(root.join("runtime/run.json"), r#"{"id":"live-run"}"#).unwrap();
    let holder = hold(&root);

    let refused = ephor(tmp.path())
        .args([
            "work",
            "run",
            "--item",
            "github-prs:acme/widget#second",
            "--json",
        ])
        .output()
        .unwrap();
    let reading: Value = serde_json::from_slice(&refused.stdout).unwrap();
    assert!(
        ephor::api::schema::holds("work-run", &reading).is_empty(),
        "a refusal must hold to the published work-run shape: {reading}"
    );
    assert!(
        !refused.status.success(),
        "the reader asked for a run and got none: {reading}"
    );
    assert_eq!(reading["refused"], 1, "{reading}");
    assert_eq!(reading["runs"][0]["outcome"], "refused", "{reading}");
    assert_eq!(
        reading["runs"][0]["says"], "a run is live in this checkout: live-run",
        "{reading}"
    );
    assert_eq!(starts(&log), 0, "nothing may be started into a busy tree");

    let forced = ephor(tmp.path())
        .args([
            "work",
            "run",
            "--item",
            "github-prs:acme/widget#second",
            "--force",
            "--json",
        ])
        .output()
        .unwrap();
    let reading: Value = serde_json::from_slice(&forced.stdout).unwrap();
    assert!(
        forced.status.success(),
        "--force starts it anyway: {}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert_eq!(reading["runs"][0]["outcome"], "started", "{reading}");
    assert_eq!(starts(&log), 1, "the forced run is the only one started");
    drop(holder);
}

/// §FS-005-dispatch.24: the guard is where runs start, never where plans are
/// written. A busy checkout still takes a ticket — that is what makes handing
/// one down to a tree somebody is working in safe.
#[test]
fn a_busy_checkout_still_takes_a_dispatched_ticket() {
    let tmp = tempdir();
    let log = tmp.path().join("runner.log");
    dispatched_but_unstarted(tmp.path(), &log);
    let holder = hold(&tmp.path().join("demo/panta"));

    // Forget what the fixture already dispatched, so this dispatch has a
    // ticket to write rather than an item it has already answered.
    ephor(tmp.path())
        .args(["work", "forget", "--item", "github-prs:acme/widget#42"])
        .assert()
        .success();
    let output = ephor(tmp.path())
        .args(["work", "dispatch", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "writing a file into a busy checkout is not a start: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reading: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reading["opened"], 1, "{reading}");
    assert_eq!(
        reading["refused"], 0,
        "nothing refuses where plans are written: {reading}"
    );
    // Dispatch starts what it just wrote where nobody has to press a key
    // (§FS-005-dispatch.24), and that continuation is the sweep — so this is
    // also the guard holding on the path a hand-down would take.
    assert_eq!(
        starts(&log),
        0,
        "the ticket waits in the plan until the tree is free"
    );
    drop(holder);
}

// ---------------------------------------------------------------------------
// The other invariant about the tree a run edits: the dispatch's own side of
// the checkout a project declared. Where a project binds a `checkout` command
// that command makes the minted workspace (§FS-004-quick-actions.7,
// §FS-006-project-interface.8) — and ephor still owes it the work store, since
// the plan the dispatch is about to write lands in it
// (§FS-004-quick-actions.7.1). What is asserted here rather than in the
// scenario is that the ledger and the tree agree.
// ---------------------------------------------------------------------------

/// A forge with one issue of the reader's. An issue has no branch until
/// somebody cuts one, so it is the matter a dispatch has to mint a workspace
/// for, and the dispatch is then the only maker there is
/// (§FS-005-dispatch.25).
const ISSUE_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities) printf '{"issues":true,"pull_requests":false}' ;;
  issues) printf '%s' '[
      { "key": "acme/widget#95", "title": "Durations read as seconds",
        "url": "https://acme.example/issue/95",
        "updated_at": "2026-09-20T12:00:00Z", "status": "open" }
    ]' ;;
  *) printf '[]' ;;
esac
"#;

/// The branch the shipped `implement` recipe renders for that issue.
const MINTED: &str = "fix/issue-95";

fn git_in(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} in {}", dir.display());
}

/// A project whose checkouts are one per branch, on a repository published on
/// `main` and cloned into the `main` workspace, watching the forge above.
/// `src/` is what a site's slice keeps and `README.md` is what it leaves out.
fn minting_fixture(tmp: &Path) -> std::path::PathBuf {
    let template = write_template(tmp);
    let root = tmp.join("demo");
    let origin = tmp.join("origin");
    fs::create_dir_all(origin.join("src")).unwrap();
    git_in(&origin, &["init", "-q", "--initial-branch=main"]);
    git_in(&origin, &["config", "user.email", "t@example.com"]);
    git_in(&origin, &["config", "user.name", "t"]);
    fs::write(origin.join("README.md"), "the project\n").unwrap();
    fs::write(origin.join("src/main.rs"), "fn main() {}\n").unwrap();
    git_in(&origin, &["add", "-A"]);
    git_in(&origin, &["commit", "-q", "-m", "the project"]);
    fs::create_dir_all(&root).unwrap();
    let status = std::process::Command::new("git")
        .args(["clone", "-q"])
        .arg(&origin)
        .arg(root.join("main"))
        .status()
        .unwrap();
    assert!(status.success());

    write_registry(
        &tmp.join("workspaces.json"),
        &json!({
            "project_types": base_project_types(&template),
            "hook_sets": [],
            "projects": [{
                "id": "demo",
                "type": "monorepo",
                "display_name": "Demo",
                "root": root.to_string_lossy(),
                "main_branch": "main",
                "branch_root_template": "{project_root}/{branch}",
                "branches": []
            }]
        }),
    );
    fs::create_dir_all(tmp.join("fakebin")).unwrap();
    make_executable(&tmp.join("fakebin/ephor-forge-acmeforge"), ISSUE_FORGE);
    fs::write(
        tmp.join("status.json"),
        serde_json::to_string_pretty(&json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "projects": { "demo": {
                "providers": [{ "provider": "acmeforge", "user": "you", "repos": ["widget"] }]
            }}
        }))
        .unwrap(),
    )
    .unwrap();
    root
}

/// Bind a checkout command on that project.
fn bind(tmp: &Path, command: &Path) {
    let path = tmp.join("status.json");
    let mut config: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    config["projects"]["demo"]["checkout"] = json!({ "command": command.to_string_lossy() });
    fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
}

/// The site's own checkout: a sparse slice rather than a whole tree, with a
/// marker naming who made it.
fn site_checkout(tmp: &Path) -> std::path::PathBuf {
    let path = tmp.join("fakebin/site-checkout");
    make_executable(
        &path,
        "#!/usr/bin/env bash\n\
         set -euo pipefail\n\
         : \"${EPHOR_WORKSPACE:?}\" \"${EPHOR_BRANCH:?}\"\n\
         git -C \"$PWD/main\" worktree add --quiet -B \"$EPHOR_BRANCH\" \"$EPHOR_WORKSPACE\" main\n\
         git -C \"$EPHOR_WORKSPACE\" sparse-checkout set --no-cone src\n\
         printf 'made by the project checkout command\\n' > \"$EPHOR_WORKSPACE/.made-by-site-checkout\"\n",
    );
    path
}

/// A command that makes the directory and no repository, and exits 0 — the
/// shape the whole contract is about: *returned* is not *made*
/// (§FS-006-project-interface.8). It logs the branch it was asked for, so a
/// case can assert it was not handed back the directory its own refusal left.
fn hollow_checkout(tmp: &Path, log: &Path) -> std::path::PathBuf {
    let path = tmp.join("fakebin/hollow-checkout");
    make_executable(
        &path,
        &format!(
            "#!/usr/bin/env bash\n\
             set -euo pipefail\n\
             printf '%s\\n' \"$EPHOR_BRANCH\" >> {log}\n\
             mkdir -p \"$EPHOR_WORKSPACE\"\n\
             exit 0\n",
            log = log.display(),
        ),
    );
    path
}

/// How many times a bound command was asked, from the log it appends to.
fn asked(log: &Path) -> usize {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn ledger(tmp: &Path) -> Value {
    let path = tmp.join("state/ephor/work.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({ "entries": {} }))
}

/// The workspace a bound command made is still owed the store, and the ledger
/// has to agree with the tree: the branch it minted, the checkout the command
/// made, the work root inside it, and the plan in that root
/// (§FS-004-quick-actions.7.1, §FS-005-dispatch.25). A dispatch cannot wait
/// for a second ask to repair the store — the plan it is writing lands there.
#[test]
fn the_store_a_dispatch_needs_is_in_the_workspace_a_bound_command_made() {
    let tmp = tempdir();
    let root = minting_fixture(tmp.path());
    let command = site_checkout(tmp.path());
    bind(tmp.path(), &command);

    ephor(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    ephor(tmp.path())
        .args(["work", "dispatch"])
        .assert()
        .success();

    let workspace = root.join(MINTED);
    assert!(
        workspace.join(".made-by-site-checkout").is_file(),
        "the project's own checkout command did not mint {}",
        workspace.display()
    );
    let store = workspace.join("panta");
    assert!(
        store.join("states.yaml").is_file(),
        "the workspace the command made has nowhere for a plan to land"
    );

    let entries = ledger(tmp.path());
    let entry = &entries["entries"]["acmeforge:acme/widget#95"];
    assert_eq!(entry["branch"], json!(MINTED), "{entry}");
    assert_eq!(entry["checkout"], json!(workspace), "{entry}");
    assert_eq!(entry["root"], json!(store), "{entry}");
    let plan = entry["plan"].as_str().unwrap_or_default();
    assert!(
        Path::new(plan).is_file(),
        "the ledger names a plan that is not in the store the command's workspace got: {entry}"
    );
    assert!(
        plan.starts_with(&store.to_string_lossy().into_owned()),
        "the plan landed outside the minted workspace: {entry}"
    );
}

/// And nothing is dispatched behind a checkout that was not made
/// (§FS-005-dispatch.25): a command that returns 0 having made a directory and
/// no repository leaves no store, no plan, and no ledger entry — where today
/// the dispatch succeeds and ephor's own verification passes.
#[test]
fn a_dispatch_behind_a_workspace_that_was_not_made_writes_nothing() {
    let tmp = tempdir();
    let root = minting_fixture(tmp.path());
    let log = tmp.path().join("hollow-calls.log");
    let command = hollow_checkout(tmp.path(), &log);
    bind(tmp.path(), &command);

    ephor(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    // Asked about the one matter: a refusal is this command's own answer only
    // where a caller asked about one, since a sweep steps over what it cannot
    // reach and says so in its tally (§FS-005-dispatch.12). What the checkout
    // refuses is the same either way.
    let refused = ephor(tmp.path())
        .args(["work", "dispatch", "--item", "acmeforge:acme/widget#95"])
        .output()
        .unwrap();
    assert!(
        !refused.status.success(),
        "a workspace that was not made read as made: {}",
        String::from_utf8_lossy(&refused.stdout)
    );

    let workspace = root.join(MINTED);
    assert!(!workspace.join("panta").exists(), "a store was made anyway");
    assert!(
        !workspace.join("src").exists(),
        "ephor's git filled in a tree the command did not make"
    );
    assert!(
        ledger(tmp.path())["entries"]
            .get("acmeforge:acme/widget#95")
            .is_none(),
        "the ledger recorded work whose workspace does not exist"
    );

    // And on the attempt after that, which is the one the directory hides. A
    // workspace is resolved from the directory being there, so the bare one the
    // refusal left behind reads as checked out: nothing is minted, the maker is
    // never asked, and a store, a plan and a ledger entry would land in a tree
    // holding none of the project's repositories — the same silence, one attempt
    // later (§FS-006-project-interface.8).
    let again = ephor(tmp.path())
        .args(["work", "dispatch", "--item", "acmeforge:acme/widget#95"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&again.stderr).into_owned();
    assert!(
        !again.status.success(),
        "a directory that is not a workspace read as one: {}",
        String::from_utf8_lossy(&again.stdout)
    );
    assert!(
        said.contains("the repository at its root not on disk there"),
        "the repository the command did not make is not named: {said}"
    );
    assert_eq!(
        asked(&log),
        1,
        "the command was handed back the directory its own refusal left behind"
    );
    assert!(
        !workspace.join("panta").exists(),
        "a store was made behind a checkout that was not made"
    );
    assert!(
        !workspace.join("src").exists(),
        "ephor's git filled in a tree the command did not make"
    );
    assert!(
        ledger(tmp.path())["entries"]
            .get("acmeforge:acme/widget#95")
            .is_none(),
        "the ledger recorded work whose workspace does not exist"
    );
}

// ---------------------------------------------------------------------------
// The same state reached the other way, and the boundary that keeps the
// question narrow. A workspace resolves as *present* from a directory being
// there whether a `branch` template minted it or the matter owns the branch, so
// the half-made question is asked about every branch workspace of a
// bound-command project — and about the project's own checkout never, which is
// not a branch workspace and was never the command's to make
// (§FS-006-project-interface.8).
// ---------------------------------------------------------------------------

/// A forge with one pull request of the reader's own, on a branch that is
/// already cut. No `branch` template is asked for a matter that owns its
/// branch, so this is the path that reaches a bare directory with nothing to
/// mint (§FS-005-dispatch.25).
const PR_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities) printf '{"issues":false,"pull_requests":true}' ;;
  pull-requests) printf '%s' '[
      { "id": "widget/7", "repo": "widget", "number": "7",
        "title": "Widen the retry window",
        "url": "https://acme.example/pr/7", "branch": "feature",
        "updated_at": "2026-09-20T12:00:00Z",
        "role": "author", "state": "open", "cited": false }
    ]' ;;
  *) printf '[]' ;;
esac
"#;

/// The branch that pull request owns, which the registry declares.
const OWNED: &str = "feature";

/// One recipe that edits the change, so the matter above needs the workspace.
fn editing_recipe(tmp: &Path) {
    let path = tmp.join("status.json");
    let mut config: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    config["work"] = json!({ "recipes": [{
        "id": "edit", "icon": "E", "description": "edit the change",
        "needs_checkout": true,
        "when": { "kinds": ["pr"], "roles": ["author"] },
        "brief": "Edit {title}."
    }]});
    fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
}

/// `minting_fixture` about a matter that owns its branch: the same project and
/// the same repository, with the branch declared and the forge serving a pull
/// request on it.
fn owned_branch_fixture(tmp: &Path) -> std::path::PathBuf {
    let root = minting_fixture(tmp);
    make_executable(&tmp.join("fakebin/ephor-forge-acmeforge"), PR_FORGE);
    let path = tmp.join("workspaces.json");
    let mut registry: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    registry["projects"][0]["branches"] = json!([{
        "id": OWNED, "branch": OWNED, "active": true, "ticket": "ABC-42"
    }]);
    fs::write(&path, serde_json::to_string_pretty(&registry).unwrap()).unwrap();
    editing_recipe(tmp);
    root
}

/// A matter that owns its branch reaches the same bare directory, and the
/// dispatch refuses it there too (§FS-006-project-interface.8). Nothing is
/// minted on this path — the branch is already cut — so before this the maker
/// was never entered at all, and a store, a plan and a ledger entry landed in a
/// tree holding none of the project's repositories one ask after the maker's own
/// refusal had said it was not a workspace.
#[test]
fn a_dispatch_about_a_matter_on_its_own_branch_refuses_a_workspace_that_was_not_made() {
    let tmp = tempdir();
    let root = owned_branch_fixture(tmp.path());
    let log = tmp.path().join("hollow-calls.log");
    let command = hollow_checkout(tmp.path(), &log);
    bind(tmp.path(), &command);

    ephor(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    // The maker's own ask first, which is what leaves the directory behind: the
    // command returns 0, the workspace is not one, and the checkout refuses.
    let refused = ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--branch",
            OWNED,
            "--item",
            "acmeforge:widget/7",
        ])
        .output()
        .unwrap();
    assert!(
        !refused.status.success(),
        "a workspace that was not made read as made: {}",
        String::from_utf8_lossy(&refused.stdout)
    );
    let workspace = root.join(OWNED);
    assert!(workspace.is_dir(), "the command made no directory to judge");
    assert_eq!(asked(&log), 1, "the command was not the maker asked");

    // And the dispatch about the matter on that branch says the same thing,
    // rather than resolving *checked out* from the directory the refusal left.
    let again = ephor(tmp.path())
        .args([
            "work",
            "dispatch",
            "--item",
            "acmeforge:widget/7",
            "--recipe",
            "edit",
        ])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&again.stderr).into_owned();
    assert!(
        !again.status.success(),
        "a directory that is not a workspace read as one: {}",
        String::from_utf8_lossy(&again.stdout)
    );
    assert!(
        said.contains("the repository at its root not on disk there"),
        "the repository the command did not make is not named: {said}"
    );
    assert!(
        !workspace.join("panta").exists(),
        "a store was made behind a checkout that was not made"
    );
    assert!(
        !workspace.join("src").exists(),
        "ephor's git filled in a tree the command did not make"
    );
    assert_eq!(
        asked(&log),
        1,
        "the command was handed back the directory its own refusal left behind"
    );
    assert!(
        ledger(tmp.path())["entries"]
            .get("acmeforge:widget/7")
            .is_none(),
        "the ledger recorded work whose workspace does not exist"
    );
}

/// A project that keeps one checkout at its root, with a command bound. Its
/// workspace *is* the project root: not a branch workspace, and never the
/// command's to make — so the half-made question is not asked about it, and a
/// dispatch there is not refused for a repository the root is missing
/// (§FS-006-project-interface.8). This is the boundary the question is drawn to:
/// the fix above reaches every branch workspace and nothing else.
#[test]
fn a_single_checkout_project_is_never_judged_by_the_half_made_question() {
    let tmp = tempdir();
    let template = write_template(tmp.path());
    let root = tmp.path().join("single");
    // A polyrepo declaring three repositories, of which only `app` is on disk,
    // so the root is a checkout with a declared repository absent from it —
    // exactly the state that refuses in a branch workspace.
    let origin = tmp.path().join("pr-origin");
    fs::create_dir_all(origin.join("src")).unwrap();
    git_in(&origin, &["init", "-q", "--initial-branch=main"]);
    git_in(&origin, &["config", "user.email", "t@example.com"]);
    git_in(&origin, &["config", "user.name", "t"]);
    fs::write(origin.join("src/main.rs"), "fn main() {}\n").unwrap();
    git_in(&origin, &["add", "-A"]);
    git_in(&origin, &["commit", "-q", "-m", "the project"]);
    fs::create_dir_all(&root).unwrap();
    let cloned = std::process::Command::new("git")
        .args(["clone", "-q"])
        .arg(&origin)
        .arg(root.join("app"))
        .status()
        .unwrap();
    assert!(cloned.success());
    git_in(&root.join("app"), &["checkout", "-q", "-b", OWNED]);

    write_registry(
        &tmp.path().join("workspaces.json"),
        &json!({
            "project_types": base_project_types(&template),
            "hook_sets": [],
            "projects": [{
                "id": "demo",
                "type": "product-workspace",
                "display_name": "Demo",
                "root": root.to_string_lossy(),
                "main_branch": "main",
                "branches": [{
                    "id": OWNED, "branch": OWNED, "active": true, "ticket": "ABC-42"
                }]
            }]
        }),
    );
    fs::create_dir_all(tmp.path().join("fakebin")).unwrap();
    make_executable(&tmp.path().join("fakebin/ephor-forge-acmeforge"), PR_FORGE);
    fs::write(
        tmp.path().join("status.json"),
        serde_json::to_string_pretty(&json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "projects": { "demo": {
                "providers": [{ "provider": "acmeforge", "user": "you", "repos": ["widget"] }]
            }}
        }))
        .unwrap(),
    )
    .unwrap();
    editing_recipe(tmp.path());
    let log = tmp.path().join("hollow-calls.log");
    let command = hollow_checkout(tmp.path(), &log);
    bind(tmp.path(), &command);

    ephor(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    let dispatched = ephor(tmp.path())
        .args([
            "work",
            "dispatch",
            "--item",
            "acmeforge:widget/7",
            "--recipe",
            "edit",
        ])
        .output()
        .unwrap();
    assert!(
        dispatched.status.success(),
        "a dispatch into the project's own checkout was refused for a repository the root is \
         missing: {}{}",
        String::from_utf8_lossy(&dispatched.stdout),
        String::from_utf8_lossy(&dispatched.stderr)
    );
    assert!(
        root.join("panta/states.yaml").is_file(),
        "the work store the dispatch has always put in the project root is gone"
    );
    assert_eq!(
        asked(&log),
        0,
        "the project's own checkout was handed to the command that makes branch workspaces"
    );
}

/// Declare this project's only repository `update_mode: skip` — the shape a
/// site uses for a checkout it keeps by hand. The row still declares the
/// repository; what it says is not to update it (§AR-004-forest.2), so the
/// forest is that one row rather than a tree probed off the disk.
fn skip_every_repository(tmp: &Path) {
    let path = tmp.join("workspaces.json");
    let mut registry: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    for repo in registry["project_types"][0]["repos"]
        .as_array_mut()
        .expect("the type declares repositories")
    {
        repo["update_mode"] = json!("skip");
    }
    fs::write(&path, serde_json::to_string_pretty(&registry).unwrap()).unwrap();
}

/// And the dispatch's half of the same question. The maker used to call the
/// bare directory *already checked out* on the second ask and the dispatch
/// wrote its plan straight into a tree holding no repository of the project
/// (§FS-006-project-interface.8). Dispatched twice, the refusal holds and
/// nothing is left behind — and it refuses by naming the declared repository
/// that is missing, because a row saying `skip` is a row saying not to update
/// a repository and not a project declaring none (§AR-004-forest.2).
#[test]
fn a_dispatch_into_a_tree_holding_no_repository_of_the_project_refuses_twice() {
    let tmp = tempdir();
    let root = minting_fixture(tmp.path());
    skip_every_repository(tmp.path());
    let log = tmp.path().join("hollow-calls.log");
    let command = hollow_checkout(tmp.path(), &log);
    bind(tmp.path(), &command);

    ephor(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    let workspace = root.join(MINTED);
    for ask in 1..=2 {
        let refused = ephor(tmp.path())
            .args(["work", "dispatch", "--item", "acmeforge:acme/widget#95"])
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&refused.stdout).into_owned()
            + &String::from_utf8_lossy(&refused.stderr);
        assert!(
            !refused.status.success(),
            "ask {ask}: a tree holding no repository of demo read as a workspace: {said}"
        );
        assert!(
            said.contains("the repository at its root not on disk there"),
            "ask {ask}: the refusal does not name the declared repository that is \
             missing: {said}"
        );
        assert!(
            !workspace.join("panta").exists(),
            "ask {ask}: a store was made behind a checkout that was not made"
        );
        assert!(
            ledger(tmp.path())["entries"]
                .get("acmeforge:acme/widget#95")
                .is_none(),
            "ask {ask}: the ledger recorded work whose workspace does not exist"
        );
    }
    assert_eq!(
        asked(&log),
        1,
        "the command was handed back the directory its own refusal left behind"
    );
}

/// The third path the census turned up, and the ordering it needs. The opening
/// move a recipe declares runs before the mint, and it replays commits in the
/// workspace — so a dispatch that is going to refuse this tree has to refuse it
/// before the first thing it does to it, which asking the question from inside
/// the mint alone did not (§FS-006-project-interface.8, §FS-005-dispatch.12).
/// Pinned on the order of the two refusals, because that is the one thing about
/// it a case can read without depending on what a replay would have done: the
/// opening move here is one ephor does not know, which `opening` refuses before
/// it reads anything, so whichever refusal arrives is the one that was asked
/// first.
#[test]
fn a_workspace_that_was_not_made_is_refused_before_the_opening_move() {
    let tmp = tempdir();
    let root = owned_branch_fixture(tmp.path());
    let path = tmp.path().join("status.json");
    let mut config: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    config["work"]["recipes"][0]["opens_with"] = json!("nonsense");
    fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
    let log = tmp.path().join("hollow-calls.log");
    let command = hollow_checkout(tmp.path(), &log);
    bind(tmp.path(), &command);

    ephor(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--branch",
            OWNED,
            "--item",
            "acmeforge:widget/7",
        ])
        .output()
        .unwrap();
    assert!(
        root.join(OWNED).is_dir(),
        "the command made no directory to judge"
    );

    let refused = ephor(tmp.path())
        .args([
            "work",
            "dispatch",
            "--item",
            "acmeforge:widget/7",
            "--recipe",
            "edit",
        ])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&refused.stdout).into_owned()
        + &String::from_utf8_lossy(&refused.stderr);
    assert!(
        !refused.status.success(),
        "a directory that is not a workspace read as one: {said}"
    );
    assert!(
        said.contains("is not a workspace of demo"),
        "the workspace was not refused first: {said}"
    );
    assert!(
        !said.contains("which ephor does not know"),
        "the opening move had its chance at the tree before the tree was judged: {said}"
    );
}
