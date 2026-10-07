//! E2E-053: a conversation settled at its source (§FS-001-forge-interface.1,
//! §FS-011-command-line.4, §DF-004-settle-at-the-source). Replays
//! agent-grounds/ephor#194 on the mail carrier of E2E-052: Dana's mail is
//! answered from ephor and marked done, and until `ephor settle` it stays in
//! the inbox it came from.
//!
//! Three stories, each its own test so each fails for its own reason. A carrier
//! that declares `settle` is asked exactly once, on the reader's move, with the
//! conversation's own id; replying and marking read never ask it, a rehearsal
//! asks nothing, and the settled row is done until the next message brings it
//! back saying why (§FS-003-feed-categories.4, §FS-007-matters.5). A carrier
//! that declares `archive` instead is refused by name and asked nothing
//! (§FS-007-matters.4). A carrier bound for the site is asked with no project,
//! wherever its conversation was placed (§FS-001-forge-interface.9).

#[allow(dead_code)]
#[path = "../reply_world.rs"]
mod reply_world;
#[path = "../support.rs"]
mod support;

use std::collections::BTreeSet;

use ephor::feed::cache::Seen;
use ephor::matter::Matter;
use reply_world::{says, MailWorld, ITEM};
use serde_json::{json, Value};
use support::{json_of, read_json, shaped};

/// The sentence every surface refuses an undeclared settle with.
const REFUSAL: &str = "mail-me cannot settle a conversation at its source";
/// The conversation's own id, as the carrier reported it under messages.
const ID: &str = "k-B";
/// What the carrier calls the conversation.
const TITLE: &str = "Quote for the garden fence";
/// The list the conversation is on, which the project claims as a room.
const LIST: &str = "lists/garden@example.org";

/// A mail carrier declaring `capabilities`, logging every subcommand from here
/// on, refreshed so the feed knows what it declared.
fn carrier(capabilities: Value) -> MailWorld {
    let world = MailWorld::new(false);
    world.put("capabilities.json", capabilities);
    world.flag("log-calls");
    world.refresh();
    world
}

/// Every subcommand the carrier was run as since it started logging.
fn calls(world: &MailWorld) -> Vec<String> {
    world
        .lines("calls.jsonl")
        .iter()
        .map(|call| call["command"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// Every settle request the carrier received, as it arrived on stdin.
fn settles(world: &MailWorld) -> Vec<Value> {
    world.lines("settles.jsonl")
}

/// Whether the row is in front of the reader, as `ephor feed --unread` says.
fn unread(world: &MailWorld) -> bool {
    let rows = shaped("feed", &world.ok(&["feed", "--unread", "--json"]));
    rows.as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["id"] == ITEM))
}

/// Why the row is back. Only the screen shows it, so it is read through the
/// library from what the commands left behind (§FS-007-matters.5).
fn resurfacing(world: &MailWorld) -> Option<String> {
    let seen: Seen =
        serde_json::from_value(read_json(&world.world.path().join("state/ephor/seen.json")))
            .expect("the seen store parses");
    let matter: Matter =
        serde_json::from_value(world.world.matter(ITEM)).expect("the cached matter parses");
    ephor::feed::cache::resurfacing(&seen, &matter)
}

/// Dana writes once more, after the reader settled the conversation.
fn dana_writes_again(world: &MailWorld) {
    let mut row = world.get("conversation.json");
    let latch = "<latch@dana.example>";
    row["threads"][0]["messages"]
        .as_array_mut()
        .expect("the thread has messages")
        .push(json!({"id": latch, "author": "dana", "mine": false,
            "text": "And the gate latch?", "when": "2026-10-07T12:00:00Z"}));
    row["threads"][0]["reply"] = json!({"in_reply_to": latch});
    row["updated_at"] = json!("2026-10-07T12:00:00Z");
    world.put("conversation.json", row);
}

/// No key on stdin that `ephor schema forge` does not tell a carrier's author
/// about.
fn undeclared_keys(world: &MailWorld, request: &Value) -> Vec<String> {
    let schema = json_of(&world.ok(&["schema", "forge"]));
    let declared: BTreeSet<String> = schema["$defs"]["request"]["properties"]
        .as_object()
        .expect("the request declares its properties")
        .keys()
        .cloned()
        .collect();
    request
        .as_object()
        .expect("a request is an object")
        .keys()
        .filter(|key| !declared.contains(*key))
        .cloned()
        .collect()
}

#[test]
fn a_declared_settle_is_asked_once_on_the_readers_move_and_the_row_is_done() {
    let world = carrier(json!({"messages": true, "replies": true, "settle": true}));

    // Answering and reading never reach the inbox.
    world.ok(&["reply", ITEM, "Yes, picked them up this morning."]);
    world.ok(&["mark-read", "demo", "--id", ITEM]);
    assert!(
        !calls(&world).iter().any(|call| call == "settle") && settles(&world).is_empty(),
        "reply and mark-read settle nothing: {:?}",
        calls(&world)
    );
    world.incoming();
    world.refresh();
    assert!(unread(&world), "Dana wrote again, so the row waits");

    let dry = world.run(&["settle", ITEM, "--dry-run"]);
    assert!(
        says(&dry).contains("would ask mail-me to settle") && says(&dry).contains(TITLE),
        "the rehearsal says which source it would ask about which conversation: {}",
        says(&dry)
    );
    assert!(dry.status.success(), "{}", says(&dry));
    assert!(
        settles(&world).is_empty() && !calls(&world).iter().any(|call| call == "settle"),
        "a rehearsal sends nothing: {:?}",
        calls(&world)
    );
    assert!(unread(&world), "a rehearsal changes nothing here either");

    let done = world.run(&["settle", ITEM]);
    assert!(
        says(&done).contains("settled at mail-me"),
        "the move says where it settled: {}",
        says(&done)
    );
    assert!(done.status.success(), "{}", says(&done));
    let sent = settles(&world);
    assert_eq!(sent.len(), 1, "one move, one request: {sent:?}");
    assert_eq!(
        sent[0]["target"],
        json!(ID),
        "the conversation's own id, verbatim"
    );
    assert_eq!(sent[0]["project"], "demo", "{}", sent[0]);
    assert_eq!(
        undeclared_keys(&world, &sent[0]),
        Vec::<String>::new(),
        "{}",
        sent[0]
    );
    assert!(
        !unread(&world),
        "accepted, the row is done as `m` leaves it"
    );

    // A repeat is success, and ephor never reads the inbox back as done.
    let again = world.run(&["settle", ITEM, "--json"]);
    let outcome = shaped("outcome", &again);
    assert_eq!(outcome["ok"], true, "{outcome:#}");
    assert!(again.status.success());
    assert_eq!(settles(&world).len(), 2, "asked again, as asked");
    assert_eq!(settles(&world)[1], sent[0], "the same request again");

    dana_writes_again(&world);
    world.refresh();
    assert!(
        unread(&world),
        "a later message brings the conversation back"
    );
    assert_eq!(
        resurfacing(&world).as_deref(),
        Some("⟳ the conversation moved"),
        "and the row says why"
    );
    assert_eq!(
        settles(&world).len(),
        2,
        "and nothing settled it on its own"
    );
}

#[test]
fn an_undeclared_settle_is_refused_by_name_and_reaches_nothing() {
    let world = carrier(json!({"messages": true, "replies": true, "archive": true}));

    for args in [&["settle", ITEM][..], &["settle", ITEM, "--dry-run"][..]] {
        let out = world.run(args);
        assert!(
            says(&out).contains(REFUSAL),
            "`ephor {}` refuses by name: {}",
            args.join(" "),
            says(&out)
        );
        assert!(!out.status.success(), "{}", says(&out));
    }
    for args in [
        &["settle", ITEM, "--json"][..],
        &["settle", ITEM, "--dry-run", "--json"][..],
    ] {
        let out = world.run(args);
        assert!(
            String::from_utf8_lossy(&out.stdout).contains(REFUSAL),
            "`ephor {}` refuses by name in its machine form: {}",
            args.join(" "),
            says(&out)
        );
        let outcome = shaped("outcome", &out);
        assert_eq!(outcome["ok"], false, "{outcome:#}");
        assert!(!out.status.success(), "{}", says(&out));
    }

    let asked = calls(&world);
    assert!(
        !asked
            .iter()
            .any(|call| call == "settle" || call == "archive"),
        "the carrier was asked for neither: {asked:?}"
    );
    assert!(settles(&world).is_empty(), "{:?}", settles(&world));
    assert!(unread(&world), "a refusal changes nothing here");
}

#[test]
fn a_site_bound_carrier_is_asked_with_no_project() {
    let world = MailWorld::new(false);
    world.put(
        "capabilities.json",
        json!({"messages": true, "replies": true, "settle": true}),
    );
    // Bound once for the site; the project claims the list the mail is on.
    world.world.configure(json!({
        "defaults": {"provider_timeout_seconds": 1},
        "sources": [{"provider": "mail-me", "user": "me"}],
        "projects": {"demo": {"providers": []}}
    }));
    world.world.register(json!({"rooms": [LIST]}));
    let mut row = world.get("conversation.json");
    row["room"] = json!(LIST);
    world.put("conversation.json", row);
    world.flag("log-calls");
    world.ok(&["refresh"]);
    let placement = world.world.matter(ITEM)["placement"].clone();
    assert_eq!(
        placement["on"]["how"], "venue",
        "the claimed list placed it: {placement:#}"
    );

    let done = world.run(&["settle", ITEM]);
    assert!(
        says(&done).contains("settled at mail-me"),
        "the site's carrier settles it: {}",
        says(&done)
    );
    assert!(done.status.success(), "{}", says(&done));
    let sent = settles(&world);
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0]["target"], json!(ID));
    assert_eq!(sent[0]["project"], "", "a site source is told no project");
}
