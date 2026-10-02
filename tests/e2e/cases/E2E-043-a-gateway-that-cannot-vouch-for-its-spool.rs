//! E2E-043-a-gateway-that-cannot-vouch-for-its-spool: a chat gateway that
//! cannot show it is still hearing the network fails, and says which way.
//!
//! A chat gateway answers from a spool its listener keeps, and the spool looks
//! the same whether the listener heard a quiet afternoon or stopped hearing
//! anything at noon. An empty answer from it means nothing is waiting, so it
//! may only be given while the gateway can show current observation of the
//! network (§FS-001-forge-interface.6). Until the gateway was asked for its
//! conversations at all (§FS-001-forge-interface.1), none of this could
//! surface: a gateway never paired, one whose listener had lost the network,
//! and one reading yesterday's spool all refreshed cleanly, as nothing.
//!
//! What this case holds ephor to, through the shipped
//! `config/chat-gateway.example.sh` bound once for the site
//! (§FS-001-forge-interface.9). A gateway nobody paired fails and the reader
//! is told to pair it. A listener that cannot reach the network is shown as
//! unreachable, the kind of failure its diagnosis names. A spool the listener
//! stopped keeping current fails without being called unreachable, and the
//! rows the last good answer placed stay, marked stale. A quiet room is none
//! of these: it is an empty answer, and a clean refresh.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};
use support::{write_json, World};

/// The gateway exactly as ephor ships it, installed as the forge `chatgw`.
const GATEWAY: &str = include_str!("../../../config/chat-gateway.example.sh");

const ROOM: &str = "whatsapp/acme#120363@g.us";
const ITEM: &str = "chatgw:whatsapp/acme#120363@g.us";

/// One project, `widget`, claiming the room, and a chat gateway bound once
/// for the site over a spool nobody has written yet.
fn chat_world() -> World {
    let world = World::new();
    world.stub("ephor-forge-chatgw", GATEWAY);
    let mut registry = world.registry_doc();
    let mut row = registry["projects"][0].clone();
    let root = world.path().join("widget");
    fs::create_dir_all(&root).expect("the project's root");
    row["id"] = json!("widget");
    row["display_name"] = json!("Widget");
    row["root"] = json!(root.to_string_lossy());
    row["rooms"] = json!([ROOM]);
    registry["projects"] = json!([row]);
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
            "projects": { "widget": { "providers": [] } }
        }),
    );
    fs::create_dir_all(spool(&world)).expect("the spool");
    world
}

fn spool(world: &World) -> PathBuf {
    world.path().join("spool")
}

/// The listener's record of the network: whether it can reach it, and when
/// the network last confirmed it was hearing it, `ago` seconds back.
fn listener(world: &World, link: &str, ago: i64) {
    let observed = (chrono::Utc::now() - chrono::Duration::seconds(ago))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    write_json(
        &spool(world).join("listener.json"),
        &json!({ "link": link, "observed_at": observed }),
    );
}

/// Dana's question to the widget group, as the listener recorded it.
fn asked(world: &World) {
    let conversation: Value = json!({
        "id": ROOM,
        "title": "Widget rollout: can we ship Friday?",
        "updated_at": "2026-10-01T09:12:00Z",
        "room": ROOM,
        "reasons": ["mentioned"],
        "threads": [{
            "messages": [{
                "author": "dana",
                "text": "Can we ship the widget on Friday?",
                "when": "2026-10-01T09:12:00Z"
            }],
            "reply": { "chat": "120363@g.us" }
        }]
    });
    let conversations = spool(world).join("conversations");
    fs::create_dir_all(&conversations).expect("the listener's conversations");
    write_json(&conversations.join("rollout.json"), &conversation);
}

fn refresh(world: &World) -> std::process::Output {
    world.ephor().arg("refresh").output().expect("a refresh")
}

fn said(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// No listener has ever been paired with an account, so there is nothing the
/// gateway could answer for. That is a failure, and the remedy is named.
#[test]
fn a_gateway_nobody_paired_fails_and_says_to_pair_it() {
    let world = chat_world();

    let done = refresh(&world);
    let stderr = said(&done);
    assert!(
        !done.status.success(),
        "a gateway that has never heard the network has no answer to give:\n{stderr}"
    );
    assert!(
        stderr.contains("chatgw"),
        "the failure names the source:\n{stderr}"
    );
    assert!(
        stderr.contains("pair"),
        "and says to pair the listener:\n{stderr}"
    );
}

/// The listener is running and cannot reach the network: the gateway reports
/// the network as refusing it, and ephor shows that as unreachable.
#[test]
fn a_listener_that_lost_the_network_is_shown_as_unreachable() {
    let world = chat_world();
    listener(&world, "down", 0);
    asked(&world);

    let done = refresh(&world);
    let stderr = said(&done);
    assert!(
        !done.status.success(),
        "a listener that cannot reach the network is not a quiet room:\n{stderr}"
    );
    assert!(
        stderr.contains("chatgw: unreachable"),
        "the failure is of the unreachable kind, named by its source:\n{stderr}"
    );
}

/// The listener stopped confirming it was hearing the network an hour ago.
/// What it recorded before then is not an answer for now: the refresh fails,
/// is not mistaken for a network outage, and keeps the last good rows, stale.
#[test]
fn a_spool_nobody_keeps_current_fails_and_keeps_the_last_rows_stale() {
    let world = chat_world();
    listener(&world, "up", 0);
    asked(&world);
    let healthy = refresh(&world);
    assert!(healthy.status.success(), "{}", said(&healthy));

    listener(&world, "up", 3600);
    let done = refresh(&world);
    let stderr = said(&done);
    assert!(
        !done.status.success(),
        "a spool nobody vouches for is not an answer:\n{stderr}"
    );
    assert!(
        stderr.contains("chatgw"),
        "the failure names the source:\n{stderr}"
    );
    assert!(
        !stderr.contains("unreachable"),
        "the network is not what failed, and the reader is not sent to check it:\n{stderr}"
    );

    let slot = &world.feed_of("widget")["providers"]["chatgw"];
    assert_eq!(
        slot["stale"], true,
        "the last good rows wait out the failure: {slot:#}"
    );
    assert!(
        world.has_matter_in("widget", ITEM),
        "dana's question is still there: {slot:#}"
    );
    let feed = world.ephor().arg("feed").output().expect("the feed");
    assert!(
        String::from_utf8_lossy(&feed.stdout).contains("(stale)"),
        "and the feed says it is stale:\n{}",
        String::from_utf8_lossy(&feed.stdout)
    );
}

/// The network confirmed just now that the listener is hearing it, and nobody
/// has said anything: an empty answer, and a clean refresh.
///
/// Already true before the Messages row existed, because nothing asked the
/// gateway anything; kept so the row cannot make a quiet room into a failure.
#[test]
fn a_quiet_room_is_an_empty_answer_and_a_clean_refresh() {
    let world = chat_world();
    listener(&world, "up", 0);

    let done = refresh(&world);
    assert!(done.status.success(), "{}", said(&done));
    assert!(
        world.matters_in("widget").is_empty(),
        "nothing was said, so nothing waits"
    );
    assert!(world.matters_in("unattributed").is_empty());
}
