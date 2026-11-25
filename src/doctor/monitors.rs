//! What the doctor says of the monitors (§FS-021-monitors.20.2), as notes
//! that move neither a project's health nor the exit code
//! (§FS-010-doctor.5): the monitors overdue when it began, and the store's
//! size, its files and where it is.

use chrono::Utc;
use serde_json::Value;

use crate::feed::cache;
use crate::feed::config::StatusConfig;
use crate::monitors::known;
use crate::monitors::store::{self, Store};

/// Each selected project's monitors that were overdue now, read before the
/// doctor's own refresh reads them: after it they would look well, and a
/// watch that works only while the doctor runs is the failure worth naming.
/// Where the last refresh's monitor slot failed, that failure is every
/// monitor's cause, and none of them is named overdue (§FS-021-monitors.7.2).
pub fn overdue(registry_doc: &Value, config: &StatusConfig, projects: &[String]) -> Vec<String> {
    let now = Utc::now();
    let mut said = Vec::new();
    for project in projects {
        let Some(project_config) = config.projects.get(project) else {
            continue;
        };
        let placement = crate::branches::Placement::load(registry_doc, project);
        let feed = cache::load_feed(project).ok().flatten();
        for id in known::overdue(
            &Store::of(project),
            placement.as_ref(),
            &project_config.monitors,
            feed.as_ref()
                .and_then(cache::ProjectFeed::monitor_slot_error),
            now,
        ) {
            said.push(format!(
                "monitor {project}/{id} was overdue when the doctor began"
            ));
        }
    }
    said
}

/// The store's size, its files and where it is, once there is a store.
pub fn store_note() -> Option<String> {
    let root = store::dir();
    if !root.is_dir() {
        return None;
    }
    let size = store::size(&root);
    Some(format!(
        "the monitor store holds {} byte(s) in {} file(s) at {}",
        size.bytes,
        size.files,
        size.path.display()
    ))
}
