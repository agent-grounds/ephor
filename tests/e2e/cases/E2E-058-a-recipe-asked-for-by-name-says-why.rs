//! E2E-058-a-recipe-asked-for-by-name-says-why: a person names the recipe one
//! matter should go to, the matter does not take it, and the refusal says why
//! in both forms.
//!
//! A project keeps one task of its own and six recipes. `task-work` applies
//! to it. `pr-fix` asks for pull requests, `forge-only` for another source,
//! `authored` for a role a task never carries, `sliced` puts its work under a
//! `{meta.context}` the task's plan never wrote, and `rebase-sweep` is the
//! rebase sweep's own, offered on no matter at all. `ephor work offers` lists
//! `task-work` alone, which is correct. Then a person names one of the others:
//! `ephor work dispatch --item <task> --recipe pr-fix`. It exited 1 with an
//! empty `items` and `refused: 0` under `--json`, and with `0 ticket(s) opened`
//! and nothing else in prose: a failure nobody explained, with the cause to be
//! read out of the recipe's selector by hand.
//!
//! What this case holds ephor to. A recipe named for one matter and not offered
//! to it is refused out loud: a `refused` row naming the recipe, counted in
//! `refused`, whose `says` names the recipe, the matter and what kept them apart
//! — `kinds` included, because here the reader asked — with the same sentence
//! in prose and the exit code unchanged (§FS-005-dispatch.27.1). A name no
//! recipe carries is refused by a sentence of its own, naming the recipes that
//! are configured instead. A refusal with an empty answer is what
//! §FS-011-command-line.7 forbids, and both forms carry the one fact
//! (§REQ-002-parity.3). The recipe that does apply is still handed over.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

/// The project's own plan, as the store writes one: one open task.
const PLAN: &str = "# Rhei: the retry window\n\n\
## Tasks\n\n\
### Task 1: Widen the retry window\n**State:** pending\n\n\
The window resets per attempt, which is not what the docs say.\n";

/// The one matter every dispatch here names.
const ITEM: &str = "rhei:window-retry.1";

/// A recipe that takes a branch-less task: no checkout, one selector.
fn recipe(id: &str, when: Value) -> Value {
    json!({
        "id": id,
        "description": format!("the {id} recipe"),
        "state": "fix",
        "needs_checkout": false,
        "when": when,
        "brief": "{title}"
    })
}

/// A world watching the project's own store, with the six recipes of the
/// scenario configured site-wide and the task in the cached feed.
fn watching() -> World {
    let world = World::new();
    world.file("panta/window-retry.rhei.md", PLAN);
    let mut sliced = recipe("sliced", json!({ "sources": ["rhei"] }));
    sliced["root"] = json!("{root}/slices/{meta.context}");
    world.configure(json!({
        "work": { "recipes": [
            recipe("task-work", json!({ "sources": ["rhei"] })),
            recipe("pr-fix", json!({ "kinds": ["pr"] })),
            recipe("forge-only", json!({ "sources": ["acme"] })),
            recipe("authored", json!({ "roles": ["author"] })),
            sliced,
            recipe("rebase-sweep", json!({})),
        ] }
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    assert!(
        world.has_matter(ITEM),
        "the task is not in the feed, so the fixture is wrong: {:#}",
        world.feed()
    );
    world
}

/// `ephor work dispatch --item <task> --recipe <recipe>`, and whatever else.
fn dispatch(world: &World, recipe: &str, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["work", "dispatch", "--item", ITEM, "--recipe", recipe];
    args.extend_from_slice(extra);
    world.ephor().args(&args).output().expect("dispatch runs")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The reading's answer about the one matter asked for, held to the shape the
/// command publishes: exit 1, one `refused` row naming the recipe that was
/// asked for, counted in `refused`, and a `says` naming the recipe and the
/// matter (§FS-005-dispatch.27.1). Returns that `says`.
fn refused(output: &std::process::Output, recipe: &str) -> String {
    assert_eq!(
        output.status.code(),
        Some(1),
        "a matter asked about and not handed over exits 1, as it always has:\n{}",
        stderr(output)
    );
    let sweep = shaped("work-dispatch", output);
    let rows = sweep["items"].as_array().expect("a row per matter reached");
    assert_eq!(
        rows.len(),
        1,
        "the one matter asked about carries one row, whatever it came to — an empty \
         `items` beside a non-zero exit is a failure nobody explained: {sweep:#}"
    );
    let row = &rows[0];
    assert_eq!(row["item"], ITEM, "{sweep:#}");
    assert_eq!(
        row["recipe"], recipe,
        "the recipe that was asked for: {sweep:#}"
    );
    assert_eq!(row["outcome"], "refused", "{sweep:#}");
    assert!(
        sweep["refused"].as_u64().is_some_and(|count| count >= 1),
        "`refused` counts the refusal: {sweep:#}"
    );
    assert_eq!(sweep["opened"], 0, "{sweep:#}");
    let says = row["says"]
        .as_str()
        .filter(|says| !says.trim().is_empty())
        .unwrap_or_else(|| panic!("a refused row says why: {sweep:#}"))
        .to_string();
    assert!(says.contains(recipe), "it names the recipe: {says}");
    assert!(says.contains(ITEM), "it names the matter: {says}");
    says
}

/// The prose form of the same dispatch says the same sentence, and the
/// closing tally counts the matter among those that could not be
/// (§REQ-002-parity.3).
fn said_in_prose(output: &std::process::Output, says: &str) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "the prose form exits as the reading does:\n{}",
        stderr(output)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let prose = format!("{stdout}{}", stderr(output));
    assert!(
        prose.contains(says),
        "the prose form says what the reading says — `{says}` — and said:\n{prose}"
    );
    assert!(
        stdout.contains("1 item(s) could not be"),
        "the tally counts the refusal as the reading's `refused` does:\n{stdout}"
    );
}

/// The control. The task offers `task-work` and not `pr-fix`, and naming the
/// recipe that applies hands the task over, so the site and the dispatch path
/// are sound and every refusal below is about the recipe that was named.
#[test]
fn the_recipe_that_applies_is_handed_over() {
    let world = watching();
    let offers = shaped(
        "work",
        &world
            .ephor()
            .args(["work", "offers", "--item", ITEM, "--json"])
            .output()
            .expect("offers runs"),
    );
    let offered: Vec<&str> = offers["offers"]
        .as_array()
        .expect("an offers list")
        .iter()
        .filter_map(|offer| offer["id"].as_str())
        .collect();
    assert_eq!(offered, ["task-work"], "{offers:#}");

    let output = dispatch(&world, "task-work", &["--again", "--json", "--dry-run"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let sweep = shaped("work-dispatch", &output);
    assert_eq!(sweep["items"][0]["item"], ITEM, "{sweep:#}");
    assert_eq!(sweep["items"][0]["recipe"], "task-work", "{sweep:#}");
    assert_eq!(sweep["items"][0]["outcome"], "would-open", "{sweep:#}");
}

/// The report's case: a recipe whose `kinds` refused the matter. The offers
/// reading names nothing for it, because nobody asked about that recipe there;
/// here somebody did, so the row names the kind the matter has
/// (§FS-005-dispatch.27.1). The same in all three forms the report ran.
#[test]
fn a_recipe_for_another_kind_of_matter_is_refused_with_the_kind_named() {
    let world = watching();

    let says = refused(
        &dispatch(&world, "pr-fix", &["--again", "--json"]),
        "pr-fix",
    );
    assert!(
        says.contains("kind") && says.contains("task"),
        "it names `kinds` as what refused, and the matter's kind: {says}"
    );

    let dry = dispatch(&world, "pr-fix", &["--again", "--json", "--dry-run"]);
    assert_eq!(
        refused(&dry, "pr-fix"),
        says,
        "a dry run refuses where the dispatch would, in the same words"
    );

    said_in_prose(&dispatch(&world, "pr-fix", &["--again"]), &says);
}

/// A recipe whose other selector fields refused the matter: the source it
/// asks for, and a role a project's own task never carries. Each row names
/// the field and what the selector asked for (§FS-005-dispatch.27.1).
#[test]
fn a_recipe_refused_by_its_source_or_its_role_names_that_field() {
    let world = watching();

    let source = refused(&dispatch(&world, "forge-only", &["--json"]), "forge-only");
    assert!(
        source.contains("source") && source.contains("acme"),
        "it names `sources` and the source asked for: {source}"
    );
    said_in_prose(&dispatch(&world, "forge-only", &[]), &source);

    let role = refused(&dispatch(&world, "authored", &["--json"]), "authored");
    assert!(
        role.contains("role") && role.contains("author"),
        "it names `roles` and the role asked for: {role}"
    );
    said_in_prose(&dispatch(&world, "authored", &[]), &role);
}

/// A recipe whose selector holds but whose `root` template needs a field the
/// matter has not got: the row names the field, as the offers reading does
/// (§FS-005-dispatch.27.1, §FS-005-dispatch.25).
#[test]
fn a_recipe_whose_root_needs_a_field_the_matter_lacks_names_the_field() {
    let world = watching();

    let says = refused(&dispatch(&world, "sliced", &["--json"]), "sliced");
    assert!(
        says.contains("meta.context"),
        "it names the field the template needed: {says}"
    );
    said_in_prose(&dispatch(&world, "sliced", &[]), &says);
}

/// The rebase sweep's own recipe has a checkout for its subject and is never
/// offered on a matter, whatever its selector says. Named for one, it is
/// refused with the sentence its reservation already gives
/// (§FS-005-dispatch.27.1, §FS-004-quick-actions.6.1).
#[test]
fn the_rebase_sweeps_own_recipe_is_refused_with_its_reservation() {
    let world = watching();

    let says = refused(
        &dispatch(&world, "rebase-sweep", &["--json"]),
        "rebase-sweep",
    );
    assert!(
        says.contains("checkout"),
        "it says the recipe's subject is a checkout rather than a matter: {says}"
    );
    said_in_prose(&dispatch(&world, "rebase-sweep", &[]), &says);
}

/// A name no recipe carries was weighed against nothing, so it is not told it
/// "does not apply": its sentence says the name is not configured for the
/// project and names the recipes that are, with the same refused row and the
/// same exit (§FS-005-dispatch.27.1).
#[test]
fn a_name_no_recipe_carries_is_refused_with_a_sentence_of_its_own() {
    let world = watching();

    let says = refused(
        &dispatch(&world, "no-such-recipe", &["--json"]),
        "no-such-recipe",
    );
    assert!(
        says.contains(PROJECT),
        "it names the project the name was looked up in: {says}"
    );
    assert!(
        says.contains("task-work") && says.contains("pr-fix"),
        "it names the recipes the project does have: {says}"
    );
    assert!(
        !says.contains("does not apply"),
        "a recipe nobody configured was weighed against nothing: {says}"
    );
    said_in_prose(&dispatch(&world, "no-such-recipe", &[]), &says);
}
