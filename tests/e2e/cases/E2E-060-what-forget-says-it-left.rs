//! E2E-060-what-forget-says-it-left: what `work forget` says about the plans of
//! each entry it drops is read from the plans the record says are the matter's,
//! and from the disk (§FS-005-dispatch.35.1).
//!
//! A project keeps one checkout per branch. Its `implement` action lays an
//! issue's plan under `fix/issue-{number}/panta`, and its `note` recipe writes a
//! plan into the project's own work root. Five issues stand for the five cases
//! the report found: #111 and #112 laid by the action and then removed with
//! their checkouts, #113 laid and kept, #114 dispatched by the recipe and its
//! plan file deleted, #115 dispatched by the recipe and kept. Before the fix,
//! `forget` printed `forgot <id> — its plan stays at <path>` for every one of
//! them, with `<path>` the ledger entry's own `plan`, and only #115 was told the
//! truth: every entry `--missing` selects has a plan that is gone, and an
//! action-laid entry's own path is a file ephor never wrote, while the plan it
//! laid sits beside it unnamed.
//!
//! What this case holds ephor to. No line says a plan *stays at* a path that is
//! not on disk. #113 is pointed at the `index.rhei.md` its workflow laid, and
//! #115 still reads as staying at its recipe plan. #111, #112 and #114 each say
//! their plan was already gone and name the path the record named: for a laid
//! plan, the dispatch's root joined to the name it recorded, never the entry's
//! own path, which was never a plan of it. `--json` keeps `item` and `plan` as
//! they were and carries the same facts in `plans`, one `{plan, missing}` per
//! plan the prose names (§REQ-002-parity.3). And the work view's heading names
//! the same plans rather than the entry's own path in their place.
//!
//! The reproduction behind the ticket built this site in a shell script with a
//! stub forge and a stub runtime; this is that site in the project's own frame,
//! laid through the real `work lay` and `work dispatch` so that the ledger holds
//! exactly what ephor writes.

#[path = "../support.rs"]
mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use support::*;

/// Laid by the action; its checkout then removed. Reached by `--item`.
const LAID_GONE: &str = "acmeforge:app#111";
/// The same, and reached by `--missing`.
const LAID_SWEPT: &str = "acmeforge:app#112";
/// Laid by the action; checkout and plan kept.
const LAID_KEPT: &str = "acmeforge:app#113";
/// Dispatched by the recipe; its plan file then deleted.
const RECIPE_GONE: &str = "acmeforge:app#114";
/// Dispatched by the recipe; plan kept — the case the sentence was written for.
const RECIPE_KEPT: &str = "acmeforge:app#115";

const LAID: [&str; 3] = [LAID_GONE, LAID_SWEPT, LAID_KEPT];
const RECIPE: [&str; 2] = [RECIPE_GONE, RECIPE_KEPT];

/// A forge with five open issues and nothing else.
const ACME_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
issue() {
  printf '{"key":"app#%s","title":"%s","status":"open","url":"https://acme.example/issues/%s","updated_at":"2026-10-20T01:00:00Z","role":"author"}' "$1" "$2" "$1"
}
case "${1:?subcommand}" in
  capabilities) printf '{"issues":true}' ;;
  issues)
    printf '['
    issue 111 "Release notes omit the previous tag"; printf ','
    issue 112 "Changelog drops the last entry"; printf ','
    issue 113 "Version banner is stale"; printf ','
    issue 114 "Docs index lists a removed page"; printf ','
    issue 115 "Footer year is hard-coded"
    printf ']' ;;
  *) printf '[]' ;;
esac
"#;

/// A runtime offering one workflow, `wf`, which renders a two-file plan into
/// whatever `--output` ephor names.
const ACME_RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
verb="$1"; shift
case "$verb" in
  templates) cat <<'JSON'
[{"name":"wf","version":"1.0.0","source":"user","path":"wf","description":"work an issue",
  "inputs":[{"name":"ticket","description":"t","type":"string","required":true,"default":null,"validate":null}]}]
JSON
  ;;
  instantiate)
    output=""
    while [ "$#" -gt 0 ]; do case "$1" in --output) output="$2"; shift 2;; *) shift;; esac; done
    mkdir -p "$output"
    printf '# Rhei: wf\n' > "$output/index.rhei.md"
    printf 'name: wf\nstates:\n  done:\n    final: true\n' > "$output/states.yaml"
    echo "Instantiated wf into $output" ;;
  *) exit 0 ;;
esac
"#;

/// What the record said about one entry before anything was removed, read the
/// way a program reads it: `work list --json`.
#[derive(Debug, Clone)]
struct Recorded {
    /// The ledger entry's own `plan` — what `forget` used to print.
    own: PathBuf,
    /// The plans the reading held, every one of them on disk at the time.
    held: Vec<PathBuf>,
    /// For an action-laid entry, the record's own name of what it laid: the
    /// dispatch's root joined to the name it recorded.
    laid: Option<PathBuf>,
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

/// An origin, and its main branch checked out at `<root>/main`; every other
/// branch is a checkout beside it.
fn project_with_main_checked_out(world: &World) {
    let origin = world.path().join("origin");
    fs::create_dir_all(&origin).expect("the remote");
    git(&origin, &["init", "-q", "--initial-branch=main"]);
    fs::write(origin.join("README.md"), "the project\n").expect("a file");
    git(&origin, &["add", "README.md"]);
    git(&origin, &["commit", "-q", "-m", "the project"]);
    let main = main_checkout(world);
    fs::create_dir_all(main.parent().expect("a parent")).expect("the project root");
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(&origin)
        .arg(&main)
        .status()
        .expect("git clones");
    assert!(status.success());
}

fn main_checkout(world: &World) -> PathBuf {
    world.forest().join("main")
}

fn number(id: &str) -> &str {
    id.rsplit('#').next().expect("an issue number")
}

/// The checkout the action's `branch: fix/issue-{number}` puts a matter in.
fn issue_checkout(world: &World, id: &str) -> PathBuf {
    world.forest().join(format!("fix/issue-{}", number(id)))
}

fn work_list(world: &World) -> Value {
    let output = world
        .ephor()
        .args(["work", "list", "--json"])
        .output()
        .expect("work list --json runs");
    assert!(output.status.success(), "work list --json failed");
    json_of(&output)
}

fn row<'a>(list: &'a Value, id: &str) -> &'a Value {
    list.as_array()
        .expect("work list --json is an array")
        .iter()
        .find(|row| row["item"] == json!(id))
        .unwrap_or_else(|| panic!("work list --json has no entry for {id}: {list}"))
}

fn path_of(value: &Value) -> PathBuf {
    PathBuf::from(value.as_str().expect("a path"))
}

/// The site the report describes, built through ephor's own verbs: three
/// matters laid by the action and two dispatched by the recipe, then #111's and
/// #112's checkouts removed and #114's plan deleted. Returns what the record
/// said about each before anything was removed.
fn site() -> (World, BTreeMap<&'static str, Recorded>) {
    let world = World::new();
    world.stub("ephor-forge-acmeforge", ACME_FORGE);
    world.stub("acme-runtime", ACME_RUNTIME);
    project_with_main_checked_out(&world);
    world.register(json!({ "branch_root_template": "{project_root}/{branch}" }));
    world.configure(json!({
        "projects": { PROJECT: {
            "providers": [
                { "provider": "acmeforge", "user": "you", "repos": ["app"], "unclaimed": true }
            ],
            "actions": [{
                "id": "implement", "icon": "*", "description": "work this issue",
                "workflow": "wf", "requires_checkout": true,
                "branch": "fix/issue-{number}", "root": "{workspace}/panta",
                "when": { "kinds": ["issue"] },
                "inputs": { "ticket": "{repo}#{number}" }
            }],
            "work": {
                "root": "{root}/panta",
                "recipes": [{
                    "id": "note", "icon": "n", "description": "note this issue",
                    "state": "fix", "needs_checkout": false,
                    "when": { "kinds": ["issue"] }, "brief": "{title}"
                }]
            }
        } },
        "work": { "runner": "acme-runtime" }
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();

    for id in LAID {
        world
            .ephor()
            .args(["work", "lay", "implement", "--item", id])
            .assert()
            .success();
        assert!(
            issue_checkout(&world, id).is_dir(),
            "work lay minted no checkout for {id}"
        );
    }
    for id in RECIPE {
        world
            .ephor()
            .args(["work", "dispatch", "--item", id, "--recipe", "note"])
            .assert()
            .success();
    }

    let list = work_list(&world);
    let mut recorded = BTreeMap::new();
    for id in LAID.into_iter().chain(RECIPE) {
        let row = row(&list, id);
        let held: Vec<PathBuf> = row["plans"]
            .as_array()
            .expect("the plans the reading holds")
            .iter()
            .map(|plan| path_of(&plan["plan"]))
            .collect();
        assert_eq!(held.len(), 1, "{id} should hold exactly one plan: {row}");
        assert!(
            held[0].is_file(),
            "{id}'s plan is not on disk even before removal"
        );
        let laid = row["workflows"]
            .as_array()
            .and_then(|laid| laid.first())
            .map(|laid| {
                path_of(&laid["root"]).join(laid["plan"].as_str().expect("the laid plan's name"))
            });
        recorded.insert(
            id,
            Recorded {
                own: path_of(&row["plan"]),
                held,
                laid,
            },
        );
    }
    for id in LAID {
        let entry = &recorded[id];
        assert!(
            !entry.own.exists(),
            "an action-laid entry's own path must be a file ephor never wrote, or this case pins nothing"
        );
        let laid = entry
            .laid
            .as_ref()
            .expect("the record names what the action laid");
        assert!(
            entry.held[0].starts_with(laid),
            "the laid plan {} is not inside the workspace the record names, {}",
            entry.held[0].display(),
            laid.display()
        );
    }
    for id in RECIPE {
        let entry = &recorded[id];
        assert_eq!(
            entry.held,
            vec![entry.own.clone()],
            "{id}'s recipe plan is its own path"
        );
    }

    for id in [LAID_GONE, LAID_SWEPT] {
        let checkout = issue_checkout(&world, id);
        git(
            &main_checkout(&world),
            &["worktree", "remove", "--force", &checkout.to_string_lossy()],
        );
        assert!(
            !checkout.exists(),
            "{} is still on disk",
            checkout.display()
        );
    }
    fs::remove_file(&recorded[RECIPE_GONE].own).expect("delete #114's plan");

    let list = work_list(&world);
    for id in LAID.into_iter().chain(RECIPE) {
        let missing = row(&list, id)["missing"] == json!(true);
        let expected = ![LAID_KEPT, RECIPE_KEPT].contains(&id);
        assert_eq!(
            missing, expected,
            "{id} does not read as the fixture means it to"
        );
    }
    (world, recorded)
}

fn ledger_path(world: &World) -> PathBuf {
    world.path().join("state/ephor/work.json")
}

fn forget(world: &World, args: &[&str]) -> std::process::Output {
    let mut argv = vec!["work", "forget"];
    argv.extend_from_slice(args);
    let output = world.ephor().args(argv).output().expect("work forget runs");
    assert!(
        output.status.success(),
        "work forget {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn forget_prose(world: &World, args: &[&str]) -> String {
    String::from_utf8_lossy(&forget(world, args).stdout).into_owned()
}

/// The same selection asked for in both forms, from one and the same ledger:
/// `--json` first, then the ledger put back as it was, then the prose.
fn forget_both(world: &World, args: &[&str]) -> (Value, String) {
    let before = fs::read(ledger_path(world)).expect("the ledger");
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let reading = shaped("work-forget", &forget(world, &json_args));
    fs::write(ledger_path(world), &before).expect("put the ledger back");
    (reading, forget_prose(world, args))
}

/// What one forget printed about one entry: from the line naming it up to the
/// next entry's line, so a report that names each plan on a line of its own is
/// read whole.
fn said_about(out: &str, id: &str) -> Option<String> {
    let lines: Vec<&str> = out.lines().collect();
    let start = lines.iter().position(|line| line.contains(id))?;
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with("forgot "))
        .map_or(lines.len(), |at| start + 1 + at);
    Some(lines[start..end].join("\n"))
}

/// Every path a report says a plan *stays at*.
fn stays_at(said: &str) -> Vec<PathBuf> {
    said.match_indices("stays at ")
        .map(|(at, phrase)| {
            let rest = &said[at + phrase.len()..];
            let token = rest
                .split(|c: char| c.is_whitespace() || c == ';' || c == ',' || c == ')')
                .next()
                .unwrap_or_default();
            PathBuf::from(token.trim_end_matches('.'))
        })
        .collect()
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

/// Judge what forget said about one entry against what is on disk: a plan on
/// disk stays at its path, a plan that is gone is said to be, and no path is
/// named that was never one of the matter's plans.
fn judge(wrong: &mut Vec<String>, out: &str, id: &str, entry: &Recorded, present: bool) {
    let Some(said) = said_about(out, id) else {
        wrong.push(format!("{id}: forget said nothing about it:\n{out}"));
        return;
    };
    for path in stays_at(&said) {
        if !path.exists() {
            wrong.push(format!(
                "{id}: says its plan stays at {}, which does not exist\n    said: {said}",
                path.display()
            ));
        }
    }
    let named = match (&entry.laid, present) {
        // A laid plan on disk is named by the file the workflow laid.
        (Some(_), true) => entry.held[0].clone(),
        // A laid plan that is gone is named by the record's own name.
        (Some(laid), false) => laid.clone(),
        // A recipe plan is its own path, there or not.
        (None, _) => entry.own.clone(),
    };
    if present {
        if !said.contains(&format!("stays at {}", shown(&named))) {
            wrong.push(format!(
                "{id}: does not say its plan stays at {}, where it is\n    said: {said}",
                shown(&named)
            ));
        }
        if said.contains("gone") {
            wrong.push(format!(
                "{id}: says a plan is gone, but it is on disk\n    said: {said}"
            ));
        }
    } else {
        if !said.contains("already gone") || !said.contains(&shown(&named)) {
            wrong.push(format!(
                "{id}: does not say its plan was already gone, naming {}\n    said: {said}",
                shown(&named)
            ));
        }
        if said.contains("stays at") {
            wrong.push(format!(
                "{id}: says something stays, but nothing of it is on disk\n    said: {said}"
            ));
        }
    }
    if entry.laid.is_some() && said.contains(&shown(&entry.own)) {
        wrong.push(format!(
            "{id}: names {}, the entry's own path, which ephor never wrote\n    said: {said}",
            shown(&entry.own)
        ));
    }
}

/// The report itself, through the three ways an entry is reached: `--item` on a
/// gone plan, the `--missing` sweep ephor recommends for exactly these entries,
/// and `--item` on the two whose plans are on disk (§FS-005-dispatch.35.1).
#[test]
fn the_forget_report_says_which_plans_stay_and_which_were_already_gone() {
    let (world, recorded) = site();
    let mut wrong = Vec::new();
    let mut outputs = Vec::new();

    let out = forget_prose(&world, &["--item", LAID_GONE]);
    judge(&mut wrong, &out, LAID_GONE, &recorded[LAID_GONE], false);
    outputs.push(out);

    let out = forget_prose(&world, &["--missing"]);
    for control in [LAID_KEPT, RECIPE_KEPT] {
        assert!(
            !out.contains(control),
            "--missing reached {control}, whose plan is on disk:\n{out}"
        );
    }
    judge(&mut wrong, &out, LAID_SWEPT, &recorded[LAID_SWEPT], false);
    judge(&mut wrong, &out, RECIPE_GONE, &recorded[RECIPE_GONE], false);
    outputs.push(out);

    for id in [LAID_KEPT, RECIPE_KEPT] {
        let out = forget_prose(&world, &["--item", id]);
        judge(&mut wrong, &out, id, &recorded[id], true);
        outputs.push(out);
    }

    assert!(
        wrong.is_empty(),
        "work forget says something about a plan that the disk contradicts:\n{}\n\nwhat it printed:\n{}",
        wrong.join("\n"),
        outputs.concat()
    );
}

/// The machine-readable report carries the same facts the prose does
/// (§REQ-002-parity.3): `item` and `plan` exactly as they were, and `plans`,
/// one `{plan, missing}` per plan the prose names, `missing` true exactly where
/// the prose says the plan was already gone (§FS-005-dispatch.35.1).
#[test]
fn the_forget_json_carries_the_plans_and_whether_each_is_still_there() {
    let (world, recorded) = site();
    let mut wrong = Vec::new();
    let mut readings = Vec::new();

    let selections: [(&[&str], &[&str]); 4] = [
        (&["--item", LAID_GONE], &[LAID_GONE]),
        (&["--missing"], &[LAID_SWEPT, RECIPE_GONE]),
        (&["--item", LAID_KEPT], &[LAID_KEPT]),
        (&["--item", RECIPE_KEPT], &[RECIPE_KEPT]),
    ];
    for (args, ids) in selections {
        let (reading, prose) = forget_both(&world, args);
        let forgot = reading["forgot"]
            .as_array()
            .expect("forgot is an array")
            .clone();
        assert_eq!(forgot.len(), ids.len(), "forget {args:?} dropped {reading}");
        for id in ids {
            let entry = &recorded[id];
            let item = forgot
                .iter()
                .find(|item| item["item"] == json!(id))
                .unwrap_or_else(|| panic!("forget {args:?} did not report {id}: {reading}"));
            // The contract a reader already took stays as it was.
            if item["plan"] != json!(shown(&entry.own)) {
                wrong.push(format!(
                    "{id}: `plan` is no longer the entry's own path: {item}"
                ));
            }
            let present = [LAID_KEPT, RECIPE_KEPT].contains(id);
            let named = match (&entry.laid, present) {
                (Some(_), true) => entry.held[0].clone(),
                (Some(laid), false) => laid.clone(),
                (None, _) => entry.own.clone(),
            };
            let expected = json!([{ "plan": shown(&named), "missing": !present }]);
            if item["plans"] != expected {
                wrong.push(format!(
                    "{id}: `plans` is {}, expected {expected}",
                    item.get("plans")
                        .map_or("absent".to_string(), Value::to_string)
                ));
            }
            // And the prose says what the reading says, plan by plan.
            let said = said_about(&prose, id).unwrap_or_default();
            for plan in item["plans"].as_array().into_iter().flatten() {
                let path = plan["plan"].as_str().unwrap_or_default();
                let agrees = match plan["missing"].as_bool() {
                    Some(false) => said.contains(&format!("stays at {path}")),
                    Some(true) => said.contains("already gone") && said.contains(path),
                    None => false,
                };
                if !agrees {
                    wrong.push(format!(
                        "{id}: the prose does not say what {plan} says\n    said: {said}"
                    ));
                }
            }
        }
        readings.push(serde_json::to_string_pretty(&reading).unwrap_or_default());
    }

    assert!(
        wrong.is_empty(),
        "work forget --json does not carry what the prose must say:\n{}\n\nwhat it printed:\n{}",
        wrong.join("\n"),
        readings.join("\n")
    );
}

/// The work view's heading is the same naming (§FS-005-dispatch.35.1): it names
/// the plans the reading placed, says so where one is gone or could not be
/// read, and never names the entry's own path in their place. Its JSON already
/// carries `plans` and `missing` and does not change.
#[test]
fn the_work_view_names_the_plans_its_reading_holds() {
    let (world, recorded) = site();
    let mut wrong = Vec::new();
    let mut outputs = Vec::new();

    for id in LAID.into_iter().chain(RECIPE) {
        let entry = &recorded[id];
        let output = world
            .ephor()
            .args(["work", "offers", "--item", id])
            .output()
            .expect("work offers runs");
        assert!(output.status.success(), "work offers --item {id} failed");
        let out = String::from_utf8_lossy(&output.stdout).into_owned();
        // The heading: everything above the offers themselves.
        let heading = out
            .split("what could be asked for")
            .next()
            .unwrap_or_default();
        let gone = heading.contains("gone");
        let unread = heading.contains("could not be read");
        match id {
            LAID_KEPT => {
                if !heading.contains(&shown(&entry.held[0])) {
                    wrong.push(format!(
                        "{id}: the heading does not name the plan its workflow laid, {}",
                        shown(&entry.held[0])
                    ));
                }
                if gone || unread {
                    wrong.push(format!("{id}: the heading says a plan on disk is gone"));
                }
            }
            LAID_GONE | LAID_SWEPT => {
                if !unread {
                    wrong.push(format!(
                        "{id}: the heading does not say a plan the record names could not be read"
                    ));
                }
            }
            RECIPE_GONE => {
                if !heading.contains(&shown(&entry.own)) || !gone {
                    wrong.push(format!(
                        "{id}: the heading does not name its recipe plan as gone"
                    ));
                }
            }
            _ => {
                if !heading.contains(&shown(&entry.own)) || gone || unread {
                    wrong.push(format!(
                        "{id}: the heading does not name its recipe plan as there"
                    ));
                }
            }
        }
        if entry.laid.is_some() && heading.contains(&shown(&entry.own)) {
            wrong.push(format!(
                "{id}: the heading names {}, the entry's own path, which ephor never wrote",
                shown(&entry.own)
            ));
        }
        // The view's machine-readable form is not what changes.
        let reading = shaped(
            "work",
            &world
                .ephor()
                .args(["work", "offers", "--item", id, "--json"])
                .output()
                .expect("work offers --json runs"),
        );
        assert_eq!(reading["status"]["plan"], json!(shown(&entry.own)), "{id}");
        outputs.push(out);
    }

    assert!(
        wrong.is_empty(),
        "the work view names something other than the plans its reading holds:\n{}\n\nwhat it printed:\n{}",
        wrong.join("\n"),
        outputs.join("\n")
    );
}
