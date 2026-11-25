//! The gate verbs in the runner's own words, and the floor beneath them
//! (§FS-005-dispatch.9.3, §AR-007-runtime.3).

use super::*;
use crate::work::runtime::human_gate::review_plan;
use std::os::unix::fs::PermissionsExt;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "fixture_readiness_tests.rs"]
mod fixture_readiness;

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
      schema) echo '{"version":1,"schemas":{"request":{"$id":"req"},"answer":{"$id":"ans"},"list":{}}}' ;;
      list)
        printf '%s\n' "$@" > list-invocation
        [ -f no-list ] && { echo 'not a recognized Panta Project' >&2; exit 1; }
        cat listing.json ;;
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
        // §REQ-003-test-isolation.5: publish the runner only after it can execute.
        crate::seams::summons::settle_executable(&runner);
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

/// The listing is one summons per work root, asked of the root itself, and
/// read for each gate's ticket, state, title, start and holds; a start the
/// runner does not know stays unknown (§FS-019-waiting.1.1.1).
#[test]
fn a_root_is_listed_once_in_the_runners_own_words() {
    let world = World::new(false);
    std::fs::write(
        world.root().join("listing.json"),
        serde_json::json!({"version": 1, "root": world.root(), "gates": [
            {"ticket": "review.1", "ticket_title": "Decide", "title": "Rule on the proposal",
             "state": "waiting", "plan": world.path, "root": world.root(), "visit": 2,
             "since": "2026-11-22T10:14:03Z", "source": "request", "required": 1,
             "holds": 6, "problems": []},
            {"ticket": "review.2", "ticket_title": "Wait", "title": "Wait",
             "state": "waiting", "plan": world.path, "root": world.root(), "visit": 1,
             "since": null, "source": "synthesized", "required": 2, "holds": 0,
             "problems": [], "a_field_a_later_runtime_adds": 1}
        ]})
        .to_string(),
    )
    .unwrap();
    let gates = list(&world.config, world.root()).unwrap();
    assert_eq!(gates.len(), 2);
    assert_eq!(gates[0].ticket, "review.1");
    assert_eq!(gates[0].title, "Rule on the proposal");
    assert_eq!(gates[0].since.as_deref(), Some("2026-11-22T10:14:03Z"));
    assert_eq!(gates[0].holds, 6);
    assert_eq!(gates[1].since, None);
    assert_eq!(
        world.lines("list-invocation"),
        [
            "gate",
            "list",
            &world.root().display().to_string(),
            "--json"
        ]
    );

    // A root the runner cannot list is the runner's sentence, and a listing
    // in a version this binding does not read is refused, not guessed at.
    std::fs::write(world.root().join("no-list"), "").unwrap();
    let err = list(&world.config, world.root()).unwrap_err();
    assert!(err.to_string().contains("not a recognized"), "{err}");
    std::fs::remove_file(world.root().join("no-list")).unwrap();
    std::fs::write(
        world.root().join("listing.json"),
        r#"{"version": 2, "root": "/", "gates": []}"#,
    )
    .unwrap();
    let err = list(&world.config, world.root()).unwrap_err();
    assert!(err.to_string().contains("version 2"), "{err}");
}

/// The guide's schemas are the runner's, printed as it gave them; a runner
/// without the gate verbs has none, and the guide is told why
/// (§FS-020-guide.4).
#[test]
fn the_schemas_are_the_runners_own_or_a_sentence_saying_why_not() {
    let world = World::new(false);
    let (request, answer) = schemas(&world.config).unwrap();
    assert_eq!(request, serde_json::json!({"$id": "req"}));
    assert_eq!(answer, serde_json::json!({"$id": "ans"}));

    let older = World::new(true);
    let why = schemas(&older.config).unwrap_err();
    assert!(why.contains("prepares no gate briefs"), "{why}");

    let unbound = WorkConfig {
        runner: Some(older.root().join("no-such-runner").display().to_string()),
        ..WorkConfig::default()
    };
    assert!(schemas(&unbound).is_err());
}

/// A review asks the runner to list each briefed gate's plan once, under the
/// machine it read the plan with, and a gate takes what it holds from the
/// listing's entry for its ticket in its state; a gate the listing does not
/// name, or a plan it cannot list, carries no count rather than zero
/// (§FS-005-dispatch.9.3).
#[test]
fn a_review_counts_what_each_briefed_gate_holds_from_one_listing_of_its_plan() {
    let world = World::new(false);
    let listing = |state: &str, holds: u64| {
        serde_json::json!({"version": 1, "root": world.root(), "gates": [
            {"ticket": "review.1", "ticket_title": "Decide", "title": "Rule on the proposal",
             "state": state, "plan": world.path, "root": world.root(), "visit": 2,
             "since": null, "source": "request", "required": 1, "holds": holds,
             "problems": []}
        ]})
        .to_string()
    };
    std::fs::write(world.root().join("listing.json"), listing("waiting", 6)).unwrap();
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    assert_eq!(review.gates[0].holds, Some(6));
    let value = serde_json::to_value(&review).unwrap();
    assert_eq!(value["gates"][0]["holds"], 6);
    assert!(value["gates"][0].get("files").is_none(), "{value}");
    let asked = world.lines("list-invocation");
    assert_eq!(
        asked[..3],
        ["gate", "list", &world.path.display().to_string()]
    );
    let machine = asked
        .iter()
        .position(|arg| arg == "--state-machine")
        .unwrap();
    assert_eq!(
        asked[machine + 1],
        world.root().join("states.yaml").display().to_string()
    );
    assert_eq!(asked.last().map(String::as_str), Some("--json"));

    // Named in another state, or not listed at all: no count.
    std::fs::write(world.root().join("listing.json"), listing("accepted", 6)).unwrap();
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    assert_eq!(review.gates[0].holds, None);
    std::fs::write(world.root().join("no-list"), "").unwrap();
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    assert_eq!(review.gates[0].holds, None);
    assert!(serde_json::to_value(&review).unwrap()["gates"][0]
        .get("holds")
        .is_none());
}

/// `held` lists each plan once however many of its gates are briefed, and
/// never lists for a gate the runner could not brief (§FS-005-dispatch.9.3).
#[test]
fn a_plan_is_listed_once_for_all_its_briefed_gates() {
    let world = World::new(false);
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    let mut second = review.gates[0].clone();
    second.ticket = "review.2".to_string();
    let mut unbriefed = review.gates[0].clone();
    unbriefed.ticket = "review.3".to_string();
    unbriefed.brief = None;
    review.gates.push(second);
    review.gates.push(unbriefed);
    let asked = std::cell::Cell::new(0);
    held(&mut review, &|_| {
        asked.set(asked.get() + 1);
        Some(vec![
            Listed {
                ticket: "review.1".into(),
                state: "waiting".into(),
                title: String::new(),
                since: None,
                holds: 2,
            },
            Listed {
                ticket: "review.3".into(),
                state: "waiting".into(),
                title: String::new(),
                since: None,
                holds: 9,
            },
        ])
    });
    assert_eq!(asked.get(), 1);
    let holds: Vec<Option<u64>> = review.gates.iter().map(|gate| gate.holds).collect();
    assert_eq!(holds, [Some(2), None, None]);
}

/// Every document the runner prints is read in the one version this binding
/// reads: a brief in another is refused, so its gate stays unbriefed with
/// that sentence, and so is a verdict; a verdict in the version read is kept
/// whole, every field the runner gave (§AR-007-runtime.1, §FS-005-dispatch.9.3).
#[test]
fn a_brief_or_verdict_in_another_version_is_refused_and_a_verdict_is_kept_whole() {
    let world = World::new(false);
    let brief: Value =
        serde_json::from_str(&std::fs::read_to_string(world.root().join("brief.json")).unwrap())
            .unwrap();
    let mut later = brief.clone();
    later["version"] = 2.into();
    std::fs::write(world.root().join("brief.json"), later.to_string()).unwrap();
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    assert_eq!(review.gates[0].brief, None);
    let unbriefed = review.gates[0].unbriefed.clone().unwrap();
    assert!(unbriefed.contains("version 2"), "{unbriefed}");
    let mut unversioned = brief;
    unversioned.as_object_mut().unwrap().remove("version");
    std::fs::write(world.root().join("brief.json"), unversioned.to_string()).unwrap();
    let mut review = world.review();
    brief_review(&world.config, &mut review);
    assert!(review.gates[0]
        .unbriefed
        .as_deref()
        .unwrap()
        .contains("no version"));

    let gate = &review.gates[0];
    let verdict = serde_json::json!({
        "version": 1, "ok": true, "dry_run": true, "ticket": "review.1", "from": "waiting",
        "to": "accepted", "route": null, "answer_path": "runtime/gates/review.1/waiting.2.answer.json",
        "result": "Accept the proposal?: yes", "errors": [], "a_field_a_later_runtime_adds": 1
    });
    world.verdict(verdict.clone());
    let read = answer(&world.config, gate, &serde_json::json!({}), true).unwrap();
    assert_eq!(read.whole, verdict);
    let mut later = verdict;
    later["version"] = 2.into();
    world.verdict(later);
    let err = answer(&world.config, gate, &serde_json::json!({}), true).unwrap_err();
    assert!(err.to_string().contains("version 2"), "{err}");
}

/// Two gates of one plan each keep every file the review found for them,
/// the plan and machine they share included, though the review lists each
/// file once (§FS-005-dispatch.9.3).
#[test]
fn two_gates_of_one_plan_each_keep_the_files_they_share() {
    let world = World::new(false);
    std::fs::write(
        &world.path,
        "# Rhei: Review\n**States:** review\n\n## Tasks\n\n### Task 1: Decide\n**State:** waiting\n\n### Task 2: Decide again\n**State:** waiting\n",
    )
    .unwrap();
    let review = review_plan(&world.config, world.root(), &world.path, "review", None).unwrap();
    assert_eq!(review.gates.len(), 2);
    let machine = world.root().join("states.yaml");
    for gate in &review.gates {
        assert!(gate.files.contains(&world.path), "{:?}", gate.files);
        assert!(gate.files.contains(&machine), "{:?}", gate.files);
    }
    let named: Vec<&str> = review
        .files
        .iter()
        .filter(|file| file.path == world.path)
        .map(|file| file.name.as_str())
        .collect();
    assert_eq!(named, ["review.1 · plan"], "listed once, under the first");
}
