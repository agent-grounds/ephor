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
use crate::monitors::condition::Judge;
use crate::monitors::episode::{self, Ending, Movement, Sort, Unwatched};
use crate::monitors::freshness::{self, Attempt, Cause, Freshness};
use crate::monitors::history::Line;
use crate::monitors::model::{Band, Health, Kind, Value};
use crate::monitors::state::{Hold, Kept, OpenDrop, OpenUnknown, Past, Record};

/// How a monitor is watched as of this step: what its homes resolve to
/// (§FS-021-monitors.4.4).
#[derive(Debug, Clone)]
pub struct Watch<'a> {
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
    /// When it was measured (§FS-021-monitors.7.1).
    pub measured: DateTime<Utc>,
    pub skew: Option<DateTime<FixedOffset>>,
    /// The accepted sample, whole, which the store keeps.
    pub sample: Option<serde_json::Value>,
}

/// What one read found (§FS-021-monitors.9.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
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
    pub found: Found,
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
            end_all(record, Unwatched::KindChanged, now, &mut effects);
            record.last_ok = None;
            record.last_reading = None;
            effects.push(Effect::RestartHistory);
        }
    }
    if watch.kind.is_some() {
        record.kind = watch.kind;
    }
    if let Event::Unwatched(why) = event {
        end_all(record, why.clone(), now, &mut effects);
        return effects;
    }
    if watch.judge.is_kpi() {
        end_all(record, Unwatched::Kpi, now, &mut effects);
    } else if let Some(drop) = &mut record.drop {
        // §FS-021-monitors.13.1: the healthy that judges it changed, from whichever home.
        if drop.judged_under != watch.judge {
            let why = Movement::ConditionChanged {
                from: std::mem::replace(&mut drop.judged_words, watch.healthy_words.clone()),
                to: watch.healthy_words.clone(),
            };
            drop.judged_under = watch.judge.clone();
            drop.moved = Some(now);
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
    effects
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
    let parked = read.found == Found::Parked;
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
            Found::Reading(_) => freshness::Found::Reading,
            Found::Unreadable { why } => freshness::Found::Unreadable { why: why.clone() },
            Found::Parked => freshness::Found::Parked,
            Found::Stale { measured } => freshness::Found::Stale {
                measured: *measured,
            },
        },
    });
    let mut passed = false;
    match &read.found {
        Found::Reading(reading) => {
            record.held_from = Some(read.at);
            record.skew = reading.skew;
            if reading.sample.is_some() {
                record.last_sample = reading.sample.clone();
            }
            let kept = Kept {
                t: reading.measured,
                v: reading.value.clone(),
            };
            record.last_reading = Some(kept.clone());
            effects.push(Effect::Record(Line::Reading {
                t: reading.measured,
                v: reading.value.clone(),
                health: reading.band.map(Health::from),
            }));
            // §FS-021-monitors.14.2: an unknown episode ends with the first reading.
            if let Some(unknown) = record.unknown.take() {
                end(
                    record,
                    Sort::Unknown,
                    unknown.named,
                    Ending::ReadAgain {
                        value: reading.said.clone(),
                    },
                    read.at,
                    effects,
                );
            }
            if let (Some(band), false) = (reading.band, watch.judge.is_kpi()) {
                passed = band.passes();
                if passed {
                    record.last_ok = Some(kept);
                }
                hold(record, watch, band, reading, read.at, effects);
            }
        }
        Found::Unreadable { .. } => effects.push(Effect::Record(Line::marker(read.at))),
        Found::Parked | Found::Stale { .. } => {}
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
            let named = drop.named;
            record.drop = None;
            settle(&mut record.hold, band.into(), at);
            let value = reading.said.clone();
            end(
                record,
                Sort::Drop,
                named,
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
                drop.moved = Some(at);
                effects.push(Effect::Moved {
                    sort: Sort::Drop,
                    named: drop.named,
                    why: Movement::LevelWorse {
                        from,
                        to: level.clone(),
                    },
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
        Found::Reading(_) => None,
        Found::Unreadable { .. } => Some("unreadable"),
        Found::Parked => Some("parked"),
        Found::Stale { .. } => Some("stale"),
    };
    for sort in [Sort::Drop, Sort::Unknown] {
        let works = read.after_work.iter().filter(|ended| ended.on == sort);
        let (seen, named, moved) = match (sort, &mut record.drop, &mut record.unknown) {
            (Sort::Drop, Some(drop), _) => (&mut drop.works_ended, drop.named, &mut drop.moved),
            (Sort::Unknown, _, Some(unknown)) => {
                (&mut unknown.works_ended, unknown.named, &mut unknown.moved)
            }
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
            *moved = Some(now);
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
            });
            effects.push(Effect::Opened {
                sort: Sort::Unknown,
                named,
            });
        }
    }
}

/// Ends every open episode (§FS-021-monitors.14.4); the hold starts again.
fn end_all(record: &mut Record, why: Unwatched, at: DateTime<Utc>, effects: &mut Vec<Effect>) {
    if let Some(drop) = record.drop.take() {
        let ending = Ending::Unwatched { why: why.clone() };
        end(record, Sort::Drop, drop.named, ending, at, effects);
    }
    if let Some(unknown) = record.unknown.take() {
        let ending = Ending::Unwatched { why };
        end(record, Sort::Unknown, unknown.named, ending, at, effects);
    }
    record.hold = Hold::default();
}

fn end(
    record: &mut Record,
    sort: Sort,
    named: DateTime<Utc>,
    ending: Ending,
    at: DateTime<Utc>,
    effects: &mut Vec<Effect>,
) {
    record.past.push(Past {
        sort,
        named,
        ended: at,
        how: ending.clone(),
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
