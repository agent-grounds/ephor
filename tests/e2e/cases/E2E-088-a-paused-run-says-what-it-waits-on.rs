//! E2E-088-a-paused-run-says-what-it-waits-on: a live run with nothing it can
//! do until a person or the world answers reads as paused, says what it waits
//! on and when it next looks, and is counted apart from the runs at work.
//!
//! The scenario is §FS-019-waiting.1.3.1 from the outside. A run holds a work
//! root's lock and no ticket. Its one open ticket polls a release tag, and the
//! runtime recorded the poll's next attempt half an hour out; the runtime's own
//! ready set for the root is empty. `ephor waiting` reads that run as paused,
//! with the header counting it apart (§FS-019-waiting.3), and `--json` carries
//! the waits whole, held to the published `waiting` shape (§FS-019-waiting.6).
//! The operations board says `paused` of the same run and keeps it live
//! (§FS-005-dispatch.15), in `ephor operations` as in its `--json`. A gate
//! beside the poll is listed first. A ready set that names a task, or one the
//! runtime cannot give, is a run between turns again, because paused is a
//! positive finding.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use support::*;

/// A runtime that lists nothing beyond the plans on disk and answers its ready
/// set from `ready.json` beside its data, refusing where there is none.
const RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
data="$(dirname "$0")/../runtime-data"
case "${1:-}" in
  list)
    case " $* " in
      *" --ready "*)
        [ -f "$data/ready.json" ] || { echo "no ready set for this root" >&2; exit 1; }
        cat "$data/ready.json" ;;
      *) printf '[]' ;;
    esac ;;
  *) exit 0 ;;
esac
"#;

/// The root's machine: a gate a person moves, a poll on a release tag, and
/// the end.
const MACHINE: &str = "name: ephor-work
version: 1.0

states:
  needs-human:
    description: A person decides whether this goes on.
    gating: true
  holding:
    description: Wait for the release tag.
    program: [\"bash\", \"hold.sh\"]
    poll:
      interval: 30m
      waiting_on: a release tag
  done:
    final: true
";

fn watching() -> World {
    let world = World::new();
    settle(&world.stub("acme-runtime", RUNTIME));
    fs::create_dir_all(world.path().join("runtime-data")).expect("the stub's data");
    world.configure(json!({ "work": { "runner": "acme-runtime" } }));
    world
}

fn work_root(world: &World) -> PathBuf {
    world.forest().join("panta")
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    fs::write(path, content).expect("write");
}

/// What the runtime answers for its ready set.
fn ready(world: &World, answer: Option<&str>) {
    let path = world.path().join("runtime-data/ready.json");
    match answer {
        Some(answer) => write(&path, answer),
        None => fs::remove_file(&path).expect("the ready set"),
    }
}

/// The plan a run left: a ticket polling a release tag whose next attempt the
/// runtime recorded at `next`, the epoch second, and, where `gate` is asked
/// for, a ticket parked at a gate beside it.
fn plan(world: &World, next: i64, gate: bool) {
    let root = work_root(world);
    write(&root.join("states.yaml"), MACHINE);
    let gated = match gate {
        true => "### Task specify: Specify the retry window\n**State:** needs-human\n\nDecide.\n\n",
        false => "",
    };
    write(
        &root.join("demo-7.rhei.md"),
        &format!(
            "# Rhei: Widen the retry window\n**States:** ephor-work\n\n\
             ---\nmetadata:\n  tasks:\n    ship:\n      pollNextAttemptAt:\n        holding: {next}\n---\n\n\
             ## Tasks\n\n{gated}\
             ### Task ship: Ship the retry window\n**State:** holding\n\nWait for the tag.\n\n\
             ### Task fix-1: Fix it\n**State:** done\n\nDone.\n"
        ),
    );
}

/// What a live run between its turns leaves: its lock held and the descriptor
/// naming it, and no ticket in hand.
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
    holder
}

fn run(world: &World, args: &[&str]) -> std::process::Output {
    let out = world.ephor().args(args).output().expect("ephor");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// The seconds since the epoch a published time stands for.
fn epoch(at: &Value) -> i64 {
    chrono::DateTime::parse_from_rfc3339(at.as_str().expect("a time"))
        .expect("an RFC 3339 time")
        .timestamp()
}

#[test]
fn a_live_run_with_nothing_it_can_do_says_it_is_paused_and_on_what() {
    let world = watching();
    // Half an hour and half a minute out, so the row reads thirty minutes
    // however long the command takes to start.
    let next = chrono::Utc::now().timestamp() + 30 * 60 + 30;
    plan(&world, next, false);
    ready(&world, Some("[]"));
    let _holder = a_run_is_live(&work_root(&world));

    // The machine form: still a live run under `running`, carrying what it
    // waits on (§FS-019-waiting.6).
    let view = shaped("waiting", &run(&world, &["waiting", "--json"]));
    let running = view["running"].as_array().expect("rows");
    assert_eq!(running.len(), 1, "{view}");
    assert_eq!(running[0]["run"], "3f9a2c", "{view}");
    assert!(
        running[0]["tickets"]
            .as_array()
            .expect("tickets")
            .is_empty(),
        "{view}"
    );
    let paused = &running[0]["paused"];
    assert_eq!(
        paused["says"], "paused · waiting on a release tag (ship) · next look in 30m",
        "{view}"
    );
    let waits = paused["waits"].as_array().expect("waits");
    assert_eq!(waits.len(), 1, "{view}");
    assert_eq!(waits[0]["ticket"], "demo-7.ship", "{view}");
    assert_eq!(waits[0]["task"], "ship", "{view}");
    assert_eq!(waits[0]["state"], "holding", "{view}");
    assert_eq!(waits[0]["kind"], "poll", "{view}");
    assert_eq!(waits[0]["waiting_on"], "a release tag", "{view}");
    assert_eq!(epoch(&waits[0]["next_attempt_at"]), next, "{view}");
    assert_eq!(epoch(&paused["next_look_at"]), next, "{view}");

    // As a person reads it: counted apart in the header and the section, and
    // the row says what it waits on where a run between turns says so
    // (§FS-019-waiting.3, §FS-019-waiting.1.3.1).
    let text = String::from_utf8_lossy(&run(&world, &["waiting"]).stdout).into_owned();
    assert!(
        text.starts_with("ephor — waiting   0 on you · 0 new · 0 running · 1 paused · "),
        "{text}"
    );
    assert!(text.contains("\nRunning (0) · Paused (1)\n"), "{text}");
    let row = text
        .lines()
        .find(|line| line.starts_with("  ⏸ "))
        .unwrap_or_else(|| panic!("no paused row in:\n{text}"));
    assert!(row.contains(" · no matter"), "{row}");
    assert!(!text.contains("  ▶ "), "{text}");
    assert!(
        text.contains("      paused · waiting on a release tag (ship) · next look in 30m\n"),
        "{text}"
    );
    assert!(!text.contains("between turns"), "{text}");

    // The board says it of the same run, and the run is still live
    // (§FS-005-dispatch.15).
    let board = shaped("operations", &run(&world, &["operations", "--json"]));
    let op = &board["operations"][0];
    assert_eq!(op["state"], "paused", "{board}");
    assert_eq!(op["live"], true, "{board}");
    assert_eq!(op["paused"]["waits"][0]["ticket"], "demo-7.ship", "{board}");
    assert!(
        op["says"]
            .as_str()
            .expect("the row's words")
            .contains("· paused · waiting on a release tag (ship) · next look in 30m · run 3f9a2c"),
        "{board}"
    );
    // And as a person reads the board: marked paused, the state on the row's
    // first line, what it waits on on its second, and the way in still there.
    let text = String::from_utf8_lossy(&run(&world, &["operations"]).stdout).into_owned();
    let root = op["id"].as_str().expect("the root").to_string();
    let first = format!(
        "⏸ {} · {root}   paused\n",
        op["project"].as_str().expect("a project")
    );
    assert!(text.contains(&first), "{text}");
    assert!(
        text.contains(&format!(
            "    {root} · paused · waiting on a release tag (ship) · next look in 30m · run 3f9a2c"
        )),
        "{text}"
    );
    assert!(
        text.contains("    watch it:  acme-runtime attach '3f9a2c'"),
        "{text}"
    );
    assert!(!text.contains("▶ "), "{text}");

    // A gate beside the poll is a wait too, and the gates come first.
    plan(&world, next, true);
    let view = shaped("waiting", &run(&world, &["waiting", "--json"]));
    let paused = &view["running"][0]["paused"];
    assert_eq!(
        paused["says"],
        "paused · waiting on you (specify) · waiting on a release tag (ship) · next look in 30m",
        "{view}"
    );
    assert_eq!(paused["waits"][0]["kind"], "gate", "{view}");
    assert!(paused["waits"][0].get("waiting_on").is_none(), "{view}");
    assert!(
        paused["waits"][0].get("next_attempt_at").is_none(),
        "{view}"
    );
    assert_eq!(view["on_you"][0]["kind"], "gate", "{view}");

    // A ready set that names a task is a run about to take a turn.
    ready(
        &world,
        Some(r#"[{"id": "demo-7.ship", "state": "holding"}]"#),
    );
    let view = shaped("waiting", &run(&world, &["waiting", "--json"]));
    assert!(view["running"][0].get("paused").is_none(), "{view}");
    let text = String::from_utf8_lossy(&run(&world, &["waiting"]).stdout).into_owned();
    assert!(text.contains("· 1 running · "), "{text}");
    assert!(!text.contains("paused"), "{text}");
    assert!(text.contains("\nRunning (1)\n"), "{text}");
    assert!(text.contains("  ▶ "), "{text}");
    assert!(!text.contains("⏸"), "{text}");
    assert!(text.contains("      between turns\n"), "{text}");
    let text = String::from_utf8_lossy(&run(&world, &["operations"]).stdout).into_owned();
    assert!(
        text.contains(&format!(
            "▶ {}",
            board["operations"][0]["project"]
                .as_str()
                .expect("a project")
        )),
        "{text}"
    );
    assert!(text.contains(&format!("{root}   running\n")), "{text}");

    // And one the runtime cannot give is not one that said nothing is ready.
    ready(&world, None);
    let view = shaped("waiting", &run(&world, &["waiting", "--json"]));
    assert!(view["running"][0].get("paused").is_none(), "{view}");
    let board = shaped("operations", &run(&world, &["operations", "--json"]));
    assert_eq!(board["operations"][0]["state"], "running", "{board}");
}
