//! What every `git` and every `ephor` the suite under `tests/` spawns is built
//! without: git's repository variables, taken off both
//! (§REQ-003-test-isolation.3), and ephor's own flag spellings, taken off every
//! `ephor` (§REQ-003-test-isolation.4).
//!
//! The one place under `tests/` that does this (§REQ-003-test-isolation.3.4,
//! §REQ-003-test-isolation.4.4). Both trees pull it in with `#[path]`:
//! `tests/e2e/support.rs` for every e2e case, and
//! `tests/integration/common/mod.rs` for every integration test. A hook runs
//! the suite with a `GIT_DIR` naming the repository being committed to. A
//! case's own `git` would act on that repository whatever its `-C` says, and so
//! would the `ephor` it spawns, which is not a test build and passes on
//! whatever it is handed (§REQ-003-test-isolation.3.3). A caller runs the suite
//! with whatever it exported, and `ephor checkout`, `ephor rebase` and the
//! rebase sweep read a flag the case did not pass from its environment
//! spelling: a rhei agent's relative `REPORT` would receive a case's report,
//! and an inherited `ITEM` would turn the case into a refusal.
//!
//! The same holds for the library a case links: under `tests/` it is not a
//! test build either, so `ephor::git::command()` takes nothing off there, and
//! `ephor::given` reads this process's own environment. A case calls the
//! library in-process only for what runs no `git` and reads no flag spelling
//! (§REQ-003-test-isolation.4.3), and reaches anything that does through
//! [`ephor_command`].

// Every case and every integration test compiles this separately and uses the
// part it needs; the rest would read as dead code under `RUSTFLAGS=-D warnings`.
#![allow(dead_code)]

use std::ffi::{OsStr, OsString};
use std::process::Command;

/// Take every `GIT_*` off `command` that would otherwise reach the child: the
/// ones this process inherited, and any already staged on the command. What a
/// caller sets afterwards is the deliberate value and is kept
/// (§REQ-003-test-isolation.3.2). Only the child is changed, never the process,
/// because a process-wide `remove_var` races every other test thread
/// (§REQ-003-test-isolation.3.4).
pub fn remove_git_variables(command: &mut Command) -> &mut Command {
    // Both lists are collected first, because `get_envs` borrows the command.
    let staged: Vec<OsString> = command
        .get_envs()
        .map(|(key, _)| key.to_os_string())
        .filter(|key| is_git_variable(key))
        .collect();
    let inherited: Vec<OsString> = std::env::vars_os()
        .map(|(key, _)| key)
        .filter(|key| is_git_variable(key))
        .collect();
    for key in staged.into_iter().chain(inherited) {
        command.env_remove(key);
    }
    command
}

/// The prefix, not a list of names. That is what covers
/// `GIT_CONFIG_PARAMETERS`, which carries an outer `git -c` down the same
/// inheritance (§REQ-003-test-isolation.3.1).
fn is_git_variable(key: &OsStr) -> bool {
    key.as_encoded_bytes().starts_with(b"GIT_")
}

/// Every `git` a case or an integration test runs itself: the one place under
/// `tests/` that names git as a program (§REQ-003-test-isolation.3.4).
pub fn git_command() -> Command {
    let mut command = Command::new("git");
    remove_git_variables(&mut command);
    command
}

/// The `ephor` binary under test, with nothing of git's repository variables
/// for it to pass on to the `git` it runs (§REQ-003-test-isolation.3.3), and
/// none of ephor's own flag spellings for it to read in place of a flag the
/// case did not pass (§REQ-003-test-isolation.4).
///
/// Every spelling is removed whether this process holds it or not, so the
/// removal is on the command to be read back, and before the caller's own
/// `.env(...)`, so a spelling a case sets on purpose is the one that reaches the
/// child (§REQ-003-test-isolation.4.2). The names are `given`'s own list
/// (§REQ-003-test-isolation.4.1), and only the child is changed, never this
/// process (§REQ-003-test-isolation.4.4).
pub fn ephor_command() -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("ephor"));
    remove_git_variables(&mut command);
    for name in ephor::given::ENVIRONMENT_SPELLINGS {
        command.env_remove(name);
    }
    command
}
