//! E2E-047-an-issue-nobody-holds-is-work: an unclaimed issue nobody asked anything on is laid as work, not as a reply.
//!
//! The scenario is a reader who watches their project's backlog: their source
//! counts an issue nobody has taken as waiting on them
//! (§FS-003-feed-categories.4), and only the recipes ephor ships are
//! configured. They filed an issue, said the last word in it themselves, and
//! nobody holds it. Nobody asked anything, so what the issue is owed is the
//! work it describes, and a plain dispatch lays the shipped issue recipe on it
//! rather than the answer recipe — and the ticket does not tell the agent that
//! somebody is waiting on a reply (§FS-005-dispatch.13.1).
//!
//! Two controls keep that from turning into "an unclaimed issue is never
//! answered". The same issue where somebody else asked a question, still held
//! by nobody, waits for both reasons and is still answered; and so is the
//! issue once somebody holds it (§FS-005-dispatch.31.2). The feed does not
//! move in any of them: the unclaimed issue still awaits the reader there
//! (§FS-003-feed-categories.4).

#[path = "../support.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use support::*;

/// The issue's id in the feed: the forge's name, then the key it reported.
const ITEM: &str = "acmeforge:app#378";

/// The reader's own word in the issue: what a lifecycle record comment, or a
/// note the author left for themselves, looks like to the forge.
fn mine() -> Value {
    json!({ "author": "you", "text": "Recorded: intake, reproduced, published.",
            "when": "2026-10-01T16:40:00Z", "mine": true })
}

/// Somebody else asking the reader something, after the reader spoke.
fn asked() -> Value {
    json!({ "author": "Ada", "text": "Which chapter should the rule live in?",
            "when": "2026-10-01T17:00:00Z", "mine": false })
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

/// The project with its main branch checked out at `<root>/main` and one
/// directory per branch beside it, so the issue recipe has somewhere to put
/// the branch it works on.
fn project_with_main_checked_out(world: &World) {
    let origin = world.path().join("origin");
    std::fs::create_dir_all(&origin).expect("the remote");
    git(&origin, &["init", "-q", "--initial-branch=main"]);
    std::fs::write(origin.join("README.md"), "the project\n").expect("a file");
    git(&origin, &["add", "README.md"]);
    git(&origin, &["commit", "-q", "-m", "the project"]);
    let main = world.forest().join("main");
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(&origin)
        .arg(&main)
        .status()
        .expect("git clones");
    assert!(status.success());
    git(&main, &["config", "user.email", "t@example.com"]);
    git(&main, &["config", "user.name", "t"]);
    world.register(json!({
        "branches": [],
        "branch_root_template": "{project_root}/{branch}"
    }));
}

/// A forge with one issue the reader opened, held by `assignees` and carrying
/// `messages`. `assigned` follows the list, so an empty one is the forge
/// saying nobody has it (§FS-001-forge-interface.1).
fn watching(assignees: &[&str], messages: Vec<Value>) -> World {
    let issue = json!([{
        "key": "app#378",
        "title": "A reader-facing rule cannot say which chapter it belongs to",
        "status": "open",
        "url": "https://acme.example/issues/378",
        "updated_at": "2026-10-01T17:00:00Z",
        "role": "author",
        "assigned": !assignees.is_empty(),
        "assignees": assignees,
        "labels": ["feature"],
        "messages": messages,
    }]);
    let forge = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${{1:?subcommand}}" in
  capabilities) printf '{{"issues":true}}' ;;
  issues) cat <<'JSON'
{issue}
JSON
    ;;
  *) printf '[]' ;;
esac
"#
    );
    let world = World::new();
    project_with_main_checked_out(&world);
    world.stub("ephor-forge-acmeforge", &forge);
    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "acmeforge", "user": "you", "repos": ["app"], "unclaimed": true }
        ] } },
        "work": { "runner": "acme-runtime" }
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

/// What the feed says of the issue: whether it awaits the reader.
fn needs_response(world: &World) -> bool {
    let listed = world
        .ephor()
        .args(["feed", "--json"])
        .output()
        .expect("the feed lists");
    let feed = json_of(&listed);
    let entry = feed
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["id"] == ITEM))
        .unwrap_or_else(|| panic!("{ITEM} is not in the feed: {feed:#}"));
    entry["needs_response"]
        .as_bool()
        .expect("needs_response is a boolean")
}

/// The reading `ephor work offers` returns for the issue.
fn offers(world: &World) -> Value {
    let read = world
        .ephor()
        .args(["work", "offers", "--item", ITEM, "--json"])
        .output()
        .expect("the offers read");
    json_of(&read)
}

fn offered(reading: &Value) -> Vec<String> {
    reading["offers"]
        .as_array()
        .map(|offers| {
            offers
                .iter()
                .filter_map(|offer| offer["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// A plain dispatch on the issue, and the plan it laid — wherever the recipe
/// put it, the project's work root or the workspace of the branch it minted.
fn dispatched(world: &World) -> String {
    world
        .ephor()
        .args(["work", "dispatch", "--item", ITEM])
        .assert()
        .success();
    let plans = plans_under(&world.forest());
    assert_eq!(plans.len(), 1, "one plan, laid once: {plans:?}");
    std::fs::read_to_string(&plans[0]).expect("the plan reads")
}

fn plans_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        if path.is_dir() {
            found.extend(plans_under(&path));
        } else if path.to_string_lossy().ends_with(".rhei.md") {
            found.push(path);
        }
    }
    found
}

/// The recipe each ticket in the plan was laid from, in order.
fn tickets(plan: &str) -> Vec<String> {
    plan.lines()
        .filter_map(|line| line.strip_prefix("### Task "))
        .filter_map(|rest| rest.split(':').next())
        .map(|task| task.rsplit_once('-').map_or(task, |(recipe, _)| recipe))
        .map(str::to_string)
        .collect()
}

/// The reader's own issue, held by nobody, whose last word is theirs: nobody
/// asked anything, so it is work for the issue recipe (§FS-005-dispatch.13.1).
#[test]
fn an_unclaimed_issue_nobody_asked_anything_on_is_laid_as_work() {
    let world = watching(&[], vec![mine()]);

    // The feed is unchanged: nobody holds it, so it still awaits the reader
    // (§FS-003-feed-categories.4).
    assert!(needs_response(&world), "the unclaimed issue stops awaiting");

    let reading = offers(&world);
    let ids = offered(&reading);
    assert!(
        ids.contains(&"implement".to_string()),
        "the issue recipe is not offered: {reading:#}"
    );
    assert!(
        !ids.contains(&"answer".to_string()),
        "`answer` is offered on an issue nobody asked anything on: {reading:#}"
    );
    // And the refusal says why: the reading names `answer` beside the reason
    // it was refused (§FS-005-dispatch.27, §FS-005-dispatch.31.2).
    let refused = reading["excluded"]
        .as_array()
        .and_then(|excluded| excluded.iter().find(|entry| entry["recipe"] == "answer"))
        .unwrap_or_else(|| panic!("`answer` is not named as refused: {reading:#}"));
    let reason = refused["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("waits only because nobody holds it"),
        "the refusal does not say why: {reason}"
    );

    let plan = dispatched(&world);
    assert_eq!(tickets(&plan), ["implement"], "{plan}");
    assert!(
        !plan.contains("an answer from me"),
        "the plan tells the agent a reply is owed: {plan}"
    );
    // The dossier names the reason it does wait (§FS-005-dispatch.13.1).
    assert!(plan.contains("nobody holds it"), "{plan}");
}

/// Held by nobody, and somebody asked a question: it waits for both reasons,
/// and the conversation is still answered (§FS-005-dispatch.13.1).
#[test]
fn an_unclaimed_issue_with_a_question_in_it_is_still_answered() {
    let world = watching(&[], vec![mine(), asked()]);
    assert!(needs_response(&world));

    let reading = offers(&world);
    assert!(
        offered(&reading).contains(&"answer".to_string()),
        "{reading:#}"
    );
    let plan = dispatched(&world);
    assert_eq!(tickets(&plan), ["answer"], "{plan}");
    assert!(plan.contains("an answer from me"), "{plan}");
}

/// Somebody holds it and somebody asked a question: an ordinary conversation
/// awaiting the reader, answered as it always was.
#[test]
fn a_held_issue_with_a_question_in_it_is_answered() {
    let world = watching(&["you"], vec![mine(), asked()]);
    assert!(needs_response(&world));

    let reading = offers(&world);
    assert!(
        offered(&reading).contains(&"answer".to_string()),
        "{reading:#}"
    );
    let plan = dispatched(&world);
    assert_eq!(tickets(&plan), ["answer"], "{plan}");
    assert!(plan.contains("an answer from me"), "{plan}");
}

/// The control: the same issue once somebody holds it, nobody having asked
/// anything. It waits on nobody, so only the issue recipe applies — which is
/// also the proof that this world can lay it at all.
#[test]
fn a_held_issue_nobody_asked_anything_on_is_laid_as_work() {
    let world = watching(&["you"], vec![mine()]);
    assert!(!needs_response(&world));

    let reading = offers(&world);
    assert_eq!(offered(&reading), ["implement"], "{reading:#}");
    let plan = dispatched(&world);
    assert_eq!(tickets(&plan), ["implement"], "{plan}");
    assert!(!plan.contains("an answer from me"), "{plan}");
}
