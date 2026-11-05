//! Every configured block and discovered store contributes to its provider's
//! complete answer (§FS-001-forge-interface.6.1). Port of issue #253's forge
//! reproducer, extended to site placement, failures and whole-slot last-good.

#[path = "../support.rs"]
mod support;

fn scenario(name: &str) {
    let world = support::World::new();
    let mut driver = std::process::Command::new("python3");
    for (key, value) in world.ephor_raw().get_envs() {
        match value {
            Some(value) => driver.env(key, value),
            None => driver.env_remove(key),
        };
    }
    let output = driver
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/e2e/repeated_providers.py"
        ))
        .arg(env!("CARGO_BIN_EXE_ephor"))
        .arg(world.path())
        .arg(name)
        .output()
        .expect("Python runs the offline contribution scenario");
    assert!(
        output.status.success(),
        "{name} (exit {:?}):\n{}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
fn issue_253_project_answers() {
    scenario("project_answers");
}

#[test]
fn issue_253_site_answers() {
    scenario("site_answers");
}

/// Thin and detailed site reports must share their reasons and placement
/// evidence before the subject is placed (§FS-001-forge-interface.6.1).
#[test]
fn issue_253_site_overlap_evidence() {
    scenario("site_overlap_evidence");
}

#[test]
fn issue_253_project_failures() {
    scenario("project_failures");
}

#[test]
fn issue_253_site_failures() {
    scenario("site_failures");
}

#[test]
fn issue_253_project_last_good() {
    scenario("project_last_good");
}

#[test]
fn issue_253_site_last_good() {
    scenario("site_last_good");
}

#[test]
fn issue_253_task_stores() {
    scenario("task_stores");
}
