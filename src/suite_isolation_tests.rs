//! The library's own tests, re-run as a git hook runs them (§REQ-003-test-isolation.2).
//!
//! The pre-commit hook runs `cargo test`, and git exports `GIT_DIR` and
//! `GIT_INDEX_FILE` to it. Every `git` a test causes inherits them, so a
//! fixture's `git init` and `git config` and the code under test's `git
//! worktree add` act on the repository being committed to. This module stands
//! a decoy in for that repository: a main checkout with a linked worktree,
//! named to a child copy of this test binary exactly as a hook committing from
//! the worktree names it, and then read back. A repository no test named keeps
//! its configuration, its refs, its `HEAD`s and its index
//! (§REQ-003-test-isolation.1).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Fixtures that reach git each way §REQ-003-test-isolation.3.3 lists for a
/// unit test: a test helper (`run_in`, `git_in`, the TUI's `git`) and the code
/// under test (`git::create`'s `git worktree add`, the forest's probes).
const FIXTURES: &[&str] = &[
    "git::tests::a_workspace_is_grown_from_the_remote_the_repository_actually_has",
    "forest::tests::a_tracked_branch_stands_against_its_recorded_copy",
    "feed::tui::tests::behind_sums_across_workspace_repos",
];

/// Every `GIT_*` this process holds, so the child the test hands the decoy to
/// starts from nothing inherited.
fn inherited_git_vars() -> Vec<OsString> {
    std::env::vars_os()
        .map(|(key, _)| key)
        .filter(|key| key.to_string_lossy().starts_with("GIT_"))
        .collect()
}

/// A `git` the test runs on the decoy on purpose, aimed by `-C` with nothing
/// inherited to override it: the library's own constructor takes that off
/// (§REQ-003-test-isolation.3.4).
fn decoy_git(dir: &Path, args: &[&str]) -> String {
    let output = crate::git::command()
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} on the decoy at {} failed: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// A main checkout with one commit and a linked worktree `wt` on `topic`,
/// holding one staged file — the shape of the checkout a hook commits from.
fn build_decoy(root: &Path) -> (PathBuf, PathBuf) {
    let main = root.join("main");
    let wt = root.join("wt");
    std::fs::create_dir_all(&main).unwrap();
    decoy_git(&main, &["init", "-q", "-b", "main"]);
    std::fs::write(main.join("README.md"), "decoy\n").unwrap();
    decoy_git(&main, &["add", "README.md"]);
    decoy_git(
        &main,
        &[
            "-c",
            "user.name=decoy",
            "-c",
            "user.email=decoy@example.invalid",
            "commit",
            "-q",
            "-m",
            "decoy",
        ],
    );
    decoy_git(&main, &["worktree", "add", "-q", "-b", "topic", "../wt"]);
    std::fs::write(wt.join("change.txt"), "the commit under test\n").unwrap();
    decoy_git(&wt, &["add", "change.txt"]);
    (main, wt)
}

/// Everything §REQ-003-test-isolation.1 measures, read without a hook's variables.
fn decoy_state(main: &Path, wt: &Path) -> Vec<String> {
    vec![
        decoy_git(main, &["config", "--local", "--list"]),
        decoy_git(main, &["for-each-ref", "--format=%(refname) %(objectname)"]),
        decoy_git(main, &["symbolic-ref", "HEAD"]),
        decoy_git(wt, &["symbolic-ref", "HEAD"]),
        decoy_git(wt, &["ls-files", "--stage"]),
    ]
}

/// §REQ-003-test-isolation.2: the fixtures pass under a hook's variables, and
/// the repository those variables name is left as it was.
#[test]
fn unit_fixtures_under_a_worktree_hook_pass_and_leave_its_repository_alone() {
    let temp = tempfile::tempdir().unwrap();
    let (main, wt) = build_decoy(temp.path());
    let before = decoy_state(&main, &wt);

    // What git exports to a hook run from a linked worktree: both absolute.
    let git_dir = main.join(".git").join("worktrees").join("wt");
    let mut child = Command::new(std::env::current_exe().unwrap());
    for key in inherited_git_vars() {
        child.env_remove(key);
    }
    let output = child
        .arg("--exact")
        .args(FIXTURES)
        .env("GIT_DIR", &git_dir)
        .env("GIT_INDEX_FILE", git_dir.join("index"))
        .current_dir(&wt)
        .output()
        .expect("the test binary re-runs");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let after = decoy_state(&main, &wt);

    assert!(
        stdout.contains(&format!("running {} tests", FIXTURES.len())),
        "the child ran something other than {FIXTURES:?}:\n{stdout}"
    );
    assert_eq!(
        before,
        after,
        "a test's git wrote into {}, a repository no test named (§REQ-003-test-isolation.1)",
        main.display()
    );
    assert!(
        output.status.success(),
        "the fixtures fail under a worktree hook's GIT_DIR and GIT_INDEX_FILE, and pass \
         without them (§REQ-003-test-isolation.2):\n{stdout}"
    );
}

/// §REQ-003-test-isolation.3.4: no file in `src/` or `tests/` names git as a
/// program except the two places that apply the removal: the library's
/// constructor, and the one helper under `tests/`. A call site corrected in
/// place leaves the next caller the same mistake to make.
#[test]
fn only_the_constructor_and_the_test_helper_name_git_as_a_program() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let allowed = [
        root.join("src").join("git.rs"),
        root.join("tests").join("shared").join("git_env.rs"),
    ];
    // Spelled rather than written, so that this file does not count itself.
    let needle = format!("Command::new({:?})", "git");
    let mut offenders: Vec<String> = Vec::new();
    for path in rust_sources_under(&root.join("src"))
        .into_iter()
        .chain(rust_sources_under(&root.join("tests")))
    {
        if allowed.contains(&path) {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a source is readable");
        let count = text.matches(needle.as_str()).count();
        if count > 0 {
            let shown = path.strip_prefix(root).unwrap_or(&path).display();
            offenders.push(format!("{shown} ({count})"));
        }
    }
    offenders.sort();
    assert!(
        offenders.is_empty(),
        "these files spawn `git` without the removal, so each inherits whatever repository a \
         hook's `GIT_DIR` names (§REQ-003-test-isolation.3): {offenders:?}. Build the command \
         with `crate::git::command()` in `src/`, or `git_command()` under `tests/`."
    );
}

/// Every `.rs` file under `dir`, so the guard reads the tree rather than a list
/// someone has to keep up to date.
fn rust_sources_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }
    found
}
