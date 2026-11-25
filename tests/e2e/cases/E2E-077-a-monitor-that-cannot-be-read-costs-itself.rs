//! E2E-077-a-monitor-that-cannot-be-read-costs-itself: a command that fails,
//! times out or answers what does not read, a file that is missing, and an
//! answer with no sample for a monitor each leave only the monitors they read
//! unreadable, with why; a command that exits `75` parks them; a file unchanged
//! since its last read is stale. None of them fails the refresh or the
//! monitor slot (§FS-021-monitors.5.2, §FS-021-monitors.5.5,
//! §FS-021-monitors.5.6, §FS-021-monitors.9.2, §FS-021-monitors.17.2).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

fn from_command(id: &str, command: &str) -> Value {
    json!({ "id": id, "healthy": { "min": 1 }, "timeout": 1, "from": { "command": command } })
}

fn from_file(id: &str, file: &str) -> Value {
    json!({ "id": id, "healthy": { "min": 1 }, "from": { "file": file } })
}

fn world() -> World {
    let world = World::new();
    world.script("scripts/fail.sh", "#!/bin/sh\nexit 1\n");
    world.script("scripts/slow.sh", "#!/bin/sh\nsleep 5\n");
    world.script(
        "scripts/garbled.sh",
        "#!/bin/sh\nprintf 'not json' > \"$EPHOR_ANSWER\"\n",
    );
    world.script("scripts/park.sh", "#!/bin/sh\nexit 75\n");
    world.script(
        "scripts/kpis.sh",
        "#!/bin/sh\nprintf '{\"v\":1,\"monitors\":[{\"id\":\"coverage\",\"value\":81}]}' > \"$EPHOR_ANSWER\"\n",
    );
    world.file(
        "out/filed.json",
        r#"{"v":1,"monitors":[{"id":"filed","value":3}]}"#,
    );
    world.configure(json!({ "projects": { PROJECT: { "monitors": [
        from_command("broken", "scripts/fail.sh"),
        from_command("slow", "scripts/slow.sh"),
        from_command("garbled", "scripts/garbled.sh"),
        from_command("parked", "scripts/park.sh"),
        from_command("coverage", "scripts/kpis.sh"),
        from_command("absent", "scripts/kpis.sh"),
        from_file("missing", "out/missing.json"),
        from_file("filed", "out/filed.json")
    ] } } }));
    world
}

fn attempts(world: &World) -> Value {
    let store = world.path().join("state/ephor/monitors").join(PROJECT);
    let kept = read_json(&store.join("monitors.json"));
    let mut found = json!({});
    for (id, record) in kept["monitors"].as_object().unwrap() {
        found[id] = record["last_attempt"].clone();
    }
    found
}

#[test]
fn each_unreadable_monitor_says_why_and_the_slot_answers() {
    let world = world();
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert_eq!(world.feed()["providers"]["monitor"]["ok"], json!(true));

    let found = attempts(&world);
    let why = |id: &str| {
        assert_eq!(
            found[id]["found"],
            json!("unreadable"),
            "{id}: {}",
            found[id]
        );
        found[id]["why"].as_str().unwrap().to_string()
    };
    assert!(why("broken").contains("exited 1"), "{}", why("broken"));
    assert!(why("slow").contains("timed out"), "{}", why("slow"));
    assert!(!why("garbled").is_empty());
    assert_eq!(why("missing"), "out/missing.json is missing");
    assert_eq!(why("absent"), "no sample for absent");
    assert_eq!(found["parked"]["found"], json!("parked"));
    assert_eq!(found["coverage"]["found"], json!("reading"));
    assert_eq!(found["filed"]["found"], json!("reading"));
}

#[test]
fn a_file_unchanged_since_its_last_read_is_stale() {
    let world = world();
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
        .ephor()
        .args(["refresh", PROJECT, "--monitors-now"])
        .assert()
        .success();
    assert_eq!(attempts(&world)["filed"]["found"], json!("stale"));
}
