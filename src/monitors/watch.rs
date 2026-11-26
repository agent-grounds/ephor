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
use crate::feed::model::Item;
use crate::monitors::after;
use crate::monitors::cadence::{self, DEFAULT_TIMEOUT_SECONDS};
use crate::monitors::declared::{self, Declarations};
use crate::monitors::episode::Unwatched;
use crate::monitors::found;
use crate::monitors::history::Line;
use crate::monitors::known::{self, Known};
use crate::monitors::machine::{self, Effect, Event, Read};
use crate::monitors::matters;
use crate::monitors::model::Source;
use crate::monitors::overlay::Monitor;
use crate::monitors::reader::{self, Clock, Got, Job, Where};
use crate::monitors::retention;
use crate::monitors::route::{self, Answer, Outcome, Owner, Route, Watched};
use crate::monitors::state::{Claim, Document, FileMark, Record};
use crate::monitors::store::{Appends, Store};

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

/// Asks which matters of a project's monitors have work that ended, by key,
/// with a mark for that work (§FS-021-monitors.15): [`after::ended`], or what
/// a test says.
pub type Ended<'a> = &'a dyn Fn(&str) -> BTreeMap<String, String>;

/// One plain refresh of one project's monitors.
#[derive(Clone)]
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
    /// Asked only where one of the project's monitors has an episode open,
    /// as only an open episode's ended work is read again
    /// (§FS-021-monitors.15): a refresh of a project with none reads no
    /// ledger.
    pub ended: Ended<'a>,
}

/// What the refresh does with the slot, and what it notes.
#[derive(Debug, Default)]
pub struct Report {
    /// None where the project has no monitors, and so no slot
    /// (§FS-021-monitors.1.4); the machinery's failure otherwise
    /// (§FS-021-monitors.17.3).
    pub slot: Option<std::result::Result<(), String>>,
    /// The matters the slot reports where it answered: its monitors' open
    /// episodes and the finished ones still reported (§FS-021-monitors.14.3).
    pub items: Vec<Item>,
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
    /// The matters whose work has ended, as asked when this read began.
    ended: BTreeMap<String, String>,
}

/// Reads a project's monitors on a plain refresh, in the store ephor keeps.
pub fn answer(request: &Request) -> Report {
    refresh(&Store::of(request.project), request, Utc::now())
}

/// Reads a project's monitors on a plain refresh, which began at `now`.
pub fn refresh(store: &Store, request: &Request, now: DateTime<Utc>) -> Report {
    let clock = Clock::starting(now);
    let declarations = declared::read(request.placement, request.site);
    let mut notes = declarations.notes();
    let kept = match store.load() {
        Ok(kept) => kept,
        Err(err) => return failed(err.to_string(), notes),
    };
    notes.extend(declarations.faulted(&kept));
    if let Some(error) = &declarations.manifest_error {
        if declarations.site_declares() || kept.keeps_any() {
            return failed(format!("the manifest cannot be read: {error}"), notes);
        }
        // Nobody can tell whether it declares any (§FS-021-monitors.17.3).
        notes.push(format!(
            "no monitor of the project is read: its manifest cannot be read: {error}"
        ));
        return Report {
            slot: None,
            notes,
            ..Report::default()
        };
    }
    let brings = request
        .brought
        .iter()
        .any(|outcome| matches!(outcome, Outcome::Answered(samples) if !samples.is_empty()));
    if !declarations.any() && !kept.keeps_any() && !brings {
        return Report {
            slot: None,
            notes,
            ..Report::default()
        };
    }

    let open = kept
        .monitors
        .values()
        .any(|record| record.drop.is_some() || record.unknown.is_some());
    let ended = match open {
        true => (request.ended)(request.project),
        false => BTreeMap::new(),
    };
    let plan =
        match store.update(|document| Ok(prepare(document, &declarations, request, &ended, now))) {
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
    let got = reader::read_all(&plan.jobs, place.as_ref(), &clock);
    let committed = store.appending(|document, appends| {
        let read = (&declarations, request, &plan, got.as_slice());
        commit(store, document, appends, read, now)
    });
    match committed {
        Ok(said) => {
            notes.extend(said);
            match store.load() {
                Ok(document) => Report {
                    slot: Some(Ok(())),
                    items: items(&declarations, &document, request, now),
                    notes,
                },
                Err(err) => failed(err.to_string(), notes),
            }
        }
        Err(err) => {
            // Nothing of this read was kept, so its claims go with it and
            // the next refresh reads again (§FS-021-monitors.6.9).
            let _ = store.update(|document| {
                release(document, &plan.claim);
                Ok(())
            });
            failed(err.to_string(), notes)
        }
    }
}

/// Lets go of every claim this read holds.
fn release(document: &mut Document, mine: &Claim) {
    for record in document.monitors.values_mut() {
        if record
            .claim
            .as_ref()
            .is_some_and(|claim| is_mine(claim, mine))
        {
            record.claim = None;
        }
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

/// The slot's matters, built from the store as this refresh left it
/// (§FS-021-monitors.12), each drop proposing its monitor's own workflow
/// (§FS-021-monitors.16.1).
fn items(
    declarations: &Declarations,
    document: &Document,
    request: &Request,
    now: DateTime<Utc>,
) -> Vec<Item> {
    let monitors: BTreeMap<String, Monitor> = known::all(declarations, document)
        .into_iter()
        .map(|(id, known)| (id, known.monitor))
        .collect();
    matters::items(
        request.project,
        document,
        &monitors,
        now,
        request.recent_days,
        &Local,
    )
}

fn failed(why: String, notes: Vec<String>) -> Report {
    Report {
        items: Vec::new(),
        slot: Some(Err(why)),
        notes,
    }
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

/// The timeout of the run that reads a monitor: the longest among the
/// monitors that name its `from` (§FS-021-monitors.5.5), and its own where it
/// names none. A claim that keeps no timeout of its own is judged by it.
fn run_timeout(monitors: &BTreeMap<String, Monitor>, id: &str) -> u64 {
    let Some(monitor) = monitors.get(id) else {
        return DEFAULT_TIMEOUT_SECONDS;
    };
    match &monitor.from {
        Some(from) => cadence::run_timeout(
            monitors
                .values()
                .filter(|other| other.from.as_ref() == Some(from))
                .map(|other| other.timeout),
        ),
        None => monitor.timeout,
    }
}

/// Whether another read holds a live claim on a monitor
/// (§FS-021-monitors.6.9): one older than its run's timeout and a minute is
/// void.
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
    ended: &BTreeMap<String, String>,
    now: DateTime<Utc>,
) -> Plan {
    let monitors = known::declared(declarations, document);
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
        timeout: None,
    };
    let elsewhere: BTreeSet<String> = document
        .monitors
        .iter()
        .filter(|(id, record)| held_elsewhere(record, &claim, run_timeout(&monitors, id), now))
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
        // §FS-021-monitors.15: work laid on its matter ended, so it is read now.
        let after_work = !after::unseen(request.project, id, record, ended).is_empty();
        if slot_came || request.when == When::Now || after_work {
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
            let timeout = cadence::run_timeout(
                reads
                    .iter()
                    .filter_map(|id| monitors.get(id))
                    .map(|monitor| monitor.timeout),
            );
            Job {
                source: source.clone(),
                reads,
                timeout,
            }
        })
        .collect();
    // Each claim keeps the timeout of the run that reads it, the longest
    // where several runs do (§FS-021-monitors.6.9).
    let mut claimed: BTreeMap<String, u64> = BTreeMap::new();
    for job in &jobs {
        for id in &job.reads {
            let timeout = claimed.entry(id.clone()).or_default();
            *timeout = (*timeout).max(job.timeout);
        }
    }
    if !request.brought.is_empty() {
        for id in route::reads(&watched, &Owner::CustomStatus).difference(&elsewhere) {
            claimed
                .entry(id.clone())
                .or_insert_with(|| run_timeout(&monitors, id));
        }
    }
    let known = known::all(declarations, document);
    for (id, timeout) in claimed {
        let cadence = known
            .get(&id)
            .map_or_else(cadence::Cadence::default_every, |known| known.cadence);
        let record = document
            .monitors
            .entry(id)
            .or_insert_with(|| Record::new(now));
        record.claim = Some(Claim {
            slot: record.schedule.next_slot(&cadence, now, &Local),
            timeout: Some(timeout),
            ..claim.clone()
        });
    }
    Plan {
        jobs,
        due,
        in_cadence,
        claim,
        ended: ended.clone(),
    }
}

/// What one monitor's step keeps in its history: whether the history
/// starts again (§FS-021-monitors.4.4), and the lines it appends.
#[derive(Debug, Default)]
struct Kept {
    id: String,
    restart: bool,
    lines: Vec<Line>,
}

/// Step three, under the lock again: route, step, thin, keep, let go.
fn commit(
    store: &Store,
    document: &mut Document,
    appends: &mut Appends,
    (declarations, request, plan, got): (&Declarations, &Request, &Plan, &[Got]),
    now: DateTime<Utc>,
) -> Result<Vec<String>> {
    let monitors = known::declared(declarations, document);
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

    let froms = known::froms(&monitors);
    let marks: BTreeMap<&Source, &FileMark> = got
        .iter()
        .filter_map(|got| got.mark.as_ref().map(|mark| (&got.source, mark)))
        .collect();
    // §FS-021-monitors.7.1: a sample is read when its answer was taken.
    let taken: BTreeMap<&Source, DateTime<Utc>> =
        got.iter().map(|got| (&got.source, got.read_at)).collect();
    let mut kept = Vec::new();
    let mut gone = Vec::new();
    let ids: Vec<String> = document.monitors.keys().cloned().collect();
    for id in ids {
        let snapshot = document.monitors[&id].clone();
        if held_elsewhere(&snapshot, &plan.claim, run_timeout(&monitors, &id), now) {
            // Read by another refresh: its state is what that one says.
            continue;
        }
        let monitor = monitors
            .get(&id)
            .cloned()
            .unwrap_or_else(|| declarations.resolve(&id, Some(&snapshot)));
        // The `from` whose answer reads it: its own, or the one that owns it.
        let from = match routes.get(&id) {
            Some(Route::From(source)) => Some(source),
            _ => match &snapshot.owner {
                Some(Owner::From(source)) => Some(source),
                _ => None,
            },
        };
        let cadence = known::cadence(&monitor, snapshot.owner.as_ref(), &froms);
        let (event, found, at) = match routed.deliveries.get(&id) {
            Some(delivery) => {
                let at = from
                    .and_then(|from| taken.get(from).copied())
                    .unwrap_or(now);
                let homes = declarations.homes(&id, Some(&snapshot));
                let mark = from.and_then(|from| marks.get(from).copied());
                let found = found::found(delivery, monitor, &homes, &snapshot, mark, &cadence, at);
                let out_of_cadence = plan.due.contains(&id)
                    && matches!(routes.get(&id), Some(Route::From(_)))
                    && !plan.in_cadence.contains(&id);
                let event = Event::Read(Read {
                    at,
                    in_cadence: !out_of_cadence,
                    found: found.finding.clone(),
                    after_work: after::unseen(request.project, &id, &snapshot, &plan.ended),
                });
                (event, found, at)
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
                (event, found, now)
            }
        };
        let watch = found::watch(&found.monitor, found.ok, found.of, snapshot.kind, cadence);
        let record = document
            .monitors
            .get_mut(&id)
            .expect("every id stepped is kept");
        let effects = machine::step(record, &watch, &event, at, &Local);
        kept.push(history(&id, effects));
        let timeout = run_timeout(&monitors, &id);
        if record
            .claim
            .as_ref()
            .is_some_and(|claim| is_mine(claim, &plan.claim) || claim.is_void(timeout, now))
        {
            record.claim = None;
        }
    }
    for id in &gone {
        if let Some(record) = document.monitors.remove(id) {
            if !record.past.is_empty() {
                document
                    .forgotten
                    .entry(id.clone())
                    .or_default()
                    .extend(record.past);
            }
        }
    }
    let recent = TimeDelta::days(i64::try_from(request.recent_days).unwrap_or(7));
    document.prune_forgotten(recent, now);
    if document.due_to_thin(now) {
        let known = known::all(declarations, document);
        thin(store, document, &known, now)?;
    }
    keep(store, appends, &kept, &gone)?;
    Ok(notes)
}

/// What a step keeps in the monitor's history.
fn history(id: &str, effects: Vec<Effect>) -> Kept {
    let mut kept = Kept {
        id: id.to_string(),
        ..Kept::default()
    };
    for effect in effects {
        match effect {
            Effect::Record(line) => kept.lines.push(line),
            Effect::RestartHistory => {
                kept.restart = true;
                kept.lines.clear();
            }
            Effect::Opened { .. } | Effect::Moved { .. } | Effect::Ended { .. } => {}
        }
    }
    kept
}

/// Writes what every step kept, and lets the forgotten histories go
/// (§FS-021-monitors.5.6). Where one write fails, or `monitors.json` cannot
/// be written after them, [`Store::appending`] takes back every line this
/// commit appended before the failure fails the slot
/// (§FS-021-monitors.17.3): the records and the histories still agree, and
/// the next read appends no line twice.
fn keep(store: &Store, appends: &mut Appends, kept: &[Kept], gone: &[String]) -> Result<()> {
    for kept in kept {
        if kept.restart {
            store.remove(&kept.id)?;
        }
        appends.append(&kept.id, &kept.lines)?;
    }
    gone.iter().try_for_each(|id| store.remove(id))
}

/// Thins every history and lets go of the episodes no history keeps any
/// more, at most once a day and under the lock (§FS-021-monitors.9.3), each
/// at the interval its freshness is judged at, so a monitor read from a
/// daily answer is never thinned (§FS-021-monitors.6.8).
fn thin(
    store: &Store,
    document: &mut Document,
    known: &BTreeMap<String, Known>,
    now: DateTime<Utc>,
) -> Result<()> {
    for (id, record) in document.monitors.iter_mut() {
        let Some(kind) = record.kind else {
            continue;
        };
        let known = known.get(id);
        let interval = known.map_or_else(
            || cadence::Cadence::default_every().interval(),
            |known| known.cadence.interval(),
        );
        let lines = store.history(id)?;
        let kept = retention::thin(
            &lines,
            interval,
            kind,
            known.and_then(|known| known.monitor.levels.as_deref()),
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

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "watch_runs_tests.rs"]
mod runs_tests;

#[cfg(test)]
#[path = "watch_place_tests.rs"]
mod place_tests;

#[cfg(test)]
#[path = "watch_matters_tests.rs"]
mod matters_tests;
