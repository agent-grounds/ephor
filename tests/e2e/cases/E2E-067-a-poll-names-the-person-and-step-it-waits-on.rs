//! E2E-067-a-poll-names-the-person-and-step-it-waits-on: the ephor#257
//! reproducer in the project's test frame (§FS-005-dispatch.9).
//!
//! Refresh and dispatch lay the real recipe plan. A supervising parent has
//! an agora parked in a poll or gate, then an unstarted ship with agora as
//! Prior. The matter row and its one-task location must name the parked step.
//! Person polls display their declared label but acquire no manual gate action.
//! Parser edge cases remain in runtime::plan's existing unit tests.

#[path = "../../integration/common/mod.rs"]
mod common;

use std::fs;
use std::path::{Path, PathBuf};

use ephor::work::{ledger::Ledger, recipe::WorkConfig, status_of_entry, WorkStatus};
use serde_json::{json, Value};

use common::*;

const ITEM: &str = "github-prs:acme/widget#42";
const PLAN: &str = "github-prs-acme-widget-42-922ddbdc";
const AGORA: &str = "fix-gate-1.agora";
const SHIP: &str = "fix-gate-1.ship";

const RUNTIME: &str = r#"#!/bin/sh
case "$1" in
  --help) printf 'Options:\n      --headless  detach it\n' ;;
  states) cat <<'JSON'
{"states":[{"name":"supervising"},{"name":"ship"},{"name":"awaiting-ruling"},
 {"name":"needs-ruling","gating":true},{"name":"done","final":true}],
 "transitions":[{"from":"needs-ruling","to":"done"}]}
JSON
    ;;
  *) exit 0 ;;
esac
"#;

fn root(site: &Path) -> PathBuf {
    site.join("demo/panta")
}

fn plan(site: &Path) -> PathBuf {
    root(site).join(format!("{PLAN}.rhei.md"))
}

fn stdout(site: &Path, args: &[&str]) -> String {
    let output = ephor(site).args(args).output().unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// Port of runtime/repro/ephor.57/run.sh: the same forge, zero launch
/// ceiling, dispatch, machine extension and nested Prior shape.
fn site(agora_state: &str, declaration: &str) -> tempfile::TempDir {
    let tmp = tempdir();
    fixture(
        tmp.path(),
        json!({
            "max_concurrent": 0,
            "recipes": [{"id":"fix-gate", "icon":"x", "description":"fix the red gate",
                "brief":"fix {title}", "autorun":true,
                "when":{"kinds":["pr"],"gate":"red"}}]
        }),
    );
    fs::create_dir_all(tmp.path().join("fakebin")).unwrap();
    make_executable(&tmp.path().join("fakebin/rhei"), RUNTIME);
    stdout(tmp.path(), &["refresh", "demo"]);
    stdout(tmp.path(), &["work", "dispatch", "--act"]);
    let machine = root(tmp.path()).join("states.yaml");
    let states = fs::read_to_string(&machine).unwrap();
    assert!(states.contains("\n  done:\n"), "{states}");
    fs::write(
        &machine,
        states.replacen(
            "\n  done:\n",
            &format!(
        "\n  supervising:\n    prompt: Supervise the ticket.\n  ship:\n    prompt: Ship it.\n\
         {declaration}  done:\n"
    ),
            1,
        ),
    )
    .unwrap();
    let body = fs::read_to_string(plan(tmp.path())).unwrap();
    assert_eq!(body.matches("**State:** fix").count(), 1, "{body}");
    fs::write(
        plan(tmp.path()),
        format!(
            "{}\n\n#### Step {AGORA}: Hold the agora\n**State:** {agora_state}\n\n\
         Wait for the ruling.\n\n#### Step {SHIP}: Ship the change\n\
         **State:** ship\n**Prior:** {AGORA}\n\nShip it.\n",
            body.replacen("**State:** fix", "**State:** supervising", 1)
                .trim_end()
        ),
    )
    .unwrap();
    tmp
}

fn poll_site(poll: &str) -> tempfile::TempDir {
    site(
        "awaiting-ruling",
        &format!(
        "  awaiting-ruling:\n    program:\n      command: [bash, scripts/await-ruling.sh]\n{poll}"
    ),
    )
}

/// Read the same shared status that feeds the one-task location. No process
/// environment is changed and no git or flag-reading operation runs in-process.
fn status(site: &Path) -> WorkStatus {
    let ledger: Ledger =
        serde_json::from_slice(&fs::read(site.join("state/ephor/work.json")).unwrap()).unwrap();
    let config = WorkConfig {
        runner: Some(site.join("fakebin/rhei").display().to_string()),
        ..WorkConfig::default()
    };
    status_of_entry(&config, &ledger.entries[ITEM], None)
}

fn review(site: &Path) -> Value {
    serde_json::from_str(&stdout(site, &["work", "review", "--item", ITEM, "--json"])).unwrap()
}

fn assert_display(site: &Path, warning: Option<&str>) {
    let listed = stdout(site, &["work", "list"]);
    let row = listed
        .lines()
        .find(|line| line.contains("Retry window"))
        .unwrap();
    let status = status(site);
    let open = status
        .open_at
        .as_ref()
        .expect("unfinished work has a location");
    let mut failures = Vec::new();
    if !row.contains(AGORA) {
        failures.push("row omits the parked agora step".to_string());
    }
    if row.contains(SHIP) {
        failures.push("row names unstarted downstream ship".to_string());
    }
    match warning {
        Some(label) if !row.contains(label) => failures.push(format!("row omits {label:?}")),
        None if row.contains("waiting on") || row.contains('⚠') => {
            failures.push("machine poll has a person warning".to_string())
        }
        _ => {}
    }
    if open.ticket != AGORA || open.state != "awaiting-ruling" || open.plan != plan(site) {
        failures.push(format!(
            "one-task location is {} at {}",
            open.ticket, open.state
        ));
    }
    assert!(
        failures.is_empty(),
        "{}\nobserved row: {row}",
        failures.join("\n")
    );
}

#[test]
fn issue257_owner_poll_names_its_person_and_parked_step() {
    let tmp = poll_site(
        "    poll:\n      interval: 10m\n      max_attempts: 1008\n      waiting_on: owner\n",
    );
    assert_display(tmp.path(), Some("waiting on owner"));
}

#[test]
fn issue257_flow_poll_names_another_declared_person() {
    let tmp = poll_site("    poll: { interval: 10m, max_attempts: 1008, waiting_on: author }\n");
    assert_display(tmp.path(), Some("waiting on author"));
}

#[test]
fn issue257_machine_poll_names_its_step_without_a_person_warning() {
    let tmp = poll_site("    poll:\n      interval: 10m\n      max_attempts: 1008\n");
    assert_display(tmp.path(), None);
}

/// Same-plan qualification must preserve the polling prerequisite edge
/// for both the matter row and its one-task location (§FS-005-dispatch.9).
#[test]
fn issue257_qualified_machine_poll_prior_names_its_step_without_a_person_warning() {
    let tmp = poll_site("    poll:\n      interval: 10m\n      max_attempts: 1008\n");
    let path = plan(tmp.path());
    let body = fs::read_to_string(&path).unwrap();
    let local = format!("**Prior:** {AGORA}\n");
    assert_eq!(body.matches(&local).count(), 1, "{body}");
    fs::write(
        &path,
        body.replace(&local, &format!("**Prior:** {PLAN}.{AGORA}\n")),
    )
    .unwrap();
    assert_display(tmp.path(), None);
}

#[test]
fn issue257_trailing_comment_does_not_declare_a_person() {
    let tmp = poll_site("    poll: { interval: 10m, max_attempts: 1008 }  # waiting_on: owner\n");
    assert_display(tmp.path(), None);
}

/// Preservation guard: this already passes, and must stay so when display
/// person waits are added. Display alone cannot confer a manual action.
#[test]
fn issue257_person_poll_offers_no_manual_gate_or_advance() {
    let tmp = poll_site("    poll:\n      interval: 10m\n      waiting_on: owner\n");
    let before = fs::read(plan(tmp.path())).unwrap();
    assert!(status(tmp.path()).advance.is_none());
    assert_eq!(review(tmp.path())["gates"], json!([]));
    assert!(stdout(tmp.path(), &["work", "review", "--item", ITEM])
        .contains("No work is waiting at a human gate for this matter."));
    assert_eq!(fs::read(plan(tmp.path())).unwrap(), before);
}

/// The reproducer's real-gate control, then the same gate beside a person
/// poll that appears first in plan order: its action must still name the gate.
#[test]
fn issue257_real_gate_keeps_its_wording_and_action_beside_a_person_poll() {
    let tmp = site("needs-ruling", "  needs-ruling:\n    gating: true\n");
    let row = stdout(tmp.path(), &["work", "list"]);
    assert!(row.contains(&format!("waiting on you · {AGORA}")), "{row}");
    assert!(!row
        .lines()
        .find(|line| line.contains("Retry window"))
        .unwrap()
        .contains(SHIP));
    let gate_status = status(tmp.path());
    let open = gate_status.open_at.as_ref().unwrap();
    assert_eq!(open.ticket, AGORA);
    assert_eq!(open.state, "needs-ruling");
    let original = gate_status.advance.expect("the gate has an advance");
    assert!(
        original.contains(AGORA) && original.contains("needs-ruling"),
        "{original}"
    );
    let states = root(tmp.path()).join("states.yaml");
    let machine = fs::read_to_string(&states).unwrap();
    fs::write(&states, machine.replace("  done:\n", "  awaiting-ruling:\n    poll:\n      interval: 10m\n      waiting_on: author\n  done:\n")).unwrap();
    let body = fs::read_to_string(plan(tmp.path())).unwrap();
    fs::write(plan(tmp.path()), body.replace(
        &format!("#### Step {AGORA}:"),
        &format!("#### Step fix-gate-1.poll: Wait for the author\n**State:** awaiting-ruling\n\n#### Step {AGORA}:")
    )).unwrap();
    assert_eq!(
        status(tmp.path()).advance.as_deref(),
        Some(original.as_str())
    );
    let reviewed = review(tmp.path());
    let gates = reviewed["gates"].as_array().unwrap();
    assert_eq!(gates.len(), 1, "{reviewed:#}");
    assert_eq!(gates[0]["ticket"], format!("{PLAN}.{AGORA}"));
    assert_eq!(gates[0]["destinations"][0]["state"], "done");
}
