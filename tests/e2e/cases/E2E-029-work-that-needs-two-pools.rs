//! E2E-029-work-that-needs-two-pools: a workflow whose targets are bought
//! against two providers is admitted whole, or nothing is written at all.
//!
//! The scenario is a machine that can reach two providers and has spent one of
//! them. The matter is one no recipe covers, and the only thing that applies is
//! a workflow entry that asked to run itself — a cross-family discussion whose
//! moderator is answered by a hand on one provider and whose second voice is
//! answered by a hand on the other. Both windows have to be open for that plan
//! to reach its end, and no ordered list of alternates can say so: a list
//! always has a survivor, and this work has none (§FS-005-dispatch.33).
//!
//! So the question is asked once, about the work rather than about a member,
//! after the veto has chosen among whatever alternates each target named
//! (§FS-005-dispatch.29). Where it answers *held*, nothing is written and the
//! matter stays in the feed for a machine that can finish it — because a plan
//! that is laid is a plan that is claimed, and half a run costs both the
//! attempts against the shut window and the claim that kept the work away from
//! whoever had both pools (§FS-005-dispatch.28, §FS-005-dispatch.24).
//!
//! Two refusals bound it, and this case is as much about them. A set of one
//! pool is never held — that case is §FS-005-dispatch.29's and keeps its
//! answer. And **unknown is not spent**: a pool nobody reported a number for
//! holds nothing at all, which is the load-bearing rule on a machine where
//! silence is the ordinary report (§REQ-001-boundary.1).

#[path = "../support.rs"]
mod support;

use predicates::prelude::*;
use serde_json::{json, Value};

use support::*;

/// The matter: one issue nobody here opened, with no conversation and no
/// branch, so no shipped recipe covers it and the workflow entry gets its turn.
const ITEM: &str = "acmeforge:acme/widget#42";
/// The plan the entry would lay about it, by the id ephor derives.
const PLAN: &str = "acmeforge-acme-widget-42-two-family-review";
/// The entry's own id, which is what the clause names.
const ENTRY: &str = "two-family-review";

const ISSUE_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities)
    printf '{"issues":true}'
    ;;
  issues)
    printf '%s' '[
      { "key": "acme/widget#42", "title": "Should the retry window be per attempt?",
        "url": "https://acme.example/issue/42",
        "updated_at": "2026-07-30T12:00:00Z", "status": "open",
        "role": "reviewer" }
    ]'
    ;;
  *)
    printf '[]'
    ;;
esac
"#;

/// The runtime: it lists one workflow with two inputs that name who does the
/// work, and renders it into the directory ephor points at. The two targets are
/// what make this work need two pools — ephor resolves each through the roster
/// and each resolution carries the pool its work is bought against
/// (§FS-005-dispatch.19).
const RENDERS: &str = r#"#!/usr/bin/env bash
set -euo pipefail
verb="$1"; shift

if [ "$verb" = templates ]; then
  cat <<JSON
[
  { "name": "two-family-review", "version": "1.0.0", "source": "project",
    "path": "$WORKFLOWS/two-family-review",
    "description": "Two families discuss one question.",
    "inputs": [
      { "name": "question", "description": "What is discussed.",
        "type": "string", "required": true, "default": null, "validate": null,
        "format": null, "positional": 1, "items": null, "properties": null },
      { "name": "moderator", "description": "Who moderates.",
        "type": "string", "required": true, "default": null, "validate": null,
        "format": "execution-target", "positional": null,
        "items": null, "properties": null },
      { "name": "second_voice", "description": "Who answers back.",
        "type": "string", "required": true, "default": null, "validate": null,
        "format": "execution-target", "positional": null,
        "items": null, "properties": null } ] }
]
JSON
  exit 0
fi

if [ "$verb" = instantiate ]; then
  ref="$1"; shift
  values=""; output=""; dry=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --values) values="$2"; shift 2 ;;
      --output) output="$2"; shift 2 ;;
      --dry-run) dry=yes; shift ;;
      *) shift ;;
    esac
  done
  if [ -n "$dry" ]; then
    echo "would render $(basename "$ref") into $output"
    exit 0
  fi
  mkdir -p "$output/tasks"
  printf '# Rhei: discuss it\n**States:** two-family-review\n' > "$output/index.rhei.md"
  printf 'name: two-family-review\nstates:\n  discussing:\n    agent: x\n  done:\n    final: true\n' \
    > "$output/states.yaml"
  printf '### Task discuss: hold the discussion\n**State:** discussing\n\nwork\n' \
    > "$output/tasks/01-discuss.md"
  cp "$values" "$output/values-as-given.json"
  echo "Instantiated $(basename "$ref") into $output"
  exit 0
fi
"#;

/// The other half of the runtime: a runner that can detach, so a plan the
/// sweep laid is a plan the sweep after it can start.
const DETACHES: &str = r#"
if [ "$verb" = run ]; then
  case "$*" in
    *--help*)
      printf 'Options:\n      --headless  detach it\n'
      exit 0 ;;
    *--headless*)
      printf '%s\n' "$*" >> "$RUNS"
      printf '{"id":"7c1b04","pid":11,"status":"running","exit_code":null}\n'
      exit 0 ;;
  esac
fi
"#;

const REFUSES: &str = r#"
echo "unknown verb $verb" >&2
exit 1
"#;

/// Two model profiles served by two different providers, and one agent that
/// carries both. The providers are what make two pools (§FS-005-dispatch.29).
const RUNTIME_SETTINGS: &str = r#"{
  "agents": { "acme-agent": { "command": ["sh"] } },
  "models": {
    "north-fast": { "provider": "north", "model": "m-north",
                    "default_agent": "acme-agent", "agents": { "acme-agent": {} } },
    "south-fast": { "provider": "south", "model": "m-south",
                    "default_agent": "acme-agent", "agents": { "acme-agent": {} } }
  }
}"#;

/// A headroom verb: the envelope goes to the file `$EPHOR_ANSWER` names, with
/// the windows on `data` (§FS-006-project-interface.4).
fn reporting(remaining: Option<f64>, resets_at: &str) -> String {
    answering(
        json!({ "v": 1, "data": { "windows": [
            { "name": "session", "remaining": remaining, "resets_at": resets_at }
        ] } }),
        0,
    )
}

/// The instant the north window lifts, written once because three assertions
/// read it.
const LIFTS: &str = "2026-09-05T18:30:00Z";

/// A world whose only offer about the issue is the entry beside the workflow,
/// with both targets answered by the entry itself. `runner` names the stub
/// deliberately: whether a runtime can detach is asked once per runner name and
/// remembered, so a case about laying and a case about starting must not share
/// one.
fn world_of(runner: &str, script: &str, moderator: &str, second: &str, work: Value) -> World {
    let world = World::new();
    world.stub("ephor-forge-acmeforge", ISSUE_FORGE);
    let workflows = world.path().join("workflows");
    std::fs::create_dir_all(workflows.join("two-family-review")).expect("a workflow directory");
    std::fs::write(
        workflows.join("two-family-review").join("template.yaml"),
        "name: two-family-review\ninputs:\n  - name: question\n    type: string\n    \
         positional: 1\n  - name: moderator\n    type: string\n    \
         format: execution-target\n  - name: second_voice\n    type: string\n    \
         format: execution-target\n",
    )
    .expect("the workflow's own manifest");
    std::fs::write(
        workflows.join("two-family-review").join(".ephor.json"),
        format!(
            r#"{{
              "id": "{ENTRY}",
              "icon": "⛬",
              "description": "hold the discussion",
              "when": {{ "kinds": ["issue"] }},
              "autorun": true,
              "inputs": {{ "question": "{{title}}",
                           "moderator": "{moderator}",
                           "second_voice": "{second}" }}
            }}"#
        ),
    )
    .expect("the entry beside the workflow");
    world.stub(
        runner,
        &script
            .replace("$WORKFLOWS", &workflows.to_string_lossy())
            .replace("$RUNS", &world.path().join("runs.log").to_string_lossy()),
    );
    let settings = world.path().join(".config/rhei/settings.json");
    std::fs::create_dir_all(settings.parent().expect("a parent")).expect("the settings directory");
    std::fs::write(&settings, RUNTIME_SETTINGS).expect("the runtime's registry");

    let mut configured = json!({ "runner": runner });
    merge_into(&mut configured, work);
    world.configure(json!({
        "projects": { PROJECT: {
            "providers": [ { "provider": "acmeforge", "user": "you", "repos": ["widget"] } ],
            // The entry's own hand is on the healthy pool, so the only thing
            // that can hold this work is the pair its targets need.
            "work": { "hands": { ENTRY: "south-fast" } }
        } },
        "work": configured,
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

/// The ordinary shape: two targets on two pools, north spent until [`LIFTS`]
/// and south healthy.
fn one_pool_spent(runner: &str, script: &str) -> World {
    let world = world_of(
        runner,
        script,
        "north-fast",
        "south-fast",
        json!({ "headroom": { "north": "north-headroom", "south": "south-headroom" } }),
    );
    world.stub("north-headroom", &reporting(Some(0.0), LIFTS));
    world.stub("south-headroom", &reporting(Some(0.42), LIFTS));
    // Probing is fetching, so it happens where fetching happens
    // (§FS-005-dispatch.29) rather than in front of the dispatch.
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

fn merge_into(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(existing) => merge_into(existing, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

fn work_root(world: &World) -> std::path::PathBuf {
    world.forest().join("panta")
}

fn plan_dir(world: &World) -> std::path::PathBuf {
    work_root(world).join(PLAN)
}

fn dispatch(world: &World) -> Value {
    let output = world
        .ephor()
        .args(["work", "dispatch", "--json"])
        .output()
        .expect("dispatch runs");
    assert!(
        output.status.success(),
        "dispatch failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    json_of(&output)
}

/// Every word a row of the dispatch reading said, so an assertion about the
/// clause does not have to guess which field carried it.
fn said(swept: &Value) -> String {
    swept.to_string()
}

/// The ticket's own scenario. One required pool is known spent, so the work is
/// held: **nothing is written** — no plan directory, no record of one — and the
/// matter is still in the feed for a machine that has both pools
/// (§FS-005-dispatch.33). This is the assertion that fails today: ephor prints
/// that north is spent one command earlier and lays the plan anyway, because
/// the only question headroom answers today is which member of one list can be
/// had.
#[test]
fn a_workflow_needing_two_pools_is_not_laid_while_one_of_them_is_spent() {
    let world = one_pool_spent("acme-held", &format!("{RENDERS}{REFUSES}"));

    let swept = dispatch(&world);
    assert!(
        !plan_dir(&world).exists(),
        "a workflow needing the north and south pools at once was laid while north is \
         spent until {LIFTS}: {}",
        plan_dir(&world).display()
    );
    assert_eq!(swept["laid"], 0, "{swept}");

    // Held is a refusal with a reason, not an error: the sweep succeeded and
    // goes on to the next matter (§FS-005-dispatch.28).
    let says = said(&swept);
    assert!(
        says.contains("north and south pools at once"),
        "the reading does not say which pools the work needs together: {says}"
    );
    assert!(
        says.contains("north") && says.contains(LIFTS),
        "the reading does not name the spent pool and when it lifts: {says}"
    );

    // And the matter is untouched: still in the feed, with nothing claimed
    // about it (§GOAL-003-nothing-lost).
    assert!(world.has_matter(ITEM), "the held matter left the feed");
}

/// The menu row carries the same clause, from the same place, so the row a
/// person greys out and the line a command prints cannot disagree
/// (§FS-005-dispatch.33). Both readings of it say it in the same words
/// (§REQ-002-parity.3).
#[test]
fn the_menu_row_is_blocked_with_the_same_clause() {
    let world = one_pool_spent("acme-menu", &format!("{RENDERS}{REFUSES}"));

    let view = world
        .ephor()
        .args(["actions", "--item", ITEM, "--json"])
        .output()
        .expect("actions runs");
    let view = json_of(&view);
    let offer = view["offers"]
        .as_array()
        .expect("a menu")
        .iter()
        .find(|offer| offer["id"] == ENTRY)
        .unwrap_or_else(|| panic!("the entry is on the menu: {view}"));
    assert_eq!(offer["gate"], "blocked", "{offer}");
    let refusal = offer["refusal"].as_str().unwrap_or_default();
    assert!(
        refusal.contains("north and south pools at once") && refusal.contains(LIFTS),
        "the blocked row does not carry the clause: {offer}"
    );

    // The text reading says it too, and says the same thing.
    world
        .ephor()
        .args(["actions", "--item", ITEM])
        .assert()
        .success()
        .stdout(predicate::str::contains("north and south pools at once"));
}

/// A plan already laid is held at the **start**, and says something else: the
/// admission's news is *held, and still anybody's*, and this one is *not
/// started, and still yours*. The sweep never sees the entry that laid the
/// plan, so it reads what the work needs from ephor's own record of the laying
/// (§FS-005-dispatch.33, §FS-005-dispatch.4). It is a `passed-over` row, the
/// shape a ceiling and a busy tree already take there, and it happens before
/// capacity is spent (§FS-005-dispatch.24).
#[test]
fn a_plan_already_laid_is_passed_over_by_the_unattended_sweep() {
    // Both pools healthy, so the plan is laid the way it always was.
    let world = world_of(
        "acme-start",
        &format!("{RENDERS}{DETACHES}{REFUSES}"),
        "north-fast",
        "south-fast",
        json!({ "headroom": { "north": "north-headroom", "south": "south-headroom" } }),
    );
    world.stub("north-headroom", &reporting(Some(0.9), LIFTS));
    world.stub("south-headroom", &reporting(Some(0.9), LIFTS));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world.ephor().args(["work", "dispatch"]).assert().success();
    assert!(
        plan_dir(&world).join("tasks/01-discuss.md").is_file(),
        "the plan was not laid while both pools were healthy"
    );

    // Then the north window shuts, and ephor is told.
    world.stub("north-headroom", &reporting(Some(0.0), LIFTS));
    world.ephor().args(["refresh", PROJECT]).assert().success();

    let output = world
        .ephor()
        .args(["work", "run", "--due", "--json"])
        .output()
        .expect("the due sweep runs");
    assert!(output.status.success());
    let reading = json_of(&output);
    assert_eq!(reading["failed"], 0, "{reading}");
    assert_eq!(
        reading["runs"][0]["outcome"], "passed-over",
        "the unattended sweep started a plan whose north pool is spent: {reading}"
    );
    let reason = reading["runs"][0]["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("north and south pools at once") && reason.contains(LIFTS),
        "the passed-over row does not carry the clause: {reading}"
    );

    // And the plan stays exactly where it is: passing over is not cancelling.
    assert!(
        plan_dir(&world).join("index.rhei.md").is_file(),
        "{reading}"
    );
}

/// A run the reader asks for by name keeps the reader's key
/// (§FS-005-dispatch.30): the plan is in front of them and already laid, so it
/// carries the same clause as a **warning** and starts. Only the sweep nobody
/// typed is held (§FS-005-dispatch.33).
#[test]
fn a_run_asked_for_by_name_is_warned_and_still_starts() {
    let world = world_of(
        "acme-named",
        &format!("{RENDERS}{DETACHES}{REFUSES}"),
        "north-fast",
        "south-fast",
        json!({ "headroom": { "north": "north-headroom", "south": "south-headroom" } }),
    );
    world.stub("north-headroom", &reporting(Some(0.9), LIFTS));
    world.stub("south-headroom", &reporting(Some(0.9), LIFTS));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world.ephor().args(["work", "dispatch"]).assert().success();
    std::fs::write(world.path().join("runs.log"), "").expect("the stub's log of runs");

    world.stub("north-headroom", &reporting(Some(0.0), LIFTS));
    world.ephor().args(["refresh", PROJECT]).assert().success();

    let output = world
        .ephor()
        .args(["work", "run", "--item", ITEM, "--json"])
        .output()
        .expect("the key runs");
    assert!(output.status.success(), "the named run was refused");
    let warned = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        warned.contains("north and south pools at once") && warned.contains(LIFTS),
        "a named run on work whose pool is spent said nothing about it: {warned}"
    );
    let handed = std::fs::read_to_string(world.path().join("runs.log")).unwrap_or_default();
    assert!(
        handed.contains(PLAN),
        "the named run was held rather than warned: {handed}"
    );
}

/// The escape, and the proof that the requirement follows the targets rather
/// than a key anybody wrote down: answer the target with a hand on the healthy
/// pool and the same entry needs one pool, so it is laid
/// (§FS-005-dispatch.33). There is no flag that overrides the hold.
#[test]
fn answering_the_target_with_a_hand_on_the_healthy_pool_lays_it() {
    let world = world_of(
        "acme-escape",
        &format!("{RENDERS}{REFUSES}"),
        "south-fast",
        "south-fast",
        json!({ "headroom": { "north": "north-headroom", "south": "south-headroom" } }),
    );
    world.stub("north-headroom", &reporting(Some(0.0), LIFTS));
    world.stub("south-headroom", &reporting(Some(0.42), LIFTS));
    world.ephor().args(["refresh", PROJECT]).assert().success();

    let swept = dispatch(&world);
    assert_eq!(swept["laid"], 1, "{swept}");
    assert!(plan_dir(&world).join("index.rhei.md").is_file());
}

/// A set of one is never held, even when that one pool is spent. This is
/// §FS-005-dispatch.29's own case and it keeps its answer entirely: the work is
/// written and it waits. Nothing above narrows it.
#[test]
fn a_set_of_one_spent_pool_is_still_laid() {
    let world = world_of(
        "acme-single",
        &format!("{RENDERS}{REFUSES}"),
        "south-fast",
        "south-fast",
        json!({ "headroom": { "south": "south-headroom" } }),
    );
    world.stub("south-headroom", &reporting(Some(0.0), LIFTS));
    world.ephor().args(["refresh", PROJECT]).assert().success();

    let swept = dispatch(&world);
    assert_eq!(
        swept["laid"], 1,
        "one spent pool held work that §FS-005-dispatch.29 says is written and waits: {swept}"
    );
    assert!(plan_dir(&world).join("index.rhei.md").is_file());
}

/// The load-bearing refusal. Two pools, neither of them reporting a number, and
/// nothing is held: absent is the ordinary case on these credentials, and a
/// rule that read silence as exhaustion would hold every cross-family workflow
/// on the machine (§FS-005-dispatch.33, §REQ-001-boundary.1).
#[test]
fn two_unknown_pools_hold_nothing() {
    // No headroom verb is bound for either pool, which is the ordinary site.
    let world = world_of(
        "acme-unknown",
        &format!("{RENDERS}{REFUSES}"),
        "north-fast",
        "south-fast",
        json!({}),
    );

    let swept = dispatch(&world);
    assert_eq!(
        swept["laid"], 1,
        "silence was read as exhaustion and the work was held: {swept}"
    );
    assert!(plan_dir(&world).join("index.rhei.md").is_file());

    // And both pools really are unknown rather than healthy: the reading says
    // so, with the reason beside each (§FS-005-dispatch.29).
    let reported = world
        .ephor()
        .args(["caps", "--json"])
        .output()
        .expect("capabilities answers");
    let pools = json_of(&reported)["pools"].clone();
    for name in ["north", "south"] {
        let entry = pools
            .as_array()
            .unwrap_or_else(|| panic!("no pools in {pools}"))
            .iter()
            .find(|entry| entry["pool"] == name)
            .unwrap_or_else(|| panic!("no pool {name} in {pools}"));
        assert!(entry["remaining"].is_null(), "{entry}");
    }
}

/// A required target this site does not reach at all holds the work exactly as
/// a spent pool does, and says something else: there is no window to wait for,
/// so such work parks until somebody answers the target with a hand this site
/// has (§FS-005-dispatch.33). Because the pools are derived from what the
/// roster answered, a derived pool is on the roster by construction and
/// "absent" surfaces as exactly one case — a required target that resolves to
/// no hand at all. That already refuses and already writes nothing, so what
/// this adds is the sentence, and the sentence names the target.
#[test]
fn a_required_target_nothing_here_reaches_holds_in_its_own_words() {
    let world = world_of(
        "acme-absent",
        &format!("{RENDERS}{REFUSES}"),
        "north-fast",
        // A hand this site's roster does not have, and nothing will make it
        // appear: there is no window here that reopens.
        "east-fast",
        json!({ "headroom": { "north": "north-headroom" } }),
    );
    world.stub("north-headroom", &reporting(Some(0.9), LIFTS));
    world.ephor().args(["refresh", PROJECT]).assert().success();

    let swept = dispatch(&world);
    // Nothing written is already today's outcome; the clause is what is new.
    assert_eq!(swept["laid"], 0, "{swept}");
    assert!(!plan_dir(&world).exists());

    let says = said(&swept);
    assert!(
        says.contains("east-fast"),
        "the refusal does not name the target nothing here reaches: {says}"
    );
    assert!(
        says.contains("nothing here reaches"),
        "an unreachable target is not told apart from a spent pool: {says}"
    );
    assert!(
        !says.contains(LIFTS),
        "an unreachable target was given an instant it lifts at: {says}"
    );
}
