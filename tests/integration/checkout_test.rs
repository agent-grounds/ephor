//! `ephor checkout` end to end (§FS-004-quick-actions.7): the branch workspace
//! a project describes but has not got, made from the registry alone — no
//! command configured anywhere.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use predicates::prelude::*;
use serde_json::json;

use common::*;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} in {}", dir.display());
}

fn commit(dir: &Path, file: &str, contents: &str, message: &str) {
    fs::write(dir.join(file), contents).unwrap();
    git(dir, &["add", file]);
    git(dir, &["commit", "-m", message]);
}

/// One repository's origin on `master`, cloned into `<root>/main/<name>` —
/// the main-branch workspace a new one is made from.
fn repo(root: &Path, name: &str) -> PathBuf {
    let origin = root.join(format!("{name}.git"));
    fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "--initial-branch=master", "-q"]);
    git(&origin, &["config", "user.email", "t@example.com"]);
    git(&origin, &["config", "user.name", "t"]);
    commit(&origin, "shared.txt", "one\n", "one");

    let clone = root.join("proj").join("main").join(name);
    fs::create_dir_all(clone.parent().unwrap()).unwrap();
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(&origin)
        .arg(&clone)
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    git(&clone, &["config", "user.email", "t@example.com"]);
    git(&clone, &["config", "user.name", "t"]);
    clone
}

/// Two repositories under one checkout, sharing a branch name — the shape
/// §FS-004-quick-actions.7 is mostly about.
fn poly_project_type(template: &Path) -> serde_json::Value {
    let repo = |id: &str| {
        json!({
            "id": id,
            "path": id,
            "role": "Repository",
            "required": true,
            "update_mode": "branch",
            "default_branch": "{branch}",
            "agents_description": format!("`{id}/` is a repository of this workspace.")
        })
    };
    json!([{
        "id": "poly",
        "layout": "polyrepo",
        "repos": [repo("ce"), repo("ee")],
        "agents": {
            "template": template.to_string_lossy(),
            "structure_intro": "This workspace contains separate repositories:",
            "summary_template": "This workspace is for branch `{branch}`."
        }
    }])
}

/// A poly-repo project whose checkouts are one per branch, with `main` on disk.
fn fixture(tmp: &Path) -> PathBuf {
    let template = write_template(tmp);
    let project_root = tmp.join("proj");
    write_registry(
        &tmp.join("workspaces.json"),
        &json!({
            "project_types": poly_project_type(&template),
            "hook_sets": [],
            "projects": [{
                "id": "demo",
                "type": "poly",
                "display_name": "Demo",
                "root": project_root.to_string_lossy(),
                "main_branch": "master",
                "branch_root_template": "{project_root}/{branch}",
                "release_branches": [
                    { "id": "demo-main", "branch": "main", "active": true }
                ],
                "branches": [
                    { "id": "demo-ticket", "branch": "feature", "active": true }
                ]
            }]
        }),
    );
    fs::write(
        tmp.join("status.json"),
        serde_json::to_string_pretty(&json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "work": { "runner": RUNNER },
            "projects": { "demo": { "providers": [] } }
        }))
        .unwrap(),
    )
    .unwrap();
    fs::create_dir_all(tmp.join("fakebin")).unwrap();
    project_root
}

/// The runtime these tests bind. A name of the fixture's own, on a PATH the
/// fixture owns, so whether the runner answers is something a test decides by
/// writing [`stub_runner`] — never something the machine running the suite
/// decides by having the real one installed.
const RUNNER: &str = "checkout-test-runtime";

/// A runner that records what it was asked and makes the manifest the real one
/// would, so the directory it was pointed at comes back a project
/// (§FS-006-project-interface.7). Its `.gitignore` is deliberately not ephor's:
/// what a runner's project says about version control is the runner's, and the
/// self-ignore ephor adds on top is what this proves.
const RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
verb="$1"; shift
if [ "$verb" = init ]; then
  here=""; note=1; title=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --here) here=1; shift ;;
      --no-agents) note=""; shift ;;
      --title) title="$2"; shift 2 ;;
      *) dir="$1"; shift ;;
    esac
  done
  [ -n "$here" ] || { echo "expected --here" >&2; exit 1; }
  mkdir -p "$dir"
  printf '%s\n' "$dir" > "$dir/asked"
  printf '%s\n' "$title" > "$dir/titled"
  printf '# Panta: %s\n' "$title" > "$dir/index.panta.md"
  printf 'runtime/\n' > "$dir/.gitignore"
  # The note the real one leaves in the host directory, where it was not
  # told to skip it — the checkout, which ephor promised not to change.
  [ -z "$note" ] || printf 'rhei lives here\n' >> "$(dirname "$dir")/AGENTS.md"
  exit 0
fi
exit 1
"#;

fn stub_runner(tmp: &Path) {
    make_executable(&tmp.join("fakebin").join(RUNNER), RUNTIME);
}

fn ephor(tmp: &Path) -> assert_cmd::Command {
    let mut cmd = ephor_cmd();
    cmd.env("XDG_STATE_HOME", tmp.join("state"));
    cmd.env("EPHOR_STATUS_CONFIG", tmp.join("status.json"));
    cmd.env("EPHOR_REGISTRY", tmp.join("workspaces.json"));
    // The fixture's own bin first, so a runner answers exactly when a test put
    // one there; git and the rest of the world stay reachable behind it.
    let path = std::env::join_paths(std::iter::once(tmp.join("fakebin")).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    cmd.env("PATH", path);
    cmd
}

/// The repository that has the branch is checked out on it; the one that does
/// not gets a branch of the same name off the base — and neither needed a
/// `checkout` command in anybody's configuration.
#[test]
fn a_missing_workspace_is_made_from_the_registry_alone() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    // The change lives in `ce` only, pushed to its origin.
    git(&ce, &["checkout", "-q", "-b", "feature"]);
    commit(&ce, "mine.txt", "mine\n", "mine");
    git(&ce, &["push", "-q", "origin", "feature"]);
    git(&ce, &["checkout", "-q", "master"]);
    git(&ce, &["branch", "-q", "-D", "feature"]);

    let target = root.join("feature");
    assert!(!target.exists());

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success()
        .stdout(predicates::str::contains("tracking the branch"))
        .stdout(predicates::str::contains("started from `origin/master`"));

    for name in ["ce", "ee"] {
        let path = target.join(name);
        assert!(path.join(".git").exists(), "{name} has no working tree");
    }
    // The repository with the change carries it; the other is at the base.
    assert!(target.join("ce/mine.txt").exists());
    assert!(!target.join("ee/mine.txt").exists());

    // A workspace ephor makes gets a task store, so the first dispatch into
    // this branch has somewhere to land and what is under way is visible from
    // the moment the tree exists (§FS-006-project-interface.7). It sits at the
    // root of the multi-repo workspace, beside the repositories rather than
    // inside one of them.
    let store = target.join("panta");
    assert!(store.is_dir(), "no task store at {}", store.display());
    assert!(store.join("states.yaml").is_file());
    // And it ignores itself, so it is ephor's planning state living in a
    // checkout rather than content the project carries
    // (§REQ-001-boundary.3): a `git status` in here is unchanged by it.
    let ignore = fs::read_to_string(store.join(".gitignore")).unwrap();
    assert!(ignore.contains('*'), "{ignore}");
}

/// The ticket's own transcript (issue #7): `ce` already has a local `feature`
/// that tracks `origin/master` — where it was cut from, not where it is
/// published (§DA-003-upstream-is-the-published-copy) — and `origin` has no
/// `feature` of its own. The checkout must not claim `ce`'s tree tracks a
/// branch the forge has; it names `origin/master` instead, changes no
/// tracking configuration, and pushes nothing (§FS-004-quick-actions.7).
#[test]
fn a_branch_that_tracks_the_base_is_reported_published_nowhere() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    // `ce` already has a local `feature`, deliberately tracking the base it
    // was cut from — never pushed anywhere (§DA-003-upstream-is-the-published-copy).
    git(&ce, &["branch", "--track", "feature", "origin/master"]);
    let before = fs::read_to_string(ce.join(".git/config")).unwrap();

    let target = root.join("feature");
    assert!(!target.exists());

    let view: serde_json::Value = serde_json::from_slice(
        &ephor(tmp.path())
            .args([
                "checkout",
                "--project",
                "demo",
                "--branch",
                "feature",
                "--json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout,
    )
    .unwrap();
    let report = view["report"].as_str().unwrap();
    assert!(report.contains("origin/master"), "{report}");
    assert!(!report.contains("tracking the branch"), "{report}");

    for name in ["ce", "ee"] {
        let path = target.join(name);
        assert!(path.join(".git").exists(), "{name} has no working tree");
    }
    // Reading the tracking configuration did not rewrite it, and nothing was
    // pushed: `ce`'s own config is unchanged, and `origin` still has no
    // `feature` of its own.
    assert_eq!(fs::read_to_string(ce.join(".git/config")).unwrap(), before);
    let ls_remote = Command::new("git")
        .args(["ls-remote", "--heads", "origin", "feature"])
        .current_dir(&ce)
        .output()
        .unwrap();
    assert!(ls_remote.status.success());
    assert!(ls_remote.stdout.is_empty());

    let rows = view["repos"].as_array().unwrap();
    let ce_row = rows.iter().find(|row| row["repo"] == "ce").unwrap();
    assert_eq!(ce_row["created"], "unpublished");
    assert_eq!(ce_row["tracks"], "origin/master");
    assert!(ce_row.get("says").is_none(), "{ce_row}");

    // The new working tree still answers `origin/master` for its upstream —
    // the fact the checkout named, not one it changed.
    let upstream = Command::new("git")
        .args([
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ])
        .current_dir(target.join("ce"))
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&upstream.stdout).trim(),
        "origin/master"
    );
}

/// Asked again it is not an error, and nothing is remade.
#[test]
fn a_workspace_that_is_already_there_says_so_and_succeeds() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success();
    let stamp = fs::metadata(root.join("feature/ce"))
        .unwrap()
        .modified()
        .unwrap();

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success()
        .stdout(predicates::str::contains("already checked out"));
    assert_eq!(
        fs::metadata(root.join("feature/ce"))
            .unwrap()
            .modified()
            .unwrap(),
        stamp
    );
}

/// The runtime makes its own project and ephor says where
/// (§FS-006-project-interface.7): the runner is asked for the work root ephor
/// resolved, ephor's own state machine goes in beside what it wrote, and the
/// self-ignore is ephor's whatever the runner's project says about version
/// control.
#[test]
fn the_runtime_makes_its_own_project_where_ephor_says() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    stub_runner(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success();

    let store = root.join("feature/panta");
    // What the runner was asked, in its own words: the work root, named
    // outright, and told to be the project rather than to make one under a
    // name of its own.
    let asked = fs::read_to_string(store.join("asked")).expect("the runner was asked");
    assert_eq!(asked.trim(), store.to_string_lossy(), "{asked}");
    // Named for the workspace it stands in, not for the directory it is.
    let titled = fs::read_to_string(store.join("titled")).unwrap();
    assert_eq!(titled.trim(), "feature", "{titled}");
    // What it wrote is still there, ephor's machine is beside it, and the
    // directory ignores itself all the same.
    assert!(store.join("index.panta.md").is_file());
    assert!(store.join("states.yaml").is_file());
    let ignore = fs::read_to_string(store.join(".gitignore")).unwrap();
    assert!(ignore.contains("runtime/"), "{ignore}");
    assert!(ignore.lines().any(|line| line.trim() == "*"), "{ignore}");
    // And nothing of the runner's landed in the checkout: ephor promised the
    // branch would be byte-for-byte what it was (§REQ-001-boundary.3), and the
    // runner's own discovery note would have been a change to it.
    assert!(
        !root.join("feature/AGENTS.md").exists(),
        "the runner left a note in the checkout"
    );
}

/// A runner that is not on the machine does not fail the checkout: the
/// workspace is made either way, ephor writes the store it can, and the note
/// says what it could not do (§FS-004-quick-actions.7).
#[test]
fn a_runner_that_is_not_there_leaves_the_checkout_whole() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success()
        .stderr(predicates::str::contains(RUNNER));

    let store = root.join("feature/panta");
    assert!(store.join("index.panta.md").is_file());
    assert!(store.join("states.yaml").is_file());
}

/// *Already checked out* answers the question about repositories, not the one
/// about work (§FS-004-quick-actions.7.1). A workspace that holds every
/// repository and no store is repaired by asking for the checkout again —
/// which is the shape of a workspace made before ephor made stores at all, or
/// made by a project's own checkout command.
#[test]
fn a_workspace_that_is_there_is_still_given_its_store() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success();
    // The workspace as somebody else's checkout command would have left it:
    // every repository, and nowhere for a plan to land.
    let store = root.join("feature/panta");
    fs::remove_dir_all(&store).unwrap();

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success()
        .stdout(predicates::str::contains("already checked out"))
        .stdout(predicates::str::contains("task store at"));
    assert!(store.join("states.yaml").is_file());

    // And the answer a runtime reads says the same thing (§REQ-002-parity.3).
    let output = ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--branch",
            "feature",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let view: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(view["store"]["made"], json!(false));
    assert_eq!(view["store"]["dir"], json!(store.to_string_lossy()));
}

/// A directory is not a workspace. This is the one command whose exit code
/// answers whether a workspace is whole (§AR-004-forest.1) — every other fold
/// names a declared repository that is not on disk and carries on — so a
/// workspace holding some of the project's repositories is completed rather
/// than reported as already there. The shape is ordinary: a workspace somebody
/// made by hand, or a repository the project gained after the workspace was
/// made.
#[test]
fn a_workspace_missing_a_declared_repository_is_completed() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    // Half a workspace, made by hand: `ce` has a working tree on the branch
    // and `ee` was never added.
    let target = root.join("feature");
    git(
        &ce,
        &[
            "worktree",
            "add",
            "-b",
            "feature",
            &target.join("ce").to_string_lossy(),
            "master",
        ],
    );
    assert!(target.join("ce/.git").exists());
    assert!(!target.join("ee").exists());

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success()
        .stdout(predicates::str::contains("is missing ee"))
        .stdout(predicates::str::contains("A working tree was already here"));

    assert!(target.join("ee/.git").exists(), "ee was not made");
    assert!(target.join("ce/.git").exists(), "ce was disturbed");
}

/// Everything a flag says, a program state's `env:` says too — the same
/// handover `ephor rebase` takes (§FS-005-dispatch.12).
#[test]
fn the_environment_a_program_state_sets_is_read_as_arguments() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let report = tmp.path().join("runtime/checkout.md");

    ephor(tmp.path())
        .arg("checkout")
        .env("PROJECT", "demo")
        .env("BRANCH", "feature")
        .env("REPORT", &report)
        .assert()
        .success();

    assert!(root.join("feature/ce/.git").exists());
    assert!(fs::read_to_string(&report).unwrap().contains("check out"));
}

/// A project whose root is its checkout has no workspace to make, and is told
/// that rather than being handed an empty directory.
#[test]
fn a_project_without_branch_workspaces_is_refused_by_name() {
    let tmp = tempdir();
    let template = write_template(tmp.path());
    let project_root = tmp.path().join("solo");
    fs::create_dir_all(&project_root).unwrap();
    write_registry(
        &tmp.path().join("workspaces.json"),
        &json!({
            "project_types": base_project_types(&template),
            "hook_sets": [],
            "projects": [{
                "id": "solo",
                "type": "monorepo",
                "display_name": "Solo",
                "root": project_root.to_string_lossy(),
                "main_branch": "master",
                "branches": [{ "id": "solo-b", "branch": "feature", "active": true }]
            }]
        }),
    );
    fs::write(tmp.path().join("status.json"), "{}").unwrap();

    ephor(tmp.path())
        .args(["checkout", "--project", "solo", "--branch", "feature"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "does not use a checkout per branch",
        ));
}

/// Nothing on disk to add a working tree from is a refusal that says so,
/// rather than a half-made directory.
#[test]
fn a_project_with_no_checkout_yet_is_refused_by_name() {
    let tmp = tempdir();
    fixture(tmp.path());

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no checkout on disk"));
}

/// Put this project's work root somewhere other than the shipped
/// `{workspace}/panta` — the configuration that makes a branch name able to
/// land on it (§FS-004-quick-actions.7.3).
fn work_root(tmp: &Path, template: &str) {
    let path = tmp.join("status.json");
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    config["work"]["root"] = json!(template);
    fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
}

/// A project whose work root is written beside its branch checkouts rather
/// than inside each of them has a directory a branch name can land on. The
/// checkout refuses the name in the ticket's own words and makes nothing —
/// asserted on the filesystem, because git's refusal would have arrived after
/// the directories were there (§FS-004-quick-actions.7.3).
#[test]
fn a_branch_named_for_the_work_root_is_refused_and_nothing_is_made() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    work_root(tmp.path(), "{root}/panta");
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "panta"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("work root"))
        .stderr(predicates::str::contains("panta"));

    assert!(
        !root.join("panta").exists(),
        "the work root was checked out over"
    );
}

/// A name whose rendered path climbs out of the area the project puts its
/// branch workspaces in is refused before anything is made. The reproduction
/// this pins is the directory that was left two levels above the project root
/// while git was still being asked (§FS-004-quick-actions.7.3).
#[test]
fn a_branch_that_climbs_out_of_the_project_is_refused_and_leaves_nothing() {
    let tmp = tempdir();
    let _root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "../escaped"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("../escaped"));

    assert!(
        !tmp.path().join("escaped").exists(),
        "a directory was made outside the project root"
    );
}

/// The ticket's first fault: a `--branch` that will not do became `None` and
/// the reader was told they had passed no branch at all. A value that is there
/// is refused naming the flag it came in on, and exits 2 — the code a value
/// ephor will not act on takes (§FS-011-command-line.9). Under `--json` the
/// refusal is on standard output as an outcome, like every other
/// (§FS-011-command-line.7).
#[test]
fn an_unresolved_branch_is_refused_by_name_rather_than_read_as_none() {
    let tmp = tempdir();
    let _root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "{branch}"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--branch"))
        .stderr(predicates::str::contains("{branch}"))
        .stderr(predicates::str::contains("Nothing says which branch").not());

    let refused = ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--branch",
            "{branch}",
            "--json",
        ])
        .assert()
        .code(2)
        .get_output()
        .stdout
        .clone();
    let outcome: serde_json::Value = serde_json::from_slice(&refused).unwrap();
    assert_eq!(outcome["ok"], json!(false));
    assert!(
        outcome["says"]
            .as_str()
            .unwrap_or_default()
            .contains("--branch"),
        "{outcome}"
    );

    // The same value a program state sets, refused by the name it set it under.
    ephor(tmp.path())
        .arg("checkout")
        .env("PROJECT", "demo")
        .env("BRANCH", "{meta.branch}")
        .assert()
        .code(2)
        .stderr(predicates::str::contains("BRANCH"));
}

/// The compatibility half, and the regression the collision guard could
/// cause: on the shipped `{workspace}/panta` the work root is inside each
/// workspace, so `panta` is an ordinary branch name and a project on the
/// default configuration keeps its checkout (§FS-004-quick-actions.7.3).
#[test]
fn the_shipped_work_root_leaves_a_branch_called_panta_alone() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "panta"])
        .assert()
        .success();

    assert!(root.join("panta/ce/.git").exists());
    assert!(root.join("panta/panta/states.yaml").is_file());
}

/// The other compatibility half: absent, and empty once trimmed, still mean
/// *nothing was given* and still fall through (§FS-011-command-line.9). A
/// state machine handing a program `BRANCH: "{meta.branch}"` about a matter
/// with no branch is the caller that depends on it.
#[test]
fn an_empty_value_is_still_nothing_given() {
    let tmp = tempdir();
    let _root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    ephor(tmp.path())
        .arg("checkout")
        .env("PROJECT", "demo")
        .env("BRANCH", "   ")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Nothing says which branch"));
}

// ---------------------------------------------------------------------------
// The checkout a project declared. Where a project binds a `checkout` command
// it is the maker of that project's branch workspaces, and ephor's git is the
// fallback for a project that binds none (§FS-004-quick-actions.7,
// §FS-006-project-interface.8). These take the two halves the surface above
// cannot: what the contract hands the command, and what ephor holds it to.
// ---------------------------------------------------------------------------

/// Bind this project's own checkout command — the one substitution point a
/// site has for what a branch workspace of its project *is*.
fn bind_checkout(tmp: &Path, command: &Path) {
    let path = tmp.join("status.json");
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    config["projects"]["demo"]["checkout"] = json!({ "command": command.to_string_lossy() });
    fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();
}

/// A checkout command that writes down everything it was told before it makes
/// anything, then makes the workspace the ordinary way. It is how a case asks
/// what the contract actually handed it rather than what the manual says it
/// does.
fn recording_checkout(tmp: &Path, record: &Path) -> PathBuf {
    let path = tmp.join("fakebin").join("recording-checkout");
    make_executable(
        &path,
        &format!(
            "#!/usr/bin/env bash\n\
             set -euo pipefail\n\
             env | sort > {record}\n\
             printf 'RECORDED_PWD=%s\\n' \"$(pwd -P)\" >> {record}\n\
             mkdir -p \"$EPHOR_WORKSPACE\"\n\
             for repo in ce ee; do\n\
             \x20 git -C \"$PWD/main/$repo\" worktree add --quiet -B \"$EPHOR_BRANCH\" \
             \"$EPHOR_WORKSPACE/$repo\" master\n\
             done\n",
            record = record.display(),
        ),
    );
    path
}

/// A command that exits 0 having made the directory and no repository — the
/// report's sharpest sentence as a fixture: the dispatch succeeds, the
/// workspace exists, and ephor's own verification passes.
fn hollow_checkout(tmp: &Path) -> PathBuf {
    let path = tmp.join("fakebin").join("hollow-checkout");
    make_executable(
        &path,
        "#!/usr/bin/env bash\nset -euo pipefail\nmkdir -p \"$EPHOR_WORKSPACE\"\nexit 0\n",
    );
    path
}

/// One name a bound command was told, out of the environment it recorded.
fn told(record: &str, name: &str) -> Option<String> {
    record
        .lines()
        .find(|line| line.starts_with(&format!("{name}=")))
        .map(|line| line[name.len() + 1..].to_string())
}

/// What a bound command is guaranteed, in its own words
/// (§FS-006-project-interface.8). `EPHOR_BRANCH` is the branch the checkout is
/// *making* — which on the dispatch's path is a name nobody has cut, so the
/// matter's own answer would be empty exactly where the command needs one —
/// `EPHOR_WORKSPACE` is where it goes, the working directory is the project's
/// root, and the matter's names are present and empty on a call with no matter
/// behind it, because a summons does not start from a cleared environment
/// (§FS-006-project-interface.3) and an unset name would be read as some other
/// matter's.
#[test]
fn a_bound_checkout_command_is_told_the_branch_it_is_making() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let record = tmp.path().join("told.env");
    let command = recording_checkout(tmp.path(), &record);
    bind_checkout(tmp.path(), &command);

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success();

    let said = fs::read_to_string(&record)
        .unwrap_or_else(|err| panic!("the bound command was never asked to make it: {err}"));
    let root_said = root.to_string_lossy().into_owned();
    assert_eq!(
        told(&said, "EPHOR_PROJECT").as_deref(),
        Some("demo"),
        "{said}"
    );
    assert_eq!(
        told(&said, "EPHOR_BRANCH").as_deref(),
        Some("feature"),
        "{said}"
    );
    assert_eq!(
        told(&said, "EPHOR_WORKSPACE").as_deref(),
        Some(root.join("feature").to_string_lossy().as_ref()),
        "{said}"
    );
    assert_eq!(
        told(&said, "EPHOR_ROOT").as_deref(),
        Some(root_said.as_str()),
        "{said}"
    );
    assert_eq!(
        told(&said, "RECORDED_PWD").as_deref(),
        Some(root_said.as_str()),
        "the command runs from the project's root, which is what lets it reach the \
         checkouts beside the one it is making:\n{said}"
    );
    for empty in [
        "EPHOR_ITEM_ID",
        "EPHOR_SOURCE",
        "EPHOR_KIND",
        "EPHOR_TITLE",
        "EPHOR_URL",
        "EPHOR_REPO",
        "EPHOR_NUMBER",
    ] {
        assert_eq!(
            told(&said, empty).as_deref(),
            Some(""),
            "{empty} is set and empty on a call with no matter, never absent:\n{said}"
        );
    }
    // And the one name it is told about ephor itself, which is what lets a
    // command wrap `ephor checkout` without summoning itself for ever.
    assert!(
        told(&said, "EPHOR_CHECKOUT_MAKING").is_some(),
        "the marker that ends the recursion was not exported:\n{said}"
    );
}

/// A bound command may wrap ephor's own checkout — the workspace made the
/// ordinary way and a step of the site's own after it — and a maker that
/// summons the binding would otherwise summon itself for ever. The marker ends
/// it: the nested operation finds its own marker there and makes the workspace
/// with git rather than asking the command again
/// (§FS-006-project-interface.8). The wrapper counts its own calls, so the
/// recursion this is about is bounded by the fixture rather than by a timeout.
#[test]
fn a_command_that_wraps_ephor_checkout_terminates_in_git() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let log = tmp.path().join("wrapper.log");
    let wrapper = tmp.path().join("fakebin").join("wrapping-checkout");
    make_executable(
        &wrapper,
        &format!(
            "#!/usr/bin/env bash\n\
             set -euo pipefail\n\
             printf 'called\\n' >> {log}\n\
             if [ \"$(wc -l < {log})\" -gt 2 ]; then\n\
             \x20 echo 'the wrapper was summoned by its own summons' >&2\n\
             \x20 exit 1\n\
             fi\n\
             {ephor} checkout --project \"$EPHOR_PROJECT\" --branch \"$EPHOR_BRANCH\"\n\
             printf 'wrapped\\n' > \"$EPHOR_WORKSPACE/.after-the-checkout\"\n",
            log = log.display(),
            ephor = assert_cmd::cargo::cargo_bin("ephor").display(),
        ),
    );
    bind_checkout(tmp.path(), &wrapper);

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success();

    let target = root.join("feature");
    assert!(
        target.join("ce/.git").exists() && target.join("ee/.git").exists(),
        "the nested checkout did not make the workspace with git"
    );
    assert!(
        target.join(".after-the-checkout").is_file(),
        "the wrapper's own step never ran, so the wrapper never composed"
    );
    assert_eq!(
        fs::read_to_string(&log).unwrap_or_default().lines().count(),
        1,
        "the wrapper is asked once and the checkout inside it is git's"
    );
}

/// What a branch is grown from is the bound command's to decide, and ephor has
/// nothing to pass it a base through — so `--from` is refused naming the input
/// it came in on, and exits 2, the code a value ephor will not act on takes
/// (§FS-004-quick-actions.7.4, §FS-011-command-line.9). A flag that parsed and
/// changed nothing would be the worse of the two answers.
#[test]
fn from_is_refused_where_a_checkout_command_is_bound() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let record = tmp.path().join("told.env");
    let command = recording_checkout(tmp.path(), &record);
    bind_checkout(tmp.path(), &command);

    ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--branch",
            "feature",
            "--from",
            "master",
        ])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--from"));

    assert!(
        !record.exists(),
        "the command was summoned behind a refusal"
    );
    assert!(
        !root.join("feature").exists(),
        "a refusal left a directory behind"
    );
}

/// *Verified* is the directory and every repository the project declares, not
/// the exit code (§FS-006-project-interface.8). A command that returns 0
/// having made a directory and no repository is the checkout not made: the
/// absent repositories are named, ephor's git does not fill in a tree it did
/// not make, and no store is put into a workspace that was not made.
#[test]
fn a_command_that_returned_without_making_it_is_the_checkout_not_made() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let command = hollow_checkout(tmp.path());
    bind_checkout(tmp.path(), &command);

    let refused = ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&refused.stdout).into_owned()
        + &String::from_utf8_lossy(&refused.stderr);
    assert!(
        !refused.status.success(),
        "a workspace that was not made read as made: {said}"
    );
    for repo in ["ce", "ee"] {
        assert!(
            said.contains(repo),
            "the repository the command did not make is not named: {said}"
        );
    }
    let target = root.join("feature");
    assert!(
        !target.join("ce").exists() && !target.join("ee").exists(),
        "ephor's git filled in a tree the command did not make"
    );
    assert!(
        !target.join("panta").exists(),
        "a plan was given somewhere to land in a workspace that was not made"
    );
}

/// And `75` is among them. Every other verb reads it as *parked* — not
/// applicable now, ask again later (§FS-006-project-interface.3) — but a
/// workspace either exists or it does not, so here it is the checkout not made
/// and the code is said (§FS-006-project-interface.8).
#[test]
fn a_parked_exit_is_the_checkout_not_made() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let command = tmp.path().join("fakebin").join("parking-checkout");
    make_executable(&command, "#!/usr/bin/env bash\nexit 75\n");
    bind_checkout(tmp.path(), &command);

    let refused = ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&refused.stdout).into_owned()
        + &String::from_utf8_lossy(&refused.stderr);
    assert!(
        !refused.status.success(),
        "a parked checkout read as a made one: {said}"
    );
    assert!(
        said.contains("75"),
        "the exit code the command answered with is not said: {said}"
    );
    assert!(
        !root.join("feature/panta").exists(),
        "a store was made in a workspace that was not"
    );
}

/// Which maker made the workspace is a fact the reading owes whoever asked, in
/// prose and in `--json` alike (§REQ-002-parity.3): the two answers hold
/// different things — the project's own command decides what a workspace of this
/// project *is*, ephor's git answers where nothing is bound — so a reader who
/// cannot tell them apart cannot tell a slice from a whole tree either, which is
/// the silence this whole contract is about (§FS-006-project-interface.8).
///
/// Absent rather than `null` where nothing was made, which is the same rule the
/// distance beside it follows (§REQ-002-parity.4): a workspace that was already
/// whole was made by nobody just now.
#[test]
fn the_reading_says_which_maker_made_the_workspace() {
    // Where nothing is bound, git is the maker and says so.
    let tmp = tempdir();
    let _root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");

    let made = ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--branch",
            "feature",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let view: serde_json::Value = serde_json::from_slice(&made).unwrap();
    assert_eq!(view["maker"], json!("git"), "{view}");

    // And a second ask made nothing, so it names no maker at all.
    let again = ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--branch",
            "feature",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let view: serde_json::Value = serde_json::from_slice(&again).unwrap();
    assert_eq!(view["ready"], json!(true), "{view}");
    assert!(
        view.get("maker").is_none(),
        "a workspace that was already there was made by nobody just now: {view}"
    );

    // Where the project bound its own command, that command is the maker — and
    // the prose says so, since a reader has the prose and not the reading.
    let tmp = tempdir();
    let _root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let record = tmp.path().join("told.env");
    let command = recording_checkout(tmp.path(), &record);
    bind_checkout(tmp.path(), &command);

    ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .assert()
        .success()
        .stdout(predicates::str::contains("checkout command made"));

    let bound = ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--branch",
            "other",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let view: serde_json::Value = serde_json::from_slice(&bound).unwrap();
    assert_eq!(view["maker"], json!("command"), "{view}");
    assert_eq!(view["ready"], json!(true), "{view}");
}
