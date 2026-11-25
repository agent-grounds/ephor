//! Where a plain refresh runs a project's monitors, and what it hands them
//! (§FS-021-monitors.5.3, §FS-021-monitors.5.4).

use serde_json::json;

use super::*;
use crate::monitors::freshness::Found;
use crate::monitors::test_site::*;

/// A script that logs where it ran and what it was told, then answers one
/// sample for each monitor it was asked for.
fn probe(dir: &Path, log: &Path) {
    std::fs::write(
        dir.join("probe.sh"),
        format!(
            "echo \"$EPHOR_MONITORS $(pwd -P) [$EPHOR_ITEM_ID] [$EPHOR_TITLE]\" >> '{}'\n\
             printf '{{\"v\":1,\"monitors\":[{{\"id\":\"%s\",\"value\":1}}]}}' \"$EPHOR_MONITORS\" > \"$EPHOR_ANSWER\"\n",
            log.display()
        ),
    )
    .unwrap();
}

use std::path::Path;

/// §FS-021-monitors.5.3, §FS-021-monitors.5.4: a project whose registry
/// keeps a checkout per branch reads its manifest from, and runs its
/// monitors in, the default branch's checkout, never the registry root
/// above it; `repo:<name>` and `root` resolve from that checkout; and every
/// variable about an item is empty.
#[test]
fn monitors_run_in_the_default_branchs_checkout() {
    let site = site_with(
        json!({"branch_root_template": "{project_root}/{branch}", "main_branch": "main"}),
        Some("main"),
    );
    let log = site.dir.path().join("probe.log");
    std::fs::create_dir_all(site.root.join("sub")).unwrap();
    probe(&site.root, &log);
    probe(&site.root.join("sub"), &log);
    std::fs::write(
        site.root.join("ephor.json"),
        serde_json::to_string(&json!({"monitors": [
            {"id": "here", "healthy": {"min": 1}, "from": {"command": "sh probe.sh"}},
            {"id": "top", "healthy": {"min": 1},
             "from": {"command": "sh probe.sh", "cwd": "root"}},
            {"id": "sub", "healthy": {"min": 1},
             "from": {"command": "sh probe.sh", "cwd": "repo:sub"}},
        ]}))
        .unwrap(),
    )
    .unwrap();
    let report = refresh_at(&site, &[], When::Due, 0);
    assert_eq!(report.slot, Some(Ok(())), "{:?}", report.notes);
    let checkout = site.root.canonicalize().unwrap();
    let mut said: Vec<String> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    said.sort();
    assert_eq!(
        said,
        vec![
            format!("here {} [] []", checkout.display()),
            format!("sub {} [] []", checkout.join("sub").display()),
            format!("top {} [] []", checkout.display()),
        ]
    );
    for id in ["here", "top", "sub"] {
        assert_eq!(found(&site, id), Some(Found::Reading), "{id}");
    }
}

/// §FS-021-monitors.5.3, §FS-021-monitors.4.6: a `from` whose path leaves
/// the checkout breaks a rule: it is noted, and nothing outside is read.
#[test]
fn a_file_outside_the_checkout_is_not_read() {
    let site = site(None);
    std::fs::write(
        site.dir.path().join("secret.json"),
        r#"{"v":1,"monitors":[{"id":"f","value":4}]}"#,
    )
    .unwrap();
    let declared = [json!({"id": "f", "healthy": {"min": 1},
                           "from": {"file": "../secret.json"}})];
    let report = refresh_at(&site, &declared, When::Due, 0);
    assert_eq!(report.slot, Some(Ok(())));
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("monitor f is not read") && note.contains("../secret.json")),
        "{:?}",
        report.notes
    );
    let document = site.store.load().unwrap();
    assert!(document
        .monitors
        .get("f")
        .is_none_or(|record| record.last_attempt.is_none()));
}

/// §AR-002-summons.3, §FS-021-monitors.17.2: a byte that is not UTF-8 in
/// what a monitor's command printed costs it nothing.
#[test]
fn output_that_is_not_utf8_still_reads() {
    let site = site(None);
    std::fs::write(
        site.root.join("kpis.sh"),
        format!("printf '\\377\\n' >&2\nprintf '%s' '{BOTH}' > \"$EPHOR_ANSWER\"\n"),
    )
    .unwrap();
    refresh_at(&site, &[from_kpis("a")], When::Due, 0);
    assert_eq!(found(&site, "a"), Some(Found::Reading));
}
