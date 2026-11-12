//! A live laid workflow names the task its run holds, even outside the last
//! open task's prerequisite ancestry (§FS-005-dispatch.15.2). The badge and
//! one-task location agree, with person waits still first (§FS-005-dispatch.9).
//! This ports issue #258's lock, run events and workflow-only ledger fixture;
//! it starts no runtime or agent (§FS-005-dispatch.35).

#[path = "../support.rs"]
mod support;

use std::fs;

use ephor::work::{ledger::Ledger, recipe::WorkConfig, runtime::plan::Plan, status_of_entry};
use serde_json::{json, Value};

use support::*;

const ITEM: &str = "issues:acme/widget#258";
const LAID: &str = "widget-258-supervised-ticket-fix";
const TASKS: &str = "\
### Task claim: Take the claim\n**State:** completed\n\n\
### Ticket ticket: Supervise the issue\n**State:** supervising\n**Prior:** claim\n\n\
#### Step ticket.specify: Specify\n**State:** specify\n\n\
#### Step ticket.implement: Implement\n**State:** implement\n**Prior:** ticket.specify\n\n\
#### Step ticket.describe: Describe\n**State:** describe\n**Prior:** ticket.implement\n\n\
#### Step ticket.ship: Ship\n**State:** ship\n**Prior:** ticket.describe\n";

/// Ship's ancestry is describe -> implement -> specify. The parent ticket is
/// deliberately outside it, as in the reported supervised workflow.
fn site() -> World {
    let world = World::new();
    let root = world.forest().join("panta");
    let laid = root.join(LAID);
    fs::create_dir_all(laid.join("tasks")).unwrap();
    fs::write(laid.join("index.rhei.md"), "# Rhei: Supervised issue\n").unwrap();
    fs::write(laid.join("tasks/01-ticket.md"), TASKS).unwrap();
    fs::write(
        laid.join("states.yaml"),
        concat!(
            "name: supervised-ticket-fix\nstates:\n",
            "  supervising:\n    prompt: Supervise.\n    execute_on: child-terminal\n",
            "  specify:\n    prompt: Specify.\n",
            "  implement:\n    prompt: Implement.\n",
            "  describe:\n    prompt: Describe.\n",
            "  ship:\n    prompt: Ship.\n",
            "  completed:\n    final: true\n",
        ),
    )
    .unwrap();
    fs::create_dir_all(world.path().join("state/ephor")).unwrap();
    write_json(
        &world.path().join("state/ephor/work.json"),
        &json!({"version": 1, "entries": {ITEM: {
            "project": PROJECT, "title": "A supervised issue", "root": root,
            "checkout": world.forest(), "plan_id": "widget-258",
            "plan": root.join("widget-258.rhei.md"),
            "dispatches": [{"ticket": "", "recipe": "supervised-ticket-fix",
                "at": "2026-11-12T19:00:00Z", "plan": LAID, "root": root,
                "snapshot": {"updated_at": "2026-11-12T19:00:00Z",
                    "state": "open", "passed": 0, "failed": 0,
                    "running": 0, "messages": 0}}]
        }}}),
    );
    world
}

fn listed(world: &World, json: bool) -> String {
    let mut command = world.ephor();
    command.args(["work", "list", "--project", PROJECT]);
    if json {
        command.arg("--json");
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "work list failed: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "red until #258: a live supervisor outside ship's prerequisites leads the row and location; implement removes this"]
fn issue258_a_live_workflow_names_its_held_supervisor() {
    let world = site();
    let root = world.forest().join("panta");
    let plan = Plan::read(&root.join(LAID).join("index.rhei.md"))
        .unwrap()
        .unwrap();
    assert!(
        plan.tickets()
            .iter()
            .all(|task| !task.prior.iter().any(|prior| prior == "ticket")),
        "the supervisor must be outside ship's prerequisite ancestry"
    );
    fs::create_dir_all(root.join(".rhei")).unwrap();
    let held = fs::File::create(root.join(".rhei/run.lock")).unwrap();
    held.lock().unwrap();
    fs::create_dir_all(root.join("runtime")).unwrap();
    let events = [
        json!({"seq":1,"event":"run_started","schema":1,"run_id":"issue258"}),
        json!({"seq":2,"event":"slot_assigned","slot":0,
            "task":format!("{LAID}.claim"),"from":"claim","to":"claim"}),
        json!({"seq":3,"event":"slot_released","slot":0,
            "task":format!("{LAID}.claim"),"from":"claim","to":"claim",
            "outcome":"completed","exit_code":0}),
        json!({"seq":4,"event":"slot_assigned","slot":0,
            "task":format!("{LAID}.ticket"),"from":"supervising","to":"supervising",
            "agent":"cdx[high]:openai:gpt-6.1-sol"}),
    ];
    fs::write(
        root.join("runtime/events.jsonl"),
        events
            .iter()
            .map(|event| format!("{event}\n"))
            .collect::<String>(),
    )
    .unwrap();

    let reading: Value = serde_json::from_str(&listed(&world, true)).unwrap();
    let tickets = reading[0]["tickets"].as_array().unwrap();
    assert_eq!(
        tickets.len(),
        6,
        "the laid workspace must be read: {reading:#}"
    );
    let ticket = |id: &str| tickets.iter().find(|ticket| ticket["id"] == id).unwrap();
    assert_eq!(ticket("claim")["finished"], true);
    assert_eq!(ticket("ticket")["state"], "supervising");
    assert_eq!(ticket("ticket")["finished"], false);
    assert_eq!(ticket("ticket.ship")["state"], "ship");
    assert_eq!(ticket("ticket.ship")["finished"], false);
    assert_eq!(ticket("ticket.ship")["running"], false);
    let running: Vec<_> = tickets
        .iter()
        .filter(|ticket| ticket["running"] == true)
        .map(|ticket| ticket["id"].as_str().unwrap())
        .collect();
    assert_eq!(running, vec!["ticket"], "the run holds only the supervisor");
    for ticket in tickets {
        println!(
            "JSON task={} state={} finished={} running={}",
            ticket["id"], ticket["state"], ticket["finished"], ticket["running"]
        );
    }

    // The CLI has no separate one-task-location verb: use the same public
    // status reading its other surfaces consume, as E2E-067 does.
    let ledger: Ledger =
        serde_json::from_value(read_json(&world.path().join("state/ephor/work.json"))).unwrap();
    let status = status_of_entry(&WorkConfig::default(), &ledger.entries[ITEM], None);
    let open = status.open_at.as_ref().expect("open work has a location");
    let badge = status.badge(100);
    let row = listed(&world, false);
    println!("Human work list:\n{row}Badge: {badge}\nLocation: {open:?}");
    assert!(
        row.contains("⚙ ticket · supervising")
            && !row.contains("ticket.ship · ship")
            && badge == "⚙ ticket · supervising"
            && open.ticket == "ticket"
            && open.state == "supervising"
            && open.plan == root.join(LAID).join("index.rhei.md"),
        "held supervisor displaced by unstarted ship: badge={badge:?}; location={open:?}; row={row:?}"
    );
    drop(held);
}
