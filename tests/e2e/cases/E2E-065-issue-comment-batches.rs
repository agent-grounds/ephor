//! §FS-001-forge-interface.8.3: distinct conversations cost bounded batches.
//! Port of #252's two-size cold/warm reproducer, with its original source and
//! forge. Payload/failure cases extend the same forge rather than replacing
//! the public refresh/feed path by a provider helper.

#[path = "../support.rs"]
mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use serde_json::{json, Value};
use support::*;

const FORGE: &str = include_str!("../fixtures/issue-comments/forge.py");
const BATCHES: &str = include_str!("../fixtures/issue-comments/batches.py");
const REPORT: &str = include_str!("../fixtures/issue-comments/report.json");

fn python() -> String {
    let out = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .expect("python3 for the offline forge");
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

struct Site {
    world: World,
    python: String,
    closed: usize,
    enriched: bool,
}

impl Site {
    fn new(closed: usize) -> Self {
        let world = World::new();
        fs::write(world.path().join("forge.py"), FORGE).unwrap();
        fs::write(world.path().join("batches.py"), BATCHES).unwrap();
        world.stub(
            "gh",
            "#!/bin/sh\nexec \"$REPRO_PY\" -I \"$COMMENT_FORGE\" \"$@\"\n",
        );
        let report: Value = serde_json::from_str(REPORT).unwrap();
        world.configure(json!({
            "defaults": {"provider_timeout_seconds": 30},
            "projects": {PROJECT: {"providers": report["projects"]["javelin"]["providers"]}}
        }));
        Self {
            world,
            python: python(),
            closed,
            enriched: false,
        }
    }

    fn command(&self) -> assert_cmd::Command {
        let mut cmd = self.world.ephor();
        for key in ["GH_HOST", "GH_TOKEN", "GITHUB_TOKEN"] {
            cmd.env_remove(key);
        }
        cmd.env("XDG_CONFIG_HOME", self.world.path().join("config"))
            .env("XDG_CACHE_HOME", self.world.path().join("cache"))
            .env("REPRO_PY", &self.python)
            .env(
                "COMMENT_FORGE",
                self.world.path().join(if self.enriched {
                    "batches.py"
                } else {
                    "forge.py"
                }),
            )
            .env("REPRO_CLOSED", self.closed.to_string())
            .env("REPRO_GH_LOG", self.log())
            .env("BATCH_FIXTURE", self.world.path().join("fixture.json"))
            .env("BATCH_LOG", self.log());
        cmd
    }

    fn log(&self) -> PathBuf {
        self.world.path().join("calls.log")
    }

    fn refresh(&self) -> String {
        fs::write(self.log(), "").unwrap();
        self.command()
            .args(["refresh", PROJECT, "--quiet"])
            .assert()
            .success();
        let calls = fs::read_to_string(self.log()).unwrap();
        assert!(!calls.is_empty(), "must actually ask the forge");
        assert!(
            !calls.contains(" unknown "),
            "unsupported fixture request: {calls}"
        );
        calls
    }

    fn rows(&self) -> Vec<Value> {
        let out = self
            .command()
            .args(["feed", "--workspace", PROJECT, "--json"])
            .assert()
            .success();
        serde_json::from_slice(&out.get_output().stdout).unwrap()
    }

    fn enriched(failure: &str, comments: bool) -> Self {
        let mut site = Self::new(0);
        site.enriched = true;
        let mut issues: Vec<Value> = (1..=45)
            .map(|n| {
                json!({
                "repo": "demo/javelin", "number": n, "state": if n == 44 { "CLOSED" } else { "OPEN" }, "author": "repro",
                    "days": 200, "talk": ["repro"], "count": 35
                })
            })
            .collect();
        issues.push(json!({"repo": "demo/library", "number": 1, "state": "OPEN",
            "author": "someone", "days": 220, "talk": ["repro"], "count": 2}));
        issues.push(
            json!({"repo": "demo/javelin", "number": 90, "state": "OPEN",
            "author": "repro", "days": 200, "talk": [], "count": 0}),
        );
        write_json(
            &site.world.path().join("fixture.json"),
            &json!({
                "issues": issues, "host": "forge.example.test", "failure": failure
            }),
        );
        site.world.configure(json!({
            "defaults": {"github_user": "repro"},
            "projects": {PROJECT: {"providers": [{
                "provider": "github-issues", "host": "forge.example.test",
                "repos": ["demo/javelin", "demo/library"], "open": true,
                "authored": true, "participating": true, "updated_within_days": 0,
                "comments": comments, "unclaimed": true, "limit": 1000,
                "labels": ["priority", "bug"]
            }]}}
        }));
        site
    }
}

fn messages(item: &Value) -> Vec<Value> {
    item["raw"]["threads"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|t| t["messages"].as_array().into_iter().flatten())
        .cloned()
        .collect()
}

fn row(rows: &[Value], number: usize) -> &Value {
    rows.iter()
        .find(|r| r["id"] == format!("github-issues:demo/javelin#{number}"))
        .unwrap_or_else(|| panic!("missing issue #{number}"))
}

fn retain_runtime_work(site: &Site) {
    let root = site.world.path().join("work");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("states.yaml"),
        "name: m\nstates:\n  fix:\n    agent: x\n  done:\n    final: true\n",
    )
    .unwrap();
    let plan = root.join("old.rhei.md");
    fs::write(&plan, "# Rhei: old issue\n**States:** m\n\n## Tasks\n\n### Task fix: open work\n**State:** fix\n\nwork\n").unwrap();
    fs::create_dir_all(site.world.path().join("state/ephor")).unwrap();
    write_json(
        &site.world.path().join("state/ephor/work.json"),
        &json!({
            "version": 1, "entries": {"github-issues:demo/javelin#100": {
                "project": PROJECT, "title": "old issue", "root": root,
                "plan_id": "old", "plan": plan, "dispatches": []
            }}
        }),
    );
    // The CLI feed has no runtime overlay; Session is the surface that owns
    // that overlay. Use real cached facts and a real open plan at this seam.
    let ledger: ephor::work::ledger::Ledger =
        serde_json::from_value(read_json(&site.world.path().join("state/ephor/work.json")))
            .unwrap();
    let feed: ephor::feed::cache::ProjectFeed = serde_json::from_value(site.world.feed()).unwrap();
    let item = feed
        .items()
        .find(|i| i.id == "github-issues:demo/javelin#100")
        .unwrap();
    let mut session = ephor::api::session::Session::default();
    assert!(!session.shows(&item, chrono::Utc::now()));
    let status = ephor::work::status_of_entry(
        &ephor::work::recipe::WorkConfig::default(),
        &ledger.entries[&item.id],
        Some(&item),
    );
    session.work.insert(item.id.clone(), status.lines(60));
    assert!(
        session.working(&item.id),
        "the fixture plan must hold work open"
    );
    assert!(session.shows(&item, chrono::Utc::now()));
    assert_eq!(messages(&serde_json::to_value(&item).unwrap()).len(), 2);
}

/// §FS-001-forge-interface.8.3, §FS-003-feed-categories.2: the original source
/// keeps quiet open conversations and recent loose ends at a bounded cost.
#[test]
fn issue_comments_closed_history_has_bounded_cold_and_warm_cost() {
    let mut warm = Vec::new();
    for size in [40, 240] {
        let site = Site::new(size);
        for turn in ["cold", "warm"] {
            let calls = site.refresh();
            let count = calls.lines().count();
            let rows = site.rows();
            for n in [1, 2, 3, 4] {
                row(&rows, n);
            }
            assert_eq!(messages(row(&rows, 1)).len(), 2);
            assert_eq!(messages(row(&rows, 1))[1]["text"], "comment 1 on #1");
            assert_eq!(messages(row(&rows, 2)).len(), 1);
            assert_eq!(messages(row(&rows, 2))[0]["text"], "comment 0 on #2");
            assert!(messages(row(&rows, 3)).is_empty());
            let recent = row(&rows, 4);
            assert_eq!(recent["state"], "closed");
            assert_eq!(recent["needs_response"], false);
            assert_eq!(messages(recent).last().unwrap()["author"], "someone");
            assert!(!rows
                .iter()
                .any(|r| r["id"] == "github-issues:demo/javelin#100"));
            assert!(calls
                .lines()
                .any(|l| l.split_whitespace().nth(2) == Some("search")));
            eprintln!("{size} closed issues {turn}: {count} gh calls");
            if turn == "warm" {
                warm.push(count);
            }
        }
        // The provider must still read old conversations: the runtime can
        // retain a finished matter long after the seven-day visibility window.
        retain_runtime_work(&site);
    }
    let slope = (warm[1] as f64 - warm[0] as f64) / 200.0;
    assert!(
        slope <= 0.25,
        "warm gh-call slope {slope:.2} exceeds 0.25: {} calls at 40, {} at 240 closed issues",
        warm[0],
        warm[1]
    );
}

fn batch_calls(log: &str) -> Vec<Value> {
    log.lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|v| v["kind"] == "comments")
        .collect()
}

fn assert_batches(log: &str) {
    let calls = batch_calls(log);
    assert_eq!(
        calls.len(),
        3,
        "46 distinct conversations require three batches of at most 20; got {} comment calls",
        calls.len()
    );
    let lengths: Vec<_> = calls
        .iter()
        .map(|v| v["keys"].as_array().unwrap().len())
        .collect();
    assert_eq!(
        lengths.iter().filter(|n| **n == 20).count(),
        2,
        "fill batches before starting the next: {lengths:?}"
    );
    assert!(lengths.iter().all(|n| *n <= 20));
    let keys: Vec<_> = calls
        .iter()
        .flat_map(|v| v["keys"].as_array().unwrap())
        .collect();
    assert_eq!(keys.len(), 46);
    assert_eq!(
        keys.iter()
            .map(|k| k.as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        46,
        "hydrate a canonical key only once"
    );
    assert!(!keys.iter().any(|k| **k == "demo/javelin#90"));
}

/// §FS-001-forge-interface.8.3: multiple batches preserve normalized payloads,
/// overlap precedence, configured host, and equal numbers in different repos.
#[test]
fn issue_comments_batches_preserve_payload_and_identity() {
    let site = Site::enriched("", true);
    let calls = site.refresh();
    let searches: Vec<Value> = calls
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|v| v["kind"] == "search")
        .collect();
    assert_eq!(searches.len(), 1);
    let questions = searches[0]["keys"].as_array().unwrap();
    assert_eq!(questions.len(), 3, "open coverage absorbs followed labels");
    for question in questions {
        let question = question.as_str().unwrap();
        assert!(question.contains("repo:demo/javelin") && question.contains("repo:demo/library"));
        assert!(!question.contains("updated:") && !question.contains("label:"));
    }
    let matters = site.world.matters();
    assert_eq!(matters.len(), 47);
    for n in 1..=45 {
        let matter = matters
            .iter()
            .find(|m| m["key"] == format!("github-issues:demo/javelin#{n}"))
            .unwrap();
        assert_eq!(matter["role"], "author");
        let talk = messages(matter);
        assert_eq!(talk.len(), 30);
        for (index, msg) in talk.iter().enumerate() {
            let i = index + 5;
            assert_eq!(msg["text"], format!("demo/javelin#{n} comment {i}"));
            assert_eq!(msg["mine"].as_bool().unwrap_or(false), i % 2 == 0);
            assert_eq!(msg["author"], if i % 2 == 0 { "repro" } else { "someone" });
            assert_eq!(msg["when"], "2001-01-01T01:02:03+00:00");
            assert_eq!(msg["react"]["host"], "forge.example.test");
            assert_eq!(
                msg["react"]["subject_id"],
                format!("IC_demo/javelin#{n}_{i}")
            );
            assert_eq!(msg["reactions"][0]["emoji"], "👍");
            assert_eq!(msg["reactions"][0]["users"], json!(["repro", "someone"]));
        }
    }
    let library = matters
        .iter()
        .find(|m| m["key"] == "github-issues:demo/library#1")
        .unwrap();
    assert_eq!(library["role"], "reviewer");
    assert_eq!(messages(library)[0]["text"], "demo/library#1 comment 0");
    // Controls: no comment requests at all when disabled, even with overlaps.
    let disabled = Site::enriched("", false);
    assert!(batch_calls(&disabled.refresh()).is_empty());
    assert_eq!(disabled.world.matters().len(), 47);
    assert!(disabled
        .world
        .matters()
        .iter()
        .all(|m| messages(m).is_empty()));
    assert_batches(&calls);
}

/// §FS-001-forge-interface.8.3: comment failures retain issues and available
/// sibling answers, with no request-by-request fallback on a refused batch.
#[test]
fn issue_comments_batch_failures_keep_issues_and_successful_siblings() {
    let mut logs = Vec::new();
    for mode in [
        "null",
        "missing",
        "partial-exit",
        "whole-exit",
        "invalid-json",
    ] {
        let site = Site::enriched(mode, true);
        let calls = site.refresh();
        let matters = site.world.matters();
        assert_eq!(matters.len(), 47, "retain every issue on {mode}");
        let failed_keys: BTreeSet<String> = if ["whole-exit", "invalid-json"].contains(&mode) {
            batch_calls(&calls)
                .into_iter()
                .find(|c| {
                    c["keys"]
                        .as_array()
                        .unwrap()
                        .contains(&json!("demo/javelin#1"))
                })
                .unwrap()["keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|k| k.as_str().unwrap().to_string())
                .collect()
        } else {
            BTreeSet::from(["demo/javelin#1".to_string()])
        };
        for matter in &matters {
            let key = matter["key"]
                .as_str()
                .unwrap()
                .strip_prefix("github-issues:")
                .unwrap();
            if failed_keys.contains(key) || key == "demo/javelin#90" {
                assert!(messages(matter).is_empty(), "{mode}: {key}");
            } else {
                assert!(
                    !messages(matter).is_empty(),
                    "lost successful sibling on {mode}: {key}"
                );
            }
        }
        logs.push(calls);
    }
    for calls in logs {
        assert_batches(&calls);
    }
}
