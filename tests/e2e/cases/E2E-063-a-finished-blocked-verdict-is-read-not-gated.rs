//! E2E-063-a-finished-blocked-verdict-is-read-not-gated: a ticket that finished
//! by saying a person must act first reads as finished, and the reason it wrote
//! is one open away from both the list and the review (§FS-005-dispatch.9.2,
//! §FS-005-dispatch.23).
//!
//! The scenario is the one ephor#243 reported. A matter's plan is over: both of
//! its `fix-gate` tickets are in the final `done`, and each wrote the shipped
//! machine's verdict `blocked — a person has to decide or do something first`,
//! with the actual reason — collection could not run because the VPN was down —
//! in the verdict's body and in the result. `work list` printed the verdict's
//! imperative sentence as though a person were being waited on now, while
//! `work review` said no work waits at a human gate and offered nothing to open.
//! The reader was sent to a gate that does not exist, and the reason stayed in
//! the plan's runtime directory.
//!
//! What this case holds ephor to. The list says the outcome is *finished
//! blocked* and prints a path to a readable verdict carrying the reason; its
//! JSON keeps `state`, `finished` and the raw `verdict` as they were and adds
//! `outcome_files` (§REQ-002-parity.3). The review keeps `gates` empty and its
//! no-gate sentence, and lists each ticket's verdict and result, labelled
//! `finished blocked`, in plan order, read through `--file N` like any review
//! file (§FS-005-dispatch.9.1). A finished ticket with no readable verdict or a
//! verdict other than `blocked` is presented exactly as before, and a live gate
//! beside a finished blocked ticket stays actionable, its materials first.
//!
//! The reproduction behind the ticket built this site in a shell script with
//! the real runtime; this is that site in the project's own frame, with a stub
//! runtime that answers `states` with the shipped machine's shape.

#[path = "../support.rs"]
mod support;

use std::path::PathBuf;

use serde_json::{json, Value};

use support::*;

const ITEM: &str = "gdev:graal";
const PLAN_ID: &str = "gdev-graal-24550-e13a1739";
const REASON: &str =
    "Collection could not run because the VPN was down; a person must restore the VPN and re-run collect.";
const BLOCKED: &str = "blocked — a person has to decide or do something first";
const NO_GATE: &str = "No work is waiting at a human gate for this matter.";

/// The effective machine the runtime reports: `done` final and not gating,
/// `human-gate` gating with one way out.
const RUNTIME: &str = r#"#!/bin/sh
case "$1" in
  states) cat <<'JSON'
{"states":[
  {"name":"todo"},
  {"name":"human-gate","gating":true,"description":"Waiting on a person"},
  {"name":"done","final":true,"description":"Finished"},
  {"name":"cancelled","final":true}
 ],
 "transitions":[{"from":"human-gate","to":"done"},{"from":"todo","to":"done"}]}
JSON
  ;;
  *) exit 0 ;;
esac
"#;

/// One ticket of the plan: its id, its state, and the verdict line it wrote,
/// if any.
struct Ticket {
    id: &'static str,
    state: &'static str,
    verdict: Option<&'static str>,
}

/// A world holding one matter whose ledger names one plan with these tickets.
fn site(tickets: &[Ticket]) -> World {
    let world = World::new();
    world.stub("gate-runtime", RUNTIME);
    world.configure(json!({"work": {"runner": "gate-runtime"}}));
    let mut plan = String::from("# Rhei: Finished collection attempt\n\n**State:** done\n**States:** states.yaml\n\n## Tasks\n");
    for ticket in tickets {
        plan.push_str(&format!(
            "\n### Task {}: Gate cannot be collected\n\n**State:** {}\n",
            ticket.id, ticket.state
        ));
        let qualified = format!("{PLAN_ID}.{}", ticket.id);
        if let Some(verdict) = ticket.verdict {
            world.file(
                &format!("panta/runtime/ephor/{qualified}.verdict.md"),
                &format!("VERDICT: {verdict}\n\n{REASON}\n"),
            );
        }
        if ticket.state == "done" {
            world.file(
                &format!("panta/runtime/results/{qualified}.md"),
                &format!("## Result\n\n{REASON}\n"),
            );
        }
    }
    world.file(
        "panta/states.yaml",
        "name: work\nstates:\n  todo: {}\n  human-gate:\n    gating: true\n  done:\n    final: true\n",
    );
    let plan_path = world.file(&format!("panta/{PLAN_ID}.rhei.md"), &plan);
    let at = "2026-10-22T12:00:00Z";
    std::fs::create_dir_all(world.path().join("state/ephor/feed")).unwrap();
    write_json(
        &world
            .path()
            .join("state/ephor/feed")
            .join(format!("{PROJECT}.json")),
        &json!({"model": 2, "project": PROJECT, "fetched_at": at,
            "providers": {"gdev": {"fetched_at": at, "ok": true, "matters": [{
                "key": ITEM, "kind": "pr", "placement": {"on": {"project": PROJECT}},
                "source": "gdev", "title": "Gate cannot be collected", "needs_response": false,
                "updated_at": at, "state": "open"}]}}}),
    );
    write_json(
        &world.path().join("state/ephor/work.json"),
        &json!({"version": 1, "entries": {ITEM: {
            "project": PROJECT, "title": "Gate cannot be collected",
            "root": world.forest().join("panta"), "checkout": world.forest(),
            "plan_id": PLAN_ID, "plan": plan_path,
            "dispatches": tickets.iter().map(|ticket| json!({
                "ticket": ticket.id, "recipe": "fix-gate", "at": at,
                "snapshot": {"updated_at": at, "state": "open"}})).collect::<Vec<_>>()}}}),
    );
    world
}

fn verdict_path(world: &World, ticket: &str) -> PathBuf {
    world
        .forest()
        .join(format!("panta/runtime/ephor/{PLAN_ID}.{ticket}.verdict.md"))
}

fn result_path(world: &World, ticket: &str) -> PathBuf {
    world
        .forest()
        .join(format!("panta/runtime/results/{PLAN_ID}.{ticket}.md"))
}

fn stdout(world: &World, args: &[&str]) -> String {
    let output = world.ephor().args(args).output().unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn json(world: &World, shape: &str, args: &[&str]) -> Value {
    let output = world.ephor().args(args).output().unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    shaped(shape, &output)
}

fn tickets_of(list: &Value) -> Vec<Value> {
    list.as_array()
        .unwrap()
        .iter()
        .find(|row| row["item"] == ITEM)
        .unwrap_or_else(|| panic!("no {ITEM} in the work list: {list:#}"))["tickets"]
        .as_array()
        .unwrap()
        .clone()
}

fn files_of(review: &Value) -> Vec<(String, PathBuf, Value)> {
    review["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| {
            (
                file["name"].as_str().unwrap().to_string(),
                PathBuf::from(file["path"].as_str().unwrap()),
                file["problem"].clone(),
            )
        })
        .collect()
}

/// The reported site: the plan and both tickets final, both verdicts
/// `blocked`, no live gate anywhere (§FS-005-dispatch.9.2, §FS-005-dispatch.23).
#[test]
fn a_finished_blocked_verdict_reads_as_finished_and_its_reason_is_one_open_away() {
    let world = site(&[
        Ticket {
            id: "fix-gate-1",
            state: "done",
            verdict: Some(BLOCKED),
        },
        Ticket {
            id: "fix-gate-2",
            state: "done",
            verdict: Some(BLOCKED),
        },
    ]);

    // The list says the outcome is finished, and names a readable verdict
    // carrying the reason (§FS-005-dispatch.23).
    let listed = stdout(&world, &["work", "list"]);
    assert!(
        listed.contains("finished blocked"),
        "the list must say the blocked outcome is finished:\n{listed}"
    );
    let latest = verdict_path(&world, "fix-gate-2");
    assert!(
        listed.contains(&latest.to_string_lossy().to_string()),
        "the list must print the latest ticket's verdict path {}:\n{listed}",
        latest.display()
    );
    assert!(std::fs::read_to_string(&latest).unwrap().contains(REASON));

    // Its JSON keeps every existing fact and carries the same files
    // (§REQ-002-parity.3).
    let list = json(&world, "work-list", &["work", "list", "--json"]);
    let tickets = tickets_of(&list);
    assert_eq!(tickets.len(), 2, "{tickets:#?}");
    for ticket in &tickets {
        assert_eq!(ticket["state"], "done");
        assert_eq!(ticket["finished"], true);
        assert_eq!(ticket["cancelled"], false);
        assert_eq!(ticket["verdict"], BLOCKED, "the raw verdict is unchanged");
        let id = ticket["id"].as_str().unwrap();
        let qualified = format!("{PLAN_ID}.{id}");
        assert_eq!(
            ticket["outcome_files"],
            json!([
                {"name": format!("{qualified} · finished blocked · verdict"),
                 "path": verdict_path(&world, id), "problem": null},
                {"name": format!("{qualified} · finished blocked · result"),
                 "path": result_path(&world, id), "problem": null},
            ]),
            "{ticket:#}"
        );
    }

    // The review keeps no gate and lists the history, in plan order
    // (§FS-005-dispatch.9.2).
    let review = json(
        &world,
        "work-review",
        &["work", "review", "--item", ITEM, "--json"],
    );
    assert_eq!(
        review["gates"],
        json!([]),
        "a finished ticket is never a gate"
    );
    let files = files_of(&review);
    let expected: Vec<(String, PathBuf, Value)> = ["fix-gate-1", "fix-gate-2"]
        .iter()
        .flat_map(|id| {
            let qualified = format!("{PLAN_ID}.{id}");
            [
                (
                    format!("{qualified} · finished blocked · verdict"),
                    verdict_path(&world, id),
                    Value::Null,
                ),
                (
                    format!("{qualified} · finished blocked · result"),
                    result_path(&world, id),
                    Value::Null,
                ),
            ]
        })
        .collect();
    assert_eq!(files, expected);

    let reviewed = stdout(&world, &["work", "review", "--item", ITEM]);
    assert!(
        reviewed.contains(NO_GATE),
        "the no-gate sentence stays:\n{reviewed}"
    );
    assert!(
        reviewed.contains(&format!(
            "file 3: {PLAN_ID}.fix-gate-2 · finished blocked · verdict"
        )),
        "the finished files are listed under it:\n{reviewed}"
    );
    assert!(
        !reviewed.contains("→"),
        "no destination is offered:\n{reviewed}"
    );

    // Opening one goes through the ordinary bounded viewer.
    let opened = stdout(&world, &["work", "review", "--item", ITEM, "--file", "3"]);
    assert!(opened.contains(REASON), "{opened}");
    let opened = json(
        &world,
        "work-review-file",
        &["work", "review", "--item", ITEM, "--file", "4", "--json"],
    );
    assert!(
        opened["text"].as_str().unwrap().contains(REASON),
        "{opened:#}"
    );

    // Reading moved nothing.
    let plan =
        std::fs::read_to_string(world.forest().join(format!("panta/{PLAN_ID}.rhei.md"))).unwrap();
    assert_eq!(plan.matches("**State:** done").count(), 3);
}

/// Only an explicit `blocked` verdict changes anything: a finished ticket with
/// no readable verdict, or with another verdict, is presented as before
/// (§FS-005-dispatch.9.2). A guard rather than a red test: it holds today and
/// must still hold after the change.
#[test]
fn a_finished_ticket_without_a_blocked_verdict_is_presented_as_before() {
    let world = site(&[
        Ticket {
            id: "fix-gate-1",
            state: "done",
            verdict: None,
        },
        Ticket {
            id: "fix-gate-2",
            state: "done",
            verdict: Some("fixed — the gate is green"),
        },
    ]);
    let listed = stdout(&world, &["work", "list"]);
    assert!(!listed.contains("finished blocked"), "{listed}");
    assert!(listed.contains("fixed — the gate is green"), "{listed}");
    assert!(!listed.contains("verdict.md"), "{listed}");

    let list = json(&world, "work-list", &["work", "list", "--json"]);
    for ticket in tickets_of(&list) {
        assert!(ticket.get("outcome_files").is_none(), "{ticket:#}");
    }

    let review = json(
        &world,
        "work-review",
        &["work", "review", "--item", ITEM, "--json"],
    );
    assert_eq!(review, json!({"gates": [], "files": []}));
    let reviewed = stdout(&world, &["work", "review", "--item", ITEM]);
    assert_eq!(reviewed, format!("{NO_GATE}\n"));
}

/// A live gate beside a finished blocked ticket stays actionable, and its
/// materials come first; the history after it is read-only
/// (§FS-005-dispatch.9.1, §FS-005-dispatch.9.2).
#[test]
fn a_live_gate_stays_actionable_beside_finished_blocked_files() {
    let world = site(&[
        Ticket {
            id: "fix-gate-1",
            state: "done",
            verdict: Some(BLOCKED),
        },
        Ticket {
            id: "fix-gate-2",
            state: "human-gate",
            verdict: None,
        },
    ]);
    let review = json(
        &world,
        "work-review",
        &["work", "review", "--item", ITEM, "--json"],
    );
    let gates = review["gates"].as_array().unwrap();
    assert_eq!(gates.len(), 1, "{review:#}");
    assert_eq!(gates[0]["ticket"], format!("{PLAN_ID}.fix-gate-2"));
    assert_eq!(gates[0]["state"], "human-gate");
    assert_eq!(gates[0]["destinations"][0]["state"], "done");

    let files = files_of(&review);
    let first_history = files
        .iter()
        .position(|(name, _, _)| name.contains("finished blocked"))
        .unwrap_or_else(|| panic!("no finished blocked file: {files:#?}"));
    assert!(
        first_history > 0,
        "the gate's materials come first: {files:#?}"
    );
    assert!(files[..first_history]
        .iter()
        .all(|(name, _, _)| name.starts_with(&format!("{PLAN_ID}.fix-gate-2 · "))));
    let qualified = format!("{PLAN_ID}.fix-gate-1");
    assert_eq!(
        files[first_history..].to_vec(),
        vec![
            (
                format!("{qualified} · finished blocked · verdict"),
                verdict_path(&world, "fix-gate-1"),
                Value::Null
            ),
            (
                format!("{qualified} · finished blocked · result"),
                result_path(&world, "fix-gate-1"),
                Value::Null
            ),
        ]
    );

    let reviewed = stdout(&world, &["work", "review", "--item", ITEM]);
    assert!(!reviewed.contains(NO_GATE), "{reviewed}");
    assert!(reviewed.contains("→ done"), "{reviewed}");
}

/// A result missing beside a readable blocked verdict is still listed, with
/// its problem (§FS-005-dispatch.9.2).
#[test]
fn a_missing_result_beside_a_blocked_verdict_carries_its_problem() {
    let world = site(&[Ticket {
        id: "fix-gate-1",
        state: "done",
        verdict: Some(BLOCKED),
    }]);
    std::fs::remove_file(result_path(&world, "fix-gate-1")).unwrap();
    let review = json(
        &world,
        "work-review",
        &["work", "review", "--item", ITEM, "--json"],
    );
    let files = files_of(&review);
    assert_eq!(files.len(), 2, "{files:#?}");
    assert!(files[0].0.ends_with("· finished blocked · verdict") && files[0].2.is_null());
    assert!(
        files[1].0.ends_with("· finished blocked · result"),
        "{files:#?}"
    );
    assert!(
        files[1].2.is_string(),
        "the missing result names its problem: {files:#?}"
    );
}
