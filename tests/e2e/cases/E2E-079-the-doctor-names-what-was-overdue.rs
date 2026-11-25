//! E2E-079-the-doctor-names-what-was-overdue: `ephor doctor` notes the
//! monitor store's size, its files and where it is, and the monitors that
//! were overdue when it began, before its own refresh reads them; neither
//! note moves the exit code (§FS-021-monitors.20.2, §FS-010-doctor.5).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

/// Moves every time the record keeps back to the start of the year.
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
    let (well, report) = doctor();
    assert!(!report["notes"].to_string().contains("overdue"), "{report}");

    // The reading is two days old: the monitor was overdue before the doctor.
    let path = world
        .path()
        .join("state/ephor/monitors")
        .join(PROJECT)
        .join("monitors.json");
    let mut kept = read_json(&path);
    age(&mut kept["monitors"]["coverage"]);
    write_json(&path, &kept);

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
    // A project with monitors alone is unreachable to the doctor
    // (§FS-021-monitors.17.5); the notes move its exit code no further.
    assert_eq!(code, well, "{report}");
}
