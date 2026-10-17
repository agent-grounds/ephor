//! CI gates on GitHub: the user's open pull requests' via `gh pr checks`, and
//! each watched branch's via the workflow runs on it.
//!
//! A pull request's gate is an observation of the pull request it is about,
//! never a row of its own (§FS-007-matters.5): this source reports the *pull
//! request*, carrying its gate, so a change with a red gate is one row and not
//! two (§FS-003-feed-categories.5). A watched branch belongs to no change, so
//! its gate is the row: one per repository and branch the source names in
//! `branches`, while that gate is red (§FS-001-forge-interface.1). Either is
//! offered what failed and the restarts as quick actions
//! (§FS-004-quick-actions.4).

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::feed::config::ActionConfig;
use crate::feed::gate::{self, Gate, RepoGate};
use crate::feed::model::{Item, ItemKind};
use crate::feed::provider::{
    command_exists, run_json, Provider, ProviderContext, ProviderError, ProviderResult,
};
use crate::feed::providers::{
    branch_actions, gh_command, parse_config, parse_github_time, restart_actions,
    show_failing_checks,
};
use crate::forge::{policy, BranchGate, Verdict, Workflow};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[allow(dead_code)]
    provider: String,
    repos: Vec<String>,
    #[serde(default)]
    host: Option<String>,
    /// The branches watched on every repository above
    /// (§FS-001-forge-interface.1). Absent, the source asks exactly what it
    /// asked before.
    #[serde(default)]
    branches: Vec<String>,
}

/// The gate is red — the condition the failing-CI quick action exists for.
/// Read off the gate the report carries rather than off a state word, now
/// that the report is of the pull request rather than of the gate.
fn gate_is_red(item: &Item) -> bool {
    gate::Gate::of(item).is_some_and(|gate| gate.is_red())
}

pub struct GithubCi {
    config: Config,
}

impl GithubCi {
    pub fn from_config(config: &Value) -> Result<Self, ProviderError> {
        Ok(GithubCi {
            config: parse_config(config)?,
        })
    }

    /// The quick action for one item: offered only on a gate that is actually
    /// red, and only while the item still names the pull request whose log is
    /// being asked for (§FS-004-quick-actions.2).
    fn failing_check_actions(&self, item: &Item) -> Vec<ActionConfig> {
        let identified = item.repo().is_some() && item.number().is_some();
        if !gate_is_red(item) || !identified {
            return Vec::new();
        }
        vec![show_failing_checks(self.config.host.as_deref())]
    }
}

impl Provider for GithubCi {
    fn name(&self) -> &'static str {
        "github-ci"
    }

    fn available(&self, _ctx: &ProviderContext) -> bool {
        command_exists("gh")
    }

    /// Nothing is offered where the CLI that would answer is not installed:
    /// a menu entry that can only print `gh: not found` is worse than no
    /// entry (§FS-004-quick-actions.2).
    fn quick_actions(&self, item: &Item) -> Vec<ActionConfig> {
        if !command_exists("gh") {
            return Vec::new();
        }
        if item.watched_branch().is_some() {
            return branch_actions(self.config.host.as_deref(), item);
        }
        let mut entries = self.failing_check_actions(item);
        entries.extend(restart_actions(self.config.host.as_deref(), item));
        entries
    }

    fn fetch(&self, ctx: &ProviderContext) -> ProviderResult {
        let mut items = Vec::new();
        for repo in &self.config.repos {
            let mut command = gh_command(self.config.host.as_deref());
            command.args([
                "pr",
                "list",
                "--repo",
                repo,
                "--author",
                "@me",
                "--json",
                "number,title,url,updatedAt",
            ]);
            let prs = run_json(command, ctx.timeout, false)?;
            for pr in prs.as_array().cloned().unwrap_or_default() {
                let number = pr.get("number").and_then(Value::as_u64).unwrap_or(0);
                // `gh pr checks` uses its exit code for check state, so accept
                // non-zero exits and parse stdout regardless.
                let mut checks_cmd = gh_command(self.config.host.as_deref());
                checks_cmd.args([
                    "pr",
                    "checks",
                    &number.to_string(),
                    "--repo",
                    repo,
                    "--json",
                    "name,state,link",
                ]);
                let checks = match run_json(checks_cmd, ctx.timeout, true) {
                    Ok(checks) => checks.as_array().cloned().unwrap_or_default(),
                    // No checks reported (e.g. no CI configured) is not an error.
                    Err(_) => Vec::new(),
                };
                if checks.is_empty() {
                    continue;
                }
                let gate = gate::from_check_states(repo, &checks);
                let (total, passing, failing, pending) =
                    (gate.total(), gate.passed(), gate.failed(), gate.running());
                if failing == 0 && pending == 0 {
                    continue;
                }

                let _ = (total, passing, pending);
                // The pull request, carrying what its gate is doing — one
                // subject, one row (§FS-003-feed-categories.5). The counts
                // are the gate's, and every surface reads them from there.
                items.push(Item {
                    id: format!("github-ci:{repo}#{number}"),
                    project: ctx.project_id.clone(),
                    source: "github-ci".to_string(),
                    kind: ItemKind::Pr,
                    role: None,
                    title: pr
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    url: pr.get("url").and_then(Value::as_str).map(String::from),
                    state: None,
                    // A red gate is work waiting on the person whose change it
                    // is; a gate still running is not.
                    needs_response: failing > 0,
                    updated_at: parse_github_time(pr.get("updatedAt").unwrap_or(&Value::Null)),
                    raw: serde_json::json!({ "repo": repo, "gate": gate.to_value() }),
                });
            }
            for branch in &self.config.branches {
                let reported = self.branch_gate(ctx, repo, branch)?;
                let Some((gate, failed_jobs)) = reported else {
                    continue;
                };
                // Whether the branch is a row is policy's, the same for every
                // forge (§FS-001-forge-interface.3).
                if let Some(mut item) =
                    policy::branch_gate_item(self.name(), &ctx.project_id, &gate)
                {
                    if let Some(raw) = item.raw.as_object_mut() {
                        raw.insert("failed_jobs".to_string(), serde_json::json!(failed_jobs));
                    }
                    items.push(item);
                }
            }
        }
        Ok(items)
    }
}

impl GithubCi {
    /// One watched branch's gate (§FS-001-forge-interface.1), with the names of
    /// the jobs that failed under it, or `None` where nothing on it is red.
    ///
    /// One listing of the branch's newest hundred runs carries every
    /// workflow's verdict, and one jobs listing more per red run gives the
    /// row its counts (§FS-001-forge-interface.8.4). Both are `gh api` and
    /// the hourly meter: `gh run list` names the workflows with a second
    /// request and `gh run view` spends three.
    fn branch_gate(
        &self,
        ctx: &ProviderContext,
        repo: &str,
        branch: &str,
    ) -> Result<Option<(BranchGate, Vec<String>)>, ProviderError> {
        let host = self.config.host.as_deref();
        let mut listing = gh_command(host);
        listing.args([
            "api",
            &format!(
                "repos/{repo}/actions/runs?branch={}&per_page=100",
                query_value(branch)
            ),
        ]);
        let listed = run_json(listing, ctx.timeout, false)?;
        let runs = listed
            .get("workflow_runs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let verdicts = newest_verdicts(repo, runs);
        let red: Vec<&Value> = verdicts
            .iter()
            .filter(|(workflow, _)| workflow.verdict == Verdict::Red)
            .map(|(_, run)| *run)
            .collect();
        if red.is_empty() {
            return Ok(None);
        }

        let mut counts = RepoGate::new(repo);
        let mut failed_jobs = Vec::new();
        for run in &red {
            let id = run_id(run);
            let mut jobs = gh_command(host);
            jobs.args([
                "api",
                &format!("repos/{repo}/actions/runs/{id}/jobs?per_page=100"),
            ]);
            let listed = run_json(jobs, ctx.timeout, false)?;
            for job in listed
                .get("jobs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let state = match job.get("status").and_then(Value::as_str) {
                    Some("completed") => job.get("conclusion").and_then(Value::as_str),
                    _ => None,
                };
                let state = state.unwrap_or("in_progress");
                counts.add(state, 1);
                if gate::classify(state) == gate::JobState::Failed {
                    if let Some(name) = job.get("name").and_then(Value::as_str) {
                        failed_jobs.push(name.to_string());
                    }
                }
            }
        }

        // The newest red run is the commit the row is about.
        let newest = red
            .iter()
            .max_by(|a, b| str_of(a, "created_at").cmp(str_of(b, "created_at")))
            .copied();
        let updated_at = red
            .iter()
            .map(|run| parse_github_time(run.get("updated_at").unwrap_or(&Value::Null)))
            .max()
            .unwrap_or_else(chrono::Utc::now);
        Ok(Some((
            BranchGate {
                repo: repo.to_string(),
                branch: branch.to_string(),
                url: None,
                head_sha: newest.and_then(|run| {
                    run.get("head_sha")
                        .and_then(Value::as_str)
                        .map(String::from)
                }),
                updated_at,
                workflows: verdicts.into_iter().map(|(workflow, _)| workflow).collect(),
                gate: Some(Gate {
                    repos: vec![counts],
                    ..Gate::default()
                }),
            },
            failed_jobs,
        )))
    }
}

/// A run's id as the API's paths spell it.
fn run_id(run: &Value) -> String {
    match run.get("id") {
        Some(Value::Number(id)) => id.to_string(),
        Some(Value::String(id)) => id.clone(),
        _ => String::new(),
    }
}

fn str_of<'a>(run: &'a Value, key: &str) -> &'a str {
    run.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// What a completed run's conclusion says, in the two words of a verdict
/// (§FS-001-forge-interface.1). A run still going, cancelled, skipped, or
/// waiting on an approval has no verdict and is passed over.
fn verdict_of(run: &Value) -> Option<Verdict> {
    if str_of(run, "status") != "completed" {
        return None;
    }
    match str_of(run, "conclusion") {
        "success" | "neutral" => Some(Verdict::Green),
        "failure" | "timed_out" | "startup_failure" => Some(Verdict::Red),
        _ => None,
    }
}

/// A run the branch itself made (§FS-001-forge-interface.1): its head is in
/// the watched repository, and no pull request's event started it. The listing
/// filters on the head branch's name alone, so a fork's pull request from its
/// own `main`, and a `workflow_run` it triggers, are listed under `main` too.
fn own_run(repo: &str, run: &Value) -> bool {
    let head = run
        .pointer("/head_repository/full_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    head.eq_ignore_ascii_case(repo) && !str_of(run, "event").starts_with("pull_request")
}

/// The newest verdict of each workflow among the branch's own runs, keyed by
/// the workflow's id and never its name, newest by run number and then
/// creation time and never by the last update, so an old red run rerun to
/// green does not clear a newer red one (§FS-001-forge-interface.1). Each
/// comes with the run that reached it.
fn newest_verdicts<'a>(repo: &str, runs: &'a [Value]) -> Vec<(Workflow, &'a Value)> {
    let mut newest: BTreeMap<u64, (u64, &str, Verdict, &Value)> = BTreeMap::new();
    for run in runs.iter().filter(|run| own_run(repo, run)) {
        let (Some(workflow), Some(verdict)) = (
            run.get("workflow_id").and_then(Value::as_u64),
            verdict_of(run),
        ) else {
            continue;
        };
        let number = run.get("run_number").and_then(Value::as_u64).unwrap_or(0);
        let created = str_of(run, "created_at");
        let newer = newest
            .get(&workflow)
            .is_none_or(|(seen, at, _, _)| (number, created) > (*seen, *at));
        if newer {
            newest.insert(workflow, (number, created, verdict, run));
        }
    }
    newest
        .into_iter()
        .map(|(id, (_, _, verdict, run))| {
            (
                Workflow {
                    id: id.to_string(),
                    name: str_of(run, "name").to_string(),
                    run: run_id(run),
                    url: run
                        .get("html_url")
                        .and_then(Value::as_str)
                        .map(String::from),
                    verdict,
                },
                run,
            )
        })
        .collect()
}

/// A branch name as one query value: everything but the unreserved characters
/// percent-encoded, so `release/1` and a name with `&` in it ask for exactly
/// that branch.
fn query_value(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn provider(host: Option<&str>) -> GithubCi {
        GithubCi {
            config: Config {
                provider: "github-ci".to_string(),
                repos: vec!["acme/widget".to_string()],
                host: host.map(String::from),
                branches: Vec::new(),
            },
        }
    }

    /// A pull request carrying a gate — what this source reports now that a
    /// gate is an observation rather than a row (§FS-007-matters.5).
    fn ci_item(failed: u64, running: u64) -> Item {
        Item {
            id: "github-ci:acme/widget#42".to_string(),
            project: "widget".to_string(),
            source: "github-ci".to_string(),
            kind: ItemKind::Pr,
            role: None,
            title: "Retry window".to_string(),
            url: None,
            state: None,
            needs_response: failed > 0,
            updated_at: Utc::now(),
            raw: serde_json::json!({
                "repo": "acme/widget",
                "gate": { "repos": [{
                    "repo": "acme/widget", "passed": 1, "failed": failed, "running": running
                }] }
            }),
        }
    }

    #[test]
    fn only_a_red_gate_is_offered_its_log() {
        let provider = provider(None);
        assert_eq!(provider.failing_check_actions(&ci_item(2, 0)).len(), 1);
        // Checks still running have no failure to show yet.
        assert!(provider.failing_check_actions(&ci_item(0, 3)).is_empty());
        // An item that no longer names its pull request cannot be asked about.
        let mut anonymous = ci_item(2, 0);
        anonymous.id = "github-ci:widget".to_string();
        anonymous.raw = serde_json::json!({
            "gate": { "repos": [{ "repo": "acme/widget", "failed": 2 }] }
        });
        assert!(provider.failing_check_actions(&anonymous).is_empty());
    }

    fn run(id: u64, workflow: u64, number: u64, conclusion: &str) -> Value {
        serde_json::json!({
            "id": id, "name": "CI", "workflow_id": workflow, "run_number": number,
            "event": "push", "status": "completed", "conclusion": conclusion,
            "created_at": format!("2026-10-14T1{number}:00:00Z"),
            "head_repository": { "full_name": "acme/widget" }
        })
    }

    /// Conclusions with no verdict are passed over, so the one before stands;
    /// and only the branch's own runs count (§FS-001-forge-interface.1).
    #[test]
    fn a_run_without_a_verdict_or_not_the_branchs_own_is_passed_over() {
        let mut fork = run(904, 11, 5, "failure");
        fork["head_repository"]["full_name"] = "stranger/widget".into();
        let mut inward = run(905, 11, 6, "failure");
        inward["event"] = "pull_request_target".into();
        let mut going = run(903, 11, 4, "success");
        going["status"] = "in_progress".into();
        let runs = [
            inward,
            fork,
            going,
            run(902, 11, 3, "cancelled"),
            run(906, 11, 2, "skipped"),
            run(901, 11, 1, "failure"),
        ];
        let verdicts = newest_verdicts("acme/widget", &runs);
        assert_eq!(verdicts.len(), 1);
        assert_eq!(verdicts[0].0.run, "901");
        assert_eq!(verdicts[0].0.verdict, Verdict::Red);

        let none = [
            run(902, 11, 3, "cancelled"),
            run(903, 11, 4, "action_required"),
        ];
        assert!(newest_verdicts("acme/widget", &none).is_empty());
    }

    #[test]
    fn a_branch_name_is_one_query_value() {
        assert_eq!(query_value("release/1"), "release%2F1");
        assert_eq!(query_value("a&b=c"), "a%26b%3Dc");
        assert_eq!(query_value("main"), "main");
    }

    #[test]
    fn the_enterprise_host_is_exported_rather_than_written_into_the_script() {
        let command = show_failing_checks(Some("ghe.example.com")).command;
        assert!(command.starts_with("export GH_HOST='ghe.example.com'\n"));
        assert!(command.contains("gh run view \"$run\" --repo \"$EPHOR_REPO\" --log-failed"));
        assert!(!show_failing_checks(None).command.contains("GH_HOST"));
    }
}
