//! E2E-075-the-guide-is-asked-of-the-ephor-that-reads: `ephor guide` lists its
//! topics and refuses one it does not know (§FS-020-guide.1); `gate` is spelled
//! with the runner the site binds and ends in a worked example and the commands
//! that check it (§FS-020-guide.2); `matter` names no runtime (§FS-020-guide.3);
//! and under `--json` the gate topic carries the runtime's own request and
//! answer schemas, unchanged, or the sentence saying why there are none
//! (§FS-020-guide.4).
//!
//! The runtime is a stub whose `gate schema` prints two schemas with marks of
//! its own, so the case can tell a schema the runtime printed from one ephor
//! might have kept; made `older`, it has no gate verbs at all.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

/// A runtime that, unless the case made it an older one, prints its gate
/// schemas.
const RUNTIME: &str = r#"#!/bin/sh
data="$(dirname "$0")/../gate-data"
case "$1" in
  gate)
    [ -f "$data/older" ] && { echo "error: unrecognized subcommand 'gate'" >&2; exit 2; }
    case "$2" in
      schema) printf '%s' '{"version":1,"schemas":{
        "request":{"$id":"acme.gate.request.v1","title":"the stub request"},
        "answer":{"$id":"acme.gate.answer.v1","title":"the stub answer"},
        "show":{},"result":{},"list":{}}}' ;;
      *) exit 1 ;;
    esac ;;
  *) exit 0 ;;
esac
"#;

fn site(older: bool) -> World {
    let world = World::new();
    settle(&world.stub("acme-runtime", RUNTIME));
    world.configure(json!({"work": {"runner": "acme-runtime"}}));
    let data = world.path().join("gate-data");
    std::fs::create_dir_all(&data).unwrap();
    if older {
        std::fs::write(data.join("older"), "").unwrap();
    }
    world
}

fn guide(world: &World, args: &[&str]) -> (String, Option<Value>) {
    let out = world
        .ephor()
        .arg("guide")
        .args(args)
        .output()
        .expect("ephor guide");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = args.contains(&"--json").then(|| shaped("guide", &out));
    (String::from_utf8_lossy(&out.stdout).into_owned(), json)
}

/// The topics, and a topic nobody wrote refused by the names there are
/// (§FS-020-guide.1).
#[test]
fn the_guide_lists_its_topics_and_refuses_one_it_does_not_know() {
    let world = site(false);
    let (text, _) = guide(&world, &[]);
    assert!(text.contains("gate "), "{text}");
    assert!(text.contains("matter "), "{text}");
    assert!(text.contains("ephor guide <topic>"), "{text}");
    let (_, view) = guide(&world, &["--json"]);
    let view = view.unwrap();
    assert!(view.get("topic").is_none(), "{view}");
    let names: Vec<&str> = view["topics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|topic| topic["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["gate", "matter"]);

    let out = world.ephor().args(["guide", "nope"]).output().unwrap();
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("no guide topic 'nope'; the topics are gate, matter"),
        "{said}"
    );
}

/// The gate topic speaks the bound runner's name in every command, ends with
/// the example and the checks, and under `--json` carries the runtime's own
/// schemas as it printed them (§FS-020-guide.2, §FS-020-guide.4).
#[test]
fn the_gate_topic_is_the_bound_runtimes_with_its_own_schemas() {
    let world = site(false);
    let (text, _) = guide(&world, &["gate"]);
    for line in [
        "acme-runtime validate notes.rhei.md",
        "acme-runtime gate show notes.rhei.md --task notes.1 --json",
        "acme-runtime gate schema request",
        "ephor work review --item <id> --json",
        "ephor work gate answer --item <id>",
        "--dry-run",
        "routes:",
        "{gate_answer}",
    ] {
        assert!(text.contains(line), "{line} missing:\n{text}");
    }
    assert!(!text.contains("No schemas"), "{text}");
    let (_, view) = guide(&world, &["gate", "--json"]);
    let view = view.unwrap();
    assert_eq!(view["topic"], "gate");
    assert_eq!(
        view["text"].as_str().unwrap(),
        text.split("\n`ephor guide").next().unwrap()
    );
    assert_eq!(
        view["schemas"],
        json!({
            "request": {"$id": "acme.gate.request.v1", "title": "the stub request"},
            "answer": {"$id": "acme.gate.answer.v1", "title": "the stub answer"}
        })
    );
    assert!(view.get("schemas_unavailable").is_none(), "{view}");
}

/// A runtime without the gate verbs has no schemas to give: the prose is
/// still the answer, and the sentence says why there is nothing more
/// (§FS-020-guide.4).
#[test]
fn an_older_runtime_gets_the_prose_and_the_reason_there_are_no_schemas() {
    let world = site(true);
    let (text, _) = guide(&world, &["gate"]);
    assert!(text.contains("acme-runtime gate show"), "{text}");
    assert!(
        text.contains("No schemas: acme-runtime prepares no gate briefs"),
        "{text}"
    );
    let (_, view) = guide(&world, &["gate", "--json"]);
    let view = view.unwrap();
    assert!(view.get("schemas").is_none(), "{view}");
    assert!(
        view["schemas_unavailable"]
            .as_str()
            .unwrap()
            .contains("prepares no gate briefs"),
        "{view}"
    );
}

/// The matter topic is ephor's own: it names no runtime, and says what makes a
/// matter new, what processed means and how a workflow becomes the proposal
/// (§FS-020-guide.3).
#[test]
fn the_matter_topic_is_ephors_own() {
    let world = site(false);
    let (text, view) = guide(&world, &["matter", "--json"]);
    let view = view.unwrap();
    let text_of = view["text"].as_str().unwrap();
    assert!(!text.is_empty());
    assert!(!text_of.contains("acme-runtime"), "{text_of}");
    for part in [
        "held",
        "processed",
        "proposal",
        "\"workflow\"",
        "ephor work dismiss",
    ] {
        assert!(text_of.contains(part), "{part} missing:\n{text_of}");
    }
    assert!(view.get("schemas").is_none() && view.get("schemas_unavailable").is_none());
}
