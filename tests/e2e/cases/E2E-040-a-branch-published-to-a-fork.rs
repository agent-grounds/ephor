//! E2E-040-a-branch-published-to-a-fork: a branch pushed to a fork is read
//! against its copy there.
//!
//! The scenario is a project its contributor can no longer push branches to.
//! They work the usual fork way: the checkout fetches from the project and
//! pushes to their own fork, and git is told so once, with
//! `remote.pushDefault`. Two branches stand in the same position. `fix/direct`
//! was pushed straight to the project before the lock; `fix/typo` went to the
//! fork, the only way left. Each carries one commit not pushed yet, and each
//! has a reviewer's fixup on its published copy that the checkout does not
//! have.
//!
//! What the scenario holds ephor to. The branch row says how far `fix/typo`
//! trails `fork/fix/typo`, exactly as it says how far `fix/direct` trails
//! `origin/fix/direct`: what was last pushed of a branch is its copy wherever
//! git records it was pushed (§FS-004-quick-actions.8). The rebase onto that
//! copy is offered and names it, and pressing it replays onto the fork as the
//! fork has it now, a fixup pushed since the last fetch included. And the
//! distance from main is still counted on the project's own remote, because
//! that is where main lives (§FS-004-quick-actions.6,
//! §FS-004-quick-actions.7.4): the fork's `main` is a commit behind it.

#[path = "../support.rs"]
mod support;

use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

use support::*;

/// Pushed straight to the project, the way branches went before the lock.
const DIRECT: &str = "fix/direct";

/// Pushed to the fork, the only way left.
const TYPO: &str = "fix/typo";

/// A git command that has to work for the world to be the world.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

/// One commit adding `file`, whose name says what it is.
fn commit(dir: &Path, file: &str) {
    std::fs::write(dir.join(file), format!("{file}\n")).expect("write the file");
    git(dir, &["add", file]);
    git(dir, &["commit", "-q", "-m", file]);
}

/// Somebody else's commit on `branch` of a remote this world keeps as a
/// working repository, made without the contributor's checkout.
fn pushed_by_a_reviewer(remote: &Path, branch: &str, file: &str) {
    git(remote, &["checkout", "-q", branch]);
    commit(remote, file);
    git(remote, &["checkout", "-q", "main"]);
}

/// The project, its fork, and the contributor's checkouts of it: `main` and
/// one working tree per branch, all of them fetching from `origin` and pushing
/// to `fork`.
fn a_contributor_working_through_a_fork() -> World {
    let world = World::new();
    world.register(json!({
        "clone_mode": "worktree",
        "branch_root_template": "{project_root}/{branch}"
    }));

    let upstream = world.path().join("upstream");
    std::fs::create_dir_all(&upstream).expect("the project");
    git(&upstream, &["init", "-q", "--initial-branch=main"]);
    commit(&upstream, "README.md");
    let fork = world.path().join("fork");
    git(
        world.path(),
        &["clone", "-q", &upstream.to_string_lossy(), "fork"],
    );

    let main = world.forest().join("main");
    std::fs::create_dir_all(world.forest()).expect("the project root");
    git(
        &world.forest(),
        &["clone", "-q", &upstream.to_string_lossy(), "main"],
    );
    // The rebase ephor runs commits in these trees, under a home with no
    // identity of its own; its worktrees share this config.
    git(&main, &["config", "user.email", "t@example.com"]);
    git(&main, &["config", "user.name", "t"]);
    git(&main, &["remote", "add", "fork", &fork.to_string_lossy()]);
    git(&main, &["fetch", "-q", "fork"]);
    git(&main, &["config", "remote.pushDefault", "fork"]);

    for branch in [DIRECT, TYPO] {
        let tree = world.forest().join(branch);
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "--no-track",
                "-b",
                branch,
                &tree.to_string_lossy(),
                "origin/main",
            ],
        );
    }
    let direct = world.forest().join(DIRECT);
    let typo = world.forest().join(TYPO);
    commit(&direct, "direct-1.txt");
    git(&direct, &["push", "-q", "origin", DIRECT]);
    commit(&typo, "typo-1.txt");
    git(&typo, &["push", "-q", "fork", TYPO]);

    pushed_by_a_reviewer(&upstream, DIRECT, "direct-fixup.txt");
    pushed_by_a_reviewer(&fork, TYPO, "typo-fixup.txt");
    // The project's main moves after the fork was taken, so the two remotes'
    // `main` are a commit apart.
    commit(&upstream, "main-moves.txt");

    commit(&direct, "direct-2.txt");
    commit(&typo, "typo-2.txt");
    // Nothing under the watch fetches (§FS-004-quick-actions.6); this is the
    // fetch the contributor made.
    git(&main, &["fetch", "-q", "origin"]);
    git(&main, &["fetch", "-q", "fork"]);
    world
}

/// The branch rows, as a reader reads them.
fn rows(world: &World) -> String {
    let output = world
        .ephor_raw()
        .args(["branches", PROJECT])
        .output()
        .expect("ran");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf-8")
}

/// The row for `branch`.
fn row<'a>(rows: &'a str, branch: &str) -> &'a str {
    rows.lines()
        .find(|line| line.split_whitespace().any(|word| word == branch))
        .unwrap_or_else(|| panic!("no row for {branch}:\n{rows}"))
}

/// What is offered on `branch`, as `(id, description)`.
fn offers(world: &World, branch: &str) -> Vec<(String, String)> {
    let output = world
        .ephor_raw()
        .args([
            "actions",
            "list",
            "--project",
            PROJECT,
            "--branch",
            branch,
            "--json",
        ])
        .output()
        .expect("ran");
    let menu: Value = json_of(&output);
    menu["offers"]
        .as_array()
        .unwrap_or_else(|| panic!("no offers in {menu}"))
        .iter()
        .map(|offer| {
            (
                offer["id"].as_str().unwrap_or_default().to_string(),
                offer["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect()
}

#[test]
fn a_branch_pushed_to_the_fork_says_how_far_behind_its_copy_there_it_is() {
    let world = a_contributor_working_through_a_fork();
    let rows = rows(&world);

    // The branch pushed the old way reads as it always has.
    let direct = row(&rows, DIRECT);
    assert!(direct.contains("1 behind origin/fix/direct"), "{rows}");
    assert!(direct.contains("1 behind main"), "{rows}");

    let typo = row(&rows, TYPO);
    assert!(
        typo.contains("1 behind fork/fix/typo"),
        "`{TYPO}` was pushed to the fork, which git records, and a reviewer moved it on there; \
         the row reads it as never pushed:\n{rows}"
    );
    assert!(
        typo.contains("1 behind main"),
        "main moved on the project and not on the fork, so it is one behind:\n{rows}"
    );
}

#[test]
fn a_branch_pushed_to_the_fork_is_offered_the_rebase_onto_its_copy_there() {
    let world = a_contributor_working_through_a_fork();

    let direct = offers(&world, DIRECT);
    assert!(
        direct
            .iter()
            .any(|(id, said)| id == "rebase-upstream" && said.contains("origin/fix/direct")),
        "{direct:?}"
    );

    let typo = offers(&world, TYPO);
    let onto_copy = typo
        .iter()
        .find(|(id, _)| id == "rebase-upstream")
        .unwrap_or_else(|| {
            panic!("`{TYPO}` trails its copy on the fork and is not offered the rebase onto it: {typo:?}")
        });
    assert!(onto_copy.1.contains("fork/fix/typo"), "{onto_copy:?}");
}

#[test]
fn the_rebase_onto_a_copy_on_the_fork_lands_on_the_fork_as_it_is_now() {
    let world = a_contributor_working_through_a_fork();
    // A second fixup lands on the fork after the contributor's last fetch.
    pushed_by_a_reviewer(&world.path().join("fork"), TYPO, "typo-fixup-2.txt");
    let typo = world.forest().join(TYPO);

    world
        .ephor()
        .args([
            "rebase",
            "--upstream",
            "--project",
            PROJECT,
            "--checkout",
            &typo.to_string_lossy(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("fork/fix/typo"))
        .stdout(predicates::str::contains("Replayed onto"));

    for file in [
        "typo-fixup.txt",
        "typo-fixup-2.txt",
        "typo-1.txt",
        "typo-2.txt",
    ] {
        assert!(
            typo.join(file).exists(),
            "{file} is not in {}",
            typo.display()
        );
    }
    // Main's move is the other rebase's.
    assert!(!typo.join("main-moves.txt").exists());
}
