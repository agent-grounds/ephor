//! A separate successful refresh leaves the feed's OLD row on screen. The
//! current terminal grid must identify that project's outdated display and
//! offer `r`, without fetching or replacing rows (§FS-001-forge-interface.7.1).
//! Port of issue #244's retained PTY reproducer: assert the current screen,
//! healthy newer cache and call count, rather than searching terminal history.

#[path = "../support.rs"]
mod support;

/// Python owns the PTY and terminal emulator; Cargo owns the regression and
/// its temporary world. The dependency is in tests/requirements.txt and CI.
#[test]
#[cfg(unix)]
fn issue_244_external_cache_refresh_marks_the_current_feed_screen_outdated() {
    let world = support::World::new();
    let mut driver = std::process::Command::new("python3");
    // Use the suite's shared command constructor's environment removals for
    // every ephor child, even though Python gives it its terminal streams.
    for (key, value) in support::ephor_command().get_envs() {
        match value {
            Some(value) => driver.env(key, value),
            None => driver.env_remove(key),
        };
    }
    let output = driver
        .args(["-u", "-c", include_str!("../cache_snapshot.py")])
        .arg(env!("CARGO_BIN_EXE_ephor"))
        .arg(world.path())
        .output()
        .expect("Python 3 runs the terminal scenario");
    assert!(
        output.status.success(),
        "issue #244 current-screen regression (exit {:?}):\n{}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
