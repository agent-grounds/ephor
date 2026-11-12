//! The GitHub closure read, against a `gh` that answers GraphQL from a forge
//! model (§FS-001-forge-interface.1, §AR-005-capabilities).
//!
//! The capability answers whether one issue is over, which pull requests the
//! forge links as closing it, and which are still open from a named head — each
//! list saying whether it is complete. Its reader is `work retire`
//! (§FS-005-dispatch.36.1), so this drives `work retire --dry-run --json`,
//! which reads the forge and writes nothing, and checks what the read put in
//! its `eligibility` and its refusals. The forge is the e2e cases' closure
//! forge: it parses the query it is handed, so the read is the provider's to
//! phrase.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::{json, Value};

use super::common::{
    base_project_types, ephor_cmd, make_executable, tempdir, write_registry, write_template,
};
use super::feed_env;

const FORGE: &str = include_str!("../e2e/fixtures/closure-forge/gh.py");
const ISSUE: &str = "github-issues:acme/widget#301";

struct Closure {
    tmp: tempfile::TempDir,
}

impl Closure {
    /// Issue #301 open on the forge, dispatched to an `answer` recipe, then
    /// moved by `change` and refreshed — the matter as `work retire` finds it.
    fn dispatched(change: impl FnOnce(&mut Value)) -> Closure {
        let tmp = tempdir();
        let root = tmp.path();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("demo")).unwrap();
        fs::write(root.join("gh.py"), FORGE).unwrap();
        make_executable(
            &root.join("bin/gh"),
            &format!(
                "#!/bin/sh\nexec python3 -I \"{}\" \"$@\"\n",
                root.join("gh.py").display()
            ),
        );
        make_executable(
            &root.join("bin/rhei"),
            "#!/bin/sh\nif [ \"${1:-}\" = runs ]; then echo '[]'; fi\nexit 0\n",
        );
        let template = write_template(root);
        write_registry(
            &root.join("workspaces.json"),
            &json!({
                "project_types": base_project_types(&template),
                "hook_sets": [],
                "projects": [{
                    "id": "demo", "type": "monorepo", "display_name": "Demo",
                    "root": root.join("demo"), "main_branch": "main", "branches": []
                }]
            }),
        );
        fs::write(
            root.join("status.json"),
            serde_json::to_string_pretty(&json!({
                "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10,
                              "github_user": "tester" },
                "projects": { "demo": {
                    "providers": [{
                        "provider": "github-issues", "repos": ["acme/widget"],
                        "participating": false, "comments": false, "updated_within_days": 0
                    }],
                    "work": { "root": "{root}/panta", "recipes": [{
                        "id": "answer", "icon": "a", "description": "answer", "state": "fix",
                        "needs_checkout": false, "when": { "kinds": ["issue"] },
                        "brief": "{title}"
                    }]}
                }},
                "work": { "runner": "rhei" }
            }))
            .unwrap(),
        )
        .unwrap();
        let site = Closure { tmp };
        let mut model = json!({
            "login": "tester",
            "repos": { "acme/widget": {
                "issues": { "301": { "title": "Remove the fallback", "state": "OPEN" } },
                "pulls": {}
            }}
        });
        site.forge(&model);
        site.ok(&["refresh", "demo", "--quiet"]);
        site.ok(&["work", "dispatch", "--item", ISSUE, "--recipe", "answer"]);
        change(&mut model);
        site.forge(&model);
        site.ok(&["refresh", "demo", "--quiet"]);
        let _ = fs::remove_file(site.calls_path());
        site
    }

    fn path(&self) -> &Path {
        self.tmp.path()
    }

    fn forge(&self, model: &Value) {
        fs::write(
            self.path().join("forge.json"),
            serde_json::to_string_pretty(model).unwrap(),
        )
        .unwrap();
    }

    fn calls_path(&self) -> PathBuf {
        self.path().join("forge.json.calls")
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut command = ephor_cmd();
        for key in ["GH_HOST", "GH_TOKEN", "GITHUB_TOKEN"] {
            command.env_remove(key);
        }
        command
            .envs(feed_env(self.path()))
            .env("HOME", self.path())
            .env("XDG_DATA_HOME", self.path().join("data"))
            .env("CLOSURE_FORGE", self.path().join("forge.json"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.path().join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .args(args)
            .output()
            .expect("ephor runs")
    }

    fn ok(&self, args: &[&str]) -> Output {
        let out = self.run(args);
        assert!(out.status.success(), "ephor {args:?}:\n{}", said(&out));
        out
    }

    /// The dry run's reading of the forge, and that it wrote nothing.
    fn read(&self) -> (Output, Value) {
        let out = self.run(&["work", "retire", "--item", ISSUE, "--dry-run", "--json"]);
        let value: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
            panic!(
                "`ephor work retire --dry-run --json` printed no JSON:\n{}",
                said(&out)
            )
        });
        assert!(
            !self.path().join("data/ephor/archive").exists(),
            "a dry run writes nothing"
        );
        (out, value)
    }

    /// The forge's own log of what was asked of it since the refresh.
    fn asked(&self) -> Vec<Value> {
        fs::read_to_string(self.calls_path())
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

fn said(out: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// #301 closed by #331, merged from `fix/issue-301`.
fn closed_by_331(model: &mut Value) {
    let repo = &mut model["repos"]["acme/widget"];
    repo["pulls"]["331"] = json!({
        "title": "Remove it", "state": "MERGED", "mergedAt": "2026-10-01T23:29:00Z",
        "closedAt": "2026-10-01T23:29:00Z", "headRefName": "fix/issue-301", "closes": [301]
    });
    repo["issues"]["301"]["state"] = json!("CLOSED");
    repo["issues"]["301"]["closedAt"] = json!("2026-10-01T23:29:30Z");
    repo["issues"]["301"]["closedBy"] = json!([331]);
}

#[test]
fn a_closed_issue_and_its_merged_closing_pull_request_are_read_from_the_forge() {
    let site = Closure::dispatched(closed_by_331);
    let (out, value) = site.read();
    assert!(out.status.success(), "{}", said(&out));
    let eligibility = &value["eligibility"];
    assert_eq!(eligibility["issue"]["state"], "closed", "{value:#}");
    assert_eq!(eligibility["issue"]["closed_at"], "2026-10-01T23:29:30Z");
    let merged = &eligibility["merged"][0];
    assert_eq!(merged["pr"], 331, "{value:#}");
    assert!(
        merged["head"]
            .as_str()
            .unwrap_or_default()
            .ends_with("fix/issue-301"),
        "{merged:#}"
    );
    let head = eligibility["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|branch| branch["name"] == "fix/issue-301")
        .unwrap_or_else(|| panic!("the closing PR's head is a candidate: {value:#}"));
    assert!(head["from"].to_string().contains("closing pr"), "{head:#}");
    assert_eq!(eligibility["open"], json!([]));
    assert!(
        site.asked().iter().any(|call| call["kind"] == "read"),
        "the forge is asked at the moment of retiring, not the cache: {:?}",
        site.asked()
    );
}

#[test]
fn an_open_pull_request_from_a_fork_head_is_found_by_name() {
    let site = Closure::dispatched(|model| {
        closed_by_331(model);
        model["repos"]["acme/widget"]["pulls"]["340"] = json!({
            "title": "my take", "state": "OPEN", "headRefName": "fix/issue-301",
            "headOwner": "someone"
        });
    });
    let (out, value) = site.read();
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert_eq!(value["refused"]["guard"], "open-pull-request", "{value:#}");
    assert!(
        value["refused"]["says"]
            .as_str()
            .unwrap_or_default()
            .contains("someone:fix/issue-301"),
        "{value:#}"
    );
}

#[test]
fn a_list_with_more_pages_is_incomplete_not_empty() {
    let site = Closure::dispatched(|model| {
        closed_by_331(model);
        model["cut_short"] = json!(["fix/issue-301"]);
    });
    let (out, value) = site.read();
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert_eq!(value["refused"]["guard"], "forge", "{value:#}");
    assert!(
        value["refused"]["says"]
            .as_str()
            .unwrap_or_default()
            .contains("cut short"),
        "{value:#}"
    );
}

#[test]
fn an_answer_with_errors_is_a_failure_in_the_forges_words() {
    let site = Closure::dispatched(|model| {
        closed_by_331(model);
        model["closure_errors"] = json!("Could not resolve to a Repository");
    });
    let (out, value) = site.read();
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert_eq!(value["refused"]["guard"], "forge", "{value:#}");
    assert!(
        value["refused"]["says"]
            .as_str()
            .unwrap_or_default()
            .contains("Could not resolve to a Repository"),
        "{value:#}"
    );
}
