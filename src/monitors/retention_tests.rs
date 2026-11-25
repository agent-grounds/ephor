use super::*;
use crate::monitors::test_zone::utc;

fn reading(t: DateTime<Utc>, v: f64, health: Health) -> Line {
    Line::Reading {
        t,
        v: Value::Number(v),
        health: Some(health),
    }
}

fn ten() -> TimeDelta {
    TimeDelta::minutes(10)
}

/// Enough recent readings that the last 104 protect nothing older.
fn recent(now: DateTime<Utc>) -> Vec<Line> {
    (0..KEPT_READINGS as i64)
        .map(|k| reading(now - TimeDelta::minutes(10 * (k + 1)), 1.0, Health::Ok))
        .rev()
        .collect()
}

fn with_recent(old: Vec<Line>, now: DateTime<Utc>) -> Vec<Line> {
    old.into_iter().chain(recent(now)).collect()
}

fn older(lines: &[Line], now: DateTime<Utc>) -> Vec<Line> {
    lines
        .iter()
        .filter(|line| line.t() < now - TimeDelta::days(1))
        .cloned()
        .collect()
}

#[test]
fn every_reading_is_kept_for_30_days() {
    let now = utc("2026-11-25T00:00:00Z");
    let lines = with_recent(
        vec![reading(now - TimeDelta::days(29), 5.0, Health::Ok)],
        now,
    );
    assert_eq!(thin(&lines, ten(), Kind::Number, None, now), lines);
}

#[test]
fn after_30_days_an_hour_is_one_line() {
    let now = utc("2026-11-25T00:00:00Z");
    let hour = utc("2026-10-15T06:00:00Z");
    let old = vec![
        reading(hour + TimeDelta::minutes(5), 70.0, Health::Down),
        Line::marker(hour + TimeDelta::minutes(15)),
        reading(hour + TimeDelta::minutes(25), 80.0, Health::Ok),
        reading(hour + TimeDelta::minutes(35), 90.0, Health::Warn),
    ];
    let kept = thin(&with_recent(old, now), ten(), Kind::Number, None, now);
    assert_eq!(
        older(&kept, now),
        vec![Line::Thinned {
            t: hour,
            min: Value::Number(70.0),
            mean: Some(80.0),
            max: Value::Number(90.0),
            n: 3,
            health: Some(Health::Down),
        }]
    );
}

#[test]
fn after_90_days_a_day_is_one_line_and_after_two_years_nothing() {
    let now = utc("2026-11-25T00:00:00Z");
    let day = utc("2026-07-01T00:00:00Z");
    let old = vec![
        reading(utc("2024-11-01T00:00:00Z"), 1.0, Health::Ok),
        reading(day + TimeDelta::hours(1), 10.0, Health::Ok),
        reading(day + TimeDelta::hours(13), 20.0, Health::Ok),
        Line::marker(day + TimeDelta::hours(14)),
    ];
    let kept = thin(&with_recent(old, now), ten(), Kind::Number, None, now);
    assert_eq!(
        older(&kept, now),
        vec![Line::Thinned {
            t: day,
            min: Value::Number(10.0),
            mean: Some(15.0),
            max: Value::Number(20.0),
            n: 2,
            health: Some(Health::Unknown),
        }]
    );
}

#[test]
fn an_hour_folds_only_once_all_of_it_is_past_30_days() {
    let now = utc("2026-11-25T00:30:00Z");
    let straddling = reading(utc("2026-10-26T00:10:00Z"), 3.0, Health::Ok);
    let past = reading(utc("2026-10-25T23:50:00Z"), 4.0, Health::Ok);
    let kept = thin(
        &with_recent(vec![past, straddling.clone()], now),
        ten(),
        Kind::Number,
        None,
        now,
    );
    let older = older(&kept, now);
    assert_eq!(older.len(), 2);
    assert!(matches!(older[0], Line::Thinned { n: 1, .. }));
    assert_eq!(older[1], straddling);
}

#[test]
fn a_bucket_of_markers_only_stays_a_marker() {
    let now = utc("2026-11-25T00:00:00Z");
    let hour = utc("2026-10-15T06:00:00Z");
    let old = vec![
        Line::marker(hour + TimeDelta::minutes(5)),
        Line::marker(hour + TimeDelta::minutes(15)),
    ];
    let kept = thin(&with_recent(old, now), ten(), Kind::Number, None, now);
    assert_eq!(older(&kept, now), vec![Line::marker(hour)]);
}

#[test]
fn a_boolean_or_a_level_has_no_mean_and_keeps_its_kinds_order() {
    let now = utc("2026-11-25T00:00:00Z");
    let hour = utc("2026-10-15T06:00:00Z");
    let boolean = |t, v| Line::Reading {
        t,
        v: Value::Boolean(v),
        health: None,
    };
    let old = vec![
        boolean(hour, true),
        boolean(hour + TimeDelta::minutes(10), false),
    ];
    let kept = thin(&with_recent(old, now), ten(), Kind::Boolean, None, now);
    assert_eq!(
        older(&kept, now),
        vec![Line::Thinned {
            t: hour,
            min: Value::Boolean(false),
            mean: None,
            max: Value::Boolean(true),
            n: 2,
            health: None,
        }]
    );
    let levels: Vec<String> = ["clean", "degraded", "failed"].map(String::from).to_vec();
    let level = |t, v: &str| Line::Reading {
        t,
        v: Value::Level(v.into()),
        health: Some(Health::Ok),
    };
    let old = vec![
        level(hour, "degraded"),
        level(hour + TimeDelta::minutes(10), "failed"),
        level(hour + TimeDelta::minutes(20), "clean"),
    ];
    let kept = thin(
        &with_recent(old, now),
        ten(),
        Kind::Level,
        Some(&levels),
        now,
    );
    assert!(matches!(
        &older(&kept, now)[0],
        Line::Thinned { min: Value::Level(min), max: Value::Level(max), mean: None, n: 3, .. }
            if min == "clean" && max == "failed"
    ));
}

#[test]
fn a_daily_or_slower_monitor_is_never_thinned_only_cut() {
    let now = utc("2026-11-25T00:00:00Z");
    let lines: Vec<Line> = (0..200)
        .map(|k| reading(now - TimeDelta::days(5 * (199 - k)), k as f64, Health::Ok))
        .collect();
    let kept = thin(&lines, TimeDelta::days(1), Kind::Number, None, now);
    // 104 readings reach back 515 days; older than two years are cut.
    assert!(kept.iter().all(|line| line.is_reading()));
    assert_eq!(
        kept.len(),
        lines
            .iter()
            .filter(|line| line.t() >= now - kept_for())
            .count()
    );
    assert_eq!(kept.len(), 147);
}

#[test]
fn the_last_104_readings_stay_whatever_their_age() {
    let now = utc("2026-11-25T00:00:00Z");
    let weekly: Vec<Line> = (0..110)
        .map(|k| reading(now - TimeDelta::weeks(109 - k), k as f64, Health::Ok))
        .collect();
    let mut with_marker = vec![Line::marker(now - TimeDelta::days(800))];
    with_marker.extend(weekly.clone());
    let kept = thin(&with_marker, TimeDelta::weeks(1), Kind::Number, None, now);
    // Weeks 0 to 104 are within two years or among the last 104; 105 to 109
    // are neither, and a marker is never kept for its count.
    assert_eq!(kept, weekly[5..].to_vec());

    let fast_and_old: Vec<Line> = (0..3)
        .map(|k| {
            reading(
                now - TimeDelta::days(800) + TimeDelta::minutes(10 * k),
                1.0,
                Health::Ok,
            )
        })
        .collect();
    assert_eq!(
        thin(&fast_and_old, ten(), Kind::Number, None, now),
        fast_and_old
    );
}

#[test]
fn thinning_again_changes_nothing_and_hours_fold_into_days() {
    let start = utc("2026-08-01T00:00:00Z");
    let mut lines: Vec<Line> = (0..6 * 24 * 3)
        .map(|k| {
            reading(
                start + TimeDelta::minutes(10 * k),
                (k % 7) as f64,
                Health::Ok,
            )
        })
        .collect();
    let now = utc("2026-10-25T12:00:00Z");
    lines.extend(recent(now));
    let once = thin(&lines, ten(), Kind::Number, None, now);
    assert_eq!(thin(&once, ten(), Kind::Number, None, now), once);
    assert_eq!(older(&once, now).len(), 72);

    let later = now + TimeDelta::days(20);
    let mut moved = once.clone();
    moved.extend(recent(later));
    let daily = thin(&moved, ten(), Kind::Number, None, later);
    let old = older(&daily, later);
    let days: Vec<_> = old
        .iter()
        .filter(|line| line.t() < utc("2026-08-04T00:00:00Z"))
        .collect();
    assert_eq!(days.len(), 3);
    for day in days {
        let Line::Thinned {
            n, mean, min, max, ..
        } = day
        else {
            panic!("{day:?}")
        };
        assert_eq!(*n, 144);
        assert_eq!((min, max), (&Value::Number(0.0), &Value::Number(6.0)));
        assert!((mean.unwrap() - 3.0).abs() < 0.1);
    }
}
