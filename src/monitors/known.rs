//! Every monitor a project has, as its declarations and its store know it
//! (§FS-021-monitors.1.4), with the cadence each is judged at
//! (§FS-021-monitors.6.8): what a refresh steps, what retention thins, and
//! what the doctor names overdue (§FS-021-monitors.20.2) all start here, so
//! the three cannot judge one monitor at two cadences.

use std::collections::BTreeMap;

use chrono::{DateTime, Local, Utc};
use serde_json::Value as Json;

use crate::branches::Placement;
use crate::monitors::cadence::Cadence;
use crate::monitors::declared::{self, Declarations};
use crate::monitors::freshness::{Cause, Freshness};
use crate::monitors::model::Source;
use crate::monitors::overlay::Monitor;
use crate::monitors::route::{self, Owner, Route};
use crate::monitors::state::Document;
use crate::monitors::store::Store;

/// One monitor the project has, resolved, and the cadence its freshness,
/// its hold and its thinning are judged at.
#[derive(Debug, Clone)]
pub struct Known {
    pub monitor: Monitor,
    pub cadence: Cadence,
}

/// Each `from` a read monitor names, with that monitor's effective cadence:
/// what the cadence of a monitor read from the answer that brings it is
/// taken from (§FS-021-monitors.6.8).
pub fn froms(monitors: &BTreeMap<String, Monitor>) -> Vec<(Source, Cadence)> {
    monitors
        .values()
        .filter_map(|monitor| match Route::of(monitor) {
            Route::From(source) => Some((source, monitor.cadence)),
            Route::Brought | Route::NotRead => None,
        })
        .collect()
}

/// The cadence a monitor is judged at (§FS-021-monitors.6.8): its own
/// effective cadence where it names a `from`, or where the site states one
/// for a monitor read from the answer that brings it; otherwise that
/// answer's: the fastest among the monitors its owner's `from` reads, and
/// the refresh's own, judged as `every: 10m`, for a custom-status answer or
/// before any answer owns it.
pub fn cadence(monitor: &Monitor, owner: Option<&Owner>, froms: &[(Source, Cadence)]) -> Cadence {
    if monitor.from.is_some() || monitor.cadence_stated {
        return monitor.cadence;
    }
    route::brought_cadence(
        owner,
        froms.iter().map(|(source, cadence)| (source, *cadence)),
    )
}

/// Every declared monitor, resolved over what the store keeps of it.
pub fn declared(declarations: &Declarations, document: &Document) -> BTreeMap<String, Monitor> {
    declarations
        .ids()
        .into_iter()
        .map(|id| {
            let monitor = declarations.resolve(&id, document.monitors.get(&id));
            (id, monitor)
        })
        .collect()
}

/// Every monitor the project has: each one declared, and each one the store
/// keeps that nothing declares, read from the description its last sample
/// brought (§FS-021-monitors.4.3).
pub fn all(declarations: &Declarations, document: &Document) -> BTreeMap<String, Known> {
    let monitors = declared(declarations, document);
    let froms = froms(&monitors);
    let mut known: BTreeMap<String, Known> = BTreeMap::new();
    for id in monitors.keys().chain(document.monitors.keys()) {
        if known.contains_key(id) {
            continue;
        }
        let record = document.monitors.get(id);
        let monitor = monitors
            .get(id)
            .cloned()
            .unwrap_or_else(|| declarations.resolve(id, record));
        let cadence = cadence(
            &monitor,
            record.and_then(|record| record.owner.as_ref()),
            &froms,
        );
        known.insert(id.clone(), Known { monitor, cadence });
    }
    known
}

/// The monitors of a project that were overdue at `now`
/// (§FS-021-monitors.20.2): what the doctor says before its own refresh
/// reads them. A monitor counts once it is unknown for that cause
/// (§FS-021-monitors.7.5), never while what it read still holds. While the
/// monitor slot fails, `slot_error` is every monitor's cause instead, and
/// none of them is overdue (§FS-021-monitors.7.2). A monitor ephor does not
/// read is never overdue (§FS-021-monitors.4.5, §FS-021-monitors.4.6).
pub fn overdue(
    store: &Store,
    placement: Option<&Placement>,
    site: &[Json],
    slot_error: Option<&str>,
    now: DateTime<Utc>,
) -> Vec<String> {
    let declarations = declared::read(placement, site);
    let Ok(document) = store.load() else {
        return Vec::new();
    };
    all(&declarations, &document)
        .into_iter()
        .filter(|(id, known)| {
            let Some(record) = document.monitors.get(id) else {
                return false;
            };
            known.monitor.is_readable()
                && matches!(
                    record.freshness(&known.cadence, now, &Local, slot_error),
                    Freshness::Unknown {
                        cause: Cause::Overdue,
                        ..
                    }
                )
        })
        .map(|(id, _)| id)
        .collect()
}

#[cfg(test)]
#[path = "known_tests.rs"]
mod tests;
