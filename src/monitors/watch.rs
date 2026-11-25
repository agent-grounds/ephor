//! The monitor slot (§FS-021-monitors.17): what a plain refresh of a project
//! does with its monitors, in three steps.
//!
//! 1. Under the store's lock, it judges which monitors are due
//!    (§FS-021-monitors.6.4) and claims them (§FS-021-monitors.6.9), passing
//!    over any another refresh holds a live claim on.
//! 2. With the lock let go, it reads each distinct `from` once, in parallel
//!    (§FS-021-monitors.5.1).
//! 3. Under the lock again, over the store as it is then, it routes every
//!    answer's samples (§FS-021-monitors.5.6), steps each monitor through the
//!    machine, appends what each kept to its history, lets its claims go, and
//!    thins the histories when a day has passed (§FS-021-monitors.9.3).
//!
//! One monitor's failure is that monitor's (§FS-021-monitors.17.2): only the
//! machinery failing — the store, the manifest, the configuration — fails the
//! slot (§FS-021-monitors.17.3).

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Local, TimeDelta, Utc};
use serde_json::Value as Json;

use crate::branches::Placement;
use crate::error::Result;
use crate::monitors::cadence::{self, Cadence, DEFAULT_TIMEOUT_SECONDS};
use crate::monitors::declared::{self, Declarations};
use crate::monitors::episode::Unwatched;
use crate::monitors::found;
use crate::monitors::freshness::Cause;
use crate::monitors::machine::{self, Effect, Event, Read};
use crate::monitors::model::Source;
use crate::monitors::overlay::Monitor;
use crate::monitors::reader::{self, Got, Job, Where};
use crate::monitors::retention;
use crate::monitors::route::{self, Answer, Outcome, Owner, Route, Watched};
use crate::monitors::state::{Claim, Document, Record};
use crate::monitors::store::Store;

/// The name of the slot a project's monitors answer in, ephor's own reading
/// rather than a source anything binds (§FS-021-monitors.17.4).
pub const SLOT: &str = "monitor";

/// Which monitors a plain refresh reads: the due ones, or every one now and
/// out of cadence (§FS-021-monitors.6.7, §FS-021-monitors.21.4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum When {
    #[default]
    Due,
    Now,
}

/// One plain refresh of one project's monitors.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    pub project: &'a str,
    pub placement: Option<&'a Placement>,
    /// The site's declarations for the project (§FS-021-monitors.4.2).
    pub site: &'a [Json],
    /// What each custom-status answer of this refresh carried in its
    /// `monitors` (§FS-021-monitors.8.3).
    pub brought: Vec<Outcome>,
    pub when: When,
    /// How long a forgotten monitor's finished matters are still reported
    /// (§FS-021-monitors.14.3).
    pub recent_days: u64,
}

/// What the refresh does with the slot, and what it notes.
#[derive(Debug, Default)]
pub struct Report {
    /// None where the project has no monitors, and so no slot
    /// (§FS-021-monitors.1.4); the machinery's failure otherwise
    /// (§FS-021-monitors.17.3).
    pub slot: Option<std::result::Result<(), String>>,
    pub notes: Vec<String>,
}

/// What step one decided and step three holds it to.
#[derive(Debug, Clone)]
struct Plan {
    jobs: Vec<Job>,
    /// The monitors whose `from` this run reads.
    due: BTreeSet<String>,
    /// The due monitors whose slot had come, as opposed to the ones read now
    /// out of cadence (§FS-021-monitors.6.7).
    in_cadence: BTreeSet<String>,
    claim: Claim,
}

/// Reads a project's monitors on a plain refresh, in the store ephor keeps.
pub fn answer(request: &Request) -> Report {
    refresh(&Store::of(request.project), request, Utc::now())
}

/// Reads a project's monitors on a plain refresh.
pub fn refresh(store: &Store, request: &Request, now: DateTime<Utc>) -> Report {
    let declarations = declared::read(request.placement, request.site);
    let mut notes = declarations.notes();
    let kept = match store.load() {
        Ok(kept) => kept,
        Err(err) => return failed(err.to_string(), notes),
    };
    if let Some(error) = &declarations.manifest_error {
        if declarations.site_declares() || kept.keeps_any() {
            return failed(format!("the manifest cannot be read: {error}"), notes);
        }
        // Nobody can tell whether it declares any (§FS-021-monitors.17.3).
        notes.push(format!(
            "no monitor of the project is read: its manifest cannot be read: {error}"
        ));
        return Report { slot: None, notes };
    }
    let brings = request
        .brought
        .iter()
        .any(|outcome| matches!(outcome, Outcome::Answered(samples) if !samples.is_empty()));
    if !declarations.any() && !kept.keeps_any() && !brings {
        return Report { slot: None, notes };
    }

    let plan = match store.update(|document| Ok(prepare(document, &declarations, request, now))) {
        Ok(plan) => plan,
        Err(err) => return failed(err.to_string(), notes),
    };
    let checkout = request.placement.map(declared::checkout);
    let forest = request
        .placement
        .zip(checkout.as_ref())
        .map(|(placement, checkout)| placement.forest(checkout));
    let place = request
        .placement
        .zip(checkout.as_ref())
        .map(|(placement, checkout)| Where {
            project: request.project,
            root: &placement.root,
            checkout,
            organization: placement.organization.as_ref(),
            forest: forest.as_ref(),
        });
    let got = reader::read_all(&plan.jobs, place.as_ref());
    match store.update(|document| commit(store, document, &declarations, request, &plan, &got, now))
    {
        Ok(said) => {
            notes.extend(said);
            Report {
                slot: Some(Ok(())),
                notes,
            }
        }
        Err(err) => failed(err.to_string(), notes),
    }
}

/// Every custom-status answer of one refresh, as the one answer they are to
/// routing (§FS-021-monitors.8.3): the samples of every block that answered,
/// or, where none did, the first that parked or failed.
fn merged(brought: &[Outcome]) -> Option<Outcome> {
    if brought
        .iter()
        .any(|outcome| matches!(outcome, Outcome::Answered(_)))
    {
        return Some(Outcome::Answered(
            brought
                .iter()
                .filter_map(|outcome| match outcome {
                    Outcome::Answered(samples) => Some(samples.clone()),
                    _ => None,
                })
                .flatten()
                .collect(),
        ));
    }
    brought
        .iter()
        .find(|outcome| matches!(outcome, Outcome::Parked))
        .or(brought.first())
        .cloned()
}

fn failed(why: String, notes: Vec<String>) -> Report {
    Report {
        slot: Some(Err(why)),
        notes,
    }
}

/// Every declared monitor, resolved over what the store keeps of it.
fn resolved(declarations: &Declarations, document: &Document) -> BTreeMap<String, Monitor> {
    declarations
        .ids()
        .into_iter()
        .map(|id| {
            let monitor = declarations.resolve(&id, document.monitors.get(&id));
            (id, monitor)
        })
        .collect()
}

fn routes(monitors: &BTreeMap<String, Monitor>) -> BTreeMap<String, Route> {
    monitors
        .iter()
        .map(|(id, monitor)| (id.clone(), Route::of(monitor)))
        .collect()
}

fn owners(document: &Document) -> BTreeMap<String, Owner> {
    document
        .monitors
        .iter()
        .filter_map(|(id, record)| record.owner.clone().map(|owner| (id.clone(), owner)))
        .collect()
}

fn timeout(monitors: &BTreeMap<String, Monitor>, id: &str) -> u64 {
    monitors
        .get(id)
        .map_or(DEFAULT_TIMEOUT_SECONDS, |monitor| monitor.timeout)
}

/// Whether another read holds a live claim on a monitor
/// (§FS-021-monitors.6.9): one older than its timeout and a minute is void.
fn held_elsewhere(record: &Record, mine: &Claim, timeout: u64, now: DateTime<Utc>) -> bool {
    record
        .claim
        .as_ref()
        .is_some_and(|claim| !is_mine(claim, mine) && !claim.is_void(timeout, now))
}

/// A claim is this read's by its process and its time; the slot it names is
/// each monitor's own.
fn is_mine(claim: &Claim, mine: &Claim) -> bool {
    claim.pid == mine.pid && claim.at == mine.at
}

/// Step one, under the lock: what is due, who reads it, and the claims.
fn prepare(
    document: &mut Document,
    declarations: &Declarations,
    request: &Request,
    now: DateTime<Utc>,
) -> Plan {
    let monitors = resolved(declarations, document);
    for (id, monitor) in &monitors {
        if monitor.is_readable() {
            document
                .monitors
                .entry(id.clone())
                .or_insert_with(|| Record::new(now));
        }
    }
    let claim = Claim {
        pid: std::process::id(),
        at: now,
        slot: None,
    };
    let elsewhere: BTreeSet<String> = document
        .monitors
        .iter()
        .filter(|(id, record)| held_elsewhere(record, &claim, timeout(&monitors, id), now))
        .map(|(id, _)| id.clone())
        .collect();
    let routes = routes(&monitors);
    let mut due = BTreeSet::new();
    let mut in_cadence = BTreeSet::new();
    for (id, monitor) in &monitors {
        let (Some(Route::From(_)), Some(record)) = (routes.get(id), document.monitors.get(id))
        else {
            continue;
        };
        if elsewhere.contains(id) {
            continue;
        }
        let slot_came = record.schedule.is_due(&monitor.cadence, now, &Local);
        if slot_came {
            in_cadence.insert(id.clone());
        }
        if slot_came || request.when == When::Now {
            due.insert(id.clone());
        }
    }
    let owners = owners(document);
    let watched = Watched {
        routes: &routes,
        owners: &owners,
        due: &due,
        plain: true,
    };
    let sources: BTreeSet<&Source> = due
        .iter()
        .filter_map(|id| match routes.get(id) {
            Some(Route::From(source)) => Some(source),
            _ => None,
        })
        .collect();
    let jobs: Vec<Job> = sources
        .into_iter()
        .map(|source| {
            let reads: BTreeSet<String> = route::reads(&watched, &Owner::From(source.clone()))
                .difference(&elsewhere)
                .cloned()
                .collect();
            let timeout = cadence::run_timeout(reads.iter().map(|id| timeout(&monitors, id)));
            Job {
                source: source.clone(),
                reads,
                timeout,
            }
        })
        .collect();
    let mut claimed: BTreeSet<String> = jobs.iter().flat_map(|job| job.reads.clone()).collect();
    if !request.brought.is_empty() {
        claimed.extend(
            route::reads(&watched, &Owner::CustomStatus)
                .difference(&elsewhere)
                .cloned(),
        );
    }
    for id in claimed {
        let cadence = monitors
            .get(&id)
            .map_or_else(Cadence::default_every, |monitor| monitor.cadence);
        let record = document
            .monitors
            .entry(id)
            .or_insert_with(|| Record::new(now));
        record.claim = Some(Claim {
            slot: record.schedule.next_slot(&cadence, now, &Local),
            ..claim.clone()
        });
    }
    Plan {
        jobs,
        due,
        in_cadence,
        claim,
    }
}

/// Step three, under the lock again: route, step, keep, let go, thin.
fn commit(
    store: &Store,
    document: &mut Document,
    declarations: &Declarations,
    request: &Request,
    plan: &Plan,
    got: &[Got],
    now: DateTime<Utc>,
) -> Result<Vec<String>> {
    let monitors = resolved(declarations, document);
    let routes = routes(&monitors);
    let owners = owners(document);
    let watched = Watched {
        routes: &routes,
        owners: &owners,
        due: &plan.due,
        plain: true,
    };
    let answers: Vec<Answer> = got
        .iter()
        .map(|got| Answer {
            source: Owner::From(got.source.clone()),
            outcome: got.outcome.clone(),
        })
        .chain(merged(&request.brought).map(|outcome| Answer {
            source: Owner::CustomStatus,
            outcome,
        }))
        .collect();
    let routed = route::route(&watched, &answers);
    let notes: Vec<String> = routed
        .strays
        .iter()
        .map(|stray| format!("a sample is not read: {stray}"))
        .collect();

    for id in routed.owners.keys().chain(&routed.brought) {
        document
            .monitors
            .entry(id.clone())
            .or_insert_with(|| Record::new(now));
    }
    for (id, record) in document.monitors.iter_mut() {
        record.owner = routed.owners.get(id).cloned();
        if routed.brought.contains(id) {
            record.last_brought = Some(now);
        }
    }

    let declared: Vec<(Source, Cadence)> = monitors
        .values()
        .filter_map(|monitor| monitor.from.clone().map(|from| (from, monitor.cadence)))
        .collect();
    let marks: BTreeMap<&Source, &crate::monitors::state::FileMark> = got
        .iter()
        .filter_map(|got| got.mark.as_ref().map(|mark| (&got.source, mark)))
        .collect();
    let mut gone = Vec::new();
    let ids: Vec<String> = document.monitors.keys().cloned().collect();
    for id in ids {
        let snapshot = document.monitors[&id].clone();
        if held_elsewhere(&snapshot, &plan.claim, timeout(&monitors, &id), now) {
            // Read by another refresh: its state is what that one says.
            continue;
        }
        let monitor = monitors
            .get(&id)
            .cloned()
            .unwrap_or_else(|| declarations.resolve(&id, Some(&snapshot)));
        let from = match routes.get(&id) {
            Some(Route::From(source)) => Some(source),
            _ => match &snapshot.owner {
                Some(Owner::From(source)) => Some(source),
                _ => None,
            },
        };
        let cadence = match routes.get(&id) {
            Some(Route::From(_)) => monitor.cadence,
            _ => route::brought_cadence(
                snapshot.owner.as_ref(),
                declared.iter().map(|(source, cadence)| (source, *cadence)),
            ),
        };
        let (event, found) = match routed.deliveries.get(&id) {
            Some(delivery) => {
                let homes = declarations.homes(&id, Some(&snapshot));
                let mark = from.and_then(|from| marks.get(from).copied());
                let found = found::found(delivery, monitor, &homes, &snapshot, mark, &cadence, now);
                let out_of_cadence = request.when == When::Now
                    && matches!(routes.get(&id), Some(Route::From(_)))
                    && !plan.in_cadence.contains(&id);
                let event = Event::Read(Read {
                    at: now,
                    in_cadence: !out_of_cadence,
                    found: found.finding.clone(),
                    after_work: Vec::new(),
                });
                (event, found)
            }
            None => {
                let event = match monitors.get(&id) {
                    Some(declared) if !declared.is_readable() => {
                        Event::Unwatched(match declared.not_run && declared.faults.is_empty() {
                            true => Unwatched::NotRun,
                            false => Unwatched::InError,
                        })
                    }
                    Some(_) => Event::Look { slot_error: None },
                    None if route::forgotten(
                        false,
                        snapshot
                            .last_brought
                            .unwrap_or(snapshot.schedule.first_seen),
                        now,
                    ) =>
                    {
                        gone.push(id.clone());
                        Event::Unwatched(Unwatched::Gone)
                    }
                    None => Event::Look { slot_error: None },
                };
                let found = found::Found {
                    finding: machine::Finding::Parked,
                    ok: snapshot.last_sample.as_ref().and_then(|sample| sample.ok),
                    of: snapshot
                        .last_sample
                        .as_ref()
                        .and_then(|sample| sample.of)
                        .or(monitor.of),
                    monitor,
                };
                (event, found)
            }
        };
        let watch = found::watch(&found.monitor, found.ok, found.of, snapshot.kind, cadence);
        let record = document
            .monitors
            .get_mut(&id)
            .expect("every id stepped is kept");
        let effects = machine::step(record, &watch, &event, now, &Local);
        keep(store, &id, effects)?;
        if record
            .claim
            .as_ref()
            .is_some_and(|claim| is_mine(claim, &plan.claim))
            || record
                .claim
                .as_ref()
                .is_some_and(|claim| claim.is_void(timeout(&monitors, &id), now))
        {
            record.claim = None;
        }
    }
    for id in gone {
        if let Some(record) = document.monitors.remove(&id) {
            if !record.past.is_empty() {
                document
                    .forgotten
                    .entry(id.clone())
                    .or_default()
                    .extend(record.past);
            }
        }
        store.remove(&id)?;
    }
    let recent = TimeDelta::days(i64::try_from(request.recent_days).unwrap_or(7));
    document.prune_forgotten(recent, now);
    if document.due_to_thin(now) {
        thin(store, document, &monitors, now)?;
    }
    Ok(notes)
}

/// Appends what a step kept to the monitor's history, starting it again
/// where the step says to (§FS-021-monitors.4.3).
fn keep(store: &Store, id: &str, effects: Vec<Effect>) -> Result<()> {
    let mut lines = Vec::new();
    for effect in effects {
        match effect {
            Effect::Record(line) => lines.push(line),
            Effect::RestartHistory => {
                store.remove(id)?;
                lines.clear();
            }
            Effect::Opened { .. } | Effect::Moved { .. } | Effect::Ended { .. } => {}
        }
    }
    store.append(id, &lines)
}

/// Thins every history and lets go of the episodes no history keeps any
/// more, at most once a day and under the lock (§FS-021-monitors.9.3).
fn thin(
    store: &Store,
    document: &mut Document,
    monitors: &BTreeMap<String, Monitor>,
    now: DateTime<Utc>,
) -> Result<()> {
    for (id, record) in document.monitors.iter_mut() {
        let Some(kind) = record.kind else {
            continue;
        };
        let monitor = monitors.get(id);
        let interval = monitor
            .map_or_else(Cadence::default_every, |monitor| monitor.cadence)
            .interval();
        let lines = store.history(id)?;
        let kept = retention::thin(
            &lines,
            interval,
            kind,
            monitor.and_then(|monitor| monitor.levels.as_deref()),
            now,
        );
        if kept != lines {
            store.rewrite(id, &kept)?;
        }
        record.past = retention::past(&record.past, &kept, now);
    }
    document.thinned = Some(now);
    Ok(())
}

/// The monitors of a project that were overdue at `now`
/// (§FS-021-monitors.20.2): what the doctor says before its own refresh reads
/// them.
pub fn overdue(
    store: &Store,
    placement: Option<&Placement>,
    site: &[Json],
    now: DateTime<Utc>,
) -> Vec<String> {
    let declarations = declared::read(placement, site);
    let Ok(document) = store.load() else {
        return Vec::new();
    };
    declarations
        .ids()
        .into_iter()
        .filter(|id| {
            let Some(record) = document.monitors.get(id) else {
                return false;
            };
            let monitor = declarations.resolve(id, Some(record));
            monitor.is_readable()
                && matches!(
                    record
                        .freshness(&monitor.cadence, now, &Local, None)
                        .cause(),
                    Some(Cause::Overdue)
                )
        })
        .collect()
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
