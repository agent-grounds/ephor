//! E2E-026-custom-status-activity: refresh is not source activity.
//!
//! A custom-status answer states when its matter moved. Dispatch snapshots
//! that activity, so an unchanged second refresh leaves the work current;
//! only a later source time or another fingerprint change reopens it
//! (§FS-006-project-interface.4, §FS-005-dispatch.5).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};
use std::fs;
use std::thread;
use std::time::Duration;

use support::*;

const ITEM: &str = "poll:fixed";
const FIRST_TIME: &str = "2026-09-01T00:00:00Z";
const LATER_TIME: &str = "2026-09-02T00:00:00Z";

const STATUS_REPORTER: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cp "$HOME/status-answer.json" "$EPHOR_ANSWER"
"#;

// Keep the dispatched ticket open without requiring the real runtime. This is
// the same polling shape as the issue reproducer: autorun starts, but does not
// finish, the one ticket sync must leave alone.
const POLLING_RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
case "$*" in
  *--help*) printf '%s\n' 'Usage: rhei run [--headless]' ;;
  *--headless*) printf '%s\n' '{"id":"polling-run","pid":1,"status":"running","exit_code":null}' ;;
  *) printf 'unexpected polling runtime arguments: %s\n' "$*" >&2; exit 2 ;;
esac
"#;

fn envelope(time: &str, state: &str) -> Value {
    json!({
        "v": 1,
        "matters": [{
            "key": ITEM,
            "kind": "status",
            "title": "fixed polling matter",
            "state": state,
            "terminal": false,
            "time": time,
            "data": { "episode": "fixed" }
        }]
    })
}

fn write_answer(world: &World, answer: Value) {
    fs::write(
        world.path().join("status-answer.json"),
        serde_json::to_string_pretty(&answer).expect("the status answer serializes"),
    )
    .expect("write the status answer");
}

fn polling_world() -> World {
    let world = World::new();
    world.stub("status-reporter", STATUS_REPORTER);
    world.stub("rhei", POLLING_RUNTIME);
    world.configure(json!({
        "projects": { PROJECT: { "providers": [{
            "provider": "custom-status",
            "command": "status-reporter",
            "format": "answer"
        }] } },
        "work": {
            "runner": "rhei",
            "recipes": [{
                "id": "poll-status",
                "icon": "poll",
                "description": "poll the status",
                "state": "fix",
                "needs_checkout": false,
                "autorun": true,
                "when": {
                    "kinds": ["status"],
                    "sources": ["custom-status"]
                },
                "brief": "Poll {title}."
            }]
        }
    }));
    write_answer(&world, envelope(FIRST_TIME, "waiting"));
    world
}

fn refresh(world: &World) {
    world.ephor().args(["refresh", PROJECT]).assert().success();
}

fn dispatch(world: &World) {
    world
        .ephor()
        .args(["work", "dispatch", "--project", PROJECT])
        .assert()
        .success();
}

fn sync(world: &World, dry_run: bool) -> Value {
    let mut args = vec!["work", "sync", "--project", PROJECT];
    if dry_run {
        args.push("--dry-run");
    }
    args.push("--json");
    let output = world.ephor().args(args).output().expect("work sync runs");
    assert!(
        output.status.success(),
        "work sync failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    json_of(&output)
}

/// The issue-93 sequence in repository form: supplied activity survives both
/// refreshes, and work sync proposes and appends no duplicate ticket
/// (§FS-006-project-interface.4, §FS-005-dispatch.5).
#[test]
fn custom_status_answer_unchanged_activity_keeps_dispatched_work_current() {
    let world = polling_world();
    refresh(&world);
    let first = world.matter(ITEM);
    dispatch(&world);
    let plan = world.forest().join("panta/poll-fixed.rhei.md");
    let before = fs::read_to_string(&plan).expect("the dispatched plan");

    thread::sleep(Duration::from_millis(20));
    refresh(&world);
    let second = world.matter(ITEM);
    let preview = sync(&world, true);
    let synced = sync(&world, false);
    let after = fs::read_to_string(&plan).expect("the plan after sync");

    let mut failures = Vec::new();
    if first["updated_at"] != FIRST_TIME {
        failures.push(format!(
            "first refresh used {:?}, expected source activity {FIRST_TIME}",
            first["updated_at"]
        ));
    }
    if second["updated_at"] != FIRST_TIME {
        failures.push(format!(
            "unchanged refresh used {:?}, expected retained activity {FIRST_TIME}",
            second["updated_at"]
        ));
    }
    if preview["reopened"] != 0 {
        failures.push(format!(
            "unchanged work proposed {} reopen(s): {}",
            preview["reopened"], preview["items"]
        ));
    }
    if synced["reopened"] != 0 {
        failures.push(format!(
            "unchanged work appended {} reopen(s): {}",
            synced["reopened"], synced["items"]
        ));
    }
    if before != after || after.matches("### Task poll-status-").count() != 1 {
        failures.push("sync changed or duplicated the dispatched ticket".to_string());
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A genuinely later source activity remains enough to reopen eligible work
/// (§FS-005-dispatch.5); the source's value, not refresh time, is the new
/// snapshot (§FS-006-project-interface.4).
#[test]
fn custom_status_answer_later_activity_reopens_dispatched_work() {
    let world = polling_world();
    refresh(&world);
    dispatch(&world);
    write_answer(&world, envelope(LATER_TIME, "waiting"));
    refresh(&world);

    let matter = world.matter(ITEM);
    let synced = sync(&world, true);
    assert_eq!(matter["updated_at"], LATER_TIME);
    assert_eq!(synced["reopened"], 1);
    assert!(
        synced["items"][0]["says"]
            .as_str()
            .is_some_and(|says| says.contains("new activity")),
        "{}",
        synced["items"]
    );
}

/// State remains an independent work fingerprint: even with a fixed activity
/// timestamp, a changed state reopens eligible work (§FS-005-dispatch.5).
#[test]
fn custom_status_answer_changed_state_reopens_dispatched_work() {
    let world = polling_world();
    refresh(&world);
    dispatch(&world);
    write_answer(&world, envelope(FIRST_TIME, "ready"));
    refresh(&world);

    let synced = sync(&world, true);
    assert_eq!(synced["reopened"], 1);
    assert!(
        synced["items"][0]["says"]
            .as_str()
            .is_some_and(|says| says.contains("the state is now ready (was waiting)")),
        "{}",
        synced["items"]
    );
}
