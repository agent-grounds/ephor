//! E2E-042-a-chat-conversation-reaches-the-feed: a conversation a chat gateway
//! heard is a row under the project whose room it is in, it waits on the reader
//! until they answer, and the answer goes back through the gateway that
//! reported it.
//!
//! Chat is one of the places people talk (§FS-001-forge-interface), and it had
//! no way in: the capability set had no row a gateway could answer with, and a
//! room was not a signal any project could claim. So a gateway that answered
//! `capabilities` with `messages` was asked nothing more, and a refresh that
//! placed nothing reported success.
//!
//! What this case holds ephor to. The gateway is the shipped
//! `config/chat-gateway.example.sh`, bound once for the site
//! (§FS-001-forge-interface.9) and reading a spool the case writes the way a
//! listener would. A conversation it reports whole is a row of its own, keyed
//! by the source and the id the gateway gave it, of the Messages kind
//! (§FS-001-forge-interface.1). It is placed by the room a project claims, at
//! the strength of a venue, whatever its messages mention
//! (§FS-008-attribution.3), and the registry row's word on rooms is final: a
//! row silent on them adopts the checkout's hint, a row that lists rooms —
//! even none — refuses it (§FS-008-attribution.1). A conversation no room
//! claims is placed by what it references, and lands in the bucket when that
//! is nothing (§FS-008-attribution.4). A reply goes to the gateway that
//! reported the row, with the thread's descriptor handed back as it was given,
//! and a dry run of it resolves the same gateway (§FS-001-forge-interface.9).
//!
//! The decision this realizes is §DF-003-chat-is-a-forge-capability.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};
use support::{shaped, write_json, World};

/// The gateway exactly as ephor ships it, installed as the forge `chatgw`.
const GATEWAY: &str = include_str!("../../../config/chat-gateway.example.sh");

/// The group a widget team talks in, as the gateway spells it.
const ROOM: &str = "whatsapp/acme#120363@g.us";
/// The row a conversation in that room is: the source, then the gateway's id.
const ITEM: &str = "chatgw:whatsapp/acme#120363@g.us";
/// Where a reply to that group goes, in the gateway's own words.
fn descriptor() -> Value {
    json!({ "chat": "120363@g.us" })
}

/// Two projects, `widget` and `gadget`, each with its own registry row, and a
/// chat gateway bound once for the site over a spool its listener keeps
/// current. `widget` and `gadget` carry whatever else of a row a case gives.
fn chat_world(widget: Value, gadget: Value) -> World {
    let world = World::new();
    world.stub("ephor-forge-chatgw", GATEWAY);
    let mut registry = world.registry_doc();
    let template = registry["projects"][0].clone();
    let row = |id: &str, display: &str, own: Value| {
        let mut row = template.clone();
        row["id"] = json!(id);
        row["display_name"] = json!(display);
        let root = world.path().join(id);
        fs::create_dir_all(&root).expect("the project's root");
        row["root"] = json!(root.to_string_lossy());
        for (key, value) in own.as_object().expect("a row is an object") {
            row[key.as_str()] = value.clone();
        }
        row
    };
    registry["projects"] = json!([
        row("widget", "Widget", widget),
        row("gadget", "Gadget", gadget)
    ]);
    write_json(&world.registry_path(), &registry);
    write_json(
        &world.config_path(),
        &json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "sources": [{
                "provider": "chatgw",
                "spool": spool(&world).to_string_lossy(),
                "account": "you",
                "max_age_seconds": 300
            }],
            "projects": {
                "widget": { "providers": [] },
                "gadget": { "providers": [] }
            }
        }),
    );
    hearing(&world);
    world
}

fn spool(world: &World) -> PathBuf {
    world.path().join("spool")
}

/// The listener's word that the chat network confirmed, just now, that it
/// is hearing it.
fn hearing(world: &World) {
    let observed = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    fs::create_dir_all(spool(world)).expect("the spool");
    write_json(
        &spool(world).join("listener.json"),
        &json!({ "link": "up", "observed_at": observed }),
    );
}

/// One conversation in the spool, as the listener records it: who said what,
/// in order, with no word on which of it was the reader's — that is the
/// gateway's to say and ephor's to judge from.
fn heard(world: &World, file: &str, conversation: Value) {
    let conversations = spool(world).join("conversations");
    fs::create_dir_all(&conversations).expect("the listener's conversations");
    write_json(&conversations.join(format!("{file}.json")), &conversation);
}

fn conversation(id: &str, room: Option<&str>, title: &str, said: &[(&str, &str)]) -> Value {
    let messages: Vec<Value> = said
        .iter()
        .enumerate()
        .map(|(at, (author, text))| {
            json!({
                "author": author,
                "text": text,
                "when": format!("2026-10-01T09:{:02}:00Z", 12 + at),
            })
        })
        .collect();
    let mut conversation = json!({
        "id": id,
        "title": title,
        "updated_at": format!("2026-10-01T09:{:02}:00Z", 11 + said.len()),
        "reasons": ["mentioned"],
        "threads": [{ "messages": messages, "reply": descriptor() }]
    });
    if let Some(room) = room {
        conversation["room"] = json!(room);
    }
    conversation
}

fn refresh(world: &World) {
    let done = world.ephor().arg("refresh").output().expect("a refresh");
    assert!(
        done.status.success(),
        "a gateway that is hearing its network refreshes cleanly:\n{}",
        String::from_utf8_lossy(&done.stderr)
    );
}

/// The row `ephor feed --json` prints for `id`, or a panic saying what the
/// feed held instead.
fn row(world: &World, id: &str) -> Value {
    let output = world
        .ephor()
        .args(["feed", "--json"])
        .output()
        .expect("the feed");
    let rows = shaped("feed", &output);
    rows.as_array()
        .and_then(|rows| rows.iter().find(|row| row["id"] == id).cloned())
        .unwrap_or_else(|| panic!("no row {id} in the feed, which holds:\n{rows:#}"))
}

/// Which project the attribution engine put `key` under, and how firmly
/// (§FS-008-attribution.3), read from the cache the refresh wrote.
fn placed(world: &World, home: &str, key: &str) -> Value {
    world.matter_in(home, key)["placement"].clone()
}

/// Whether `key` is in the bucket of what nothing claimed, as
/// `ephor feed --unattributed --json` prints it.
fn unattributed(world: &World, key: &str) -> bool {
    let output = world
        .ephor()
        .args(["feed", "--unattributed", "--json"])
        .output()
        .expect("the bucket");
    shaped("feed", &output)
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["id"] == key))
}

/// The case the ticket was opened about: dana asks the widget group a
/// question, and the reader sees it as widget's, waiting on them.
#[test]
fn a_conversation_in_a_claimed_room_is_a_row_that_waits_on_the_reader() {
    let world = chat_world(json!({ "rooms": [ROOM] }), json!({}));
    heard(
        &world,
        "rollout",
        conversation(
            ROOM,
            Some(ROOM),
            "Widget rollout: can we ship Friday?",
            &[("dana", "Can we ship the widget on Friday?")],
        ),
    );
    refresh(&world);

    let row = row(&world, ITEM);
    assert_eq!(
        row["kind"], "message",
        "a conversation is a Messages row: {row:#}"
    );
    assert_eq!(row["source"], "chatgw", "{row:#}");
    assert_eq!(row["project"], "widget", "it is in widget's room: {row:#}");
    assert_eq!(
        row["needs_response"], true,
        "dana has the last word, so it waits on the reader: {row:#}"
    );
    assert_eq!(
        placed(&world, "widget", ITEM)["on"]["how"],
        "venue",
        "the room placed it, not the word `widget` in its title"
    );

    let thread = world
        .ephor()
        .args(["thread", ITEM])
        .output()
        .expect("the thread");
    assert!(
        thread.status.success(),
        "{}",
        String::from_utf8_lossy(&thread.stderr)
    );
    assert!(
        String::from_utf8_lossy(&thread.stdout).contains("Can we ship the widget on Friday?"),
        "the thread shows what dana said:\n{}",
        String::from_utf8_lossy(&thread.stdout)
    );
}

/// The answer goes back the way the question came: to the site's gateway,
/// with the descriptor it put on the thread, and the row stops waiting once
/// the listener has heard the reader say it.
#[test]
fn a_reply_goes_back_to_the_gateway_that_reported_the_row() {
    let world = chat_world(json!({ "rooms": [ROOM] }), json!({}));
    let asked = [("dana", "Can we ship the widget on Friday?")];
    heard(
        &world,
        "rollout",
        conversation(
            ROOM,
            Some(ROOM),
            "Widget rollout: can we ship Friday?",
            &asked,
        ),
    );
    refresh(&world);
    assert_eq!(row(&world, ITEM)["needs_response"], true, "dana is waiting");
    let outbox = spool(&world).join("outbox.jsonl");

    let dry = world
        .ephor()
        .args(["reply", ITEM, "Friday", "works.", "--dry-run", "--json"])
        .output()
        .expect("the dry run");
    let planned = shaped("outcome", &dry);
    assert_eq!(
        planned["ok"], true,
        "the gateway declared replies: {planned:#}"
    );
    assert!(
        planned["says"]
            .as_str()
            .unwrap_or_default()
            .contains("chatgw"),
        "the dry run names the source the reply would go through: {planned:#}"
    );
    assert!(!outbox.exists(), "and sends nothing");

    let sent = world
        .ephor()
        .args(["reply", ITEM, "Friday", "works.", "--json"])
        .output()
        .expect("the reply");
    let happened = shaped("outcome", &sent);
    assert_eq!(happened["ok"], true, "{happened:#}");
    let queued = fs::read_to_string(&outbox).expect("the gateway queued the reply");
    let lines: Vec<Value> = queued
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
        .collect();
    assert_eq!(lines.len(), 1, "one reply, sent once: {queued}");
    assert_eq!(
        lines[0]["target"],
        descriptor(),
        "the thread's descriptor came back exactly as the gateway gave it"
    );
    assert_eq!(lines[0]["text"], "Friday works.");

    // The listener sends it and records it as the reader's own.
    heard(
        &world,
        "rollout",
        conversation(
            ROOM,
            Some(ROOM),
            "Widget rollout: can we ship Friday?",
            &[asked[0], ("you", "Friday works.")],
        ),
    );
    refresh(&world);
    assert_eq!(
        row(&world, ITEM)["needs_response"],
        false,
        "the reader has answered, so nothing waits on them"
    );
}

/// A checkout may say which rooms are its project's, and the row adopts that
/// where it says nothing about rooms itself.
#[test]
fn a_room_the_checkout_hints_is_adopted_where_the_row_is_silent() {
    let world = chat_world(json!({}), json!({}));
    hint(&world, &[ROOM]);
    heard(
        &world,
        "friday",
        conversation(
            ROOM,
            Some(ROOM),
            "Ship on Friday?",
            &[("dana", "Can we ship on Friday?")],
        ),
    );
    refresh(&world);

    assert_eq!(
        placed(&world, "widget", ITEM)["on"]["how"],
        "venue",
        "the hinted room places it"
    );
}

/// A row that lists rooms has the last word, and `[]` is a list: a checkout
/// cannot claim a room its row has said is none of its project's.
#[test]
fn a_row_that_lists_no_rooms_refuses_the_hint() {
    let world = chat_world(json!({ "rooms": [] }), json!({}));
    hint(&world, &[ROOM]);
    heard(
        &world,
        "friday",
        conversation(
            ROOM,
            Some(ROOM),
            "Ship on Friday?",
            &[("dana", "Can we ship on Friday?")],
        ),
    );
    refresh(&world);

    assert!(
        unattributed(&world, ITEM),
        "nothing the row allows claims it, so it waits in the bucket"
    );
    assert!(!world.has_matter_in("widget", ITEM), "and not under widget");
}

/// A row that lists its own rooms is not widened by the hint: the hinted room
/// is nobody's, the listed one is widget's.
#[test]
fn a_row_that_lists_its_rooms_overrides_the_hint() {
    let elsewhere = "slack/acme#C0WIDGET";
    let world = chat_world(json!({ "rooms": [elsewhere] }), json!({}));
    hint(&world, &[ROOM]);
    heard(
        &world,
        "friday",
        conversation(
            ROOM,
            Some(ROOM),
            "Ship on Friday?",
            &[("dana", "Can we ship on Friday?")],
        ),
    );
    heard(
        &world,
        "standup",
        conversation(
            elsewhere,
            Some(elsewhere),
            "Standup moved",
            &[("eli", "Standup is at ten today.")],
        ),
    );
    refresh(&world);

    let listed = format!("chatgw:{elsewhere}");
    assert_eq!(
        placed(&world, "widget", &listed)["on"]["how"],
        "venue",
        "the row's own room places its conversation"
    );
    assert!(
        unattributed(&world, ITEM),
        "the hinted room is not widget's"
    );
}

/// A room outranks what is said in it: a conversation in widget's room that
/// mentions one of gadget's pull requests is still widget's conversation, and
/// the pull request is something it links to, not where it goes.
#[test]
fn a_room_outranks_what_its_messages_mention() {
    let world = chat_world(
        json!({ "rooms": [ROOM] }),
        json!({ "territory": ["acme/gadget"] }),
    );
    heard(
        &world,
        "review",
        conversation(
            ROOM,
            Some(ROOM),
            "Ship on Friday?",
            &[(
                "dana",
                "Only once https://forge.example/acme/gadget/pull/88 is in — acme/gadget#88.",
            )],
        ),
    );
    refresh(&world);

    let placement = placed(&world, "widget", ITEM);
    assert_eq!(placement["on"]["how"], "venue", "{placement:#}");
    assert!(
        !world.has_matter_in("gadget", ITEM),
        "a mention does not move it to the project it names"
    );
}

/// A direct conversation has no room to claim, so it is placed like anything
/// else: by what it references, and in the bucket when that is nothing.
#[test]
fn a_direct_conversation_goes_where_it_points_or_to_the_bucket() {
    let world = chat_world(
        json!({ "rooms": [ROOM] }),
        json!({ "territory": ["acme/gadget"] }),
    );
    let pointed = "whatsapp/acme#4915550001@s.whatsapp.net";
    let idle = "whatsapp/acme#4915550002@s.whatsapp.net";
    heard(
        &world,
        "pointed",
        conversation(
            pointed,
            None,
            "Quick question",
            &[("fay", "Is acme/gadget ready to tag?")],
        ),
    );
    heard(
        &world,
        "idle",
        conversation(idle, None, "Lunch?", &[("gus", "Lunch at noon?")]),
    );
    refresh(&world);

    let pointed = format!("chatgw:{pointed}");
    assert_eq!(
        row(&world, &pointed)["project"],
        "gadget",
        "it names gadget's territory and nothing else"
    );
    assert!(
        unattributed(&world, &format!("chatgw:{idle}")),
        "it names nothing, so it waits in the bucket"
    );
}

/// The checkout's own word on its project's rooms, in its manifest
/// (§FS-006-project-interface.2).
fn hint(world: &World, rooms: &[&str]) {
    write_json(
        &world.path().join("widget").join("ephor.json"),
        &json!({ "version": 1, "identity": { "rooms": rooms } }),
    );
}
