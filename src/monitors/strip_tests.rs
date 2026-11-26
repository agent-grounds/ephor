use chrono::{TimeDelta, Utc};
use serde_json::{json, Value as Json};

use super::*;
use crate::monitors::declared;
use crate::monitors::test_zone::utc;

fn now() -> DateTime<Utc> {
    utc("2026-11-25T12:00:00Z")
}

fn monitor(declaration: Json) -> Monitor {
    let id = declaration["id"].as_str().unwrap().to_string();
    declared::read(None, &[declaration]).resolve(&id, None)
}

fn latency() -> Monitor {
    monitor(json!({
        "id": "latency", "unit": "ms", "better": "lower", "healthy": {"max": 50}
    }))
}

fn reading(hours_ago: i64, v: Value, health: Option<Health>) -> Line {
    Line::Reading {
        t: now() - TimeDelta::hours(hours_ago),
        v,
        health,
        out_of_cadence: false,
    }
}

fn number(hours_ago: i64, v: f64) -> Line {
    reading(hours_ago, Value::Number(v), Some(Health::Ok))
}

/// §FS-021-monitors.18.5: the strip covers the window, or the last 24
/// readings where those reach further back.
#[test]
fn the_span_is_the_window_or_the_last_24_readings() {
    let hourly: Vec<Line> = (0..48).rev().map(|ago| number(ago, 10.0)).collect();
    let span = span(&hourly, Window::Week, now());
    assert_eq!(span.from, now() - TimeDelta::days(7));
    assert!(!span.by_readings);

    let weekly: Vec<Line> = (0..30)
        .rev()
        .map(|weeks| number(weeks * 24 * 7, 10.0))
        .collect();
    let span = super::span(&weekly, Window::Week, now());
    assert!(span.by_readings);
    assert_eq!(span.from, now() - TimeDelta::weeks(23));

    let few = vec![number(24 * 20, 1.0), number(1, 2.0)];
    assert_eq!(
        super::span(&few, Window::Week, now()).from,
        now() - TimeDelta::days(20),
        "fewer than 24 readings reach as far as the first"
    );
}

/// §FS-021-monitors.18.5, §FS-021-monitors.18.9: 24 cells over the span,
/// nothing before the history began, a gap where nothing was read, and
/// every cell judged under today's condition rather than the one it was
/// recorded under.
#[test]
fn a_strip_has_24_cells_judged_under_todays_condition() {
    let monitor = latency();
    let judge = monitor.judge(None);
    let lines = vec![
        reading(3, Value::Number(40.0), Some(Health::Down)),
        Line::marker(now() - TimeDelta::hours(2)),
        number(0, 60.0),
    ];
    let span = span(&lines, Window::SixHours, now());
    let cells = strip(&lines, &span, &monitor, &judge);
    assert_eq!(cells.len(), CELLS);
    assert_eq!(cells[0].mark, Mark::Before);
    let read: Vec<&Cell> = cells
        .iter()
        .filter(|cell| cell.mark == Mark::Read)
        .collect();
    assert_eq!(read.len(), 2);
    assert_eq!(read[0].health, Some(Health::Ok), "40 ms is healthy today");
    assert_eq!(read[0].height, Some(0));
    assert_eq!(read[1].health, Some(Health::Down));
    assert_eq!(read[1].height, Some(7));
    let gap = cells
        .iter()
        .find(|cell| cell.mark == Mark::Gap && cell.health.is_some())
        .unwrap();
    assert_eq!(gap.health, Some(Health::Unknown));
    assert_eq!(gap.glyph(), '·');
    let drawn: String = cells.iter().map(Cell::glyph).collect();
    assert_eq!(drawn.chars().count(), CELLS);
    assert!(drawn.ends_with('█'), "{drawn}");
}

/// §FS-021-monitors.18.9: a thinned line takes the worse of its least and
/// greatest value.
#[test]
fn a_thinned_line_is_judged_by_the_worse_of_its_two() {
    let monitor = latency();
    let judge = monitor.judge(None);
    let lines = vec![Line::Thinned {
        t: now() - TimeDelta::hours(1),
        min: Value::Number(10.0),
        mean: Some(30.0),
        max: Value::Number(70.0),
        n: 12,
        health: Some(Health::Ok),
    }];
    let span = span(&lines, Window::SixHours, now());
    let cell = strip(&lines, &span, &monitor, &judge)
        .into_iter()
        .find(|cell| cell.mark == Mark::Read)
        .unwrap();
    assert_eq!(cell.value, Some(30.0));
    assert_eq!(cell.health, Some(Health::Down));
    assert_eq!(cell.height, Some(3), "one value alone sits in the middle");
}

/// §FS-021-monitors.18.6: a number's difference with its unit, coloured by
/// `better`; a percentage's in points; `±0` and `—`.
#[test]
fn a_numbers_change_is_its_difference_coloured_by_better() {
    let monitor = latency();
    let judge = monitor.judge(None);
    let lines = vec![number(5, 42.0), number(1, 44.5), number(0, 40.0)];
    let span = span(&lines, Window::SixHours, now());
    let change = change(&lines, &span, &monitor, &judge, 1);
    assert_eq!(change.says, "▾ 2 ms");
    assert_eq!(change.way, Way::Better);

    let rising = vec![number(5, 40.0), number(0, 42.0)];
    assert_eq!(
        super::change(&rising, &span, &monitor, &judge, 1),
        Change {
            says: "▴ 2 ms".into(),
            way: Way::Worse
        }
    );
    let flat = vec![number(5, 40.0), number(0, 40.0)];
    assert_eq!(super::change(&flat, &span, &monitor, &judge, 1).says, "±0");
    assert_eq!(super::change(&[], &span, &monitor, &judge, 1).says, "—");

    let percent =
        super::tests::monitor(json!({"id": "coverage", "unit": "%", "healthy": {"min": 80}}));
    let lines = vec![number(5, 81.0), number(0, 82.2)];
    let change = super::change(&lines, &span, &percent, &percent.judge(None), 1);
    assert_eq!(change.says, "▴ 1.2 pt");
    assert_eq!(change.way, Way::Plain, "nothing says which way is better");

    let size = super::tests::monitor(json!({"id": "size", "unit": "bytes", "healthy": null}));
    let lines = vec![number(5, 20_000_000.0), number(0, 18_213_798.0)];
    let change = super::change(&lines, &span, &size, &size.judge(None), 1);
    assert_eq!(change.says, "▾ 1.7 MiB", "a size moves in binary units");
    let cost = super::tests::monitor(json!({"id": "cost", "unit": "$", "healthy": null}));
    let lines = vec![number(5, 36.0), number(0, 42.5)];
    let change = super::change(&lines, &span, &cost, &cost.judge(None), 1);
    assert_eq!(change.says, "▴ $6.50", "money moves to the cent");
}

/// §FS-021-monitors.18.6, §FS-021-monitors.10.1, §FS-021-monitors.10.2: a
/// drop is counted as the hold counts one, so a flap shorter than
/// `drop_after` is none and one passing reading short of `recover_after`
/// ends nothing; an unreadable read neither adds nor breaks a run; and a
/// span that opens already down counts no drop for it.
#[test]
fn drops_are_counted_by_the_hold() {
    let yes = |ago| reading(ago, Value::Boolean(true), None);
    let no = |ago| reading(ago, Value::Boolean(false), None);
    let held = monitor(json!({"id": "green", "kind": "boolean", "drop_after": 2}));
    let judge = held.judge(None);
    let lines = vec![
        yes(9),
        no(8),
        yes(7),
        no(6),
        Line::marker(now() - TimeDelta::minutes(330)),
        no(5),
        yes(4),
        no(3),
        no(2),
    ];
    let span = span(&lines, Window::Day, now());
    assert_eq!(change(&lines, &span, &held, &judge, 1).says, "2 drops");

    let plain = monitor(json!({"id": "green", "kind": "boolean"}));
    let judge = plain.judge(None);
    let lines = vec![no(5), yes(4), no(3), yes(2), yes(1)];
    assert_eq!(change(&lines, &span, &plain, &judge, 1).says, "2 drops");
    assert_eq!(change(&lines, &span, &plain, &judge, 2).says, "1 drop");

    let lines = vec![no(30), no(5), no(4), yes(3), no(2)];
    let six = Span {
        from: now() - TimeDelta::hours(6),
        to: now(),
        by_readings: false,
    };
    assert_eq!(
        change(&lines, &six, &plain, &judge, 1).says,
        "1 drop",
        "down before the span opened, recovered inside it, then down again"
    );
    let lines = vec![no(30), no(5), no(4)];
    assert_eq!(change(&lines, &six, &plain, &judge, 1).says, "no drops");
}

/// §FS-021-monitors.18.6: a boolean says how often it dropped, judged under
/// today's condition.
#[test]
fn a_booleans_change_counts_its_drops() {
    let monitor = monitor(json!({"id": "green", "kind": "boolean"}));
    let judge = monitor.judge(None);
    let yes = |ago| reading(ago, Value::Boolean(true), None);
    let no = |ago| reading(ago, Value::Boolean(false), None);
    let lines = vec![yes(5), no(4), no(3), yes(2), no(1), yes(0)];
    let span = span(&lines, Window::SixHours, now());
    assert_eq!(change(&lines, &span, &monitor, &judge, 1).says, "2 drops");
    let lines = vec![yes(1), yes(0)];
    assert_eq!(change(&lines, &span, &monitor, &judge, 1).says, "no drops");
    let cells = strip(&lines, &span, &monitor, &judge);
    let read = cells.iter().find(|cell| cell.mark == Mark::Read).unwrap();
    assert_eq!(read.height, Some(7));
}

/// §FS-021-monitors.18.5: a boolean draws a strip of health, so it reads
/// without colour: tall while ok, low while down; a KPI's by its value.
#[test]
fn a_booleans_strip_is_drawn_by_its_health() {
    let yes = |ago| reading(ago, Value::Boolean(true), None);
    let no = |ago| reading(ago, Value::Boolean(false), None);
    let lines = vec![yes(3), no(2), no(1), yes(0)];
    let span = span(&lines, Window::SixHours, now());
    let drawn = |monitor: &Monitor| -> String {
        strip(&lines, &span, monitor, &monitor.judge(None))
            .iter()
            .filter(|cell| cell.mark == Mark::Read)
            .map(Cell::glyph)
            .collect()
    };
    let green = monitor(json!({"id": "green", "kind": "boolean"}));
    assert_eq!(drawn(&green), "█▁▁█");
    let kpi = monitor(json!({"id": "green", "kind": "boolean", "healthy": null}));
    assert_eq!(drawn(&kpi), "█▁▁█");
    let inverted = monitor(json!({"id": "red", "kind": "boolean", "healthy": {"is": false}}));
    assert_eq!(drawn(&inverted), "▁██▁");
}

/// §FS-021-monitors.18.5, §FS-021-monitors.18.6: a series draws its last 24
/// points and changes between the first and the last of them.
#[test]
fn a_series_draws_and_changes_over_its_last_24_points() {
    let monitor = monitor(json!({
        "id": "bench", "kind": "series", "unit": "ms", "healthy": {"max": 30}
    }));
    let points: Vec<Point> = (0..30)
        .map(|at| Point {
            t: None,
            x: Some(format!("v{at}")),
            v: f64::from(at),
        })
        .collect();
    let cells = series(&points, &monitor, &monitor.judge(None));
    assert_eq!(cells.len(), CELLS);
    assert_eq!(cells[0].value, Some(6.0));
    assert_eq!(series_change(&points, &monitor).says, "▴ 23 ms");
    let short = &points[..3];
    let cells = series(short, &monitor, &monitor.judge(None));
    assert_eq!(cells[20].mark, Mark::Before);
    assert_eq!(cells[23].value, Some(2.0));
}
