//! E2E-088-the-screen-says-what-it-reads: `ephor tui` on a real terminal is on
//! the screen before its session has loaded, and says what it is reading
//! (§FS-019-waiting.5.1). A `git` on the world's `PATH` holds every branch
//! measurement until the case opens a gate, so the load cannot finish before
//! the case lets it: the first frame comes all the same, names the step it is
//! held in with the checkouts it counts, and draws nothing a reading would
//! bring. Opening the gate brings the Waiting screen (§FS-019-waiting.2).
//! `q` quits while the load is held and leaves the terminal as it found it,
//! and a load that fails says why with the sentence and the exit code
//! `ephor waiting` gives in the same world. Python's pty and pyte are the
//! test-only scaffold; the ephor binary is the real path under test.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use support::*;

const BRANCHES: [&str; 2] = ["feature-1", "feature-2"];

/// The matter the cached feed holds, which no frame before the load may show.
const TITLE: &str = "A matter the cache remembers";

/// What the frame says while the measurements are held.
const HELD: &str = "Reading the branches of 2 checkouts — 0 done…";

fn git(dir: &Path, args: &[&str]) {
    let output = git_command()
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

/// A project with two branch checkouts on disk and a cached feed holding one
/// matter, then a `git` that holds every branch measurement until the gate
/// this returns exists.
fn world() -> (World, PathBuf) {
    let world = World::new();
    world.register(json!({
        "branch_root_template": "{project_root}/{branch}",
        "branches": BRANCHES.iter().map(|branch| {
            json!({ "id": branch, "branch": branch, "active": true })
        }).collect::<Vec<_>>()
    }));
    for branch in BRANCHES {
        let repo = world.forest().join(branch);
        fs::create_dir_all(&repo).expect("the checkout");
        git(&repo, &["init", "-q", "--initial-branch=main"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "base"]);
        git(&repo, &["checkout", "-q", "-b", branch]);
    }
    world.file("status.txt", &format!("{TITLE}\n"));
    world.configure(json!({ "projects": { PROJECT: { "providers": [
        { "provider": "custom-status", "command": "cat status.txt" }
    ] } } }));
    world.ephor().args(["refresh", PROJECT]).assert().success();

    let real = std::process::Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("the real git is found");
    let real = String::from_utf8_lossy(&real.stdout).trim().to_string();
    assert!(!real.is_empty(), "a git is on PATH");
    let gate = world.path().join("gate");
    // At most thirty seconds held, so a run that never opens the gate
    // leaves nothing waiting behind it.
    world.stub(
        "git",
        &format!(
            "#!/bin/sh\ncase \" $* \" in\n  *\" for-each-ref \"*refs/heads*)\n    i=0\n    \
             while [ ! -e '{gate}' ] && [ \"$i\" -lt 600 ]; do sleep 0.05; i=$((i + 1)); done ;;\n\
             esac\nexec '{real}' \"$@\"\n",
            gate = gate.display(),
        ),
    );
    (world, gate)
}

/// Drive `ephor tui` through `steps` on a `cols` by 24 terminal and return
/// each step's screen by name, with the program's exit status.
#[cfg(unix)]
fn drive(world: &World, cols: u16, steps: serde_json::Value) -> (Vec<(String, String)>, i32) {
    let ephor = world.ephor_raw();
    let mut driver = std::process::Command::new("python3");
    // The removals too, so the `ephor` the driver starts inherits no `GIT_*`
    // (§REQ-003-test-isolation.3.3).
    for (key, value) in ephor.get_envs() {
        match value {
            Some(value) => driver.env(key, value),
            None => driver.env_remove(key),
        };
    }
    let output = driver
        .args(["-u", "-c", include_str!("../monitors_screen.py")])
        .arg(ephor.get_program())
        .args([cols.to_string(), "24".to_string()])
        .arg(steps.to_string())
        .output()
        .expect("Python 3 runs the terminal scenario");
    let printed = String::from_utf8_lossy(&output.stdout);
    let screens: Vec<(String, String)> = printed
        .split("=== ")
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (name, screen) = part.split_once('\n').unwrap_or((part, ""));
            (name.to_string(), screen.to_string())
        })
        .collect();
    let code = output.status.code().unwrap_or_else(|| {
        panic!(
            "the terminal scenario was killed:\n{printed}\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert!(
        code != 2 && code != 124,
        "a screen never came:\n{printed}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (screens, code)
}

fn screen<'a>(screens: &'a [(String, String)], name: &str) -> &'a str {
    &screens
        .iter()
        .find(|(each, _)| each == name)
        .unwrap_or_else(|| panic!("no screen {name}: {screens:?}"))
        .1
}

#[cfg(unix)]
#[test]
fn the_first_frame_says_what_is_read_before_the_load_can_finish() {
    let (world, gate) = world();
    let (screens, code) = drive(
        &world,
        80,
        json!([
            ["loading", "", HELD],
            ["loaded", "", "overview: ", gate.to_string_lossy()],
            ["quit", "q", null]
        ]),
    );
    assert_eq!(code, 0);

    let loading = screen(&screens, "loading");
    let lines: Vec<&str> = loading.lines().collect();
    assert_eq!(lines[0], " ephor", "{loading}");
    assert_eq!(lines[1], format!("  {HELD}"), "{loading}");
    assert!(loading.contains(" q quit"), "{loading}");
    for brought in [TITLE, "overview: ", "No cached data", "Nothing is waiting"] {
        assert!(
            !loading.contains(brought),
            "nothing a reading brings is drawn before it lands ({brought}):\n{loading}"
        );
    }

    let loaded = screen(&screens, "loaded");
    assert!(loaded.starts_with(" ephor — waiting"), "{loaded}");
    assert!(loaded.contains("overview: 1 matter"), "{loaded}");
    assert!(!loaded.contains("Reading the"), "{loaded}");
    assert_eq!(screen(&screens, "restored").trim(), "yes");
}

#[cfg(unix)]
#[test]
fn q_quits_while_the_load_is_held_and_leaves_the_terminal_as_it_found_it() {
    let (world, gate) = world();
    let (screens, code) = drive(
        &world,
        80,
        json!([["loading", "", HELD], ["quit", "q", null]]),
    );
    // Let the measurements the quit abandoned go.
    fs::write(&gate, "").expect("the gate opens");
    assert_eq!(code, 0, "{screens:?}");
    assert_eq!(screen(&screens, "restored").trim(), "yes");
}

#[cfg(unix)]
#[test]
fn a_load_that_fails_says_why_as_a_command_does_with_its_exit_code() {
    let (world, gate) = world();
    fs::write(&gate, "").expect("nothing is held");
    let seen = world.path().join("state/ephor/seen.json");
    fs::create_dir_all(seen.parent().expect("the state directory")).expect("state");
    fs::write(&seen, "not json").expect("a seen file nothing can read");
    let command = world.ephor().arg("waiting").output().expect("ephor runs");
    let said = String::from_utf8_lossy(&command.stderr).trim().to_string();
    assert!(said.contains("Corrupt seen file"), "{said}");

    let (screens, code) = drive(&world, 200, json!([["failed", "", null]]));
    assert_eq!(Some(code), command.status.code(), "{screens:?}");
    let failed = screen(&screens, "failed");
    assert!(
        failed.contains(&said),
        "the same sentence:\n{failed}\n{said}"
    );
    assert_eq!(screen(&screens, "restored").trim(), "yes");
}
