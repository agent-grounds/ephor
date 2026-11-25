use super::*;

use chrono::{DateTime, TimeZone, Utc};

use crate::monitors::model::Value;
use crate::monitors::state::Record;

fn at(minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 2, 9, minute, 0).unwrap()
}

fn reading(minute: u32, value: f64) -> Line {
    Line::Reading {
        t: at(minute),
        v: Value::Number(value),
        health: None,
        out_of_cadence: false,
    }
}

#[test]
fn a_project_with_no_store_reads_empty() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::at(root.path(), "alpha");
    assert_eq!(store.load().unwrap(), Document::default());
    assert!(store.history("uptime").unwrap().is_empty());
}

/// §FS-021-monitors.9.4: `monitors.json` is written whole and renamed into
/// place, leaving no temporary file behind, and reads back as written.
#[test]
fn an_update_rewrites_the_document_whole() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::at(root.path(), "alpha");
    store
        .update(|document| {
            document
                .monitors
                .insert("uptime".to_string(), Record::new(at(0)));
            Ok(())
        })
        .unwrap();
    let read = store.load().unwrap();
    assert!(read.monitors.contains_key("uptime"));
    let names: Vec<String> = std::fs::read_dir(root.path().join("alpha"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !names.iter().any(|name| name.ends_with(".tmp")),
        "{names:?}"
    );
    assert!(names.contains(&FILE.to_string()), "{names:?}");
}

/// A change that fails writes nothing (§FS-021-monitors.9.4).
#[test]
fn a_failed_change_leaves_the_store_as_it_was() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::at(root.path(), "alpha");
    let result: Result<()> = store.update(|document| {
        document.thinned = Some(at(0));
        Err(EphorError::Command("no".to_string()))
    });
    assert!(result.is_err());
    assert_eq!(store.load().unwrap().thinned, None);
}

/// §FS-021-monitors.17.3: a store that is there and does not read is the
/// machinery failing, said with the path.
#[test]
fn a_store_that_does_not_read_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::at(root.path(), "alpha");
    std::fs::create_dir_all(root.path().join("alpha")).unwrap();
    std::fs::write(store.path(), "{ not json").unwrap();
    let error = store.load().unwrap_err().to_string();
    assert!(error.contains("monitors.json"), "{error}");
    assert!(store.update(|_| Ok(())).is_err());
}

/// §FS-021-monitors.9.4: changes made under the lock from many threads at
/// once all land, none lost to another's rewrite.
#[test]
fn concurrent_updates_all_land_under_the_lock() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::at(root.path(), "alpha");
    std::thread::scope(|scope| {
        for n in 0..8 {
            let store = store.clone();
            scope.spawn(move || {
                store
                    .update(|document| {
                        document
                            .monitors
                            .insert(format!("m{n}"), Record::new(at(n)));
                        Ok(())
                    })
                    .unwrap();
            });
        }
    });
    assert_eq!(store.load().unwrap().monitors.len(), 8);
}

/// §FS-021-monitors.9.4: a history's half-written last line is passed over,
/// and the next append starts on a line of its own.
#[test]
fn a_torn_last_line_is_passed_over_and_cut_before_the_next_append() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::at(root.path(), "alpha");
    std::fs::create_dir_all(root.path().join("alpha")).unwrap();
    store.append("uptime", &[reading(0, 1.0)]).unwrap();
    let mut text = std::fs::read_to_string(store.history_path("uptime")).unwrap();
    text.push_str("{\"t\":\"2026-11-02T09:10:00Z\",\"v\":");
    std::fs::write(store.history_path("uptime"), &text).unwrap();
    assert_eq!(store.history("uptime").unwrap(), vec![reading(0, 1.0)]);

    store.append("uptime", &[reading(20, 2.0)]).unwrap();
    assert_eq!(
        store.history("uptime").unwrap(),
        vec![reading(0, 1.0), reading(20, 2.0)]
    );
    let text = std::fs::read_to_string(store.history_path("uptime")).unwrap();
    assert_eq!(text.lines().count(), 2, "{text}");
}

#[test]
fn a_history_is_rewritten_and_removed() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::at(root.path(), "alpha");
    std::fs::create_dir_all(root.path().join("alpha")).unwrap();
    store
        .append("uptime", &[reading(0, 1.0), reading(10, 2.0)])
        .unwrap();
    store.rewrite("uptime", &[reading(10, 2.0)]).unwrap();
    assert_eq!(store.history("uptime").unwrap(), vec![reading(10, 2.0)]);
    store.remove("uptime").unwrap();
    store.remove("uptime").unwrap();
    assert!(!store.history_path("uptime").exists());
}

/// §FS-021-monitors.20.2: the doctor reports the store's size and files.
#[test]
fn size_counts_every_file_under_the_store() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::at(root.path(), "alpha");
    store.update(|_| Ok(())).unwrap();
    store.append("uptime", &[reading(0, 1.0)]).unwrap();
    let size = super::size(root.path());
    // monitors.json, its lock, and one history.
    assert_eq!(size.files, 3);
    assert!(size.bytes > 0);
    assert_eq!(size.path, root.path());
    assert_eq!(super::size(&root.path().join("none")).files, 0);
}
