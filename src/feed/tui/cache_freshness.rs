//! Local cache evidence about the displayed snapshot (§FS-001-forge-interface.7.1).

use std::collections::BTreeMap;
use std::time::SystemTime;

use chrono::{DateTime, Utc};

use crate::feed::cache::{self, ProjectFeed};

/// Only metadata is checked until a known project's file changes
/// (§FS-001-forge-interface.7.1). Length also distinguishes replacements on
/// filesystems with coarse modification times.
#[derive(PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl Stamp {
    /// One bounded local metadata read (§FS-001-forge-interface.7.1).
    fn of(project: &str) -> Option<Self> {
        let metadata = std::fs::metadata(cache::feed_path(project)).ok()?;
        Some(Self {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        })
    }
}

/// Observation never loads rows into the session. Newer valid fetch evidence
/// survives missing/broken replacements and unrelated arrivals; the actual
/// displayed feed decides whether it has caught up (§FS-001-forge-interface.7.1).
#[derive(Default)]
pub(super) struct CacheFreshness {
    observed: BTreeMap<String, Option<Stamp>>,
    newest: BTreeMap<String, DateTime<Utc>>,
}

impl CacheFreshness {
    /// Capture metadata BEFORE the read supplying displayed rows. Capturing it
    /// afterwards could hide a replacement made during that read. Both initial
    /// loading and later absorption/reloads use this baseline maintenance
    /// (§FS-001-forge-interface.7.1).
    pub(super) fn before_read<'a>(&mut self, projects: impl IntoIterator<Item = &'a str>) {
        for project in projects {
            self.observed.insert(project.to_owned(), Stamp::of(project));
        }
    }

    /// Inspect changed files only, without changing rows, provider health or
    /// reader state. Stat before parsing so a replacement during inspection
    /// remains detectable next tick (§FS-001-forge-interface.7.1).
    pub(super) fn observe(&mut self, feeds: &[ProjectFeed]) -> bool {
        let before = self.outdated(feeds);
        for (project, observed) in &mut self.observed {
            let stamp = Stamp::of(project);
            if stamp == *observed {
                continue;
            }
            *observed = stamp;
            let Ok(Some(feed)) = cache::load_feed(project) else {
                continue;
            };
            if feed.model != cache::MODEL || feed.project != *project {
                continue;
            }
            if let Some(fetched) = feed.fetched_at {
                self.newest
                    .entry(project.clone())
                    .and_modify(|newest| *newest = (*newest).max(fetched))
                    .or_insert(fetched);
            }
        }
        before != self.outdated(feeds)
    }

    /// Compare with the snapshot actually displayed, so only catching up that
    /// project retires its warning (§FS-001-forge-interface.7.1).
    fn outdated(&self, feeds: &[ProjectFeed]) -> Vec<String> {
        self.newest
            .iter()
            .filter(|(project, fetched)| {
                feeds
                    .iter()
                    .any(|feed| feed.project == **project && feed.fetched_at < Some(**fetched))
            })
            .map(|(project, _)| project.clone())
            .collect()
    }

    /// A persistent, project-attributed screen warning, independent of transient
    /// messages, refresh progress and provider staleness (§FS-001-forge-interface.7.1).
    pub(super) fn warning(&self, feeds: &[ProjectFeed]) -> Option<String> {
        let projects = self.outdated(feeds);
        (!projects.is_empty()).then(|| {
            format!(
                "Displayed feed outdated: {} — newer cache available; press r to refresh",
                projects.join(", ")
            )
        })
    }
}
