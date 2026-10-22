//! A workflow hand refusal offers a recovery that its input accepts, and an
//! accepted pick explains the target answers it leaves at their defaults
//! (§FS-005-dispatch.19, §FS-005-dispatch.14).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};
use support::*;

const ITEM: &str = "gdev:graal/25705";
const RAW: &str = "codex[xhigh]:openai:gpt-5.6-luna";

/// Reconstruct the report's isolated registry, agent-only roster and fake
/// workflow runtime. The input is recognized either by its manifest mark or
/// by entry listing, never by a guess about its name.
fn watching(marked: bool, with_model: bool, inputs: Value, pin: Value) -> World {
    let world = World::new();
    assert!(git_command()
        .current_dir(world.forest())
        .args(["init", "-q"])
        .status()
        .expect("initialize the project checkout")
        .success());
    world.stub(
        "status-reporter",
        &answering(
            json!({"v": 1, "matters": [{
                "key": ITEM, "kind": "issue", "title": "Review target hand", "state": "open"
            }]}),
            0,
        ),
    );
    let mut target = json!({
        "name": "review_targets", "description": "Reviewers.", "type": "array",
        "required": false, "default": [RAW], "validate": null,
        "items": {"type": "string", "validate": null}
    });
    if marked {
        target["items"]["format"] = json!("execution-target");
    }
    let listing = json!([{
        "name": "code-review", "version": "1.0.0", "source": "built-in",
        "path": "code-review", "description": "Review a code change.", "inputs": [target]
    }]);
    world.stub(
        "advice-runtime",
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
case "$1" in
  templates) cat <<'LISTING'
{listing}
LISTING
    ;;
  instantiate) printf '%s\n' 'dry-run accepted by the fake runtime' ;;
  *) echo "unexpected fake runtime invocation: $*" >&2; exit 2 ;;
esac
"#
        ),
    );
    let mut settings = json!({
        "agents": {"codex": {"command": ["sh"], "modes": {"xhigh": []}}}
    });
    if with_model {
        settings["models"] = json!({"review-model": {
            "provider": "openai", "model": "gpt-5.6-luna", "default_agent": "codex",
            "agents": {"codex": {}}
        }});
    }
    std::fs::create_dir_all(world.path().join(".config/rhei")).unwrap();
    write_json(&world.path().join(".config/rhei/settings.json"), &settings);
    let mut action = json!({
        "id": "code-review", "icon": "r", "description": "review the change",
        "workflow": "code-review", "inputs": inputs
    });
    if !marked {
        action["hands"] = json!(["review_targets"]);
    }
    let mut work = json!({"runner": "advice-runtime"});
    if !pin.is_null() {
        work["hands"] = json!({"code-review": pin});
    }
    world.configure(json!({
        "actions": [action], "work": work,
        "projects": {PROJECT: {"providers": [{
            "provider": "custom-status", "format": "answer", "command": "status-reporter"
        }]}}
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

fn lay(world: &World, extra: &[&str]) -> std::process::Output {
    world
        .ephor()
        .current_dir(world.path())
        .args(["work", "lay", "code-review", "--item", ITEM, "--dry-run"])
        .args(extra)
        .output()
        .expect("work lay runs")
}

fn recovery(marked: bool, object: bool) {
    let world = watching(marked, false, json!({}), Value::Null);
    let value = if object {
        json!([{"agent": "codex", "model": "gpt-5.6-luna", "effort": "xhigh"}])
    } else {
        json!([RAW])
    };
    write_json(
        &world.path().join("values.json"),
        &json!({"review_targets": value}),
    );
    let output = lay(&world, &["--values", "values.json"]);
    assert!(
        !output.status.success(),
        "invalid hand inputs remain refused"
    );
    assert!(
        !world.forest().join("panta").exists(),
        "a refusal writes no plan"
    );
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("review_targets"),
        "refusal must name its input:\n{error}"
    );
    assert!(
        error.contains("<hand>:") || error.contains("<hand-id>"),
        "refusal must explain roster hand-id syntax:\n{error}"
    );
    assert!(
        error.contains("models") && error.contains("profile") && error.contains("agent"),
        "refusal must offer a model profile with an agent carrier in models:\n{error}"
    );
    assert!(
        !error.contains("spell them out as"),
        "a workflow hand input cannot accept the general parser's object remedy:\n{error}"
    );
}

fn agent_pick(marked: bool) {
    let world = watching(marked, false, json!({}), Value::Null);
    let output = lay(&world, &["--hand", "codex:xhigh"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains(&format!("review_targets  [\"{RAW}\"]  (the workflow)")),
        "agent-only pick leaves the displayed workflow default unchanged:\n{text}"
    );
    // The generic per-run note is already present. This regression requires
    // explaining the unchanged target and source, not merely repeating it.
    let notes = text
        .split_once("note:")
        .map(|(_, notes)| notes)
        .unwrap_or("");
    assert!(
        notes.contains("review_targets") && notes.contains("workflow") && notes.contains("default"),
        "accepted agent-only pick must explain the unchanged input and source:\n{text}"
    );
    assert!(
        notes.contains("no model") || notes.contains("agent-only"),
        "explain why this pick supplies no model selector:\n{text}"
    );
    assert!(
        notes.contains("models")
            && notes.contains("profile")
            && notes.contains("--hand")
            && (notes.contains("--set") || notes.contains("--values")),
        "offer a model-carrying pick or an explicit roster-id answer:\n{text}"
    );
}

#[test]
fn malformed_manifest_target_advice() {
    recovery(true, false);
}

#[test]
fn malformed_entry_target_advice() {
    recovery(false, false);
}

#[test]
fn object_manifest_target_advice() {
    recovery(true, true);
}

#[test]
fn object_entry_target_advice() {
    recovery(false, true);
}

#[test]
fn agent_only_manifest_pick_advice() {
    agent_pick(true);
}

#[test]
fn agent_only_entry_pick_advice() {
    agent_pick(false);
}

/// Preservation checks pass before the fix; they constrain its scope rather
/// than demonstrate the diagnostic defect (§FS-005-dispatch.19).
#[test]
fn model_picks_and_explicit_answers_keep_their_binding_and_sources() {
    for marked in [true, false] {
        let world = watching(marked, true, json!({}), Value::Null);
        let output = lay(&world, &["--hand", "review-model:xhigh", "--json"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let view = shaped("work-lay", &output);
        assert_eq!(
            view["answers"],
            json!([{
                "input": "review_targets", "shown": format!("[\"{RAW}\"]"), "from": "the hand"
            }])
        );
        write_json(
            &world.path().join("values.json"),
            &json!({"review_targets": ["review-model:xhigh"]}),
        );
        let output = lay(&world, &["--values", "values.json", "--json"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            shaped("work-lay", &output)["answers"][0]["from"],
            "a values file"
        );
        let output = lay(&world, &["--hand", "codex:xhigh", "--json"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            shaped("work-lay", &output)["answers"],
            json!([{
                "input": "review_targets", "shown": format!("[\"{RAW}\"]"), "from": "the workflow"
            }])
        );
    }
}

/// Object pins remain valid in the configuration surface that accepts them
/// (§FS-005-dispatch.14), even without a registered model profile.
#[test]
fn a_configuration_object_pin_still_supplies_the_model() {
    let world = watching(
        true,
        false,
        json!({}),
        json!({
            "agent": "codex", "model": "gpt-5.6-luna", "effort": "xhigh"
        }),
    );
    let output = lay(&world, &["--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answer = &shaped("work-lay", &output)["answers"][0];
    assert_eq!(answer["from"], "the hand");
    assert!(answer["shown"].as_str().unwrap().contains("gpt-5.6-luna"));
}
