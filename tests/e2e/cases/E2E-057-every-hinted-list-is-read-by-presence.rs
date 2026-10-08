//! E2E-057-every-hinted-list-is-read-by-presence: a registry row says which
//! aliases, territory, addresses and rooms are its project's by writing the
//! list, `[]` included, and only a row that leaves a key out adopts what the
//! checkout's manifest hints.
//!
//! A project's `ephor.json`, which anyone who can push to its repository can
//! edit, hinted an alias, the organization `acme`, the address
//! `alice@example.org` and a room. Every message author is address evidence,
//! so Alice's mail was this project's whatever its registry row said: a row
//! that wrote `[]` for every list still adopted the alias, the organization and
//! the address and refused only the room, and a row that wrote its own
//! addresses was not read at all. The owner could not refuse the claim from
//! their own registry.
//!
//! What this case holds ephor to. A site-level mail source and a checkout that
//! hints all four lists, run once under each of three rows: one silent on
//! every list, one that writes `[]` for each, and one that writes lists of its
//! own. A list the row writes is final and only a row without the key adopts
//! the hint (§FS-008-attribution.1.1). An address places at the strength of a
//! reference and an alias at that of a resemblance, a room is a venue
//! (§FS-008-attribution.3), and what no list claims waits in the unattributed
//! bucket (§FS-008-attribution.4). A row whose `addresses` is not a list of
//! non-empty strings is refused by name, as one with such `rooms` is
//! (§FS-008-attribution.1.1).
//!
//! The decision this realizes is §DF-006-every-hinted-list-is-read-by-presence.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};
use support::{shaped, write_json, World, PROJECT};

/// The site-level mail source, as `status.json` names it and as the forge stub
/// is installed.
const SOURCE: &str = "mail-like";

/// The rooms, as the source states them: the one the checkout hints and the
/// one a row writes for itself.
const HINTED_ROOM: &str = "whatsapp/acme#1@g.us";
const OWN_ROOM: &str = "whatsapp/acme#2@g.us";

/// What the checkout's manifest hints, on every list a checkout can hint. The
/// issue's alias was `the widget`; a word of its own is used here so that a
/// resemblance can show whether it was adopted.
fn hints() -> Value {
    json!({
        "aliases": ["widget"],
        "territory": ["acme"],
        "addresses": ["alice@example.org"],
        "rooms": [HINTED_ROOM]
    })
}

/// The three rows of the issue, by what each writes on the identity lists.
#[derive(Clone, Copy, Debug)]
enum Row {
    /// Writes none of the keys.
    Silent,
    /// Writes `[]` for every one of them.
    SaysNone,
    /// Writes a list of its own for every one of them.
    SaysItsOwn,
}

impl Row {
    fn keys(self) -> Value {
        match self {
            Row::Silent => json!({}),
            Row::SaysNone => json!({
                "aliases": [], "territory": [], "addresses": [], "rooms": []
            }),
            Row::SaysItsOwn => json!({
                "aliases": ["gizmo"],
                "territory": ["acme/plugin"],
                "addresses": ["bob@example.org"],
                "rooms": [OWN_ROOM]
            }),
        }
    }
}

/// One mail per signal: who wrote it, which room it is in, and what it says.
/// Each names exactly one thing a list could claim, and nothing else.
const MAILS: [(&str, &str, Option<&str>, &str); 8] = [
    // The address the checkout hints, and the one a row writes.
    (
        "from-alice",
        "alice@example.org",
        None,
        "Are you around on Thursday?",
    ),
    (
        "from-bob",
        "bob@example.org",
        None,
        "Are you around on Thursday?",
    ),
    // A repository under the organization the checkout hints, and the one
    // repository of it a row writes.
    (
        "acme-tools",
        "dana",
        None,
        "Could you look at https://github.com/acme/tools/issues/4 before Friday?",
    ),
    (
        "acme-plugin",
        "dana",
        None,
        "Could you look at https://github.com/acme/plugin/issues/9 before Friday?",
    ),
    // The alias the checkout hints, and the one a row writes.
    ("widget", "dana", None, "Is the widget build green yet?"),
    ("gizmo", "dana", None, "Is the gizmo build green yet?"),
    // The room the checkout hints, and the one a row writes.
    ("hinted-room", "dana", Some(HINTED_ROOM), "Lunch at noon?"),
    ("own-room", "dana", Some(OWN_ROOM), "Lunch at noon?"),
];

/// A world of one project whose checkout hints every list, under `row`, with
/// the mail source bound once for the site so the engine places what it
/// reports (§FS-001-forge-interface.9).
fn mail_world(row: Row) -> World {
    let world = World::new();
    world.register(row.keys());
    world.manifest(json!({ "version": 1, "identity": hints() }));
    world.configure(json!({ "sources": [{ "provider": SOURCE }] }));

    let messages: Vec<Value> = MAILS
        .iter()
        .map(|(id, author, room, text)| {
            let mut mail = json!({
                "id": id,
                "title": "A question",
                "updated_at": "2026-10-07T09:02:00Z",
                "reasons": ["in-thread"],
                "threads": [{
                    "reply": { "thread": format!("t-{id}") },
                    "messages": [{
                        "author": author, "mine": false,
                        "text": text,
                        "when": "2026-10-07T09:02:00Z"
                    }]
                }]
            });
            if let Some(room) = room {
                mail["room"] = json!(room);
            }
            mail
        })
        .collect();
    let inbox_path = world.path().join("inbox.json");
    write_json(&inbox_path, &json!(messages));
    world.stub(
        &format!("ephor-forge-{SOURCE}"),
        &format!(
            "#!/bin/sh\ncase \"$1\" in\n  capabilities) echo '{{\"messages\":true,\"replies\":true}}' ;;\n  messages) cat '{}' ;;\n  reply) cat >/dev/null; echo '{{}}' ;;\n  *) exit 64 ;;\nesac\n",
            inbox_path.display()
        ),
    );
    world
}

/// The cache key of one of the mails.
fn key(id: &str) -> String {
    format!("{SOURCE}:{id}")
}

/// Both streams of a run, so a failure shows what it printed wherever it went.
fn printed(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Where each mail is after a refresh, read from the cached feed every surface
/// reads: `demo by <strength>` under the project, `unattributed` in the bucket.
/// A mail in two homes shows both, and one in none shows nothing.
fn placements(world: &World) -> Value {
    let refresh = world.ephor().arg("refresh").output().expect("refresh");
    assert!(
        refresh.status.success(),
        "the registry must load and the mail source answer; refresh printed:\n{}",
        printed(&refresh)
    );
    let mut found = serde_json::Map::new();
    for (id, _, _, _) in MAILS {
        let homes: Vec<Value> = [PROJECT, "unattributed"]
            .into_iter()
            .flat_map(|home| {
                world
                    .matters_in(home)
                    .into_iter()
                    .filter(|matter| matter["key"] == key(id))
                    .map(move |matter| match home {
                        "unattributed" => json!("unattributed"),
                        _ => json!(format!(
                            "{home} by {}",
                            matter["placement"]["on"]["how"]
                                .as_str()
                                .unwrap_or("nothing weighed")
                        )),
                    })
            })
            .collect();
        found.insert(id.to_string(), json!(homes));
    }
    Value::Object(found)
}

/// The mails `ephor feed --unattributed --json` lists, by their ids.
fn bucket(world: &World) -> Vec<String> {
    let output = world
        .ephor()
        .args(["feed", "--unattributed", "--json"])
        .output()
        .expect("read the unattributed feed");
    let mut ids: Vec<String> = shaped("feed", &output)
        .as_array()
        .expect("the feed is an array")
        .iter()
        .filter_map(|row| row["id"].as_str())
        .filter_map(|id| id.strip_prefix(&format!("{SOURCE}:")))
        .map(String::from)
        .collect();
    ids.sort();
    ids
}

/// What one row compiles the checkout's hints to, seen through where its mail
/// lands. `on` names the mails the project should keep, each with the strength
/// that should place it; every other mail should wait in the bucket.
fn holds(row: Row, on: &[(&str, &str)]) {
    let world = mail_world(row);
    let observed = json!({ "placements": placements(&world), "unattributed": bucket(&world) });
    let mut placed = serde_json::Map::new();
    let mut waiting: Vec<String> = Vec::new();
    for (id, _, _, _) in MAILS {
        match on.iter().find(|(kept, _)| *kept == id) {
            Some((_, how)) => {
                placed.insert(id.to_string(), json!([format!("{PROJECT} by {how}")]));
            }
            None => {
                placed.insert(id.to_string(), json!(["unattributed"]));
                waiting.push(id.to_string());
            }
        }
    }
    waiting.sort();
    let expected = json!({ "placements": placed, "unattributed": waiting });
    assert_eq!(
        observed,
        expected,
        "under the {row:?} row ({}), a list the row writes must be final, `[]` included, and \
         only a list it leaves out may adopt the checkout's hint ({}); observed:\n{observed:#}",
        row.keys(),
        hints()
    );
}

/// A row that writes none of the lists adopts every hint, as before: Alice's
/// mail and a mail on an `acme` repository are the project's.
#[test]
fn a_row_silent_on_every_list_adopts_every_hint() {
    holds(
        Row::Silent,
        &[
            ("from-alice", "reference"),
            ("acme-tools", "reference"),
            ("acme-plugin", "reference"),
            ("widget", "resemblance"),
            ("hinted-room", "venue"),
        ],
    );
}

/// A row that writes `[]` for every list refuses every hint: nothing the
/// checkout claimed places a mail, so all of it waits in the bucket.
#[test]
fn a_row_that_writes_none_refuses_every_hint() {
    holds(Row::SaysNone, &[]);
}

/// A row that writes lists of its own gets exactly those: Bob's mail is the
/// project's and Alice's waits, and so for each list.
#[test]
fn a_row_that_writes_its_own_lists_replaces_every_hint() {
    holds(
        Row::SaysItsOwn,
        &[
            ("from-bob", "reference"),
            ("acme-plugin", "reference"),
            ("gizmo", "resemblance"),
            ("own-room", "venue"),
        ],
    );
}

/// A row's `addresses` is a list of non-empty strings. Anything else is
/// refused by name rather than read as silence, which would let the checkout's
/// hint stand in for what the row tried to say; `[]` and a list of addresses
/// load.
#[test]
fn a_row_whose_addresses_is_not_a_list_of_addresses_is_refused_by_name() {
    let verdict = |addresses: Value| {
        let world = World::new();
        world.register(json!({ "addresses": addresses }));
        let output = world.ephor().arg("validate").output().expect("validate");
        let said = printed(&output);
        let named = said
            .lines()
            .any(|line| line.contains("/projects/0/addresses"));
        json!({ "loads": output.status.success(), "names /projects/0/addresses": named })
    };
    let observed = json!({
        "a string": verdict(json!("alice@example.org")),
        "an empty address": verdict(json!([""])),
        "a number": verdict(json!([7])),
        "[]": verdict(json!([])),
        "a list of addresses": verdict(json!(["bob@example.org"])),
    });
    let refused = json!({ "loads": false, "names /projects/0/addresses": true });
    let loads = json!({ "loads": true, "names /projects/0/addresses": false });
    let expected = json!({
        "a string": refused,
        "an empty address": refused,
        "a number": refused,
        "[]": loads,
        "a list of addresses": loads,
    });
    assert_eq!(
        observed, expected,
        "a row's addresses must be a list of non-empty strings, refused by name otherwise, \
         as rooms are; observed:\n{observed:#}"
    );
}
