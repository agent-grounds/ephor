//! The gate verbs in the runner's own words, and the floor beneath them
//! (§FS-005-dispatch.9.3, §AR-007-runtime.3).

use super::*;
use crate::work::runtime::human_gate::review_plan;
use std::os::unix::fs::PermissionsExt;

/// A runner that answers `states` with one gating state and, unless told it
/// is an older one, the gate verbs: `show` with `brief.json`, `answer` with
/// `verdict.json`, and a copy of the answers it was handed.
const RUNNER: &str = r#"#!/bin/sh
here=$(dirname "$0")
case "$1" in
  states) cat machine.json ;;
  gate)
    [ -f "$here/older" ] && { echo "error: unrecognized subcommand 'gate'" >&2; exit 2; }
    case "$2" in
      schema) echo '{"version":1,"schemas":{}}' ;;
      show)
        printf '%s\n' "$@" > show-invocation
        if [ -f no-brief ]; then echo 'review.1 is in accepted, which is not a gate' >&2; exit 1; fi
        cat brief.json ;;
      answer)
        printf '%s\n' "$@" > answer-invocation
        while [ $# -gt 0 ]; do
          if [ "$1" = --answers ]; then cp "$2" answers-seen.json; printf '%s\n' "$2" > answers-path; fi
          shift
        done
        cat verdict.json
        [ -f refuse ] && exit 1
        exit 0 ;;
    esac ;;
esac
"#;

struct World {
    dir: tempfile::TempDir,
    config: WorkConfig,
    path: PathBuf,
}

impl World {
    /// A world whose runner has the gate verbs, or is `older` and has none.
    fn new(older: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let runner = root.join("runtime-stub");
        std::fs::write(&runner, RUNNER).unwrap();
        std::fs::set_permissions(&runner, std::fs::Permissions::from_mode(0o755)).unwrap();
        if older {
            std::fs::write(root.join("older"), "").unwrap();
        }
        let config = WorkConfig {
            runner: Some(runner.display().to_string()),
            ..WorkConfig::default()
        };
        let path = root.join("review.rhei.md");
        std::fs::write(
            &path,
            "# Rhei: Review\n**States:** review\n\n## Tasks\n\n### Task 1: Decide\n**State:** waiting\n",
        )
        .unwrap();
        std::fs::write(
            root.join("states.yaml"),
            "name: review\nstates:\n  waiting:\n    gating: true\n  accepted:\n    final: true\n",
        )
        .unwrap();
        std::fs::write(
            root.join("machine.json"),
            serde_json::json!({
                "states": [{"name": "waiting", "gating": true}, {"name": "accepted", "final": true}],
                "transitions": [{"from": "waiting", "to": "accepted"}]
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            root.join("brief.json"),
            serde_json::json!({
                "version": 1, "ticket": "review.1", "ticket_title": "Decide",
                "title": "Rule on the proposal", "state": "waiting", "visit": 2,
                "since": null, "plan": path, "root": root, "source": "request",
                "summary": "Read the proposal, then rule.",
                "display": {"path": "proposal.md", "absolute": root.join("proposal.md"),
                            "readable": true, "problem": null},
                "attachments": [],
                "questions": [
                    {"id": "accept", "kind": "boolean", "prompt": "Accept the proposal?",
                     "required": true},
                    {"id": "note", "kind": "text", "prompt": "Anything to carry forward?",
                     "required": false, "help": "One line is enough."}
                ],
                "destinations": [{"state": "accepted", "description": null,
                                  "condition": null, "final": true}],
                "routes": [{"to": "accepted", "when": {"accept": true}}],
                "digest": "sha256:feed",
                "answer_path": "runtime/gates/review.1/waiting.2.answer.json",
                "problems": [],
                "a_field_a_later_runtime_adds": true
            })
            .to_string(),
        )
        .unwrap();
        Self { dir, config, path }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn review(&self) -> Review {
        review_plan(&self.config, self.root(), &self.path, "review", Some("1")).unwrap()
    }

    fn verdict(&self, verdict: serde_json::Value) {
        std::fs::write(self.root().join("verdict.json"), verdict.to_string()).unwrap();
    }

    fn lines(&self, name: &str) -> Vec<String> {
        std::fs::read_to_string(self.root().join(name))
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

/// The feature is asked of the runner itself, and an older runner without the
/// gate verbs briefs nothing: the review reads exactly as it did, with no
/// field for a brief in its JSON (§AR-005-capabilities.4, §AR-007-runtime.3).
#[test]
fn an_older_runner_briefs_nothing_and_its_review_reads_as_before() {
    let world = World::new(true);
    assert!(!briefs(&world.config));
    let before = serde_json::to_value(world.review()).unwrap();
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    assert_eq!(serde_json::to_value(&review).unwrap(), before);
    assert!(before["gates"][0].get("brief").is_none());
    assert!(before["gates"][0].get("unbriefed").is_none());
    assert!(!world.root().join("show-invocation").exists());
    let refusal = refusal(&world.config).unwrap();
    assert!(refusal.contains("prepares no gate briefs"), "{refusal}");
    assert!(refusal.contains("work transition"), "{refusal}");
    let err = answer(
        &world.config,
        &review.gates[0],
        &serde_json::json!({}),
        true,
    )
    .unwrap_err();
    assert_eq!(err.to_string(), refusal);
    assert!(!world.root().join("answer-invocation").exists());
}

/// A runner with the gate verbs briefs each gate, asked for the plan, the
/// ticket and the machine the review read it under; a field it adds later is
/// passed over, and `since` it does not know stays unknown (§FS-005-dispatch.9.3).
#[test]
fn a_briefing_runner_gives_each_gate_its_brief() {
    let world = World::new(false);
    assert!(briefs(&world.config));
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    let gate = &review.gates[0];
    let brief = gate.brief.as_ref().expect("the gate is briefed");
    assert_eq!(brief.title, "Rule on the proposal");
    assert_eq!(brief.visit, 2);
    assert_eq!(brief.since, None);
    assert_eq!(brief.questions.len(), 2);
    assert!(brief.questions[0].required);
    assert!(!brief.questions[1].required);
    assert!(brief.asks_its_own());
    assert_eq!(gate.unbriefed, None);
    let asked = world.lines("show-invocation");
    assert_eq!(
        asked[..5],
        [
            "gate",
            "show",
            &world.path.display().to_string(),
            "--task",
            "review.1"
        ]
    );
    assert!(asked.contains(&"--state-machine".to_string()), "{asked:?}");
    assert_eq!(asked.last().map(String::as_str), Some("--json"));
}

/// Only the runtime's stand-ins — a destination and a reason — leave a plain
/// transition open; a required question of the workflow's own does not
/// (§FS-005-dispatch.9.3).
#[test]
fn only_the_workflows_own_required_questions_close_the_plain_transition() {
    let world = World::new(false);
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    let mut brief = review.gates[0].brief.clone().unwrap();
    for question in &mut brief.questions {
        question.required = false;
    }
    assert!(!brief.asks_its_own());
    brief.questions = serde_json::from_value(serde_json::json!([
        {"id": "destination", "kind": "choice", "prompt": "Where?", "required": true,
         "options": [{"value": "accepted"}]},
        {"id": "reason", "kind": "text", "prompt": "Why?", "required": true}
    ]))
    .unwrap();
    assert!(!brief.asks_its_own());
}

/// A gate the runner could not brief keeps the runner's own sentence and stays
/// reviewable (§FS-005-dispatch.9.3).
#[test]
fn a_gate_the_runner_could_not_brief_says_why() {
    let world = World::new(false);
    std::fs::write(world.root().join("no-brief"), "").unwrap();
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    let gate = &review.gates[0];
    assert_eq!(gate.brief, None);
    assert!(
        gate.unbriefed.as_deref().unwrap().contains("not a gate"),
        "{:?}",
        gate.unbriefed
    );
    assert_eq!(gate.destinations.len(), 1);
}

/// The answer document reaches the runner unchanged, through a private file
/// that is gone once the runner has answered; the dry run is the same call
/// with one flag more (§FS-005-dispatch.9.3).
#[test]
fn an_answer_is_sent_unchanged_and_leaves_no_file_behind() {
    let world = World::new(false);
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    let gate = &review.gates[0];
    let brief = gate.brief.as_ref().unwrap();
    let mut answers = BTreeMap::new();
    answers.insert("accept".to_string(), Value::Bool(true));
    let sent = document(brief, &answers);
    assert_eq!(
        sent,
        serde_json::json!({"version": 1, "request_digest": "sha256:feed",
                           "answers": {"accept": true}})
    );
    world.verdict(serde_json::json!({
        "version": 1, "ok": true, "dry_run": true, "ticket": "review.1", "from": "waiting",
        "to": "accepted", "route": {"index": 0, "when": {"accept": true}},
        "answer_path": "runtime/gates/review.1/waiting.2.answer.json",
        "result": "Accept the proposal?: yes", "errors": []
    }));
    let verdict = answer(&world.config, gate, &sent, true).unwrap();
    assert!(verdict.ok && verdict.dry_run);
    assert_eq!(verdict.to.as_deref(), Some("accepted"));
    assert_eq!(verdict.says(), None);
    let asked = world.lines("answer-invocation");
    assert_eq!(asked[..2], ["gate", "answer"]);
    let from = asked.iter().position(|arg| arg == "--from").unwrap();
    assert_eq!(asked[from + 1], "waiting");
    assert!(asked.contains(&"--dry-run".to_string()), "{asked:?}");
    assert!(asked.contains(&"--json".to_string()), "{asked:?}");
    let seen: Value = serde_json::from_str(
        &std::fs::read_to_string(world.root().join("answers-seen.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(seen, sent);
    let private = PathBuf::from(world.lines("answers-path").remove(0));
    assert!(
        !private.starts_with(world.root()),
        "the answers never sit beside the plan"
    );
    assert!(!private.exists(), "the private file is removed");
    assert!(
        !private.parent().unwrap().exists(),
        "and so is its directory"
    );

    answer(&world.config, gate, &sent, false).unwrap();
    assert!(!world
        .lines("answer-invocation")
        .contains(&"--dry-run".to_string()));
}

/// A refusal is a verdict, read from standard output although the runner
/// exits non-zero, with every field it leaves unknown read as unknown; output
/// that is no verdict at all is the runner's sentence (§FS-005-dispatch.9.3).
#[test]
fn a_refusal_is_the_runners_verdict_with_its_unknowns_left_unknown() {
    let world = World::new(false);
    let review = world.review();
    let gate = &review.gates[0];
    std::fs::write(world.root().join("refuse"), "").unwrap();
    world.verdict(serde_json::json!({
        "version": 1, "ok": false, "dry_run": true, "ticket": "review.1", "from": "waiting",
        "to": null, "route": null, "answer_path": null, "result": null,
        "errors": [
            {"kind": "invalid", "question": "accept", "message": "a required question is unanswered"},
            {"kind": "stale", "question": null, "message": "the brief changed since it was read"}
        ]
    }));
    let verdict = answer(&world.config, gate, &serde_json::json!({}), true).unwrap();
    assert!(!verdict.ok);
    assert_eq!((verdict.to.as_ref(), verdict.route.as_ref()), (None, None));
    assert_eq!(verdict.answer_path, None);
    assert_eq!(verdict.errors[0].question.as_deref(), Some("accept"));
    assert_eq!(
        verdict.says().unwrap(),
        "accept: a required question is unanswered; the brief changed since it was read"
    );

    std::fs::write(world.root().join("verdict.json"), "Segmentation fault\n").unwrap();
    let err = answer(&world.config, gate, &serde_json::json!({}), true).unwrap_err();
    assert!(err.to_string().contains("Segmentation fault"), "{err}");
}
