//! E2E-064-a-provider-failure-names-the-command: a spent GraphQL budget
//! reports the command and diagnosis without repeating the query document
//! (§FS-001-forge-interface.6). Port of the issue #250 four-source reproducer:
//! fresh workspace status, cached project status, then the stored error in JSON.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};
use std::fs;
use std::process::Output;
use support::*;

const REASON: &str = "API rate limit already exceeded for user ID 352820.";
const SOURCES: [&str; 4] = ["github-ci", "github-issues", "github-prs", "github-threads"];

// REST still answers; GraphQL and its porcelain wrappers refuse exactly as
// gh does. Log the full request only in the fixture, to prove it contained
// aliased searches and to detect any forge call from a cached reading.
const GH: &str = r#"#!/usr/bin/env bash
set -euo pipefail
[ "$#" -gt 0 ] || exit 0
printf '%s\n' "$*" >> "$HOME/gh.calls"
LIMIT='API rate limit already exceeded for user ID 352820.'
case "${1:-}" in --version) echo 'gh version 2.83.0 (stub)'; exit 0 ;; esac
if [ "${1:-}" = api ]; then
  case "${2:-}" in
    graphql)
      printf '{"errors":[{"type":"RATE_LIMITED","code":"graphql_rate_limit","message":"%s"}]}\n' "$LIMIT"
      printf 'gh: %s\n' "$LIMIT" >&2
      exit 1 ;;
    user) echo repro; exit 0 ;;
    user/teams) exit 0 ;;
    repos/*) echo '{"total_count":0,"workflow_runs":[]}'; exit 0 ;;
  esac
fi
printf 'GraphQL: %s\n' "$LIMIT" >&2
exit 1
"#;

fn watching() -> World {
    let world = World::new();
    let gh = world.stub("gh", GH);
    settle(&gh);
    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            {"provider": "github-prs", "repos": ["demo/javelin"], "reviews": true},
            {"provider": "github-ci", "repos": ["demo/javelin"]},
            {"provider": "github-issues", "repos": ["demo/javelin"],
             "authored": true, "participating": true, "updated_within_days": 0,
             "limit": 1000, "comments": true, "unclaimed": true,
             "labels": ["state:triage", "priority", "high-priority", "bug",
                        "enhancement", "usability", "tokens", "flaky-test"]},
            {"provider": "github-threads", "repos": ["demo/javelin"]}
        ] } }
    }));
    world
}

fn fresh(world: &World) -> Output {
    let output = world
        .ephor()
        .args(["status", "--workspace", PROJECT])
        .output()
        .expect("workspace status runs");
    // Status readings retain their existing exit policy; refresh has its own.
    assert!(output.status.success(), "{output:?}");
    let calls = world.read("gh.calls");
    for fragment in [
        "api graphql",
        "query=",
        "q0:search(",
        "q9:search(",
        "r0:search(",
    ] {
        assert!(
            calls.contains(fragment),
            "fixture did not exercise {fragment}"
        );
    }
    output
}

fn no_payload(text: &str, reading: &str) {
    for fragment in ["query=", ":search(", "pageInfo{", "nodes{", "-f "] {
        assert!(
            !text.contains(fragment),
            "{reading} repeats invocation payload ({fragment})"
        );
    }
}

fn short_errors(output: &Output, cached: bool) {
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Assert the useful content before its size, so silence cannot pass.
    for source in SOURCES {
        let prefix = if cached {
            format!("[{source}]")
        } else {
            format!("error: {PROJECT}: {source}:")
        };
        let lines: Vec<_> = stderr
            .lines()
            .filter(|line| line.contains(&prefix))
            .collect();
        assert_eq!(lines.len(), 1, "missing or repeated source {prefix}");
        assert!(lines[0].contains(REASON), "{source} lost the diagnosis");
        assert!(
            lines[0].contains("failed (exit status: 1)"),
            "{source} lost failure information"
        );
        if source == "github-issues" || source == "github-prs" {
            assert!(
                lines[0].contains("gh api graphql"),
                "{source} lost command identity"
            );
        }
    }
    if cached {
        assert!(String::from_utf8_lossy(&output.stdout).contains(PROJECT));
        assert!(
            stderr.contains("NO DATA"),
            "an unfetched source must remain visible"
        );
    }
    no_payload(
        &stderr,
        if cached {
            "cached status"
        } else {
            "fresh status"
        },
    );
    assert!(
        stderr.lines().all(|line| line.len() <= 300),
        "diagnostic line exceeds 300 bytes"
    );
    assert!(
        output.stderr.len() <= 1200,
        "stderr exceeds 1,200 bytes: {}",
        output.stderr.len()
    );
}

#[test]
#[ignore = "red until #250: fresh status omits GraphQL invocation payloads; implement removes this"]
fn issue_250_fresh_status_names_each_source_without_its_query() {
    let world = watching();
    short_errors(&fresh(&world), false);
}

#[test]
#[ignore = "red until #250: cached status retains concise stored failures; implement removes this"]
fn issue_250_cached_status_keeps_the_diagnosis_without_fetching() {
    let world = watching();
    fresh(&world);
    let before = world.read("gh.calls");
    let output = world
        .ephor()
        .args(["status", "--cached", PROJECT])
        .output()
        .expect("cached status runs");
    assert_eq!(
        world.read("gh.calls"),
        before,
        "cached status called the forge"
    );
    short_errors(&output, true);
}

#[test]
#[ignore = "red until #250: JSON stores command identity without query data; implement removes this"]
fn issue_250_json_preserves_failed_slots_without_invocation_payloads() {
    let world = watching();
    fresh(&world);
    let before = world.read("gh.calls");
    let output = world
        .ephor()
        .args(["status", "--json", PROJECT])
        .output()
        .expect("JSON status runs");
    assert!(output.status.success(), "{output:?}");
    assert!(
        output.stderr.is_empty(),
        "cached JSON gains no extra diagnostic rendering"
    );
    assert_eq!(
        world.read("gh.calls"),
        before,
        "fresh JSON cache called the forge"
    );
    let reading: Value = serde_json::from_slice(&output.stdout).expect("status emits JSON");
    let rows = reading.as_array().expect("existing array shape");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["project"], PROJECT);
    let stored = world.feed();
    assert_eq!(
        rows[0]["providers"], stored["providers"],
        "JSON exposes the stored slots"
    );
    let slots = rows[0]["providers"].as_object().expect("provider map");
    assert_eq!(slots.len(), 4);
    for source in SOURCES {
        let slot = &slots[source];
        assert_eq!(slot["ok"], false, "{source} must remain failed");
        assert_eq!(slot["stale"], false, "{source} has no last-good data yet");
        assert_eq!(slot["matters"], json!([]));
        assert!(
            slot.get("unreachable").is_none(),
            "rate limits keep their existing classification"
        );
        let error = slot["error"].as_str().expect("a stored diagnosis");
        assert!(error.contains(REASON), "{source} lost its diagnosis");
        assert!(error.contains("failed (exit status: 1)"));
    }
    no_payload(&String::from_utf8_lossy(&output.stdout), "JSON status");
    no_payload(
        &fs::read_to_string(world.path().join("state/ephor/feed/demo.json")).expect("stored cache"),
        "stored cache",
    );
}
