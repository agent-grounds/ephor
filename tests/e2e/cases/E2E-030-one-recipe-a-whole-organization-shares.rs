//! One recipe written where an organization's projects already share a work
//! root and a budget, and read by every project the registry puts inside it
//! (§FS-005-dispatch.1, §FS-005-dispatch.24).
//!
//! The scenario is the one the ticket reports from the outside: a site
//! configuration whose `organizations.<id>.work` block carries `recipes`
//! beside the `root` those same projects already share. Today that file does
//! not load at all — not the recipe, not the root above it, not the rest of
//! the site's configuration — so what four projects share has to be written
//! four times, and four copies of a selector can disagree with each other
//! silently in a sweep nobody is watching.
//!
//! Three scopes now write recipes, so the order they accumulate in is
//! observable and is pinned here rather than left to be discovered: shipped,
//! then the site's, then the organization's, then the project's, with a
//! recipe reusing an earlier one's id replacing it *where it already stands*.
//! Position is the order dispatch offers in, so a displacement that quietly
//! moved a recipe to the end of the menu would change which brief an
//! unattended sweep hands over.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

/// The organization both watched projects are declared to be part of.
const ORGANIZATION: &str = "foundation";

/// The second project of that organization: it writes no recipes of its own,
/// so what it is offered is whatever the organization wrote.
const SIBLING: &str = "mill";

/// A third project the registry places in no organization at all — the edge
/// that must go on reading the site's recipes and nothing else, silently.
const OUTSIDER: &str = "outland";

/// A status reporter that names its matter after the checkout it ran in, so
/// each project reports one matter of its own and a case can tell them apart.
/// A provider command runs in the project's root, which is the only thing
/// this leans on.
const REPORTER: &str = r#"#!/usr/bin/env bash
set -euo pipefail
name="$(basename "$PWD")"
cat > "$EPHOR_ANSWER" <<EOF
{ "v": 1, "matters": [ {
  "key": "gate:$name", "kind": "status", "title": "the gate on $name",
  "state": "red", "terminal": false, "time": "2026-09-01T00:00:00Z" } ] }
EOF
"#;

/// The recipe the ticket writes, in this world's vocabulary: the same shape —
/// an id, a description, a brief, a state and a `when` — selecting the matter
/// these projects actually report.
fn recipe(id: &str, brief: &str) -> Value {
    json!({
        "id": id,
        "description": "look at the gate",
        "brief": brief,
        "state": "fix",
        "needs_checkout": false,
        "when": { "kinds": ["status"] }
    })
}

/// A site watching three projects: two the registry places in one
/// organization, one it places in none. Nothing is configured about work yet —
/// each case writes the block it is about.
fn three_projects() -> World {
    let world = World::new();
    world.organize(ORGANIZATION, "Foundation");
    world.stub("status-reporter", REPORTER);

    let mut registry = world.registry_doc();
    for (id, display, organization) in [
        (SIBLING, "Mill", Some(ORGANIZATION)),
        (OUTSIDER, "Outland", None),
    ] {
        let root = world.path().join(id);
        std::fs::create_dir_all(&root).expect("the other forest");
        let mut row = registry["projects"][0].clone();
        row["id"] = json!(id);
        row["display_name"] = json!(display);
        row["root"] = json!(root.to_string_lossy());
        match organization {
            Some(organization) => row["organization"] = json!(organization),
            None => {
                row.as_object_mut().expect("a row").remove("organization");
            }
        }
        registry["projects"]
            .as_array_mut()
            .expect("projects")
            .push(row);
    }
    // The organization is rooted, so `{org_root}` has an answer and the
    // ticket's own block can be written whole (§FS-005-dispatch.6.1).
    registry["organizations"][0]["root"] = json!(world.path().join("shared").to_string_lossy());
    write_json(&world.registry_path(), &registry);
    world
}

/// The site configuration, with whatever work block a case is about, and the
/// one provider every project reports through.
fn watching(world: &World, work: Value) {
    let watcher = json!({ "providers": [
        { "provider": "custom-status", "command": "status-reporter", "format": "answer" }
    ] });
    let mut config = json!({
        "projects": {
            PROJECT: watcher.clone(),
            SIBLING: watcher.clone(),
            OUTSIDER: watcher
        }
    });
    merge_into(&mut config, work);
    world.configure(config);
    for project in [PROJECT, SIBLING, OUTSIDER] {
        world.ephor().args(["refresh", project]).assert().success();
    }
}

/// Merge a work block over the configuration a case starts from, object by
/// object — a case writes `projects.<id>.work` beside the providers this file
/// already put under the same key, so a shallow insert would drop one of them.
fn merge_into(into: &mut Value, from: Value) {
    match (into, from) {
        (Value::Object(into), Value::Object(from)) => {
            for (key, value) in from {
                merge_into(into.entry(key).or_insert(Value::Null), value);
            }
        }
        (into, from) => *into = from,
    }
}

/// One project's sweep, read as the machine form. The assertion that it
/// *loaded at all* is the ticket's own reproducer, so the failure quotes what
/// the run said rather than leaving a JSON parse to report it.
fn dispatch(world: &World, project: &str, extra: &[&str]) -> Value {
    let mut args = vec!["work", "dispatch", "--project", project, "--json"];
    args.extend_from_slice(extra);
    let output = world.ephor_raw().args(&args).output().expect("ran");
    assert!(
        output.status.success(),
        "`ephor work dispatch --project {project}` did not run: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    json_of(&output)
}

/// Which recipe a sweep chose for the one matter a project reports.
fn chose(swept: &Value) -> String {
    let items = swept["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "one matter per project: {swept:#}");
    items[0]["recipe"]
        .as_str()
        .expect("a recipe id")
        .to_string()
}

/// The matter id a sweep reported, which is what `actions` is asked about.
fn matter_of(swept: &Value) -> String {
    swept["items"][0]["item"]
        .as_str()
        .expect("a matter id")
        .to_string()
}

/// Every plan a project's work root holds, as text — where the brief the
/// reader wrote actually lands (§FS-005-dispatch.2).
fn plans(root: &std::path::Path) -> String {
    let mut text = String::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return text;
    };
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|e| e.to_str()) == Some("md") {
            text.push_str(&std::fs::read_to_string(entry.path()).expect("a plan reads"));
        }
    }
    text
}

/// The reproducer, and the ticket's own file. One `recipes` key beside the
/// `root` an organization's projects already share: the configuration loads,
/// and every project the registry places inside that organization is offered
/// the recipe written once.
#[test]
fn the_recipe_an_organization_writes_once_is_offered_to_every_project_it_holds() {
    let world = three_projects();
    watching(
        &world,
        json!({ "organizations": { ORGANIZATION: { "work": {
            "root": "{org_root}/panta",
            "recipes": [recipe("gate-watch", "The gate is red on {title}. Make it green.")]
        } } } }),
    );

    for project in [PROJECT, SIBLING] {
        let swept = dispatch(&world, project, &["--dry-run"]);
        assert_eq!(swept["opened"], json!(1), "{swept:#}");
        assert_eq!(chose(&swept), "gate-watch", "{swept:#}");
    }
}

/// The companion the ticket names: the same recipe object, byte for byte,
/// under the project's own key. That loads today and must go on loading, and
/// it still reaches exactly the project it was written under and no sibling.
/// Which scope wins where several write one id is pinned by the two cases
/// below, not here.
#[test]
fn the_same_recipe_under_the_project_loads_exactly_as_it_did() {
    let world = three_projects();
    watching(
        &world,
        json!({ "projects": { PROJECT: { "work": {
            "recipes": [recipe("gate-watch", "The gate is red on {title}. Make it green.")]
        } } } }),
    );

    let swept = dispatch(&world, PROJECT, &["--dry-run"]);
    assert_eq!(chose(&swept), "gate-watch", "{swept:#}");
    // And it reaches exactly the project it was written under: the sibling in
    // the same organization is offered nothing, because nothing was written
    // where the sibling would read it.
    let sibling = dispatch(&world, SIBLING, &["--dry-run"]);
    assert_eq!(sibling["opened"], json!(0), "{sibling:#}");
}

/// Three scopes write one id, and which brief is handed over is the innermost
/// scope that wrote one: the project's where it wrote one, the organization's
/// where it did not, and the site's for a project the registry places in no
/// organization at all (§FS-005-dispatch.1).
#[test]
fn the_innermost_scope_that_wrote_the_id_is_the_brief_that_is_handed_over() {
    let world = three_projects();
    watching(
        &world,
        json!({
            "work": { "recipes": [recipe("gate-watch", "The site's brief for {title}.")] },
            "organizations": { ORGANIZATION: { "work": {
                "recipes": [recipe("gate-watch", "The organization's brief for {title}.")]
            } } },
            "projects": { PROJECT: { "work": {
                "recipes": [recipe("gate-watch", "The project's brief for {title}.")]
            } } }
        }),
    );

    for (project, root, expected) in [
        (PROJECT, world.forest(), "The project's brief"),
        (
            SIBLING,
            world.path().join(SIBLING),
            "The organization's brief",
        ),
        (OUTSIDER, world.path().join(OUTSIDER), "The site's brief"),
    ] {
        let swept = dispatch(&world, project, &[]);
        assert_eq!(swept["opened"], json!(1), "{swept:#}");
        let written = plans(&root.join("panta"));
        assert!(
            written.contains(expected),
            "{project} was handed the wrong brief:\n{written}"
        );
    }
}

/// Displacement keeps the displaced recipe's place. The organization writes a
/// new `sweep-pins`; the project writes `local-triage` first and its own
/// `sweep-pins` second. Replacing in place leaves `sweep-pins` ahead of
/// `local-triage`; moving a displaced recipe to the end would put the project's
/// `sweep-pins` behind it, and an unattended sweep would lay a different
/// ticket (§FS-005-dispatch.1, §FS-005-dispatch.32.3).
#[test]
fn a_recipe_a_narrower_scope_displaces_keeps_the_place_it_already_had() {
    let world = three_projects();
    watching(
        &world,
        json!({
            "organizations": { ORGANIZATION: { "work": {
                "recipes": [recipe("sweep-pins", "The organization's pin sweep for {title}.")]
            } } },
            "projects": { PROJECT: { "work": { "recipes": [
                recipe("local-triage", "Triage {title} here."),
                recipe("sweep-pins", "The project's pin sweep for {title}.")
            ] } } }
        }),
    );

    let matter = matter_of(&dispatch(&world, PROJECT, &["--dry-run"]));
    let listed = world
        .ephor_raw()
        .args(["actions", "--item", &matter, "--json"])
        .output()
        .expect("ran");
    assert!(
        listed.status.success(),
        "`ephor actions --item {matter}` did not run: {}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let menu = json_of(&listed);
    let order: Vec<String> = menu["offers"]
        .as_array()
        .expect("offers")
        .iter()
        .filter_map(|offer| offer["id"].as_str())
        .filter(|id| *id == "sweep-pins" || *id == "local-triage")
        .map(str::to_string)
        .collect();
    assert_eq!(
        order,
        vec!["sweep-pins".to_string(), "local-triage".to_string()],
        "a displaced recipe moved to the end of the menu: {menu:#}"
    );
    // And the brief in force is the project's, at the organization's position.
    assert_eq!(
        chose(&dispatch(&world, PROJECT, &["--dry-run"])),
        "sweep-pins"
    );
}

/// A recipe written over an organization is held to ephor's own namespace the
/// way the other two scopes are, and is refused by the key it was written
/// under — not by the whole file being unreadable (§FS-005-dispatch.1).
#[test]
fn an_organizations_recipe_that_squats_ephors_namespace_is_refused_by_name() {
    let world = three_projects();
    watching(
        &world,
        json!({ "work": { "recipes": [recipe("gate-watch", "b")] } }),
    );
    // Written after the refresh, so the refusal is the only thing under test.
    let mut config: Value = read_json(&world.config_path());
    config["organizations"] = json!({ ORGANIZATION: { "work": {
        "recipes": [recipe("@freehand", "b")]
    } } });
    write_json(&world.config_path(), &config);

    let refused = world
        .ephor_raw()
        .args(["work", "dispatch", "--project", PROJECT, "--dry-run"])
        .output()
        .expect("ran");
    assert_eq!(refused.status.code(), Some(2));
    let said = String::from_utf8_lossy(&refused.stderr);
    assert!(
        said.contains(&format!(
            "organizations.{ORGANIZATION}.work.recipes has a recipe that is refused"
        )),
        "{said}"
    );
}
