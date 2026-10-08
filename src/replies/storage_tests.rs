//! Binding, state transitions and real persistence faults (§FS-005-dispatch.13).

use super::*;
use crate::feed::{config::Defaults, model::ItemKind, providers::Sources};
use crate::replies::binding::fingerprints;
use serde_json::json;

pub(crate) fn item() -> Item {
    Item {
        id: "mail:k-B".into(),
        project: "demo".into(),
        source: "mail".into(),
        kind: ItemKind::Message,
        role: None,
        title: "Fence".into(),
        url: None,
        state: None,
        needs_response: true,
        updated_at: chrono::Utc::now(),
        raw: json!({"threads": [{"messages": [{"author":"dana", "when":"2026-10-07T09:00:00Z", "text":"€100?", "mine":false}], "reply":{"in_reply_to":"H"}}]}),
    }
}

pub(crate) fn sources() -> Sources {
    Sources {
        project: "demo".into(),
        own: vec![json!({"provider":"mail","user":"me"})],
        site: vec![],
    }
}

pub(crate) fn pending(item: &Item, path: PathBuf, reconciliation: bool) -> Record {
    let mut record = Record::new(item);
    let binding = Binding::capture(
        item,
        &sources(),
        &Defaults::default(),
        &record,
        path.clone(),
    )
    .unwrap();
    record.intent = Some(Intent {
        request: Some(Request {
            config: binding.config.clone(),
            project: binding.context.clone(),
            user: binding.user.clone(),
            tickets: vec![],
            timeout_seconds: 1,
        }),
        binding,
        text: "Thanks".into(),
        reconciliation,
        draft: Some(path),
        status: Status::Pending,
        note: None,
    });
    record
}

#[test]
fn fingerprints_compare_author_time_words_and_ownership_only() {
    let row = item();
    let thread = &row.raw["threads"][0];
    let baseline = fingerprints(thread);
    for (field, value) in [
        ("author", json!("other")),
        ("when", json!("later")),
        ("text", json!("€150?")),
        ("mine", json!(true)),
    ] {
        let mut changed = thread.clone();
        changed["messages"][0][field] = value;
        assert_ne!(fingerprints(&changed), baseline, "{field}");
    }
    let mut changed = thread.clone();
    changed["messages"][0]["reactions"] = json!([{"emoji":"👍","users":["me"]}]);
    changed["messages"][0]["task"] = json!({"resolved":true});
    // A file is no part of what a draft was bound to, and neither is whether
    // the discussion waits (§FS-005-dispatch.13, §FS-001-forge-interface.1).
    changed["messages"][0]["attachments"] = json!([{"name":"IMG_2041.jpg","id":"att:1"}]);
    changed["awaits_reader"] = json!(true);
    assert_eq!(fingerprints(&changed), baseline);
    let mut changed_row = row.clone();
    changed_row.state = Some("done".into());
    changed_row.raw["threads"][0] = changed;
    let binding = pending(&row, "draft".into(), true).intent.unwrap().binding;
    assert!(binding.freshness(&changed_row, &Record::new(&row)).is_ok());
}

#[test]
fn identity_disappearance_reordering_ambiguity_and_advancing_label_refuse() {
    let row = item();
    let record = pending(&row, "draft".into(), true);
    let binding = &record.intent.as_ref().unwrap().binding;
    for threads in [
        json!([]),
        json!([{"messages":[]},row.raw["threads"][0]]),
        json!([row.raw["threads"][0], row.raw["threads"][0]]),
    ] {
        let mut changed = row.clone();
        changed.raw["threads"] = threads;
        assert!(binding.freshness(&changed, &record).is_err());
    }
    let mut advanced = row.clone();
    advanced.raw["threads"][0]["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"author":"dana","when":"later","text":"€150","mine":false}));
    let reason = binding.freshness(&advanced, &record).unwrap_err();
    for fact in ["dana", "later", "€150"] {
        assert!(reason.contains(fact), "{reason}");
    }
    let mut unrelated = row.clone();
    unrelated.raw["threads"]
        .as_array_mut()
        .unwrap()
        .push(json!({"messages":[{"author":"other","text":"elsewhere"}]}));
    assert!(binding.freshness(&unrelated, &record).is_ok());
}

#[test]
fn original_request_context_and_account_must_match() {
    let row = item();
    let record = pending(&row, "draft".into(), true);
    let intent = record.intent.unwrap();
    assert!(intent
        .binding
        .matches_request(intent.request.as_ref().unwrap()));
    let changed_defaults = Defaults {
        github_user: Some("another-account".into()),
        ..Defaults::default()
    };
    assert!(intent
        .binding
        .routing(&sources(), &changed_defaults)
        .is_err());
    for field in ["config", "project", "user"] {
        let mut request = intent.request.clone().unwrap();
        match field {
            "config" => request.config["user"] = json!("other"),
            "project" => request.project.clear(),
            _ => request.user = Some("other".into()),
        }
        assert!(!intent.binding.matches_request(&request));
    }
    for changed in [
        Sources::default(),
        Sources {
            own: vec![json!({"provider":"mail","user":"other"})],
            ..sources()
        },
        Sources {
            project: "elsewhere".into(),
            ..sources()
        },
    ] {
        assert!(intent
            .binding
            .routing(&changed, &Defaults::default())
            .is_err());
    }
}

#[test]
fn transitions_retain_confirmed_identities_and_thread_specific_generations() {
    let row = item();
    let mut record = pending(&row, "first".into(), true);
    let first = record.intent.as_ref().unwrap().binding.clone();
    assert!(!record.intent.as_ref().unwrap().held());
    assert!(record.release().is_err());
    record.intent.as_mut().unwrap().status = Status::Held;
    assert!(record.intent.as_ref().unwrap().held());
    record.release().unwrap();
    assert_eq!(record.intent.as_ref().unwrap().status, Status::NotSent);
    assert_eq!(record.generation(&first), 0);
    assert!(record.confirmed.is_empty());
    assert!(record.confirm().is_err());
    record.intent.as_mut().unwrap().status = Status::Pending;
    record.confirm().unwrap();
    assert_eq!(record.generation(&first), 1);
    assert!(record.confirmed.contains(Path::new("first")));
    assert!(record.confirm().is_err());
    assert!(first
        .freshness(&row, &record)
        .unwrap_err()
        .contains("Thanks"));
    let mut later = pending(&row, "second".into(), false).intent.unwrap();
    later.binding.thread = 1;
    assert!(later.held(), "legacy pending is held after interruption");
    let second = later.binding.clone();
    record.intent = Some(later);
    record.confirm().unwrap();
    assert_eq!(record.generation(&first), 1);
    assert_eq!(record.generation(&second), 1);
    assert_eq!(record.confirmed.len(), 2);
    let roundtrip: Record = serde_json::from_slice(&serde_json::to_vec(&record).unwrap()).unwrap();
    assert_eq!(roundtrip.confirmed, record.confirmed);
    assert_eq!(roundtrip.generation(&first), 1);
}

#[test]
fn writer_orders_real_phases_and_faults_preserve_recoverable_state() {
    for fail in [
        None,
        Some(SavePhase::Write),
        Some(SavePhase::FileSync),
        Some(SavePhase::Rename),
        Some(SavePhase::DirectorySync),
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let row = item();
        let store = Store::open(tmp.path(), &row.id, true).unwrap();
        let record = pending(&row, "draft".into(), true);
        let mut phases = vec![];
        let result = store.save_with(&record, |phase| {
            phases.push(phase);
            // Observe the real files at the writer's boundaries, rather than
            // merely restating the list of phase names.
            if matches!(phase, SavePhase::FileSync | SavePhase::Rename) {
                let temporary = fs::read_dir(tmp.path())
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .find(|path| path.extension().is_some_and(|ext| ext == "tmp"))
                    .unwrap();
                let written: serde_json::Value =
                    serde_json::from_slice(&fs::read(temporary).unwrap()).unwrap();
                assert_eq!(written, serde_json::to_value(&record).unwrap());
                assert!(
                    !store.path.exists(),
                    "replacement precedes neither writing nor file sync"
                );
            }
            if phase == SavePhase::DirectorySync {
                assert_eq!(
                    store.read().unwrap().unwrap().intent.unwrap().text,
                    "Thanks"
                );
            }
            if Some(phase) == fail {
                Err(io::Error::other("injected"))
            } else {
                Ok(())
            }
        });
        let order = [
            SavePhase::Write,
            SavePhase::FileSync,
            SavePhase::Rename,
            SavePhase::DirectorySync,
        ];
        let end = fail
            .map(|phase| order.iter().position(|p| *p == phase).unwrap() + 1)
            .unwrap_or(4);
        assert_eq!(phases, order[..end]);
        assert_eq!(result.is_err(), fail.is_some());
        assert!(fs::read_dir(tmp.path()).unwrap().all(|entry| entry
            .unwrap()
            .path()
            .extension()
            .is_none_or(|ext| ext != "tmp")));
        let saved = store.read().unwrap();
        assert_eq!(
            saved.is_some(),
            fail.is_none() || fail == Some(SavePhase::DirectorySync)
        );
        if let Some(saved) = saved {
            assert_eq!(saved.intent.unwrap().text, "Thanks");
        }
    }
}

#[test]
fn invalid_unreadable_records_refuse_and_corruption_is_isolated_in_collection() {
    let tmp = tempfile::tempdir().unwrap();
    let row = item();
    let store = Store::open(tmp.path(), &row.id, true).unwrap();
    store.save(&pending(&row, "draft".into(), false)).unwrap();
    fs::write(tmp.path().join("unrelated.json"), "{broken").unwrap();
    let recovery = Store::recovery_in(tmp.path());
    assert_eq!(
        recovery
            .rows
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        [row.id.as_str()]
    );
    assert_eq!(recovery.diagnostics.len(), 1);
    assert!(recovery.diagnostics[0].contains("unrelated.json"));
    assert!(recovery.diagnostics[0].contains("Invalid saved reply"));
    for change in ["version", "identity", "payload", "request", "baseline"] {
        let mut record = pending(&row, "draft".into(), true);
        match change {
            "version" => record.version = 2,
            "identity" => record.intent.as_mut().unwrap().binding.row = "other".into(),
            "payload" => record.intent.as_mut().unwrap().text = " ".into(),
            "request" => record
                .intent
                .as_mut()
                .unwrap()
                .request
                .as_mut()
                .unwrap()
                .project
                .clear(),
            _ => record.intent.as_mut().unwrap().binding.messages.clear(),
        }
        store.save(&record).unwrap();
        assert!(store.read().is_err(), "{change}");
    }
    fs::remove_file(&store.path).unwrap();
    fs::create_dir(&store.path).unwrap();
    assert!(
        store.read().is_err(),
        "a directory is unreadable record content even as root"
    );
}

#[test]
fn readonly_open_and_read_do_not_create_or_write_even_transiently() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("absent");
    take_operations();
    let dry = Store::open(&dir, "mail:k-B", false).unwrap();
    assert!(dry.read().unwrap().is_none());
    assert!(take_operations().is_empty());
    assert!(!dir.exists());
}
