//! What one read found for one monitor (§FS-021-monitors.9.2): the delivery
//! routing gave it, checked against the monitor and judged for freshness,
//! and the monitor as the machine judges it.

use chrono::{DateTime, Utc};

use crate::monitors::cadence::Cadence;
use crate::monitors::condition;
use crate::monitors::freshness::{self, Measured};
use crate::monitors::machine::{Finding, Reading, Watch};
use crate::monitors::model::Kind;
use crate::monitors::overlay::{Homes, Monitor};
use crate::monitors::route::Delivery;
use crate::monitors::sample;
use crate::monitors::state::{FileMark, Record};
use crate::monitors::words;

/// One monitor's finding, with the monitor as it was judged: the sample's
/// description laid under its declarations where a sample was read
/// (§FS-021-monitors.4.4).
#[derive(Debug, Clone)]
pub struct Found {
    pub finding: Finding,
    pub monitor: Monitor,
    pub ok: Option<bool>,
    pub of: Option<f64>,
}

/// Judges one delivery. A sample that breaks a rule is unreadable with the
/// sentence that says why (§FS-021-monitors.8.2); a file the last read found
/// as it is now, or a measurement no newer than the one held, is stale
/// (§FS-021-monitors.7.1, §FS-021-monitors.9.2).
pub fn found(
    delivery: &Delivery,
    monitor: Monitor,
    homes: &Homes,
    record: &Record,
    mark: Option<&FileMark>,
    cadence: &Cadence,
    now: DateTime<Utc>,
) -> Found {
    let kept = record.last_sample.as_ref();
    let mut found = Found {
        finding: Finding::Parked,
        ok: kept.and_then(|sample| sample.ok),
        of: kept.and_then(|sample| sample.of).or(monitor.of),
        monitor,
    };
    found.finding = match delivery {
        Delivery::Parked => Finding::Parked,
        Delivery::Unreadable(why) => Finding::Unreadable { why: why.clone() },
        Delivery::Sample(_) if mark.is_some_and(|mark| record.file_unchanged(mark)) => {
            Finding::Stale { measured: None }
        }
        Delivery::Sample(read) => match sample::check(read, homes) {
            Err(why) => Finding::Unreadable { why },
            Ok(checked) => {
                let judge = checked.monitor.judge(checked.ok);
                let held = record.last_reading.as_ref().map(|kept| kept.t);
                let series_held = checked
                    .last_point
                    .as_ref()
                    .zip(record.points().and_then(<[_]>::last))
                    .is_some_and(|(new, old)| new.same_measurement(old));
                let finding = match freshness::measured(
                    checked.at,
                    now,
                    held,
                    cadence.interval(),
                    series_held,
                ) {
                    Measured::Stale { at } => Finding::Stale { measured: Some(at) },
                    Measured::Fresh { at, skew } => Finding::Reading(Reading {
                        said: words::value(
                            &checked.value,
                            checked.monitor.unit.as_deref(),
                            checked.of,
                        ),
                        band: condition::band(
                            &judge,
                            checked.monitor.warn.as_ref(),
                            &checked.value,
                            checked.ok,
                            checked.monitor.levels.as_deref(),
                        ),
                        value: checked.value.clone(),
                        of: checked.of,
                        measured: at,
                        skew,
                        sample: Some((**read).clone()),
                        mark: mark.cloned(),
                    }),
                };
                let mut monitor = checked.monitor;
                monitor.kind = Some(checked.kind);
                found.monitor = monitor;
                found.ok = checked.ok;
                found.of = checked.of;
                finding
            }
        },
    };
    found
}

/// The monitor as the machine judges it (§FS-021-monitors.3,
/// §FS-021-monitors.11): what judges its readings, in words, and the cadence
/// its freshness is judged at.
pub fn watch<'a>(
    monitor: &'a Monitor,
    ok: Option<bool>,
    of: Option<f64>,
    kind: Option<Kind>,
    cadence: Cadence,
) -> Watch<'a> {
    let judge = monitor.judge(ok);
    Watch {
        title: &monitor.title,
        unit: monitor.unit.as_deref(),
        healthy_words: words::healthy(&judge, &monitor.terms(of)),
        judge,
        kind: monitor.kind.or(kind),
        levels: monitor.levels.as_deref(),
        drop_after: monitor.drop_after,
        recover_after: monitor.recover_after(&cadence),
        cadence,
    }
}
