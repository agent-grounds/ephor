//! The organization a matter belongs to, handed over with the matter
//! (§FS-005-dispatch.8, §FS-005-dispatch.6.1, §FS-006-project-interface.3).
//!
//! A site keeps one directory of instructions per organization and wants every
//! summons its projects make to be able to find it. The registry has always
//! known which organization a project's row places it in and where that
//! organization is rooted — a work root template renders `{org}` and
//! `{org_root}` from exactly those two facts — but neither reaches a *program*:
//! not the environment a summoned command is given, and not the structured
//! metadata a ticket carries for the scripts in a state machine. So the answer
//! is written again into every recipe of every project, and renaming or
//! re-rooting an organization leaves as many stale copies as there are
//! repositories.
//!
//! The scenario is the one the ticket reports from the outside: a project
//! whose `custom-status` command records what it was told, and a plan
//! `work dispatch` lays for one of its matters. Three projects, because
//! membership and the root are absent independently — one placed in a rooted
//! organization, one placed in an organization that declares no `root`, and
//! one the registry places in no organization at all.
//!
//! The two halves answer a gap differently, and that is the design rather than
//! a wrinkle. A summons defines both names always and leaves them empty,
//! because a summoned command does not start from a cleared environment and an
//! undefined name is inherited from whatever launched ephor rather than
//! absent — which is why this case exports both names before it runs ephor at
//! all, and asserts that no summons sees them. A ticket writes no key for a
//! value it has not got, the way it writes no `branch` for a matter with none,
//! and nothing reading a ticket has an environment to inherit from.

#[path = "../support.rs"]
mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use support::*;

/// The organization the watched project is placed in, declared with a `root`.
const ROOTED: &str = "foundation";

/// An organization declared without a `root`: it can name itself and has
/// nowhere to point, which is the second of the three absences.
const ROOTLESS: &str = "personal";

/// The project the rootless organization holds.
const HALF_PLACED: &str = "mill";

/// A project the registry places in no organization at all — the third
/// absence, and the one that would otherwise read the launching shell's answer.
const OUTSIDER: &str = "outland";

/// What the shell that starts ephor already holds for the two names. Nothing
/// ephor summons may see it: a name ephor does not set is not absent but
/// inherited, which is the whole reason the answer is empty rather than unset.
const INHERITED: &str = "whatever-the-launching-shell-was-in";

/// Where each summons writes down what it was told about its organization.
fn records(world: &World) -> PathBuf {
    world.path().join("records")
}

/// A status reporter that records its own summons before answering. It names
/// the file after the checkout it ran in, so each project's record is its own;
/// a provider command runs in the project's root, which is the only thing this
/// leans on.
fn reporter(records: &Path) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
name="$(basename "$PWD")"
env | grep '^EPHOR_' | sort > "{records}/$name.env"
cat > "$EPHOR_ANSWER" <<EOF
{{ "v": 1, "matters": [ {{
  "key": "gate:$name", "kind": "status", "title": "the gate on $name",
  "state": "red", "terminal": false, "time": "2026-09-01T00:00:00Z" }} ] }}
EOF
"#,
        records = records.display()
    )
}

/// The one recipe every project is offered, so a sweep has something to lay.
fn recipe() -> Value {
    json!({
        "id": "gate-watch",
        "description": "look at the gate",
        "brief": "The gate is red on {title}. Make it green.",
        "state": "fix",
        "needs_checkout": false,
        "when": { "kinds": ["status"] }
    })
}

/// A site watching three projects, one per absence: the watched project in a
/// rooted organization, a second in an organization that declares no root, and
/// a third the registry places in none.
fn a_site_of_three_projects() -> World {
    let world = World::new();
    std::fs::create_dir_all(records(&world)).expect("somewhere to record each summons");
    world.organize(ROOTED, "Foundation");
    world.stub("status-reporter", &reporter(&records(&world)));

    let mut registry = world.registry_doc();
    registry["organizations"][0]["root"] = json!(world.path().join("shared").to_string_lossy());
    registry["organizations"]
        .as_array_mut()
        .expect("organizations")
        .push(json!({ "id": ROOTLESS, "name": "Personal" }));
    for (id, display, organization) in [
        (HALF_PLACED, "Mill", Some(ROOTLESS)),
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
    write_json(&world.registry_path(), &registry);

    let watcher = json!({ "providers": [
        { "provider": "custom-status", "command": "status-reporter", "format": "answer" }
    ] });
    world.configure(json!({
        "work": { "recipes": [recipe()] },
        "projects": {
            PROJECT: watcher.clone(),
            HALF_PLACED: watcher.clone(),
            OUTSIDER: watcher
        }
    }));
    world
}

/// One project's refresh, run from a shell that already holds both
/// organization names. That is the environment the rule is about: ephor's
/// answer has to overwrite it, in every shape and whether or not there is an
/// organization to name.
fn refresh(world: &World, project: &str) {
    let ran = world
        .ephor_raw()
        .env("EPHOR_ORG", INHERITED)
        .env("EPHOR_ORG_ROOT", INHERITED)
        .args(["refresh", project])
        .output()
        .expect("ran");
    assert!(
        ran.status.success(),
        "`ephor refresh {project}` did not run: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
}

/// What one project's summons was told, name by name. A name the summons left
/// out is missing from this map; a name it answered empty is present and
/// empty, and those are the two different things the rule is about.
fn told(world: &World, project: &str) -> BTreeMap<String, String> {
    let path = records(world).join(format!("{project}.env"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{project}'s summons recorded nothing at {path:?}: {err}"));
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

/// Assert one name of one project's summons, quoting the whole record when it
/// is wrong — the record is short, and what is *missing* from it is the answer.
fn answered(world: &World, project: &str, name: &str, expected: &str) {
    let told = told(world, project);
    let value = told
        .get(name)
        .unwrap_or_else(|| panic!("{project}'s summons was never told {name}: {told:#?}"));
    assert_eq!(value, expected, "{project}'s summons was told {name}");
}

/// One project's sweep, which lays a ticket for the one matter it reports.
/// The scope is one project, so it writes without `--act`
/// (§FS-011-command-line.10).
fn dispatch(world: &World, project: &str) {
    let ran = world
        .ephor_raw()
        .args(["work", "dispatch", "--project", project])
        .output()
        .expect("ran");
    assert!(
        ran.status.success(),
        "`ephor work dispatch --project {project}` did not run: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
}

/// Every plan a project's work root holds, as text.
fn plans(root: &Path) -> String {
    let mut text = String::new();
    let entries = std::fs::read_dir(root)
        .unwrap_or_else(|err| panic!("no work root at {}: {err}", root.display()));
    for entry in entries.flatten() {
        if entry.path().extension().and_then(|e| e.to_str()) == Some("md") {
            text.push_str(&std::fs::read_to_string(entry.path()).expect("a plan reads"));
        }
    }
    assert!(!text.is_empty(), "no plan was laid in {}", root.display());
    text
}

/// The ticket's own reproducer, on the summons side: a placed project's
/// command is told which organization it is in and where that organization is
/// rooted, so it can reach what the site keeps there without being told
/// separately.
#[test]
fn a_placed_projects_summons_is_told_its_organization_and_where_it_is_rooted() {
    let world = a_site_of_three_projects();
    refresh(&world, PROJECT);

    answered(&world, PROJECT, "EPHOR_ORG", ROOTED);
    answered(
        &world,
        PROJECT,
        "EPHOR_ORG_ROOT",
        &world.path().join("shared").to_string_lossy(),
    );
    // The project's own place is unchanged: the organization is told beside
    // it, not instead of it.
    answered(&world, PROJECT, "EPHOR_PROJECT", PROJECT);
}

/// Membership and the root are absent independently, so an organization that
/// declares no `root` names itself and answers an empty root — rather than
/// disowning the project or refusing the summons the way a work root does.
#[test]
fn an_organization_that_declares_no_root_names_itself_and_answers_an_empty_root() {
    let world = a_site_of_three_projects();
    refresh(&world, HALF_PLACED);

    answered(&world, HALF_PLACED, "EPHOR_ORG", ROOTLESS);
    answered(&world, HALF_PLACED, "EPHOR_ORG_ROOT", "");
}

/// A project the registry places in no organization is told so — both names
/// present and empty — rather than being left to inherit the answer of the
/// shell that started ephor, which is what an unset name would mean.
#[test]
fn a_project_in_no_organization_is_told_so_rather_than_inheriting_an_answer() {
    let world = a_site_of_three_projects();
    refresh(&world, OUTSIDER);

    answered(&world, OUTSIDER, "EPHOR_ORG", "");
    answered(&world, OUTSIDER, "EPHOR_ORG_ROOT", "");
}

/// The other half of the one vocabulary: the ticket a sweep lays carries the
/// organization as data, beside the project, where a program in a state
/// machine reads it (§FS-005-dispatch.8).
///
/// And it carries it the way a ticket carries anything: a value it has not got
/// is not written. So the project in no organization gets a ticket with
/// neither key, exactly as a matter with no branch gets no `branch` key —
/// which is a different answer from the summons's empty string, and
/// deliberately so.
#[test]
fn the_ticket_carries_the_organization_beside_the_project() {
    let world = a_site_of_three_projects();
    for project in [PROJECT, HALF_PLACED, OUTSIDER] {
        refresh(&world, project);
        dispatch(&world, project);
    }

    let placed = plans(&world.forest().join("panta"));
    assert!(
        placed.contains(&format!(r#"org: "{ROOTED}""#)),
        "the ticket does not say which organization the work belongs to:\n{placed}"
    );
    assert!(
        placed.contains(&format!(
            r#"org_root: "{}""#,
            world.path().join("shared").to_string_lossy()
        )),
        "the ticket does not say where that organization is rooted:\n{placed}"
    );

    // An organization with no root names itself on the ticket too, and the
    // root it has not got is simply not written.
    let half = plans(&world.path().join(HALF_PLACED).join("panta"));
    assert!(
        half.contains(&format!(r#"org: "{ROOTLESS}""#)),
        "the ticket does not name the organization that has no root:\n{half}"
    );
    assert!(
        !half.contains("org_root:"),
        "a root nobody declared was written onto the ticket anyway:\n{half}"
    );

    // And the outsider's ticket carries neither key, while still being a
    // ticket about a project — so the absence is a missing key rather than a
    // missing ticket.
    let outsider = plans(&world.path().join(OUTSIDER).join("panta"));
    assert!(
        outsider.contains(&format!(r#"project: "{OUTSIDER}""#)),
        "no ticket was laid for the project in no organization:\n{outsider}"
    );
    assert!(
        !outsider.contains("org:") && !outsider.contains("org_root:"),
        "a project the registry places in no organization was given one:\n{outsider}"
    );
}
