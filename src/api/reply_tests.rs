//! The real move's persistence/crash windows, exclusion and rehearsal
//! (§FS-005-dispatch.13, §FS-011-command-line.4).

use super::*;
use crate::replies::storage::tests::{item, pending};
use crate::replies::storage::{self, ReplyStorage, SavePhase};
use serde_json::{json, Value};
use std::{cell::Cell, fs, io, path::PathBuf};

/// An isolated executable carrier lets the actual move count calls without
/// process-global environment changes (§FS-001-forge-interface.2).
pub(crate) struct World {
    pub tmp: tempfile::TempDir,
    pub session: Session,
    pub item: Item,
    _directory: storage::TestDirectory,
    _ledger: crate::work::ledger::TestLedgerPath,
}

impl World {
    pub fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let _directory = storage::use_test_directory(tmp.path().join("replies"));
        let _ledger = crate::work::ledger::use_test_path(tmp.path().join("work.json"));
        let command = tmp.path().join("carrier");
        fs::write(
            &command,
            format!(
                r#"#!/usr/bin/env python3
import json, pathlib, sys
root = pathlib.Path({root:?})
request = json.loads(sys.stdin.read())
if sys.argv[1] == 'capabilities':
    if (root/'fail-cap').exists():
        print('cannot read declaration', file=sys.stderr)
        sys.exit(1)
    if (root/'caps.json').exists():
        print((root/'caps.json').read_text())
        sys.exit(0)
    print('{{"replies":true,"reply_reconciliation":true}}')
else:
    with (root / 'calls').open('a') as stream:
        stream.write(json.dumps(request,sort_keys=True)+'\n')
    records = [json.loads(p.read_text()) for p in (root/'replies').glob('*.json')]
    (root/'before').write_text(json.dumps(records))
    print('{{"status":"accepted"}}')
"#,
                root = tmp.path().to_str().unwrap()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&command, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut session = Session::default();
        session.provider_blocks.insert(
            "demo".into(),
            vec![json!({"provider":"mail","command":command,"user":"me"})],
        );
        Self {
            tmp,
            session,
            item: item(),
            _directory,
            _ledger,
        }
    }
    pub fn calls(&self) -> Vec<Value> {
        fs::read_to_string(self.tmp.path().join("calls"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    pub fn record(&self, reconciliation: bool) -> Record {
        let mut record = pending(
            &self.item,
            self.tmp.path().join("draft.reply.md"),
            reconciliation,
        );
        let intent = record.intent.as_mut().unwrap();
        intent.binding.config = self.session.provider_blocks["demo"][0].clone();
        intent.request.as_mut().unwrap().config = intent.binding.config.clone();
        record
    }
    pub fn draft(&self, text: &str) -> PathBuf {
        let record = self.record(true);
        let intent = record.intent.unwrap();
        fs::write(&intent.binding.path, text).unwrap();
        let ledger = json!({"entries": {self.item.id.clone(): {
            "project":"demo","title":"Fence","root":self.tmp.path(),"checkout":self.tmp.path(),
            "plan_id":"fence","plan":self.tmp.path().join("fence.rhei.md"),
            "dispatches":[{"ticket":"answer-1","recipe":"answer","at":chrono::Utc::now(),
                "snapshot":crate::work::ledger::Snapshot::of(&self.item),"reply_path":intent.binding.path,"reply_binding":intent.binding}]
        }}});
        fs::write(
            self.tmp.path().join("work.json"),
            serde_json::to_vec(&ledger).unwrap(),
        )
        .unwrap();
        record_path(&ledger)
    }
}

fn record_path(ledger: &Value) -> PathBuf {
    PathBuf::from(
        ledger["entries"]["mail:k-B"]["dispatches"][0]["reply_path"]
            .as_str()
            .unwrap(),
    )
}

/// Wrap the real atomic writer at a selected save number and phase.
struct Fault<'a> {
    store: &'a Store,
    save: Cell<usize>,
    at: usize,
    phase: SavePhase,
    crash: bool,
}
impl ReplyStorage for Fault<'_> {
    fn read(&self) -> crate::error::Result<Option<Record>> {
        self.store.read()
    }
    fn save(&self, record: &Record) -> crate::error::Result<()> {
        self.save.set(self.save.get() + 1);
        self.store.save_with(record, |phase| {
            if self.save.get() == self.at && phase == self.phase {
                assert!(!self.crash, "simulated process interruption");
                Err(io::Error::other("injected writer failure"))
            } else {
                Ok(())
            }
        })
    }
}

#[test]
fn each_initial_save_fault_prevents_actual_invocation_and_keeps_draft() {
    for phase in [
        SavePhase::Write,
        SavePhase::FileSync,
        SavePhase::Rename,
        SavePhase::DirectorySync,
    ] {
        for drafted in [false, true] {
            let world = World::new();
            let path = world.draft("Thanks");
            let store = Store::site(&world.item.id, true).unwrap();
            let fault = Fault {
                store: &store,
                save: Cell::new(0),
                at: 1,
                phase,
                crash: false,
            };
            assert!(world
                .session
                .perform_reply_locked(
                    &world.item,
                    if drafted { None } else { Some(" Thanks ") },
                    None,
                    Sending::Now,
                    &fault
                )
                .is_err());
            assert!(world.calls().is_empty(), "{phase:?}");
            assert_eq!(fs::read_to_string(&path).unwrap(), "Thanks");
        }
    }
}

#[test]
fn acknowledgement_then_confirmation_fault_or_crash_recovers_exact_saved_operation() {
    for phase in [SavePhase::Write, SavePhase::FileSync, SavePhase::Rename] {
        for crash in [false, true] {
            let world = World::new();
            let path = world.draft("Thanks");
            let store = Store::site(&world.item.id, true).unwrap();
            let fault = Fault {
                store: &store,
                save: Cell::new(0),
                at: 2,
                phase,
                crash,
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                world
                    .session
                    .perform_reply_locked(&world.item, None, None, Sending::Now, &fault)
            }));
            assert!(result.is_err() || result.unwrap().is_err());
            assert_eq!(world.calls().len(), 1);
            let saved = store.read().unwrap().unwrap();
            assert_eq!(saved.intent.as_ref().unwrap().status, Status::Pending);
            assert_eq!(saved.intent.as_ref().unwrap().text, "Thanks");
            fs::write(&path, "Edited").unwrap();
            let mut advanced = world.item.clone();
            advanced.raw["threads"][0]["reply"] = json!({"in_reply_to":"M"});
            drop(store);
            let restarted = Store::site(&world.item.id, true).unwrap();
            world
                .session
                .perform_reply_locked(&advanced, None, None, Sending::Now, &restarted)
                .unwrap();
            let calls = world.calls();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0], calls[1]);
            assert!(restarted.read().unwrap().unwrap().confirmed.contains(&path));
        }
    }
}

/// Stop immediately after the real durable confirmation, before `retire`.
struct AfterConfirmation<'a>(&'a Store);
impl ReplyStorage for AfterConfirmation<'_> {
    fn read(&self) -> crate::error::Result<Option<Record>> {
        self.0.read()
    }
    fn save(&self, record: &Record) -> crate::error::Result<()> {
        self.0.save(record)?;
        assert!(
            record.intent.as_ref().unwrap().status != Status::Sent,
            "crash after durable confirmation"
        );
        Ok(())
    }
}

#[test]
fn durable_confirmation_before_retirement_crash_suppresses_after_restart_and_later_intent() {
    let world = World::new();
    let path = world.draft("Thanks");
    let store = Store::site(&world.item.id, true).unwrap();
    let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        world.session.perform_reply_locked(
            &world.item,
            None,
            None,
            Sending::Now,
            &AfterConfirmation(&store),
        )
    }));
    assert!(crash.is_err());
    assert!(path.exists(), "retirement did not run");
    drop(store);
    let restarted = Store::site(&world.item.id, true).unwrap();
    let confirmed = restarted.read().unwrap().unwrap().confirmed;
    let mut advanced = world.item.clone();
    advanced.raw["threads"][0]["reply"] = json!({"in_reply_to":"M"});
    world
        .session
        .perform_reply_locked(
            &advanced,
            Some("Later deliberate words"),
            None,
            Sending::Now,
            &restarted,
        )
        .unwrap();
    assert!(world
        .session
        .perform_reply_locked(&advanced, None, None, Sending::Now, &restarted)
        .unwrap_err()
        .to_string()
        .contains("already been sent"));
    assert_eq!(world.calls().len(), 2);
    assert_eq!(restarted.read().unwrap().unwrap().confirmed, confirmed);
}

#[test]
fn invalid_and_unreadable_records_refuse_before_carrier_call() {
    let world = World::new();
    let store = Store::site(&world.item.id, true).unwrap();
    let mut record = world.record(true);
    record.version = 2;
    store.save(&record).unwrap();
    for resolution in [None, Some(Resolution::Sent), Some(Resolution::NotSent)] {
        assert!(world
            .session
            .perform_reply_locked(
                &world.item,
                Some("Thanks"),
                resolution,
                Sending::Now,
                &store
            )
            .is_err());
    }
    assert!(world.calls().is_empty());
    let file = fs::read_dir(world.tmp.path().join("replies"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .unwrap();
    fs::remove_file(&file).unwrap();
    fs::create_dir(&file).unwrap();
    assert!(world
        .session
        .perform_reply_locked(&world.item, Some("Thanks"), None, Sending::Now, &store)
        .is_err());
    assert!(world.calls().is_empty());
}

#[test]
fn actual_row_lock_refuses_all_send_replay_and_resolution_combinations() {
    for held in [false, true] {
        let world = World::new();
        world.draft("Thanks");
        let store = Store::site(&world.item.id, true).unwrap();
        let mut record = world.record(true);
        if held {
            record.intent.as_mut().unwrap().status = Status::Held;
        }
        store.save(&record).unwrap();
        // The held inode represents an in-flight drafted/typed/replay or
        // resolution move. Acquisition is nonblocking for every contender.
        for words in [None, Some("Thanks"), Some("new send")] {
            for resolution in [None, Some(Resolution::Sent), Some(Resolution::NotSent)] {
                let outcome =
                    world
                        .session
                        .reply_move(&world.item, words, resolution, Sending::Now);
                assert!(
                    outcome.says.contains("already in progress"),
                    "{}",
                    outcome.says
                );
            }
        }
        assert!(world.calls().is_empty());
        drop(store);
        if held {
            let outcome =
                world
                    .session
                    .resolve_reply(&world.item, None, Resolution::NotSent, Sending::Now);
            assert!(outcome.says.contains("not-sent"));
        } else {
            assert!(world
                .session
                .reply(&world.item, None, Sending::Now)
                .says
                .contains("posted"));
            assert_eq!(world.calls().len(), 1);
        }
    }
}

#[test]
fn acquisition_rereads_changed_durable_state_and_committed_proposal() {
    let world = World::new();
    let path = world.draft("old words");
    // Load the old proposal just as a screen does before a competing commit.
    let loaded = world.session.conversation(&world.item);
    assert_eq!(loaded.draft.unwrap().text, "old words");
    fs::write(&path, "committed newer words").unwrap();
    let store = Store::site(&world.item.id, true).unwrap();
    let mut record = world.record(false);
    record.intent.as_mut().unwrap().text = "saved before acquisition".into();
    store.save(&record).unwrap();
    drop(store);
    assert!(world
        .session
        .reply(&world.item, None, Sending::Now)
        .says
        .contains("held"));
    assert!(world.calls().is_empty());
    let store = Store::site(&world.item.id, true).unwrap();
    let mut record = store.read().unwrap().unwrap();
    record.intent = None;
    store.save(&record).unwrap();
    drop(store);
    let outcome = world.session.reply(&world.item, None, Sending::Now);
    assert!(outcome.says.contains("posted"), "{}", outcome.says);
    assert_eq!(world.calls()[0]["text"], "committed newer words");
}

#[test]
fn dry_send_replay_and_both_resolutions_have_no_transient_storage_operations() {
    let world = World::new();
    world.draft("Thanks");
    storage::take_operations();
    assert!(world
        .session
        .reply(&world.item, Some(" Thanks "), Sending::Dry)
        .says
        .contains("Thanks"));
    assert!(world
        .session
        .reply(&world.item, None, Sending::Dry)
        .says
        .contains("Thanks"));
    assert!(storage::take_operations().is_empty());
    assert!(!world.tmp.path().join("replies").exists());
    let store = Store::site(&world.item.id, true).unwrap();
    store.save(&world.record(true)).unwrap();
    drop(store);
    storage::take_operations();
    let retry = world.session.reply(&world.item, None, Sending::Dry);
    assert!(
        retry.says.contains("retry saved send")
            && retry.says.contains("H")
            && retry.says.contains("Thanks")
    );
    assert!(storage::take_operations().is_empty());
    let store = Store::site(&world.item.id, true).unwrap();
    store.save(&world.record(false)).unwrap();
    drop(store);
    storage::take_operations();
    for resolution in [Resolution::Sent, Resolution::NotSent] {
        assert!(world
            .session
            .resolve_reply(&world.item, None, resolution, Sending::Dry)
            .says
            .contains("would resolve"));
    }
    assert!(storage::take_operations().is_empty());
    assert!(world.calls().is_empty());
}

#[test]
fn deterministic_owners_exclude_drafted_typed_replay_and_checked_resolution_contenders() {
    for owner in ["drafted", "typed", "replay", "sent", "not-sent"] {
        let world = World::new();
        world.draft("Thanks");
        if ["replay", "sent", "not-sent"].contains(&owner) {
            let store = Store::site(&world.item.id, true).unwrap();
            store.save(&world.record(owner == "replay")).unwrap();
        }
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let world = &world;
            let running = scope.spawn(move || {
                let _ledger =
                    crate::work::ledger::use_test_path(world.tmp.path().join("work.json"));
                let store =
                    Store::open(&world.tmp.path().join("replies"), &world.item.id, true).unwrap();
                ready_tx.send(()).unwrap();
                if release_rx.recv().is_err() {
                    return;
                }
                let resolution = match owner {
                    "sent" => Some(Resolution::Sent),
                    "not-sent" => Some(Resolution::NotSent),
                    _ => None,
                };
                let words = (owner == "typed").then_some("Thanks");
                world
                    .session
                    .perform_reply_locked(&world.item, words, resolution, Sending::Now, &store)
                    .unwrap();
            });
            ready_rx
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap();
            for (words, resolution) in [
                (None, None),
                (Some("Thanks"), None),
                (None, Some(Resolution::Sent)),
                (None, Some(Resolution::NotSent)),
            ] {
                let refusal =
                    world
                        .session
                        .reply_move(&world.item, words, resolution, Sending::Now);
                assert!(
                    !refusal.ok && refusal.says.contains("already in progress"),
                    "{owner}: {}",
                    refusal.says
                );
            }
            assert!(world.calls().is_empty());
            release_tx.send(()).unwrap();
            running.join().unwrap();
        });
        assert_eq!(
            world.calls().len(),
            usize::from(!["sent", "not-sent"].contains(&owner))
        );
    }
}

/// Re-read is observed on the existing storage seam after lock acquisition.
struct ChangedAtRead<'a> {
    store: &'a Store,
    record: std::cell::RefCell<Option<Record>>,
}
impl ReplyStorage for ChangedAtRead<'_> {
    fn read(&self) -> crate::error::Result<Option<Record>> {
        if let Some(record) = self.record.borrow_mut().take() {
            self.store.save(&record)?;
        }
        self.store.read()
    }
    fn save(&self, record: &Record) -> crate::error::Result<()> {
        self.store.save(record)
    }
}

#[test]
fn state_and_provenance_are_observed_at_locked_read_instead_of_session_snapshot() {
    let world = World::new();
    let original_path = world.draft("old displayed words");
    assert_eq!(
        world.session.conversation(&world.item).draft.unwrap().text,
        "old displayed words"
    );
    let store = Store::site(&world.item.id, true).unwrap();
    let changed = ChangedAtRead {
        store: &store,
        record: std::cell::RefCell::new(Some(world.record(false))),
    };
    assert!(world
        .session
        .perform_reply_locked(&world.item, None, None, Sending::Now, &changed)
        .unwrap_err()
        .to_string()
        .contains("held"));
    assert!(world.calls().is_empty());
    let mut record = store.read().unwrap().unwrap();
    record.intent = None;
    store.save(&record).unwrap();
    let mut ledger: Value =
        serde_json::from_slice(&fs::read(world.tmp.path().join("work.json")).unwrap()).unwrap();
    let new_path = world.tmp.path().join("new-request.reply.md");
    fs::write(&new_path, "latest committed request").unwrap();
    let dispatch = &mut ledger["entries"][&world.item.id]["dispatches"][0];
    dispatch["reply_path"] = json!(new_path);
    dispatch["reply_binding"]["path"] = json!(new_path);
    dispatch["reply_binding"]["target"] = json!({"in_reply_to":"M"});
    fs::write(
        world.tmp.path().join("work.json"),
        serde_json::to_vec(&ledger).unwrap(),
    )
    .unwrap();
    let mut advanced = world.item.clone();
    advanced.raw["threads"][0]["reply"] = json!({"in_reply_to":"M"});
    world
        .session
        .perform_reply_locked(&advanced, None, None, Sending::Now, &store)
        .unwrap();
    assert_eq!(world.calls()[0]["text"], "latest committed request");
    assert_eq!(world.calls()[0]["target"], json!({"in_reply_to":"M"}));
    assert_eq!(
        fs::read_to_string(original_path).unwrap(),
        "old displayed words"
    );
}

#[test]
fn confirmation_directory_sync_failure_keeps_visible_confirmation_and_suppresses_replay() {
    let world = World::new();
    let path = world.draft("Thanks");
    let store = Store::site(&world.item.id, true).unwrap();
    let fault = Fault {
        store: &store,
        save: Cell::new(0),
        at: 2,
        phase: SavePhase::DirectorySync,
        crash: false,
    };
    assert!(world
        .session
        .perform_reply_locked(&world.item, None, None, Sending::Now, &fault)
        .is_err());
    // The atomic rename happened; if this synced file remains after restart,
    // it must suppress. Earlier-phase failures pin exact pending replay.
    assert_eq!(
        store.read().unwrap().unwrap().intent.unwrap().status,
        Status::Sent
    );
    assert!(path.exists(), "retirement did not follow a failed sync");
    drop(store);
    assert!(world
        .session
        .reply(&world.item, None, Sending::Now)
        .says
        .contains("already been sent"));
    assert_eq!(world.calls().len(), 1);
}

#[test]
fn dry_refusals_match_actual_moves_without_attempting_storage_mutation() {
    let world = World::new();
    world.draft("Thanks");
    let store = Store::site(&world.item.id, true).unwrap();
    store.save(&world.record(false)).unwrap();
    drop(store);
    for (words, resolution) in [
        (Some("different"), None),
        (Some("Thanks"), Some(Resolution::Sent)),
        (Some("Thanks"), Some(Resolution::NotSent)),
    ] {
        storage::take_operations();
        let dry = world
            .session
            .reply_move(&world.item, words, resolution, Sending::Dry);
        assert!(storage::take_operations().is_empty());
        let now = world
            .session
            .reply_move(&world.item, words, resolution, Sending::Now);
        assert!(!dry.ok && !now.ok);
        assert_eq!(dry.says, now.says);
    }
    assert!(world.calls().is_empty());
}

#[test]
fn held_decision_save_faults_call_nothing_and_never_silently_release_before_replacement() {
    for resolution in [Resolution::Sent, Resolution::NotSent] {
        for phase in [
            SavePhase::Write,
            SavePhase::FileSync,
            SavePhase::Rename,
            SavePhase::DirectorySync,
        ] {
            let world = World::new();
            let path = world.draft("Thanks");
            let store = Store::site(&world.item.id, true).unwrap();
            store.save(&world.record(false)).unwrap();
            let fault = Fault {
                store: &store,
                save: Cell::new(0),
                at: 1,
                phase,
                crash: false,
            };
            assert!(world
                .session
                .perform_reply_locked(&world.item, None, Some(resolution), Sending::Now, &fault)
                .is_err());
            assert!(world.calls().is_empty());
            assert_eq!(fs::read_to_string(path).unwrap(), "Thanks");
            let saved = store.read().unwrap().unwrap();
            if phase == SavePhase::DirectorySync {
                assert_eq!(
                    saved.intent.as_ref().unwrap().status,
                    if resolution == Resolution::Sent {
                        Status::Sent
                    } else {
                        Status::NotSent
                    }
                );
            } else {
                assert!(saved.intent.as_ref().unwrap().held());
                assert!(saved.confirmed.is_empty());
                assert!(saved.accepted.is_empty());
            }
        }
    }
}

#[test]
fn eligible_pending_is_not_a_checked_hold_and_refuses_both_choices_and_words() {
    let world = World::new();
    let store = Store::site(&world.item.id, true).unwrap();
    store.save(&world.record(true)).unwrap();
    for sending in [Sending::Now, Sending::Dry] {
        for resolution in [Resolution::Sent, Resolution::NotSent] {
            assert!(world
                .session
                .perform_reply_locked(&world.item, None, Some(resolution), sending, &store)
                .unwrap_err()
                .to_string()
                .contains("Only a held"));
            assert!(world
                .session
                .perform_reply_locked(
                    &world.item,
                    Some("Thanks"),
                    Some(resolution),
                    sending,
                    &store
                )
                .unwrap_err()
                .to_string()
                .contains("no words"));
        }
    }
    assert_eq!(
        store.read().unwrap().unwrap().intent.unwrap().status,
        Status::Pending
    );
    assert!(world.calls().is_empty());
}
