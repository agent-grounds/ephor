//! A case re-run as its caller would run it with ephor's own flag spellings
//! exported (§REQ-003-test-isolation.4).
//!
//! `ephor checkout`, `ephor rebase` and the rebase sweep read each flag from an
//! environment variable when the flag is absent (§FS-011-command-line.9). A
//! rhei agent exports a relative `REPORT` to everything it runs, and a caller
//! may export any of the rest for its own reasons. Both trees pull this in with
//! `#[path]` beside `git_env.rs`: `tests/e2e/support.rs` and
//! `tests/integration/common/mod.rs`.

// Every case and every integration test compiles this separately, and most use
// none of it.
#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

use super::git_env::remove_git_variables;

/// Where the caller's `REPORT` points: relative, as a rhei agent's is, so it
/// resolves under whatever directory the `ephor` that reads it runs in.
pub const CALLERS_REPORT: &str = "out/callers-report.md";

/// Every flag spelling ephor reads, each with a value that changes the run of a
/// case that does not set it itself.
fn callers_spellings(dir: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("REPORT", CALLERS_REPORT.to_string()),
        // A sweep refuses it beside a selector, and one checkout reads it as
        // "onto the published copy".
        ("UPSTREAM", "1".to_string()),
        ("PROJECT", "elsewhere".to_string()),
        (
            "CHECKOUT",
            dir.join("nowhere").to_string_lossy().into_owned(),
        ),
        ("ITEM", "github-issues:o/r#1".to_string()),
        ("HAND", "elsewhere".to_string()),
        ("ONTO", "elsewhere".to_string()),
        // A placeholder nothing filled is refused wherever it is read.
        ("BRANCH", "{meta.branch}".to_string()),
        ("FROM", "no-such-base".to_string()),
    ]
}

/// Re-runs `tests` of the calling binary as a caller that exported every one of
/// ephor's flag spellings runs them, from a working directory of its own, and
/// holds §REQ-003-test-isolation.4: they pass, and nothing is written at the
/// caller's `REPORT`.
pub fn holds_with_the_callers_flag_spellings(tests: &[&str]) {
    let temp = tempfile::tempdir().expect("a working directory for the caller");
    let mut child = Command::new(std::env::current_exe().expect("this binary"));
    remove_git_variables(&mut child)
        .arg("--exact")
        .args(tests)
        .current_dir(temp.path());
    for (name, value) in callers_spellings(temp.path()) {
        child.env(name, value);
    }
    let output = child.output().expect("the binary re-runs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains(&format!("running {} tests", tests.len())),
        "the child ran something other than {tests:?}:\n{stdout}"
    );
    let mut wrong: Vec<String> = Vec::new();
    let report = temp.path().join(CALLERS_REPORT);
    if let Ok(written) = std::fs::read_to_string(&report) {
        let first = written.lines().next().unwrap_or_default();
        wrong.push(format!(
            "an ephor a test spawned wrote its report to the caller's REPORT, {CALLERS_REPORT}: \
             {first}"
        ));
    }
    if !output.status.success() {
        wrong.push(format!(
            "the tests fail with the caller's flag spellings inherited, and pass without them:\n\
             {stdout}"
        ));
    }
    assert!(
        wrong.is_empty(),
        "every ephor the suite causes runs without ephor's own flag spellings \
         (§REQ-003-test-isolation.4):\n{}",
        wrong.join("\n")
    );
}
