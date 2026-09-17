//! E2E-027-a-sweep-nobody-has-to-type: a recipe asks for its own sweep, and
//! says how often it should happen (§FS-005-dispatch.31).
//!
//! The scenario is the loop that was automatic in its second half only. A timer
//! runs `refresh`, then `work sync`, then `work run --due`: sync reopens matters
//! already dispatched (§FS-005-dispatch.5) and the due sweep starts tickets that
//! already exist (§FS-005-dispatch.24), and neither of those introduces a
//! matter. So a newly assigned issue sat in the feed — matched by a selector its
//! author had checked with `--dry-run`, carrying `autorun`, wanting nothing from
//! anybody — until a person typed the one verb no unit runs.
//!
//! What this case pins is the field that closes it. `dispatch` on a recipe says
//! both that its sweep needs nobody and how often that sweep should happen, and
//! its presence is the opt-in: silence leaves the key exactly where it was. The
//! interval paces the looking rather than triggering it — ephor has no daemon,
//! so the caller's own rate is the ceiling on every value, which is the whole
//! meaning of `"0h"`.
//!
//! Beside it are the two things that keep an unattended sweep from being the
//! expensive kind of blind: the recipe's own bound on one sweep, and the record
//! of when each recipe last swept — ephor's own, kept outside the ledger, and
//! read forgivingly in the one direction that costs a sweep rather than a queue.

#[path = "../support.rs"]
mod support;

use serde_json::json;

use support::*;

/// Four issues, none of which anybody has dispatched. They are what a sweep
/// nobody typed has to find.
const FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities)
    printf '{"issues":true}'
    ;;
  issues)
    printf '%s' '[
      { "key": "acme/widget#10", "title": "Alpha", "url": "https://acme.example/issue/10",
        "updated_at": "2026-07-27T12:00:00Z", "status": "open" },
      { "key": "acme/widget#11", "title": "Beta", "url": "https://acme.example/issue/11",
        "updated_at": "2026-07-28T12:00:00Z", "status": "open" },
      { "key": "acme/widget#12", "title": "Gamma", "url": "https://acme.example/issue/12",
        "updated_at": "2026-07-29T12:00:00Z", "status": "open" },
      { "key": "acme/widget#13", "title": "Delta", "url": "https://acme.example/issue/13",
        "updated_at": "2026-07-30T12:00:00Z", "status": "open" }
    ]'
    ;;
  *)
    printf '[]'
    ;;
esac
"#;

/// A recipe every issue matches, needing no checkout — so what this case
/// measures is the sweep and never whether a branch happened to be on disk.
fn recipe(dispatch: Option<serde_json::Value>) -> serde_json::Value {
    let mut recipe = json!({
        "id": "implement",
        "description": "look at the issue",
        "when": { "kinds": ["issue"] },
        "needs_checkout": false,
        "state": "fix",
        "brief": "Look at {title}."
    });
    if let Some(every) = dispatch {
        recipe["dispatch"] = every;
    }
    recipe
}

fn world_with(dispatch: Option<serde_json::Value>) -> World {
    let world = World::new();
    world.stub("ephor-forge-acmeforge", FORGE);
    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "acmeforge", "user": "you", "repos": ["widget"] }
        ] } },
        "work": { "recipes": [recipe(dispatch)] },
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

/// `ephor work sync` as a timer runs it, read as the machine form so the
/// assertions are about outcomes rather than about wrapping.
fn sync(world: &World, extra: &[&str]) -> serde_json::Value {
    let mut args = vec!["work", "sync", "--json"];
    args.extend_from_slice(extra);
    let output = world.ephor().args(&args).output().expect("sync runs");
    assert!(
        output.status.success(),
        "sync failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    json_of(&output)
}

/// What one sync opened, by matter id and in the order it opened them.
fn opened(synced: &serde_json::Value) -> Vec<String> {
    synced["items"]
        .as_array()
        .expect("items is an array")
        .iter()
        .filter(|row| matches!(row["outcome"].as_str(), Some("opened" | "would-open")))
        .map(|row| row["item"].as_str().expect("an item id").to_string())
        .collect()
}

/// Where ephor keeps its own record of when each recipe last swept: beside the
/// feed cache and the burn store, and never in the ledger
/// (§FS-005-dispatch.31).
fn record(world: &World) -> std::path::PathBuf {
    world.path().join("state").join("ephor").join("sweeps.json")
}

/// The reproducer. A recipe that says nothing about sweeping is swept by
/// nobody: sync reopens what moved, and the four matters it has no work about
/// stay exactly where they were. This is the behaviour the field must not
/// disturb — silence means the key, one step earlier than `autorun`.
#[test]
fn a_recipe_that_says_nothing_is_still_the_readers_to_sweep() {
    let world = world_with(None);

    let synced = sync(&world, &[]);

    assert!(
        opened(&synced).is_empty(),
        "a recipe that never asked for its own sweep was swept anyway: {synced}"
    );
    assert_eq!(synced["opened"], json!(0));
    assert!(
        !record(&world).exists(),
        "a sweep nobody asked for wrote a record of having swept"
    );
    // And the reader's own verb still reaches every one of them.
    let dispatched = world
        .ephor()
        .args(["work", "dispatch", "--dry-run", "--json"])
        .output()
        .expect("dispatch runs");
    assert_eq!(json_of(&dispatched)["opened"], json!(4));
}

/// The fix, at the rhythm `"0h"` names: every matter the recipe covers is
/// opened by the sweep a timer already runs, with nobody present.
#[test]
fn a_recipe_that_asks_for_its_own_sweep_is_swept_by_the_timers_own_verb() {
    let world = world_with(Some(json!("0h")));

    let synced = sync(&world, &[]);

    assert_eq!(
        opened(&synced),
        vec![
            "acmeforge:acme/widget#13",
            "acmeforge:acme/widget#12",
            "acmeforge:acme/widget#11",
            "acmeforge:acme/widget#10",
        ],
        "the sweep did not open what the recipe covers: {synced}"
    );
    assert_eq!(synced["opened"], json!(4));

    // Having been opened, they are work — so the next sync reopens rather than
    // opens, and opens nothing a second time.
    let again = sync(&world, &[]);
    assert!(
        opened(&again).is_empty(),
        "the sweep opened matters it had already opened: {again}"
    );
}

/// The interval paces the looking. A recipe that sweeps every six hours is
/// swept once by a timer that fires far more often than that, and the matters
/// it did not reach wait for the interval rather than for a person.
/// A bound alongside the interval, so there is always a matter left over for
/// the next sweep to prove itself on. Without one, four matters are four
/// matters ephor has work about after the first sweep, and every sweep after
/// that is silent for a reason that has nothing to do with the interval.
#[test]
fn an_interval_that_has_not_elapsed_is_skipped_and_the_timer_keeps_firing() {
    let world = world_with(Some(json!({ "every": "6h", "limit": 2 })));

    let first = sync(&world, &[]);
    assert_eq!(first["opened"], json!(2), "the first sweep should look");

    // The timer fires again — twice, as a half-hourly unit would inside one
    // six-hour window — and the two matters still waiting stay waiting.
    let second = sync(&world, &[]);
    let third = sync(&world, &[]);
    assert!(
        opened(&second).is_empty() && opened(&third).is_empty(),
        "a recipe whose interval had not elapsed was swept anyway: {second} {third}"
    );

    // The record is what makes that true, and moving it back is the same thing
    // six hours passing is.
    let path = record(&world);
    let mut sweeps = read_json(&path);
    sweeps["swept"][format!("{PROJECT}/implement")] = json!("2020-01-01T00:00:00Z");
    write_json(&path, &sweeps);

    let later = sync(&world, &[]);
    assert_eq!(
        opened(&later),
        vec!["acmeforge:acme/widget#11", "acmeforge:acme/widget#10"],
        "an interval that had elapsed was not swept: {later}"
    );
    assert_ne!(
        read_json(&path)["swept"][format!("{PROJECT}/implement")],
        json!("2020-01-01T00:00:00Z"),
        "a sweep that looked did not mark its own clock"
    );
}

/// A recipe may bound its own sweep, because the reader is not there to type
/// `--limit` and the verb hosting the sweep is not the verb that flag is on
/// (§FS-005-dispatch.31.4).
#[test]
fn a_recipe_may_bound_what_one_sweep_of_its_own_opens() {
    let world = world_with(Some(json!({ "every": "0h", "limit": 2 })));

    let synced = sync(&world, &[]);

    assert_eq!(
        opened(&synced),
        vec!["acmeforge:acme/widget#13", "acmeforge:acme/widget#12"],
        "the recipe's own bound did not hold: {synced}"
    );

    // The bound is on one sweep and not on the queue: the next one takes the
    // next two, so a bounded recipe drains its backlog rather than stalling.
    let next = sync(&world, &[]);
    assert_eq!(
        opened(&next),
        vec!["acmeforge:acme/widget#11", "acmeforge:acme/widget#10"],
        "a bounded sweep did not carry on where it left off: {next}"
    );
}

/// A dry run resolves everything and writes nothing, and that includes the
/// sweep's own record of having swept — otherwise reporting what *would* be
/// opened would silently close the window on doing it.
#[test]
fn a_dry_run_opens_nothing_and_marks_no_clock() {
    let world = world_with(Some(json!("6h")));

    let reported = sync(&world, &["--dry-run"]);

    assert_eq!(
        opened(&reported).len(),
        4,
        "a dry run should still say what it would open: {reported}"
    );
    assert_eq!(reported["dry_run"], json!(true));
    assert!(
        !record(&world).exists(),
        "a dry run wrote the sweep record it was reporting on"
    );

    // And the real sweep afterwards is unaffected by having been reported on.
    assert_eq!(sync(&world, &[])["opened"], json!(4));
}

/// The reader's own sweep marks the clock too: it opens what a self-sweeping
/// recipe would have opened, so a record that ignored it would send the timer
/// to look again at a queue a person had just emptied by hand.
#[test]
fn a_sweep_the_reader_typed_marks_the_same_clock() {
    let world = world_with(Some(json!("6h")));

    world.ephor().args(["work", "dispatch"]).assert().success();

    let path = record(&world);
    assert!(
        path.exists(),
        "the reader's own sweep did not mark the recipe's clock"
    );
    assert!(
        read_json(&path)["swept"][format!("{PROJECT}/implement")].is_string(),
        "the clock was marked under some other key: {}",
        read_json(&path)
    );
}

/// A record ephor cannot read is one that was never written, and both mean
/// *due now*: the cost is one early sweep, where erring the other way is a
/// queue that silently stops being looked at (§REQ-001-boundary.1).
#[test]
fn an_unreadable_record_means_due_now_rather_than_a_refusal() {
    let world = world_with(Some(json!({ "every": "7d", "limit": 1 })));

    assert_eq!(sync(&world, &[])["opened"], json!(1));
    // Seven days have not passed, so the three still waiting keep waiting.
    assert_eq!(sync(&world, &[])["opened"], json!(0));

    // Until the record saying so stops being readable, which is the same thing
    // as one that was never written.
    let path = record(&world);
    std::fs::write(&path, "{ this is not json").expect("a corrupt record");

    let synced = sync(&world, &[]);
    assert_eq!(
        synced["opened"],
        json!(1),
        "an unreadable record refused the sweep instead of meaning due now: {synced}"
    );
    assert!(
        read_json(&path)["swept"].is_object(),
        "the sweep did not rewrite the record it could not read: {}",
        std::fs::read_to_string(&path).unwrap_or_default()
    );
}

/// An interval nobody can read is refused where the recipe is read. This is the
/// field that spends agents unattended, so a rhythm ephor cannot parse must not
/// quietly become one its author never chose.
#[test]
fn an_interval_that_is_not_one_is_refused_by_name() {
    // Built with a rhythm that reads, so the refusal below is the configuration
    // being rewritten under a working world rather than a world that never
    // started — which is how a reader meets this: editing a recipe they had.
    let world = world_with(Some(json!("6h")));
    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "acmeforge", "user": "you", "repos": ["widget"] }
        ] } },
        "work": { "recipes": [recipe(Some(json!("sometimes")))] },
    }));

    let output = world
        .ephor()
        .args(["work", "sync", "--json"])
        .output()
        .expect("sync runs");

    assert!(
        !output.status.success(),
        "a rhythm nobody can read was taken"
    );
    let said = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        said.contains("sometimes") && said.contains("is not an interval"),
        "the refusal did not name the value or say what it wanted: {said}"
    );
    // And it says what a rhythm looks like, including the one spelling a reader
    // would otherwise have to be told about.
    assert!(
        said.contains("'0h' is every sweep"),
        "the refusal did not say how to write one: {said}"
    );
}

/// The configured ranking orders the unattended sweep too (§FS-005-dispatch.26).
/// It only shows where a bound stops the sweep short — which is exactly the
/// case a reader who wrote both a ranking and a limit is asking about.
#[test]
fn a_bounded_sweep_takes_the_ranking_the_site_configured() {
    let world = World::new();
    world.stub("ephor-forge-acmeforge", FORGE);
    let ranking = world.path().join("ranking.txt");
    std::fs::write(
        &ranking,
        "acmeforge:acme/widget#10\nacmeforge:acme/widget#11\n",
    )
    .expect("a ranking");
    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "acmeforge", "user": "you", "repos": ["widget"] }
        ] } },
        "work": {
            "ranking": ranking.to_string_lossy(),
            "recipes": [recipe(Some(json!({ "every": "0h", "limit": 2 })))],
        },
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();

    let synced = sync(&world, &[]);

    // Without the ranking this would be #13 and #12, newest first.
    assert_eq!(
        opened(&synced),
        vec!["acmeforge:acme/widget#10", "acmeforge:acme/widget#11"],
        "the unattended sweep ignored the configured ranking: {synced}"
    );
}
