//! E2E-076-a-refresh-reads-the-due-monitors: a plain refresh reads a
//! project's due monitors, runs a `from` they share once, keeps what it read
//! in the published store shapes, and answers the monitor slot even when
//! nothing was due (§FS-021-monitors.5.1, §FS-021-monitors.6.4,
//! §FS-021-monitors.9.1, §FS-021-monitors.17.1). `--monitors-now` reads every
//! one out of cadence, a narrowed refresh reads none, and neither changes what
//! `refresh --json` prints (§FS-021-monitors.21.4, §FS-021-monitors.17.4,
//! §FS-011-command-line.9.2).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

const KPIS: &str = r#"#!/bin/sh
echo "$EPHOR_MONITORS" >> "$HOME/runs.log"
printf '%s' '{"v":1,"monitors":[{"id":"coverage","value":81},{"id":"flaky","value":2}]}' > "$EPHOR_ANSWER"
"#;

fn world() -> World {
    let world = World::new();
    world.script("scripts/kpis.sh", KPIS);
    world.stub(
        "status-reporter",
        "#!/bin/sh\nprintf '{\"v\":1}' > \"$EPHOR_ANSWER\"\n",
    );
    world.configure(json!({ "projects": { PROJECT: {
        "providers": [{ "provider": "custom-status", "command": "status-reporter", "format": "answer" }],
        "monitors": [
            { "id": "coverage", "healthy": { "min": 80 }, "from": { "command": "scripts/kpis.sh" } },
            { "id": "flaky", "healthy": { "max": 5 }, "from": { "command": "scripts/kpis.sh" } }
        ]
    } } }));
    world
}

fn runs(world: &World) -> Vec<String> {
    std::fs::read_to_string(world.path().join("runs.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn store(world: &World) -> std::path::PathBuf {
    world.path().join("state/ephor/monitors").join(PROJECT)
}

#[test]
fn a_plain_refresh_reads_the_due_monitors_once_and_keeps_them() {
    let world = world();
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert_eq!(runs(&world), vec!["coverage,flaky"]);

    let kept = read_json(&store(&world).join("monitors.json"));
    assert_eq!(kept["version"], json!(1));
    for id in ["coverage", "flaky"] {
        let record = &kept["monitors"][id];
        assert!(record["schedule"]["first_seen"].is_string(), "{record}");
        assert!(record["last_reading"]["t"].is_string(), "{record}");
        let history = std::fs::read_to_string(store(&world).join(format!("{id}.jsonl"))).unwrap();
        let line: Value = serde_json::from_str(history.lines().next().unwrap()).unwrap();
        assert!(line["t"].is_string() && line["v"].is_number(), "{line}");
    }
    assert_eq!(world.feed()["providers"]["monitor"]["ok"], json!(true));

    // Nothing is due ten minutes from now; the slot answers all the same.
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert_eq!(runs(&world).len(), 1);
    assert_eq!(world.feed()["providers"]["monitor"]["ok"], json!(true));
}

#[test]
fn monitors_now_reads_every_monitor_out_of_cadence() {
    let world = world();
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
        .ephor()
        .args(["refresh", PROJECT, "--monitors-now"])
        .assert()
        .success();
    assert_eq!(runs(&world).len(), 2);
    let history = std::fs::read_to_string(store(&world).join("coverage.jsonl")).unwrap();
    let last: Value = serde_json::from_str(history.lines().last().unwrap()).unwrap();
    assert_eq!(last["out_of_cadence"], json!(true));
}

#[test]
fn a_narrowed_refresh_reads_no_monitor_and_refuses_to_read_now() {
    let world = world();
    world
        .ephor()
        .args(["refresh", PROJECT, "--source", "custom-status"])
        .assert()
        .success();
    assert!(runs(&world).is_empty());

    let refused = world
        .ephor()
        .args([
            "refresh",
            PROJECT,
            "--source",
            "custom-status",
            "--monitors-now",
        ])
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("--monitors-now"));

    let refused = world
        .ephor()
        .args(["refresh", PROJECT, "--source", "monitor"])
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("ephor's own reading"));
    assert!(runs(&world).is_empty());
}

#[test]
fn refresh_json_prints_the_fields_it_printed_before() {
    let world = world();
    let out = world
        .ephor()
        .args(["refresh", PROJECT, "--json", "--monitors-now"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let reading = json_of(&out);
    let mut keys: Vec<&str> = reading
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(keys, ["degraded", "lost", "notes", "projects", "refreshed"]);
    let mut keys: Vec<&str> = reading["projects"][0]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(keys, ["failures", "items", "lost", "notes", "project"]);
}
