//! CLI contract for the opt-in repository-open question
//! (§FS-001-forge-interface.1, §FS-001-forge-interface.8.1).

use std::fs;
use std::path::Path;

use chrono::{Duration, Utc};
use serde_json::{json, Value};

use super::common::{ephor_cmd, make_executable, tempdir};
use super::{feed_env, write_feed_fixture};

// Emulate the forge, including state/repository/time selection and paging.
// Check the entire batch independently of alias order. Extra label questions
// and separate requests per repository are refused, rather than tolerated by
// a canned response that could make an inefficient implementation pass.
const GH: &str = r#"#!/usr/bin/env python3
import json, os, re, shlex, sys
from pathlib import Path

fixture_path = Path(os.environ['OPEN_ISSUE_FIXTURE'])
fixture = json.loads(fixture_path.read_text())
args = sys.argv[1:]
assert args[:2] == ['api', 'graphql'], args
query = next(arg[6:] for arg in args if arg.startswith('query='))
fields = re.findall(r'(\w+):search\(query:("(?:\\.|[^"\\])*"),type:ISSUE,first:(\d+)(?:,after:("(?:\\.|[^"\\])*"))?\)', query)
assert fields, query
log = fixture_path.with_suffix('.calls')
calls = json.loads(log.read_text()) if log.exists() else []
page = len(calls)
assert page < len(fixture['pages']), 'unexpected extra GraphQL request'
expected = fixture['questions']
assert len(fields) == len(expected), 'redundant or missing questions: ' + query
data = {}
seen = []
for alias, literal, first, cursor in fields:
    text = json.loads(literal)
    terms = shlex.split(text)
    kind = ('authored' if 'author:@me' in terms else
            'participating' if 'involves:@me' in terms else
            'label' if any(t.startswith('label:') for t in terms) else 'open')
    assert kind in expected, 'unrequested question: ' + text
    assert sorted(terms) == sorted(expected[kind]), 'wrong scope: ' + text
    assert int(first) == fixture['limit'] - fixture['collected'][page], query
    assert (json.loads(cursor) if cursor else None) == (None if page == 0 else 'next'), query
    seen.append(kind)
    nodes = []
    for node in fixture['pages'][page]:
        if 'state:open' in terms and node['state'] != 'OPEN':
            continue
        repos = [t[5:] for t in terms if t.startswith('repo:')]
        if repos and node['repository']['nameWithOwner'] not in repos:
            continue
        since = next((t[10:] for t in terms if t.startswith('updated:>=')), None)
        if since and node['updatedAt'][:10] < since:
            continue
        if kind == 'authored' and node['author']['login'] != 'tester':
            continue
        if kind == 'participating' and not node.get('involved', False):
            continue
        if kind == 'label' and not any(n['name'] == 'priority' for n in node['labels']['nodes']):
            continue
        nodes.append(node)
    more = page + 1 < len(fixture['pages']) or fixture.get('more_at_end', False)
    data[alias] = {'pageInfo': {'hasNextPage': more, 'endCursor': 'next' if more else None}, 'nodes': nodes}
assert sorted(seen) == sorted(expected), 'duplicate question kinds'
calls.append([json.loads(f[1]) for f in fields])
log.write_text(json.dumps(calls))
print(json.dumps({'data': data}))
"#;

fn issue(number: u64, author: &str, state: &str, labels: &[&str]) -> Value {
    json!({
        "number": number,
        "title": format!("Issue {number}"),
        "url": format!("https://github.com/acme/widget/issues/{number}"),
        "updatedAt": Utc::now().to_rfc3339(),
        "state": state,
        "repository": {"nameWithOwner": "acme/widget"},
        "author": {"login": author},
        "assignees": {"nodes": []},
        "labels": {"nodes": labels.iter().map(|name| json!({"name": name})).collect::<Vec<_>>()},
        "comments": {"totalCount": 0}
    })
}

fn open_config() -> Value {
    json!({
        "provider": "github-issues", "repos": ["acme/widget"], "open": true,
        "authored": false, "participating": false, "comments": false,
        "updated_within_days": 0, "limit": 10
    })
}

fn questions(config: &Value) -> Value {
    let mut bounds = vec!["is:issue".to_string(), "sort:updated-desc".to_string()];
    if let Some(repos) = config["repos"].as_array() {
        bounds.extend(
            repos
                .iter()
                .map(|repo| format!("repo:{}", repo.as_str().unwrap())),
        );
    }
    let days = config["updated_within_days"].as_i64().unwrap_or(30);
    if days > 0 {
        bounds.push(format!(
            "updated:>={}",
            (Utc::now() - Duration::days(days)).format("%Y-%m-%d")
        ));
    }
    let mut expected = serde_json::Map::new();
    for (enabled, kind, extra) in [
        (
            config["authored"].as_bool().unwrap_or(true),
            "authored",
            vec!["author:@me"],
        ),
        (
            config["participating"].as_bool().unwrap_or(true),
            "participating",
            vec!["involves:@me"],
        ),
        (
            config["open"].as_bool().unwrap_or(false),
            "open",
            vec!["state:open"],
        ),
        (
            !config["open"].as_bool().unwrap_or(false)
                && config["labels"].as_array().is_some_and(|v| !v.is_empty()),
            "label",
            vec!["label:priority", "state:open"],
        ),
    ] {
        if enabled {
            let terms: Vec<_> = bounds.iter().map(String::as_str).chain(extra).collect();
            expected.insert(kind.into(), json!(terms));
        }
    }
    Value::Object(expected)
}

fn setup(tmp: &Path, config: &Value, pages: Vec<Vec<Value>>, collected: Vec<usize>) {
    write_feed_fixture(tmp);
    fs::write(
        tmp.join("status.json"),
        serde_json::to_vec_pretty(&json!({
            "defaults": {"github_user": "tester", "provider_timeout_seconds": 10},
            "projects": {"demo": {"providers": [config]}}
        }))
        .unwrap(),
    )
    .unwrap();
    let fake_bin = tmp.join("fakebin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("gh"), GH);
    fs::write(
        tmp.join("open-fixture.json"),
        serde_json::to_vec(&json!({
            "questions": questions(config), "limit": config["limit"].as_u64().unwrap_or(30),
            "pages": pages, "collected": collected
        }))
        .unwrap(),
    )
    .unwrap();
}

fn command(tmp: &Path) -> assert_cmd::Command {
    let mut cmd = ephor_cmd();
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            tmp.join("fakebin").display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    cmd.env("OPEN_ISSUE_FIXTURE", tmp.join("open-fixture.json"));
    for (key, value) in feed_env(tmp) {
        cmd.env(key, value);
    }
    cmd
}

fn matters(tmp: &Path) -> Vec<Value> {
    let cache: Value =
        serde_json::from_slice(&fs::read(tmp.join("state/ephor/feed/demo.json")).unwrap()).unwrap();
    cache["providers"]["github-issues"]["matters"]
        .as_array()
        .unwrap()
        .clone()
}

fn calls(tmp: &Path) -> Vec<Value> {
    serde_json::from_slice(
        &fs::read(tmp.join("open-fixture.calls"))
            .expect("the provider must actually ask the forge"),
    )
    .unwrap()
}

/// §FS-001-forge-interface.1 and §FS-003-feed-categories.1: even an old,
/// unlabelled issue the reader never touched reaches Participating; their own
/// issue reaches My Issues. Check both retained matters and the public feed.
#[test]
fn open_issue_question_finds_unlabelled_and_unfollowed_issues_under_actual_roles() {
    let tmp = tempdir();
    let mut old = issue(1, "stranger", "OPEN", &[]);
    old["updatedAt"] = json!("2001-01-01T00:00:00Z");
    setup(
        tmp.path(),
        &open_config(),
        vec![vec![
            old,
            issue(2, "teammate", "OPEN", &["bug"]),
            issue(3, "tester", "OPEN", &[]),
            issue(4, "stranger", "CLOSED", &[]),
        ]],
        vec![0],
    );
    command(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    let cached = matters(tmp.path());
    assert_eq!(cached.len(), 3, "all open issues, no closed history");
    let out = command(tmp.path())
        .args(["feed", "--json"])
        .assert()
        .success();
    let feed: Vec<Value> = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert_eq!(feed.len(), 3, "every fetched open issue reaches the feed");
    for (number, role) in [(1, "reviewer"), (2, "reviewer"), (3, "author")] {
        let key = format!("github-issues:acme/widget#{number}");
        let retained = cached
            .iter()
            .find(|m| m["key"] == key)
            .expect("retained matter");
        let row = feed
            .iter()
            .find(|m| m["id"] == key)
            .expect("visible feed row");
        for item in [retained, row] {
            assert_eq!(item["kind"], "issue");
            assert_eq!(
                item["role"], role,
                "author is My Issues; reviewer is Participating"
            );
            assert_eq!(item["state"], "open");
        }
    }
    assert_eq!(calls(tmp.path()).len(), 1);
}

/// §FS-001-forge-interface.1: one question spans every named repository and
/// inherits the default window, excluding closed, old, and out-of-scope work.
#[test]
fn open_issue_question_combines_repositories_with_the_default_window() {
    let tmp = tempdir();
    let mut config = open_config();
    config["repos"] = json!(["acme/widget", "acme/library"]);
    config
        .as_object_mut()
        .unwrap()
        .remove("updated_within_days");
    let mut second = issue(2, "stranger", "OPEN", &[]);
    second["repository"]["nameWithOwner"] = json!("acme/library");
    second["url"] = json!("https://github.com/acme/library/issues/2");
    let mut outside = issue(3, "stranger", "OPEN", &[]);
    outside["repository"]["nameWithOwner"] = json!("other/repo");
    let mut old = issue(4, "stranger", "OPEN", &[]);
    old["updatedAt"] = (Utc::now() - Duration::days(31)).to_rfc3339().into();
    setup(
        tmp.path(),
        &config,
        vec![vec![
            issue(1, "stranger", "OPEN", &[]),
            second,
            outside,
            old,
            issue(5, "stranger", "CLOSED", &[]),
        ]],
        vec![0],
    );
    command(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    let found = matters(tmp.path());
    assert_eq!(found.len(), 2);
    assert!(found
        .iter()
        .any(|m| m["key"] == "github-issues:acme/widget#1"));
    assert!(found
        .iter()
        .any(|m| m["key"] == "github-issues:acme/library#2"));
    assert_eq!(calls(tmp.path()).len(), 1);
}

/// §FS-001-forge-interface.1: an explicit nonzero window bounds open too.
#[test]
fn open_issue_question_uses_the_shared_explicit_window() {
    let tmp = tempdir();
    let mut config = open_config();
    config["updated_within_days"] = json!(7);
    let mut old = issue(2, "stranger", "OPEN", &[]);
    old["updatedAt"] = (Utc::now() - Duration::days(8)).to_rfc3339().into();
    setup(
        tmp.path(),
        &config,
        vec![vec![issue(1, "stranger", "OPEN", &[]), old]],
        vec![0],
    );
    command(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    assert_eq!(matters(tmp.path()).len(), 1);
    assert_eq!(calls(tmp.path()).len(), 1);
}

/// §FS-001-forge-interface.1: the new mode alone requires named repositories.
#[test]
fn open_issue_question_refuses_empty_repositories_before_fetching() {
    let tmp = tempdir();
    let mut config = open_config();
    config["repos"] = json!([]);
    setup(tmp.path(), &config, vec![vec![]], vec![0]);
    let out = command(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .failure();
    let err = String::from_utf8_lossy(&out.get_output().stderr);
    assert!(
        err.contains("open")
            && err.contains("repos")
            && (err.contains("nonempty")
                || err.contains("non-empty")
                || err.contains("at least one")),
        "open requires named repositories: {err}"
    );
    assert!(!tmp.path().join("open-fixture.calls").exists());
}

/// §FS-001-forge-interface.8.1: labels reuse open, but closed role answers
/// remain. An author found by all three questions keeps one author matter.
#[test]
fn open_issue_question_reuses_labels_and_retains_closed_role_answers_and_author_precedence() {
    let tmp = tempdir();
    let mut config = open_config();
    config["authored"] = json!(true);
    config["participating"] = json!(true);
    config["labels"] = json!(["priority", "high-priority"]);
    let mut mine = issue(1, "tester", "OPEN", &["priority"]);
    mine["involved"] = json!(true);
    let mut closed_participant = issue(4, "stranger", "CLOSED", &[]);
    closed_participant["involved"] = json!(true);
    setup(
        tmp.path(),
        &config,
        vec![vec![
            mine,
            issue(2, "stranger", "OPEN", &["bug"]),
            issue(3, "tester", "CLOSED", &[]),
            closed_participant,
        ]],
        vec![0],
    );
    command(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    let found = matters(tmp.path());
    assert_eq!(
        found.len(),
        4,
        "deduplicate overlaps and retain closed role answers"
    );
    for (number, role, state) in [
        (1, "author", "open"),
        (2, "reviewer", "open"),
        (3, "author", "closed"),
        (4, "reviewer", "closed"),
    ] {
        let item = found
            .iter()
            .find(|m| m["key"] == format!("github-issues:acme/widget#{number}"))
            .unwrap();
        assert_eq!(item["role"], role);
        assert_eq!(item["state"], state);
    }
    assert_eq!(
        calls(tmp.path()).len(),
        1,
        "all questions in the same batch"
    );
}

fn backwards_compatible(open: Option<bool>) {
    let tmp = tempdir();
    let mut config = json!({"provider": "github-issues", "repos": [], "labels": ["priority"], "comments": false, "updated_within_days": 0, "limit": 10});
    if let Some(open) = open {
        config["open"] = json!(open);
    }
    let mut involved = issue(2, "stranger", "CLOSED", &[]);
    involved["involved"] = json!(true);
    setup(
        tmp.path(),
        &config,
        vec![vec![
            issue(1, "tester", "CLOSED", &[]),
            involved,
            issue(3, "stranger", "OPEN", &["priority"]),
            issue(4, "stranger", "OPEN", &[]),
        ]],
        vec![0],
    );
    command(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    let found = matters(tmp.path());
    assert_eq!(
        found.len(),
        3,
        "default role questions and label question stay on, global scope stays valid"
    );
    assert!(!found
        .iter()
        .any(|m| m["key"] == "github-issues:acme/widget#4"));
    assert_eq!(calls(tmp.path()).len(), 1);
}

/// §FS-001-forge-interface.1: characterization of the unchanged default.
#[test]
fn open_issue_question_omitted_preserves_existing_global_role_and_label_questions() {
    backwards_compatible(None);
}

/// §FS-001-forge-interface.1: explicit false is the same as omission.
#[test]
fn open_issue_question_false_preserves_existing_global_role_and_label_questions() {
    backwards_compatible(Some(false));
}

/// §FS-001-forge-interface.1: below-limit answers are complete after paging.
#[test]
fn open_issue_question_pages_to_a_complete_answer_below_limit() {
    let tmp = tempdir();
    let mut config = open_config();
    config["limit"] = json!(4);
    setup(
        tmp.path(),
        &config,
        vec![
            vec![
                issue(1, "stranger", "OPEN", &[]),
                issue(2, "stranger", "OPEN", &[]),
            ],
            vec![issue(3, "tester", "OPEN", &[])],
        ],
        vec![0, 2],
    );
    command(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    assert_eq!(matters(tmp.path()).len(), 3);
    assert_eq!(calls(tmp.path()).len(), 2, "follow the forge's cursor");
}

fn saturation(paged: bool, more_at_end: bool) {
    let tmp = tempdir();
    // Establish last-good work through a configuration already supported
    // today. A failed open answer must retain it, never replace it by a prefix.
    let seed = json!({
        "provider": "github-issues", "repos": ["acme/widget"],
        "authored": true, "participating": false, "comments": false,
        "updated_within_days": 0, "limit": 10
    });
    setup(
        tmp.path(),
        &seed,
        vec![vec![issue(99, "tester", "OPEN", &[])]],
        vec![0],
    );
    command(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .success();
    fs::remove_file(tmp.path().join("open-fixture.calls")).unwrap();
    let mut config = open_config();
    config["limit"] = json!(3);
    config["labels"] = json!(["priority"]);
    // Three raw matches but one distinct key, and none carries the followed
    // label: either deduplication or label filtering before the check is wrong.
    let node = issue(1, "stranger", "OPEN", &["bug"]);
    let (pages, collected) = if paged {
        (
            vec![vec![node.clone()], vec![node.clone(), node]],
            vec![0, 1],
        )
    } else {
        (vec![vec![node.clone(), node.clone(), node]], vec![0])
    };
    setup(tmp.path(), &config, pages, collected);
    if more_at_end {
        let path = tmp.path().join("open-fixture.json");
        let mut fixture: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        fixture["more_at_end"] = json!(true);
        fs::write(path, serde_json::to_vec(&fixture).unwrap()).unwrap();
    }
    let out = command(tmp.path())
        .args(["refresh", "demo"])
        .assert()
        .failure();
    let err = String::from_utf8_lossy(&out.get_output().stderr);
    for text in [
        "github-issues",
        "open",
        "limit",
        "3",
        "matching work may remain",
        "raise `limit`",
        "repos",
        "updated_within_days",
        "labels",
    ] {
        assert!(
            err.contains(text),
            "missing {text:?} from open saturation diagnosis: {err}"
        );
    }
    assert!(
        err.contains("disable `open`")
            || err.contains("turn `open` off")
            || err.contains("set `open` to false"),
        "narrow labels only after disabling open: {err}"
    );
    let cache: Value =
        serde_json::from_slice(&fs::read(tmp.path().join("state/ephor/feed/demo.json")).unwrap())
            .unwrap();
    let slot = &cache["providers"]["github-issues"];
    assert!(slot["error"].as_str().is_some_and(|e| e.contains("open")));
    assert_eq!(
        slot["stale"], true,
        "retain last-good work visibly as stale"
    );
    let retained = slot["matters"].as_array().unwrap();
    assert_eq!(
        retained.len(),
        1,
        "do not cache a partial answer as complete"
    );
    assert_eq!(retained[0]["key"], "github-issues:acme/widget#99");
    assert_eq!(calls(tmp.path()).len(), if paged { 2 } else { 1 });
}

/// §FS-001-forge-interface.1, §FS-001-forge-interface.6: reaching limit fails
/// before deduplication/filtering even when hasNextPage is false.
#[test]
fn open_issue_question_saturation_fails_before_deduplication_or_label_filtering() {
    saturation(false, false);
}

/// §FS-001-forge-interface.1: reaching limit across pages also fails visibly.
#[test]
fn open_issue_question_paged_saturation_fails_with_effective_remedies() {
    saturation(true, false);
}

/// §FS-001-forge-interface.1: when limit is full, a remaining cursor cannot
/// turn a prefix into success, or cause another request past the bound.
#[test]
fn open_issue_question_saturation_with_more_pages_fails_without_exceeding_limit() {
    saturation(false, true);
}
