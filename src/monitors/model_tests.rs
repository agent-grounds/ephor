use serde_json::json;

use super::*;
use crate::monitors::shape::{read_declaration, read_sample};

const MANIFEST: &str = include_str!("../../assets/ephor-manifest.schema.json");
const ANSWER: &str = include_str!("../../assets/ephor-answer.schema.json");

/// A validator for one of a published schema's `$defs`.
fn validator(schema: &str, definition: &str) -> jsonschema::Validator {
    let schema: serde_json::Value = serde_json::from_str(schema).unwrap();
    let rooted = json!({
        "$schema": schema["$schema"],
        "$defs": schema["$defs"],
        "$ref": format!("#/$defs/{definition}"),
    });
    jsonschema::validator_for(&rooted).unwrap()
}

#[test]
fn a_declaration_writes_the_shape_the_manifest_schema_publishes() {
    let schema = validator(MANIFEST, "monitor");
    let entries = [
        json!({
            "id": "coverage", "title": "Line coverage", "description": "Lines the tests run.",
            "kind": "number", "unit": "%", "better": "higher", "of": 100,
            "healthy": {"min": 80}, "warn": {"min": 85},
            "from": {"command": "scripts/coverage", "cwd": "repo:core"},
            "every": "1d", "at": "06:00", "timeout": 120, "drop_after": 2, "recover_after": 1,
            "workflow": "raise-coverage"
        }),
        json!({"id": "releasable", "levels": ["clean", "dirty"], "healthy": {"at_most": "clean"}, "action": "notify"}),
        json!({"id": "site-only", "warn": null, "at": null, "from": null, "workflow": null, "healthy": null}),
        json!({"id": "file", "from": {"file": "out/x.json"}, "every": "2w", "at": "Mon 06:00"}),
    ];
    for entry in entries {
        assert!(schema.is_valid(&entry), "{entry}");
        let declaration = read_declaration(&entry).unwrap();
        let written = serde_json::to_value(&declaration).unwrap();
        assert!(schema.is_valid(&written), "{written}");
        assert_eq!(read_declaration(&written).unwrap(), declaration);
    }
}

#[test]
fn a_sample_writes_the_shape_the_answer_schema_publishes() {
    let schema = validator(ANSWER, "monitorSample");
    let entries = [
        json!({
            "id": "coverage", "value": 82.4, "of": 100, "at": "2026-11-25T05:40:00+01:00",
            "ok": true, "detail": "two crates under", "url": "https://example.invalid/r",
            "title": "Coverage", "kind": "number", "unit": "%", "better": "higher",
            "healthy": {"min": 80}, "warn": {"min": 85}, "description": "Lines the tests run."
        }),
        json!({"id": "latency", "points": [{"x": "p50", "v": 12}, {"x": "p95", "v": 40}]}),
        json!({"id": "trend", "value": 3, "points": [{"t": "2026-11-25T05:40:00Z", "v": 3}]}),
        json!({"id": "state", "value": "clean", "levels": ["clean", "dirty"], "healthy": null}),
    ];
    for entry in entries {
        assert!(schema.is_valid(&entry), "{entry}");
        let sample = read_sample(&entry).unwrap();
        let written = serde_json::to_value(&sample).unwrap();
        assert!(schema.is_valid(&written), "{written}");
        assert_eq!(serde_json::from_value::<Sample>(written).unwrap(), sample);
    }
}

#[test]
fn kinds_values_and_healths_have_their_words() {
    for kind in [Kind::Boolean, Kind::Number, Kind::Level, Kind::Series] {
        assert_eq!(Kind::parse(kind.name()), Some(kind));
    }
    assert_eq!(Kind::parse("gauge"), None);
    assert!(Kind::Series.is_numeric() && !Kind::Level.is_numeric());
    assert_eq!(Value::Boolean(true).kind(), Kind::Boolean);
    assert_eq!(Value::Level("a".into()).as_number(), None);
    assert_eq!(
        serde_json::to_value(Value::Level("a".into())).unwrap(),
        json!("a")
    );
    assert_eq!(Health::from(Band::Warn).name(), "warn");
    assert!(Band::Warn.passes() && !Band::Down.passes());
    assert!(Health::Down.badness() > Health::Unknown.badness());
    assert!(Health::Unknown.badness() > Health::Warn.badness());
    assert_eq!(Home::Manifest.phrase(), "the manifest");
}

#[test]
fn two_sources_are_the_same_when_they_name_the_same_thing_from_the_same_place() {
    let run = |cwd: Option<&str>| Source::Command {
        command: "x".into(),
        cwd: cwd.map(String::from),
    };
    assert_eq!(run(None), run(None));
    assert_ne!(run(None), run(Some("root")));
    assert!(run(None).is_command());
    assert!(!Source::File {
        file: "x".into(),
        cwd: None
    }
    .is_command());
}
