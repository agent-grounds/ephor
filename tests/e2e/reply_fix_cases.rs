//! Follow-up coverage for checked recovery and request lifecycles
//! (§FS-005-dispatch.4, §FS-005-dispatch.13, §FS-001-forge-interface.9,
//! §FS-011-command-line.4, §FS-011-command-line.7).

use super::*;
use support::shaped;

fn saved(world: &MailWorld) -> (std::path::PathBuf, Value) {
    let path = fs::read_dir(world.world.path().join("state/ephor/replies"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension().is_some_and(|ext| ext == "json")
                && path.file_name().unwrap() != "unrelated.json"
        })
        .unwrap();
    let value = read_json(&path);
    (path, value)
}

fn held(world: &MailWorld, drafted: bool, unknown: bool) -> Option<std::path::PathBuf> {
    let draft = drafted.then(|| world.drafted("Thanks"));
    world.flag(if unknown { "unknown" } else { "fail-ack" });
    let out = world.reply(if drafted { None } else { Some("Thanks") });
    assert!(!out.status.success(), "{}", says(&out));
    assert_eq!(world.requests().len(), 1);
    draft
}

#[test]
fn checked_resolution_matrix_saves_without_calls_or_gateway_changes_after_restart_and_absence() {
    for drafted in [false, true] {
        for unknown in [false, true] {
            for choice in ["sent", "not-sent"] {
                let world = MailWorld::new(unknown);
                let draft = held(&world, drafted, unknown);
                let gateway = tree(&world.world.path().join("mail"));
                world.flag("absent");
                world.refresh();
                let row = world.thread();
                assert_eq!(row["pending_reply"]["status"], "held");
                let dry = world.ok(&["reply", ITEM, "--resolve", choice, "--dry-run", "--json"]);
                assert!(shaped("outcome", &dry)["says"]
                    .as_str()
                    .unwrap()
                    .contains("would resolve"));
                let (path, before) = saved(&world);
                let with_words =
                    world.run(&["reply", ITEM, "Changed", "--resolve", choice, "--json"]);
                assert!(!with_words.status.success());
                shaped("outcome", &with_words);
                assert_eq!(read_json(&path), before);
                let out = world.ok(&["reply", ITEM, "--resolve", choice, "--json"]);
                assert_eq!(shaped("outcome", &out)["ok"], true);
                assert_eq!(read_json(&path)["intent"]["status"], choice);
                assert_eq!(world.requests().len(), 1);
                let mut after = tree(&world.world.path().join("mail"));
                after.remove(&world.world.path().join("mail/absent"));
                assert_eq!(
                    after, gateway,
                    "a local resolution cannot amend gateway state"
                );
                if let Some(draft) = draft {
                    if choice == "sent" {
                        assert!(!draft.exists());
                        assert!(read_json(&path)["confirmed"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|value| value == &json!(draft)));
                    } else {
                        assert_eq!(fs::read_to_string(&draft).unwrap(), "Thanks");
                        // Restore a row, then deliberately advance it. Not-sent
                        // is not permission to send stale words on a new move.
                        fs::remove_file(world.world.path().join("mail/absent")).unwrap();
                        world.incoming();
                        world.refresh();
                        stale(&world, &["reply", ITEM], NEW_WORDS);
                    }
                }
                let again = world.run(&["reply", ITEM, "--resolve", choice]);
                assert!(
                    !again.status.success(),
                    "nonheld outcomes cannot resolve twice"
                );
                assert_eq!(world.requests().len(), 1);
            }
        }
    }
}

#[test]
fn not_sent_is_separate_from_a_future_checked_send_and_sent_advances_only_its_thread() {
    let world = MailWorld::new(true);
    let draft = held(&world, true, true).unwrap();
    world.ok(&["reply", ITEM, "--resolve", "not-sent"]);
    assert_eq!(fs::read_to_string(&draft).unwrap(), "Thanks");
    world.ok(&["reply", ITEM]);
    assert_eq!(world.delivered(), 1);
    assert_eq!(world.requests().len(), 2);

    let world = MailWorld::new(true);
    let mut row = world.get("conversation.json");
    row["threads"].as_array_mut().unwrap().push(json!({"messages":[message("other", "Other thread", false)],"reply":{"in_reply_to":"other"}}));
    world.put("conversation.json", row);
    world.refresh();
    held(&world, true, true);
    let (_, before) = saved(&world);
    let bound = before["intent"]["binding"]["thread"].as_u64().unwrap();
    assert_eq!(bound, 1);
    world.ok(&["reply", ITEM, "--resolve", "sent"]);
    let (_, record) = saved(&world);
    let accepted = record["accepted"].as_object().unwrap();
    assert_eq!(accepted.len(), 1);
    assert!(accepted.keys().all(|key| key.starts_with("[1,")));
}

#[test]
fn withdrawn_declaration_holds_resolves_or_restores_but_failed_capability_is_not_withdrawal() {
    for choice in ["sent", "not-sent", "restore"] {
        let world = MailWorld::new(true);
        held(&world, false, false);
        // Above is eligible pending because uncertainty was implicit.
        world.put("capabilities.json", json!({"messages":true,"replies":true}));
        let row = world.thread();
        assert_eq!(row["pending_reply"]["status"], "held");
        assert_eq!(row["pending_reply"]["retry"], false);
        if choice == "restore" {
            world.put(
                "capabilities.json",
                json!({"messages":true,"replies":true,"reply_reconciliation":true}),
            );
            world.ok(&["reply", ITEM, "Thanks"]);
            assert_eq!(world.requests().len(), 2);
            assert_eq!(world.delivered(), 1);
        } else {
            world.ok(&["reply", ITEM, "--resolve", choice]);
            assert_eq!(world.requests().len(), 1);
        }
    }
    let world = MailWorld::new(true);
    held(&world, false, false);
    let (path, before) = saved(&world);
    world.flag("fail-capabilities");
    let out = world.run(&["reply", ITEM, "--resolve", "sent"]);
    assert!(!out.status.success() && says(&out).contains("cannot read declaration"));
    assert_eq!(read_json(&path), before);
    assert_eq!(world.requests().len(), 1);
}

#[test]
fn original_legacy_and_unknown_holds_resolve_even_after_configuration_is_removed() {
    for unknown in [false, true] {
        for drafted in [false, true] {
            for choice in ["sent", "not-sent"] {
                let world = MailWorld::new(unknown);
                held(&world, drafted, unknown);
                world.flag("absent");
                world.refresh();
                world
                    .world
                    .configure(json!({"projects":{"demo":{"providers":[]}}}));
                world.ok(&["reply", ITEM, "--resolve", choice]);
                assert_eq!(world.requests().len(), 1);
                assert_eq!(saved(&world).1["intent"]["status"], choice);
            }
        }
    }
}

#[test]
fn original_site_context_survives_new_project_precedence_placement_replay_and_resolution() {
    for choice in ["replay", "sent", "not-sent"] {
        let world = MailWorld::new(true);
        world.world.configure(json!({"defaults":{"provider_timeout_seconds":1},"sources":[{"provider":"mail-me","user":"me"}],"projects":{"demo":{"providers":[]}}}));
        // Retain the already cached row's placement; fetch attribution is
        // independent of the binding chosen for the write.
        held(&world, false, choice != "replay");
        assert_eq!(world.requests()[0]["project"], "");
        world.world.configure(json!({"defaults":{"provider_timeout_seconds":1},"sources":[{"provider":"mail-me","user":"me"}],"projects":{"demo":{"providers":[{"provider":"mail-me","user":"other-account"}]}}}));
        world.flag("absent");
        world.refresh();
        if choice == "replay" {
            world.ok(&["reply", ITEM, "Thanks"]);
            assert_eq!(world.requests().len(), 2);
            assert_eq!(world.wires()[0], world.wires()[1]);
            assert_eq!(world.delivered(), 1);
        } else {
            world.ok(&["reply", ITEM, "--resolve", choice]);
            assert_eq!(world.requests().len(), 1);
        }
    }
}

#[test]
fn command_provider_and_account_changes_refuse_original_recovery_without_call() {
    for changed in [
        json!({"provider":"mail-me","user":"other"}),
        json!({"provider":"mail-me","user":"me","command":"different-command"}),
        json!({"provider":"other-provider","user":"me"}),
    ] {
        let world = MailWorld::new(true);
        held(&world, false, false);
        world
            .world
            .configure(json!({"projects":{"demo":{"providers":[changed]}}}));
        let (path, before) = saved(&world);
        let out = world.reply(Some("Thanks"));
        assert!(!out.status.success(), "{}", says(&out));
        assert_eq!(read_json(&path), before);
        assert_eq!(world.requests().len(), 1);
    }
}

#[test]
fn newest_empty_removed_and_absent_output_never_revives_older_words() {
    for state in ["empty", "removed", "absent"] {
        let world = MailWorld::new(false);
        let old = world.drafted("older words");
        let newest = if state == "absent" {
            world.ok(&[
                "work", "dispatch", "--item", ITEM, "--recipe", "answer", "--again",
            ]);
            world.requested_reply()
        } else {
            world.drafted("newer words")
        };
        match state {
            "empty" => fs::write(&newest, " \n").unwrap(),
            "removed" => fs::remove_file(&newest).unwrap(),
            _ => assert!(!newest.exists()),
        }
        assert_ne!(old, newest);
        assert!(world.thread().get("draft").is_none());
        assert!(!world.reply(None).status.success());
        assert!(world.requests().is_empty());
    }
}

#[test]
fn pending_payload_survives_new_request_edit_withdrawal_and_forgetting_work() {
    for change in ["new", "edit", "withdraw", "forget"] {
        let world = MailWorld::new(true);
        let draft = world.drafted("Thanks");
        world.flag("fail-ack");
        assert!(!world.reply(None).status.success());
        match change {
            "new" => {
                world.drafted("New request words");
            }
            "edit" => fs::write(&draft, "Edited words").unwrap(),
            "withdraw" => fs::write(&draft, "").unwrap(),
            _ => {
                world.ok(&["work", "forget", "--item", ITEM]);
            }
        }
        world.incoming();
        world.refresh();
        world.ok(&["reply", ITEM]);
        assert_eq!(world.wires()[0], world.wires()[1]);
        assert_eq!(world.delivered(), 1);
    }
}

#[test]
fn custom_ask_and_reopened_answer_advertise_distinct_bound_request_paths() {
    let world = MailWorld::new(false);
    let first = world.drafted("First");
    world.ok(&[
        "work",
        "ask",
        "--item",
        ITEM,
        "Answer and put words at {reply}",
    ]);
    let custom = world.requested_reply();
    assert_ne!(custom, first);
    let ledger = read_json(&world.world.path().join("state/ephor/work.json"));
    let dispatches = ledger["entries"][ITEM]["dispatches"].as_array().unwrap();
    for dispatch in dispatches {
        assert_eq!(dispatch["reply_binding"]["path"], dispatch["reply_path"]);
        assert_eq!(
            dispatch["reply_binding"]["target"],
            json!({"in_reply_to":H})
        );
    }
    fs::write(&custom, "Custom words").unwrap();
    assert_eq!(world.thread()["draft"]["text"], "Custom words");
    let reopened = world.drafted("Reopened");
    assert_ne!(reopened, first);
    assert_ne!(reopened, custom);
    assert_eq!(world.thread()["draft"]["text"], "Reopened");
}

#[test]
fn valid_recovery_plus_corrupt_record_remains_visible_with_shared_diagnostics_and_strict_moves() {
    let world = MailWorld::new(false);
    held(&world, false, false);
    world.flag("absent");
    world.refresh();
    fs::write(
        world
            .world
            .path()
            .join("state/ephor/replies/unrelated.json"),
        "{broken",
    )
    .unwrap();
    let thread = world.thread();
    assert_eq!(thread["pending_reply"]["text"], "Thanks");
    assert!(thread["reply_diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str().unwrap().contains("unrelated.json")));
    let human = world.ok(&["thread", ITEM]);
    assert!(says(&human).contains("unrelated.json"));
    let feed = world.ok(&["feed", "--json"]);
    assert!(shaped("feed", &feed)
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == ITEM));
    assert!(says(&feed).contains("unrelated.json"));
    let (path, _) = saved(&world);
    fs::write(&path, "{broken").unwrap();
    for args in [
        vec!["reply", ITEM, "Thanks"],
        vec!["reply", ITEM, "--resolve", "sent"],
        vec!["reply", ITEM, "--resolve", "not-sent"],
    ] {
        assert!(!world.run(&args).status.success());
    }
    assert_eq!(world.requests().len(), 1);
}

#[test]
fn additive_cli_views_and_outcomes_validate_in_all_reply_states() {
    let world = MailWorld::new(true);
    let path = world.drafted("Thanks");
    world.thread(); // bound view passes the actual published validator
    world.incoming();
    world.refresh();
    let stale = world.thread();
    let reason = stale["draft"]["stale_reason"].as_str().unwrap();
    for fact in ["dana", "2026-10-07T11:00:00Z", NEW_WORDS] {
        assert!(reason.contains(fact));
    }
    let out = world.run(&["reply", ITEM, "--json"]);
    assert!(!out.status.success());
    shaped("outcome", &out);
    world.flag("unknown");
    let out = world.run(&["reply", ITEM, "Thanks", "--json"]);
    assert!(!out.status.success());
    shaped("outcome", &out);
    fs::write(path, "Edited display words").unwrap();
    let held = world.thread();
    assert_eq!(held["pending_reply"]["text"], "Thanks");
    assert!(held["pending_reply"]["note"]
        .as_str()
        .unwrap()
        .contains("delivery unknown"));
    shaped(
        "outcome",
        &world.ok(&["reply", ITEM, "--resolve", "not-sent", "--json"]),
    );
    shaped(
        "outcome",
        &world.ok(&["reply", ITEM, "Typed deliberate words", "--json"]),
    );
    world.thread();
}

/// Native writes use the same durable guard, but never infer reconciliation
/// from a native adapter's success/failure (§FS-001-forge-interface.2).
#[test]
fn native_carrier_saves_before_call_and_both_checked_choices_never_call_it() {
    for drafted in [false, true] {
        for choice in ["sent", "not-sent", "accepted"] {
            let world = MailWorld::new(true);
            let mut row = world.get("conversation.json");
            row["threads"][0]["reply"] = json!({"provider":"github","subject_id":"NODE_H"});
            world.put("conversation.json", row);
            world.refresh();
            let draft = drafted.then(|| world.drafted("Thanks"));
            world.world.stub(
                "gh",
                r#"#!/usr/bin/env python3
import json, os, pathlib, sys
root=pathlib.Path(os.environ['REPLY_WORLD'])
with (root/'mail/native-calls').open('a') as stream:
    stream.write(json.dumps(sys.argv[1:])+'\n')
records=[json.loads(p.read_text()) for p in (root/'state/ephor/replies').glob('*.json')]
(root/'mail/native-before.json').write_text(json.dumps(records))
if (root/'mail/native-fail').exists():
    print('native acknowledgement lost',file=sys.stderr)
    sys.exit(1)
print('{}')
"#,
            );
            if choice != "accepted" {
                world.flag("native-fail");
            }
            let out = world.reply(if drafted { None } else { Some(" Thanks ") });
            assert_eq!(out.status.success(), choice == "accepted", "{}", says(&out));
            let before = world.get("native-before.json");
            let intent = &before[0]["intent"];
            assert_eq!(intent["text"], "Thanks");
            assert_eq!(intent["status"], "pending");
            assert_eq!(intent["reconciliation"], false);
            assert_eq!(intent["request"], Value::Null);
            assert_eq!(intent["binding"]["target"]["subject_id"], "NODE_H");
            assert!(
                world.requests().is_empty(),
                "native write never reaches external reply"
            );
            if choice != "accepted" {
                let row = world.thread();
                assert_eq!(row["pending_reply"]["retry"], false);
                assert_eq!(row["pending_reply"]["status"], "held");
                assert!(!world.reply(None).status.success());
                world.flag("absent");
                world.refresh();
                // Original native holds can be checked locally without any
                // remaining source configuration.
                world
                    .world
                    .configure(json!({"projects":{"demo":{"providers":[]}}}));
                world.ok(&["reply", ITEM, "--resolve", choice]);
                assert_eq!(saved(&world).1["intent"]["status"], choice);
                if choice == "not-sent" {
                    if let Some(path) = draft {
                        assert_eq!(fs::read_to_string(path).unwrap(), "Thanks");
                    }
                }
            }
            assert_eq!(
                fs::read_to_string(world.world.path().join("mail/native-calls"))
                    .unwrap()
                    .lines()
                    .count(),
                1
            );
        }
    }
}

#[test]
fn native_initial_save_failure_prevents_invocation() {
    let world = MailWorld::new(false);
    let mut row = world.get("conversation.json");
    row["threads"][0]["reply"] = json!({"provider":"github","subject_id":"NODE_H"});
    world.put("conversation.json", row);
    world.refresh();
    world.world.stub(
        "gh",
        "#!/bin/sh\nprintf 'called' > \"$REPLY_WORLD/mail/native-called\"\nprintf '{}'\n",
    );
    fs::write(world.world.path().join("state/ephor/replies"), "obstructed").unwrap();
    assert!(!world.reply(Some("Thanks")).status.success());
    assert!(!world.world.path().join("mail/native-called").exists());
}

#[test]
fn readonly_and_legacy_unbound_drafts_remain_editable_copyable_and_withdrawable() {
    for unbound in [false, true] {
        let world = MailWorld::new(false);
        if !unbound {
            let mut row = world.get("conversation.json");
            row["threads"][0].as_object_mut().unwrap().remove("reply");
            world.put("conversation.json", row);
            world.refresh();
        }
        let path = world.drafted("Copy me");
        if unbound {
            let ledger_path = world.world.path().join("state/ephor/work.json");
            let mut ledger = read_json(&ledger_path);
            ledger["entries"][ITEM]["dispatches"][0]
                .as_object_mut()
                .unwrap()
                .remove("reply_binding");
            write_json(&ledger_path, &ledger);
        }
        let view = world.thread();
        assert_eq!(view["draft"]["sendable"], false);
        assert_eq!(view["draft"]["text"], "Copy me");
        assert_eq!(fs::read_to_string(&path).unwrap(), "Copy me");
        fs::write(&path, "Edited copy").unwrap();
        assert_eq!(world.thread()["draft"]["text"], "Edited copy");
        assert!(!world.reply(None).status.success());
        fs::write(&path, " ").unwrap();
        assert!(world.thread().get("draft").is_none());
        assert!(world.requests().is_empty());
    }
}

#[test]
fn released_draft_future_send_still_checks_unbound_source_and_current_capability() {
    for change in ["unbound", "account", "readonly"] {
        let world = MailWorld::new(true);
        let path = held(&world, true, true).unwrap();
        world.ok(&["reply", ITEM, "--resolve", "not-sent"]);
        match change {
            "unbound" => {
                let ledger_path = world.world.path().join("state/ephor/work.json");
                let mut ledger = read_json(&ledger_path);
                ledger["entries"][ITEM]["dispatches"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("reply_binding");
                write_json(&ledger_path, &ledger);
            }
            "account" => world.world.configure(
                json!({"projects":{"demo":{"providers":[{"provider":"mail-me","user":"other"}]}}}),
            ),
            _ => world.put(
                "capabilities.json",
                json!({"messages":true,"replies":false,"reply_reconciliation":true}),
            ),
        }
        let out = world.reply(None);
        assert!(!out.status.success(), "{change}: {}", says(&out));
        assert_eq!(world.requests().len(), 1);
        assert_eq!(world.delivered(), 0);
        assert_eq!(fs::read_to_string(path).unwrap(), "Thanks");
    }
}

#[test]
fn finished_work_reopens_with_new_binding_and_its_own_advertised_output() {
    let world = MailWorld::new(false);
    let old = world.drafted("Old answer");
    let ledger_path = world.world.path().join("state/ephor/work.json");
    let ledger = read_json(&ledger_path);
    let plan = ledger["entries"][ITEM]["plan"].as_str().unwrap();
    let before = fs::read_to_string(plan).unwrap();
    let done = before
        .lines()
        .map(|line| {
            if line.starts_with("**State:**") {
                "**State:** done"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(plan, done + "\n").unwrap();
    world.incoming();
    world.refresh();
    world.ok(&["work", "sync", "--item", ITEM]);
    let path = world.requested_reply();
    assert_ne!(path, old);
    let ledger = read_json(&ledger_path);
    let latest = ledger["entries"][ITEM]["dispatches"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(
        latest["reply_binding"]["target"],
        json!({"in_reply_to":reply_world::M})
    );
    assert_eq!(latest["reply_binding"]["path"], json!(path));
    assert!(world.thread().get("draft").is_none());
    fs::write(path, "New answer").unwrap();
    assert_eq!(world.thread()["draft"]["text"], "New answer");
    assert_eq!(fs::read_to_string(old).unwrap(), "Old answer");
}

#[test]
fn saved_site_context_is_used_when_feed_placement_moves_to_a_different_project() {
    for choice in ["replay", "sent", "not-sent"] {
        let world = MailWorld::new(true);
        world.world.configure(json!({"defaults":{"provider_timeout_seconds":1},"sources":[{"provider":"mail-me","user":"me"}],"projects":{"demo":{"providers":[]},"other":{"providers":[{"provider":"mail-me","user":"another-account"}]}}}));
        held(&world, false, choice != "replay");
        assert_eq!(world.requests()[0]["project"], "");
        let old = world.world.path().join("state/ephor/feed/demo.json");
        let new = world.world.path().join("state/ephor/feed/other.json");
        let mut feed = read_json(&old);
        feed["project"] = json!("other");
        for slot in feed["providers"].as_object_mut().unwrap().values_mut() {
            for matter in slot["matters"].as_array_mut().unwrap() {
                matter["placement"] = json!({"on":{"project":"other"}});
            }
        }
        write_json(&new, &feed);
        fs::remove_file(old).unwrap();
        assert_eq!(world.thread()["project"], "other");
        if choice == "replay" {
            world.ok(&["reply", ITEM, "Thanks"]);
            assert_eq!(world.wires()[0], world.wires()[1]);
            assert_eq!(world.requests()[1]["project"], "");
            assert_eq!(world.delivered(), 1);
        } else {
            world.ok(&["reply", ITEM, "--resolve", choice]);
            assert_eq!(world.requests().len(), 1);
            assert_eq!(saved(&world).1["intent"]["status"], choice);
        }
    }
}
