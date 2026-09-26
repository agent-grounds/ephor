//! E2E-030-a-standing-instruction-kept-in-its-file: a recipe names the file its
//! brief is kept in, and every ticket opens with that file's current words.
//!
//! The scenario is a site that has one standing instruction — how work is done
//! under this organization — and does not want a second copy of it inside
//! `status.json`. The recipe names the file instead, and ephor reads it at the
//! moment the ticket is written, so the plan carries the instruction rather
//! than a path to it (§FS-005-dispatch.34, §FS-005-dispatch.2).
//!
//! Four things are pinned here, and each of them is a decision rather than a
//! mechanism. The file's text and the rendered `brief` **compose**, in that
//! order, because a standing instruction and a per-matter ask are different
//! things (§FS-005-dispatch.34.1). The file is a document and not a template:
//! its own `{title}` survives as those seven characters, while the `{title}` in
//! `brief` is the matter's (§FS-005-dispatch.34). Its headings arrive flattened,
//! because a heading inside a plan is a node the runtime would read as a task
//! (§FS-005-dispatch.3). And each ticket records **which** version of the
//! instruction it was given — the rendered path and a sha256 of the bytes as
//! read — so a second ticket written after the file changed keeps its own
//! answer and does not correct the first (§FS-005-dispatch.34.2,
//! §FS-005-dispatch.8).
//!
//! The last case is the refusal. A rendered path with no readable file behind
//! it is a path, not prose, so it refuses naming the path and leaves nothing
//! behind — no work root and no plan (§FS-005-dispatch.34,
//! §FS-005-dispatch.6.1).

#[path = "../support.rs"]
mod support;

use predicates::prelude::*;
use serde_json::json;

use support::*;

/// The matter: one issue of the reader's own, so the recipe below is what
/// applies to it.
const ITEM: &str = "acmeforge:acme/widget#7";
/// The plan ephor derives from that key.
const PLAN: &str = "acmeforge-acme-widget-7";
/// The recipe's id, which is also the ticket prefix.
const RECIPE: &str = "desires";

/// The standing instruction, as its owner wrote it: a document with headings,
/// and an example that names a placeholder it does not want substituted.
const INSTRUCTION: &str = r#"# How we work under this organization

Read the spec before the code, and cite the point the change realizes.

## A standing instruction is not a template

Braces here are characters. An example naming `{title}` must reach the ticket
as those seven characters and not as the matter's own title.
"#;

/// `sha256sum` of exactly those bytes. Written out rather than computed here,
/// so the case pins the algorithm the specification names and not whatever the
/// implementation happens to do (§FS-005-dispatch.34.2).
const INSTRUCTION_SHA256: &str = "3cae84eb74c8f515a91508448f6d1054ace8a8f9a239e4bb98417b478b43556a";

/// The same instruction after its owner edited it.
const INSTRUCTION_EDITED: &str = r#"# How we work under this organization

The instruction has changed since the first ticket was written, and the second
ticket must carry the new words and a hash of its own.
"#;

const INSTRUCTION_EDITED_SHA256: &str =
    "418bf68b2ca0121387118ea96c585d4a135cdab45de37f4f0e8edc050ea7cf11";

/// A forge with one issue of the reader's. When it was last touched is read
/// out of a file, so the case can move the matter and watch the work reopen
/// (§FS-005-dispatch.5).
const ACME_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities)
    printf '{"issues":true}'
    ;;
  issues)
    printf '%s' '[
      { "key": "acme/widget#7", "title": "Widen the retry window",
        "url": "https://acme.example/issue/7",
        "updated_at": "'"$(cat "$HOME/touched-at")"'", "status": "open",
        "role": "author" }
    ]'
    ;;
  *)
    printf '[]'
    ;;
esac
"#;

const FIRST_TIME: &str = "2026-07-30T12:00:00Z";
const LATER_TIME: &str = "2026-07-31T09:00:00Z";

/// A world watching that forge, with one recipe that keeps its brief in a file
/// beside the project. The path is a template rendered from the same names a
/// work root is rendered from, which is what makes `{root}` legal here
/// (§FS-005-dispatch.34, §FS-005-dispatch.6.1).
fn watching() -> World {
    let world = World::new();
    world.stub("ephor-forge-acmeforge", ACME_FORGE);
    std::fs::write(world.path().join("touched-at"), FIRST_TIME).expect("the forge's clock");
    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "acmeforge", "user": "you", "repos": ["acme/widget"] }
        ] } },
        "work": {
            "runner": "acme-runtime",
            "recipes": [{
                "id": RECIPE,
                "icon": "📜",
                "description": "work an issue under the organization's standing instruction",
                "state": "fix",
                "needs_checkout": false,
                "when": { "kinds": ["issue"] },
                "brief_file": "{root}/DESIRES.md",
                "brief": "Work {title} — {url}."
            }]
        }
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

/// The same world with the instruction on disk.
fn instructed() -> World {
    let world = watching();
    world.file("DESIRES.md", INSTRUCTION);
    world
}

fn plan_path(world: &World) -> std::path::PathBuf {
    world.forest().join("panta").join(format!("{PLAN}.rhei.md"))
}

fn dispatch(world: &World) -> assert_cmd::assert::Assert {
    world
        .ephor()
        .args(["work", "dispatch", "--item", ITEM, "--recipe", RECIPE])
        .assert()
}

/// The position of `needle` in `text`, or a failure naming what was there —
/// a case about the order two things arrive in has to say what it found.
fn at(text: &str, needle: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("nothing in the plan says {needle:?}:\n{text}"))
}

/// A recipe may name the file its brief is kept in, and the preview shows what
/// the hand-over would actually carry — the file's words, then the rendered
/// `brief` (§FS-005-dispatch.34, §FS-005-dispatch.34.1, §REQ-002-parity.3).
#[test]
fn a_recipe_may_name_the_file_its_brief_is_kept_in() {
    let world = instructed();

    let offered = world
        .ephor()
        .args(["work", "offers", "--item", ITEM, "--json"])
        .output()
        .expect("work offers runs");
    assert!(
        offered.status.success(),
        "the configuration did not load: {}",
        String::from_utf8_lossy(&offered.stderr)
    );

    let view = json_of(&offered);
    let offer = view["offers"]
        .as_array()
        .expect("the offers are a list")
        .iter()
        .find(|offer| offer["id"] == RECIPE)
        .unwrap_or_else(|| panic!("no {RECIPE} offer in {}", view["offers"]));
    let brief = offer["brief"]
        .as_str()
        .unwrap_or_else(|| panic!("the offer carries no brief: {offer}"));

    // The file's own words, read now rather than named for the run to open.
    assert!(
        brief.contains("Read the spec before the code"),
        "the preview does not carry the file's words:\n{brief}"
    );
    // And the rendered `brief` after them, with the matter's own title in it.
    assert!(
        at(brief, "Read the spec before the code")
            < at(brief, "Work acme/widget#7 Widen the retry window"),
        "the file's text does not come first:\n{brief}"
    );
    assert!(
        brief.contains("Work acme/widget#7 Widen the retry window — https://acme.example/issue/7."),
        "the preview does not carry the rendered brief:\n{brief}"
    );
}

/// The ticket carries the instruction as text — flattened, with its own braces
/// intact — and says which version of it this ticket was given
/// (§FS-005-dispatch.34, §FS-005-dispatch.34.2, §FS-005-dispatch.3).
#[test]
fn the_ticket_carries_the_instruction_and_says_which_version_it_got() {
    let world = instructed();

    dispatch(&world)
        .success()
        .stdout(predicate::str::contains("1 ticket(s) opened"));

    let plan = std::fs::read_to_string(plan_path(&world)).expect("the plan is on disk");

    // A heading inside a plan is a node, so an embedded document's headings
    // arrive as emphasis (§FS-005-dispatch.3).
    assert!(
        plan.contains("**How we work under this organization**"),
        "the instruction's heading was not flattened:\n{plan}"
    );
    assert!(
        !plan.contains("\n# How we work under this organization"),
        "the instruction's heading reached the plan as a heading:\n{plan}"
    );

    // The file is a document and not a template: its own braces survive.
    assert!(
        plan.contains("An example naming `{title}` must reach the ticket"),
        "the file's own placeholder was substituted:\n{plan}"
    );

    // And the rendered brief follows it, with the matter's title in it.
    assert!(
        at(&plan, "Read the spec before the code")
            < at(
                &plan,
                "Work acme/widget#7 Widen the retry window — https://acme.example/issue/7."
            ),
        "the file's text does not come first:\n{plan}"
    );

    // Which words this ticket got, on the ticket rather than in the dossier.
    let instruction = world.forest().join("DESIRES.md");
    assert!(
        plan.contains(&format!("instruction: \"{}\"", instruction.display())),
        "the ticket does not record the path it read:\n{plan}"
    );
    assert!(
        plan.contains(&format!("instruction_sha256: \"{INSTRUCTION_SHA256}\"")),
        "the ticket does not record a sha256 of the bytes as read:\n{plan}"
    );
}

/// The file is edited and the matter moves: the second ticket carries the new
/// words and its own hash, and the first keeps the answer it was given
/// (§FS-005-dispatch.34.2, §FS-005-dispatch.5).
#[test]
fn an_edited_instruction_reaches_the_next_ticket_and_the_first_keeps_its_own() {
    let world = instructed();
    dispatch(&world).success();

    world.file("DESIRES.md", INSTRUCTION_EDITED);
    std::fs::write(world.path().join("touched-at"), LATER_TIME).expect("the forge's clock moves");
    world.ephor().args(["refresh", PROJECT]).assert().success();

    let synced = world
        .ephor()
        .args(["work", "sync", "--project", PROJECT, "--json"])
        .output()
        .expect("work sync runs");
    assert!(
        synced.status.success(),
        "work sync failed: {}",
        String::from_utf8_lossy(&synced.stderr)
    );
    assert_eq!(json_of(&synced)["reopened"], 1);

    let plan = std::fs::read_to_string(plan_path(&world)).expect("the plan is on disk");

    // The second ticket has the new words…
    assert!(
        plan.contains("The instruction has changed since the first ticket was written"),
        "the reopened ticket does not carry the edited instruction:\n{plan}"
    );
    // …and its own hash, while the first ticket keeps the one it was given. A
    // hash that named another ticket's text would be worse than none.
    assert!(
        plan.contains(&format!(
            "instruction_sha256: \"{INSTRUCTION_EDITED_SHA256}\""
        )),
        "the reopened ticket does not record a hash of what it read:\n{plan}"
    );
    assert!(
        plan.contains(&format!("instruction_sha256: \"{INSTRUCTION_SHA256}\"")),
        "the first ticket's hash was corrected by the second:\n{plan}"
    );
}

/// A rendered path with no readable file behind it is a path and not prose:
/// dispatch refuses naming it, and nothing is written — no work root, no plan
/// (§FS-005-dispatch.34, §FS-005-dispatch.6.1).
#[test]
fn a_path_with_no_file_behind_it_refuses_naming_it_and_writes_nothing() {
    let world = watching();
    let missing = world.forest().join("DESIRES.md");

    dispatch(&world)
        .failure()
        .stderr(predicate::str::contains(missing.display().to_string()));

    assert!(
        !world.forest().join("panta").exists(),
        "a refused dispatch left a work root behind"
    );
    assert!(
        !plan_path(&world).exists(),
        "a refused dispatch left a plan"
    );
}
