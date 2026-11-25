//! A project on disk for the tests of the monitor slot: its checkout, a
//! store of its own under the test's directory, never the person's
//! (§REQ-003-test-isolation), and the helpers each test reads it with.

use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use serde_json::{json, Value as Json};

use crate::branches::Placement;
use crate::monitors::freshness::Found;
use crate::monitors::route::Outcome;
use crate::monitors::store::Store;
use crate::monitors::watch::{refresh, Report, Request, When};

pub struct Site {
    pub dir: tempfile::TempDir,
    /// Where the project's monitors run: its checkout.
    pub root: PathBuf,
    pub store: Store,
    pub placement: Placement,
}

/// A project `alpha` with its checkout on disk and a store of its own.
pub fn site(trust: Option<&str>) -> Site {
    let mut row = json!({});
    if let Some(trust) = trust {
        row["manifest_trust"] = json!(trust);
    }
    site_with(row, None)
}

/// A project `alpha` whose registry row carries `row`'s fields, with its
/// checkout at `checkout` under the registry root where one is named, and
/// at that root otherwise.
pub fn site_with(row: Json, checkout: Option<&str>) -> Site {
    let dir = tempfile::tempdir().unwrap();
    let registry_root = dir.path().join("alpha");
    let root = checkout.map_or_else(|| registry_root.clone(), |at| registry_root.join(at));
    std::fs::create_dir_all(&root).unwrap();
    let mut row = row;
    row["id"] = json!("alpha");
    row["root"] = json!(registry_root);
    let placement = Placement::load(&json!({ "projects": [row] }), "alpha").unwrap();
    let store = Store::at(&dir.path().join("state"), "alpha");
    Site {
        dir,
        root,
        store,
        placement,
    }
}

/// A script that says which monitors it was asked for, then answers.
pub fn script(root: &Path, answer: &str, exit: i32) {
    std::fs::write(
        root.join("kpis.sh"),
        format!(
            "echo \"$EPHOR_MONITORS\" >> runs.log\nprintf '%s' '{answer}' > \"$EPHOR_ANSWER\"\nexit {exit}\n"
        ),
    )
    .unwrap();
}

pub fn runs(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join("runs.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

pub fn from_kpis(id: &str) -> Json {
    json!({"id": id, "healthy": {"min": 1}, "from": {"command": "sh kpis.sh"}})
}

pub fn at(minutes: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 2, 9, 0, 0).unwrap() + TimeDelta::minutes(minutes)
}

pub fn refresh_at(site: &Site, declared: &[Json], when: When, minutes: i64) -> Report {
    refresh_then(site, declared, when, at(minutes))
}

pub fn refresh_then(site: &Site, declared: &[Json], when: When, now: DateTime<Utc>) -> Report {
    refresh_bringing(site, declared, Vec::new(), when, now)
}

/// A plain refresh whose custom-status answers carried `brought`.
pub fn refresh_bringing(
    site: &Site,
    declared: &[Json],
    brought: Vec<Outcome>,
    when: When,
    now: DateTime<Utc>,
) -> Report {
    let request = Request {
        project: "alpha",
        placement: Some(&site.placement),
        site: declared,
        brought,
        when,
        recent_days: 7,
    };
    refresh(&site.store, &request, now)
}

pub fn found(site: &Site, id: &str) -> Option<Found> {
    let document = site.store.load().unwrap();
    document.monitors[id]
        .last_attempt
        .as_ref()
        .map(|attempt| attempt.found.clone())
}

pub fn unreadable(site: &Site, id: &str) -> String {
    match found(site, id) {
        Some(Found::Unreadable { why }) => why,
        other => panic!("{id} is not unreadable: {other:?}"),
    }
}

pub const BOTH: &str = r#"{"v":1,"monitors":[{"id":"a","value":81},{"id":"b","value":3}]}"#;
