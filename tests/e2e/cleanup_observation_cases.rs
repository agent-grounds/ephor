//! Controlled PTY observation proofs for §FS-016-browser-opening.2.
//! The Linux reproducer supplies the actual pending-SIGKILL evidence. These
//! cases substitute an owned process for the delayed descendant, exercising
//! the same Enter-handoff observation without depending on CPU starvation.

use super::*;

struct ObservedProcess {
    child: Child,
    first_probe: Option<Instant>,
    stopped: bool,
}

impl ObservedProcess {
    fn new() -> Self {
        Self {
            child: Command::new("/bin/sleep")
                .arg("60")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("start the isolated observation fixture"),
            first_probe: None,
            stopped: false,
        }
    }

    fn probe(&mut self, pid: &str, stop_after: Option<Duration>) -> bool {
        assert_eq!(
            pid,
            self.child.id().to_string(),
            "observe the owned fixture"
        );
        let first = *self.first_probe.get_or_insert_with(Instant::now);
        // Stop only when the harness asks again after the chosen delay.
        // A single sample cannot accidentally pass because the process died
        // before the notice or while the test was starting up.
        if !self.stopped && stop_after.is_some_and(|delay| first.elapsed() >= delay) {
            self.child.kill().expect("release the delayed-stop fixture");
            assert!(self
                .child
                .wait_timeout(Duration::from_secs(2))
                .expect("reap the released fixture")
                .is_some());
            self.stopped = true;
        }
        process_alive(pid)
    }

    fn world(&self, automatic: bool) -> World {
        let body = format!(
            "printf '%s\\n' '{}' > \"$HOME/opener.pid\"; /bin/sleep 30",
            self.child.id()
        );
        if automatic {
            let world = remote_world(URL, None, None);
            opener(&world, "xdg-open", &format!("#!/bin/sh\n{body}\n"));
            world
        } else {
            remote_world(
                URL,
                Some(json!({"open": format!("{body} & exit 0 # {{url}}") })),
                None,
            )
        }
    }
}

impl Drop for ObservedProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait_timeout(Duration::from_secs(2));
    }
}

/// Exercise the actual PTY observation with an eight-second delayed stop.
/// This exceeds the seven-second notice assertion but fits the summons proof's
/// 12-second observation bound (§FS-016-browser-opening.2).
fn assert_delayed_cleanup_observation(automatic: bool) {
    let mut fixture = ObservedProcess::new();
    let world = fixture.world(automatic);
    let run = tui_with_process_probe(
        &world,
        b"jo",
        &[("DISPLAY", ":1")],
        Some(Duration::from_secs(7)),
        |pid| fixture.probe(pid, Some(Duration::from_secs(8))),
    );
    // All evidence precedes Drop: teardown cannot make cleanup pass.
    let alive_after_enter = process_alive(&fixture.child.id().to_string());
    assert!(run.output.status.success(), "{:?}", run.output);
    assert!(contains(
        &run,
        "Browser opener did not finish within 5 seconds"
    ));
    assert!(contains(&run, URL));
    assert!(run.waited_for_enter);
    assert_eq!(
        run.opener_alive_before_enter,
        Some(false),
        "delayed-stop fixture still ran before Enter (automatic={automatic})"
    );
    assert!(!alive_after_enter, "fixture still ran after Enter");
    let observation_after = fixture
        .first_probe
        .expect("PID evidence is required")
        .elapsed();
    assert!(
        observation_after < Duration::from_secs(12),
        "stopped process should return before the full bound: {observation_after:?}"
    );
    assert!(
        run.outcome_after < Duration::from_secs(7),
        "cleanup wait spent the notice budget: {:?}",
        run.outcome_after
    );
}

/// The exited-shell timeout path observes eventual stop (§FS-016-browser-opening.2).
#[test]
#[ignore = "red until #235: inherited-pipe cleanup observes eventual stop before Enter; implement removes this"]
fn cleanup_observation_waits_for_inherited_pipes_without_spending_notice_budget() {
    assert_delayed_cleanup_observation(false);
}

/// The automatic-opener timeout path uses the same observation (§FS-016-browser-opening.2).
#[test]
#[ignore = "red until #235: automatic-opener cleanup observes eventual stop before Enter; implement removes this"]
fn cleanup_observation_waits_for_automatic_opener_without_spending_notice_budget() {
    assert_delayed_cleanup_observation(true);
}

/// Waiting must reject a persistent live process at a finite deadline, rather
/// than reporting a quick snapshot as proof of bounded cleanup observation
/// (§FS-016-browser-opening.2).
#[test]
#[ignore = "red until #235: persistent live process exhausts bounded cleanup observation; implement removes this"]
fn cleanup_observation_rejects_a_persistent_live_process_at_the_bound() {
    let mut fixture = ObservedProcess::new();
    let world = fixture.world(false);
    let run = tui_with_process_probe(
        &world,
        b"jo",
        &[("DISPLAY", ":1")],
        Some(Duration::from_secs(7)),
        |pid| fixture.probe(pid, None),
    );
    let observation_after = fixture
        .first_probe
        .expect("PID evidence is required")
        .elapsed();
    let alive_after_enter = process_alive(&fixture.child.id().to_string());
    assert!(run.output.status.success(), "{:?}", run.output);
    assert_eq!(
        run.opener_alive_before_enter,
        Some(true),
        "a live process must be rejected as uncleaned"
    );
    assert!(alive_after_enter, "teardown must not establish termination");
    assert!(observation_after >= Duration::from_secs(12), "persistent process was sampled once instead of exhausting the stop bound: {observation_after:?}");
    // Allows separate before/after-Enter observation bounds and PTY shutdown.
    assert!(
        observation_after < Duration::from_secs(30),
        "unbounded cleanup observation: {observation_after:?}"
    );
    assert!(
        run.outcome_after < Duration::from_secs(7),
        "cleanup wait spent the notice budget: {:?}",
        run.outcome_after
    );
}
