//! A monitor's hold, episodes and movement as one state machine
//! (§FS-021-monitors.9.2, §FS-021-monitors.10, §FS-021-monitors.11,
//! §FS-021-monitors.13.1, §FS-021-monitors.14, §FS-021-monitors.15).
//!
//! [`step`] takes what happened to a monitor and changes its record, and
//! returns what the store and the feed must do about it: a line to append,
//! an episode opened, moved or ended. It reads no clock and no file, so
//! every table of the specification runs under test.

use chrono::{DateTime, FixedOffset, TimeZone, Utc};

use crate::monitors::cadence::Cadence;
use crate::monitors::condition::{self, Judge};
use crate::monitors::episode::{self, Ending, Movement, Sort, Unwatched};
use crate::monitors::freshness::{self, Attempt, Cause, Freshness};
use crate::monitors::history::Line;
use crate::monitors::model::{Band, Health, Kind, Sample, Value};
use crate::monitors::state::{
    FileMark, Hold, Kept, Moved, OpenDrop, OpenUnknown, Past, Record, Shown,
};

/// How a monitor is watched as of this step: what its homes resolve to
/// (§FS-021-monitors.4.4).
#[derive(Debug, Clone)]
pub struct Watch<'a> {
    /// Its title and unit, which its matters say (§FS-021-monitors.12.1).
    pub title: &'a str,
    pub unit: Option<&'a str>,
    /// What judges its readings; a KPI has no hold and no episodes.
    pub judge: Judge,
    /// That judge in words, `healthy ≥ 80%`, which a changed condition names
    /// (§FS-021-monitors.13.2).
    pub healthy_words: String,
    pub kind: Option<Kind>,
    pub levels: Option<&'a [String]>,
    pub drop_after: u32,
    pub recover_after: u32,
    /// The cadence its freshness is judged at (§FS-021-monitors.6.2,
    /// §FS-021-monitors.6.8).
    pub cadence: Cadence,
}

/// A reading: a valid sample, judged (§FS-021-monitors.9.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub value: Value,
    /// The value as the page says it, `81.0%`, for the sentence that ends an
    /// episode (§FS-021-monitors.14.1).
    pub said: String,
    /// Its own band; none for a KPI.
    pub band: Option<Band>,
    /// Its whole, where it is a fraction (§FS-021-monitors.2.2).
    pub of: Option<f64>,
    /// When it was measured (§FS-021-monitors.7.1).
    pub measured: DateTime<Utc>,
    pub skew: Option<DateTime<FixedOffset>>,
    /// The accepted sample, whole, which the store keeps
    /// (§FS-021-monitors.9.1).
    pub sample: Option<Sample>,
    /// The file it was read from as this read found it, for a monitor read
    /// from a file (§FS-021-monitors.9.2).
    pub mark: Option<FileMark>,
}

/// What one read found (§FS-021-monitors.9.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    Reading(Reading),
    Unreadable { why: String },
    Parked,
    Stale { measured: Option<DateTime<Utc>> },
}

/// Work laid on one of the monitor's matters that has ended
/// (§FS-021-monitors.15).
#[derive(Debug, Clone, PartialEq)]
pub struct EndedWork {
    pub on: Sort,
    pub work: String,
}

/// One read of the monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct Read {
    pub at: DateTime<Utc>,
    /// Read at a slot, rather than out of cadence (§FS-021-monitors.6.7).
    pub in_cadence: bool,
    pub found: Finding,
    /// The work whose end this read follows (§FS-021-monitors.15).
    pub after_work: Vec<EndedWork>,
}

/// What happened to a monitor.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Read(Read),
    /// A refresh that read nothing for it, judging its freshness. A failing
    /// monitor slot is every late monitor's cause (§FS-021-monitors.7.2).
    Look {
        slot_error: Option<String>,
    },
    /// It stopped being watched as it was (§FS-021-monitors.14.4).
    Unwatched(Unwatched),
}

/// What the store and the feed must do about a step.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Append a line to its history (§FS-021-monitors.9.2).
    Record(Line),
    /// Its history starts again (§FS-021-monitors.4.4).
    RestartHistory,
    Opened {
        sort: Sort,
        named: DateTime<Utc>,
    },
    Moved {
        sort: Sort,
        named: DateTime<Utc>,
        why: Movement,
    },
    Ended {
        sort: Sort,
        named: DateTime<Utc>,
        ending: Ending,
    },
}

/// Takes one event at `now` (§FS-021-monitors.9.2, §FS-021-monitors.10,
/// §FS-021-monitors.11, §FS-021-monitors.13.1, §FS-021-monitors.14,
/// §FS-021-monitors.15), and judges the monitor's freshness after it
/// (§FS-021-monitors.7.4).
pub fn step<Tz: TimeZone>(
    record: &mut Record,
    watch: &Watch,
    event: &Event,
    now: DateTime<Utc>,
    zone: &Tz,
) -> Vec<Effect> {
    let mut effects = Vec::new();
    if let (Some(was), Some(is)) = (record.kind, watch.kind) {
        if was != is {
            end_all(record, watch, Unwatched::KindChanged, now, &mut effects);
            record.last_ok = None;
            record.last_reading = None;
            record.earlier_points = None;
            effects.push(Effect::RestartHistory);
        }
    }
    if watch.kind.is_some() {
        record.kind = watch.kind;
    }
    if let Event::Unwatched(why) = event {
        end_all(record, watch, why.clone(), now, &mut effects);
        return effects;
    }
    if watch.judge.is_kpi() {
        end_all(record, watch, Unwatched::Kpi, now, &mut effects);
    } else if let Some(drop) = &mut record.drop {
        // §FS-021-monitors.13.1: the healthy that judges it changed, from whichever home;
        // one written anew that accepts the same values, or reads the same, did not.
        let changed =
            !condition::same_judge(&drop.judged_under, &watch.judge, watch.kind, watch.levels)
                && drop.judged_words != watch.healthy_words;
        drop.judged_under = watch.judge.clone();
        // Its words stand as they were when it last changed: a fraction's `of`
        // and the unit are said in them, and neither is movement (§FS-021-monitors.12.2).
        if changed {
            let from = std::mem::replace(&mut drop.judged_words, watch.healthy_words.clone());
            let why = Movement::ConditionChanged {
                from,
                to: watch.healthy_words.clone(),
            };
            drop.moved = Some(Moved {
                at: now,
                why: why.clone(),
            });
            effects.push(Effect::Moved {
                sort: Sort::Drop,
                named: drop.named,
                why,
            });
        }
    }
    let slot_error = match event {
        Event::Read(read) => {
            take(record, watch, read, now, zone, &mut effects);
            None
        }
        Event::Look { slot_error } => slot_error.as_deref(),
        Event::Unwatched(_) => None,
    };
    judge_freshness(record, watch, now, zone, slot_error, &mut effects);
    // §FS-021-monitors.12: what its open matters say, as of this step.
    let shown = shown(record, watch);
    if let Some(drop) = &mut record.drop {
        drop.shown = shown.clone();
    }
    if let Some(unknown) = &mut record.unknown {
        unknown.shown = unknown_since(shown, unknown.named);
    }
    effects
}

/// An unknown matter's health is `unknown`, which began when its episode
/// opened, whatever health the hold kept beside it (§FS-021-monitors.12.3,
/// §FS-021-monitors.11.2).
fn unknown_since(shown: Shown, named: DateTime<Utc>) -> Shown {
    Shown {
        since: Some(named),
        ..shown
    }
}

/// What a matter of the monitor says of it now (§FS-021-monitors.12.1,
/// §FS-021-monitors.12.3).
fn shown(record: &Record, watch: &Watch) -> Shown {
    let sample = record.last_sample.as_ref();
    let last = record.last_reading.as_ref();
    Shown {
        title: watch.title.to_string(),
        value: last.map(|kept| kept.v.clone()),
        of: last.and_then(|kept| kept.of),
        unit: watch.unit.map(str::to_string),
        condition: watch.healthy_words.clone(),
        since: record.hold.since,
        last_ok: record.last_ok.clone(),
        detail: sample.and_then(|sample| sample.detail.clone()),
        url: sample.and_then(|sample| sample.url.clone()),
    }
}

/// One read, as §FS-021-monitors.9.2's table says.
fn take<Tz: TimeZone>(
    record: &mut Record,
    watch: &Watch,
    read: &Read,
    now: DateTime<Utc>,
    zone: &Tz,
    effects: &mut Vec<Effect>,
) {
    let parked = read.found == Finding::Parked;
    if read.in_cadence {
        record.schedule.read(&watch.cadence, read.at, parked, zone);
    } else if parked {
        record.schedule.parked_since.get_or_insert(read.at);
    } else {
        record.schedule.parked_since = None;
    }
    record.last_attempt = Some(Attempt {
        at: read.at,
        found: match &read.found {
            Finding::Reading(_) => freshness::Found::Reading,
            Finding::Unreadable { why } => freshness::Found::Unreadable { why: why.clone() },
            Finding::Parked => freshness::Found::Parked,
            Finding::Stale { measured } => freshness::Found::Stale {
                measured: *measured,
            },
        },
    });
    let mut passed = false;
    match &read.found {
        Finding::Reading(reading) => {
            record.held_from = Some(read.at);
            record.skew = reading.skew;
            if let Some(sample) = &reading.sample {
                record.keep_sample(sample.clone());
            }
            if reading.mark.is_some() {
                record.file = reading.mark.clone();
            }
            let kept = Kept {
                t: reading.measured,
                v: reading.value.clone(),
                of: reading.of,
            };
            record.last_reading = Some(kept.clone());
            effects.push(Effect::Record(Line::Reading {
                t: reading.measured,
                v: reading.value.clone(),
                health: reading.band.map(Health::from),
                out_of_cadence: !read.in_cadence,
            }));
            // §FS-021-monitors.14.2: an unknown episode ends with the first reading.
            let value = reading.said.clone();
            end(
                record,
                watch,
                Sort::Unknown,
                Ending::ReadAgain { value },
                read.at,
                effects,
            );
            if let (Some(band), false) = (reading.band, watch.judge.is_kpi()) {
                passed = band.passes();
                if passed {
                    record.last_ok = Some(kept);
                }
                hold(record, watch, band, reading, read.at, effects);
            }
        }
        Finding::Unreadable { .. } => effects.push(Effect::Record(Line::marker(read.at))),
        Finding::Parked | Finding::Stale { .. } => {}
    }
    after_work(record, read, passed, now, effects);
}

/// The hold (§FS-021-monitors.10): failing readings in a row open a drop,
/// passing ones in a row end it, and between ok and warn there is none.
fn hold(
    record: &mut Record,
    watch: &Watch,
    band: Band,
    reading: &Reading,
    at: DateTime<Utc>,
    effects: &mut Vec<Effect>,
) {
    let passes = band.passes();
    let hold = &mut record.hold;
    hold.run = match hold.band {
        Some(last) if last.passes() == passes => hold.run.saturating_add(1),
        _ => 1,
    };
    hold.band = Some(band);
    let run = hold.run;
    if let Some(drop) = &mut record.drop {
        if passes && run >= watch.recover_after {
            settle(&mut record.hold, band.into(), at);
            let value = reading.said.clone();
            end(
                record,
                watch,
                Sort::Drop,
                Ending::Recovered { value },
                at,
                effects,
            );
        } else if let (Value::Level(level), Some(levels)) = (&reading.value, watch.levels) {
            // §FS-021-monitors.13.1: a level worse than any the drop has reached.
            let rank = |name: &str| levels.iter().position(|known| known == name);
            let worse = match &drop.worst {
                Some(worst) => rank(level) > rank(worst),
                None => true,
            };
            if worse {
                let from = drop.worst.replace(level.clone()).unwrap_or_default();
                let why = Movement::LevelWorse {
                    from,
                    to: level.clone(),
                };
                drop.moved = Some(Moved {
                    at,
                    why: why.clone(),
                });
                effects.push(Effect::Moved {
                    sort: Sort::Drop,
                    named: drop.named,
                    why,
                });
            }
        }
    } else if !passes {
        if run >= watch.drop_after {
            let named = episode::name(at, record.named.drop);
            record.named.drop = Some(named);
            record.drop = Some(OpenDrop {
                named,
                moved: None,
                worst: match &reading.value {
                    Value::Level(level) => Some(level.clone()),
                    _ => None,
                },
                judged_under: watch.judge.clone(),
                judged_words: watch.healthy_words.clone(),
                works_ended: Vec::new(),
                renewed: None,
                shown: Shown::default(),
            });
            settle(&mut record.hold, Health::Down, at);
            effects.push(Effect::Opened {
                sort: Sort::Drop,
                named,
            });
        }
    } else {
        settle(&mut record.hold, band.into(), at);
    }
}

fn settle(hold: &mut Hold, health: Health, at: DateTime<Utc>) {
    if hold.health != Some(health) {
        hold.health = Some(health);
        hold.since = Some(at);
    }
}

/// Still down, or still unknown, after the work laid on it ended
/// (§FS-021-monitors.15): once per ended work, while its episode is open.
fn after_work(
    record: &mut Record,
    read: &Read,
    passed: bool,
    now: DateTime<Utc>,
    effects: &mut Vec<Effect>,
) {
    let cause = match &read.found {
        Finding::Reading(_) => None,
        Finding::Unreadable { .. } => Some("unreadable"),
        Finding::Parked => Some("parked"),
        Finding::Stale { .. } => Some("stale"),
    };
    for sort in [Sort::Drop, Sort::Unknown] {
        let works = read.after_work.iter().filter(|ended| ended.on == sort);
        let (seen, named, moved, renewed) = match (sort, &mut record.drop, &mut record.unknown) {
            (Sort::Drop, Some(drop), _) => (
                &mut drop.works_ended,
                drop.named,
                &mut drop.moved,
                &mut drop.renewed,
            ),
            (Sort::Unknown, _, Some(unknown)) => (
                &mut unknown.works_ended,
                unknown.named,
                &mut unknown.moved,
                &mut unknown.renewed,
            ),
            _ => continue,
        };
        let mut new = false;
        for ended in works {
            if !seen.contains(&ended.work) {
                seen.push(ended.work.clone());
                new = true;
            }
        }
        let why = match (sort, cause) {
            (Sort::Drop, _) if !passed => Some(Movement::StillDown),
            (Sort::Unknown, Some(cause)) => Some(Movement::StillUnknown {
                cause: cause.to_string(),
            }),
            _ => None,
        };
        if let (true, Some(why)) = (new, why) {
            *renewed = Some(now);
            *moved = Some(Moved {
                at: now,
                why: why.clone(),
            });
            effects.push(Effect::Moved { sort, named, why });
        }
    }
}

/// Freshness after the step (§FS-021-monitors.7.4): an unknown status opens
/// an unknown episode, named at the moment its value stopped holding, unless
/// the cause is `overdue` (§FS-021-monitors.11.2).
fn judge_freshness<Tz: TimeZone>(
    record: &mut Record,
    watch: &Watch,
    now: DateTime<Utc>,
    zone: &Tz,
    slot_error: Option<&str>,
    effects: &mut Vec<Effect>,
) {
    let freshness = record.freshness(&watch.cadence, now, zone, slot_error);
    record.cause = freshness.cause().cloned();
    let Freshness::Unknown { cause, since } = freshness else {
        return;
    };
    if watch.judge.is_kpi() || cause == Cause::Overdue {
        return;
    }
    match &mut record.unknown {
        Some(open) => open.cause = cause,
        None => {
            let named = episode::name(since, record.named.unknown);
            record.named.unknown = Some(named);
            record.unknown = Some(OpenUnknown {
                named,
                moved: None,
                cause,
                works_ended: Vec::new(),
                renewed: None,
                shown: Shown::default(),
            });
            effects.push(Effect::Opened {
                sort: Sort::Unknown,
                named,
            });
        }
    }
}

/// Ends every open episode (§FS-021-monitors.14.4); the hold starts again.
fn end_all(
    record: &mut Record,
    watch: &Watch,
    why: Unwatched,
    at: DateTime<Utc>,
    effects: &mut Vec<Effect>,
) {
    for sort in [Sort::Drop, Sort::Unknown] {
        let ending = Ending::Unwatched { why: why.clone() };
        end(record, watch, sort, ending, at, effects);
    }
    record.hold = Hold::default();
}

/// Ends the open episode of `sort`, if there is one, and keeps it with what
/// its finished matter says (§FS-021-monitors.14.3).
fn end(
    record: &mut Record,
    watch: &Watch,
    sort: Sort,
    ending: Ending,
    at: DateTime<Utc>,
    effects: &mut Vec<Effect>,
) {
    let (named, moved, cause) = match sort {
        Sort::Drop => match record.drop.take() {
            Some(drop) => (drop.named, drop.moved, None),
            None => return,
        },
        Sort::Unknown => match record.unknown.take() {
            Some(unknown) => (unknown.named, unknown.moved, Some(unknown.cause)),
            None => return,
        },
    };
    let shown = match sort {
        Sort::Drop => shown(record, watch),
        Sort::Unknown => unknown_since(shown(record, watch), named),
    };
    record.past.push(Past {
        sort,
        named,
        ended: at,
        how: ending.clone(),
        moved,
        cause,
        shown,
    });
    effects.push(Effect::Ended {
        sort,
        named,
        ending,
    });
}

#[cfg(test)]
#[path = "machine_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "machine_episode_tests.rs"]
mod episode_tests;
