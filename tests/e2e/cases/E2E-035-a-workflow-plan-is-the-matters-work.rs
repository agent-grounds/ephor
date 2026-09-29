//! E2E-035-a-workflow-plan-is-the-matters-work: a ledger entry whose only work
//! is a plan a workflow laid is an entry with work, and every surface reads it
//! from that plan (§FS-005-dispatch.35).
//!
//! The scenario is the site this project is maintained on: issue work is laid as
//! a workflow, so the recipe that would otherwise apply is deliberately gated
//! off (§FS-005-dispatch.28) and the ledger entry's only dispatch is a workflow
//! — `ticket` empty, `plan` naming the laid plan. Its laid plan holds a task
//! that is plainly not final, and a run is holding it.
//!
//! What is pinned here is one reading answering three surfaces. `work sync` over
//! a matter that moved says the work is still going and names the plan and the
//! task, instead of calling the matter dormant and offering a command that would
//! untrack it (§FS-005-dispatch.5 keeps its words for the case it is about).
//! `work list` badges the row with what that task is doing rather than with the
//! fact that a workflow exists (§FS-005-dispatch.15). And `work forget --done`
//! selects nothing, because what may be forgotten is read from the plans and
//! never from which of them ephor wrote (§FS-005-dispatch.4).
//!
//! Three asymmetries are pinned beside it, and each is a decision rather than a
//! mechanism. A laid plan that is **gone** is not a finished one: the entry
//! reports as missing, `--done` leaves it alone, and `--missing` and `--item`
//! are the verbs that reach it. A deleted **recipe** plan keeps the older rule
//! and is still selected by `--done`, because ephor wrote that plan and its
//! absence is ephor's own record of something gone. And a laid plan whose tasks
//! are all final *is* dormant, hint and all — which is what makes the first
//! assertion of this case about the plan rather than about the dispatch.
//!
//! The reproduction behind the ticket fabricated a whole site in a shell script
//! and read the two halves off the printed output; this is that shape in the
//! project's own frame, with the laid plan where the record actually names it.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use support::*;

/// The matter: one open issue of the reader's own, which is the whole world a
/// report about workflow-laid issue work needs.
const ITEM: &str = "acmeforge:acme/widget#10";
/// A second matter, whose work is an ordinary recipe ticket. It is in the
/// ledger only so that one `forget --done` reads both kinds at once.
const RECIPE_ITEM: &str = "acmeforge:acme/widget#11";
/// The matter's own plan id — the file the entry points at, which on a site
/// that lays workflow work was never written.
const OWN: &str = "widget-10";
/// The plan the workflow laid, as the dispatch records it: a name inside the
/// dispatch's own root, and *not* the entry's `plan_id`.
const LAID: &str = "widget-10-implement";
/// The task inside that laid plan, and what it is doing.
const TASK: &str = "widget-10.1";
const DOING: &str = "implementing";

/// A forge with one open issue, answering the same three verbs a forge binding
/// is asked for (§FS-001-forge-interface.2).
const ACME_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities) printf '{"issues":true}' ;;
  issues) printf '%s' '[{ "key": "acme/widget#10", "title": "Alpha",
      "url": "https://acme.example/issue/10",
      "updated_at": "2026-07-29T12:00:00Z", "status": "open" }]' ;;
  *) printf '[]' ;;
esac
"#;

/// The machine the laid plan runs under. `supervising` and `implementing` are
/// states work is still at; only `done` is over.
const MACHINE: &str = r#"name: workflow-machine
version: 1.0

states:
  supervising:
    description: Decide what happens next.
  implementing:
    description: Build the change.
  needs-human:
    description: A person has to answer before this goes on.
    gating: true
  done:
    description: Over.
    final: true

transitions:
  - from: supervising
    to: implementing
  - from: implementing
    to: supervising
  - from: implementing
    to: needs-human
  - from: needs-human
    to: done
  - from: supervising
    to: done
"#;

/// The laid plan as a workflow writes one: a workspace directory with an index,
/// a machine of its own, and a nested task nothing has finished.
fn laid_plan(doing: &str) -> String {
    format!(
        "# Rhei: Alpha\n**States:** workflow-machine\n\n## Tasks\n\n\
         ### Ticket {OWN}: work the issue through\n**State:** {doing}\n\n\
         #### Step {TASK}: implement the fix\n**State:** {doing}\n\nwork\n"
    )
}

fn work_root(world: &World) -> PathBuf {
    world.forest().join("panta")
}

/// Where the record says the laid plan is: the dispatch's own root, under the
/// laid plan's own id. Nothing is ever written at the entry's `plan` path — a
/// file there would be reached by the legacy fallback and would hide the whole
/// defect (§FS-005-dispatch.4).
fn laid_dir(world: &World) -> PathBuf {
    work_root(world).join(LAID)
}

/// The site: the forge watched, the recipe that would apply gated off the way a
/// site whose issue work is a workflow gates it, and a feed refreshed so the
/// matter is there to be synced against.
fn site() -> World {
    let world = World::new();
    world.stub("ephor-forge-acmeforge", ACME_FORGE);
    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "acmeforge", "user": "you", "repos": ["widget"] }
        ] } },
        "work": {
            "recipes": [{
                "id": "implement",
                "icon": "◆",
                "description": "look at the issue",
                "state": "fix",
                "needs_checkout": false,
                // The gate: no matter carries this source, so nothing is
                // offered and `sync` takes the branch under test. Turning the
                // recipe back on to quiet it would lay recipe tickets beside
                // the workflow's (§FS-005-dispatch.28).
                "when": { "sources": ["issue-work-is-a-workflow-here"] },
                "brief": "Look at {title}."
            }]
        }
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert!(
        world.has_matter(ITEM),
        "the forge stub put no issue in the feed"
    );

    let root = work_root(&world);
    fs::create_dir_all(&root).expect("the work root");
    fs::write(root.join("states.yaml"), MACHINE).expect("the root's machine");
    let laid = laid_dir(&world);
    fs::create_dir_all(&laid).expect("the laid plan's workspace");
    fs::write(laid.join("states.yaml"), MACHINE).expect("the laid plan's own machine");
    fs::write(laid.join("index.rhei.md"), laid_plan(DOING)).expect("the laid plan");
    assert!(
        !plan_path(&world, OWN).exists(),
        "nothing may sit at the entry's own plan path, or the fallback hides the defect"
    );

    write_json(&ledger_path(&world), &ledger(&world, json!({})));
    world
}

fn plan_path(world: &World, plan_id: &str) -> PathBuf {
    work_root(world).join(format!("{plan_id}.rhei.md"))
}

fn ledger_path(world: &World) -> PathBuf {
    let path = world.path().join("state/ephor/work.json");
    fs::create_dir_all(path.parent().expect("the state directory")).expect("the state directory");
    path
}

/// A fingerprint from before the item moved, so the matter reads as stale and
/// `sync` has something to report (§FS-005-dispatch.5).
fn stale_snapshot() -> Value {
    json!({
        "updated_at": "2026-07-27T12:00:00Z",
        "state": "closed",
        "passed": 0, "failed": 0, "running": 0, "messages": 0
    })
}

/// The ledger the report describes: one entry, one dispatch, and that dispatch
/// is a workflow — `ticket` empty and `plan` naming what it laid. `extra`
/// merges further entries in.
fn ledger(world: &World, extra: Value) -> Value {
    let root = work_root(world);
    let mut entries = json!({
        ITEM: {
            "project": PROJECT,
            "title": "acme/widget#10 Alpha",
            "url": "https://acme.example/issue/10",
            "root": root,
            "checkout": world.forest(),
            "branch": "main",
            "plan_id": OWN,
            "plan": plan_path(world, OWN),
            "dispatches": [{
                "ticket": "",
                "recipe": "implement",
                "at": "2026-07-28T00:00:00Z",
                "plan": LAID,
                "root": root,
                "checkout": world.forest(),
                "branch": "main",
                "snapshot": stale_snapshot()
            }]
        }
    });
    if let (Value::Object(entries), Value::Object(extra)) = (&mut entries, extra) {
        entries.extend(extra);
    }
    json!({ "version": 1, "entries": entries })
}

/// A second entry whose work is an ordinary recipe ticket, and whose plan file
/// was deleted — the case §FS-005-dispatch.35 deliberately leaves selectable.
fn deleted_recipe_entry(world: &World) -> Value {
    json!({ RECIPE_ITEM: {
        "project": PROJECT,
        "title": "acme/widget#11 Beta",
        "root": work_root(world),
        "checkout": world.forest(),
        "branch": "main",
        "plan_id": "widget-11",
        "plan": plan_path(world, "widget-11"),
        "dispatches": [{
            "ticket": "implement-1",
            "recipe": "implement",
            "at": "2026-07-28T00:00:00Z",
            "root": work_root(world),
            "checkout": world.forest(),
            "branch": "main",
            "snapshot": stale_snapshot()
        }]
    } })
}

/// Move every task in the laid plan to a final state, the way the runtime would
/// have: the plan is the artifact and every surface reads it back from there.
fn finish(world: &World) {
    fs::write(laid_dir(world).join("index.rhei.md"), laid_plan("done"))
        .expect("the laid plan is rewritten");
}

fn sync(world: &World) -> String {
    let output = world
        .ephor()
        .args(["work", "sync", "--project", PROJECT])
        .output()
        .expect("work sync runs");
    assert!(
        output.status.success(),
        "work sync failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// What `sync` said about the matter, as a program reads it.
fn sync_outcome(world: &World) -> String {
    let output = world
        .ephor()
        .args(["work", "sync", "--project", PROJECT, "--json"])
        .output()
        .expect("work sync --json runs");
    let reading = json_of(&output);
    reading["items"]
        .as_array()
        .expect("sync reports its items")
        .iter()
        .find(|item| item["item"] == json!(ITEM))
        .map(|item| item["outcome"].as_str().unwrap_or_default().to_string())
        .unwrap_or_else(|| panic!("no report about {ITEM}: {reading}"))
}

fn listed(world: &World, open: bool) -> String {
    let mut args = vec!["work", "list"];
    if open {
        args.push("--open");
    }
    let output = world.ephor().args(args).output().expect("work list runs");
    assert!(
        output.status.success(),
        "work list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn forget(world: &World, args: &[&str]) -> String {
    let mut argv = vec!["work", "forget"];
    argv.extend_from_slice(args);
    let output = world.ephor().args(argv).output().expect("work forget runs");
    assert!(
        output.status.success(),
        "work forget failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn ledger_bytes(world: &World) -> Vec<u8> {
    fs::read(ledger_path(world)).expect("the committed ledger")
}

fn entry_keys(world: &World) -> Vec<String> {
    read_json(&ledger_path(world))["entries"]
        .as_object()
        .expect("the ledger's entries")
        .keys()
        .cloned()
        .collect()
}

/// A run holding the laid plan, the way the report found it: a locked run lock
/// in the work root. Kept only so the fixture is the situation the ticket
/// describes; nothing below reads liveness.
fn hold(root: &Path) -> fs::File {
    fs::create_dir_all(root.join(".rhei")).expect("the runtime directory");
    fs::write(root.join(".rhei/run.lock"), "").expect("the run lock");
    let holder = fs::File::open(root.join(".rhei/run.lock")).expect("open the run lock");
    holder.lock().expect("hold the run lock");
    holder
}

/// The whole of the first half: a matter whose only work is a laid workflow plan
/// with an unfinished task is a matter with work, on every surface that reads it
/// (§FS-005-dispatch.35).
#[test]
fn a_laid_workflow_plan_that_is_going_is_the_matters_open_work() {
    let world = site();
    let _run = hold(&work_root(&world));
    let laid = laid_dir(&world).join("index.rhei.md");

    // `sync` describes work under a workflow as work, names the plan it is in
    // and the task it is at, and offers nothing to clear.
    let said = sync(&world);
    assert!(
        said.contains(&laid.display().to_string()),
        "sync did not name the laid plan:\n{said}"
    );
    assert!(
        said.contains(DOING),
        "sync did not say what the task is doing:\n{said}"
    );
    assert!(
        !said.contains("no recipe applies"),
        "sync called a matter with work dormant:\n{said}"
    );
    assert!(
        !said.contains("forget --done"),
        "sync offered to untrack work that is still going:\n{said}"
    );
    assert_eq!(
        sync_outcome(&world),
        "underway",
        "the machine-readable outcome"
    );

    // The row says what the laid plan's task is doing, rather than that a
    // workflow exists at all.
    let rows = listed(&world, true);
    assert!(
        rows.contains(&format!("⚙ {TASK} · {DOING}")),
        "the open row does not badge the laid task:\n{rows}"
    );

    // And nothing is forgettable as done, with the ledger left untouched.
    let before = ledger_bytes(&world);
    let forgot = forget(&world, &["--done"]);
    assert!(
        forgot.contains("Nothing to forget"),
        "`forget --done` selected an entry whose laid plan is still going:\n{forgot}"
    );
    assert_eq!(
        ledger_bytes(&world),
        before,
        "`forget --done` changed the ledger"
    );
}

/// The other side of the same reading, and what keeps §FS-005-dispatch.5 intact:
/// once the laid plan's tasks are final the matter is dormant, hint and all, and
/// `--done` clears it. The discriminator is the plan, never the dispatch.
#[test]
fn a_laid_workflow_plan_that_is_finished_is_dormant_and_is_forgotten() {
    let world = site();
    finish(&world);

    let said = sync(&world);
    assert!(
        said.contains("no recipe applies to it now"),
        "a matter with nothing open is not dormant:\n{said}"
    );
    assert!(
        said.contains("ephor work forget --done"),
        "the dormant report lost its hint:\n{said}"
    );
    assert_eq!(sync_outcome(&world), "dormant");

    let forgot = forget(&world, &["--done"]);
    assert!(
        forgot.contains(ITEM),
        "`forget --done` left a finished matter in the ledger:\n{forgot}"
    );
    assert!(entry_keys(&world).is_empty(), "the entry was not dropped");
}

/// A laid plan that is gone is not a finished one (§FS-005-dispatch.35): the
/// entry reports as missing, `--done` will not touch it, and `--missing` is the
/// verb for it.
#[test]
fn a_laid_plan_that_is_gone_is_missing_rather_than_done() {
    let world = site();
    fs::remove_dir_all(laid_dir(&world)).expect("take the laid plan away");

    let rows = listed(&world, false);
    assert!(
        rows.contains("⚠ plan missing"),
        "a laid plan nobody can read is not reported as missing:\n{rows}"
    );

    let before = ledger_bytes(&world);
    let refused = forget(&world, &["--done"]);
    assert!(
        refused.contains("Nothing to forget"),
        "`forget --done` read an unreadable plan as a finished one:\n{refused}"
    );
    assert_eq!(
        ledger_bytes(&world),
        before,
        "`forget --done` changed the ledger"
    );

    let forgot = forget(&world, &["--missing"]);
    assert!(
        forgot.contains(ITEM),
        "`--missing` did not reach the entry whose laid plan is gone:\n{forgot}"
    );
    assert!(entry_keys(&world).is_empty(), "the entry was not dropped");
}

/// And the report about that entry names the verb that reaches it
/// (§FS-005-dispatch.35). A plan nobody can read is not a matter that is over,
/// so the dormant sentence and its `--done` are not what it is offered: the two
/// readings are one question asked once, and a recommendation that does nothing
/// is the reported defect one case to the left.
#[test]
fn a_matter_whose_plan_cannot_be_read_is_not_offered_the_verb_for_one_that_is_over() {
    let world = site();
    fs::remove_dir_all(laid_dir(&world)).expect("take the laid plan away");

    let said = sync(&world);
    assert!(
        said.contains("cannot be read"),
        "sync did not say the plan could not be read:\n{said}"
    );
    assert!(
        said.contains("ephor work forget --missing"),
        "sync did not name the verb that reaches the entry:\n{said}"
    );
    assert!(
        !said.contains("no recipe applies to it now"),
        "an unreadable plan was reported as a matter that is over:\n{said}"
    );
    assert!(
        !said.contains("forget --done"),
        "sync recommended a verb that will not touch this entry:\n{said}"
    );
    assert_eq!(sync_outcome(&world), "unread");

    // And what it named does what it says: the sentence and the selection are
    // the same reading, so the reader who takes the hint is not refused.
    let forgot = forget(&world, &["--missing"]);
    assert!(
        forgot.contains(ITEM),
        "the verb sync named did not reach the entry:\n{forgot}"
    );
}

/// `--item` names one entry and is the escape hatch, whatever its plans say
/// (§FS-005-dispatch.35).
#[test]
fn item_forget_still_reaches_an_entry_whose_laid_plan_is_going() {
    let world = site();
    let forgot = forget(&world, &["--item", ITEM]);
    assert!(
        forgot.contains(ITEM),
        "`--item` refused a named entry:\n{forgot}"
    );
    assert!(entry_keys(&world).is_empty(), "the entry was not dropped");
}

/// The asymmetry, read by one command over a mixed ledger: ephor wrote the
/// recipe plan, so its absence is ephor's own record of something gone and
/// `--done` still selects it, while the entry whose laid plan is still going
/// stays (§FS-005-dispatch.35).
#[test]
fn a_deleted_recipe_plan_is_still_done_while_a_going_workflow_plan_is_not() {
    let world = site();
    let extra = deleted_recipe_entry(&world);
    write_json(&ledger_path(&world), &ledger(&world, extra));
    assert!(
        !plan_path(&world, "widget-11").exists(),
        "the recipe entry's plan must be the deleted one"
    );

    let forgot = forget(&world, &["--done"]);
    assert!(
        forgot.contains(RECIPE_ITEM),
        "`--done` stopped selecting an entry whose recipe plan ephor wrote and lost:\n{forgot}"
    );
    assert_eq!(
        entry_keys(&world),
        vec![ITEM.to_string()],
        "`--done` took the matter whose laid plan is still going"
    );
}

/// The preview a reader gets before any of the above: the badge for a laid plan
/// that holds no task at all stays what it was, because there is nothing there
/// to say (§FS-005-dispatch.19).
#[test]
fn a_laid_plan_with_no_task_is_still_badged_as_a_workflow() {
    let world = site();
    fs::write(
        laid_dir(&world).join("index.rhei.md"),
        "# Rhei: Alpha\n**States:** workflow-machine\n",
    )
    .expect("a laid plan with nothing in it");

    let rows = listed(&world, false);
    assert!(
        rows.contains("⛬ 1 workflow"),
        "a laid plan holding no task lost its badge:\n{rows}"
    );
    assert!(
        !rows.contains("plan missing"),
        "a readable laid plan was reported as missing:\n{rows}"
    );
}
