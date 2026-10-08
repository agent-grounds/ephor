//! E2E-056-a-private-source-stays-the-persons: a direct message the person's own
//! chat account heard still shows under the project it is about, and no work on
//! it is written into that project's tree or started with nobody present.
//!
//! The scenario is the one ephor#191 reported. A site binds the shipped chat
//! gateway once (§FS-001-forge-interface.9) and watches rhei, whose registry row
//! claims `agent-grounds/rhei`. Rhei trusts answers to be drafted without the
//! person, so its `answer` recipe runs itself and sweeps every hour
//! (§FS-005-dispatch.24, §FS-005-dispatch.32). A colleague's direct message
//! about `agent-grounds/rhei#12` is rightly placed on rhei's row
//! (§FS-008-attribution). Before this case, what followed was wrong: the
//! unattended sweep wrote the message into rhei's work root, its words in the
//! dossier, and started an agent on it.
//!
//! What this case holds ephor to, once the site lists the gateway in
//! `work.private` (§FS-018-private-sources.1). Each step of the proposal is its
//! own test, so each one's failure is read on its own:
//!
//! 1. Placement does not move: refresh still puts the message on rhei's row.
//! 2. No sweep writes or starts work on it. `work sync`, opening or reopening,
//!    and a `work dispatch` without `--item` pass it over with the hold
//!    `private`, and nothing under rhei's registry root carries its words
//!    (§FS-018-private-sources.3).
//! 3. Every named move that writes — `dispatch --item`, `ask`, `lay` — writes
//!    under the private root alone, ahead of a branch workspace and of the
//!    recipe's own `root`, and the run still starts from the checkout the matter
//!    resolves to (§FS-018-private-sources.2, §FS-005-dispatch.6.1,
//!    §FS-005-dispatch.13).
//! 4. `work run --due` passes the private root over with the hold `private`,
//!    asked first so the gated report shows it too, and so does a ticket laid
//!    before the source was listed; `work run --item` starts it
//!    (§FS-005-dispatch.24, §FS-005-dispatch.24.2).
//! 5. A listed source with no root, or a root inside the project's, is refused
//!    by name and nothing is written (§FS-018-private-sources.2).
//!
//! The site watches a second project, so a sweep without `--act` would be a
//! gated report that writes nothing, and every "nothing was written" here would
//! hold for the wrong reason (§FS-011-command-line.10). Every sweep that is
//! meant to act passes `--act`.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use support::{shaped, write_json, World};

/// The gateway exactly as ephor ships it, installed as the forge `chatgw`.
const GATEWAY: &str = include_str!("../../../config/chat-gateway.example.sh");

/// The project the message is about, and a second one beside it so a bare
/// sweep reaches more than one project.
const RHEI: &str = "rhei";
const OTHER: &str = "grund";
/// The organization both belong to, which the private root repeats.
const ORG: &str = "agent-grounds";

/// The direct conversation, as the gateway spells it and as ephor keys it.
const CHAT: &str = "signal/you#+4915550001";
const ITEM: &str = "chatgw:signal/you#+4915550001";
/// What Dana said, and the words that must not end up in the organization's tree.
const SAID: &str = "About agent-grounds/rhei#12: between us, I think we should drop \
                    the fallback. Please keep this off the issue for now.";
const WORDS: &str = "between us";

/// A branch of rhei with a workspace on disk, named in the message's title so
/// the matter resolves to it (§FS-005-dispatch.13).
const BRANCH: &str = "drop-fallback";
/// The runtime's workflow `work lay` lays down about the message.
const WORKFLOW: &str = "draft-reply";
/// The runtime, by a name of this case's own.
const RUNNER: &str = "stub-runtime";

/// A runtime that lists one workflow, instantiates it, and records every run it
/// is asked to make — the directory it was started in, then its arguments — in
/// `$HOME/starts`. It reports each run finished inside the handshake, so no
/// lock is left and nothing else is remembered about it.
const RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
verb="$1"; shift
if [ "$verb" = templates ]; then
  printf '%s' '[{ "name": "draft-reply", "version": "1.0.0", "source": "project",
    "path": "$WORKFLOWS/draft-reply", "description": "Draft a reply.", "inputs": [] }]'
  exit 0
fi
if [ "$verb" = instantiate ]; then
  shift
  output=""; dry=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --output) output="$2"; shift 2 ;;
      --dry-run) dry=yes; shift ;;
      *) shift ;;
    esac
  done
  if [ -n "$dry" ]; then echo "would render into $output"; exit 0; fi
  mkdir -p "$output/tasks"
  printf '# Rhei: draft a reply\n**States:** draft-reply\n' > "$output/index.rhei.md"
  printf 'name: draft-reply\nstates:\n  drafting:\n    agent: x\n  done:\n    final: true\n' \
    > "$output/states.yaml"
  printf '### Task draft: draft the reply\n**State:** drafting\n\nwork\n' > "$output/tasks/01-draft.md"
  echo "Instantiated into $output"
  exit 0
fi
if [ "$verb" = run ] && [ "${1:-}" = --help ]; then
  printf '%s\n' '      --headless  Detach the run'
  exit 0
fi
if [ "$verb" = run ]; then
  while [ "${1:-}" = --headless ] || [ "${1:-}" = --json ]; do shift; done
  printf '%s %s\n' "$PWD" "$*" >> "$HOME/starts"
  printf '{"id":"stub-run","status":"finished"}\n'
  exit 0
fi
echo "unknown verb $verb" >&2
exit 1
"#;

/// What the site says about whose the gateway is.
#[derive(Clone, Copy, PartialEq)]
enum Private {
    /// Nothing: the site as the issue found it.
    Unlisted,
    /// The gateway is the person's, and its work goes under a root of theirs
    /// outside every project.
    Listed,
    /// The gateway is listed, and no root is declared for its work.
    Rootless,
    /// The gateway is listed, with a root that renders inside rhei's own.
    Inside,
}

/// Where rhei's work goes when no private rung answers.
#[derive(Clone, Copy, PartialEq)]
enum Ladder {
    /// The issue's own: `{root}/panta`, and the recipe names no root.
    Project,
    /// The shipped default `{workspace}/panta` over a branch workspace on disk,
    /// and the recipe carrying a `root` of its own inside the project.
    Branch,
}

/// The template the person writes for their own root, which repeats the
/// organization and the project in its own path.
fn private_template(world: &World) -> String {
    format!("{}/me/private/{{org}}/{{project}}", world.path().display())
}

/// Where that template puts rhei's private work.
fn private_root(world: &World) -> PathBuf {
    world.path().join("me/private").join(ORG).join(RHEI)
}

/// Rhei's registry root: its checkouts, and its own work root.
fn rhei_root(world: &World) -> PathBuf {
    world.path().join(RHEI)
}

fn private_block(world: &World, private: Private) -> Option<Value> {
    match private {
        Private::Unlisted => None,
        Private::Listed => Some(json!({
            "sources": ["chatgw"],
            "root": private_template(world)
        })),
        Private::Rootless => Some(json!({ "sources": ["chatgw"] })),
        Private::Inside => Some(json!({ "sources": ["chatgw"], "root": "{root}/private" })),
    }
}

/// The issue's site: the gateway bound once for the site over a spool its
/// listener keeps current, rhei and a second project in one organization, and
/// rhei's `answer` recipe running itself. Dana's message is in the spool.
fn site(private: Private, ladder: Ladder, autorun: bool) -> World {
    let world = World::new();
    world.stub("ephor-forge-chatgw", GATEWAY);
    let workflows = world.path().join("workflows");
    fs::create_dir_all(workflows.join(WORKFLOW)).expect("the workflow's directory");
    fs::write(
        workflows.join(WORKFLOW).join("template.yaml"),
        "name: draft-reply\ninputs: []\n",
    )
    .expect("the workflow's own manifest");
    world.stub(
        RUNNER,
        &RUNTIME.replace("$WORKFLOWS", &workflows.to_string_lossy()),
    );
    register(&world, ladder);
    configure(&world, private, ladder, autorun);
    hearing(&world);
    heard(&world, &[("Dana", SAID)], ladder);
    world
}

/// The site as the issue wrote it, with the gateway listed as the person's.
fn listed() -> World {
    site(Private::Listed, Ladder::Project, true)
}

fn register(world: &World, ladder: Ladder) {
    let mut registry = world.registry_doc();
    let template = registry["projects"][0].clone();
    let row = |id: &str, display: &str| {
        let mut row = template.clone();
        row["id"] = json!(id);
        row["display_name"] = json!(display);
        let root = world.path().join(id);
        fs::create_dir_all(&root).expect("the project's root");
        row["root"] = json!(root.to_string_lossy());
        row["organization"] = json!(ORG);
        row["territory"] = json!([format!("{ORG}/{id}")]);
        row
    };
    let mut rhei = row(RHEI, "Rhei");
    if ladder == Ladder::Branch {
        rhei["branch_root_template"] = json!("{project_root}/{branch}");
        rhei["branches"] = json!([{ "id": BRANCH, "branch": BRANCH, "active": true }]);
        fs::create_dir_all(rhei_root(world).join(BRANCH)).expect("the branch workspace");
    }
    registry["organizations"] = json!([{ "id": ORG, "name": "Agent Grounds" }]);
    registry["projects"] = json!([rhei, row(OTHER, "Grund")]);
    write_json(&world.registry_path(), &registry);
}

fn configure(world: &World, private: Private, ladder: Ladder, autorun: bool) {
    let mut recipe = json!({
        "id": "answer",
        "description": "draft a reply",
        "autorun": autorun,
        "dispatch": "1h",
        "when": { "needs_response": true, "awaits": ["conversation"] },
        "brief": "Draft a reply to {title}."
    });
    let root = match ladder {
        Ladder::Project => "{root}/panta",
        Ladder::Branch => {
            recipe["root"] = json!("{workspace}/answers");
            "{workspace}/panta"
        }
    };
    let mut work = json!({ "runner": RUNNER });
    if let Some(block) = private_block(world, private) {
        work["private"] = block;
    }
    write_json(
        &world.config_path(),
        &json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "sources": [{
                "provider": "chatgw",
                "spool": spool(world).to_string_lossy(),
                "account": "you",
                "max_age_seconds": 300
            }],
            "work": work,
            "projects": {
                RHEI: { "providers": [], "work": { "root": root, "recipes": [recipe] } },
                OTHER: { "providers": [] }
            }
        }),
    );
}

fn spool(world: &World) -> PathBuf {
    world.path().join("spool")
}

/// The listener's word that the chat network confirmed, just now, that it is
/// hearing it.
fn hearing(world: &World) {
    let observed = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    fs::create_dir_all(spool(world)).expect("the spool");
    write_json(
        &spool(world).join("listener.json"),
        &json!({ "link": "up", "observed_at": observed }),
    );
}

/// The direct conversation as the listener records it. It has no room, so it
/// is placed by the repository it names. On the branch ladder its title names
/// the branch, which is how the matter resolves to that workspace.
fn heard(world: &World, said: &[(&str, &str)], ladder: Ladder) {
    let messages: Vec<Value> = said
        .iter()
        .enumerate()
        .map(|(at, (author, text))| {
            json!({
                "author": author,
                "text": text,
                "when": format!("2026-10-01T09:{:02}:00Z", 12 + at),
            })
        })
        .collect();
    let title = match ladder {
        Ladder::Project => "Dana".to_string(),
        Ladder::Branch => format!("Dana: {BRANCH}"),
    };
    let conversations = spool(world).join("conversations");
    fs::create_dir_all(&conversations).expect("the listener's conversations");
    write_json(
        &conversations.join("dana.json"),
        &json!({
            "id": CHAT,
            "title": title,
            "updated_at": format!("2026-10-01T09:{:02}:00Z", 11 + said.len()),
            "reasons": ["mentioned"],
            "threads": [{ "messages": messages, "reply": { "chat": "+4915550001" } }]
        }),
    );
}

fn refresh(world: &World) {
    let done = world.ephor().arg("refresh").output().expect("a refresh");
    assert!(
        done.status.success(),
        "the site refreshes cleanly:\n{}{}",
        String::from_utf8_lossy(&done.stdout),
        String::from_utf8_lossy(&done.stderr)
    );
}

/// `ephor <args> --json`, which must succeed, held to the shape it publishes.
fn reading(world: &World, args: &[&str], shape: &str) -> Value {
    let output = world
        .ephor_raw()
        .args(args)
        .arg("--json")
        .output()
        .expect("ephor runs");
    assert!(
        output.status.success(),
        "`ephor {}` failed:\n{}{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    shaped(shape, &output)
}

/// A sweep at the site's width, told to act (§FS-011-command-line.10).
fn acting(world: &World, args: &[&str], shape: &str) -> Value {
    let mut args = args.to_vec();
    args.push("--act");
    reading(world, &args, shape)
}

/// Every run the runtime was asked to make: the directory it started in, then
/// the root and the rest of its arguments.
fn starts(world: &World) -> Vec<String> {
    fs::read_to_string(world.path().join("starts"))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// Every file under `dir`, at any depth.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Every file under `dir` that carries `words`.
fn carrying(dir: &Path, words: &str) -> Vec<PathBuf> {
    files_under(dir)
        .into_iter()
        .filter(|path| {
            fs::read(path)
                .map(|bytes| String::from_utf8_lossy(&bytes).contains(words))
                .unwrap_or(false)
        })
        .collect()
}

/// Every plan file under `dir`.
fn plans_under(dir: &Path) -> Vec<PathBuf> {
    files_under(dir)
        .into_iter()
        .filter(|path| path.to_string_lossy().ends_with(".rhei.md"))
        .collect()
}

/// The row a sweep's reading holds about the message.
fn landing(reading: &Value) -> Value {
    reading["items"]
        .as_array()
        .and_then(|items| items.iter().find(|row| row["item"] == ITEM).cloned())
        .unwrap_or_else(|| panic!("no row about {ITEM} in the reading:\n{reading:#}"))
}

/// The row a run reading holds about the work root `root`.
fn run_row(reading: &Value, root: &Path) -> Value {
    reading["runs"]
        .as_array()
        .and_then(|runs| {
            runs.iter()
                .find(|run| run["root"].as_str().map(Path::new) == Some(root))
                .cloned()
        })
        .unwrap_or_else(|| {
            panic!(
                "no row about {} in the reading:\n{reading:#}",
                root.display()
            )
        })
}

/// A sweep's row about the message passed it over, naming the source as
/// private, as data and in its sentence (§FS-018-private-sources.3).
fn assert_passed_over_as_private(row: &Value, reading: &Value) {
    assert_eq!(
        row["outcome"], "passed-over",
        "a sweep passes a private matter over: {reading:#}"
    );
    assert_eq!(row["hold"]["kind"], "private", "{reading:#}");
    assert_eq!(row["hold"]["source"], "chatgw", "{reading:#}");
    let says = row["says"].as_str().unwrap_or_default();
    assert!(
        says.contains("chatgw") && says.contains("private"),
        "the sentence names the source as private: {says}"
    );
}

/// What a sweep may leave behind about a private matter: nothing anywhere.
fn assert_nothing_written(world: &World, sweep: &str) {
    let leaked = carrying(&rhei_root(world), WORDS);
    let started = starts(world);
    assert!(
        leaked.is_empty() && started.is_empty(),
        "`{sweep}` wrote the private message into rhei's tree {leaked:#?} and started the \
         runtime {started:#?}"
    );
    let plans = plans_under(world.path());
    assert!(plans.is_empty(), "`{sweep}` wrote plans: {plans:#?}");
    let listed = reading(world, &["work", "list"], "work-list");
    assert_eq!(
        listed,
        json!([]),
        "`{sweep}` recorded work on the private matter"
    );
}

/// The plan id a named dispatch wrote, read off the plan path it reports.
fn plan_id_of(row: &Value) -> String {
    let plan = row["plan"].as_str().expect("the dispatch names its plan");
    Path::new(plan)
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".rhei.md"))
        .expect("a plan file")
        .to_string()
}

/// Step 1. Placement does not move: listing the gateway as the person's changes
/// whose the work is, never which project the message is about
/// (§FS-018-private-sources, §FS-008-attribution).
#[test]
fn step_1_the_message_still_shows_under_the_project_it_is_about() {
    let world = listed();
    refresh(&world);

    let rows = reading(&world, &["feed"], "feed");
    let row = rows
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["id"] == ITEM).cloned())
        .unwrap_or_else(|| panic!("no row {ITEM} in the feed, which holds:\n{rows:#}"));
    assert_eq!(row["project"], RHEI, "it names rhei#12: {row:#}");
    assert_eq!(row["source"], "chatgw", "{row:#}");
    assert_eq!(row["needs_response"], true, "Dana is waiting: {row:#}");
}

/// Step 2, the issue's own transcript: the hourly sweep `work sync` runs opens
/// nothing on a private matter, starts nothing, and says why
/// (§FS-018-private-sources.3, §FS-005-dispatch.24).
#[test]
fn step_2_sync_writes_and_starts_nothing_on_a_private_matter() {
    let world = listed();
    refresh(&world);

    let synced = acting(&world, &["work", "sync"], "work-sync");
    assert_nothing_written(&world, "work sync");
    assert_eq!(synced["opened"], 0, "{synced:#}");
    assert_passed_over_as_private(&landing(&synced), &synced);
}

/// Step 2, the other sweep: a `work dispatch` that names no item is a sweep
/// whoever typed it, and passes the private matter over the same way.
#[test]
fn step_2_a_dispatch_without_an_item_writes_and_starts_nothing_on_a_private_matter() {
    let world = listed();
    refresh(&world);

    let swept = acting(&world, &["work", "dispatch"], "work-dispatch");
    assert_nothing_written(&world, "work dispatch");
    assert_eq!(swept["opened"], 0, "{swept:#}");
    assert_passed_over_as_private(&landing(&swept), &swept);
}

/// Step 2, reopening: work the person laid on a private matter is not reopened
/// by `work sync` when the matter moves — the plan is not written to and
/// nothing is started (§FS-018-private-sources.3).
#[test]
fn step_2_sync_does_not_reopen_a_private_matter_that_moved() {
    let world = listed();
    refresh(&world);
    let dispatched = reading(
        &world,
        &["work", "dispatch", "--item", ITEM],
        "work-dispatch",
    );
    let plan = PathBuf::from(
        landing(&dispatched)["plan"]
            .as_str()
            .expect("the dispatch names its plan"),
    );
    let before = fs::read_to_string(&plan).expect("the plan the person laid");
    let started = starts(&world);

    hearing(&world);
    heard(
        &world,
        &[
            ("Dana", SAID),
            ("Dana", "Also: the fallback test is flaky."),
        ],
        Ladder::Project,
    );
    refresh(&world);
    let synced = acting(&world, &["work", "sync"], "work-sync");

    let after = fs::read_to_string(&plan).expect("the plan is still there");
    assert!(
        after == before && starts(&world) == started,
        "`work sync` reopened the private matter's work: {synced:#}\nthe plan now reads:\n{after}"
    );
    assert_eq!(synced["reopened"], 0, "{synced:#}");
    assert_passed_over_as_private(&landing(&synced), &synced);
    let leaked = carrying(&rhei_root(&world), WORDS);
    assert!(
        leaked.is_empty(),
        "rhei's tree carries the message: {leaked:#?}"
    );
}

/// Step 3. Every named move that writes a plan writes it under the private
/// root, and only there — though the matter resolves to a branch workspace
/// (§FS-005-dispatch.13) and the recipe names a root of its own
/// (§FS-005-dispatch.6.1). The run still starts from the branch workspace,
/// and ephor writes nothing into it (§FS-018-private-sources.2).
#[test]
fn step_3_every_named_move_writes_under_the_private_root_alone() {
    let world = site(Private::Listed, Ladder::Branch, true);
    refresh(&world);
    let mine = private_root(&world);
    let workspace = rhei_root(&world).join(BRANCH);

    let dispatched = reading(
        &world,
        &["work", "dispatch", "--item", ITEM],
        "work-dispatch",
    );
    let asked = reading(
        &world,
        &[
            "work", "ask", "--item", ITEM, "Draft", "a", "short", "reply.",
        ],
        "work-ask",
    );
    let laid = reading(
        &world,
        &["work", "lay", WORKFLOW, "--item", ITEM],
        "work-lay",
    );

    let written = files_under(&rhei_root(&world));
    assert!(
        written.is_empty(),
        "named moves on a private matter wrote into rhei's tree {written:#?}\n\
         dispatch: {dispatched:#}\nask: {asked:#}\nlay: {laid:#}"
    );
    let plan = landing(&dispatched)["plan"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        Path::new(&plan).starts_with(&mine),
        "`dispatch --item` writes under the private root ahead of the recipe's own: {dispatched:#}"
    );
    let says = asked["says"].as_str().unwrap_or_default();
    assert!(
        says.contains(&*mine.to_string_lossy()),
        "`ask --item` writes under the private root: {asked:#}"
    );
    let laid_plan = laid["plan"].as_str().unwrap_or_default();
    assert!(
        Path::new(laid_plan).starts_with(&mine),
        "`lay --item` writes under the private root: {laid:#}"
    );
    assert!(
        carrying(&rhei_root(&world), WORDS).is_empty(),
        "rhei's tree carries the message"
    );

    // The run is the person's, and it reads the change where the change is.
    assert!(
        starts(&world).is_empty(),
        "nothing ran before it was asked for"
    );
    let ran = reading(&world, &["work", "run", "--item", ITEM], "work-run");
    let row = run_row(&ran, &mine);
    assert_eq!(
        row["checkout"],
        json!(workspace.to_string_lossy()),
        "the run starts from the checkout the matter resolves to: {ran:#}"
    );
    let started = starts(&world);
    assert_eq!(
        started.len(),
        1,
        "one run, of the private root: {started:#?}"
    );
    assert!(
        started[0].starts_with(&format!("{} {}", workspace.display(), mine.display())),
        "the runtime was started in the branch workspace over the private root: {started:#?}"
    );
    let written = files_under(&rhei_root(&world));
    assert!(
        written.is_empty(),
        "ephor wrote into the checkout: {written:#?}"
    );
}

/// Step 4. The due sweep starts nothing on a private root — not the one
/// `dispatch --item` runs after it writes, not the gated report, not the sweep
/// that acts — and says why, as the first hold it asks (§FS-005-dispatch.24.2).
/// The person's own `run --item` starts it, and `ephor work` finds the plan
/// where it was written.
#[test]
fn step_4_the_due_sweep_passes_a_private_root_over_and_a_named_run_starts_it() {
    let world = listed();
    refresh(&world);
    let mine = private_root(&world);

    let dispatched = reading(
        &world,
        &["work", "dispatch", "--item", ITEM],
        "work-dispatch",
    );
    let after_dispatch = starts(&world);
    let due = acting(&world, &["work", "run", "--due"], "work-run");
    let after_due = starts(&world);
    assert!(
        after_due.is_empty(),
        "no sweep may start work on a private matter, but the runtime was started: after \
         `dispatch --item`'s own sweep {after_dispatch:#?}, after `run --due` {after_due:#?}\n\
         the due sweep said: {due:#}"
    );

    let row = run_row(&due, &mine);
    let plan_id = plan_id_of(&landing(&dispatched));
    assert_eq!(row["outcome"], "passed-over", "{due:#}");
    assert_eq!(row["hold"]["kind"], "private", "{due:#}");
    assert_eq!(row["hold"]["source"], "chatgw", "{due:#}");
    let held = row["hold"]["tickets"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        held.len() == 1
            && held[0]["ticket"] == json!(format!("{plan_id}.answer-1"))
            && held[0]["state"]
                .as_str()
                .is_some_and(|state| !state.is_empty()),
        "the hold names each ticket it keeps and its state, as `person` does: {due:#}"
    );
    let reason = row["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("chatgw") && reason.contains("private"),
        "the reason says the source is private: {reason}"
    );

    // Asked first, so a sweep held at the gate says the same.
    let gated = reading(&world, &["work", "run", "--due"], "work-run");
    assert_eq!(
        gated["gated"], true,
        "two projects gate a bare sweep: {gated:#}"
    );
    let held = run_row(&gated, &mine);
    assert_eq!(held["outcome"], "passed-over", "{gated:#}");
    assert_eq!(held["hold"]["kind"], "private", "{gated:#}");
    assert!(
        starts(&world).is_empty(),
        "the gated report started nothing"
    );

    // The person's key.
    let ran = reading(&world, &["work", "run", "--item", ITEM], "work-run");
    assert_eq!(
        starts(&world).len(),
        1,
        "`run --item` starts the private work once: {ran:#}"
    );
    let row = run_row(&ran, &mine);
    assert_eq!(
        row["checkout"],
        json!(rhei_root(&world).to_string_lossy()),
        "from the checkout the matter resolves to: {ran:#}"
    );

    let listed = reading(&world, &["work", "list"], "work-list");
    let entry = listed
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["item"] == ITEM).cloned())
        .unwrap_or_else(|| panic!("`ephor work` does not list the private work: {listed:#}"));
    assert_eq!(entry["root"], json!(mine.to_string_lossy()), "{entry:#}");
    assert!(
        Path::new(entry["plan"].as_str().unwrap_or_default()).starts_with(&mine),
        "{entry:#}"
    );
}

/// Step 4, a ticket laid before the source was listed: it stays where it was
/// written, and stops autorunning the moment the site lists its source,
/// because whose a ticket is is read off the ticket (§FS-005-dispatch.8,
/// §FS-018-private-sources.3).
#[test]
fn step_4_a_ticket_laid_before_its_source_was_listed_stops_autorunning() {
    let world = site(Private::Unlisted, Ladder::Project, false);
    refresh(&world);
    let laid = reading(
        &world,
        &["work", "dispatch", "--item", ITEM],
        "work-dispatch",
    );
    let panta = rhei_root(&world).join("panta");
    assert!(
        Path::new(landing(&laid)["plan"].as_str().unwrap_or_default()).starts_with(&panta),
        "an unlisted source climbs the ladder it always did: {laid:#}"
    );
    assert!(
        starts(&world).is_empty(),
        "the recipe did not run itself yet"
    );

    configure(&world, Private::Listed, Ladder::Project, true);
    let due = acting(&world, &["work", "run", "--due"], "work-run");
    let started = starts(&world);
    assert!(
        started.is_empty(),
        "the due sweep started a ticket about a private matter: {started:#?}\n{due:#}"
    );
    let row = run_row(&due, &panta);
    assert_eq!(row["outcome"], "passed-over", "{due:#}");
    assert_eq!(row["hold"]["kind"], "private", "{due:#}");
    assert_eq!(row["hold"]["source"], "chatgw", "{due:#}");
}

/// The refusal a named move gets where the private rung cannot answer: it
/// fails, says `words`, and writes nothing anywhere (§FS-018-private-sources.2).
fn assert_refused(world: &World, args: &[&str], words: &[&str]) {
    let output = world.ephor_raw().args(args).output().expect("ephor runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "`ephor {}` wrote work on a private matter it had nowhere of the person's to put:\n{said}",
        args.join(" ")
    );
    for word in words {
        assert!(
            said.contains(word),
            "`ephor {}` is refused by name, saying `{word}`:\n{said}",
            args.join(" ")
        );
    }
    let plans = plans_under(world.path());
    assert!(plans.is_empty(), "a refusal wrote plans: {plans:#?}");
    let written = files_under(&rhei_root(world));
    assert!(
        written.is_empty(),
        "a refusal wrote into rhei's tree: {written:#?}"
    );
    assert!(starts(world).is_empty(), "a refusal started the runtime");
}

/// Every named move that writes work about the message.
fn writing_moves() -> [Vec<&'static str>; 3] {
    [
        vec!["work", "dispatch", "--item", ITEM],
        vec!["work", "ask", "--item", ITEM, "Draft", "it."],
        vec!["work", "lay", WORKFLOW, "--item", ITEM],
    ]
}

/// Step 5. A source listed with no root has nowhere of the person's to go, and
/// falling back to the project's ladder would be the leak itself, so every
/// named move that writes is refused by name. The file still loads: the feed
/// is not the refusal's business.
#[test]
fn step_5_a_private_source_with_no_root_is_refused_by_name() {
    let world = site(Private::Rootless, Ladder::Project, true);
    refresh(&world);

    let words = ["work.private.root", "chatgw"];
    for writing in writing_moves() {
        assert_refused(&world, &writing, &words);
    }
}

/// Step 5, the other refusal: a private root that renders inside the project's
/// registry root is the organization's root under another name.
#[test]
fn step_5_a_private_root_inside_the_projects_root_is_refused_by_name() {
    let world = site(Private::Inside, Ladder::Project, true);
    refresh(&world);

    // It names the key, what the key rendered, and the root that holds it.
    let rendered = rhei_root(&world).join("private");
    let project = rhei_root(&world);
    let words = [
        "work.private.root",
        &*rendered.to_string_lossy(),
        &*project.to_string_lossy(),
    ];
    for writing in writing_moves() {
        assert_refused(&world, &writing, &words);
    }
}
