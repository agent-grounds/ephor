//! E2E-052: a proposal stays bound, and recovery finishes the saved send
//! (§FS-005-dispatch.4, §FS-005-dispatch.13). Adapted from ephor.34's mail
//! reproducer, with stricter stale refusal and an opt-in reconciling fixture
//! (§FS-001-forge-interface.1, §FS-001-forge-interface.2).
//! All moves use fresh CLI processes. Thread and dry-run checks hold the
//! machine and human readings to §FS-011-command-line.4 and
//! §FS-011-command-line.7; routing remains §FS-001-forge-interface.9.

#[path = "../reply_world.rs"]
mod reply_world;
#[path = "../support.rs"]
mod support;

#[path = "../reply_fix_cases.rs"]
mod fix_cases;

#[path = "../reply_evidence_cases.rs"]
mod evidence_cases;

#[path = "../reply_review_cases.rs"]
mod review_cases;

use std::fs;
use std::process::Stdio;
use std::time::{Duration, Instant};

use reply_world::{contains, message, says, tree, MailWorld, H, ITEM, NEW_WORDS};
use serde_json::{json, Value};
use support::{read_json, write_json};

fn stale(world: &MailWorld, args: &[&str], advancing: &str) {
    let before = world.requests().len();
    let out = world.run(args);
    assert!(
        !out.status.success(),
        "stale draft must refuse, got: {}",
        says(&out)
    );
    let reason = says(&out);
    assert!(
        reason.to_lowercase().contains("stale"),
        "must explain staleness: {reason}"
    );
    assert!(
        reason.contains(advancing),
        "must name the advancing message/send {advancing:?}: {reason}"
    );
    assert_eq!(
        world.requests().len(),
        before,
        "stale refusal must call nothing"
    );
}

#[test]
fn h_draft_after_m_refuses_names_m_and_calls_nothing() {
    let world = MailWorld::new(true);
    world.drafted("I accept €100 and can start on Monday.");
    world.incoming();
    world.refresh();
    stale(&world, &["reply", ITEM], NEW_WORDS);
}

fn recovered(drafted: bool, new_mail: bool) {
    let world = MailWorld::new(true);
    if drafted {
        world.drafted("Thanks");
    }
    let words = if drafted { None } else { Some("  Thanks\n") };
    world.uncertain(words);
    if new_mail {
        world.incoming();
    }
    world.refresh();
    let out = world.reply(words);
    assert!(
        out.status.success(),
        "saved replay must recover acceptance: {}",
        says(&out)
    );
    let requests = world.requests();
    assert_eq!(requests.len(), 2, "exactly one explicit recovery call");
    assert_eq!(
        requests[0], requests[1],
        "recovery must reuse original target and exact prepared payload"
    );
    assert_eq!(requests[1]["target"], json!({"in_reply_to": H}));
    assert_eq!(requests[1]["text"], "Thanks");
    assert_eq!(
        world.wires()[0],
        world.wires()[1],
        "recovery request is byte-identical at the carrier"
    );
    assert_eq!(
        world.delivered(),
        1,
        "reconciliation must return prior acceptance with one delivery"
    );
    if drafted {
        assert!(
            world.thread()["draft"].is_null(),
            "recovered draft must retire"
        );
    }
}

#[test]
fn drafted_lost_ack_replays_saved_request_after_refresh_and_restart() {
    recovered(true, false);
}
#[test]
fn typed_lost_ack_replays_saved_request_after_refresh_and_restart() {
    recovered(false, false);
}
#[test]
fn drafted_lost_ack_replays_before_freshness_even_after_new_mail() {
    recovered(true, true);
}
#[test]
fn typed_lost_ack_replays_original_target_even_after_new_mail() {
    recovered(false, true);
}

fn intentional_repeat(drafted: bool) {
    let world = MailWorld::new(true);
    if drafted {
        world.drafted("Thanks");
    }
    let first = world.reply(if drafted { None } else { Some("Thanks") });
    assert!(first.status.success(), "{}", says(&first));
    world.refresh();
    let second = world.reply(Some("Thanks"));
    assert!(
        second.status.success(),
        "intentional repeat must start new send: {}",
        says(&second)
    );
    let requests = world.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["target"], json!({"in_reply_to": H}));
    assert_eq!(
        requests[1]["target"],
        json!({"in_reply_to": "<s1@me.example>"})
    );
    assert_eq!(world.delivered(), 2);
}

#[test]
fn control_acknowledged_draft_then_intentional_repeat_delivers_twice() {
    intentional_repeat(true);
}
#[test]
fn control_acknowledged_typed_intentional_repeat_delivers_twice() {
    intentional_repeat(false);
}

fn legacy_hold(drafted: bool) {
    let world = MailWorld::new(false);
    if drafted {
        world.drafted("Thanks");
    }
    let words = if drafted { None } else { Some("Thanks") };
    world.uncertain(words);
    world.refresh();
    let out = world.reply(words);
    assert!(
        !out.status.success(),
        "nondeclaring uncertain send must hold: {}",
        says(&out)
    );
    assert_eq!(
        world.requests().len(),
        1,
        "held retry must make zero further calls"
    );
    assert!(
        says(&out).contains("resolve"),
        "held send must explain channel check and resolution: {}",
        says(&out)
    );
}

#[test]
fn legacy_drafted_lost_ack_holds_without_a_retry_call() {
    legacy_hold(true);
}
#[test]
fn legacy_typed_lost_ack_holds_without_a_retry_call() {
    legacy_hold(false);
}

#[test]
fn explicit_unknown_on_declaring_forge_remains_held_after_restart() {
    let world = MailWorld::new(true);
    world.flag("unknown");
    let first = world.reply(Some("Thanks"));
    world.refresh();
    let retry = world.reply(Some("Thanks"));
    assert_eq!(
        world.requests().len(),
        1,
        "explicit unknown must remain held after restart, with zero retry calls"
    );
    assert!(
        !first.status.success(),
        "explicit unknown must not be reported as posted: {}",
        says(&first)
    );
    assert!(
        says(&first).contains("delivery unknown"),
        "carrier note must be shown"
    );
    assert!(
        !retry.status.success(),
        "explicit unknown must hold after restart"
    );
    assert_eq!(
        world.requests().len(),
        1,
        "unknown is not eligible for automatic saved replay"
    );
}

fn saved_before_call(drafted: bool) {
    let world = MailWorld::new(false);
    if drafted {
        world.drafted("Thanks");
    }
    let out = world.reply(if drafted { None } else { Some("  Thanks\n") });
    assert!(out.status.success(), "fixture send: {}", says(&out));
    let before = world.lines("before.jsonl");
    let records = &before[0]["records"];
    assert!(
        !records.as_object().unwrap().is_empty(),
        "durable site reply record must exist before forge invocation: {records}"
    );
    assert!(
        contains(records, &json!({"in_reply_to": H})),
        "saved original target: {records}"
    );
    assert!(
        contains(records, &json!("Thanks")),
        "saved exact prepared words: {records}"
    );
    assert!(
        contains(records, &json!(ITEM)),
        "saved original row: {records}"
    );
    assert!(
        contains(records, &json!("mail-me")),
        "saved original source: {records}"
    );
    assert!(
        contains(records, &json!("demo")),
        "saved project context: {records}"
    );
}

#[test]
fn drafted_send_is_saved_durably_before_forge_invocation() {
    saved_before_call(true);
}
#[test]
fn typed_send_is_saved_durably_before_forge_invocation() {
    saved_before_call(false);
}

#[test]
fn persistence_failure_prevents_any_forge_call() {
    let world = MailWorld::new(false);
    // A deterministic filesystem obstruction, including when tests run as root.
    fs::write(
        world.world.path().join("state/ephor/replies"),
        "not a directory",
    )
    .unwrap();
    let out = world.reply(Some("Thanks"));
    assert!(
        !out.status.success(),
        "saving failure must refuse: {}",
        says(&out)
    );
    assert_eq!(
        world.requests().len(),
        0,
        "cannot send without saving first"
    );
}

#[test]
fn drafted_persistence_failure_keeps_words_and_prevents_call() {
    let world = MailWorld::new(false);
    let draft = world.drafted("Thanks");
    fs::remove_dir_all(world.world.path().join("state/ephor/replies")).unwrap();
    fs::write(
        world.world.path().join("state/ephor/replies"),
        "not a directory",
    )
    .unwrap();
    let out = world.reply(None);
    assert!(
        !out.status.success(),
        "saving failure must refuse drafted send: {}",
        says(&out)
    );
    assert_eq!(world.requests().len(), 0);
    assert_eq!(fs::read_to_string(draft).unwrap(), "Thanks");
}

#[test]
fn handoff_records_binding_and_refresh_never_replaces_it() {
    let world = MailWorld::new(false);
    let draft = world.drafted("Thanks");
    let ledger_path = world.world.path().join("state/ephor/work.json");
    let ledger = read_json(&ledger_path);
    let dispatch = &ledger["entries"][ITEM]["dispatches"][0];
    let binding = &dispatch["reply_binding"];
    assert!(
        binding.is_object(),
        "hand-off must save per-request reply_binding: {dispatch}"
    );
    assert!(
        contains(binding, &json!({"in_reply_to": H})),
        "binding keeps opaque H target"
    );
    assert!(
        contains(binding, &json!(ITEM)) && contains(binding, &json!("mail-me")),
        "binding keeps row/source"
    );
    assert!(
        contains(dispatch, &json!(draft)),
        "request keeps the actual reply path"
    );
    world.incoming();
    world.refresh();
    let refreshed = read_json(&ledger_path);
    assert_eq!(
        &refreshed["entries"][ITEM]["dispatches"][0]["reply_binding"], binding,
        "refresh must never replace a saved binding"
    );
}

fn own_send_stales(refresh: bool) {
    let world = MailWorld::new(false);
    world.drafted("Old answer");
    let sent = world.reply(Some("My newer answer"));
    assert!(sent.status.success(), "{}", says(&sent));
    if refresh {
        world.refresh();
    }
    stale(&world, &["reply", ITEM], "My newer answer");
}

#[test]
fn own_accepted_send_stales_draft_before_refresh() {
    own_send_stales(false);
}
#[test]
fn own_accepted_send_stales_draft_after_refresh() {
    own_send_stales(true);
}

#[test]
fn acknowledged_repeat_with_unchanged_descriptor_requires_refresh() {
    let world = MailWorld::new(true);
    assert!(world.reply(Some("Thanks")).status.success());
    let repeat = world.reply(Some("Thanks"));
    assert!(
        !repeat.status.success(),
        "intentional repeat with confirmed descriptor must refuse: {}",
        says(&repeat)
    );
    assert!(
        says(&repeat).to_lowercase().contains("refresh"),
        "must ask for refresh"
    );
    assert_eq!(
        world.requests().len(),
        1,
        "do not silently reconcile an intentional repeat"
    );
}

#[test]
fn changed_typed_words_cannot_replace_pending_payload() {
    let world = MailWorld::new(true);
    world.uncertain(Some("Thanks"));
    world.refresh();
    let changed = world.reply(Some("Actually, no"));
    assert!(
        !changed.status.success(),
        "changed pending words must refuse: {}",
        says(&changed)
    );
    assert_eq!(world.requests().len(), 1, "changed words must call nothing");
}

#[test]
fn edited_draft_cannot_change_saved_recovery_words() {
    let world = MailWorld::new(true);
    let path = world.drafted("Thanks");
    world.uncertain(None);
    fs::write(path, "Actually, no").unwrap();
    world.incoming();
    world.refresh();
    let out = world.reply(None);
    assert!(
        out.status.success(),
        "saved draft replay must recover: {}",
        says(&out)
    );
    let requests = world.requests();
    assert_eq!(
        requests[0], requests[1],
        "editing the file must not edit the pending send"
    );
    assert_eq!(world.delivered(), 1);
}

#[test]
fn changed_account_cannot_reroute_pending_recovery() {
    let world = MailWorld::new(true);
    world.uncertain(Some("Thanks"));
    let mut config = read_json(&world.world.config_path());
    config["projects"]["demo"]["providers"][0]["user"] = json!("somebody-else");
    write_json(&world.world.config_path(), &config);
    world.refresh();
    let out = world.reply(Some("Thanks"));
    assert!(
        !out.status.success(),
        "changed binding must refuse recovery: {}",
        says(&out)
    );
    assert_eq!(world.requests().len(), 1, "no send through changed account");
}

#[test]
fn withdrawn_reconciliation_declaration_cannot_replay() {
    let world = MailWorld::new(true);
    world.uncertain(Some("Thanks"));
    world.put(
        "capabilities.json",
        json!({"messages": true, "replies": true}),
    );
    world.refresh();
    let out = world.reply(Some("Thanks"));
    assert!(
        !out.status.success(),
        "current declaration required: {}",
        says(&out)
    );
    assert_eq!(world.requests().len(), 1);
}

#[test]
fn changed_project_to_site_context_cannot_reroute_recovery() {
    let world = MailWorld::new(true);
    world.uncertain(Some("Thanks"));
    let mut config = read_json(&world.world.config_path());
    config["sources"] = config["projects"]["demo"]["providers"].clone();
    config["projects"]["demo"]["providers"] = json!([]);
    write_json(&world.world.config_path(), &config);
    world.refresh();
    let out = world.reply(Some("Thanks"));
    assert!(
        !out.status.success(),
        "original source scope/context required: {}",
        says(&out)
    );
    assert_eq!(
        world.requests().len(),
        1,
        "must not recover through a replacement site binding"
    );
}

#[test]
fn control_site_source_send_receives_no_project() {
    let world = MailWorld::new(false);
    let mut config = read_json(&world.world.config_path());
    config["sources"] = config["projects"]["demo"]["providers"].clone();
    config["projects"]["demo"]["providers"] = json!([]);
    write_json(&world.world.config_path(), &config);
    world.refresh();
    let out = world.reply(Some("Thanks"));
    assert!(
        out.status.success(),
        "site-source move works: {}",
        says(&out)
    );
    assert!(
        world.requests()[0]["project"]
            .as_str()
            .unwrap_or_default()
            .is_empty(),
        "site source receives no project"
    );
}

#[test]
fn later_declaration_does_not_make_legacy_uncertainty_replayable() {
    let world = MailWorld::new(false);
    world.uncertain(Some("Thanks"));
    world.put(
        "capabilities.json",
        json!({"messages": true, "replies": true, "reply_reconciliation": true}),
    );
    world.refresh();
    let out = world.reply(Some("Thanks"));
    assert!(
        !out.status.success(),
        "original eligibility required: {}",
        says(&out)
    );
    assert_eq!(world.requests().len(), 1);
}

#[test]
fn forgetting_work_cannot_erase_pending_draft_recovery() {
    let world = MailWorld::new(true);
    world.drafted("Thanks");
    world.uncertain(None);
    world.ok(&["work", "forget", "--item", ITEM]);
    world.incoming();
    world.refresh();
    let out = world.reply(None);
    assert!(
        out.status.success(),
        "recovery must outlive work.json: {}",
        says(&out)
    );
    assert_eq!(world.requests()[0], world.requests()[1]);
    assert_eq!(world.delivered(), 1);
}

#[test]
fn pending_recovery_can_address_row_absent_from_feed() {
    let world = MailWorld::new(true);
    world.uncertain(Some("Thanks"));
    world.flag("absent");
    world.refresh();
    let out = world.reply(Some("Thanks"));
    assert!(
        out.status.success(),
        "saved row remains addressable: {}",
        says(&out)
    );
    assert_eq!(world.requests()[0], world.requests()[1]);
    assert_eq!(world.delivered(), 1);
}

#[test]
fn legacy_unbound_draft_is_readable_but_cannot_post() {
    let world = MailWorld::new(false);
    world.drafted("Legacy words");
    let path = world.world.path().join("state/ephor/work.json");
    let mut ledger = read_json(&path);
    fn unbind(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.remove("reply_binding");
                map.remove("reply_path");
                for value in map.values_mut() {
                    unbind(value);
                }
            }
            Value::Array(values) => {
                for value in values {
                    unbind(value);
                }
            }
            _ => {}
        }
    }
    unbind(&mut ledger);
    let entry = &ledger["entries"][ITEM];
    let legacy_path = std::path::Path::new(entry["root"].as_str().unwrap())
        .join("runtime/ephor")
        .join(format!("{}.reply.md", entry["plan_id"].as_str().unwrap()));
    fs::write(legacy_path, "Legacy words").unwrap();
    write_json(&path, &ledger);
    assert_eq!(world.thread()["draft"]["text"], "Legacy words");
    let out = world.reply(None);
    assert!(
        !out.status.success(),
        "unbound draft must refuse: {}",
        says(&out)
    );
    assert_eq!(world.requests().len(), 0);
}

#[test]
fn late_older_output_has_its_own_path_and_cannot_replace_newer_words() {
    let world = MailWorld::new(false);
    let old = world.drafted("Old words");
    world.incoming();
    world.refresh();
    let new = world.drafted("New words");
    assert_ne!(old, new, "each request needs its own reply path");
    fs::write(old, "Late old words").unwrap();
    assert_eq!(world.thread()["draft"]["text"], "New words");
}

#[test]
fn latest_request_supersedes_old_proposal_before_output_exists() {
    let world = MailWorld::new(false);
    world.drafted("Old words");
    world.ok(&[
        "work", "dispatch", "--item", ITEM, "--recipe", "answer", "--again",
    ]);
    assert!(
        world.thread()["draft"].is_null(),
        "new request must suppress older output even before new output"
    );
}

#[test]
fn control_unrelated_thread_and_reactions_do_not_stale_bound_draft() {
    let world = MailWorld::new(false);
    let mut row = world.get("conversation.json");
    let other = json!({"messages": [message("<other>", "Unrelated thread", false)], "reply": {"in_reply_to": "<other>"}});
    row["threads"].as_array_mut().unwrap().insert(0, other);
    world.put("conversation.json", row);
    world.refresh();
    world.drafted("Thanks"); // Bound to H in the last shown sendable thread.
    let mut row = world.get("conversation.json");
    row["threads"][0]["messages"]
        .as_array_mut()
        .unwrap()
        .push(message("<other2>", "Unrelated advance", false));
    row["threads"][0]["reply"] = json!({"in_reply_to": "<other2>"});
    row["threads"][1]["messages"][0]["reactions"] =
        json!([{"emoji": "thumbs-up", "users": ["me"]}]);
    world.put("conversation.json", row);
    world.refresh();
    let out = world.reply(None);
    assert!(
        out.status.success(),
        "unrelated advance must not stale H: {}",
        says(&out)
    );
    assert_eq!(world.requests()[0]["target"], json!({"in_reply_to": H}));
}

fn identity_refuses(change: &str) {
    let world = MailWorld::new(false);
    if change == "reordered" {
        world.incoming();
        world.refresh();
    }
    world.drafted("Thanks");
    let mut row = world.get("conversation.json");
    match change {
        "ambiguous" => {
            let copy = row["threads"][0].clone();
            row["threads"].as_array_mut().unwrap().push(copy);
        }
        "missing" => {
            row["threads"][0]["messages"] =
                json!([message("<other>", "Replaced conversation", false)]);
        }
        "reordered" => {
            row["threads"][0]["messages"]
                .as_array_mut()
                .unwrap()
                .reverse();
        }
        _ => unreachable!(),
    }
    world.put("conversation.json", row);
    world.refresh();
    let out = world.reply(None);
    assert!(
        !out.status.success(),
        "{change} thread identity must refuse safely: {}",
        says(&out)
    );
    assert_eq!(world.requests().len(), 0);
}

#[test]
fn ambiguous_thread_identity_refuses_safely() {
    identity_refuses("ambiguous");
}
#[test]
fn missing_thread_identity_refuses_safely() {
    identity_refuses("missing");
}
#[test]
fn reordered_thread_identity_refuses_safely() {
    identity_refuses("reordered");
}

#[test]
fn crash_after_remote_acceptance_before_confirmation_recovers_saved_send() {
    let world = MailWorld::new(true);
    world.drafted("Thanks");
    world.flag("crash-caller");
    let out = world.reply(None);
    assert!(
        !out.status.success(),
        "fixture kills ephor before confirmation"
    );
    assert_eq!(world.delivered(), 1);
    world.incoming();
    world.refresh();
    let replay = world.reply(None);
    assert!(replay.status.success(), "{}", says(&replay));
    assert_eq!(
        world.requests()[0],
        world.requests()[1],
        "crash recovery must use saved request"
    );
    assert_eq!(world.delivered(), 1);
}

fn retirement_failure(later_send: bool) {
    let world = MailWorld::new(false);
    let path = world.drafted("Thanks");
    // Block every possible posted-file spelling with a directory. The exact
    // retirement filename is internal; confirmed suppression is observable.
    for entry in [path.clone(), world.requested_reply()] {
        fs::create_dir_all(entry.with_extension("posted.md")).unwrap();
    }
    let out = world.reply(None);
    assert!(
        out.status.success(),
        "confirmed remote acceptance: {}",
        says(&out)
    );
    assert!(path.exists(), "fixture must actually obstruct retirement");
    world.refresh();
    if later_send {
        assert!(world.reply(Some("Later intentional send")).status.success());
        world.refresh();
    }
    let before = world.requests().len();
    let retry = world.reply(None);
    assert!(
        !retry.status.success(),
        "confirmed draft must not repost after retirement failure: {}",
        says(&retry)
    );
    assert_eq!(
        world.requests().len(),
        before,
        "confirmation suppresses old draft across later intents"
    );
    assert!(
        world.thread()["draft"].is_null() || world.thread()["draft"]["sendable"] == false,
        "confirmed draft must not be offered for posting"
    );
}

#[test]
fn confirmation_suppresses_draft_when_retirement_fails() {
    retirement_failure(false);
}
#[test]
fn confirmed_draft_suppression_survives_later_intents() {
    retirement_failure(true);
}

#[test]
fn row_lock_refuses_competing_send_while_confirmation_is_pending() {
    let world = MailWorld::new(false);
    let mut config = read_json(&world.world.config_path());
    config["defaults"]["provider_timeout_seconds"] = json!(8);
    write_json(&world.world.config_path(), &config);
    world.flag("block");
    let mut first = world
        .command()
        .args(["reply", ITEM, "Thanks"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let entered = world.world.path().join("mail/entered");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !entered.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !entered.exists() {
        let _ = first.kill();
        let _ = first.wait();
        panic!("fixture never entered send");
    }
    let second = world.reply(Some("Thanks"));
    fs::remove_file(world.world.path().join("mail/block")).unwrap();
    let first_status = first.wait().unwrap();
    assert!(first_status.success(), "first send must finish");
    assert!(!second.status.success(), "competing send must refuse");
    assert_eq!(
        world.requests().len(),
        1,
        "row lock prevents second invocation"
    );
}

#[test]
fn dry_run_stale_refuses_with_same_reason_and_changes_nothing() {
    let world = MailWorld::new(false);
    world.drafted("I accept €100");
    world.incoming();
    world.refresh();
    let before = tree(world.world.path());
    stale(&world, &["reply", ITEM, "--dry-run"], NEW_WORDS);
    assert_eq!(
        tree(world.world.path()),
        before,
        "rehearsal writes no files, including locks"
    );
}

#[test]
fn dry_run_replay_shows_original_target_and_saved_words_without_writing() {
    let world = MailWorld::new(true);
    let path = world.drafted("Thanks");
    world.uncertain(None);
    fs::write(path, "Changed file").unwrap();
    world.incoming();
    world.refresh();
    let before = tree(world.world.path());
    let out = world.ok(&["reply", ITEM, "--dry-run"]);
    assert!(
        says(&out).contains(H),
        "rehearsal must show actual saved target: {}",
        says(&out)
    );
    assert!(
        says(&out).contains("Thanks") && !says(&out).contains("Changed file"),
        "rehearsal uses exact pending words"
    );
    assert_eq!(
        tree(world.world.path()),
        before,
        "dry replay writes nothing"
    );
}

#[test]
fn control_fresh_typed_dry_run_is_immutable_and_never_calls_reply() {
    let world = MailWorld::new(false);
    let before = tree(world.world.path());
    world.ok(&["reply", ITEM, "Thanks", "--dry-run"]);
    assert_eq!(tree(world.world.path()), before);
    assert_eq!(world.requests().len(), 0);
}

#[test]
fn fresh_dry_run_displays_actual_opaque_target() {
    let world = MailWorld::new(false);
    let out = world.ok(&["reply", ITEM, "Thanks", "--dry-run"]);
    assert!(
        says(&out).contains(H),
        "rehearsal must show actual target: {}",
        says(&out)
    );
}

#[test]
fn thread_json_preserves_bound_target_and_marks_stale() {
    let world = MailWorld::new(false);
    world.drafted("I accept €100");
    world.incoming();
    world.refresh();
    let thread = world.thread();
    assert!(
        contains(&thread["draft"], &json!({"in_reply_to": H})),
        "thread must show saved target, not M: {thread}"
    );
    assert_eq!(
        thread["draft"]["sendable"], false,
        "stale draft cannot offer posting"
    );
    assert!(
        thread["draft"].to_string().contains(NEW_WORDS),
        "stale reason names M"
    );
}

#[test]
fn thread_reading_shows_pending_words_and_saved_retry_after_draft_edit() {
    let world = MailWorld::new(true);
    let path = world.drafted("Thanks");
    world.uncertain(None);
    fs::write(path, "Edited unsent words").unwrap();
    world.incoming();
    world.refresh();
    let out = world.ok(&["thread", ITEM]);
    let reading = says(&out);
    assert!(
        reading.contains(H),
        "pending reading must show saved target: {reading}"
    );
    assert!(
        reading.contains("Thanks"),
        "pending prepared words stay visible: {reading}"
    );
    assert!(
        reading.to_lowercase().contains("retry saved send"),
        "offer must distinguish saved recovery: {reading}"
    );
}

#[test]
fn thread_reading_names_held_outcome_and_resolution() {
    let world = MailWorld::new(false);
    world.uncertain(Some("Thanks"));
    world.refresh();
    let out = world.ok(&["thread", ITEM]);
    let reading = says(&out);
    assert!(
        reading.contains("Thanks") && reading.to_lowercase().contains("resolve"),
        "held words and channel-check resolution must be visible: {reading}"
    );
}

#[test]
fn capability_default_false_is_published_in_both_transports() {
    let legacy: ephor::forge::Capabilities =
        serde_json::from_value(json!({"replies": true})).unwrap();
    assert_eq!(
        serde_json::to_value(legacy).unwrap()["reply_reconciliation"],
        false,
        "legacy omission must expose default-false reconciliation"
    );
}

#[test]
fn capability_roundtrip_keeps_reconciliation_opt_in_and_default_false() {
    let capabilities: ephor::forge::Capabilities =
        serde_json::from_value(json!({"replies": true, "reply_reconciliation": true})).unwrap();
    let encoded = serde_json::to_value(capabilities).unwrap();
    assert_eq!(
        encoded["reply_reconciliation"], true,
        "both transports must retain declaration"
    );
    let legacy: ephor::forge::Capabilities =
        serde_json::from_value(json!({"replies": true})).unwrap();
    assert_eq!(
        serde_json::to_value(legacy).unwrap()["reply_reconciliation"],
        false,
        "legacy defaults false"
    );
}

#[test]
fn forge_schema_publishes_reconciliation_default_and_typed_outcomes() {
    let world = MailWorld::new(false);
    let out = world.ok(&["schema", "forge"]);
    let schema = support::json_of(&out);
    let declaration =
        &schema["$defs"]["capabilities_response"]["properties"]["reply_reconciliation"];
    assert_eq!(
        declaration["type"], "boolean",
        "schema must declare reply_reconciliation"
    );
    assert_eq!(declaration["default"], false);
    let response = schema["$defs"]["reply_response"].to_string();
    assert!(
        response.contains("accepted") && response.contains("unknown"),
        "schema must publish typed reply outcomes: {response}"
    );
}
