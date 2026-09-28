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

/// A forge with one pull request of the reader's own, on the very branch this
/// project's registry declares — which is the shape a ticket key lives in. The
/// matter is matched to that row (§FS-008-attribution.3), and the row is where
/// `EPHOR_TICKET` comes from.
const FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities)
    printf '{"pull_requests":true}'
    ;;
  pull-requests)
    printf '%s' '[
      { "id": "app/7", "repo": "app", "number": "7",
        "title": "Widen the retry window",
        "url": "https://acme.example/pr/7",
        "branch": "feature",
        "updated_at": "2026-08-01T12:00:00Z",
        "role": "author", "state": "open", "cited": false }
    ]'
    ;;
  *) printf '[]' ;;
esac
"#;

/// That pull request in the feed. `ephor checkout --item` is what the menu
/// entry and the state machine's program state both run
/// (§FS-004-quick-actions.7), so this is the matter a bound command meets when a
/// reader presses the key.
const MATTER: &str = "acme:app/7";

/// Put a matter behind the checkout: the forge above, watched, and the ticket
/// key the registry's `feature` row carries. The fixture declares that row
/// already — all this adds is the key, because a branch with no ticket is the
/// one shape that cannot tell an emptied name from a name that was never there.
fn a_matter_on_the_declared_branch(tmp: &Path) {
    make_executable(&tmp.join("fakebin").join("ephor-forge-acme"), FORGE);

    let path = tmp.join("workspaces.json");
    let mut registry: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    registry["projects"][0]["branches"][0]["ticket"] = json!("ABC-42");
    write_registry(&path, &registry);

    let path = tmp.join("status.json");
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    config["projects"]["demo"]["providers"] =
        json!([{ "provider": "acme", "user": "you", "repos": ["app"] }]);
    fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    ephor(tmp).args(["refresh", "demo"]).assert().success();
}

/// The matter's own names are the matter's where there is one, and the ticket
/// key of the registry branch it was matched to is among them
/// (§FS-006-project-interface.8). This is the name that went missing when the
/// menu entry became a caller of `ephor checkout`: the chain built its dossier
/// from the matched row and the maker rebuilt it from the ask, which carries no
/// row — so a command that had been told `ABC-42` was told the empty string, and
/// nothing in the suite asked.
#[test]
fn a_bound_command_is_told_the_ticket_of_the_matter_it_is_making_for() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let record = tmp.path().join("told.env");
    let command = recording_checkout(tmp.path(), &record);
    bind_checkout(tmp.path(), &command);
    a_matter_on_the_declared_branch(tmp.path());

    // Every value the menu entry passes, in the order it passes them
    // (§FS-004-quick-actions.7): the project, the matter and the branch.
    ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--item",
            MATTER,
            "--branch",
            "feature",
        ])
        .assert()
        .success();

    let said = fs::read_to_string(&record)
        .unwrap_or_else(|err| panic!("the bound command was never asked to make it: {err}"));
    assert_eq!(
        told(&said, "EPHOR_TICKET").as_deref(),
        Some("ABC-42"),
        "the ticket key of the branch the matter was matched to:\n{said}"
    );
    // And the rest of the matter with it, because the row fills in more than the
    // one name: restoring `EPHOR_TICKET` alone would leave the next of them to
    // be found by whoever next binds a command.
    assert_eq!(
        told(&said, "EPHOR_ITEM_ID").as_deref(),
        Some(MATTER),
        "{said}"
    );
    assert_eq!(
        told(&said, "EPHOR_TITLE").as_deref(),
        Some("Widen the retry window"),
        "{said}"
    );
    assert_eq!(
        told(&said, "EPHOR_WORKSPACE").as_deref(),
        Some(root.join("feature").to_string_lossy().as_ref()),
        "{said}"
    );
    assert!(
        root.join("feature").join("ce").is_dir(),
        "the command did not make the workspace it was told about"
    );
}

/// And the branch is still the one being *made*. The matter has a branch of its
/// own and it is the wrong answer here — on the dispatch's path it is a name
/// nobody has cut — so the matched row filling the dossier must not take
/// `EPHOR_BRANCH` back off the ask (§FS-005-dispatch.25). Asked for a branch the
/// matter does not own, the command is told the one it is making and the
/// matter's ticket, which are two different rows' worth of fact arriving
/// together.
#[test]
fn the_branch_being_made_wins_over_the_branch_the_matter_owns() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let record = tmp.path().join("told.env");
    let command = recording_checkout(tmp.path(), &record);
    bind_checkout(tmp.path(), &command);
    a_matter_on_the_declared_branch(tmp.path());

    ephor(tmp.path())
        .args([
            "checkout",
            "--project",
            "demo",
            "--item",
            MATTER,
            "--branch",
            "spike/elsewhere",
        ])
        .assert()
        .success();

    let said = fs::read_to_string(&record)
        .unwrap_or_else(|err| panic!("the bound command was never asked to make it: {err}"));
    assert_eq!(
        told(&said, "EPHOR_BRANCH").as_deref(),
        Some("spike/elsewhere"),
        "the branch this checkout is making, not the one the matter owns:\n{said}"
    );
    assert_eq!(
        told(&said, "EPHOR_TICKET").as_deref(),
        Some("ABC-42"),
        "the matter is still the matter it was matched to:\n{said}"
    );
    assert_eq!(
        told(&said, "EPHOR_WORKSPACE").as_deref(),
        Some(root.join("spike/elsewhere").to_string_lossy().as_ref()),
        "{said}"
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

/// And a refusal reaches `--report` whichever maker refused. The file is where a
/// program state reads the checkout out of rather than the terminal
/// (§FS-005-dispatch.12), and git's path has always written it before its own
/// refusal was tested — so a bound command whose refusal left none would be the
/// one maker that fails silently exactly where it is being watched
/// (§REQ-002-parity.3).
#[test]
fn a_refused_checkout_writes_its_report_on_either_maker() {
    let tmp = tempdir();
    let _root = fixture(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let command = hollow_checkout(tmp.path());
    bind_checkout(tmp.path(), &command);
    let report = tmp.path().join("runtime/checkout.md");

    let refused = ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .arg("--report")
        .arg(&report)
        .output()
        .unwrap();
    assert!(
        !refused.status.success(),
        "a workspace that was not made read as made: {}",
        String::from_utf8_lossy(&refused.stderr)
    );

    let written = fs::read_to_string(&report).unwrap_or_else(|err| {
        panic!("the bound command's refusal left no report where git's leaves one: {err}")
    });
    for repo in ["ce", "ee"] {
        assert!(
            written.contains(repo),
            "the report does not name what the command did not make: {written}"
        );
    }
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

/// Declare every repository of this project type `update_mode: skip` — a
/// schema-valid shape a site uses for a checkout it keeps by hand. The
/// declarations a placement reads filter skipped repositories out, so the
/// project's declared forest is empty and the forest is probed on disk instead
/// (§AR-004-forest.2).
fn skip_every_repository(tmp: &Path) {
    let path = tmp.join("workspaces.json");
    let mut registry: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    for repo in registry["project_types"][0]["repos"]
        .as_array_mut()
        .expect("the type declares repositories")
    {
        repo["update_mode"] = json!("skip");
    }
    fs::write(&path, serde_json::to_string_pretty(&registry).unwrap()).unwrap();
}

/// A project whose declared forest is empty answers *whole* by one question
/// rather than three (§FS-006-project-interface.8). With nothing declared there
/// is nothing to be absent, so a bare directory used to be refused by the
/// verification after the command returned and called *already checked out* by
/// the maker's own short-circuit on the very next ask — one command, two
/// answers, and the second one put a store into a directory holding no
/// repository of the project. Asked twice, the refusal holds.
#[test]
fn a_directory_holding_no_repository_of_the_project_is_refused_on_every_ask() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    skip_every_repository(tmp.path());
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let command = hollow_checkout(tmp.path());
    bind_checkout(tmp.path(), &command);
    let target = root.join("feature");

    for ask in 1..=2 {
        let refused = ephor(tmp.path())
            .args(["checkout", "--project", "demo", "--branch", "feature"])
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&refused.stdout).into_owned()
            + &String::from_utf8_lossy(&refused.stderr);
        assert!(
            !refused.status.success(),
            "ask {ask}: a directory holding no repository of demo read as a workspace: {said}"
        );
        assert!(
            said.contains("no repository of this project is in it"),
            "ask {ask}: the refusal does not say why it is not a workspace: {said}"
        );
        assert!(
            !target.join("panta").exists(),
            "ask {ask}: a plan was given somewhere to land in a workspace that was not made"
        );
    }
}

/// The roles this project's declaration gives its two repositories — a name a
/// person reads, which is what a report reaches for before it reaches for the
/// directory a program opens (§FS-011-command-line.11.2).
fn name_the_repositories(tmp: &Path, ce: &str, ee: &str) {
    let path = tmp.join("workspaces.json");
    let mut registry: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    registry["project_types"][0]["repos"][0]["role"] = json!(ce);
    registry["project_types"][0]["repos"][1]["role"] = json!(ee);
    fs::write(&path, serde_json::to_string_pretty(&registry).unwrap()).unwrap();
}

/// Everything §FS-011-command-line.11.1 forbids a terminal to be handed. The
/// assertions are about the form of what is printed and never about its
/// wording, so the sentences stay free to improve.
fn carries_no_markup(text: &str, what: &str) {
    for line in text.lines() {
        assert!(
            !line.trim_end().starts_with('#'),
            "{what} carries a markdown heading a terminal does not render: {line:?}\n{text}"
        );
        assert!(
            !line.trim_start().starts_with("```"),
            "{what} carries a fence: {line:?}\n{text}"
        );
    }
}

/// The seam this whole change can be wired backwards at: three surfaces, two
/// forms. What the command prints is prose; the file `--report` writes is the
/// markdown document it always was (§FS-011-command-line.11.1).
#[test]
fn the_terminal_is_handed_prose_while_the_report_file_stays_markdown() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    name_the_repositories(
        tmp.path(),
        "the community edition",
        "the enterprise edition",
    );
    let _ce = repo(tmp.path(), "ce");
    let _ee = repo(tmp.path(), "ee");
    let report = tmp.path().join("runtime/checkout.md");

    let made = ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .arg("--report")
        .arg(&report)
        .output()
        .unwrap();
    assert!(made.status.success(), "{made:?}");
    let printed = String::from_utf8_lossy(&made.stdout).into_owned();

    carries_no_markup(&printed, "what `ephor checkout` printed");
    // Each repository is introduced by the name its declaration gave it.
    for name in ["the community edition", "the enterprise edition"] {
        assert!(
            printed.contains(name),
            "`{name}` is never named in what the command printed:\n{printed}"
        );
    }
    assert!(root.join("feature/ce/.git").exists());

    // And the document is where it was declared to be, unchanged in kind.
    let written = fs::read_to_string(&report).unwrap();
    assert!(
        written.lines().any(|line| line.starts_with("# ")),
        "the report file lost its markdown heading:\n{written}"
    );
    assert!(
        written.lines().any(|line| line.starts_with("## ")),
        "the report file lost its per-repository heading:\n{written}"
    );
}

/// The refusal, which is the moment a reader most needs to act: git's own
/// words, kept, with nothing between them and the reader but ephor's sentence
/// — while the `report` field of `--json` goes on carrying the markdown
/// document and `repo` goes on carrying the path a program opens
/// (§FS-011-command-line.11).
#[test]
fn a_refusal_reads_as_prose_while_the_reading_keeps_the_document_and_the_path() {
    let tmp = tempdir();
    let root = fixture(tmp.path());
    name_the_repositories(
        tmp.path(),
        "the community edition",
        "the enterprise edition",
    );
    let _ce = repo(tmp.path(), "ce");
    let ee = repo(tmp.path(), "ee");
    // Nothing to look the branch up on, which is git's refusal to give.
    git(&ee, &["remote", "remove", "origin"]);

    let refused = ephor(tmp.path())
        .args(["checkout", "--project", "demo", "--branch", "feature"])
        .output()
        .unwrap();
    let printed = String::from_utf8_lossy(&refused.stdout).into_owned()
        + &String::from_utf8_lossy(&refused.stderr);
    assert!(
        printed.contains("fatal:"),
        "git did not refuse, so there is no refusal to judge:\n{printed}"
    );

    carries_no_markup(&printed, "what a refused `ephor checkout` printed");
    assert!(
        printed.contains("the enterprise edition"),
        "the refused repository is not named for its reader:\n{printed}"
    );
    for line in printed.lines().filter(|line| line.contains("fatal:")) {
        assert!(
            line.starts_with(' ') || line.starts_with('\t'),
            "git's message is not indented under the repository it refused: {line:?}\n{printed}"
        );
    }
    assert!(root.join("feature/ce/.git").exists());

    // The same outcome for a program: still the document, still the path.
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
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    let report = view["report"].as_str().unwrap();
    assert!(
        report.lines().any(|line| line.starts_with("# ")),
        "the `report` field lost its markdown heading:\n{report}"
    );
    assert!(
        report.lines().any(|line| line.starts_with("## ")),
        "the `report` field lost its per-repository heading:\n{report}"
    );
    let rows = view["repos"].as_array().unwrap();
    let names: Vec<&str> = rows.iter().filter_map(|row| row["repo"].as_str()).collect();
    assert_eq!(
        names,
        vec!["ce", "ee"],
        "the machine form stopped carrying the path a program opens: {view:#}"
    );
}
