//! How Waiting's overview line counts the watched projects' statuses
//! (§FS-021-monitors.20.1): down, unknown, and overdue a second time, by the
//! rank the Monitors page's header counts them under
//! (§FS-021-monitors.18.2).

use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use serde_json::Value as Json;

use crate::branches::Placement;
use crate::monitors::declared;
use crate::monitors::freshness::{Cause, Freshness};
use crate::monitors::known;
use crate::monitors::model::Health;
use crate::monitors::store::Store;

/// The statuses' counts, `{down, unknown, overdue}` (§FS-019-waiting.6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub down: usize,
    pub unknown: usize,
    pub overdue: usize,
}

impl Counts {
    /// `monitors: 1 down · 2 unknown · 1 overdue`, overdue only when it is
    /// not zero (§FS-021-monitors.20.1).
    pub fn say(&self) -> String {
        let mut said = format!("monitors: {} down · {} unknown", self.down, self.unknown);
        if self.overdue > 0 {
            said.push_str(&format!(" · {} overdue", self.overdue));
        }
        said
    }

    pub fn add(&mut self, other: Counts) {
        self.down += other.down;
        self.unknown += other.unknown;
        self.overdue += other.overdue;
    }
}

/// One project's statuses at `now`, None where it has none. Each counts once,
/// by the rank its row shows (§FS-021-monitors.18.2, §FS-021-monitors.18.3):
/// down while its drop is open, whatever its freshness; unknown when its
/// value no longer holds, or its declaration is in error
/// (§FS-021-monitors.4.6, §FS-021-monitors.21.2); and overdue a second time
/// when that is why it is unknown (§FS-021-monitors.7.5). A monitor not run,
/// and one with no reading yet inside its first grace, counts in none of
/// them, and neither does a KPI. While the slot fails, `slot_error` is every
/// monitor's cause (§FS-021-monitors.7.2).
pub fn of(
    store: &Store,
    placement: Option<&Placement>,
    site: &[Json],
    slot_error: Option<&str>,
    now: DateTime<Utc>,
) -> Option<Counts> {
    let declarations = declared::read(placement, site);
    let document = store.load().ok()?;
    let mut counts = Counts::default();
    let mut statuses = 0;
    for (id, known) in known::all(&declarations, &document) {
        let record = document.monitors.get(&id);
        let ok = record
            .and_then(|record| record.last_sample.as_ref())
            .and_then(|sample| sample.ok);
        let faulted = !known.monitor.faults.is_empty();
        if known.monitor.judge(ok).is_kpi() || (known.monitor.not_run && !faulted) {
            continue;
        }
        statuses += 1;
        if faulted {
            counts.unknown += 1;
            continue;
        }
        let Some(record) = record else {
            continue;
        };
        let down = record.drop.is_some() || record.hold.health == Some(Health::Down);
        match record.freshness(&known.cadence, now, &Local, slot_error) {
            Freshness::Unknown { cause, .. } => {
                match down {
                    true => counts.down += 1,
                    false => counts.unknown += 1,
                }
                if cause == Cause::Overdue {
                    counts.overdue += 1;
                }
            }
            _ if down => counts.down += 1,
            _ => {}
        }
    }
    (statuses > 0).then_some(counts)
}

#[cfg(test)]
#[path = "counts_tests.rs"]
mod tests;
