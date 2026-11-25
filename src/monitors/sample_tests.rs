use serde_json::json;

use super::*;
use crate::monitors::model::{Declaration, Kind};
use crate::monitors::shape::{read_declaration, read_sample};

fn manifest(entry: serde_json::Value) -> Declaration {
    read_declaration(&entry).unwrap()
}

fn checked(
    declaration: Option<serde_json::Value>,
    sample: serde_json::Value,
) -> Result<Checked, String> {
    let declaration = declaration.map(manifest);
    let sample = read_sample(&sample).unwrap();
    check(
        &sample,
        &Homes {
            manifest: declaration.as_ref(),
            ..Homes::default()
        },
    )
}

#[test]
fn a_sample_that_fits_its_monitor_is_read() {
    let read = checked(
        Some(json!({"id": "coverage", "kind": "number", "unit": "%", "healthy": {"min": 80}})),
        json!({"id": "coverage", "value": 82.4, "at": "2026-11-25T05:40:00Z", "ok": true}),
    )
    .unwrap();
    assert_eq!(read.kind, Kind::Number);
    assert_eq!(read.value, Value::Number(82.4));
    assert_eq!(read.ok, Some(true));
    assert!(read.at.is_some());
    assert_eq!(read.monitor.words(None, None), "healthy ≥ 80%");
}

#[test]
fn the_value_states_the_kind_where_nothing_else_does() {
    let boolean = checked(None, json!({"id": "m", "value": true})).unwrap();
    assert_eq!(boolean.kind, Kind::Boolean);
    assert_eq!(
        boolean.monitor.judge(None).condition().unwrap().is,
        Some(true)
    );
    let number = checked(None, json!({"id": "m", "value": 3})).unwrap();
    assert_eq!(number.kind, Kind::Number);
    let series = checked(
        None,
        json!({"id": "m", "points": [{"x": "a", "v": 1}, {"x": "b", "v": 2}]}),
    )
    .unwrap();
    assert_eq!(series.kind, Kind::Series);
    assert_eq!(series.value, Value::Number(2.0));
    assert_eq!(series.last_point.unwrap().x.as_deref(), Some("b"));
    assert_eq!(
        checked(None, json!({"id": "m", "value": "clean"})).unwrap_err(),
        "the value \"clean\" is a level's name, and no levels say which levels there are"
    );
    let level = checked(
        None,
        json!({"id": "m", "value": "clean", "levels": ["clean", "dirty"]}),
    )
    .unwrap();
    assert_eq!(level.kind, Kind::Level);
    assert_eq!(
        checked(None, json!({"id": "m"})).unwrap_err(),
        "the sample has no value"
    );
}

#[test]
fn a_value_that_does_not_fit_its_kind_is_unreadable() {
    let number = Some(json!({"id": "m", "kind": "number"}));
    assert_eq!(
        checked(number.clone(), json!({"id": "m", "value": "x"})).unwrap_err(),
        "the value \"x\" does not fit a number"
    );
    assert_eq!(
        checked(number.clone(), json!({"id": "m", "value": true})).unwrap_err(),
        "the value true does not fit a number"
    );
    assert_eq!(
        checked(
            number,
            json!({"id": "m", "value": 2, "points": [{"x": "a", "v": 1}]})
        )
        .unwrap_err(),
        "the sample's points are labelled by x, and a number's are drawn against time"
    );
    let level = Some(json!({"id": "m", "levels": ["clean", "dirty"]}));
    assert_eq!(
        checked(level, json!({"id": "m", "value": "spotless"})).unwrap_err(),
        "the value \"spotless\" is not one of its levels: clean, dirty"
    );
    let series = Some(json!({"id": "m", "kind": "series"}));
    assert_eq!(
        checked(series, json!({"id": "m", "value": 3})).unwrap_err(),
        "a series' sample carries points, and this one has none"
    );
}

#[test]
fn a_description_that_breaks_a_rule_is_an_unreadable_read() {
    assert_eq!(
        checked(
            Some(json!({"id": "m", "kind": "number"})),
            json!({"id": "m", "value": 3, "warn": {"max": 2}})
        )
        .unwrap_err(),
        "warn has no healthy to narrow: declare a healthy, or drop the warn"
    );
    assert_eq!(
        checked(
            None,
            json!({"id": "m", "value": 3, "healthy": {"is": true}})
        )
        .unwrap_err(),
        "the value 3 does not fit a boolean"
    );
}

#[test]
fn the_samples_whole_is_this_readings() {
    let declared = Some(json!({"id": "m", "kind": "number", "of": 15}));
    assert_eq!(
        checked(declared.clone(), json!({"id": "m", "value": 12}))
            .unwrap()
            .of,
        Some(15.0)
    );
    assert_eq!(
        checked(declared, json!({"id": "m", "value": 12, "of": 16}))
            .unwrap()
            .of,
        Some(16.0)
    );
}

#[test]
fn the_declaration_outranks_what_the_sample_says_of_itself() {
    let read = checked(
        Some(json!({"id": "m", "kind": "number", "healthy": {"max": 5}})),
        json!({"id": "m", "value": 3, "healthy": {"max": 1}, "kind": "number"}),
    )
    .unwrap();
    assert_eq!(read.monitor.judge(None).condition().unwrap().max, Some(5.0));
}
