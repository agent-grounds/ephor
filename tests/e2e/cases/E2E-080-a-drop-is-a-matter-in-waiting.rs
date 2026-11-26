//! E2E-080-a-drop-is-a-matter-in-waiting: a status that drops is a New
//! matter of the monitor slot in Waiting, keyed by its episode, titled with
//! its value and condition, carrying its facts as meta and proposing its
//! monitor's own workflow (§FS-021-monitors.11.3, §FS-021-monitors.12,
//! §FS-021-monitors.16.1, §FS-019-waiting.1.2). A value wobbling inside its
//! band does not move it (§FS-021-monitors.13.1), and once it recovers its
//! matter is finished and says when (§FS-021-monitors.14.1). A KPI never
//! makes a matter (§FS-021-monitors.1.2).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

fn world() -> World {
    let world = World::new();
    world.configure(json!({ "projects": { PROJECT: {
        "providers": [],
        "monitors": [
            { "id": "coverage", "title": "Line coverage", "unit": "%",
              "healthy": { "min": 80 }, "workflow": "fix-coverage", "recover_after": 2,
              "from": { "command": "scripts/kpis.sh" } },
            { "id": "lines", "from": { "command": "scripts/kpis.sh" } }
        ]
    } } }));
    world
}

fn answer(world: &World, coverage: f64) {
    world.script(
        "scripts/kpis.sh",
        &format!(
            "#!/bin/sh\nprintf '%s' '{{\"v\":1,\"monitors\":[{{\"id\":\"coverage\",\"value\":{coverage},\"url\":\"https://ci.example/7\"}},{{\"id\":\"lines\",\"value\":1200}}]}}' > \"$EPHOR_ANSWER\"\n"
        ),
    );
}

fn refresh_now(world: &World) {
    world
        .ephor()
        .args(["refresh", PROJECT, "--monitors-now"])
        .assert()
        .success();
}

fn monitor_matters(world: &World) -> Vec<Value> {
    world.feed()["providers"]["monitor"]["matters"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn waiting(world: &World) -> Value {
    let output = world.ephor().args(["waiting", "--json"]).output().unwrap();
    json_of(&output)
}

#[test]
fn a_drop_is_new_in_waiting_with_its_monitors_workflow() {
    let world = world();
    answer(&world, 79.2);
    refresh_now(&world);

    let matters = monitor_matters(&world);
    assert_eq!(matters.len(), 1, "the KPI makes none: {matters:?}");
    let matter = &matters[0];
    let key = matter["key"].as_str().unwrap();
    assert!(
        key.starts_with(&format!("monitor:{PROJECT}/coverage@")),
        "{key}"
    );
    assert_eq!(
        matter["title"],
        json!("Line coverage dropped: 79.2% (healthy ≥ 80%)")
    );
    assert_eq!(matter["state"], json!("down"));
    assert_eq!(matter["url"], json!("https://ci.example/7"));
    let meta = &matter["raw"]["meta"];
    assert_eq!(meta["monitor"], json!("coverage"));
    assert_eq!(meta["value"], json!("79.2"));
    assert_eq!(meta["condition"], json!("healthy ≥ 80%"));

    let reading = waiting(&world);
    let new: Vec<&Value> = reading["new"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["id"] == key)
        .collect();
    assert_eq!(new.len(), 1, "{reading}");
    assert_eq!(
        new[0]["proposal"]["entry"],
        json!("fix-coverage"),
        "{reading}"
    );
    assert_eq!(
        reading["overview"]["monitors"],
        json!({"down": 1, "unknown": 0, "overdue": 0})
    );
}

#[test]
fn a_wobble_does_not_move_it_and_a_recovery_finishes_it() {
    let world = world();
    answer(&world, 79.2);
    refresh_now(&world);
    let first = monitor_matters(&world)[0].clone();

    answer(&world, 79.3);
    refresh_now(&world);
    let wobbled = monitor_matters(&world)[0].clone();
    assert_eq!(wobbled["key"], first["key"]);
    assert_eq!(wobbled["updated_at"], first["updated_at"], "{wobbled}");
    assert_eq!(wobbled["state"], json!("down"));

    // One passing reading is not yet `recover_after: 2` of them in a row.
    answer(&world, 81.0);
    refresh_now(&world);
    assert_eq!(monitor_matters(&world)[0]["state"], json!("down"));
    refresh_now(&world);
    let recovered = monitor_matters(&world)
        .into_iter()
        .find(|matter| matter["key"] == first["key"])
        .unwrap();
    assert_eq!(recovered["state"], json!("recovered"), "{recovered}");
    let title = recovered["title"].as_str().unwrap();
    assert!(
        title.starts_with("Line coverage recovered at ") && title.ends_with("(81.0%)"),
        "{title}"
    );
    let reading = waiting(&world);
    assert!(
        !reading["new"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == first["key"]),
        "{reading}"
    );
}
