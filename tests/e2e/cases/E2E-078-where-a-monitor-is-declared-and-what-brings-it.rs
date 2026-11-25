//! E2E-078-where-a-monitor-is-declared-and-what-brings-it: a project's
//! manifest declares monitors beside the site's, a row that trusts only
//! descriptions runs none of the manifest's commands, a custom-status answer's
//! samples bring their monitors, a declaration that breaks a rule costs that
//! monitor alone, is noted, and is named by `validate --manifest`, and the
//! site cannot name the monitor slot as a provider block
//! (§FS-021-monitors.4.1, §FS-021-monitors.4.5,
//! §FS-021-monitors.4.6, §FS-021-monitors.8.3, §FS-021-monitors.17.4).

#[path = "../support.rs"]
mod support;

use serde_json::json;

use support::*;

const KPIS: &str = r#"#!/bin/sh
echo "$EPHOR_MONITORS" >> "$HOME/runs.log"
printf '%s' '{"v":1,"monitors":[{"id":"coverage","value":81},{"id":"lint","value":0}]}' > "$EPHOR_ANSWER"
"#;

fn runs(world: &World) -> Vec<String> {
    std::fs::read_to_string(world.path().join("runs.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn kept(world: &World) -> serde_json::Value {
    read_json(
        &world
            .path()
            .join("state/ephor/monitors")
            .join(PROJECT)
            .join("monitors.json"),
    )
}

fn declaring_world() -> World {
    let world = World::new();
    world.script("scripts/kpis.sh", KPIS);
    world.manifest(json!({ "monitors": [
        { "id": "coverage", "healthy": { "min": 80 }, "from": { "command": "scripts/kpis.sh" } }
    ] }));
    world.configure(json!({ "projects": { PROJECT: { "monitors": [
        { "id": "lint", "healthy": { "max": 0 }, "from": { "command": "scripts/kpis.sh" } }
    ] } } }));
    world
}

#[test]
fn the_manifest_and_the_site_declare_monitors_together() {
    let world = declaring_world();
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert_eq!(runs(&world), vec!["coverage,lint"]);
    let kept = kept(&world);
    assert_eq!(
        kept["monitors"]["coverage"]["last_attempt"]["found"],
        json!("reading")
    );
    assert_eq!(
        kept["monitors"]["lint"]["last_attempt"]["found"],
        json!("reading")
    );
}

#[test]
fn a_row_that_trusts_only_descriptions_runs_no_manifest_command() {
    let world = declaring_world();
    world.register(json!({ "manifest_trust": "descriptions" }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert_eq!(runs(&world), vec!["lint"]);
    assert!(kept(&world)["monitors"]["coverage"]["last_attempt"].is_null());
}

#[test]
fn a_custom_status_answer_brings_its_monitors() {
    let world = World::new();
    world.stub(
        "status-reporter",
        "#!/bin/sh\nprintf '{\"v\":1,\"monitors\":[{\"id\":\"queue\",\"value\":4}]}' > \"$EPHOR_ANSWER\"\n",
    );
    world.configure(json!({ "projects": { PROJECT: { "providers": [
        { "provider": "custom-status", "command": "status-reporter", "format": "answer" }
    ] } } }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    let record = &kept(&world)["monitors"]["queue"];
    assert_eq!(record["owner"], json!("custom_status"), "{record}");
    assert_eq!(
        record["last_attempt"]["found"],
        json!("reading"),
        "{record}"
    );
    assert_eq!(world.feed()["providers"]["monitor"]["ok"], json!(true));
}

#[test]
fn a_declaration_that_breaks_a_rule_costs_that_monitor_alone() {
    let world = declaring_world();
    world.manifest(json!({ "monitors": [
        { "id": "coverage", "healthy": { "min": 80 }, "from": { "command": "scripts/kpis.sh" } },
        { "id": "coverage", "healthy": { "min": 70 }, "from": { "command": "scripts/kpis.sh" } }
    ] }));
    let out = world
        .ephor()
        .args(["refresh", PROJECT, "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let notes = json_of(&out)["projects"][0]["notes"].to_string();
    assert!(notes.contains("manifest monitor coverage"), "{notes}");
    assert_eq!(runs(&world), vec!["lint"]);
}

#[test]
fn the_site_cannot_name_the_monitor_slot_a_provider() {
    let world = World::new();
    world.configure(
        json!({ "projects": { PROJECT: { "providers": [{ "provider": "monitor" }] } } }),
    );
    let out = world.ephor().args(["refresh", PROJECT]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("monitor"));
}

#[test]
fn validate_names_a_manifest_monitor_that_is_not_read() {
    let world = declaring_world();
    world.manifest(json!({ "monitors": [{ "healthy": { "min": 80 } }] }));
    let manifest = world.forest().join("ephor.json");
    let out = world
        .ephor()
        .args(["validate", "--manifest"])
        .arg(&manifest)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("declares monitors that are not read"),
        "{said}"
    );
    assert!(said.contains("monitors[0]"), "{said}");
}

#[test]
fn a_site_declaration_of_the_wrong_shape_refuses_the_configuration() {
    let world = World::new();
    world.configure(json!({ "projects": { PROJECT: { "monitors": [{ "id": "x", "from": 3 }] } } }));
    let out = world.ephor().args(["refresh", PROJECT]).output().unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("monitors"), "{said}");
}
