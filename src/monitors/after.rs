//! Work laid on a monitor's matter that has ended (§FS-021-monitors.15): what
//! makes the first plain refresh after it read the monitor out of cadence, so
//! a fix is confirmed when it lands, and what the monitor's matter resurfaces
//! on when the fix did not hold.
//!
//! The ledger says what was laid on which matter, and the runtime whether it
//! is over; ended means a matter whose work the ledger records has no line
//! left open, as `Session::working` reads it (§FS-005-dispatch.23).

use std::collections::BTreeMap;

use crate::monitors::episode::{self, Sort};
use crate::monitors::machine::EndedWork;
use crate::monitors::state::Record;
use crate::work::Tone;

/// Each matter of `project`'s monitors whose work has ended, by its key,
/// with a mark for that work: its last ticket and when it was laid. Empty
/// where the ledger records no work on them, or the site cannot be read.
pub fn ended(project: &str) -> BTreeMap<String, String> {
    let prefix = format!("{}:{project}/", crate::monitors::matters::SOURCE);
    let Ok(ledger) = crate::work::ledger::load() else {
        return BTreeMap::new();
    };
    let laid: Vec<_> = ledger
        .entries
        .iter()
        .filter(|(key, _)| key.starts_with(&prefix))
        .collect();
    if laid.is_empty() {
        return BTreeMap::new();
    }
    let Ok(config) = crate::feed::config::load_config() else {
        return BTreeMap::new();
    };
    laid.into_iter()
        .filter_map(|(key, entry)| {
            let status = crate::work::status_of_entry(&config.work, entry, None);
            let open = status
                .lines(60)
                .iter()
                .any(|line| matches!(line.tone, Tone::Running | Tone::Going | Tone::Waiting));
            let last = entry.last()?;
            (!open).then(|| {
                let at = episode::id(last.at);
                (key.clone(), format!("{}@{at}", last.ticket))
            })
        })
        .collect()
}

/// The ended work on a monitor's open episodes that its record has not yet
/// seen: once per work, while the episode it was laid on is open
/// (§FS-021-monitors.15).
pub fn unseen(
    project: &str,
    id: &str,
    record: &Record,
    ended: &BTreeMap<String, String>,
) -> Vec<EndedWork> {
    let drop = record
        .drop
        .as_ref()
        .map(|drop| (Sort::Drop, drop.named, &drop.works_ended));
    let unknown = record
        .unknown
        .as_ref()
        .map(|unknown| (Sort::Unknown, unknown.named, &unknown.works_ended));
    drop.into_iter()
        .chain(unknown)
        .filter_map(|(on, named, seen)| {
            let work = ended.get(&episode::key(project, id, on, named))?;
            (!seen.contains(work)).then(|| EndedWork {
                on,
                work: work.clone(),
            })
        })
        .collect()
}
