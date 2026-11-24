//! E2E-073-a-new-matter-is-processed-where-it-is-shown: every new row on
//! Waiting carries the rhei it would be assigned, and one command processes
//! it, dismisses it, or processes them all.
//!
//! The scenario is §FS-019-waiting.7 from the outside, read the way a program
//! reads it: `ephor waiting --json`, `ephor work process` and `ephor work
//! dismiss`, each held to its published shape. A world with five pull
//! requests of the reader's: three with a red gate that the site's recipe
//! hands over, one with a green gate that nothing hands over, and one
//! carrying the label the recipe excludes.
//!
//! Each new row proposes the first entry the menu would hand it over with
//! (§FS-019-waiting.7.1). The one nothing hands over is offered the runtime's
//! `triage`, which this runtime does not offer, so it says it is not
//! available; the held matter is offered nothing. Processing one lays its
//! ticket, records it and asks the sweep's own admission for a start
//! (§FS-019-waiting.7.2, §FS-005-dispatch.24.5): with room, it starts at
//! once; with `max_active` full, it is laid all the same and queued, and the
//! next sweep starts it. A dismissal needs a reason and is lifted when the
//! matter moves (§FS-019-waiting.7.4, §FS-005-dispatch.5). Processing every
//! row previews first, and above one project acts only under `--act`
//! (§FS-019-waiting.7.5, §FS-011-command-line.10). Three confirmations of the
//! same recipe name the settings that would make it automatic
//! (§FS-019-waiting.7.7).
//!
//! Every run here is a stand-in's: the runtime is a bash stub on the world's
//! PATH, and the world's HOME, state and configuration are its own.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use support::*;

/// A matter the recipe hands over.
const ITEM: &str = "acmeforge:app/101";
/// Two more of the same.
const SECOND: &str = "acmeforge:app/104";
const THIRD: &str = "acmeforge:app/105";
/// A matter of the reader's whose gate is green: no recipe, shipped or the
/// site's, hands it over.
const GREEN: &str = "acmeforge:app/103";
/// A matter the site's selector holds out of the machine's hands.
const DISCUSSED: &str = "acmeforge:app/102";

/// Three red pull requests of the reader's, one green, and one under
/// discussion. `$GREEN_AT` is when the green one last moved, so a case can
/// move it. Every call is counted in the world's HOME.
const ACME_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
printf '%s\n' "$1" >> "$HOME/forge-calls"
red='"gate": { "repos": [ { "repo": "app", "passed": 5, "failed": 1, "running": 0 } ] }'
case "${1:?subcommand}" in
  capabilities)
    printf '{"pull_requests":true,"conversation":true,"gate":true}'
    ;;
  pull-requests)
    printf '%s' '[
      { "id": "app/101", "repo": "app", "number": "101",
        "title": "Widen the retry window", "url": "https://acme.example/pr/101",
        "branch": "you/ABC-42-retry", "updated_at": "2026-07-30T12:00:00Z",
        "role": "author", "state": "open", "cited": false, "labels": [], '"$red"' },
      { "id": "app/102", "repo": "app", "number": "102",
        "title": "Rename the lock file", "url": "https://acme.example/pr/102",
        "branch": "you/rename-lock", "updated_at": "2026-07-30T13:00:00Z",
        "role": "author", "state": "open", "cited": false,
        "labels": ["human-discussion"], '"$red"' },
      { "id": "app/103", "repo": "app", "number": "103",
        "title": "Count generated files separately", "url": "https://acme.example/pr/103",
        "branch": "you/generated", "updated_at": "$GREEN_AT",
        "role": "author", "state": "open", "cited": false, "labels": [],
        "gate": { "repos": [ { "repo": "app", "passed": 6, "failed": 0, "running": 0 } ] } },
      { "id": "app/104", "repo": "app", "number": "104",
        "title": "Drop the second lock", "url": "https://acme.example/pr/104",
        "branch": "you/second-lock", "updated_at": "2026-07-30T15:00:00Z",
        "role": "author", "state": "open", "cited": false, "labels": [], '"$red"' },
      { "id": "app/105", "repo": "app", "number": "105",
        "title": "Name the retry budget", "url": "https://acme.example/pr/105",
        "branch": "you/budget", "updated_at": "2026-07-30T16:00:00Z",
        "role": "author", "state": "open", "cited": false, "labels": [], '"$red"' }
    ]'
    ;;
  *)
    printf '[]'
    ;;
esac
"#;

/// A runtime that offers no workflows — so no `triage` — lists no runs, and
/// starts a detached run by naming it, recording every start in the world's
/// HOME.
const ACME_RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
verb="${1:-}"
shift || true
case "$verb" in
  list|templates) printf '[]' ;;
  run)
    if [ "${1:-}" = --help ]; then
      printf '%s\n' '      --headless  Detach the run'
      exit 0
    fi
    printf '%s\n' "$*" >> "$HOME/starts"
    printf '{"id":"run-%s","status":"running"}\n' "$(wc -l < "$HOME/starts" | tr -d ' ')"
    ;;
  *) exit 0 ;;
esac
"#;

/// The site: one project watching the forge, the runtime bound, and one
/// recipe that hands a red pull request of the reader's over unless it is
/// under discussion. It asks for no sweep of its own, so whatever starts its
/// work here is processing's own ask.
fn configured(max_active: Value) -> Value {
    json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "acmeforge", "user": "you", "repos": ["app"] }
        ] } },
        "work": {
            "runner": "acme-runtime",
            "max_active": max_active,
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
    })
}

fn forge(world: &World, green_at: &str) {
    world.stub(
        "ephor-forge-acmeforge",
        &ACME_FORGE.replace("$GREEN_AT", green_at),
    );
}

fn watching() -> World {
    let world = World::new();
    forge(&world, "2026-07-30T14:00:00Z");
    world.stub("acme-runtime", ACME_RUNTIME);
    world.configure(configured(Value::Null));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

fn work_root(world: &World) -> PathBuf {
    world.forest().join("panta")
}

/// The plans laid in the work root.
fn plans(world: &World) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(work_root(world)) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.to_string_lossy().ends_with(".rhei.md"))
        .collect()
}

/// How many runs the runtime was asked to start.
fn starts(world: &World) -> usize {
    fs::read_to_string(world.path().join("starts"))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
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

/// How many times the forge was launched, for any reason.
fn forge_calls(world: &World) -> usize {
    fs::read_to_string(world.path().join("forge-calls"))
        .unwrap_or_default()
        .lines()
        .count()
}

fn new_row<'a>(reading: &'a Value, id: &str) -> Option<&'a Value> {
    reading["new"]
        .as_array()
        .expect("new rows")
        .iter()
        .find(|row| row["id"] == id)
}

/// `ephor work process …` with `--json`, held to the `work-process` shape.
fn process(world: &World, args: &[&str]) -> (bool, Value) {
    let mut all = vec!["work", "process"];
    all.extend_from_slice(args);
    all.push("--json");
    let out = world
        .ephor()
        .args(&all)
        .output()
        .expect("ephor work process");
    (out.status.success(), shaped("work-process", &out))
}

/// Every new row says what it would be assigned; the one nothing hands over
/// is offered `triage`, not available here; the held one is offered nothing
/// (§FS-019-waiting.7.1). The prose says the same, and reading the proposals
/// launches no forge (§FS-019-waiting.6).
#[test]
fn every_new_row_carries_the_rhei_it_would_be_assigned() {
    let world = watching();
    let fetched = forge_calls(&world);
    let reading = waiting(&world);
    assert_eq!(
        forge_calls(&world),
        fetched,
        "a reading asks the forge nothing"
    );

    let proposal = &new_row(&reading, ITEM).expect("the red one is new")["proposal"];
    assert_eq!(proposal["entry"], "fix-gate", "{reading:#}");
    assert_eq!(proposal["kind"], "agent", "{reading:#}");
    assert_eq!(proposal["gate"], "ready", "{reading:#}");
    assert_eq!(proposal["unambiguous"], true, "{reading:#}");
    assert_eq!(proposal["fallback"], false, "{reading:#}");
    assert_eq!(
        proposal["root"],
        json!(work_root(&world).to_string_lossy()),
        "the menu's own work root: {reading:#}"
    );

    let triage = &new_row(&reading, GREEN).expect("the green one is new")["proposal"];
    assert_eq!(triage["entry"], "triage", "{reading:#}");
    assert_eq!(triage["fallback"], true, "{reading:#}");
    assert_eq!(triage["unambiguous"], false, "{reading:#}");
    assert!(
        triage["refusal"]
            .as_str()
            .is_some_and(|says| says.contains("not available")),
        "{reading:#}"
    );

    assert!(new_row(&reading, DISCUSSED).is_none(), "{reading:#}");
    let held = &reading["held"][0];
    assert_eq!(held["id"], DISCUSSED, "{reading:#}");
    assert!(held.get("proposal").is_none(), "held is offered nothing");

    let prose = world
        .ephor()
        .arg("waiting")
        .output()
        .expect("ephor waiting");
    let prose = String::from_utf8_lossy(&prose.stdout);
    assert!(prose.contains("→ fix-gate"), "{prose}");
    assert!(prose.contains("→ triage · not available"), "{prose}");
}

/// The dry run is the account and writes nothing; confirming lays the
/// ticket, records the matter as processed and starts it through the sweep's
/// admission. A held matter, a matter with work already, and a matter
/// nothing can take are refused, each saying why (§FS-019-waiting.7.2).
#[test]
fn processing_lays_records_and_starts_what_it_proposed() {
    let world = watching();

    let (ok, account) = process(&world, &["--item", ITEM, "--dry-run"]);
    assert!(ok, "{account:#}");
    let row = &account["processed"][0];
    assert_eq!(row["entry"], "fix-gate", "{account:#}");
    assert_eq!(row["ok"], true, "{account:#}");
    assert!(row.get("start").is_none(), "a dry run starts nothing");
    assert!(plans(&world).is_empty(), "a dry run writes nothing");
    assert!(new_row(&waiting(&world), ITEM).is_some());

    let (ok, done) = process(&world, &["--item", ITEM]);
    assert!(ok, "{done:#}");
    let row = &done["processed"][0];
    assert_eq!(row["ok"], true, "{done:#}");
    assert_eq!(row["start"], "started", "{done:#}");
    assert_eq!(plans(&world).len(), 1, "one plan laid");
    assert_eq!(starts(&world), 1, "one run asked for");
    let reading = waiting(&world);
    assert!(new_row(&reading, ITEM).is_none(), "{reading:#}");
    assert!(new_row(&reading, SECOND).is_some(), "{reading:#}");
    assert_eq!(reading["queued"], json!([]), "{reading:#}");

    let (ok, again) = process(&world, &["--item", ITEM]);
    assert!(!ok, "{again:#}");
    assert!(
        again["processed"][0]["says"]
            .as_str()
            .is_some_and(|says| says.contains("already has work")),
        "{again:#}"
    );
    let (ok, held) = process(&world, &["--item", DISCUSSED]);
    assert!(!ok, "{held:#}");
    assert!(
        held["processed"][0]["says"]
            .as_str()
            .is_some_and(|says| says.contains("human-discussion")),
        "{held:#}"
    );
    let (ok, triage) = process(&world, &["--item", GREEN]);
    assert!(!ok, "{triage:#}");
    assert!(
        triage["processed"][0]["says"]
            .as_str()
            .is_some_and(|says| says.contains("not available")),
        "{triage:#}"
    );
    assert_eq!(plans(&world).len(), 1, "nothing refused was laid");
    assert_eq!(starts(&world), 1);
}

/// `--entry` chooses by name, and a name nothing answers to is refused with
/// where to look (§FS-019-waiting.7.3).
#[test]
fn another_entry_is_chosen_by_name() {
    let world = watching();
    let (ok, chosen) = process(
        &world,
        &["--item", SECOND, "--entry", "fix-gate", "--dry-run"],
    );
    assert!(ok, "{chosen:#}");
    assert_eq!(chosen["processed"][0]["entry"], "fix-gate", "{chosen:#}");

    let (ok, unknown) = process(
        &world,
        &["--item", SECOND, "--entry", "polish", "--dry-run"],
    );
    assert!(!ok, "{unknown:#}");
    assert!(
        unknown["processed"][0]["says"]
            .as_str()
            .is_some_and(|says| says.contains("ephor actions --item")),
        "{unknown:#}"
    );
}

/// A dismissal needs a reason, takes the matter out of New with nothing
/// laid, and is lifted the moment the matter moves (§FS-019-waiting.7.4).
#[test]
fn a_dismissal_needs_a_reason_and_lifts_when_the_matter_moves() {
    let world = watching();
    let dismiss = |reason: &str, item: &str| {
        let out = world
            .ephor()
            .args([
                "work", "dismiss", "--item", item, "--reason", reason, "--json",
            ])
            .output()
            .expect("ephor work dismiss");
        (out.status.success(), shaped("outcome", &out))
    };

    let (ok, refused) = dismiss("  ", GREEN);
    assert!(!ok, "{refused:#}");
    assert!(new_row(&waiting(&world), GREEN).is_some());
    let (ok, held) = dismiss("not mine", DISCUSSED);
    assert!(!ok, "a held matter is offered nothing: {held:#}");

    let (ok, recorded) = dismiss("green, and nothing to do", GREEN);
    assert!(ok, "{recorded:#}");
    assert!(
        recorded["says"]
            .as_str()
            .is_some_and(|says| says.contains("green, and nothing to do")),
        "{recorded:#}"
    );
    assert!(new_row(&waiting(&world), GREEN).is_none());
    assert!(plans(&world).is_empty(), "a dismissal lays nothing");

    // The pull request moves on the forge: the dismissal no longer stands,
    // and nothing was written to lift it.
    forge(&world, "2026-08-02T09:00:00Z");
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert!(new_row(&waiting(&world), GREEN).is_some());
}

/// `--all` previews first; confirmed on a full site it lays everything and
/// queues what the admission would not start, which Running names, and the
/// next sweep starts it. Three confirmations of the recipe name what would
/// make it automatic (§FS-019-waiting.7.5, §FS-019-waiting.7.6,
/// §FS-019-waiting.7.7).
#[test]
fn processing_all_lays_everything_and_starts_what_fits() {
    let world = watching();
    world.configure(configured(json!(0)));

    let (ok, preview) = process(&world, &["--all", "--dry-run"]);
    assert!(ok, "{preview:#}");
    let would: Vec<&str> = preview["processed"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["item"].as_str().expect("an id"))
        .collect();
    assert_eq!(would.len(), 3, "{preview:#}");
    for id in [ITEM, SECOND, THIRD] {
        assert!(would.contains(&id), "{preview:#}");
    }
    assert_eq!(preview["left"][0]["item"], GREEN, "{preview:#}");
    assert!(plans(&world).is_empty(), "the preview writes nothing");

    let (ok, done) = process(&world, &["--all"]);
    assert!(ok, "{done:#}");
    for row in done["processed"].as_array().expect("rows") {
        assert_eq!(row["ok"], true, "{done:#}");
        assert_eq!(row["start"], "queued", "{done:#}");
    }
    assert_eq!(starts(&world), 0, "nothing fits under max_active 0");

    let reading = waiting(&world);
    assert_eq!(
        reading["new"].as_array().map(Vec::len),
        Some(1),
        "{reading:#}"
    );
    let queued = reading["queued"].as_array().expect("queued rows");
    assert_eq!(queued.len(), 3, "{reading:#}");
    assert!(
        queued[0]["says"]
            .as_str()
            .is_some_and(|says| says.contains("max_active 0")),
        "the admission's own sentence: {reading:#}"
    );
    assert_eq!(
        reading["adopt"],
        "fix-gate confirmed for 3 pull requests · \"autorun\": true and \"dispatch\" on \
         fix-gate would do it without you",
        "{reading:#}"
    );
    let prose = world
        .ephor()
        .arg("waiting")
        .output()
        .expect("ephor waiting");
    let prose = String::from_utf8_lossy(&prose.stdout);
    assert!(prose.contains("queued 3 · "), "{prose}");

    // Room again: the sweep starts the queued work although the recipe asked
    // for no sweep of its own, and Running no longer names it as queued.
    world.configure(configured(Value::Null));
    world
        .ephor()
        .args(["work", "run", "--due", "--json"])
        .assert()
        .success();
    assert!(starts(&world) >= 1, "the sweep started the queued root");
    assert_eq!(waiting(&world)["queued"], json!([]));
}

/// Above one project `--all` is a sweep held to `--act`: it previews, says
/// so, and writes nothing; `--project` narrows it to one and it acts
/// (§FS-019-waiting.7.5, §FS-011-command-line.10).
#[test]
fn processing_all_above_one_project_acts_only_under_act() {
    let world = watching();
    let far = world.path().join("far");
    fs::create_dir_all(&far).expect("the other forest");
    let mut registry = world.registry_doc();
    let mut row = registry["projects"][0].clone();
    row["id"] = json!("far");
    row["display_name"] = json!("Far");
    row["root"] = json!(far.to_string_lossy());
    registry["projects"]
        .as_array_mut()
        .expect("projects")
        .push(row);
    write_json(&world.registry_path(), &registry);
    fs::write(far.join("status.txt"), "far needs a look\n").expect("its status");
    let mut config = configured(Value::Null);
    config["projects"]["far"] = json!({ "providers": [
        { "provider": "custom-status", "command": "cat status.txt" }
    ] });
    world.configure(config);
    world.ephor().args(["refresh"]).assert().success();

    let (ok, held) = process(&world, &["--all"]);
    assert!(ok, "{held:#}");
    assert_eq!(held["gated"], true, "{held:#}");
    assert_eq!(held["dry_run"], true, "{held:#}");
    assert!(
        held["says"]
            .as_str()
            .is_some_and(|says| says.contains("--act")),
        "{held:#}"
    );
    assert!(plans(&world).is_empty(), "a held sweep writes nothing");

    let (ok, narrowed) = process(&world, &["--all", "--project", PROJECT]);
    assert!(ok, "{narrowed:#}");
    assert!(narrowed.get("gated").is_none(), "{narrowed:#}");
    assert_eq!(plans(&world).len(), 3, "{narrowed:#}");
}
