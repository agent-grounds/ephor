//! E2E-046-a-root-waiting-on-a-person: the sweep starts nothing on a root whose
//! every would-be-due ticket sits in a tree a gate holds, says it waits on a
//! person, and takes the root up again the moment the gate moves
//! (§FS-005-dispatch.24.3).
//!
//! The scenario is the one ephor#160 reported. A supervisor ticket sits in
//! `supervising`, which is not a gate, and its triage subtask sits in
//! `human-gate`, which is. Taken ticket by ticket the supervisor makes the root
//! due, so the sweep starts a run; the runtime halts at the closed gate in a
//! tenth of a second having spawned nothing; the no-advance rest counts that as
//! a strike; and after three of them the root is stopped — on a plan working as
//! designed, and stopped past the moment the person answers.
//!
//! What this case pins is the due test taken over the tree
//! (§FS-005-dispatch.24.3.1): such a root is a `passed-over` row with the hold
//! `person` naming the gated ticket (§FS-005-dispatch.24.3.2,
//! §FS-005-dispatch.24.2), no run is made there and none is judged, a stop the
//! root already carried is dropped by the sweep that finds it waiting
//! (§FS-005-dispatch.24.3.3), and a ready ticket in another tree of the same
//! root still starts it.

#[path = "../support.rs"]
mod support;

use serde_json::json;

use support::*;

/// The matter the recipe turns into a ticket.
const ITEM: &str = "acmeforge:app/101";

/// The second project, watched beside the first only so a bare sweep is gated
/// (§FS-011-command-line.10).
const OTHER: &str = "far";

/// A forge with one pull request of the reader's own and a red gate, so there
/// is a matter the recipe picks up and hands over.
const ACME_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities)
    printf '{"pull_requests":true,"conversation":true,"gate":true,"replies":true}'
    ;;
  pull-requests)
    printf '%s' '[
      { "id": "app/101", "repo": "app", "number": "101",
        "title": "Widen the retry window",
        "url": "https://acme.example/pr/101",
        "branch": "you/ABC-42-retry",
        "updated_at": "2026-07-30T12:00:00Z",
        "role": "author", "state": "open", "cited": false,
        "gate": { "repos": [ { "repo": "app", "passed": 5, "failed": 1, "running": 0 } ] } }
    ]'
    ;;
  *)
    printf '[]'
    ;;
esac
"#;

/// A runtime that does what rhei did on the reported root: it detaches, finds
/// the gate closed, spawns nothing, and writes a stream with no slot and no
/// pass in it — `run_started`, then `run_finished` — beside a report saying it
/// stopped for human attention. It reports itself finished inside the
/// handshake, so it leaves no lock and the root is a candidate again on the very
/// next sweep. One line per start is the measurement: the ticket counted three
/// where there should have been none.
const ACME_RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
verb="$1"; shift
if [ "$verb" = run ] && [ "${1:-}" = --help ]; then
  printf '%s\n' '      --headless  Detach the run'
  exit 0
fi
while [ "${1:-}" = --headless ] || [ "${1:-}" = --json ]; do
  shift
done
root="$1"
printf '%s\n' "$root" >> "$HOME/starts"
id="acme-run-$(wc -l < "$HOME/starts" | tr -d ' ')"
mkdir -p "$root/runtime"
{
  printf '{"seq":1,"ts":"2026-10-01T22:54:43Z","event":"run_started","schema":1,"run_id":"%s","workspace":"%s","parallel":1,"total_tasks":2}\n' "$id" "$root"
  printf '{"seq":2,"ts":"2026-10-01T22:54:43Z","event":"run_finished","summary":{"agents_spawned":0,"programs_spawned":0,"terminal_tasks":0,"total_tasks":2}}\n'
} > "$root/runtime/events.jsonl"
printf '# Run Report\n\nRun: %s\nDuration: 0.1s\nResult: stopped for human attention\n' "$id" > "$root/runtime/run-report.md"
printf '{"id":"%s","status":"finished"}\n' "$id"
"#;

/// The machine on the reported root, narrowed to what the sweep has to tell
/// apart: work an agent does, a supervisor's state that is not a gate, a gate a
/// person moves, and the end.
const MACHINE: &str = "name: ephor-work
version: 1.0

states:
  fix:
    description: Do what the ticket asks.
    agent: claude-code
  supervising:
    description: The supervisor chooses the path. Not a gate.
    agent: claude-code
  human-gate:
    description: A person decides whether this goes on.
    gating: true
  done:
    final: true
  cancelled:
    final: true
";

/// A world watching the forest, with the runtime bound and one autorun recipe,
/// whose ticket is written before any sweep runs and whose root runs under
/// [`MACHINE`]. The ticket is written with autorun off and turned on afterwards,
/// so nothing has run when the case begins (§FS-005-dispatch.24).
fn watching() -> World {
    let world = World::new();
    world.stub("ephor-forge-acmeforge", ACME_FORGE);
    world.stub("acme-runtime", ACME_RUNTIME);
    world.configure(configured(false));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world.ephor().args(["work", "dispatch"]).assert().success();
    assert_eq!(starts(&world), 0, "the ticket is written before any run");
    std::fs::write(work_root(&world).join("states.yaml"), MACHINE).expect("the machine");
    world.configure(configured(true));
    world
}

fn configured(autorun: bool) -> serde_json::Value {
    json!({
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
                "autorun": autorun,
                "when": { "kinds": ["pr"], "roles": ["author"], "gate": "failing" },
                "brief": "Fix the gate on {title}."
            }]
        }
    })
}

fn work_root(world: &World) -> std::path::PathBuf {
    world.forest().join("panta")
}

/// The one plan the dispatch wrote, and its id.
fn plan_of(world: &World) -> (std::path::PathBuf, String) {
    let plan = std::fs::read_dir(work_root(world))
        .expect("the work root")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| path.to_string_lossy().ends_with(".rhei.md"))
        .expect("the plan the dispatch wrote");
    let id = plan
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".rhei.md"))
        .expect("a plan id")
        .to_string();
    (plan, id)
}

/// Put the plan into the reported shape: the dispatched ticket is a supervisor
/// in `supervising`, and its triage subtask waits in the gate.
fn gate_the_supervisor(world: &World) {
    let (plan, _) = plan_of(world);
    let text = std::fs::read_to_string(&plan).expect("the plan");
    assert!(
        text.contains("**State:** fix\n"),
        "the dispatched ticket starts in `fix`:\n{text}"
    );
    let text = text.replacen("**State:** fix\n", "**State:** supervising\n", 1);
    let text = format!(
        "{}\n#### Task fix-gate-1.triage: Triage the report\n**State:** human-gate\n\nTriage it.\n",
        text.trim_end()
    );
    std::fs::write(&plan, text).expect("the gated plan");
}

/// What a person does: the triage ticket leaves its gate.
fn the_person_answers(world: &World) {
    let (plan, _) = plan_of(world);
    let text = std::fs::read_to_string(&plan).expect("the plan");
    assert!(text.contains("**State:** human-gate\n"), "{text}");
    std::fs::write(
        &plan,
        text.replacen("**State:** human-gate\n", "**State:** done\n", 1),
    )
    .expect("the answered plan");
}

/// How many runs the runtime has actually been asked to make.
fn starts(world: &World) -> usize {
    std::fs::read_to_string(world.path().join("starts"))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

/// One due sweep at one project's width, which acts.
fn sweep(world: &World, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["work", "run", "--due"];
    args.extend_from_slice(extra);
    args.push("--json");
    world
        .ephor_raw()
        .args(&args)
        .output()
        .expect("the sweep runs")
}

/// Every rest ephor's own record holds is over: each instant in it is moved
/// three hours back, past the longest rest there is. The reproducer's own step,
/// so a sweep after it is asking the due test and nothing else.
fn elapse(world: &World) {
    fn back(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, field) in map.iter_mut() {
                    if key == "at" {
                        if let Some(at) = field
                            .as_str()
                            .and_then(|at| at.parse::<chrono::DateTime<chrono::Utc>>().ok())
                        {
                            *field = json!((at - chrono::Duration::hours(3)).to_rfc3339());
                            continue;
                        }
                    }
                    back(field);
                }
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(back),
            _ => {}
        }
    }
    let path = world.path().join("state/ephor/work.json");
    if !path.exists() {
        return;
    }
    let mut ledger = read_json(&path);
    back(&mut ledger);
    write_json(&path, &ledger);
}

/// Whether ephor's own record holds a no-advance verdict about any run here.
/// Read as text, as E2E-022 reads it: what the record is called is the
/// implementation's.
fn ledger_names_a_verdict(world: &World) -> bool {
    std::fs::read_to_string(world.path().join("state/ephor/work.json"))
        .unwrap_or_default()
        .contains("acme-run-")
}

/// The row a reading holds about this project's root.
fn row_of<'a>(reading: &'a serde_json::Value, project: &str) -> &'a serde_json::Value {
    reading["runs"]
        .as_array()
        .and_then(|runs| runs.iter().find(|run| run["project"] == project))
        .unwrap_or_else(|| panic!("no row about {project}: {reading:#}"))
}

/// The row of a root waiting on a person, as data and as the sentence rendered
/// from it (§FS-005-dispatch.24.3.2, §FS-005-dispatch.24.2).
fn assert_waits_on_a_person(row: &serde_json::Value, plan_id: &str, reading: &serde_json::Value) {
    let gated = format!("{plan_id}.fix-gate-1.triage");
    assert_eq!(
        row["outcome"], "passed-over",
        "a root waiting on a person is passed over, never started: {reading:#}"
    );
    assert_eq!(
        row["hold"],
        json!({ "kind": "person", "tickets": [ { "ticket": gated, "state": "human-gate" } ] }),
        "the hold names the gated ticket and the state it waits in: {reading:#}"
    );
    let why = row["reason"].as_str().unwrap_or_default();
    assert!(
        why.contains("waits on a person") && why.contains(&gated) && why.contains("human-gate"),
        "the reason says it waits on a person and names the gate: {why}"
    );
    assert_eq!(
        row["tickets"],
        json!([gated]),
        "the ticket list under the row names the gated tickets: {reading:#}"
    );
}

/// The reproduction, from the outside: sweep after sweep starts nothing on a
/// root whose only would-be-due ticket is a supervisor above a gated subtask,
/// takes no strike there, and starts it once the person moves the gate
/// (§FS-005-dispatch.24.3).
#[test]
fn a_root_waiting_on_a_person_is_passed_over_and_taken_up_once_the_gate_moves() {
    let world = watching();
    gate_the_supervisor(&world);
    let (_, plan_id) = plan_of(&world);

    for round in 1..=6 {
        let output = sweep(&world, &[]);
        assert!(output.status.success(), "{output:?}");
        let reading = shaped("work-run", &output);
        assert_eq!(
            starts(&world),
            0,
            "sweep {round} started a run on a root waiting on a person — the runtime halts at \
             the gate having done nothing, which is the loop ephor#160 reports: {reading:#}"
        );
        assert_eq!(reading["failed"], 0, "{reading:#}");
        assert_waits_on_a_person(row_of(&reading, PROJECT), &plan_id, &reading);
        assert!(
            !ledger_names_a_verdict(&world),
            "a wait is never a strike, so nothing is judged (§FS-005-dispatch.24.3.3)"
        );
        elapse(&world);
    }

    // The text says the same and is headed as passed over, never as a start
    // (§FS-005-dispatch.24.1).
    let text = world
        .ephor_raw()
        .args(["work", "run", "--due"])
        .output()
        .expect("the sweep runs");
    assert!(text.status.success(), "{text:?}");
    let said = String::from_utf8_lossy(&text.stdout).into_owned();
    assert!(said.contains("passed over"), "{said}");
    assert!(said.contains("waits on a person"), "{said}");
    assert!(said.contains("fix-gate-1.triage"), "{said}");
    assert!(
        !said.contains('▶'),
        "no start marker on a root that got no run: {said}"
    );
    assert_eq!(starts(&world), 0, "{said}");

    // The person answers. The next sweep finds the root due with nothing
    // remembered against it.
    the_person_answers(&world);
    let after = sweep(&world, &[]);
    assert!(after.status.success(), "{after:?}");
    let after = json_of(&after);
    assert_eq!(
        starts(&world),
        1,
        "once the gate moves the root is due again on the next sweep: {after:#}"
    );
    assert_eq!(row_of(&after, PROJECT)["outcome"], "done", "{after:#}");
}

/// A root already stopped by three empty runs — the state the reported root
/// reached before this point existed — is found waiting on a person, and that
/// sweep drops the stop, so the root comes back by itself once the person moves
/// the gate rather than needing a start by hand (§FS-005-dispatch.24.3.3).
#[test]
fn a_root_found_waiting_on_a_person_loses_its_no_advance_stop() {
    let world = watching();
    let (_, plan_id) = plan_of(&world);

    // Three runs in a row on the root advance nothing. The sweep makes the
    // first; the reader starts the next two by name, which the rest never
    // refuses, and each sweep in between takes its verdict on the last one.
    let first = json_of(&sweep(&world, &[]));
    assert_eq!(row_of(&first, PROJECT)["outcome"], "done", "{first:#}");
    for _ in 0..2 {
        let rested = json_of(&sweep(&world, &[]));
        assert_eq!(
            row_of(&rested, PROJECT)["outcome"],
            "passed-over",
            "{rested:#}"
        );
        world
            .ephor()
            .args(["work", "run", "--item", ITEM, "--json"])
            .assert()
            .success();
    }
    let stopped = json_of(&sweep(&world, &[]));
    assert_eq!(
        row_of(&stopped, PROJECT)["hold"]["kind"],
        "stopped",
        "the fixture is a root the sweep has stopped starting: {stopped:#}"
    );
    assert_eq!(starts(&world), 3, "three runs, none of which advanced");

    // The gate closes over the supervisor. The first sweep that finds the root
    // waiting says so — the wait comes before the stop in the order of holds —
    // and drops the stop.
    gate_the_supervisor(&world);
    let output = sweep(&world, &[]);
    assert!(output.status.success(), "{output:?}");
    let waiting = shaped("work-run", &output);
    assert_waits_on_a_person(row_of(&waiting, PROJECT), &plan_id, &waiting);
    assert!(
        !ledger_names_a_verdict(&world),
        "a sweep that finds the root waiting on a person drops its no-advance record"
    );
    assert_eq!(starts(&world), 3, "{waiting:#}");

    // So the person's answer is enough on its own.
    the_person_answers(&world);
    let after = json_of(&sweep(&world, &[]));
    assert_eq!(
        starts(&world),
        4,
        "a root stopped before it was found waiting stayed stopped after the gate moved: \
         {after:#}"
    );
    assert_eq!(row_of(&after, PROJECT)["outcome"], "done", "{after:#}");
}

/// A mixed root: one tree held by its gate, another ready. The root is due, the
/// run is made, and the row lists the ready ticket alone — one tree never holds
/// back another (§FS-005-dispatch.24.3.1, §FS-005-dispatch.24.3.2).
#[test]
fn a_gate_holds_its_own_tree_and_never_a_sibling() {
    let world = watching();
    gate_the_supervisor(&world);
    let (plan, plan_id) = plan_of(&world);
    let text = std::fs::read_to_string(&plan).expect("the plan");
    std::fs::write(
        &plan,
        format!(
            "{}\n\n### Task fix-gate-2: Fix the other gate\n**State:** fix\n\nFix it.\n",
            text.trim_end()
        ),
    )
    .expect("a second, ready tree");

    let output = sweep(&world, &[]);
    assert!(output.status.success(), "{output:?}");
    let reading = shaped("work-run", &output);
    assert_eq!(
        starts(&world),
        1,
        "a ready tree beside the gate starts the root: {reading:#}"
    );
    let row = row_of(&reading, PROJECT);
    assert_eq!(row["outcome"], "done", "{reading:#}");
    assert_eq!(
        row["tickets"],
        json!([format!("{plan_id}.fix-gate-2")]),
        "the row lists the ready tickets only, never one a gate holds: {reading:#}"
    );
}

/// The gated report asks the wait too, since it needs no capacity read, and
/// gives the same row the acting sweep would (§FS-005-dispatch.24.2,
/// §FS-011-command-line.10).
#[test]
fn the_gated_report_names_a_root_waiting_on_a_person() {
    let world = watching();
    gate_the_supervisor(&world);
    let (_, plan_id) = plan_of(&world);
    watch_a_second_project(&world);

    let output = sweep(&world, &[]);
    assert!(output.status.success(), "{output:?}");
    let report = shaped("work-run", &output);
    assert_eq!(report["gated"], json!(true), "{report:#}");
    assert_waits_on_a_person(row_of(&report, PROJECT), &plan_id, &report);
    assert_eq!(starts(&world), 0, "a gated report started a run");
}

/// A run asked for by name is blind to the wait, as to every guard the sweep has
/// because nobody is present: the reader keeps the key (§FS-005-dispatch.24.3.3,
/// §FS-005-dispatch.30).
#[test]
fn a_run_asked_for_by_name_is_blind_to_the_wait() {
    let world = watching();
    gate_the_supervisor(&world);
    world
        .ephor()
        .args(["work", "run", "--item", ITEM, "--json"])
        .assert()
        .success();
    assert_eq!(
        starts(&world),
        1,
        "the wait refused a run the reader asked for by name"
    );
}

/// A second watched project with no providers and no work, so a bare sweep is a
/// sweep over two (§FS-011-command-line.10).
fn watch_a_second_project(world: &World) {
    let far = world.path().join(OTHER);
    std::fs::create_dir_all(&far).expect("the other forest");
    let mut registry = world.registry_doc();
    let mut row = registry["projects"][0].clone();
    row["id"] = json!(OTHER);
    row["display_name"] = json!("Far");
    row["root"] = json!(far.to_string_lossy());
    registry["projects"]
        .as_array_mut()
        .expect("projects")
        .push(row);
    write_json(&world.registry_path(), &registry);
    let mut settings = configured(true);
    settings["projects"][OTHER] = json!({ "providers": [] });
    world.configure(settings);
}
