//! The bounded unknown-evidence repair and its still-open commitment boundary
//! (§FS-005-dispatch.13, §FS-001-forge-interface.2, §FS-011-command-line.4).

use super::tests::World;
use super::*;
use crate::replies::storage::{self, EvidencePhase, SavePhase};
use serde_json::{json, Value};
use std::{cell::Cell, fs, io, path::PathBuf};

/// All failures use the production writer/receipt and the real shared move.
struct Fault<'a> {
    store: &'a Store,
    saves: Cell<usize>,
    row: Option<SavePhase>,
    evidence: Option<EvidencePhase>,
    crash: bool,
}

impl ReplyStorage for Fault<'_> {
    fn read(&self) -> crate::error::Result<Option<Record>> {
        self.store.read()
    }
    fn save(&self, record: &Record) -> crate::error::Result<()> {
        self.saves.set(self.saves.get() + 1);
        self.store.save_with(record, |phase| {
            if self.saves.get() == 2 && self.row == Some(phase) {
                assert!(!self.crash, "death before primary Held replacement phase");
                Err(io::Error::other("primary outcome writer failed"))
            } else {
                Ok(())
            }
        })
    }
    fn reserve(&self, record: &mut Record) -> crate::error::Result<Receipt> {
        self.store.reserve_with(record, |phase| {
            if self.evidence == Some(phase) {
                Err(io::Error::other("evidence reservation failed"))
            } else {
                Ok(())
            }
        })
    }
    fn hold(&self, receipt: &mut Receipt, note: &str) -> crate::error::Result<()> {
        receipt.hold_with(note, |phase| {
            if self.evidence == Some(phase) {
                assert!(
                    !self.crash,
                    "death after decoding Unknown, before evidence write/sync"
                );
                Err(io::Error::other("evidence outcome writer failed"))
            } else {
                Ok(())
            }
        })
    }
}

fn fault(store: &Store, row: Option<SavePhase>, evidence: Option<EvidencePhase>) -> Fault<'_> {
    Fault {
        store,
        saves: Cell::new(0),
        row,
        evidence,
        crash: false,
    }
}

fn advanced(world: &World) -> Item {
    let mut item = world.item.clone();
    item.raw["threads"][0]["messages"]
        .as_array_mut()
        .unwrap()
        .push(
            json!({"author":"dana","when":"2026-10-07T11:00:00Z","text":"new words","mine":false}),
        );
    item.raw["threads"][0]["reply"] = json!({"in_reply_to":"M"});
    item
}

fn files(world: &World) -> (PathBuf, PathBuf) {
    let entries: Vec<_> = fs::read_dir(world.tmp.path().join("replies"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    (
        entries
            .iter()
            .find(|path| path.extension().is_some_and(|ext| ext == "json"))
            .unwrap()
            .clone(),
        entries
            .iter()
            .find(|path| path.extension().is_some_and(|ext| ext == "receipt"))
            .unwrap()
            .clone(),
    )
}

#[test]
fn committed_unknown_survives_every_primary_writer_failure_and_interruption_for_both_intents() {
    for drafted in [false, true] {
        for phase in [
            SavePhase::Write,
            SavePhase::FileSync,
            SavePhase::Rename,
            SavePhase::DirectorySync,
        ] {
            for crash in [false, true] {
                let world = World::new();
                let draft = world.draft("Thanks");
                fs::write(world.tmp.path().join("unknown"), "").unwrap();
                let store = Store::site(&world.item.id, true).unwrap();
                let mut fault = fault(&store, Some(phase), None);
                fault.crash = crash;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    world.session.perform_reply_locked(
                        &world.item,
                        (!drafted).then_some(" Thanks "),
                        None,
                        Sending::Now,
                        &fault,
                    )
                }));
                if crash {
                    assert!(result.is_err());
                } else {
                    let diagnostic = result.unwrap().unwrap_err().to_string();
                    assert!(
                        diagnostic.contains("delivery unknown")
                            && diagnostic.contains("primary outcome writer failed"),
                        "{diagnostic}"
                    );
                }
                assert_eq!(world.calls().len(), 1);
                assert!(draft.exists());
                let (row, _) = files(&world);
                let raw: Value = serde_json::from_slice(&fs::read(row).unwrap()).unwrap();
                assert_eq!(
                    raw["intent"]["status"],
                    if phase == SavePhase::DirectorySync {
                        "held"
                    } else {
                        "pending"
                    }
                );
                // DirectorySync observes post-rename visibility, not a proof
                // that that replacement survived power loss. The receipt was
                // independently synced BEFORE this phase.
                drop(store);
                let reopened = Store::site(&world.item.id, true).unwrap();
                let saved = reopened.read().unwrap().unwrap();
                assert_eq!(saved.intent.as_ref().unwrap().status, Status::Held);
                assert_eq!(saved.intent.as_ref().unwrap().text, "Thanks");
                assert!(saved
                    .intent
                    .as_ref()
                    .unwrap()
                    .note
                    .as_ref()
                    .unwrap()
                    .contains("delivery unknown"));
                drop(reopened);
                fs::write(&draft, "Edited newer words").unwrap();
                let advanced = advanced(&world);
                let view = world.session.conversation(&advanced).view(&advanced);
                let pending = view.pending_reply.as_ref().unwrap();
                assert_eq!(pending.status, "held");
                assert!(!pending.retry);
                assert_eq!(pending.target, Some(json!({"in_reply_to":"H"})));
                assert_eq!(pending.resolutions, ["sent", "not-sent"]);
                let value = serde_json::to_value(view).unwrap();
                assert!(crate::api::schema::holds("thread", &value).is_empty());
                assert_eq!(world.session.recovery_rows().unwrap()[0].id, world.item.id);
                assert!(world.session.reply_item(&world.item.id).unwrap().is_some());
                storage::take_operations();
                let dry = world.session.reply(&advanced, None, Sending::Dry);
                assert!(!dry.ok && dry.says.contains("delivery unknown"));
                assert!(storage::take_operations().is_empty());
                let now =
                    world
                        .session
                        .reply(&advanced, (!drafted).then_some("Thanks"), Sending::Now);
                assert!(!now.ok && now.says.contains("held"));
                assert_eq!(world.calls().len(), 1);
            }
        }
    }
}

#[test]
fn reservation_failure_makes_zero_calls_for_new_intents_and_legacy_recovery() {
    for phase in [
        EvidencePhase::ReserveWrite,
        EvidencePhase::ReserveFileSync,
        EvidencePhase::ReserveDirectorySync,
    ] {
        for drafted in [false, true] {
            for legacy in [false, true] {
                let world = World::new();
                let path = world.draft("Thanks");
                let store = Store::site(&world.item.id, true).unwrap();
                if legacy {
                    store.save(&world.record(true)).unwrap();
                }
                let before = store
                    .read()
                    .unwrap()
                    .map(|r| serde_json::to_value(r).unwrap());
                let fault = fault(&store, None, Some(phase));
                let result = world.session.perform_reply_locked(
                    &world.item,
                    (!drafted).then_some("Thanks"),
                    None,
                    Sending::Now,
                    &fault,
                );
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains("evidence reservation failed"));
                assert!(world.calls().is_empty());
                assert_eq!(
                    store
                        .read()
                        .unwrap()
                        .map(|r| serde_json::to_value(r).unwrap()),
                    before
                );
                assert_eq!(fs::read_to_string(&path).unwrap(), "Thanks");
                assert!(fs::read_dir(world.tmp.path().join("replies"))
                    .unwrap()
                    .all(|entry| entry
                        .unwrap()
                        .path()
                        .extension()
                        .is_none_or(|ext| ext != "receipt")));
            }
        }
    }
}

#[test]
fn evidence_bound_hold_resolutions_keep_history_and_later_intents_use_new_identity() {
    for drafted in [false, true] {
        for resolution in [Resolution::Sent, Resolution::NotSent] {
            let world = World::new();
            let path = world.draft("Thanks");
            fs::write(world.tmp.path().join("unknown"), "").unwrap();
            let store = Store::site(&world.item.id, true).unwrap();
            let fault = fault(&store, Some(SavePhase::Rename), None);
            assert!(world
                .session
                .perform_reply_locked(
                    &world.item,
                    (!drafted).then_some("Thanks"),
                    None,
                    Sending::Now,
                    &fault
                )
                .is_err());
            let old = store.read().unwrap().unwrap();
            let evidence = old.evidence.clone();
            let binding = old.intent.as_ref().unwrap().binding.clone();
            drop(store);
            storage::take_operations();
            assert!(
                world
                    .session
                    .resolve_reply(&world.item, None, resolution, Sending::Dry)
                    .ok
            );
            assert!(storage::take_operations().is_empty());
            assert!(
                !world
                    .session
                    .resolve_reply(&world.item, Some("changed"), resolution, Sending::Now)
                    .ok
            );
            assert!(
                world
                    .session
                    .resolve_reply(&world.item, None, resolution, Sending::Now)
                    .ok
            );
            assert_eq!(world.calls().len(), 1);
            let store = Store::site(&world.item.id, true).unwrap();
            let resolved = store.read().unwrap().unwrap();
            assert_eq!(resolved.evidence, evidence);
            assert_eq!(
                resolved.generation(&binding),
                u64::from(resolution == Resolution::Sent)
            );
            assert_eq!(
                resolved.confirmed.contains(&path),
                drafted && resolution == Resolution::Sent
            );
            if resolution == Resolution::NotSent {
                assert_eq!(fs::read_to_string(&path).unwrap(), "Thanks");
            }
            drop(store);
            assert!(
                !world
                    .session
                    .resolve_reply(&world.item, None, resolution, Sending::Now)
                    .ok
            );
            fs::remove_file(world.tmp.path().join("unknown")).unwrap();
            let advanced = advanced(&world);
            assert!(
                world
                    .session
                    .reply(&advanced, Some("Thanks"), Sending::Now)
                    .ok
            );
            let later = Store::inspect(&world.item.id).unwrap().unwrap();
            assert_ne!(
                later.evidence, evidence,
                "byte-identical words still start a later operation"
            );
            assert_eq!(later.confirmed, resolved.confirmed);
            assert_eq!(world.calls().len(), 2);
            assert_eq!(world.calls()[1]["target"], json!({"in_reply_to":"M"}));
        }
    }
}

#[test]
fn invalid_unreadable_truncated_or_conflicting_evidence_blocks_all_moves_and_isolates_rows() {
    for damage in [
        "missing",
        "unreadable",
        "truncate",
        "extra",
        "note",
        "operation",
        "target",
        "words",
        "context",
        "request",
        "eligibility",
        "draft",
        "version",
        "resolved",
        "receipt-version",
        "receipt-operation",
        "receipt-payload",
    ] {
        let world = World::new();
        world.draft("Thanks");
        fs::write(world.tmp.path().join("unknown"), "").unwrap();
        assert!(!world.session.reply(&world.item, None, Sending::Now).ok);
        let (row, receipt) = files(&world);
        let mut record: Value = serde_json::from_slice(&fs::read(&row).unwrap()).unwrap();
        match damage {
            "missing" => fs::remove_file(&receipt).unwrap(),
            "unreadable" => {
                fs::remove_file(&receipt).unwrap();
                fs::create_dir(&receipt).unwrap();
            }
            "truncate" => {
                let mut bytes = fs::read(&receipt).unwrap();
                bytes.pop();
                fs::write(&receipt, bytes).unwrap();
            }
            "extra" => {
                use std::io::Write;
                writeln!(
                    fs::OpenOptions::new().append(true).open(&receipt).unwrap(),
                    "{{\"unknown\":\"conflict\"}}"
                )
                .unwrap();
            }
            "note" => record["intent"]["note"] = json!("a conflicting note"),
            "operation" => record["evidence"] = json!("../escape"),
            "target" => record["intent"]["binding"]["target"] = json!({"in_reply_to":"M"}),
            "words" => record["intent"]["text"] = json!("changed payload"),
            "context" => {
                record["intent"]["binding"]["context"] = json!("elsewhere");
                record["intent"]["request"]["project"] = json!("elsewhere");
            }
            "request" => record["intent"]["request"]["timeout_seconds"] = json!(500),
            "eligibility" => record["intent"]["reconciliation"] = json!(false),
            "draft" => record["intent"]["draft"] = json!("different.reply.md"),
            "resolved" => {
                record["intent"]["status"] = json!("sent");
                record["intent"]["note"] = Value::Null;
            }
            "receipt-version" | "receipt-operation" | "receipt-payload" => {
                let bytes = fs::read_to_string(&receipt).unwrap();
                let mut lines = bytes.lines();
                let mut reservation: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
                match damage {
                    "receipt-version" => reservation["version"] = json!(99),
                    "receipt-operation" => reservation["operation"] = json!("deadbeef"),
                    _ => reservation["payload"]["text"] = json!("unrelated operation"),
                }
                fs::write(
                    &receipt,
                    format!("{}\n{}\n", reservation, lines.next().unwrap()),
                )
                .unwrap();
            }
            _ => record["version"] = json!(1),
        }
        fs::write(&row, serde_json::to_vec(&record).unwrap()).unwrap();
        // A second valid unresolved row remains addressable during corruption.
        let mut other = world.record(false);
        other.row.id = "mail:valid-other".into();
        other.intent.as_mut().unwrap().binding.row = other.row.id.clone();
        Store::site(&other.row.id, true)
            .unwrap()
            .save(&other)
            .unwrap();
        assert!(Store::inspect(&world.item.id).is_err(), "{damage}");
        assert_eq!(world.session.recovery_rows().unwrap()[0].id, other.row.id);
        assert_eq!(world.session.recovery_diagnostics().len(), 1);
        for sending in [Sending::Dry, Sending::Now] {
            for resolution in [None, Some(Resolution::Sent), Some(Resolution::NotSent)] {
                assert!(
                    !world
                        .session
                        .reply_move(&world.item, None, resolution, sending)
                        .ok,
                    "{damage}"
                );
            }
        }
        let value = serde_json::to_value(world.session.conversation(&world.item).view(&world.item))
            .unwrap();
        assert!(value["reply_error"].is_string());
        assert_eq!(value["draft"]["sendable"], false);
        assert!(crate::api::schema::holds("thread", &value).is_empty());
        assert_eq!(world.calls().len(), 1);
    }
}

#[test]
fn checked_decision_failures_keep_evidence_hold_until_replacement_is_visible() {
    for resolution in [Resolution::Sent, Resolution::NotSent] {
        for phase in [
            SavePhase::Write,
            SavePhase::FileSync,
            SavePhase::Rename,
            SavePhase::DirectorySync,
        ] {
            let world = World::new();
            world.draft("Thanks");
            fs::write(world.tmp.path().join("unknown"), "").unwrap();
            let store = Store::site(&world.item.id, true).unwrap();
            assert!(world
                .session
                .perform_reply_locked(
                    &world.item,
                    None,
                    None,
                    Sending::Now,
                    &fault(&store, Some(SavePhase::Write), None)
                )
                .is_err());
            let fault = fault(&store, Some(phase), None);
            fault.saves.set(1); // Inject into this decision's actual row save.
            assert!(world
                .session
                .perform_reply_locked(&world.item, None, Some(resolution), Sending::Now, &fault)
                .is_err());
            assert_eq!(world.calls().len(), 1);
            drop(store);
            let reopened = Store::inspect(&world.item.id).unwrap().unwrap();
            if phase == SavePhase::DirectorySync {
                // Visible rename alone does not prove synchronized commitment.
                assert_eq!(
                    reopened.intent.unwrap().status,
                    if resolution == Resolution::Sent {
                        Status::Sent
                    } else {
                        Status::NotSent
                    }
                );
            } else {
                assert_eq!(reopened.intent.unwrap().status, Status::Held);
            }
        }
    }
}

#[test]
fn unknown_without_committed_evidence_exposes_the_remaining_blocking_boundary() {
    for drafted in [false, true] {
        for phase in [EvidencePhase::OutcomeWrite, EvidencePhase::OutcomeFileSync] {
            for crash in [false, true] {
                let world = World::new();
                world.draft("Thanks");
                fs::write(world.tmp.path().join("unknown"), "").unwrap();
                let store = Store::site(&world.item.id, true).unwrap();
                let mut fault = fault(&store, Some(SavePhase::Write), Some(phase));
                fault.crash = crash;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    world.session.perform_reply_locked(
                        &world.item,
                        (!drafted).then_some("Thanks"),
                        None,
                        Sending::Now,
                        &fault,
                    )
                }));
                if crash {
                    assert!(result.is_err());
                } else {
                    let diagnostic = result.unwrap().unwrap_err().to_string();
                    assert!(
                        diagnostic.contains("delivery unknown")
                            && diagnostic.contains("evidence outcome writer failed")
                            && diagnostic.contains("primary outcome writer failed")
                    );
                }
                let (row, receipt) = files(&world);
                let before: Value =
                    serde_json::from_slice(&fs::read(world.tmp.path().join("before")).unwrap())
                        .unwrap();
                assert_eq!(
                    serde_json::from_slice::<Value>(&fs::read(row).unwrap()).unwrap(),
                    before[0]
                );
                let bytes = fs::read(&receipt).unwrap();
                if phase == EvidencePhase::OutcomeFileSync {
                    assert_eq!(
                        store.read().unwrap().unwrap().intent.unwrap().status,
                        Status::Held,
                        "visible unsynced evidence already refuses"
                    );
                    // Model loss of the unsynchronized append. This is a fault
                    // model, NOT a claim that ordinary reopening causes loss.
                    let end = bytes.iter().position(|byte| *byte == b'\n').unwrap() + 1;
                    fs::write(&receipt, &bytes[..end]).unwrap();
                }
                let before_receipts: Value = serde_json::from_slice(
                    &fs::read(world.tmp.path().join("before-receipts")).unwrap(),
                )
                .unwrap();
                assert_eq!(fs::read(&receipt).unwrap(), before_receipts[receipt.file_name().unwrap().to_str().unwrap()].as_str().unwrap().as_bytes(),
                    "no outcome bytes distinguish this operation from its pre-invocation reservation");
                drop(store);
                let pending = Store::inspect(&world.item.id).unwrap().unwrap();
                assert_eq!(pending.intent.as_ref().unwrap().status, Status::Pending);
                assert!(
                    world
                        .session
                        .conversation(&world.item)
                        .pending_reply
                        .unwrap()
                        .retry
                );
                // This is the unresolved contract gap, not accepted behavior:
                // durable observations equal an invocation with no outcome.
                // The required safety reproducer is not weakened.
                assert_eq!(world.calls().len(), 1);
            }
        }
    }
}

#[test]
fn failed_evidence_write_still_attempts_primary_hold_and_preserves_both_diagnostics() {
    for drafted in [false, true] {
        for phase in [EvidencePhase::OutcomeWrite, EvidencePhase::OutcomeFileSync] {
            let world = World::new();
            world.draft("Thanks");
            fs::write(world.tmp.path().join("unknown"), "").unwrap();
            let store = Store::site(&world.item.id, true).unwrap();
            let fault = fault(&store, None, Some(phase));
            let diagnostic = world
                .session
                .perform_reply_locked(
                    &world.item,
                    (!drafted).then_some("Thanks"),
                    None,
                    Sending::Now,
                    &fault,
                )
                .unwrap_err()
                .to_string();
            assert!(
                diagnostic.contains("delivery unknown")
                    && diagnostic.contains("evidence outcome writer failed")
            );
            drop(store);
            assert_eq!(
                Store::inspect(&world.item.id)
                    .unwrap()
                    .unwrap()
                    .intent
                    .unwrap()
                    .status,
                Status::Held
            );
            assert!(
                !world
                    .session
                    .conversation(&world.item)
                    .pending_reply
                    .unwrap()
                    .retry
            );
            assert!(!world.session.reply(&world.item, None, Sending::Now).ok);
            assert_eq!(world.calls().len(), 1);
        }
    }
}
