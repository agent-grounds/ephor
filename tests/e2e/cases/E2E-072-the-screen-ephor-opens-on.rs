//! E2E-072-the-screen-ephor-opens-on: `ephor waiting` is the first screen as a
//! command — what waits on the reader, what nothing has been done about, and
//! what is running.
//!
//! The scenario is §FS-019-waiting from the outside, read the way a program
//! reads it: `ephor waiting --json`, held to the published `waiting` shape. A
//! world with three matters of the reader's: one the recipe will hand over,
//! one under review, and one carrying the label the site's recipe excludes.
//!
//! Before anything is handed over the two unlabelled matters are new and the
//! labelled one is held, listed apart with the label that holds it
//! (§FS-019-waiting.1.2). Handing one over records it in the ledger, which is
//! what takes it out of New. Its plan is then parked by hand at a gate, beside
//! a poll on the reader and a poll on a release tag: the gate is a row named
//! by its ticket's own heading and reviewed by its matter, the poll on the
//! reader is a row that is not the reader's to move, and the release tag waits
//! on the world and is no row at all (§FS-019-waiting.1.1). A run taking the
//! root's lock makes it running, aged from the start it published and nothing
//! else (§FS-019-waiting.1.3). The command answers for the site and refuses a
//! scope rather than ignoring it (§FS-019-waiting.6).

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use support::*;

/// The matter the recipe hands over. It reports its labels, none, because a
/// selector that excludes a label refuses a matter whose source said nothing
/// about labels at all (§FS-005-dispatch.31).
const ITEM: &str = "acmeforge:app/101";
/// A matter the reader is reviewing, which nothing hands over.
const REVIEWING: &str = "acmeforge:app/103";
/// A matter the site's selector holds out of the machine's hands.
const DISCUSSED: &str = "acmeforge:app/102";

const ACME_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities)
    printf '{"pull_requests":true,"conversation":true,"gate":true}'
    ;;
  pull-requests)
    printf '%s' '[
      { "id": "app/101", "repo": "app", "number": "101",
        "title": "Widen the retry window",
        "url": "https://acme.example/pr/101",
        "branch": "you/ABC-42-retry",
        "updated_at": "2026-07-30T12:00:00Z",
        "role": "author", "state": "open", "cited": false, "labels": [],
        "gate": { "repos": [ { "repo": "app", "passed": 5, "failed": 1, "running": 0 } ] } },
      { "id": "app/102", "repo": "app", "number": "102",
        "title": "Rename the lock file",
        "url": "https://acme.example/pr/102",
        "branch": "you/rename-lock",
        "updated_at": "2026-07-30T13:00:00Z",
        "role": "author", "state": "open", "cited": false,
        "labels": ["human-discussion"],
        "gate": { "repos": [ { "repo": "app", "passed": 5, "failed": 1, "running": 0 } ] } },
      { "id": "app/103", "repo": "app", "number": "103",
        "title": "Count generated files separately",
        "url": "https://acme.example/pr/103",
        "branch": "luna/generated",
        "updated_at": "2026-07-30T14:00:00Z",
        "role": "reviewer", "state": "open", "cited": false }
    ]'
    ;;
  *)
    printf '[]'
    ;;
esac
"#;

/// A runtime that is bound and answers its listing with nothing, so the plan
/// file is the floor every state is read from.
const ACME_RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  list) printf '[]' ;;
  *) exit 0 ;;
esac
"#;

/// The root's machine: work, a gate a person moves, a poll on the matter's
/// author, a poll on a release tag, and the end.
const MACHINE: &str = "name: ephor-work
version: 1.0

states:
  fix:
    description: Do what the ticket asks.
    agent: claude-code
  needs-human:
    description: A person decides whether this goes on.
    gating: true
  plan-approval:
    description: The author approves the plan on the forge.
    program: [\"bash\", \"await.sh\"]
    poll:
      interval: 30m
      waiting_on: author
  holding:
    description: Wait for the release tag.
    program: [\"bash\", \"hold.sh\"]
    poll:
      interval: 30m
      waiting_on: a release tag
  done:
    final: true
  cancelled:
    final: true
";

/// A runtime that prepares gate briefs, and so lists the gates under a work
/// root (§FS-019-waiting.1.1.1): it keeps every listing it was asked for, and
/// answers from `listing.json` beside it, or refuses where there is none.
const LISTING_RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
data="$(dirname "$0")/../gate-data"
case "${1:-}" in
  list) printf '[]' ;;
  gate)
    case "${2:-}" in
      schema) printf '{"version":1,"schemas":{"request":{},"answer":{}}}' ;;
      list)
        printf '%s\n' "$*" >> "$data/listed"
        [ -f "$data/listing.json" ] || { echo "cannot list this root" >&2; exit 1; }
        cat "$data/listing.json" ;;
      *) exit 1 ;;
    esac ;;
  *) exit 0 ;;
esac
"#;

fn watching() -> World {
    watching_with(ACME_RUNTIME)
}

fn watching_with(runtime: &str) -> World {
    let world = World::new();
    world.stub("ephor-forge-acmeforge", ACME_FORGE);
    settle(&world.stub("acme-runtime", runtime));
    fs::create_dir_all(world.path().join("gate-data")).expect("the stub's data");
    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "acmeforge", "user": "you", "repos": ["app"] }
        ] } },
        "work": {
            "runner": "acme-runtime",
            "recipes": [{
                "id": "fix-gate",
                "icon": "🛠",
                "description": "fix the red gate",
                "state": "fix",
                "needs_checkout": false,
                "when": {
                    "kinds": ["pr"], "roles": ["author"], "gate": "failing",
                    "labels": ["!human-discussion"]
                },
                "brief": "Fix the gate on {title}."
            }]
        }
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

fn work_root(world: &World) -> PathBuf {
    world.forest().join("panta")
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    fs::write(path, content).expect("write");
}

/// `ephor waiting --json`, held to the published shape.
fn waiting(world: &World) -> Value {
    let out = world
        .ephor()
        .args(["waiting", "--json"])
        .output()
        .expect("ephor waiting");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    shaped("waiting", &out)
}

fn ids(rows: &Value, key: &str) -> Vec<String> {
    rows.as_array()
        .expect("rows")
        .iter()
        .map(|row| row[key].as_str().expect("a string").to_string())
        .collect()
}

/// The one plan the dispatch wrote.
fn plan_of(world: &World) -> PathBuf {
    fs::read_dir(work_root(world))
        .expect("the work root")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| path.to_string_lossy().ends_with(".rhei.md"))
        .expect("the plan the dispatch wrote")
}

/// Park the dispatched ticket at the gate, and add a poll on the reader and a
/// poll on the world beside it — the plan a run leaves when it stops.
fn park(world: &World) -> String {
    let plan = plan_of(world);
    let text = fs::read_to_string(&plan).expect("the plan");
    assert!(text.contains("**State:** fix\n"), "{text}");
    let heading = text
        .lines()
        .find_map(|line| line.strip_prefix("### Task fix-gate-1: "))
        .expect("the dispatched ticket's heading")
        .to_string();
    let text = format!(
        "{}\n\n### Task approve-1: Approve the retry plan\n**State:** plan-approval\n\nAsk.\n\n\
         ### Task tag-1: Wait for the tag\n**State:** holding\n\nWait.\n",
        text.replacen("**State:** fix\n", "**State:** needs-human\n", 1)
            .trim_end()
    );
    write(&plan, &text);
    write(&work_root(world).join("states.yaml"), MACHINE);
    heading
}

/// What a live run leaves: its lock held, the descriptor naming it, and the
/// journal saying which ticket it holds.
fn a_run_is_live(root: &Path) -> fs::File {
    write(&root.join(".rhei/run.lock"), "");
    let holder = fs::File::open(root.join(".rhei/run.lock")).expect("the lock file");
    holder.lock().expect("the lock");
    write(
        &root.join("runtime/run.json"),
        concat!(
            "{\"id\":\"3f9a2c\",\"pid\":48213,\"status\":\"running\",",
            "\"started_at\":\"2026-08-22T14:03:22Z\",\"headless\":true,\"exit_code\":null}"
        ),
    );
    write(
        &root.join("runtime/transitions.log"),
        "2026-08-22T14:05:00Z  tag-1  start@holding  runtime/logs/task-tag-1.log\n",
    );
    write(&root.join("runtime/logs/task-tag-1.log"), "at it\n");
    holder
}

#[test]
fn the_first_screen_is_a_command_that_says_what_waits_on_the_reader() {
    let world = watching();

    // Nothing handed over yet: the two unlabelled matters are new, newest
    // first, and the labelled one is held apart (§FS-019-waiting.1.2).
    let view = waiting(&world);
    assert_eq!(ids(&view["new"], "id"), vec![REVIEWING, ITEM], "{view}");
    assert_eq!(ids(&view["held"], "id"), vec![DISCUSSED], "{view}");
    assert_eq!(view["held"][0]["label"], "human-discussion", "{view}");
    assert!(
        view["on_you"].as_array().expect("rows").is_empty(),
        "{view}"
    );
    assert!(
        view["running"].as_array().expect("rows").is_empty(),
        "{view}"
    );
    assert_eq!(view["overview"]["matters"], 3, "{view}");
    assert_eq!(view["overview"]["unread"], 3, "{view}");

    // Handing one over records it, and that is what takes it out of New.
    world
        .ephor()
        .args(["work", "dispatch", "--item", ITEM])
        .assert()
        .success();
    let view = waiting(&world);
    assert_eq!(ids(&view["new"], "id"), vec![REVIEWING], "{view}");

    // Parked at a gate, beside a poll on the reader and one on the world
    // (§FS-019-waiting.1.1).
    let heading = park(&world);
    let view = waiting(&world);
    let rows = view["on_you"].as_array().expect("rows");
    assert_eq!(rows.len(), 2, "the release tag waits on the world: {view}");
    let gate = &rows[0];
    assert_eq!(gate["kind"], "gate", "{view}");
    assert_eq!(gate["title"], heading.as_str(), "{view}");
    assert_eq!(gate["state"], "needs-human", "{view}");
    assert_eq!(gate["item"], ITEM, "{view}");
    assert_eq!(gate["matter"], "Widen the retry window", "{view}");
    assert_eq!(gate["review"], ITEM, "{view}");
    assert_eq!(gate["movable"], true, "{view}");
    // No runtime here says how long a gate has waited, so nothing does.
    assert!(gate.get("since").is_none(), "{view}");
    let poll = &rows[1];
    assert_eq!(poll["kind"], "poll", "{view}");
    assert_eq!(poll["title"], "Approve the retry plan", "{view}");
    assert_eq!(poll["waiting_on"], "author", "{view}");
    assert_eq!(poll["movable"], false, "{view}");

    // A run taking the root is running, aged from the start it published
    // (§FS-019-waiting.1.3).
    let _holder = a_run_is_live(&work_root(&world));
    let view = waiting(&world);
    let running = view["running"].as_array().expect("rows");
    assert_eq!(running.len(), 1, "{view}");
    assert_eq!(running[0]["run"], "3f9a2c", "{view}");
    assert_eq!(running[0]["started_at"], "2026-08-22T14:03:22Z", "{view}");
    assert_eq!(
        running[0]["attach"], "acme-runtime attach '3f9a2c'",
        "{view}"
    );
    assert_eq!(running[0]["item"], ITEM, "{view}");
    assert_eq!(
        ids(&running[0]["tickets"], "state"),
        vec!["holding"],
        "{view}"
    );

    // The same reading, as a person reads it.
    let out = world
        .ephor()
        .args(["waiting"])
        .output()
        .expect("ephor waiting");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.starts_with("ephor — waiting   2 on you · 1 new · 1 running"),
        "{text}"
    );
    assert!(
        text.contains("waiting on author · not yours to move here"),
        "{text}"
    );
    assert!(text.contains("held by human-discussion"), "{text}");
    assert!(
        text.contains("overview: 3 matters · 3 changed since you looked"),
        "{text}"
    );

    // It answers for the site, so a scope is refused rather than ignored
    // (§FS-019-waiting.6).
    let out = world
        .ephor_raw()
        .args(["waiting", "--project", PROJECT])
        .output()
        .expect("ephor waiting --project");
    assert!(
        !out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// A runtime that lists its gates lends each gate row its brief's title, when
/// it began waiting and what it holds, asked once per work root; a root it
/// cannot list keeps the rows the enumeration gave (§FS-019-waiting.1.1.1).
#[test]
fn a_runtime_that_lists_its_gates_says_how_long_each_has_waited() {
    let world = watching_with(LISTING_RUNTIME);
    world
        .ephor()
        .args(["work", "dispatch", "--item", ITEM])
        .assert()
        .success();
    let heading = park(&world);
    let data = world.path().join("gate-data");

    // The listing could not be had: the rows are the enumeration's.
    let view = waiting(&world);
    let gate = &view["on_you"][0];
    assert_eq!(gate["kind"], "gate", "{view}");
    assert_eq!(gate["title"], heading.as_str(), "{view}");
    assert!(
        gate.get("since").is_none() && gate.get("holds").is_none(),
        "{view}"
    );
    let ticket = gate["ticket"]
        .as_str()
        .expect("the gate's ticket")
        .to_string();
    let listed = fs::read_to_string(data.join("listed")).expect("the root was listed");
    assert_eq!(
        listed.lines().collect::<Vec<_>>(),
        vec![format!("gate list {} --json", work_root(&world).display())],
        "one listing for the one root that holds a gate"
    );

    // Listed: the brief's title, the start and the holds; an unknown start
    // is null, never zero.
    write(
        &data.join("listing.json"),
        &json!({"version": 1, "root": work_root(&world), "gates": [
            {"ticket": ticket, "ticket_title": heading, "title": "Rule on the retry window",
             "state": "needs-human", "plan": plan_of(&world), "root": work_root(&world),
             "visit": 1, "since": "2026-08-20T09:00:00Z", "source": "questions",
             "required": 1, "holds": 2, "problems": []}
        ]})
        .to_string(),
    );
    let view = waiting(&world);
    let gate = &view["on_you"][0];
    assert_eq!(gate["title"], "Rule on the retry window", "{view}");
    assert_eq!(gate["since"], "2026-08-20T09:00:00Z", "{view}");
    assert_eq!(gate["holds"], 2, "{view}");
    assert_eq!(view["on_you"][1]["kind"], "poll", "{view}");
    assert!(view["on_you"][1].get("since").is_none(), "{view}");
    let out = world
        .ephor()
        .args(["waiting"])
        .output()
        .expect("ephor waiting");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Rule on the retry window   needs-human · waited "),
        "{text}"
    );
    assert!(text.contains(" · holds 2 tasks"), "{text}");

    write(
        &data.join("listing.json"),
        &json!({"version": 1, "root": work_root(&world), "gates": [
            {"ticket": ticket, "ticket_title": heading, "title": "Rule on the retry window",
             "state": "needs-human", "plan": plan_of(&world), "root": work_root(&world),
             "visit": 1, "since": null, "source": "questions",
             "required": 1, "holds": 0, "problems": []}
        ]})
        .to_string(),
    );
    let view = waiting(&world);
    let gate = &view["on_you"][0];
    assert!(
        gate["since"].is_null() && gate.get("since").is_some(),
        "{view}"
    );
    let out = world
        .ephor()
        .args(["waiting"])
        .output()
        .expect("ephor waiting");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("needs-human · waited unknown · holds nothing"),
        "{text}"
    );
}
