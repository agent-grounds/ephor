//! The actual gate fixtures survive a child retaining their writer descriptor
//! (§REQ-003-test-isolation.5). Linux x86_64 only: LD_PRELOAD, /proc and SYS_fork.

use std::process::Command;

#[test]
fn runtime_stubs_are_ready_before_gate_operations() {
    let scratch = tempfile::tempdir().unwrap();
    let source = scratch.path().join("contention.c");
    let library = scratch.path().join("contention.so");
    let trace = scratch.path().join("trace");
    std::fs::write(&source, include_str!("fixture_contention.c")).unwrap();
    let build = Command::new("cc")
        .args(["-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&library)
        .output()
        .expect("Linux contention regression requires a C compiler");
    assert!(build.status.success(), "{build:?}");

    // Re-run the existing behavioral assertions, including the already-settled
    // exclusion fixture as a positive control. Each child creates one runner.
    let tests = [
        "work::runtime::gate::tests::a_gate_the_runner_could_not_brief_says_why",
        "work::runtime::human_gate::tests::human_gate_choices_and_files_are_read_without_moving_or_sending",
        "work::runtime::exclusion::tests::an_unreadable_entry_is_undecided_for_every_root",
    ];
    let mut failures = Vec::new();
    for test in tests {
        for contended in [false, true] {
            std::fs::write(&trace, "").unwrap();
            let mut child = Command::new(std::env::current_exe().unwrap());
            child
                .args([test, "--exact", "--nocapture"])
                .current_dir(scratch.path())
                .env_remove("LD_PRELOAD")
                .env_remove("EPHOR_271_TRACE");
            if contended {
                child
                    .env("LD_PRELOAD", &library)
                    .env("EPHOR_271_TRACE", &trace);
            }
            let output = child.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stdout.contains("running 1 test"),
                "{test}: {stdout}\n{stderr}"
            );
            let events = std::fs::read_to_string(&trace).unwrap();
            if contended {
                assert!(
                    events.contains("fork-held-writable-fd") && !events.contains("INJECTION-ERROR"),
                    "descriptor retention was not injected: {events}"
                );
            }
            if !output.status.success() {
                failures.push(format!(
                    "{test}, contention={contended}, status={}\n{stdout}\n{stderr}\n{events}",
                    output.status
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
