use serde_json::json;

use super::*;
use crate::monitors::model::{Proposal, Source, Stated};

fn refused(entry: serde_json::Value) -> Vec<String> {
    read_declaration(&entry).expect_err("refused").faults
}

#[test]
fn an_id_is_lowercase_letters_digits_and_dashes() {
    assert!(is_id("coverage"));
    assert!(is_id("p95-latency"));
    assert!(is_id("3rd-party"));
    assert!(!is_id(""));
    assert!(!is_id("-lead"));
    assert!(!is_id("Coverage"));
    assert!(!is_id("a_b"));
    assert!(!is_id("a.b"));
}

#[test]
fn a_full_declaration_reads_every_field() {
    let declaration = read_declaration(&json!({
        "id": "coverage",
        "title": "Line coverage",
        "description": "Lines the tests run.",
        "kind": "number",
        "unit": "%",
        "better": "higher",
        "healthy": {"min": 80},
        "warn": {"min": 85},
        "from": {"command": "scripts/coverage", "cwd": "repo:core"},
        "every": "1d",
        "at": "06:00",
        "timeout": 120,
        "drop_after": 2,
        "recover_after": 3,
        "workflow": "raise-coverage"
    }))
    .unwrap();
    assert_eq!(declaration.id, "coverage");
    assert_eq!(declaration.title.as_deref(), Some("Line coverage"));
    assert_eq!(declaration.kind, Some(Kind::Number));
    assert_eq!(declaration.better, Some(Better::Higher));
    assert_eq!(declaration.healthy.said().unwrap().min, Some(80.0));
    assert_eq!(declaration.warn.said().unwrap().min, Some(85.0));
    assert_eq!(
        declaration.from,
        Stated::Said(Source::Command {
            command: "scripts/coverage".into(),
            cwd: Some("repo:core".into())
        })
    );
    assert_eq!(declaration.every.as_deref(), Some("1d"));
    assert_eq!(declaration.at, Stated::Said("06:00".into()));
    assert_eq!(declaration.timeout, Some(120));
    assert_eq!(declaration.drop_after, Some(2));
    assert_eq!(declaration.recover_after, Some(3));
    assert_eq!(declaration.workflow, Stated::Said("raise-coverage".into()));
    assert!(declaration.action.is_unsaid());
    assert!(declaration.notes.is_empty());
}

#[test]
fn an_unknown_key_is_noted_and_read_without() {
    let declaration = read_declaration(&json!({
        "id": "coverage",
        "heathy": {"min": 80},
        "from": {"file": "out/coverage.json", "shell": "bash"}
    }))
    .unwrap();
    assert_eq!(
        declaration.notes,
        vec!["unknown field heathy", "unknown field from.shell"]
    );
    assert!(declaration.healthy.is_unsaid());
}

#[test]
fn null_sets_a_field_aside() {
    let declaration = read_declaration(&json!({
        "id": "coverage", "healthy": null, "warn": null, "from": null, "at": null,
        "workflow": null
    }))
    .unwrap();
    assert_eq!(declaration.healthy, Stated::Null);
    assert_eq!(declaration.warn, Stated::Null);
    assert_eq!(declaration.from, Stated::Null);
    assert_eq!(declaration.at, Stated::Null);
    assert_eq!(declaration.workflow, Stated::Null);
}

#[test]
fn a_declaration_without_an_id_or_with_a_bad_one_is_refused() {
    assert_eq!(
        refused(json!({"kind": "number"})),
        vec!["a monitor's declaration needs an id"]
    );
    let faults = refused(json!({"id": "Coverage"}));
    assert!(faults[0].starts_with("id \"Coverage\" is not a monitor id"));
    assert_eq!(
        refused(json!(["coverage"])),
        vec!["a monitor is declared as an object, not a list"]
    );
}

#[test]
fn each_field_out_of_shape_says_so() {
    let cases = [
        (json!({"kind": "gauge"}), "kind \"gauge\" is not boolean, number, level or series"),
        (json!({"better": "up"}), "better \"up\" is not higher or lower"),
        (json!({"title": ""}), "title is empty"),
        (json!({"unit": 3}), "unit is a number, not a string"),
        (json!({"of": "four"}), "of is a string, not a number"),
        (json!({"timeout": 0}), "timeout is 0, not a whole number of seconds of at least 1"),
        (json!({"drop_after": 1.5}), "drop_after is 1.5, not a whole number of at least 1"),
        (json!({"healthy": {}}), "healthy is empty: a condition holds is, min, max, in or at_most"),
        (json!({"healthy": 80}), "healthy is a number, not a condition such as {\"min\": 80}"),
        (json!({"healthy": {"min": "80"}}), "healthy's min must be a number"),
        (json!({"warn": {"is": "yes"}}), "warn's is must be true or false"),
        (json!({"healthy": {"in": []}}), "healthy's in must list level names"),
        (json!({"levels": []}), "levels must be a list of distinct level names, best first, such as [\"clean\", \"degraded\", \"failed\"]"),
        (json!({"levels": ["a", "a"]}), "levels must be a list of distinct level names, best first, such as [\"clean\", \"degraded\", \"failed\"]"),
        (json!({"every": "0m"}), "every \"0m\" is not a duration: a whole number and m, h, d or w, such as 10m, 4h, 1d or 1w"),
        (json!({"every": "90s"}), "every \"90s\" is not a duration: a whole number and m, h, d or w, such as 10m, 4h, 1d or 1w"),
        (json!({"at": "6:00"}), "at \"6:00\" is not a time of day: HH:MM, or a day and a time such as Mon 06:00"),
        (json!({"at": "Monday 06:00"}), "at \"Monday 06:00\" is not a time of day: HH:MM, or a day and a time such as Mon 06:00"),
        (json!({"at": "24:00"}), "at \"24:00\" is not a time of day: HH:MM, or a day and a time such as Mon 06:00"),
        (json!({"from": {"command": "a", "file": "b"}}), "from names a command or a file, not both"),
        (json!({"from": {}}), "from must name a command or a file"),
        (json!({"from": {"command": ""}}), "from's command must be a non-empty string"),
        (json!({"from": {"file": "x", "cwd": "/abs"}}), "from's cwd \"/abs\" is not 'root' or 'repo:<name>'"),
        (json!({"workflow": ""}), "workflow \"\" is not a name, nor null"),
        (json!({"workflow": "a", "action": "b"}), "workflow and action are one choice; name one of them"),
        (json!({"workflow": "a", "action": null}), "workflow and action are one choice; name one of them"),
    ];
    for (mut entry, sentence) in cases {
        entry["id"] = json!("m");
        let faults = refused(entry.clone());
        assert_eq!(faults, vec![sentence.to_string()], "for {entry}");
        assert_eq!(
            read_declaration(&entry).unwrap_err().id.as_deref(),
            Some("m")
        );
    }
}

#[test]
fn an_unknown_condition_key_is_kept_for_the_monitor_to_refuse() {
    let declaration = read_declaration(&json!({"id": "m", "healthy": {"minimum": 80}})).unwrap();
    assert_eq!(declaration.healthy.said().unwrap().unknown, vec!["minimum"]);
}

#[test]
fn an_id_declared_twice_is_refused_wherever_it_stands() {
    let declared = read_declarations(
        &[
            json!({"id": "a"}),
            json!({"id": "b"}),
            json!({"id": "a", "kind": "number"}),
            json!({"id": "c", "kind": "gauge"}),
        ],
        Home::Manifest,
    );
    assert_eq!(
        declared
            .declarations
            .iter()
            .map(|d| d.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b"]
    );
    assert_eq!(
        declared.faults("a"),
        vec![
            "a is declared more than once in the manifest",
            "a is declared more than once in the manifest"
        ]
    );
    assert_eq!(declared.refused[2].index, 3);
    assert_eq!(declared.refused[2].id.as_deref(), Some("c"));
    assert!(declared.get("b").is_some());
    assert!(declared.get("a").is_none());
}

#[test]
fn a_refused_and_a_read_declaration_of_one_id_are_both_refused() {
    let declared = read_declarations(
        &[json!({"id": "a", "kind": "gauge"}), json!({"id": "a"})],
        Home::Site,
    );
    assert!(declared.declarations.is_empty());
    assert!(declared
        .faults("a")
        .contains(&"a is declared more than once in the site configuration".to_string()));
}

#[test]
fn the_proposal_reads_as_one_choice() {
    let declaration = read_declaration(&json!({"id": "m", "action": "notify"})).unwrap();
    assert_eq!(declaration.action, Stated::Said("notify".into()));
    let _ = Proposal::Action("notify".into());
}

#[test]
fn a_sample_reads_its_measure_and_its_description() {
    let sample = read_sample(&json!({
        "id": "coverage",
        "value": 82.4,
        "of": 100,
        "at": "2026-11-25T05:40:00+01:00",
        "ok": true,
        "detail": "two crates under",
        "url": "https://example.invalid/report",
        "title": "Coverage",
        "kind": "number",
        "unit": "%",
        "healthy": {"min": 80},
        "warn": {"min": 85}
    }))
    .unwrap();
    assert_eq!(sample.value, Some(Value::Number(82.4)));
    assert_eq!(sample.of, Some(100.0));
    assert_eq!(sample.at.unwrap().to_rfc3339(), "2026-11-25T05:40:00+01:00");
    assert_eq!(sample.ok, Some(true));
    assert_eq!(sample.description.unit.as_deref(), Some("%"));
    assert_eq!(sample.description.warn.as_ref().unwrap().min, Some(85.0));
}

#[test]
fn a_samples_points_are_all_timed_or_all_labelled() {
    let sample = read_sample(&json!({
        "id": "latency",
        "points": [{"x": "p50", "v": 12}, {"x": "p95", "v": 40}]
    }))
    .unwrap();
    assert_eq!(sample.points.unwrap().len(), 2);
    let mixed = read_sample(&json!({
        "id": "latency",
        "points": [{"x": "p50", "v": 12}, {"t": "2026-11-25T05:40:00Z", "v": 40}]
    }))
    .unwrap_err();
    assert_eq!(
        mixed.sentence,
        "the sample's points mix t and x; one sample uses one"
    );
    assert_eq!(mixed.id.as_deref(), Some("latency"));
    let empty = read_sample(&json!({"id": "latency", "points": []})).unwrap_err();
    assert_eq!(
        empty.sentence,
        "the sample's points must list at least one point"
    );
    let both = read_sample(
        &json!({"id": "l", "points": [{"x": "a", "t": "2026-11-25T05:40:00Z", "v": 1}]}),
    )
    .unwrap_err();
    assert!(both.sentence.starts_with("the sample's point {"));
}

#[test]
fn a_sample_with_no_id_is_read_for_no_monitor() {
    let fault = read_sample(&json!({"value": 1})).unwrap_err();
    assert_eq!(fault.id, None);
    assert_eq!(fault.sentence, "a sample with no id is read for no monitor");
    let fault = read_sample(&json!({"id": "Bad", "value": 1})).unwrap_err();
    assert_eq!(fault.id, None);
    let fault = read_sample(&json!(3)).unwrap_err();
    assert_eq!(
        fault.sentence,
        "a sample is an object, not a number, so it is read for no monitor"
    );
}

#[test]
fn a_sample_out_of_shape_names_its_monitor() {
    let cases = [
        (json!({"value": [1]}), "the sample's value is a list, not true, false, a number or a level"),
        (json!({"value": 1, "at": "yesterday"}), "the sample's at \"yesterday\" is not a time with its offset, such as 2026-11-25T05:40:00Z"),
        (json!({"value": 1, "at": "2026-11-25T05:40:00"}), "the sample's at \"2026-11-25T05:40:00\" is not a time with its offset, such as 2026-11-25T05:40:00Z"),
        (json!({"value": 1, "ok": "yes"}), "ok is a string, not true or false"),
    ];
    for (mut entry, sentence) in cases {
        entry["id"] = json!("m");
        let fault = read_sample(&entry).unwrap_err();
        assert_eq!(fault.sentence, sentence);
        assert_eq!(fault.id.as_deref(), Some("m"));
    }
}

#[test]
fn a_sample_deserializes_through_the_same_reader() {
    let sample: Sample = serde_json::from_value(json!({"id": "m", "value": "clean"})).unwrap();
    assert_eq!(sample.value, Some(Value::Level("clean".into())));
    let refused: Result<Sample, _> = serde_json::from_value(json!({"id": "m", "value": {}}));
    assert!(refused.is_err());
}

#[test]
fn a_refused_declaration_keeps_its_unknown_field_notes() {
    let refused =
        read_declaration(&json!({"id": "m", "kind": "gauge", "heathy": {"min": 1}})).unwrap_err();
    assert_eq!(refused.notes, vec!["unknown field heathy"]);
    assert_eq!(refused.id.as_deref(), Some("m"));
}

#[test]
fn a_refusal_says_whether_the_shape_or_a_second_declaration_refused_it() {
    let declared = read_declarations(
        &[
            json!({"id": "a", "rate": 1}),
            json!({"id": "a"}),
            json!({"id": "b", "kind": "numbr"}),
        ],
        Home::Site,
    );
    let whys: Vec<_> = declared
        .refused
        .iter()
        .map(|refused| (refused.id.clone().unwrap(), refused.why))
        .collect();
    assert_eq!(
        whys,
        vec![
            ("a".to_string(), Refusal::DeclaredTwice),
            ("a".to_string(), Refusal::DeclaredTwice),
            ("b".to_string(), Refusal::Shape),
        ]
    );
    assert_eq!(declared.refused[0].notes, vec!["unknown field rate"]);
}

/// §FS-021-monitors.21.2: a refused declaration keeps whether it says
/// something judges its monitor, so a status in error stays a status.
#[test]
fn a_refused_declaration_says_whether_something_judges_it() {
    let judged = |entry: serde_json::Value| read_declaration(&entry).expect_err("refused").judged;
    assert_eq!(
        judged(json!({"id": "a", "healthy": {"min": 80}, "every": "90s"})),
        Some(true)
    );
    assert_eq!(judged(json!({"id": "a", "healthy": "high"})), Some(true));
    assert_eq!(
        judged(json!({"id": "a", "kind": "level", "every": "90s"})),
        Some(true)
    );
    assert_eq!(
        judged(json!({"id": "a", "levels": ["ok", "bad"], "every": "9"})),
        Some(true)
    );
    assert_eq!(
        judged(json!({"id": "a", "healthy": null, "every": "90s"})),
        Some(false)
    );
    assert_eq!(
        judged(json!({"id": "a", "kind": "number", "every": "90s"})),
        None
    );
}
