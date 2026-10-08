//! E2E-055-a-row-is-a-sources-fallback-home: a registry row that claims a
//! conversation source as its fallback home keeps what that source reports and
//! nothing names, while a mail that references a project still reaches it.
//!
//! A personal inbox kept as a site-level source had no home. Its mail that
//! named no project went to the unattributed bucket beside real mapping
//! failures, and a family mail whose German *Grund* matched a project's name
//! became that project's matter. Binding the source under the person's row
//! instead took the mail about another project's issue with it.
//!
//! What this case holds ephor to. A row names the sources it is the fallback
//! home for, exactly as `status.json` names them (§FS-008-attribution.1). A
//! conversation from a claimed source goes to the claiming row unless a venue
//! or a reference places it, and resemblance never places it
//! (§FS-008-attribution.3), so what it reports and nothing names is the
//! claimer's, not the bucket's (§FS-008-attribution.4). A claim on a source
//! bound only under a project can place nothing, and is said as a note by
//! `refresh` and `doctor` alike, in prose and in JSON, without moving the exit
//! code (§FS-008-attribution.4).
//!
//! The decision this realizes is §DF-005-a-row-is-a-sources-fallback-home.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};
use std::fs;
use support::{shaped, write_json, World};

/// The personal inbox, as `status.json` names it and as the forge stub is
/// installed.
const SOURCE: &str = "mail-me";

/// The issue's three mails, verbatim: one references rhei's issue, one names
/// nothing, and one is German whose *Grund* matches a project's name.
const MAILS: [(&str, &str, &str); 3] = [
    (
        "before-friday",
        "Before Friday",
        "Could you look at https://github.com/agent-grounds/rhei/issues/12 before Friday?",
    ),
    ("sunday", "Sunday", "Mum asks if you can bring the cake."),
    (
        "absage",
        "Absage",
        "Aus welchem Grund kommst du am Samstag nicht?",
    ),
];

/// Where the inbox is bound (§FS-001-forge-interface.9).
#[derive(Clone, Copy)]
enum Binding {
    /// Once for the site, so the engine places what it reports.
    Site,
    /// Under the `me` project only, so what it reports is `me`'s unweighed.
    UnderMe,
}

/// E2E-051's mail world without its fourth project: a personal row `me`, and
/// `rhei` and `grund` claiming their repositories as territory. `claim` is
/// what `me` writes as its `fallback_sources`, or nothing.
fn mail_world(binding: Binding, claim: Option<&str>) -> World {
    let world = World::new();
    let mut registry = world.registry_doc();
    let template = registry["projects"][0].clone();
    registry["projects"] = [
        ("me", None),
        ("rhei", Some("agent-grounds/rhei")),
        ("grund", Some("agent-grounds/grund")),
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
        if let (Some(source), "me") = (claim, id) {
            row["fallback_sources"] = json!([source]);
        }
        row
    })
    .collect();
    write_json(&world.registry_path(), &registry);

    let inbox = json!({ "provider": SOURCE });
    let (sources, mine) = match binding {
        Binding::Site => (json!([inbox]), json!([])),
        Binding::UnderMe => (json!([]), json!([inbox])),
    };
    write_json(
        &world.config_path(),
        &json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "sources": sources,
            "projects": {
                "me": { "providers": mine },
                "rhei": { "providers": [] },
                "grund": { "providers": [] }
            }
        }),
    );

    let messages: Vec<Value> = MAILS
        .iter()
        .map(|(id, title, text)| {
            json!({
                "id": id,
                "title": title,
                "updated_at": "2026-10-07T09:02:00Z",
                "reasons": ["in-thread"],
                "threads": [{
                    "reply": { "thread": format!("t-{id}") },
                    "messages": [{
                        "author": "dana", "mine": false,
                        "text": text,
                        "when": "2026-10-07T09:02:00Z"
                    }]
                }]
            })
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

/// The cache key of one of the three mails.
fn key(id: &str) -> String {
    format!("{SOURCE}:{id}")
}

/// Every home each mail is in, with its placement there, read from the cached
/// feed every surface reads. A mail in two homes shows as two entries. A site
/// with no site-level source writes no bucket at all.
fn placements(world: &World) -> Value {
    let cached = |home: &str| {
        world
            .path()
            .join("state/ephor/feed")
            .join(format!("{home}.json"))
            .exists()
    };
    let mut found = serde_json::Map::new();
    for (id, _, _) in MAILS {
        let homes: Vec<Value> = ["me", "rhei", "grund", "unattributed"]
            .into_iter()
            .filter(|home| cached(home))
            .filter_map(|home| {
                world
                    .matters_in(home)
                    .into_iter()
                    .find(|matter| matter["key"] == key(id))
                    .map(|matter| json!({ "home": home, "placement": matter["placement"] }))
            })
            .collect();
        found.insert(key(id), json!(homes));
    }
    Value::Object(found)
}

/// The ids `ephor feed --unattributed --json` prints.
fn bucket(world: &World) -> Vec<Value> {
    let output = world
        .ephor()
        .args(["feed", "--unattributed", "--json"])
        .output()
        .expect("read the unattributed feed");
    shaped("feed", &output)
        .as_array()
        .expect("the feed is an array")
        .iter()
        .map(|row| row["id"].clone())
        .collect()
}

/// Whether a sentence is the note on `me`'s claim of the project-bound inbox.
/// It is pinned by what it must name, never by its wording: the row, the
/// source, and why the claim places nothing, which is that the source is bound
/// under a project (§FS-008-attribution.4). The source's own name contains
/// `me`, so the row has to be there as a word of its own.
fn names_the_dead_claim(sentence: &str) -> bool {
    let words: Vec<&str> = sentence
        .split(|character: char| {
            !(character.is_alphanumeric() || character == '-' || character == '_')
        })
        .collect();
    words.contains(&"me") && words.contains(&SOURCE) && sentence.to_lowercase().contains("project")
}

/// Every string anywhere in a JSON document.
fn strings(value: &Value) -> Vec<&str> {
    match value {
        Value::String(text) => vec![text.as_str()],
        Value::Array(items) => items.iter().flat_map(strings).collect(),
        Value::Object(fields) => fields.values().flat_map(strings).collect(),
        _ => Vec::new(),
    }
}

/// Every string in an array a JSON document holds under a key named `notes`.
fn notes(value: &Value) -> Vec<&str> {
    match value {
        Value::Array(items) => items.iter().flat_map(notes).collect(),
        Value::Object(fields) => fields
            .iter()
            .flat_map(|(field, inner)| match (field.as_str(), inner) {
                ("notes", Value::Array(_)) => strings(inner),
                _ => notes(inner),
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Both streams of a run, so a case can read what it printed wherever it went.
fn printed(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// What one world shows of the dead claim: the refresh and doctor runs it
/// takes, and the doctor's exit code.
struct Said {
    refresh_prose: String,
    refresh_json: Value,
    doctor_prose: String,
    doctor_json: Value,
    doctor_code: Option<i32>,
}

fn said(world: &World) -> Said {
    let refresh = world.ephor().arg("refresh").output().expect("refresh");
    let refresh_json = world
        .ephor()
        .args(["refresh", "--json"])
        .output()
        .expect("refresh --json");
    let doctor = world
        .ephor()
        .args(["doctor", "--skip-self"])
        .output()
        .expect("doctor");
    let doctor_json = world
        .ephor()
        .args(["doctor", "--skip-self", "--json"])
        .output()
        .expect("doctor --json");
    assert_eq!(
        doctor.status.code(),
        doctor_json.status.code(),
        "doctor's prose and JSON forms must give one verdict"
    );
    Said {
        refresh_prose: printed(&refresh),
        refresh_json: shaped("refresh", &refresh_json),
        doctor_prose: printed(&doctor),
        doctor_json: shaped("doctor", &doctor_json),
        doctor_code: doctor.status.code(),
    }
}

#[test]
fn a_claimed_inbox_keeps_what_names_no_project_and_yields_to_a_reference() {
    let world = mail_world(Binding::Site, Some(SOURCE));
    let refresh = world.ephor().arg("refresh").output().expect("refresh");
    assert!(
        refresh.status.success(),
        "the claim must load and the inbox answer; refresh printed:\n{}",
        printed(&refresh)
    );

    let observed = json!({ "placements": placements(&world), "unattributed": bucket(&world) });
    let expected = json!({
        "placements": {
            key("before-friday"): [{
                "home": "rhei",
                "placement": { "on": { "project": "rhei", "how": "reference" } }
            }],
            key("sunday"): [{
                "home": "me",
                "placement": { "on": { "project": "me", "how": "fallback" } }
            }],
            key("absage"): [{
                "home": "me",
                "placement": { "on": { "project": "me", "how": "fallback" } }
            }]
        },
        "unattributed": []
    });
    assert_eq!(
        observed, expected,
        "a reference must still place its mail on rhei, and the claimed inbox must keep what \
         names no project and what only resembles one, leaving the bucket empty; observed:\n{observed:#}"
    );

    // A claim on a source the site binds for attribution is a working claim,
    // and nothing is said about it.
    let prose = printed(&refresh);
    let noted: Vec<&str> = prose
        .lines()
        .filter(|line| line.starts_with("note:") && line.contains(SOURCE))
        .collect();
    assert!(
        noted.is_empty(),
        "a claim that places mail must not be reported as one that cannot: {noted:?}"
    );
}

#[test]
fn a_claim_on_a_source_bound_under_a_project_is_said_and_places_nothing() {
    // The same inbox under `me` and no claim: what ephor does and says today.
    let control = mail_world(Binding::UnderMe, None);
    let before = said(&control);
    let today = placements(&control);
    for (id, _, _) in MAILS {
        assert_eq!(
            today[key(id)].as_array().map(|homes| homes.len()),
            Some(1),
            "{id} must be in exactly one home; otherwise the fixture is unsound: {today:#}"
        );
        assert_eq!(
            today[key(id)][0]["home"],
            "me",
            "a source bound under me reports me's mail; otherwise the fixture is unsound: {today:#}"
        );
    }
    let already: Vec<&str> = before
        .refresh_prose
        .lines()
        .chain(before.doctor_prose.lines())
        .chain(notes(&before.refresh_json))
        .chain(strings(&before.doctor_json))
        .filter(|sentence| names_the_dead_claim(sentence))
        .collect();
    assert!(
        already.is_empty(),
        "with no claim written, nothing may read as the dead-claim note, or the pin below \
         proves nothing: {already:?}"
    );

    let claimed = mail_world(Binding::UnderMe, Some(SOURCE));
    let after = said(&claimed);
    let refresh_note: Vec<&str> = after
        .refresh_prose
        .lines()
        .filter(|line| line.starts_with("note:") && names_the_dead_claim(line))
        .collect();
    let refresh_json_note: Vec<&str> = notes(&after.refresh_json)
        .into_iter()
        .filter(|note| names_the_dead_claim(note))
        .collect();
    let doctor_note: Vec<&str> = after
        .doctor_prose
        .lines()
        .filter(|line| names_the_dead_claim(line))
        .collect();
    let doctor_json_note: Vec<&str> = strings(&after.doctor_json)
        .into_iter()
        .filter(|sentence| names_the_dead_claim(sentence))
        .collect();
    let observed = json!({
        "refresh note:": !refresh_note.is_empty(),
        "refresh --json notes": !refresh_json_note.is_empty(),
        "doctor": !doctor_note.is_empty(),
        "doctor --json": !doctor_json_note.is_empty(),
        "doctor exit code": after.doctor_code,
        "placements": placements(&claimed),
    });
    let expected = json!({
        "refresh note:": true,
        "refresh --json notes": true,
        "doctor": true,
        "doctor --json": true,
        "doctor exit code": before.doctor_code,
        "placements": today,
    });
    assert_eq!(
        observed, expected,
        "a claim on a source bound under a project must be said by refresh and doctor, naming \
         the row, the source and that the source is bound under a project, without moving \
         doctor's exit code or any mail; observed:\n{observed:#}\nrefresh printed:\n{}\n\
         doctor printed:\n{}",
        after.refresh_prose, after.doctor_prose
    );
}
