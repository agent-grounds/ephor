//! Which monitor each sample of an answer is for (§FS-021-monitors.5.6).
//!
//! A pure function over the declarations, the store's owner map, the
//! answers a run took and the monitors that were due. Running the commands
//! and reading the files that gave those answers is the caller's.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::monitors::cadence::{self, Cadence};
use crate::monitors::model::{Sample, Source};
use crate::monitors::overlay::Monitor;
use crate::monitors::shape;

/// How long a monitor nothing declared is kept with no answer bringing it
/// (§FS-021-monitors.5.6).
pub const FORGOTTEN_AFTER_DAYS: i64 = 30;

/// Where a declared monitor is read from (§FS-021-monitors.4.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Its `from`'s answer only.
    From(Source),
    /// The answer that brings it: it names no `from`.
    Brought,
    /// Nowhere: its declaration is in error (§FS-021-monitors.4.6), or its
    /// registry row runs no command for it (§FS-021-monitors.4.5). A sample
    /// for it in any answer is ignored.
    NotRead,
}

impl Route {
    /// Where a resolved monitor is read from.
    pub fn of(monitor: &Monitor) -> Route {
        match &monitor.from {
            _ if !monitor.is_readable() => Route::NotRead,
            Some(source) => Route::From(source.clone()),
            None => Route::Brought,
        }
    }
}

/// An answer, by where it came from: the answer that owns a monitor read
/// from the answer that brings it, as the store records it
/// (§FS-021-monitors.5.6, §FS-021-monitors.9.1).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    /// A `from` some declared monitor names.
    From(Source),
    /// The project's custom-status answer (§FS-021-monitors.8.3).
    CustomStatus,
}

/// What taking one answer came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Its entries, each a sample or not.
    Answered(Vec<Json>),
    /// It failed, and why (§FS-021-monitors.5.5).
    Failed(String),
    /// Its command exited 75.
    Parked,
}

/// One answer a run took, in the order it took them.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub source: Owner,
    pub outcome: Outcome,
}

/// What a run gives one monitor (§FS-021-monitors.5.6).
#[derive(Debug, Clone, PartialEq)]
pub enum Delivery {
    /// The one sample for it, in its published shape. Whether it fits the
    /// monitor is [`crate::monitors::sample::check`]'s.
    Sample(Box<Sample>),
    /// An unreadable read of it, with the sentence that says why.
    Unreadable(String),
    /// A parked read of it.
    Parked,
}

/// Everything one run gives the project's monitors.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Routed {
    /// Per monitor id, what this run read for it. A monitor absent here
    /// records nothing (§FS-021-monitors.6.6).
    pub deliveries: BTreeMap<String, Delivery>,
    /// Owners this run gave or took back, the whole map as it now stands.
    pub owners: BTreeMap<String, Owner>,
    /// Monitors brought by an answer in this run, which keeps one nothing
    /// declared from being forgotten.
    pub brought: BTreeSet<String>,
    /// Entries read for no monitor, noted on the refresh
    /// (§FS-021-monitors.8.2).
    pub strays: Vec<String>,
}

/// What routing reads: the project's declared monitors by id, the owners
/// the store records, and which monitors are due.
#[derive(Debug, Clone, Copy)]
pub struct Watched<'a> {
    pub routes: &'a BTreeMap<String, Route>,
    pub owners: &'a BTreeMap<String, Owner>,
    pub due: &'a BTreeSet<String>,
    /// A plain refresh of the project, rather than one read out of cadence
    /// for chosen monitors: the one that says `no answer carries <id>`.
    pub plain: bool,
}

/// The ids a run that takes `source`'s answer reads (§FS-021-monitors.5.4,
/// §FS-021-monitors.5.6), which `EPHOR_MONITORS` names so a script that
/// answers for many monitors may compute only those: the due monitors that
/// name it as their `from`, the monitors it owns, and, on a plain refresh,
/// every declared monitor that names no `from` while no answer owns it. A
/// monitor nothing declared is named once an answer has brought it.
pub fn reads(watched: &Watched, source: &Owner) -> BTreeSet<String> {
    let owners = kept_owners(watched);
    let mut reads = taken(watched, &owners, source);
    if watched.plain {
        reads.extend(
            watched
                .routes
                .iter()
                .filter(|(id, route)| **route == Route::Brought && !owners.contains_key(*id))
                .map(|(id, _)| id.clone()),
        );
    }
    reads
}

/// The owners the store records that still hold (§FS-021-monitors.5.6): an
/// owner whose `from` no declared monitor names any more is gone, and so is
/// one of a monitor now read from its own `from`, or not read at all.
fn kept_owners(watched: &Watched) -> BTreeMap<String, Owner> {
    let named: BTreeSet<&Source> = watched
        .routes
        .values()
        .filter_map(|route| match route {
            Route::From(source) => Some(source),
            Route::Brought | Route::NotRead => None,
        })
        .collect();
    watched
        .owners
        .iter()
        .filter(|(id, owner)| {
            matches!(watched.routes.get(*id), Some(Route::Brought) | None)
                && match owner {
                    Owner::From(source) => named.contains(source),
                    Owner::CustomStatus => true,
                }
        })
        .map(|(id, owner)| (id.clone(), owner.clone()))
        .collect()
}

/// The monitors an answer from `source` is taken to read before it is
/// looked at: the due ones that name it, and the ones it owns.
fn taken(watched: &Watched, owners: &BTreeMap<String, Owner>, source: &Owner) -> BTreeSet<String> {
    let due_from = watched.routes.iter().filter(|(id, route)| {
        matches!((route, source), (Route::From(from), Owner::From(source)) if from == source)
            && watched.due.contains(*id)
    });
    due_from
        .map(|(id, _)| id.clone())
        .chain(
            owners
                .iter()
                .filter(|(_, owner)| *owner == source)
                .map(|(id, _)| id.clone()),
        )
        .collect()
}

/// Gives each monitor the sample carrying its id, from the answer it is read
/// from (§FS-021-monitors.5.6): a declared `from`'s answer while it is due
/// (§FS-021-monitors.6.6), and otherwise the answer that owns it, the first
/// to bring it owning it.
pub fn route(watched: &Watched, answers: &[Answer]) -> Routed {
    let mut owners = kept_owners(watched);
    let mut routed = Routed::default();

    for answer in answers {
        let mut carried: BTreeMap<String, Vec<&Json>> = BTreeMap::new();
        if let Outcome::Answered(entries) = &answer.outcome {
            for entry in entries {
                match shape::sample_id(entry) {
                    Ok(id) => carried.entry(id).or_default().push(entry),
                    Err(fault) => routed.strays.push(fault.sentence),
                }
            }
        }
        // The monitors this answer was taken to read.
        let mut taken = taken(watched, &owners, &answer.source);
        // The first answer to bring an unowned monitor owns it.
        for id in carried.keys() {
            let free = !owners.contains_key(id)
                && matches!(watched.routes.get(id), Some(Route::Brought) | None);
            if free {
                owners.insert(id.clone(), answer.source.clone());
                taken.insert(id.clone());
            }
        }
        for id in taken {
            if routed.deliveries.contains_key(&id) {
                continue;
            }
            let delivery = match &answer.outcome {
                Outcome::Failed(why) => Delivery::Unreadable(why.clone()),
                Outcome::Parked => Delivery::Parked,
                Outcome::Answered(_) => match carried.get(&id).map(Vec::as_slice) {
                    None | Some([]) => Delivery::Unreadable(format!("no sample for {id}")),
                    Some([one]) => match shape::read_sample(one) {
                        Ok(sample) => Delivery::Sample(Box::new(sample)),
                        Err(fault) => Delivery::Unreadable(fault.sentence),
                    },
                    Some(_) => Delivery::Unreadable(format!("two samples for {id}")),
                },
            };
            if carried.contains_key(&id) {
                routed.brought.insert(id.clone());
            }
            routed.deliveries.insert(id, delivery);
        }
    }

    if watched.plain {
        for (id, route) in watched.routes {
            if *route == Route::Brought && !owners.contains_key(id) {
                routed
                    .deliveries
                    .entry(id.clone())
                    .or_insert_with(|| Delivery::Unreadable(format!("no answer carries {id}")));
            }
        }
    }
    routed.owners = owners;
    routed
}

/// Whether a monitor is forgotten: one nothing declared that no answer has
/// brought for 30 days (§FS-021-monitors.5.6). A declared monitor never is.
pub fn forgotten(declared: bool, last_brought: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    !declared && now.signed_duration_since(last_brought) >= TimeDelta::days(FORGOTTEN_AFTER_DAYS)
}

/// The cadence a monitor read from the answer that brings it takes
/// (§FS-021-monitors.6.8): the fastest effective cadence among the declared
/// monitors that name its owner's `from`, and the refresh's own, judged as
/// `every: 10m`, for a custom-status answer or before any answer owns it.
/// `declared` is each declared monitor's `from` and effective cadence.
pub fn brought_cadence<'a>(
    owner: Option<&Owner>,
    declared: impl IntoIterator<Item = (&'a Source, Cadence)>,
) -> Cadence {
    match owner {
        Some(Owner::From(owner)) => cadence::fastest(
            declared
                .into_iter()
                .filter(|(source, _)| *source == owner)
                .map(|(_, cadence)| cadence),
        )
        .unwrap_or_else(Cadence::default_every),
        _ => Cadence::default_every(),
    }
}

#[cfg(test)]
#[path = "route_tests.rs"]
mod tests;
