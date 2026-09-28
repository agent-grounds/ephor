//! E2E-033-what-a-store-says-about-one-task: the word a plan keeps about one of
//! its own tasks becomes a fact of the matter, and every surface can key on it.
//!
//! The scenario is a project whose own work divides into slices — customers,
//! environments, subsystems — and which says so once, in its own files, where it
//! was already saying it for its own sake (§FS-006-project-interface.7). ephor
//! reads the store's plan already; what it read past was everything the plan
//! said about the task it was reading. So the word reached nothing: a recipe
//! keyed on it was refused where the configuration loads, a brief naming it
//! landed in the laid plan as the characters it was written with, a work root
//! became a directory of that name, and a summoned program had no variable to
//! read. Such a project cannot be swept unattended at all — the sweep takes
//! every slice or none — which is the case ephor exists for.
//!
//! What the case pins is the fact arriving under ephor's own name and reaching
//! the four places a matter's other facts already reach: `raw.meta` and so
//! `--json`, the selector (§FS-005-dispatch.31.1), the template vocabulary that
//! renders a brief and a `root`, and the summons environment. The field is a
//! reserved `raw` key rather than a field on the model — the third after
//! `assignees` and `labels`, through the same door — so nothing about the cache
//! or any other provider moves.
//!
//! And it pins the bound, which is what keeps *identifiers only* true of a map
//! whose keys nobody but the store chose: a plan that writes a paragraph or a
//! list loses that key and keeps the rest, the store is not marked failed for
//! having answered, and the drop is said out loud. A task that vanished from
//! the feed because somebody wrote prose about it would be the worse failure,
//! and a drop nobody was told about is how a selector silently stops matching.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

/// The plan as the store itself writes one. Two open tasks, and a block about
/// each of them in the store's own frontmatter, keyed by the task's own id —
/// there for the project's sake, and true whether or not ephor ever runs.
///
/// Task 2's block is the bound's case: one scalar it keeps, a list and a
/// paragraph it cannot.
const PLAN: &str = "# Rhei: slices\n\n\
---\n\
metadata:\n\
\x20 tasks:\n\
\x20   1:\n\
\x20     context: acme-labs\n\
\x20     tier: 1\n\
\x20   2:\n\
\x20     context: field-notes\n\
\x20     owners:\n\
\x20       - ana\n\
\x20       - bo\n\
\x20     handover: |\n\
\x20       The certificate was last rotated in March.\n\
\n\
\x20       Ask whoever keeps the store before touching it.\n\
---\n\n\
## Tasks\n\n\
### Task 1: Renew the staging certificate\n**State:** open\n\n\
The window resets per attempt.\n\n\
### Task 2: Rotate the signing key\n**State:** open\n\n\
Nobody has claimed this one.\n";

/// The matter the acme-labs slice is about, and the matter it is not.
const MINE: &str = "rhei:slices.1";
const THEIRS: &str = "rhei:slices.2";

/// The recipe from the report, in this world's words: a sweep that takes one
/// slice, puts its work somewhere of its own, and hands the word to the brief.
/// `tier` is asked as `"1"` against a `tier` the plan wrote as a bare `1`,
/// because a store writing its own YAML should not have to quote a digit to
/// stay selectable (§FS-005-dispatch.31.1).
fn slice_recipe() -> Value {
    json!({
        "id": "slice",
        "description": "work the acme-labs slice",
        "when": { "kinds": ["task"], "meta": { "context": "acme-labs", "tier": "1" } },
        "needs_checkout": false,
        "state": "fix",
        "root": "{root}/slices/{meta.context}",
        "brief": "{title} — in {meta.context}."
    })
}

/// A world watching the project's own store and nothing else. `work` is what
/// the case varies: the configuration that carries the recipe is the same file
/// every other command reads, so a case about the reader keeps it out.
fn watching(work: Option<Value>) -> World {
    let world = World::new();
    world.file("panta/slices.rhei.md", PLAN);
    let mut config = json!({});
    if let Some(work) = work {
        config["work"] = work;
    }
    world.configure(config);
    world
}

fn refresh(world: &World) -> std::process::Output {
    let output = world
        .ephor()
        .args(["refresh", PROJECT])
        .output()
        .expect("refresh runs");
    assert!(
        output.status.success(),
        "a store that answered failed the refresh:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// The one plan a dispatch laid in a work root, as text.
fn laid_in(root: &std::path::Path) -> String {
    let laid = std::fs::read_dir(root)
        .unwrap_or_else(|err| panic!("no work root at {}: {err}", root.display()))
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "md"))
        .unwrap_or_else(|| panic!("no plan was laid in {}", root.display()));
    std::fs::read_to_string(&laid).expect("the laid plan is readable")
}

/// 1. The fact arrives on the matter, under ephor's own name, and so reaches
/// `--json` for free: `meta` is a reserved `raw` key and `raw` is what
/// `--json` prints (§REQ-002-parity.3).
#[test]
fn what_the_store_said_about_one_task_arrives_on_the_matter() {
    let world = watching(None);
    refresh(&world);

    let mine = world.matter(MINE);
    // The path the plan was read out of was already there; what the plan said
    // about *this task* was not.
    assert_eq!(
        mine["raw"]["meta"],
        json!({ "context": "acme-labs", "tier": 1 }),
        "the store's own words about this task: {:#?}",
        mine["raw"]
    );
    // A sibling task in the same file is a different matter with its own
    // answer — the block is keyed by the task, not by the plan.
    assert_eq!(
        world.matter(THEIRS)["raw"]["meta"]["context"],
        "field-notes"
    );

    // And the same fact through the surface a program reads.
    let printed = world
        .ephor()
        .args(["feed", "--project", PROJECT, "--json"])
        .output()
        .expect("the feed prints");
    let feed = shaped("feed", &printed);
    let mine = feed
        .as_array()
        .expect("the feed prints an array")
        .iter()
        .find(|item| item["id"] == MINE || item["key"] == MINE)
        .unwrap_or_else(|| panic!("no {MINE} in {feed:#?}"));
    assert_eq!(mine["raw"]["meta"]["context"], "acme-labs");
}

/// 2. A recipe keyed on it loads — today the configuration itself is refused,
/// so no command reading it works at all — and it selects the one slice
/// (§FS-005-dispatch.31.1). Every key must hold: the sibling task carries a
/// `context` of its own and is not this recipe's.
#[test]
fn a_recipe_keyed_on_it_loads_and_selects_the_one_slice() {
    let world = watching(Some(json!({ "recipes": [slice_recipe()] })));
    refresh(&world);

    let output = world
        .ephor()
        .args([
            "work",
            "dispatch",
            "--project",
            PROJECT,
            "--dry-run",
            "--json",
        ])
        .output()
        .expect("dispatch runs");
    assert!(
        output.status.success(),
        "a recipe asking `meta` was refused where the configuration loads:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let swept = shaped("work-dispatch", &output);
    let would: Vec<&str> = swept["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter(|row| row["outcome"] == "would-open")
        .map(|row| row["item"].as_str().expect("an item id"))
        .collect();
    assert_eq!(
        would,
        vec![MINE],
        "the sweep took the wrong slices: {swept:#?}"
    );
}

/// 3. The task the selector passed over is told which field refused it and what
/// it carried instead (§FS-005-dispatch.27). A store nobody has annotated yet
/// and a recipe asking the wrong key read identically from an empty list, and
/// `meta` is about to be the commonest refusal a task gets.
#[test]
fn the_task_the_slice_passed_over_is_told_that_meta_refused_it() {
    let world = watching(Some(json!({ "recipes": [slice_recipe()] })));
    refresh(&world);

    let output = world
        .ephor()
        .args(["work", "offers", "--item", THEIRS, "--json"])
        .output()
        .expect("offers runs");
    assert!(
        output.status.success(),
        "offers could not read the configuration:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let view = shaped("work", &output);
    let refused = view["excluded"]
        .as_array()
        .expect("excluded is an array")
        .iter()
        .find(|row| row["recipe"] == "slice")
        .unwrap_or_else(|| panic!("the recipe refused this matter silently: {view:#?}"));
    let reason = refused["reason"].as_str().expect("a reason");
    assert!(reason.contains("meta"), "the field that refused: {reason}");
    assert!(
        reason.contains("field-notes"),
        "what the matter carried instead: {reason}"
    );
}

/// 4. The word renders where the work is asked for and where the work is
/// placed: a brief and a `root` template, from the one widened vocabulary
/// (§FS-005-dispatch.1). Neither recipe here asks anything of `meta` in its
/// `when`, so this is the half of the report that is not refused at all — the
/// configuration loads, and the name reaches the ticket as itself.
///
/// Two recipes rather than one, because the two failures are different and
/// `implement` should be able to watch them separately: a brief renders the
/// gap it was given, and a path cannot — a `root` naming something no matter
/// answers is refused by name rather than turned into a directory nobody meant
/// (§FS-005-dispatch.25).
#[test]
fn a_brief_and_a_work_root_render_the_word_the_store_wrote() {
    let asking = |id: &str, root: &str| {
        let mut recipe = slice_recipe();
        recipe["id"] = json!(id);
        recipe["when"] = json!({ "kinds": ["task"] });
        recipe["root"] = json!(root);
        recipe
    };
    let world = watching(Some(json!({
        "recipes": [
            asking("asked", "{root}/slices/asked"),
            asking("placed", "{root}/slices/{meta.context}"),
            asking("elsewhere", "{root}/slices/{meta.nobody}"),
        ]
    })));
    refresh(&world);

    // The words the ticket was given. Today they reach it as the fourteen
    // characters the recipe was written with.
    let output = world
        .ephor()
        .args(["work", "dispatch", "--item", MINE, "--recipe", "asked"])
        .output()
        .expect("dispatch runs");
    assert!(
        output.status.success(),
        "the dispatch failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan = laid_in(&world.forest().join("slices").join("asked"));
    assert!(
        plan.contains("Renew the staging certificate — in acme-labs."),
        "the brief reached the ticket unrendered:\n{plan}"
    );
    assert!(
        !plan.contains("{meta."),
        "a template name survived into the plan as itself:\n{plan}"
    );

    // And where the work was put. Work for two slices has to be able to land
    // in two places, which is what the `yq`-over-`EPHOR_RAW` workaround cannot
    // reach at all.
    let output = world
        .ephor()
        .args([
            "work", "dispatch", "--item", MINE, "--recipe", "placed", "--again",
        ])
        .output()
        .expect("dispatch runs");
    assert!(
        output.status.success(),
        "the dispatch failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let root = world.forest().join("slices").join("acme-labs");
    assert!(
        root.is_dir(),
        "the work did not land in the slice's own root; `slices/` holds {:?}",
        std::fs::read_dir(world.forest().join("slices"))
            .map(|entries| entries
                .filter_map(std::result::Result::ok)
                .map(|entry| entry.file_name())
                .collect::<Vec<_>>())
            .unwrap_or_default()
    );
    assert!(laid_in(&root).contains("in acme-labs."));

    // A `root` naming a key this matter has not got means the entry does not
    // serve this matter: it is withheld rather than offered and then refused,
    // because another matter can carry the key and render the same template
    // (§FS-005-dispatch.25).
    let listing = world
        .ephor()
        .args(["work", "offers", "--item", MINE, "--json"])
        .output()
        .expect("offers runs");
    let offered: Vec<String> = shaped("work", &listing)["offers"]
        .as_array()
        .expect("offers")
        .iter()
        .filter_map(|offer| offer["id"].as_str().map(str::to_string))
        .collect();
    assert!(
        offered.iter().any(|id| id == "asked"),
        "the entry this matter does serve was withheld too: {offered:?}"
    );
    assert!(
        !offered.iter().any(|id| id == "elsewhere"),
        "an entry whose root names a key this matter has not got was offered: {offered:?}"
    );
}

/// 5. A summoned command reads it by name, and one fixed name says which of
/// those names are this matter's (§FS-006-project-interface.3). The vocabulary
/// is one vocabulary, so what a freehand entry is told here is what a program
/// in a state machine is told (§FS-005-dispatch.8).
#[test]
fn a_summoned_command_reads_it_by_name_and_is_told_which_names_are_the_matters() {
    let world = watching(None);
    refresh(&world);

    world
        .ephor()
        .args([
            "actions",
            "run",
            "--item",
            MINE,
            "--command",
            "printf '%s\\n' \"${EPHOR_META_CONTEXT-<unset>}\" > context.txt; \
             printf '%s\\n' \"${EPHOR_META_TIER-<unset>}\" > tier.txt; \
             printf '%s\\n' \"${EPHOR_META_KEYS-<unset>}\" > keys.txt",
        ])
        .assert()
        .success();

    assert_eq!(
        world.read(&format!("{PROJECT}/context.txt")).trim(),
        "acme-labs"
    );
    // A number the store wrote is handed over in its canonical spelling, the
    // same one the selector compares.
    assert_eq!(world.read(&format!("{PROJECT}/tier.txt")).trim(), "1");
    // The enumeration, so a command can tell this matter's variable from one
    // it inherited: a summons does not start from a cleared environment, and
    // an open namespace cannot be blanked name by name.
    let listed = world.read(&format!("{PROJECT}/keys.txt"));
    let mut keys: Vec<&str> = listed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["context", "tier"]);
}

/// 6. The bound. A plan that wrote a list and a paragraph about a task loses
/// those keys, keeps its scalar one, stays in the feed, and the drop is said
/// out loud — with the store's slot still `ok`, because a store that answered
/// has not failed (§FS-005-dispatch.8).
#[test]
fn a_key_the_bound_refuses_is_dropped_and_the_drop_is_named() {
    let world = watching(None);
    let output = refresh(&world);

    // The task is still a matter, which is the half that matters most: a task
    // vanishing because somebody wrote prose about it is the worse failure.
    let theirs = world.matter(THEIRS);
    assert_eq!(theirs["title"], "Rotate the signing key");
    // The scalar survives; the list and the paragraph do not.
    assert_eq!(
        theirs["raw"]["meta"],
        json!({ "context": "field-notes" }),
        "the bound kept the wrong keys: {:#?}",
        theirs["raw"]["meta"]
    );

    // The store answered, so its slot says so: a *NO DATA* line about a plan
    // that merely wrote a paragraph would be a lie about the store.
    assert_eq!(world.feed()["providers"]["rhei"]["ok"], json!(true));

    // And the drop is not silent. Which matter, which key, and enough of the
    // reason to go and fix the plan.
    let said = String::from_utf8_lossy(&output.stderr);
    for expected in [THEIRS, "owners", "handover"] {
        assert!(
            said.contains(expected),
            "the refresh dropped a key without saying '{expected}':\n{said}"
        );
    }
}
