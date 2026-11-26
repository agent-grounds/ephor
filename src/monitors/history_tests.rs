use serde_json::json;

use super::*;
use crate::monitors::test_zone::utc;

fn line(json: serde_json::Value) -> Line {
    serde_json::from_value(json).unwrap()
}

#[test]
fn each_line_reads_and_writes_its_shape() {
    let reading = line(json!({"t": "2026-11-25T06:00:00Z", "v": 81.0, "health": "ok"}));
    assert_eq!(
        reading,
        Line::Reading {
            t: utc("2026-11-25T06:00:00Z"),
            v: Value::Number(81.0),
            health: Some(Health::Ok),
            out_of_cadence: false,
        }
    );
    let kpi = line(json!({"t": "2026-11-25T06:00:00Z", "v": "fast"}));
    assert_eq!(kpi.health(), None);
    let marker = line(json!({"t": "2026-11-25T06:00:00Z", "health": "unknown"}));
    assert_eq!(marker, Line::marker(utc("2026-11-25T06:00:00Z")));
    let thinned = line(json!({
        "t": "2026-11-25T06:00:00Z", "min": 70.0, "mean": 75.0, "max": 80.0, "n": 6, "health": "down"
    }));
    assert!(matches!(
        thinned,
        Line::Thinned {
            n: 6,
            mean: Some(_),
            ..
        }
    ));
    for line in [reading, kpi, marker, thinned] {
        let back: Line = serde_json::from_value(serde_json::to_value(&line).unwrap()).unwrap();
        assert_eq!(back, line);
    }
    let boolean = line(
        json!({"t": "2026-11-25T06:00:00Z", "min": false, "max": true, "n": 2, "health": "down"}),
    );
    assert_eq!(serde_json::to_value(&boolean).unwrap().get("mean"), None);
    assert!(!boolean.is_reading());
}

#[test]
fn history_is_drawn_under_todays_condition() {
    let judge = Judge::Condition(serde_json::from_value(json!({"min": 85})).unwrap());
    let recorded_ok = line(json!({"t": "2026-11-25T06:00:00Z", "v": 81.0, "health": "ok"}));
    assert_eq!(
        rejudged(&recorded_ok, &judge, None, None),
        Some(Health::Down)
    );
    assert_eq!(recorded_ok.health(), Some(Health::Ok));
    let thinned = line(
        json!({"t": "2026-11-25T06:00:00Z", "min": 80.0, "mean": 88.0, "max": 95.0, "n": 4, "health": "ok"}),
    );
    assert_eq!(rejudged(&thinned, &judge, None, None), Some(Health::Down));
    let warn: Condition = serde_json::from_value(json!({"min": 90})).unwrap();
    let high =
        line(json!({"t": "2026-11-25T06:00:00Z", "min": 86.0, "mean": 88.0, "max": 95.0, "n": 4}));
    assert_eq!(
        rejudged(&high, &judge, Some(&warn), None),
        Some(Health::Warn)
    );
    let marker = Line::marker(utc("2026-11-25T06:00:00Z"));
    assert_eq!(rejudged(&marker, &judge, None, None), Some(Health::Unknown));
    assert_eq!(rejudged(&recorded_ok, &Judge::Kpi, None, None), None);
    assert_eq!(
        rejudged(&recorded_ok, &Judge::SampleOk, None, None),
        Some(Health::Ok)
    );
}

#[test]
fn the_worst_health_is_down_then_unknown_then_warn_then_ok() {
    assert_eq!(
        worst([Some(Health::Ok), Some(Health::Warn)]),
        Some(Health::Warn)
    );
    assert_eq!(
        worst([Some(Health::Warn), Some(Health::Unknown), None]),
        Some(Health::Unknown)
    );
    assert_eq!(
        worst([Some(Health::Unknown), Some(Health::Down)]),
        Some(Health::Down)
    );
    assert_eq!(worst([None, None]), None);
}

/// §FS-021-monitors.2.2: a number's points stand before ephor's own first
/// line, in time order and judged under today's condition; a point at or
/// after that line, or labelled by `x`, is not drawn.
#[test]
fn a_numbers_points_are_drawn_before_its_own_first_line() {
    let at = |text: &str| chrono::DateTime::parse_from_rfc3339(text).unwrap();
    let point = |t: &str, v: f64| Point {
        t: Some(at(t)),
        x: None,
        v,
    };
    let points = [
        point("2026-11-03T00:00:00Z", 85.0),
        point("2026-11-01T00:00:00Z", 70.0),
        point("2026-11-05T00:00:00Z", 99.0),
        Point {
            t: None,
            x: Some("a4f5f87".into()),
            v: 1.0,
        },
    ];
    let own = [line(
        serde_json::json!({"t": "2026-11-05T00:00:00Z", "v": 90, "health": "ok"}),
    )];
    let judge = Judge::Condition(Condition {
        min: Some(80.0),
        ..Condition::default()
    });
    let drawn = handed_over(&points, &own, &judge, None);
    let seen: Vec<(String, Option<Health>)> = drawn
        .iter()
        .map(|line| (line.t().format("%d").to_string(), line.health()))
        .collect();
    assert_eq!(
        seen,
        [
            ("01".to_string(), Some(Health::Down)),
            ("03".to_string(), Some(Health::Ok)),
            ("05".to_string(), Some(Health::Ok)),
        ]
    );
    assert_eq!(drawn[2], own[0]);
}
