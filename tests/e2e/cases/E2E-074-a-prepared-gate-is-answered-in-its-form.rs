//! E2E-074-a-prepared-gate-is-answered-in-its-form: a gate the workflow
//! prepared is briefed in the review, answered through the runtime as a dry run
//! and then for real, and refused in the runtime's own words; where the runtime
//! prepares no briefs, every surface reads as it did (§FS-005-dispatch.9.3).
//!
//! The runtime here is a stub that replays the gate contract's JSON — a brief
//! for `show`, a verdict for `answer` — and keeps what it was handed, so the
//! case can hold ephor to the one thing it does with an answer: pass it on
//! unchanged. Where the ticket goes is the runtime's to say, and the stub says
//! it; ephor maps nothing (§FS-005-dispatch.9.3). The review's JSON and the
//! answer's JSON are read through the published shapes (§REQ-002-parity.4).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

const ITEM: &str = "gdev:storage";
const PLAN_ID: &str = "inbox";
const TICKET: &str = "inbox.2";

/// A runtime that answers `states` with one gating state and, unless the case
/// made it an older one, the gate verbs, from the files beside it.
const RUNTIME: &str = r#"#!/bin/sh
data="$(dirname "$0")/../gate-data"
case "$1" in
  states) cat <<'JSON'
{"states":[
  {"name":"vetting","gating":true,"description":"A person vets the plan"},
  {"name":"apply","description":"An agent applies the ruling"},
  {"name":"cancelled","final":true,"description":"Dropped"}
 ],
 "transitions":[{"from":"vetting","to":"apply"},{"from":"vetting","to":"cancelled"}]}
JSON
  ;;
  transition) printf '%s\n' "$@" > "$data/transition-invocation" ;;
  gate)
    [ -f "$data/older" ] && { echo "error: unrecognized subcommand 'gate'" >&2; exit 2; }
    case "$2" in
      schema) echo '{"version":1,"schemas":{"brief":{},"answer":{},"verdict":{}}}' ;;
      list)
        printf '%s\n' "$@" > "$data/list-invocation"
        cat "$data/listing.json" ;;
      show)
        printf '%s\n' "$@" > "$data/show-invocation"
        [ -f "$data/no-brief" ] && { echo "inbox.2 is not waiting at a gate" >&2; exit 1; }
        cat "$data/brief.json" ;;
      answer)
        printf '%s\n' "$@" > "$data/answer-invocation"
        while [ $# -gt 0 ]; do
          [ "$1" = --answers ] && cp "$2" "$data/answers-seen.json"
          shift
        done
        cat "$data/verdict.json"
        [ -f "$data/refuse" ] && exit 1
        exit 0 ;;
    esac ;;
  *) exit 0 ;;
esac
"#;

/// A world holding one matter whose ledger names one plan, its one ticket
/// waiting at the gate `vetting`; the runtime is `older` where asked.
fn site(older: bool) -> World {
    let world = World::new();
    let stub = world.stub("gate-runtime", RUNTIME);
    settle(&stub);
    world.configure(json!({"work": {"runner": "gate-runtime"}}));
    let data = world.path().join("gate-data");
    std::fs::create_dir_all(&data).unwrap();
    if older {
        std::fs::write(data.join("older"), "").unwrap();
    }
    world.file(
        "panta/states.yaml",
        "name: inbox\nstates:\n  vetting:\n    gating: true\n  apply: {}\n  cancelled:\n    final: true\n",
    );
    let plan = world.file(
        &format!("panta/{PLAN_ID}.rhei.md"),
        "# Rhei: Inbox Sample\n\n**States:** states.yaml\n\n## Tasks\n\n\
         ### Task 2: Vet the storage migration\n\n**State:** vetting\n",
    );
    let display = world.file(
        "panta/runtime/drafts/inbox.2.md",
        "# The migration\n\nTwo tables move.\n",
    );
    write_json(
        &data.join("brief.json"),
        &json!({
            "version": 1, "ticket": TICKET, "ticket_title": "Vet the storage migration",
            "title": "Vet the storage migration", "state": "vetting", "visit": 1,
            "since": "2026-11-24T16:59:22Z", "plan": plan, "root": world.forest().join("panta"),
            "source": "static", "summary": "Vet the storage migration, in Inbox Sample",
            "display": {"path": "runtime/drafts/inbox.2.md", "absolute": display,
                        "readable": true, "problem": null},
            "attachments": [],
            "questions": [
                {"id": "approve", "kind": "boolean", "prompt": "Go ahead?", "required": true},
                {"id": "scope", "kind": "choice", "prompt": "How much of it?", "required": true,
                 "options": [{"value": "small", "label": "The first table only"},
                             {"value": "all", "label": "Every table"}]},
                {"id": "note", "kind": "text", "prompt": "Anything to respect?", "required": false}
            ],
            "destinations": [
                {"state": "apply", "description": "An agent applies the ruling",
                 "condition": null, "final": false},
                {"state": "cancelled", "description": "Dropped", "condition": null, "final": true}
            ],
            "routes": [{"to": "apply", "when": {"approve": true}}, {"to": "cancelled"}],
            "digest": "sha256:3f59537b",
            "answer_path": "runtime/gates/inbox.2/vetting.1.answer.json",
            "problems": []
        }),
    );
    write_json(
        &data.join("listing.json"),
        &json!({"version": 1, "root": plan, "gates": [
            {"ticket": TICKET, "ticket_title": "Vet the storage migration",
             "title": "Vet the storage migration", "state": "vetting", "plan": plan,
             "root": world.forest().join("panta"), "visit": 1,
             "since": "2026-11-24T16:59:22Z", "source": "static", "required": 2,
             "holds": 6, "problems": []}
        ]}),
    );
    let at = "2026-11-24T12:00:00Z";
    std::fs::create_dir_all(world.path().join("state/ephor/feed")).unwrap();
    write_json(
        &world
            .path()
            .join("state/ephor/feed")
            .join(format!("{PROJECT}.json")),
        &json!({"model": 3, "project": PROJECT, "fetched_at": at,
            "providers": {"gdev": {"fetched_at": at, "ok": true, "matters": [{
                "key": ITEM, "kind": "pr", "placement": {"on": {"project": PROJECT}},
                "source": "gdev", "title": "Storage migration", "needs_response": false,
                "updated_at": at, "state": "open"}]}}}),
    );
    write_json(
        &world.path().join("state/ephor/work.json"),
        &json!({"version": 1, "entries": {ITEM: {
            "project": PROJECT, "title": "Storage migration",
            "root": world.forest().join("panta"), "checkout": world.forest(),
            "plan_id": PLAN_ID, "plan": plan,
            "dispatches": [{"ticket": "2", "recipe": "fix-gate", "at": at,
                            "snapshot": {"updated_at": at, "state": "open"}}]}}}),
    );
    world
}

fn data(world: &World, name: &str) -> std::path::PathBuf {
    world.path().join("gate-data").join(name)
}

fn verdict(world: &World, verdict: Value) {
    write_json(&data(world, "verdict.json"), &verdict);
}

fn invocation(world: &World, name: &str) -> Vec<String> {
    std::fs::read_to_string(data(world, name))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn answer_args(ticket: &str, file: &str) -> Vec<String> {
    [
        "work",
        "gate",
        "answer",
        "--item",
        ITEM,
        "--ticket",
        ticket,
        "--from",
        "vetting",
        "--answers",
        file,
    ]
    .iter()
    .map(|arg| arg.to_string())
    .collect()
}

/// The qualified ticket the review lists, which the commands take.
fn ticket_of(review: &Value) -> String {
    review["gates"][0]["ticket"].as_str().unwrap().to_string()
}

/// The review carries the brief per gate, a typed gate is not moved by a
/// reason alone, and the answer goes to the runtime unchanged — as a dry run,
/// then for real from standard input (§FS-005-dispatch.9.3).
#[test]
fn a_prepared_gate_is_briefed_dry_run_and_answered_through_the_runtime() {
    let world = site(false);
    let output = world
        .ephor()
        .args(["work", "review", "--item", ITEM, "--json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let review = shaped("work-review", &output);
    let ticket = ticket_of(&review);
    let brief = &review["gates"][0]["brief"];
    assert_eq!(brief["title"], "Vet the storage migration", "{review:#}");
    assert_eq!(brief["digest"], "sha256:3f59537b");
    assert_eq!(brief["questions"].as_array().unwrap().len(), 3);
    assert_eq!(brief["questions"][2]["required"], false);
    assert_eq!(brief["destinations"][1]["final"], true);
    // What the gate holds, from one listing of its plan.
    assert_eq!(review["gates"][0]["holds"], 6, "{review:#}");
    assert_eq!(
        invocation(&world, "list-invocation")[..3],
        [
            "gate",
            "list",
            &world
                .forest()
                .join("panta/inbox.rhei.md")
                .display()
                .to_string()
        ]
    );
    let shown = invocation(&world, "show-invocation");
    assert_eq!(
        shown[..5],
        [
            "gate",
            "show",
            &world
                .forest()
                .join("panta/inbox.rhei.md")
                .display()
                .to_string(),
            "--task",
            &ticket
        ]
    );

    let text = world
        .ephor()
        .args(["work", "review", "--item", ITEM])
        .output()
        .unwrap();
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(
        text.contains("brief: Vet the storage migration · 3 question(s)"),
        "{text}"
    );
    assert!(text.contains(" · holds 6 tasks · "), "{text}");
    assert!(text.contains("ephor work gate answer"), "{text}");

    // A reason alone would be refused by the runtime; ephor names the way
    // that works, and nothing is asked of the runtime.
    let moved = world
        .ephor()
        .args([
            "work",
            "transition",
            "--item",
            ITEM,
            "--ticket",
            &ticket,
            "--from",
            "vetting",
            "--to",
            "apply",
            "--result",
            "Looks fine",
        ])
        .output()
        .unwrap();
    assert!(!moved.status.success(), "{moved:?}");
    let said = String::from_utf8_lossy(&moved.stderr);
    assert!(said.contains("asks questions of its own"), "{said}");
    assert!(said.contains("ephor work gate answer"), "{said}");
    assert!(!data(&world, "transition-invocation").exists());

    // The dry run: the document reaches the runtime exactly as written, and
    // the verdict is the runtime's.
    let sent = json!({"version": 1, "request_digest": "sha256:3f59537b",
                      "answers": {"approve": true, "scope": "small"}});
    let file = world.file("answers.json", &sent.to_string());
    let said = json!({
        "version": 1, "ok": true, "dry_run": true, "ticket": ticket, "from": "vetting",
        "to": "apply", "route": {"index": 0, "when": {"approve": true}},
        "answer_path": "runtime/gates/inbox.2/vetting.1.answer.json",
        "result": "Go ahead?: yes\nHow much of it?: The first table only", "errors": [],
        "a_field_a_later_runtime_adds": {"kept": true}
    });
    verdict(&world, said.clone());
    let args = answer_args(&ticket, &file.display().to_string());
    let dry = world
        .ephor()
        .args(&args)
        .args(["--dry-run", "--json"])
        .output()
        .unwrap();
    assert!(dry.status.success(), "{dry:?}");
    let dry = shaped("work-gate-answer", &dry);
    // The verdict whole, every field under the runtime's own name, with the
    // matter beside it.
    let mut whole = said.clone();
    whole["item"] = ITEM.into();
    assert_eq!(dry, whole);
    assert_eq!(dry["ok"], true);
    assert_eq!(dry["dry_run"], true);
    assert_eq!(dry["to"], "apply");
    assert_eq!(dry["route"]["when"], json!({"approve": true}));
    assert_eq!(read_json(&data(&world, "answers-seen.json")), sent);
    let asked = invocation(&world, "answer-invocation");
    assert_eq!(asked[..2], ["gate", "answer"]);
    let from = asked.iter().position(|arg| arg == "--from").unwrap();
    assert_eq!(asked[from + 1], "vetting");
    assert!(asked.contains(&"--dry-run".to_string()), "{asked:?}");
    let private = asked[asked.iter().position(|arg| arg == "--answers").unwrap() + 1].clone();
    assert!(
        !std::path::Path::new(&private).exists(),
        "the private copy is gone"
    );
    assert!(!private.starts_with(&world.forest().display().to_string()));

    // For real, from standard input: the same call without the flag.
    verdict(
        &world,
        json!({
            "version": 1, "ok": true, "dry_run": false, "ticket": ticket, "from": "vetting",
            "to": "apply", "route": {"index": 0, "when": {"approve": true}},
            "answer_path": "runtime/gates/inbox.2/vetting.1.answer.json",
            "result": "Go ahead?: yes", "errors": []
        }),
    );
    let args = answer_args(&ticket, "-");
    let done = world
        .ephor()
        .args(&args)
        .write_stdin(sent.to_string())
        .output()
        .unwrap();
    assert!(done.status.success(), "{done:?}");
    let said = String::from_utf8(done.stdout).unwrap();
    assert!(said.contains("vetting → apply"), "{said}");
    assert!(!invocation(&world, "answer-invocation").contains(&"--dry-run".to_string()));
    assert_eq!(read_json(&data(&world, "answers-seen.json")), sent);
}

/// A refusal is the runtime's verdict, every field it leaves unknown read as
/// unknown, and the command exits non-zero saying why (§FS-005-dispatch.9.3).
#[test]
fn a_refused_answer_says_the_runtimes_reasons() {
    let world = site(false);
    let review = shaped(
        "work-review",
        &world
            .ephor()
            .args(["work", "review", "--item", ITEM, "--json"])
            .output()
            .unwrap(),
    );
    let ticket = ticket_of(&review);
    std::fs::write(data(&world, "refuse"), "").unwrap();
    verdict(
        &world,
        json!({
            "version": 1, "ok": false, "dry_run": true, "ticket": ticket, "from": "vetting",
            "to": null, "route": null, "answer_path": null, "result": null,
            "errors": [
                {"kind": "invalid", "question": "approve",
                 "message": "`approve` is a boolean question; answer true or false"},
                {"kind": "stale", "question": null,
                 "message": "the brief changed since it was read"}
            ]
        }),
    );
    let file = world.file(
        "answers.json",
        r#"{"version":1,"request_digest":"sha256:0000","answers":{"approve":"yes"}}"#,
    );
    let args = answer_args(&ticket, &file.display().to_string());
    let refused = world
        .ephor()
        .args(&args)
        .args(["--dry-run", "--json"])
        .output()
        .unwrap();
    assert!(!refused.status.success(), "{refused:?}");
    let refused = shaped("work-gate-answer", &refused);
    assert_eq!(refused["ok"], false);
    assert_eq!(
        (&refused["to"], &refused["route"], &refused["answer_path"]),
        (&Value::Null, &Value::Null, &Value::Null)
    );
    assert_eq!(refused["errors"][0]["question"], "approve");
    assert_eq!(refused["errors"][1]["question"], Value::Null);
    let says = refused["says"].as_str().unwrap();
    assert!(
        says.contains("approve: `approve` is a boolean question"),
        "{says}"
    );

    let text = world.ephor().args(&args).arg("--dry-run").output().unwrap();
    assert!(!text.status.success());
    let shown = format!(
        "{}{}",
        String::from_utf8_lossy(&text.stdout),
        String::from_utf8_lossy(&text.stderr)
    );
    assert!(
        shown.contains("the brief changed since it was read"),
        "{shown}"
    );
}

/// A gate the runtime could not brief keeps its review and its plain move,
/// with the runtime's sentence for why it has no form (§FS-005-dispatch.9.3).
#[test]
fn a_gate_without_a_brief_is_moved_as_before() {
    let world = site(false);
    std::fs::write(data(&world, "no-brief"), "").unwrap();
    let review = shaped(
        "work-review",
        &world
            .ephor()
            .args(["work", "review", "--item", ITEM, "--json"])
            .output()
            .unwrap(),
    );
    let gate = &review["gates"][0];
    assert!(gate.get("brief").is_none(), "{review:#}");
    assert!(
        gate["unbriefed"]
            .as_str()
            .unwrap()
            .contains("not waiting at a gate"),
        "{review:#}"
    );
    let ticket = ticket_of(&review);
    let moved = world
        .ephor()
        .args([
            "work",
            "transition",
            "--item",
            ITEM,
            "--ticket",
            &ticket,
            "--from",
            "vetting",
            "--to",
            "apply",
            "--result",
            "Looks fine",
        ])
        .output()
        .unwrap();
    assert!(moved.status.success(), "{moved:?}");
    assert_eq!(invocation(&world, "transition-invocation")[0], "transition");
}

/// An older runtime, without the gate verbs: the review reads exactly as it
/// did, a transition moves the gate as it did, and the answer command refuses
/// and names the transition (§FS-005-dispatch.9.3).
#[test]
fn an_older_runtime_changes_nothing() {
    let world = site(true);
    let review = shaped(
        "work-review",
        &world
            .ephor()
            .args(["work", "review", "--item", ITEM, "--json"])
            .output()
            .unwrap(),
    );
    let gate = &review["gates"][0];
    assert!(
        gate.get("brief").is_none() && gate.get("unbriefed").is_none(),
        "{review:#}"
    );
    assert!(gate.get("holds").is_none(), "{review:#}");
    assert!(
        !data(&world, "list-invocation").exists(),
        "nothing is listed"
    );
    assert!(
        !data(&world, "show-invocation").exists(),
        "no brief is asked for"
    );
    let text = world
        .ephor()
        .args(["work", "review", "--item", ITEM])
        .output()
        .unwrap();
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(!text.contains("brief"), "{text}");
    assert!(
        text.contains("  → apply — An agent applies the ruling"),
        "{text}"
    );

    let ticket = ticket_of(&review);
    let file = world.file(
        "answers.json",
        r#"{"version":1,"request_digest":"sha256:0","answers":{}}"#,
    );
    let args = answer_args(&ticket, &file.display().to_string());
    let refused = world.ephor().args(&args).output().unwrap();
    assert!(!refused.status.success(), "{refused:?}");
    let said = String::from_utf8_lossy(&refused.stderr);
    assert!(said.contains("prepares no gate briefs"), "{said}");
    assert!(said.contains("ephor work transition"), "{said}");
    assert!(!data(&world, "answer-invocation").exists());
    // Under `--json`, a refusal ephor gives before asking is made of what was
    // asked, in the published shape.
    let refused = world.ephor().args(&args).arg("--json").output().unwrap();
    assert!(!refused.status.success(), "{refused:?}");
    let refused = shaped("work-gate-answer", &refused);
    assert_eq!(
        (&refused["item"], &refused["ticket"], &refused["from"]),
        (&json!(ITEM), &json!(ticket), &json!("vetting"))
    );
    assert_eq!(
        (&refused["ok"], &refused["errors"]),
        (&json!(false), &json!([]))
    );
    assert!(refused.get("version").is_none(), "{refused:#}");
    assert!(refused["says"]
        .as_str()
        .unwrap()
        .contains("prepares no gate briefs"));

    let moved = world
        .ephor()
        .args([
            "work",
            "transition",
            "--item",
            ITEM,
            "--ticket",
            &ticket,
            "--from",
            "vetting",
            "--to",
            "apply",
            "--result",
            "Looks fine",
        ])
        .output()
        .unwrap();
    assert!(moved.status.success(), "{moved:?}");
    assert_eq!(invocation(&world, "transition-invocation")[0], "transition");
}
