//! E2E-087-what-a-monitors-matter-is-proposed: a drop whose monitor names an
//! `action` no entry hands work over is passed over, the detail view says
//! why, and the matter is proposed what any matter is
//! (§FS-021-monitors.16.1, §FS-021-monitors.16.2). An unknown matter is New
//! with that proposal too, never the monitor's own workflow, because what it
//! needs is the read mended (§FS-021-monitors.12.4, §FS-021-monitors.16.2).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

fn world(monitor: Value) -> World {
    let world = World::new();
    world.configure(json!({ "projects": { PROJECT: {
        "providers": [],
        "monitors": [monitor]
    } } }));
    world
}

fn kpis(world: &World, answer: &str, exit: i32) {
    world.script(
        "scripts/kpis.sh",
        &format!("#!/bin/sh\nprintf '%s' '{answer}' > \"$EPHOR_ANSWER\"\nexit {exit}\n"),
    );
}

const DOWN: &str = r#"{"v":1,"monitors":[{"id":"coverage","value":79.2}]}"#;

fn refresh_now(world: &World) {
    world
        .ephor()
        .args(["refresh", PROJECT, "--monitors-now"])
        .assert()
        .success();
}

fn new_rows(world: &World) -> Vec<Value> {
    let output = world.ephor().args(["waiting", "--json"]).output().unwrap();
    json_of(&output)["new"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn an_action_no_entry_hands_over_is_passed_over_and_the_detail_says_why() {
    let world = world(json!({
        "id": "coverage", "title": "Line coverage", "healthy": { "min": 80 },
        "action": "nope", "from": { "command": "scripts/kpis.sh" }
    }));
    kpis(&world, DOWN, 0);
    refresh_now(&world);

    let monitor = format!("{PROJECT}/coverage");
    let output = world
        .ephor()
        .args(["monitors", "show", &monitor, "--json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let matter = shaped("monitor", &output)["matter"].clone();
    let why = matter["passed_over"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        why.contains("`nope`") && why.contains("no entry by that name hands work over"),
        "{matter}"
    );
    assert_eq!(matter["proposal"]["entry"], json!("triage"), "{matter}");
    assert_eq!(matter["proposal"]["fallback"], json!(true), "{matter}");

    let output = world
        .ephor()
        .args(["monitors", "show", &monitor])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("matter: "))
        .unwrap_or_else(|| panic!("{text}"));
    assert!(line.ends_with(&format!(" · {why}")), "{line}");
}

#[test]
fn an_unknown_matter_is_proposed_what_any_matter_is_not_the_monitors_workflow() {
    let world = world(json!({
        "id": "coverage", "title": "Line coverage", "healthy": { "min": 80 },
        "workflow": "fix-coverage", "from": { "command": "scripts/kpis.sh" }
    }));
    kpis(&world, DOWN, 0);
    refresh_now(&world);

    // Every time the store keeps put months back, then a read that fails.
    let path = world
        .path()
        .join("state/ephor/monitors")
        .join(PROJECT)
        .join("monitors.json");
    let mut kept = read_json(&path);
    aged(&mut kept["monitors"]);
    write_json(&path, &kept);
    kpis(&world, "", 3);
    refresh_now(&world);

    let rows = new_rows(&world);
    let row = |infix: &str| {
        rows.iter()
            .find(|row| {
                row["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with(&format!("monitor:{PROJECT}/coverage{infix}")))
            })
            .unwrap_or_else(|| panic!("{rows:?}"))
            .clone()
    };
    let drop = row("@");
    assert_eq!(drop["proposal"]["entry"], json!("fix-coverage"), "{drop}");
    let unknown = row(".unknown@");
    assert_eq!(unknown["proposal"]["entry"], json!("triage"), "{unknown}");
    assert_eq!(unknown["proposal"]["fallback"], json!(true), "{unknown}");
}

fn aged(value: &mut Value) {
    match value {
        Value::String(text) if text.len() > 10 && text.as_bytes()[10] == b'T' => {
            *text = "2026-01-01T00:00:00Z".to_string();
        }
        Value::Array(values) => values.iter_mut().for_each(aged),
        Value::Object(fields) => fields.values_mut().for_each(aged),
        _ => {}
    }
}
