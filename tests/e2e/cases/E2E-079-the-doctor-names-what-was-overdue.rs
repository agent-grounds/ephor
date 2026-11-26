//! E2E-079-the-doctor-names-what-was-overdue: `ephor doctor` notes the
//! monitor store's size, its files and where it is, and the monitors that
//! were overdue when it began, before its own refresh reads them, never one
//! whose value still holds; neither note moves the exit code
//! (§FS-021-monitors.20.2, §FS-021-monitors.7.5, §FS-010-doctor.5). The
//! monitor slot counts among the sources configured, asked and answering,
//! and a project whose only source it is is not watched rather than
//! unreachable (§FS-021-monitors.17.5).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

/// Moves every time the record keeps to `when`.
fn age(value: &mut Value, when: &str) {
    match value {
        Value::String(text) if text.len() > 10 && text.as_bytes()[10] == b'T' => {
            *text = when.to_string();
        }
        Value::Array(values) => values.iter_mut().for_each(|value| age(value, when)),
        Value::Object(fields) => fields.values_mut().for_each(|value| age(value, when)),
        _ => {}
    }
}

#[test]
fn the_doctor_notes_the_store_and_what_was_overdue() {
    let world = World::new();
    world.script(
        "scripts/kpis.sh",
        "#!/bin/sh\nprintf '{\"v\":1,\"monitors\":[{\"id\":\"coverage\",\"value\":81}]}' > \"$EPHOR_ANSWER\"\n",
    );
    world.configure(json!({ "projects": { PROJECT: { "monitors": [
        { "id": "coverage", "healthy": { "min": 80 }, "from": { "command": "scripts/kpis.sh" } }
    ] } } }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    let doctor = || {
        let out = world
            .ephor()
            .args(["doctor", "--skip-self", "--json"])
            .output()
            .unwrap();
        (out.status.code(), json_of(&out))
    };
    let (first, report) = doctor();
    assert!(!report["notes"].to_string().contains("overdue"), "{report}");

    // The monitor slot is one source configured, asked and answering; with
    // no other source the project is not watched, which is degraded rather
    // than unreachable.
    let project = &report["projects"][0];
    assert_eq!(
        project["sources"],
        json!({"configured": 1, "asked": 1, "answering": 1}),
        "{report}"
    );
    assert_eq!(project["health"], "degraded", "{report}");
    assert_eq!(first, Some(4), "{report}");
    // The line that announces the project counts the slot among its sources too.
    let shown = world
        .ephor()
        .args(["doctor", "--skip-self"])
        .output()
        .unwrap();
    let progress = String::from_utf8_lossy(&shown.stderr);
    assert!(
        progress.contains(&format!("{PROJECT} (1 source(s))")),
        "{progress}"
    );

    let path = world
        .path()
        .join("state/ephor/monitors")
        .join(PROJECT)
        .join("monitors.json");
    let aged = |when: &str| {
        let mut kept = read_json(&path);
        age(&mut kept["monitors"]["coverage"], when);
        write_json(&path, &kept);
    };

    // Half an hour old, a `10m` monitor's value still holds: it is late,
    // not overdue.
    let late = (chrono::Utc::now() - chrono::Duration::minutes(30))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    aged(&late);
    let (_, report) = doctor();
    assert!(!report["notes"].to_string().contains("overdue"), "{report}");

    // Months old: the monitor was overdue before the doctor.
    aged("2026-01-01T00:00:00Z");

    let (code, report) = doctor();
    let notes: Vec<String> = report["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|note| note.as_str().unwrap().to_string())
        .collect();
    assert!(
        notes.contains(&format!(
            "monitor {PROJECT}/coverage was overdue when the doctor began"
        )),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|note| note.starts_with("the monitor store holds ")
                && note.contains(" file(s) at ")),
        "{notes:?}"
    );
    // The notes move its exit code no further.
    assert_eq!(code, first, "{report}");
}
