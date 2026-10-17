//! A send the carrier definitely refused is ended, not held
//! (§FS-005-dispatch.13.3). The carrier says so with the typed `refused`
//! outcome (§FS-001-forge-interface.2), and its note is the move's answer in
//! prose and under `--json` (§FS-011-command-line.7). Nothing is delivered or
//! confirmed, a refused descriptor stays unused (§FS-001-forge-interface.1),
//! and the next send is a new move under the ordinary checks. Ported from the
//! ephor.44 reproducer: scenario B is the typed refusal, and scenario A, an
//! exit code alone, stays pending. Every case counts the carrier's own
//! delivery ledger rather than trusting what ephor says about it.

use super::*;
use support::shaped;

const STALE: &str = "descriptor is no longer fresh";
const AT_FRESH: &str = "the recipient's server rejected the message";
const FIRST: &str = "Yes, €100 works.";
const OTHER: &str = "Sorry, €150 is too much for me.";

fn saved(world: &MailWorld) -> (std::path::PathBuf, Value) {
    let path = fs::read_dir(world.world.path().join("state/ephor/replies"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .expect("the send is saved before the carrier is called");
    let value = read_json(&path);
    (path, value)
}

fn reply(world: &MailWorld, words: Option<&str>, machine: bool) -> std::process::Output {
    let mut args = vec!["reply", ITEM];
    args.extend(words);
    if machine {
        args.push("--json");
    }
    world.run(&args)
}

/// The move failed, and its answer is the carrier's note, in prose or as an
/// `ok: false` outcome.
fn refused(out: &std::process::Output, note: &str, machine: bool) {
    let text = says(out);
    let expected = format!("Reply not sent: {note}");
    assert!(
        !out.status.success(),
        "a refused send is not a success: {text}"
    );
    if machine {
        let outcome = shaped("outcome", out);
        assert_eq!(outcome["ok"], false, "{outcome}");
        assert!(
            outcome["says"].as_str().unwrap().contains(&expected),
            "the outcome must carry {expected:?}: {outcome}"
        );
    } else {
        assert!(
            text.contains(&expected),
            "the answer must say {expected:?}: {text}"
        );
    }
}

/// The intent ended as not-sent with its note, and the row reads as it does
/// after `--resolve not-sent`: no saved send, nothing to retry or resolve, no
/// accepted send and no confirmed draft.
fn released(world: &MailWorld, note: &str) {
    let (_, record) = saved(world);
    assert_eq!(record["intent"]["status"], "not-sent", "{record}");
    assert_eq!(record["intent"]["note"], note, "{record}");
    assert_eq!(
        record["accepted"],
        json!({}),
        "a refusal advances no accepted-send generation: {record}"
    );
    assert_eq!(
        record["confirmed"],
        json!([]),
        "a refusal confirms no draft: {record}"
    );
    let thread = world.thread();
    assert!(
        thread.get("pending_reply").is_none(),
        "a released row has no saved send: {thread}"
    );
    let calls = world.requests().len();
    let resolve = world.run(&["reply", ITEM, "--resolve", "not-sent"]);
    assert!(
        !resolve.status.success(),
        "nothing is left to resolve: {}",
        says(&resolve)
    );
    assert_eq!(world.requests().len(), calls);
}

#[test]
fn a_refused_typed_send_is_released_with_its_note_and_other_words_go_out_once() {
    for machine in [false, true] {
        let world = MailWorld::new(true);
        world.flag("typed-refusal");
        // The gateway moved; ephor has not fetched it, so the send is stale.
        world.incoming();
        refused(&reply(&world, Some(FIRST), machine), STALE, machine);
        assert_eq!(world.requests().len(), 1);
        assert_eq!(world.delivered(), 0, "the carrier delivered nothing");
        released(&world, STALE);
        world.refresh();
        let out = world.reply(Some(OTHER));
        assert!(
            out.status.success(),
            "a released row takes other words: {}",
            says(&out)
        );
        assert_eq!(world.delivered(), 1, "the other words go out once");
        let last = world.requests().pop().unwrap();
        assert_eq!(last["text"], OTHER);
        assert_eq!(last["target"], json!({"in_reply_to": reply_world::M}));
    }
}

#[test]
fn a_refused_drafted_send_keeps_its_draft_unposted_and_its_thread_unadvanced() {
    for machine in [false, true] {
        let world = MailWorld::new(true);
        let draft = world.drafted(FIRST);
        world.flag("typed-refusal");
        world.incoming();
        refused(&reply(&world, None, machine), STALE, machine);
        assert_eq!(world.delivered(), 0, "the carrier delivered nothing");
        released(&world, STALE);
        assert_eq!(
            fs::read_to_string(&draft).unwrap(),
            FIRST,
            "the draft is neither retired nor moved aside as posted"
        );
        let thread = world.thread();
        assert_eq!(thread["draft"]["text"], FIRST, "{thread}");
        assert_eq!(
            thread["draft"]["sendable"], true,
            "until a refresh shows the conversation moved past it, the draft is still sendable: {thread}"
        );
        // Release supplies no freshness override.
        world.refresh();
        stale(&world, &["reply", ITEM], NEW_WORDS);
        let out = world.reply(Some(OTHER));
        assert!(out.status.success(), "{}", says(&out));
        assert_eq!(world.delivered(), 1, "the typed words go out once");
    }
}

#[test]
fn a_replay_the_upgraded_carrier_refuses_sends_the_saved_bytes_and_ends_the_send() {
    for legacy in [false, true] {
        let world = MailWorld::new(true);
        world.incoming();
        // Scenario A first: an exit code alone keeps the send pending.
        let first = world.reply(Some(FIRST));
        assert!(
            !first.status.success() && says(&first).contains(STALE),
            "{}",
            says(&first)
        );
        let (path, mut pending) = saved(&world);
        assert_eq!(pending["intent"]["status"], "pending");
        if legacy {
            // An older pending record: version 1, with no operation receipt.
            pending["version"] = json!(1);
            pending.as_object_mut().unwrap().remove("evidence");
            write_json(&path, &pending);
        }
        // The carrier is upgraded to say what it knew all along.
        world.flag("typed-refusal");
        world.refresh();
        refused(&world.reply(None), STALE, false);
        let wires = world.wires();
        assert_eq!(wires.len(), 2, "one replay");
        assert_eq!(
            wires[0], wires[1],
            "the replay sends the saved request byte for byte"
        );
        assert_eq!(world.delivered(), 0, "the carrier delivered nothing");
        released(&world, STALE);
        let out = world.reply(Some(OTHER));
        assert!(out.status.success(), "{}", says(&out));
        assert_eq!(world.delivered(), 1);
        assert_eq!(
            world.requests()[2]["target"],
            json!({"in_reply_to": reply_world::M})
        );
    }
}

#[test]
fn after_release_a_reply_without_words_never_replays_the_ended_send() {
    // Typed, with no draft: there is nothing to send.
    let world = MailWorld::new(true);
    world.flag("typed-refusal");
    world.incoming();
    refused(&world.reply(Some(FIRST)), STALE, false);
    let out = world.reply(None);
    assert!(
        !out.status.success() && says(&out).contains("No reply has been drafted"),
        "with the send ended and no draft, there is nothing to send: {}",
        says(&out)
    );
    assert_eq!(world.requests().len(), 1, "the ended send is not replayed");

    // Drafted, refused at a still-fresh descriptor: a new intent from the draft.
    let world = MailWorld::new(true);
    let draft = world.drafted(FIRST);
    world.flag("refuse-once");
    refused(&world.reply(None), AT_FRESH, false);
    let (_, ended) = saved(&world);
    let out = world.reply(None);
    assert!(
        out.status.success(),
        "the fresh draft is sent as a new intent: {}",
        says(&out)
    );
    let (_, sent) = saved(&world);
    assert_eq!(sent["intent"]["status"], "sent", "{sent}");
    assert_ne!(
        sent["evidence"], ended["evidence"],
        "a new intent has an operation of its own, never the ended one's"
    );
    assert_eq!(world.requests().len(), 2);
    assert_eq!(world.delivered(), 1);
    assert!(!draft.exists(), "the accepted draft is retired");
}

#[test]
fn a_refused_descriptor_is_not_used_so_words_there_go_out_once() {
    let others = "Yes, €100 works, and I can start on Monday.";
    for words in [FIRST, others] {
        let world = MailWorld::new(true);
        world.flag("refuse-once");
        refused(&world.reply(Some(FIRST)), AT_FRESH, false);
        assert_eq!(world.delivered(), 0, "the carrier delivered nothing");
        released(&world, AT_FRESH);
        // No refresh: the same descriptor, which the refusal left unused.
        let out = world.reply(Some(words));
        assert!(
            out.status.success(),
            "neither a changed pending payload nor a repeat at a confirmed descriptor: {}",
            says(&out)
        );
        let requests = world.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["target"], json!({"in_reply_to": H}));
        assert_eq!(requests[1]["target"], requests[0]["target"]);
        assert_eq!(requests[1]["text"], words);
        assert_eq!(world.delivered(), 1, "delivered once");
    }
}

/// Scenario A of the reproducer, as it is today and stays: an exit code
/// proves nothing, so the send stays pending, replay is the way on, and
/// resolution stays held-only.
#[test]
fn a_carrier_that_refuses_only_with_an_exit_code_leaves_the_send_pending() {
    let world = MailWorld::new(true);
    world.incoming();
    let out = world.reply(Some(FIRST));
    assert!(
        !out.status.success() && says(&out).contains(STALE),
        "{}",
        says(&out)
    );
    assert!(
        !says(&out).contains("Reply not sent"),
        "an exit code is not a refusal: {}",
        says(&out)
    );
    assert_eq!(world.delivered(), 0);
    let (path, pending) = saved(&world);
    assert_eq!(pending["intent"]["status"], "pending");
    let thread = world.thread();
    assert_eq!(thread["pending_reply"]["status"], "pending", "{thread}");
    assert_eq!(thread["pending_reply"]["retry"], true, "{thread}");
    assert_eq!(
        thread["pending_reply"]["resolutions"],
        json!([]),
        "{thread}"
    );
    let resolve = world.run(&["reply", ITEM, "--resolve", "not-sent"]);
    assert!(
        !resolve.status.success() && says(&resolve).contains("Only a held outcome can be resolved"),
        "{}",
        says(&resolve)
    );
    world.refresh();
    let other = world.reply(Some(OTHER));
    assert!(
        !other.status.success() && says(&other).contains("Changed words cannot replace"),
        "{}",
        says(&other)
    );
    assert_eq!(read_json(&path), pending, "nothing rewrote the saved send");
    assert_eq!(world.requests().len(), 1);
}

#[test]
fn a_dry_run_neither_sends_nor_predicts_a_refusal_and_writes_nothing() {
    for drafted in [false, true] {
        let world = MailWorld::new(true);
        if drafted {
            world.drafted(FIRST);
        }
        world.flag("typed-refusal");
        world.flag("refuse-once");
        world.incoming();
        let state = tree(&world.world.path().join("state/ephor"));
        let gateway = tree(&world.world.path().join("mail"));
        let out = if drafted {
            world.ok(&["reply", ITEM, "--dry-run", "--json"])
        } else {
            world.ok(&["reply", ITEM, FIRST, "--dry-run", "--json"])
        };
        let outcome = shaped("outcome", &out);
        assert_eq!(outcome["ok"], true, "{outcome}");
        let text = outcome["says"].as_str().unwrap();
        assert!(text.contains("would send"), "{text}");
        assert!(
            !text.contains("not sent") && !text.contains(STALE) && !text.contains(AT_FRESH),
            "a rehearsal cannot know what the carrier would answer: {text}"
        );
        assert_eq!(tree(&world.world.path().join("state/ephor")), state);
        assert_eq!(
            tree(&world.world.path().join("mail")),
            gateway,
            "the carrier was asked to send nothing"
        );
        assert!(world.requests().is_empty());
        assert_eq!(world.delivered(), 0);
    }
}
