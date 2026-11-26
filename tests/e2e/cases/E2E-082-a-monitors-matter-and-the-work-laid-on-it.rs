//! E2E-082-a-monitors-matter-and-the-work-laid-on-it: work laid on a
//! monitor's matter by a recipe the site writes for it
//! (§FS-021-monitors.16.3) ends, and the first plain refresh after reads the
//! monitor out of cadence (§FS-021-monitors.15). Still down, the matter
//! resurfaces once with `still down after its work ended`, makes its work
//! stale, and is New again though the ledger records that work
//! (§FS-021-monitors.13.2, §FS-019-waiting.1.2, §FS-005-dispatch.5). A
//! passing reading there counts toward recovery and moves nothing.
//!
//! Work laid on a drop is made stale by what moves the matter and by nothing
//! else: a value wobbling in its band does not, a worse level and a changed
//! condition do, each saying why in its own sentence (§FS-021-monitors.13.1,
//! §FS-021-monitors.13.2, §FS-005-dispatch.5). And a matter that recovered
//! while a run still has its ticket in hand stays on the feed until that run
//! lets it go (§FS-021-monitors.14.3, §FS-003-feed-categories.2).
//!
//! The runtime is a stub that offers no workflows and lists no runs; a live
//! run is a held lock and the journal line naming the ticket it holds, as
//! E2E-013 has it.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use predicates::prelude::*;
use serde_json::{json, Value};

use support::*;

/// Appends a line per run to `kpis.runs` and answers what `coverage.value`
/// and `release.value` hold, so a case changes the world without rewriting
/// the script.
const KPIS: &str = r#"#!/bin/sh
echo run >> "@WORLD@/kpis.runs"
printf '{"v":1,"monitors":[{"id":"coverage","value":%s},{"id":"release","value":"%s"}]}' \
  "$(cat "@WORLD@/coverage.value")" "$(cat "@WORLD@/release.value")" > "$EPHOR_ANSWER"
"#;

/// A runtime that offers no workflows, lists no runs, and starts nothing.
const RUNTIME: &str = r#"#!/bin/sh
case "${1:-}" in
  list|templates|runs) printf '[]' ;;
  *) exit 0 ;;
esac
"#;

fn site(coverage: Value) -> Value {
    json!({
        "projects": { PROJECT: {
            "providers": [],
            "monitors": [
                coverage,
                { "id": "release", "levels": ["ok", "degraded", "failed"],
                  "healthy": { "at_most": "ok" }, "from": { "command": "scripts/kpis.sh" } }
            ]
        } },
        "work": {
            "runner": "acme-runtime",
            "recipes": [{
                "id": "mend", "icon": "M", "description": "mend what the monitor saw",
                "state": "fix", "needs_checkout": false,
                "when": { "sources": ["monitor"] }, "brief": "{title}"
            }]
        }
    })
}

fn coverage_at(min: u32) -> Value {
    json!({ "id": "coverage", "title": "Line coverage", "unit": "%",
            "healthy": { "min": min }, "recover_after": 2,
            "from": { "command": "scripts/kpis.sh" } })
}

fn world(coverage_value: &str, release: &str) -> World {
    let world = World::new();
    world.script(
        "scripts/kpis.sh",
        &KPIS.replace("@WORLD@", &world.path().to_string_lossy()),
    );
    world.stub("acme-runtime", RUNTIME);
    world.configure(site(coverage_at(80)));
    answer(&world, coverage_value, release);
    world
}

fn answer(world: &World, coverage: &str, release: &str) {
    fs::write(world.path().join("coverage.value"), coverage).unwrap();
    fs::write(world.path().join("release.value"), release).unwrap();
}

/// How many times the monitors' command ran.
fn runs(world: &World) -> usize {
    fs::read_to_string(world.path().join("kpis.runs"))
        .unwrap_or_default()
        .lines()
        .count()
}

fn refresh(world: &World, now: bool) {
    let mut args = vec!["refresh", PROJECT];
    if now {
        args.push("--monitors-now");
    }
    world.ephor().args(&args).assert().success();
}

fn matter_of(world: &World, monitor: &str) -> Value {
    let prefix = format!("monitor:{PROJECT}/{monitor}@");
    world.feed()["providers"]["monitor"]["matters"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|matter| matter["key"].as_str().unwrap().starts_with(&prefix))
        .cloned()
        .unwrap_or_else(|| panic!("no {monitor} drop in {:#}", world.feed()))
}

fn dispatch(world: &World, opened: usize) {
    world
        .ephor()
        .args(["work", "dispatch", "--project", PROJECT])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "{opened} ticket(s) opened"
        )));
}

/// The plans laid in the work root.
fn plans(world: &World) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(world.forest().join("panta")) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.to_string_lossy().ends_with(".rhei.md"))
        .collect()
}

/// The runtime finishes every ticket laid, as a run that ended would.
fn finish(plan: &Path) {
    let text = fs::read_to_string(plan).unwrap();
    assert!(text.contains("**State:** fix"), "{text}");
    fs::write(plan, text.replace("**State:** fix", "**State:** done")).unwrap();
}

fn waiting(world: &World) -> Value {
    let out = world.ephor().args(["waiting", "--json"]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    shaped("waiting", &out)
}

fn is_new(reading: &Value, key: &Value) -> bool {
    reading["new"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| &row["id"] == key)
}

/// The ledger's account of the work on `key`, held to its published shape.
fn work_on(world: &World, key: &Value) -> Value {
    let out = world
        .ephor()
        .args(["work", "list", "--json"])
        .output()
        .unwrap();
    let listed = shaped("work-list", &out);
    listed
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| &entry["item"] == key)
        .cloned()
        .unwrap_or_else(|| panic!("no work on {key} in {listed:#}"))
}

#[test]
fn still_down_after_its_work_ended_is_new_again() {
    let world = world("79.2", "ok");
    refresh(&world, false);
    assert_eq!(runs(&world), 1, "never read, so due");
    let key = matter_of(&world, "coverage")["key"].clone();
    assert!(is_new(&waiting(&world), &key));

    dispatch(&world, 1);
    assert!(!is_new(&waiting(&world), &key), "work is laid on it");
    // Work laid and still open reads nothing out of cadence.
    refresh(&world, false);
    assert_eq!(runs(&world), 1);

    for plan in plans(&world) {
        finish(&plan);
    }
    refresh(&world, false);
    assert_eq!(runs(&world), 2, "the work ended, so it is read again");
    let matter = matter_of(&world, "coverage");
    assert_eq!(matter["key"], key, "the same episode");
    assert_eq!(matter["state"], json!("down"));
    assert!(
        matter["raw"]["_ephor_monitor"]["moves"]["renewed"].is_string(),
        "{matter:#}"
    );
    assert_eq!(
        matter["fingerprint"]["health"]["renewed"],
        matter["raw"]["_ephor_monitor"]["moves"]["renewed"],
        "{matter:#}"
    );

    let reading = waiting(&world);
    assert!(is_new(&reading, &key), "New again: {reading:#}");
    let work = work_on(&world, &key);
    assert_eq!(work["stale"], json!(true), "{work:#}");
    assert!(
        work["changes"]
            .as_array()
            .unwrap()
            .contains(&json!("still down after its work ended")),
        "{work:#}"
    );
    // Offered work again: processing it is not refused for the work it had.
    let out = world
        .ephor()
        .args([
            "work",
            "process",
            "--item",
            key.as_str().unwrap(),
            "--dry-run",
            "--json",
        ])
        .output()
        .unwrap();
    let account = shaped("work-process", &out);
    assert_eq!(account["processed"][0]["ok"], json!(true), "{account:#}");

    // Once per work that ended: the next refresh reads it on its cadence.
    refresh(&world, false);
    assert_eq!(runs(&world), 2);
    assert_eq!(
        matter_of(&world, "coverage")["updated_at"],
        matter["updated_at"]
    );
}

/// The fix landed: the read after the work ended passes, which counts toward
/// recovery and moves nothing, so the work is not stale, nothing would be
/// reopened, and the matter is not New again (§FS-021-monitors.15,
/// §FS-021-monitors.13.1).
#[test]
fn a_passing_reading_after_its_work_ended_moves_nothing() {
    let world = world("79.2", "ok");
    refresh(&world, false);
    let key = matter_of(&world, "coverage")["key"].clone();
    dispatch(&world, 1);
    for plan in plans(&world) {
        finish(&plan);
    }
    answer(&world, "81.0", "ok");
    refresh(&world, false);
    assert_eq!(runs(&world), 2, "the work ended, so it is read again");
    let matter = matter_of(&world, "coverage");
    assert_eq!(matter["key"], key);
    assert_eq!(matter["state"], json!("down"), "1 of 2 recovery reads");
    assert_eq!(matter["fingerprint"]["health"].get("renewed"), None);

    let work = work_on(&world, &key);
    assert_eq!(work["stale"], json!(false), "{work:#}");
    assert!(!is_new(&waiting(&world), &key));
    world
        .ephor()
        .args(["work", "sync", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("would reopen").not());

    refresh(&world, true);
    let matter = matter_of(&world, "coverage");
    assert_eq!(matter["state"], json!("recovered"), "{matter:#}");
}

#[test]
fn only_what_moves_the_matter_makes_its_work_stale() {
    let world = world("79.2", "degraded");
    refresh(&world, false);
    let coverage = matter_of(&world, "coverage")["key"].clone();
    let release = matter_of(&world, "release")["key"].clone();
    dispatch(&world, 2);

    answer(&world, "79.3", "degraded");
    refresh(&world, true);
    let work = work_on(&world, &coverage);
    assert_eq!(work["stale"], json!(false), "a wobble: {work:#}");
    assert_eq!(work_on(&world, &release)["stale"], json!(false));

    std::thread::sleep(std::time::Duration::from_millis(1100));
    answer(&world, "79.3", "failed");
    refresh(&world, true);
    let work = work_on(&world, &release);
    assert_eq!(work["stale"], json!(true), "{work:#}");
    assert_eq!(
        work["changes"],
        json!(["level worse: degraded → failed"]),
        "{work:#}"
    );

    world.configure(site(coverage_at(85)));
    refresh(&world, true);
    let work = work_on(&world, &coverage);
    assert_eq!(work["stale"], json!(true), "{work:#}");
    assert_eq!(
        work["changes"],
        json!(["condition changed: healthy ≥ 80% → healthy ≥ 85%"]),
        "{work:#}"
    );
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A run holding `ticket` of `plan` at `fix` in `root`: its lock held, and
/// the journal naming the ticket it took up.
fn a_run_holds(root: &Path, plan: &str, ticket: &str) -> fs::File {
    let lock = root.join(".rhei/run.lock");
    write(&lock, "");
    let holder = fs::File::open(&lock).unwrap();
    holder.lock().unwrap();
    let log = format!("runtime/logs/task-{ticket}.log");
    write(
        &root.join("runtime/transitions.log"),
        &format!("2026-11-25T06:05:00Z  {plan}.{ticket}  start@fix  {log}\n"),
    );
    write(&root.join(&log), "at it\n");
    holder
}

#[test]
fn a_recovered_matter_stays_while_a_run_has_its_work_in_hand() {
    let world = world("79.2", "ok");
    refresh(&world, false);
    let key = matter_of(&world, "coverage")["key"].clone();
    dispatch(&world, 1);
    let plan = plans(&world).remove(0);
    let plan_id = plan
        .file_name()
        .unwrap()
        .to_string_lossy()
        .trim_end_matches(".rhei.md")
        .to_string();
    let text = fs::read_to_string(&plan).unwrap();
    let ticket = text
        .lines()
        .find_map(|line| line.strip_prefix("### Task "))
        .and_then(|rest| rest.split(':').next())
        .unwrap()
        .to_string();
    let holder = a_run_holds(&world.forest().join("panta"), &plan_id, &ticket);

    answer(&world, "81.0", "ok");
    refresh(&world, true);
    refresh(&world, true);
    let matter = matter_of(&world, "coverage");
    assert_eq!(matter["key"], key);
    assert_eq!(matter["state"], json!("recovered"), "{matter:#}");

    let reading = waiting(&world);
    let running: Vec<&Value> = reading["running"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["item"] == key)
        .collect();
    assert_eq!(running.len(), 1, "{reading:#}");
    assert_eq!(
        running[0]["tickets"].as_array().map(Vec::len),
        Some(1),
        "the run has the ticket in hand: {reading:#}"
    );
    assert_eq!(reading["overview"]["matters"], json!(1), "{reading:#}");

    // The run lets it go, finished: the recovered matter has no loose end
    // left, and leaves the feed.
    drop(holder);
    finish(&plan);
    let reading = waiting(&world);
    assert_eq!(reading["overview"]["matters"], json!(0), "{reading:#}");
}
