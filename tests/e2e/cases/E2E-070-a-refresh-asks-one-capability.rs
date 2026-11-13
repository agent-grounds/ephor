//! E2E-070-a-refresh-asks-one-capability: a ticket registered a moment ago is
//! one `issues` call away from the feed (§FS-011-command-line.9.2).
//!
//! The scenario is issue #263's. A project watches `gdev`, a forge extension
//! answering for both its pull requests and its tickets through two
//! configuration blocks, and `acme`, a second forge that reports pull requests
//! only. A branch is registered with the ticket `GR-45248`, and the next step is
//! to lay work on it. Before this change the only way to bring the ticket into
//! the feed was a refresh of the whole project — every pull request with its
//! threads and gate, every branch gate, every notice, from every source — and
//! `work lay` refused the item until all of it had answered
//! (§FS-001-forge-interface.8).
//!
//! What this case pins:
//!
//! - a plain refresh asks and writes exactly what it did before
//!   (`plain_refresh_is_unchanged`, which passes before the change and must
//!   stay green after it);
//! - `refresh --source gdev --only issues` asks gdev's `issues` alone, leaves
//!   every other answer as it was, and does not make the project read fresh
//!   (§FS-001-forge-interface.6.1, §FS-011-command-line.12), so an unheld
//!   rebase sweep still refetches it before it reads (§FS-004-quick-actions.6.1);
//! - a narrowed refresh over identical answers yields identical matters, so
//!   work does not move (§FS-005-dispatch.5);
//! - a history never asked is not an empty answer: matters it would have
//!   reported are not gone, and read marks are not pruned for them, judged from
//!   the project's bindings rather than from what the cache holds;
//! - every refusal of the new flags is decided before any data call.
//!
//! Every forge call lands in one log (`tests/e2e/fixtures/capability-forge.py`),
//! so "what was asked" is read off the log rather than inferred from the feed.

#[path = "../support.rs"]
mod support;

use std::process::Output;

use serde_json::{json, Value};

use support::*;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/e2e/fixtures/capability-forge.py"
);
const GDEV_PR: &str = "gdev:G/7";
const ORG: &str = "foundation";

/// The two forges as stubs on `PATH`, each handing its calls to the fixture
/// with its own name, the shared log and the shared control file.
fn install_forges(world: &World) {
    for source in ["gdev", "acme"] {
        world.stub(
            &format!("ephor-forge-{source}"),
            &format!(
                "#!/usr/bin/env bash\nexec python3 '{FIXTURE}' {source} '{}' '{}' \"$@\"\n",
                log_path(world).display(),
                world.path().join("control.json").display(),
            ),
        );
    }
}

/// gdev twice — two blocks that report the same pull request — and acme once,
/// all bound to the project.
fn bound_blocks() -> Value {
    json!([
        { "provider": "gdev", "block": "a", "repos": ["G"] },
        { "provider": "gdev", "block": "b", "repos": ["G"] },
        { "provider": "acme", "repos": ["app"] }
    ])
}

/// The project in an organization, so the rebase sweep can be pointed at it,
/// watching gdev and acme under `ttl` seconds.
fn watching(ttl: u64) -> World {
    let world = World::new();
    world.organize(ORG, "Foundation");
    install_forges(&world);
    world.configure(json!({
        "defaults": { "ttl_seconds": ttl },
        "projects": { PROJECT: { "providers": bound_blocks() } },
        "work": { "recipes": [{
            "id": "answer",
            "description": "answer the review",
            "when": { "kinds": ["pr"] },
            "needs_checkout": false,
            "state": "fix",
            "brief": "Answer {title}."
        }] }
    }));
    world
}

fn log_path(world: &World) -> std::path::PathBuf {
    world.path().join("forge.log")
}

/// Every forge call since the log was last cleared, one line each:
/// `<source> <block> <subcommand> [tickets]`.
fn calls(world: &World) -> Vec<String> {
    std::fs::read_to_string(log_path(world))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn clear(world: &World) {
    std::fs::write(log_path(world), "").expect("the call log clears");
}

fn fail(world: &World, sources: &[&str]) {
    write_json(
        &world.path().join("control.json"),
        &json!({ "fail": sources }),
    );
}

/// The subcommands one block asked, in the order it asked them.
fn asked_by(calls: &[String], source: &str, block: &str) -> Vec<String> {
    calls
        .iter()
        .filter_map(|line| {
            let mut words = line.split(' ');
            (words.next() == Some(source) && words.next() == Some(block))
                .then(|| words.next().unwrap_or("").to_string())
        })
        .collect()
}

fn run(world: &World, args: &[&str]) -> Output {
    world.ephor_raw().args(args).output().expect("ephor runs")
}

fn said(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The feed's rows for the project, as a program reads them.
fn rows(world: &World) -> Vec<Value> {
    let output = run(world, &["feed", "--workspace", PROJECT, "--json"]);
    assert!(output.status.success(), "{}", said(&output));
    json_of(&output)
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("feed --json is a list: {}", said(&output)))
}

fn ids(rows: &[Value]) -> Vec<String> {
    let mut ids: Vec<String> = rows
        .iter()
        .map(|row| row["id"].as_str().unwrap_or("").to_string())
        .collect();
    ids.sort();
    ids
}

/// A plain refresh asks what it asked before #263, in the same order per
/// block, writes the same rows, prints the same JSON fields and exits the same
/// — and a `status` inside the TTL asks nothing, failing source and all
/// (§FS-011-command-line.9.2: "a plain `ephor refresh` … asks and writes
/// exactly what it did before"). This passes before the change and is the
/// compatibility the change must keep.
#[test]
fn plain_refresh_is_unchanged() {
    let world = watching(600);
    fail(&world, &["acme"]);

    let output = run(&world, &["refresh", PROJECT, "--json"]);
    assert_eq!(output.status.code(), Some(4), "{}", said(&output));

    // The calls: each gdev block asks every capability gdev declares, in the
    // forge interface's order; acme stops at its first failure. Blocks run
    // concurrently, so the order is pinned per block and the whole as a set.
    let made = calls(&world);
    for block in ["a", "b"] {
        assert_eq!(
            asked_by(&made, "gdev", block),
            ["pull-requests", "branch-gates", "issues", "notices"],
            "gdev block {block}: {made:#?}"
        );
    }
    assert_eq!(
        asked_by(&made, "acme", "-"),
        ["capabilities", "pull-requests"]
    );
    assert_eq!(
        asked_by(&made, "gdev", "-"),
        ["capabilities", "capabilities"],
        "{made:#?}"
    );
    assert_eq!(made.len(), 12, "no call beyond these: {made:#?}");

    // The JSON: exactly today's fields, and no `answers` on a plain run
    // (§FS-011-command-line.7).
    let reading = json_of(&output);
    let keys = |value: &Value| -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .unwrap_or_else(|| panic!("an object: {value:#}"))
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    };
    assert_eq!(
        keys(&reading),
        ["degraded", "lost", "notes", "projects", "refreshed"],
        "{reading:#}"
    );
    let project = &reading["projects"][0];
    assert_eq!(
        keys(project),
        ["failures", "items", "lost", "notes", "project"],
        "{reading:#}"
    );
    assert_eq!(reading["refreshed"], 1, "{reading:#}");
    assert_eq!(reading["degraded"], 1, "{reading:#}");
    assert_eq!(project["project"], PROJECT, "{reading:#}");
    assert_eq!(project["items"], 1, "{reading:#}");
    let failures = project["failures"].as_array().expect("failures");
    assert_eq!(failures.len(), 1, "{reading:#}");
    assert!(failures[0].as_str().unwrap_or("").contains("acme"));

    // The rows: the two blocks' reports of one pull request are one row, and
    // the first block's report wins the tie: a slot is read by source, then
    // block, then capability, whichever run wrote it (§FS-001-forge-interface.6.1).
    let feed = rows(&world);
    assert_eq!(ids(&feed), [GDEV_PR], "{feed:#?}");
    assert_eq!(feed[0]["title"], "Retry window (block a)", "{feed:#?}");

    // Inside the TTL, `status` reads the cache and asks nobody, the failing
    // source included (§FS-011-command-line.12).
    clear(&world);
    let status = run(&world, &["status", PROJECT]);
    assert!(status.status.success(), "{}", said(&status));
    assert!(calls(&world).is_empty(), "{:#?}", calls(&world));
}
