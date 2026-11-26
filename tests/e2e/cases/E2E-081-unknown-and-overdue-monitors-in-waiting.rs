//! E2E-081-unknown-and-overdue-monitors-in-waiting: a monitor whose value no
//! longer holds because it cannot be read opens an unknown matter of its own,
//! naming the cause, and the next reading ends it
//! (§FS-021-monitors.11.2, §FS-021-monitors.12.1, §FS-021-monitors.14.2). A
//! monitor overdue opens nothing, and Waiting's overview line still counts it
//! (§FS-021-monitors.7.5, §FS-021-monitors.20.1, §FS-019-waiting.3). A level
//! reaching one worse than its drop had reached moves the matter
//! (§FS-021-monitors.13.1).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

fn world(answer: &str, exit: i32) -> World {
    let world = World::new();
    kpis(&world, answer, exit);
    world.configure(json!({ "projects": { PROJECT: {
        "providers": [],
        "monitors": [
            { "id": "coverage", "title": "Line coverage", "healthy": { "min": 80 },
              "from": { "command": "scripts/kpis.sh" } },
            { "id": "release", "levels": ["ok", "degraded", "failed"],
              "healthy": { "at_most": "ok" }, "from": { "command": "scripts/kpis.sh" } }
        ]
    } } }));
    world
}

fn kpis(world: &World, answer: &str, exit: i32) {
    world.script(
        "scripts/kpis.sh",
        &format!("#!/bin/sh\nprintf '%s' '{answer}' > \"$EPHOR_ANSWER\"\nexit {exit}\n"),
    );
}

const GOOD: &str =
    r#"{"v":1,"monitors":[{"id":"coverage","value":81},{"id":"release","value":"ok"}]}"#;

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

/// Every time the store keeps put months back, as if nothing had read the
/// project since.
fn age(value: &mut Value) {
    match value {
        Value::String(text) if text.len() > 10 && text.as_bytes()[10] == b'T' => {
            *text = "2026-01-01T00:00:00Z".to_string();
        }
        Value::Array(values) => values.iter_mut().for_each(age),
        Value::Object(fields) => fields.values_mut().for_each(age),
        _ => {}
    }
}

fn aged(world: &World) {
    let path = world
        .path()
        .join("state/ephor/monitors")
        .join(PROJECT)
        .join("monitors.json");
    let mut kept = read_json(&path);
    age(&mut kept["monitors"]);
    write_json(&path, &kept);
}

#[test]
fn an_unreadable_monitor_opens_its_own_matter_and_the_next_reading_ends_it() {
    let world = world(GOOD, 0);
    refresh_now(&world);
    assert!(monitor_matters(&world).is_empty());

    aged(&world);
    kpis(&world, "", 3);
    refresh_now(&world);
    let matters = monitor_matters(&world);
    let unknown = matters
        .iter()
        .find(|matter| {
            matter["key"]
                .as_str()
                .unwrap()
                .starts_with(&format!("monitor:{PROJECT}/coverage.unknown@"))
        })
        .unwrap_or_else(|| panic!("{matters:?}"))
        .clone();
    assert_eq!(unknown["state"], json!("unknown"));
    let title = unknown["title"].as_str().unwrap();
    assert!(
        title.starts_with("Line coverage cannot be read: "),
        "{title}"
    );
    assert_eq!(unknown["raw"]["meta"]["cause"], json!("unreadable"));

    kpis(&world, GOOD, 0);
    refresh_now(&world);
    let ended = monitor_matters(&world)
        .into_iter()
        .find(|matter| matter["key"] == unknown["key"])
        .unwrap();
    assert_eq!(ended["state"], json!("readable"), "{ended}");
    let title = ended["title"].as_str().unwrap();
    assert!(
        title.starts_with("Line coverage read again at ") && title.ends_with("(81)"),
        "{title}"
    );
}

/// Every time the store keeps moved `hours` back, as if they had passed.
fn shift(value: &mut Value, hours: i64) {
    match value {
        Value::String(text) => {
            if let Ok(at) = chrono::DateTime::parse_from_rfc3339(text) {
                let at = at.with_timezone(&chrono::Utc) - chrono::Duration::hours(hours);
                *text = at.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
            }
        }
        Value::Array(values) => values.iter_mut().for_each(|value| shift(value, hours)),
        Value::Object(fields) => fields.values_mut().for_each(|value| shift(value, hours)),
        _ => {}
    }
}

/// A monitor whose site says its value is fresh for ten minutes, brought by
/// a daily command, is overdue two hours on: no read was tried since its
/// slot (§FS-021-monitors.6.8, §FS-021-monitors.7.2). A plain refresh then
/// judges it, finds the daily command not due, and opens nothing for it,
/// while Waiting's overview line counts it (§FS-021-monitors.7.5,
/// §FS-021-monitors.11.2, §FS-021-monitors.20.1).
#[test]
fn an_overdue_monitor_opens_nothing_and_the_overview_line_counts_it() {
    let world = World::new();
    let runs = world.path().join("kpis.runs");
    world.script(
        "scripts/kpis.sh",
        &format!(
            "#!/bin/sh\necho run >> '{}'\nprintf '%s' '{}' > \"$EPHOR_ANSWER\"\n",
            runs.display(),
            r#"{"v":1,"monitors":[{"id":"daily","value":5},{"id":"pipeline","value":90}]}"#
        ),
    );
    world.configure(json!({ "projects": { PROJECT: {
        "providers": [],
        "monitors": [
            { "id": "daily", "title": "Daily", "healthy": { "min": 1 }, "every": "1d",
              "from": { "command": "scripts/kpis.sh" } },
            { "id": "pipeline", "title": "Pipeline", "healthy": { "min": 80 }, "every": "10m" }
        ]
    } } }));
    let ran = || {
        std::fs::read_to_string(&runs)
            .unwrap_or_default()
            .lines()
            .count()
    };
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert_eq!(ran(), 1);

    let path = world
        .path()
        .join("state/ephor/monitors")
        .join(PROJECT)
        .join("monitors.json");
    let mut kept = read_json(&path);
    assert_eq!(
        kept["monitors"]["pipeline"]["owner"],
        json!({"from": {"command": "scripts/kpis.sh"}}),
        "{kept:#}"
    );
    assert_eq!(kept["monitors"]["pipeline"].get("cause"), None);
    shift(&mut kept["monitors"], 2);
    write_json(&path, &kept);

    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert_eq!(ran(), 1, "the daily command is not due");
    let kept = read_json(&path);
    assert_eq!(
        kept["monitors"]["pipeline"]["cause"],
        json!({"cause": "overdue"}),
        "judged overdue by this refresh: {kept:#}"
    );
    assert_eq!(kept["monitors"]["pipeline"].get("unknown"), None);
    assert!(world.feed()["providers"]["monitor"].is_object());
    assert!(monitor_matters(&world).is_empty(), "{:#}", world.feed());

    let output = world.ephor().args(["waiting"]).output().unwrap();
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("   monitors: 0 down · 1 unknown · 1 overdue"),
        "{text}"
    );
    let reading = json_of(&world.ephor().args(["waiting", "--json"]).output().unwrap());
    assert_eq!(
        reading["overview"]["monitors"],
        json!({"down": 0, "unknown": 1, "overdue": 1})
    );
}

#[test]
fn a_worse_level_moves_the_matter() {
    let degraded =
        r#"{"v":1,"monitors":[{"id":"coverage","value":81},{"id":"release","value":"degraded"}]}"#;
    let world = world(degraded, 0);
    refresh_now(&world);
    let first = monitor_matters(&world)[0].clone();
    assert_eq!(first["fingerprint"]["health"]["worst"], json!("degraded"));

    std::thread::sleep(std::time::Duration::from_millis(1100));
    kpis(&world, &degraded.replace("degraded", "failed"), 0);
    refresh_now(&world);
    let moved = monitor_matters(&world)[0].clone();
    assert_eq!(moved["key"], first["key"]);
    assert_eq!(moved["state"], json!("down"));
    assert_eq!(moved["fingerprint"]["health"]["worst"], json!("failed"));
    assert_ne!(moved["updated_at"], first["updated_at"], "{moved}");
}
