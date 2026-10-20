//! E2E-062-a-cache-past-its-ttl-is-named: `ephor feed` says which project it
//! read from a cache past its TTL, and which it has never fetched.
//!
//! The scenario is the one the issue reports: a project's feed was refreshed
//! days ago, the TTL is ten minutes, and `ephor feed` prints the cached rows
//! exactly as it would print a current reading — so a matter that arrived since
//! the last refresh is absent without a word. Every cached row is still
//! printed, and standard output and the `--json` document stay what they were;
//! what changes is one line on the error stream naming the project, its age,
//! the TTL it is past and the `ephor refresh` that cures it
//! (§FS-011-command-line.12). A project never fetched is named the same way,
//! and a cache younger than its TTL draws no line.

#[path = "../support.rs"]
mod support;

use chrono::{Duration, SecondsFormat, Utc};
use serde_json::{json, Value};
use std::fs;

use support::*;

const TITLE: &str = "cached fixture item";

const STATUS_REPORTER: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cp "$HOME/status-answer.json" "$EPHOR_ANSWER"
"#;

/// A world watching one project through a custom-status source that reports
/// one open matter, with the ten-minute TTL the world configures.
fn watching() -> World {
    let world = World::new();
    world.stub("status-reporter", STATUS_REPORTER);
    fs::write(
        world.path().join("status-answer.json"),
        serde_json::to_string_pretty(&json!({
            "v": 1,
            "matters": [{
                "key": "fixture",
                "kind": "status",
                "title": TITLE,
                "state": "open",
                "terminal": false,
                "time": "2026-09-01T00:00:00Z"
            }]
        }))
        .expect("the status answer serializes"),
    )
    .expect("write the status answer");
    world.configure(json!({
        "defaults": { "ttl_seconds": 600 },
        "projects": { PROJECT: { "providers": [{
            "provider": "custom-status",
            "command": "status-reporter",
            "format": "answer"
        }] } }
    }));
    world
}

fn refreshed() -> World {
    let world = watching();
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

/// Rewrite the cache's `fetched_at` to five days ago, as if nobody had
/// refreshed the project since.
fn aged(world: &World) {
    let path = world
        .path()
        .join("state/ephor/feed")
        .join(format!("{PROJECT}.json"));
    let mut feed = read_json(&path);
    feed["fetched_at"] =
        json!((Utc::now() - Duration::days(5)).to_rfc3339_opts(SecondsFormat::Secs, true));
    write_json(&path, &feed);
}

fn feed(world: &World, json: bool) -> std::process::Output {
    let mut args = vec!["feed"];
    if json {
        args.push("--json");
    }
    let output = world.ephor().args(&args).output().expect("ephor feed runs");
    assert!(
        output.status.success(),
        "feed exits as it always did: {:?}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
#[ignore = "red until #225: feed does not say a cache is past its TTL; implement removes this"]
fn a_cache_past_its_ttl_is_named_and_its_rows_still_printed() {
    let world = refreshed();
    aged(&world);

    let output = feed(&world, false);
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    assert!(
        stdout.contains(TITLE),
        "the cached row is still printed on stdout:\n{stdout}"
    );
    for part in [PROJECT, "5d", "10m", "TTL", "ephor refresh demo"] {
        assert!(
            stderr.contains(part),
            "the error stream names the stale cache ({part:?} missing)\nstderr:\n{stderr}\nstdout:\n{stdout}"
        );
    }
    assert!(
        !stdout.contains("TTL"),
        "the notice is on the error stream, not among the rows:\n{stdout}"
    );

    // The same notice under `--json`, with the document left to the program.
    let output = feed(&world, true);
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("ephor refresh demo") && stderr.contains("TTL"),
        "--json names the stale cache on the error stream too:\n{stderr}"
    );
    let items = json_of(&output);
    assert!(
        items
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["title"] == TITLE)),
        "the --json document still carries the cached row: {items}"
    );
}

#[test]
#[ignore = "red until #225: feed does not say a project was never fetched; implement removes this"]
fn a_project_never_fetched_is_named() {
    let world = watching();

    let output = feed(&world, false);
    let stderr = text(&output.stderr);
    for part in [PROJECT, "never fetched", "ephor refresh demo"] {
        assert!(
            stderr.contains(part),
            "the error stream names the project with no cache ({part:?} missing):\n{stderr}"
        );
    }
}

#[test]
fn a_fresh_cache_draws_no_line() {
    let world = refreshed();

    let output = feed(&world, false);
    let stderr = text(&output.stderr);
    assert!(
        text(&output.stdout).contains(TITLE),
        "the fresh row is printed"
    );
    assert!(
        !stderr.contains("TTL") && !stderr.contains("ephor refresh"),
        "a cache younger than its TTL draws no line:\n{stderr}"
    );
}

#[test]
fn the_json_document_does_not_change_with_the_cache_age() {
    let world = refreshed();
    let fresh: Value = json_of(&feed(&world, true));
    aged(&world);
    let stale: Value = json_of(&feed(&world, true));
    assert_eq!(
        fresh, stale,
        "feed --json prints the same document whatever the cache's age"
    );
}
