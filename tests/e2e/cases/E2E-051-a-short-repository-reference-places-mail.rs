//! A mail naming an issue as `owner/name#N` goes to the repository's project
//! at Reference, as its URL control does (§FS-008-attribution.3). A matching
//! project name must not weaken it to Resemblance, and a differing name must
//! not leave it in the unattributed bucket (§FS-008-attribution.4).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};
use std::fs;
use support::{shaped, write_json, World};

/// The upstream reproducer's four conversations: a site-bound mail source,
/// neutral titles, and territory claims rather than repository layout paths.
fn mail_world() -> World {
    let world = World::new();
    let mut registry = world.registry_doc();
    let template = registry["projects"][0].clone();
    registry["projects"] = [
        ("me", None),
        ("rhei", Some("agent-grounds/rhei")),
        ("grund", Some("agent-grounds/grund")),
        ("herald", Some("agent-grounds/keryx")),
    ]
    .into_iter()
    .map(|(id, territory)| {
        let mut row = template.clone();
        let root = world.path().join(id);
        fs::create_dir_all(&root).expect("a project root");
        row["id"] = json!(id);
        row["display_name"] = json!(id);
        row["root"] = json!(root);
        if let Some(repo) = territory {
            row["territory"] = json!([repo]);
        }
        row
    })
    .collect();
    write_json(&world.registry_path(), &registry);
    write_json(
        &world.config_path(),
        &json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "sources": [{ "provider": "mail-like" }],
            "projects": {
                "me": { "providers": [] },
                "rhei": { "providers": [] },
                "grund": { "providers": [] },
                "herald": { "providers": [] }
            }
        }),
    );

    let inbox: Vec<Value> = [
        ("rhei-short", "agent-grounds/rhei#12"),
        (
            "rhei-url",
            "https://github.com/agent-grounds/rhei/issues/12",
        ),
        ("keryx-short", "agent-grounds/keryx#7"),
        (
            "keryx-url",
            "https://github.com/agent-grounds/keryx/issues/7",
        ),
    ]
    .into_iter()
    .map(|(id, reference)| {
        json!({
            "id": id,
            "title": "Before Friday",
            "updated_at": "2026-10-07T09:02:00Z",
            "reasons": ["in-thread"],
            "threads": [{
                "reply": { "thread": format!("t-{id}") },
                "messages": [{
                    "author": "dana", "mine": false,
                    "text": format!("Could you look at {reference} before Friday?"),
                    "when": "2026-10-07T09:02:00Z"
                }]
            }]
        })
    })
    .collect();
    let inbox_path = world.path().join("inbox.json");
    write_json(&inbox_path, &json!(inbox));
    world.stub(
        "ephor-forge-mail-like",
        &format!(
            "#!/bin/sh\ncase \"$1\" in\n  capabilities) echo '{{\"messages\":true,\"replies\":true}}' ;;\n  messages) cat '{}' ;;\n  reply) cat >/dev/null; echo '{{}}' ;;\n  *) exit 64 ;;\nesac\n",
            inbox_path.display()
        ),
    );
    world
}

#[test]
fn numeric_repository_reference_places_mail_like_its_url_controls() {
    let world = mail_world();
    world.ephor().arg("refresh").assert().success();

    // Controls are checked first: a failure here is a fixture problem, rather
    // than evidence of the short-form defect.
    for (id, home) in [("rhei-url", "rhei"), ("keryx-url", "herald")] {
        let placement = world.matter_in(home, &format!("mail-like:{id}"))["placement"].clone();
        assert_eq!(
            placement,
            json!({ "on": { "project": home, "how": "reference" } }),
            "URL control {id} must place by reference; otherwise the fixture is unsound"
        );
    }

    let mut placements = serde_json::Map::new();
    for home in ["me", "rhei", "grund", "herald", "unattributed"] {
        for matter in world.matters_in(home) {
            let key = matter["key"].as_str().expect("a cached matter key");
            if ["mail-like:rhei-short", "mail-like:keryx-short"].contains(&key) {
                assert!(
                    placements
                        .insert(key.to_string(), matter["placement"].clone())
                        .is_none(),
                    "a conversation must appear in only one home"
                );
            }
        }
    }
    let output = world
        .ephor()
        .args(["feed", "--unattributed", "--json"])
        .output()
        .expect("read the unattributed feed");
    let bucket = shaped("feed", &output);
    let bucket_ids: Vec<Value> = bucket
        .as_array()
        .expect("the feed is an array")
        .iter()
        .map(|row| row["id"].clone())
        .collect();
    let observed = json!({ "placements": placements, "unattributed": bucket_ids });
    let expected = json!({
        "placements": {
            "mail-like:rhei-short": { "on": { "project": "rhei", "how": "reference" } },
            "mail-like:keryx-short": { "on": { "project": "herald", "how": "reference" } }
        },
        "unattributed": []
    });
    assert_eq!(
        observed, expected,
        "both short forms must match their URL controls at Reference, leaving no mail unattributed; observed:\n{observed:#}"
    );
}
