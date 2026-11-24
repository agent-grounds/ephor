//! The guide's `gate` topic in the bound runtime's own plan language
//! (§FS-020-guide.2).
//!
//! What a gate's brief holds and where its answer lands is the runtime's
//! format, so the prose about it lives with the binding that reads it, and
//! nothing above the binding names the runtime (§REQ-001-boundary.5). Every
//! command is spelled with the runner the site binds, so the line an agent
//! copies is the line that runs here. The worked example at the end is held
//! to the runtime it is written for: `the_worked_example_is_one_piece` keeps
//! its files consistent with each other, and the runtime's own `validate`,
//! `gate show` and `gate answer --dry-run` were run over it when it was
//! written; a runtime that refuses it is a defect of this text.

use crate::work::recipe::WorkConfig;

/// Where the bound runner's name goes in the text below.
const RUNNER_SLOT: &str = "@RUNNER@";

/// The contract, before the example.
const CONTRACT: &str = "\
GATE — how a workflow prepares the question its gate asks (@RUNNER@)

A gating state stops a ticket for a person. Give it a `gate:` block and the
person answers a form instead of writing prose. The runtime turns the answers
into the move; ephor only shows the brief and sends the answers back.

Declare it on the gating state, in the machine (states.yaml):
  gate:
    display: <path>     the one file the person reads before answering
    request: <path>     questions the state before the gate writes, as JSON
    questions: [...]    or questions declared here once (never both)
    routes: [...]       which answer leads where
    title, summary      optional; a request's own title and summary win
Paths are relative to the work root and may use {task_id} and {state}.

A question is {id, kind, prompt, required, options, help}, in four kinds:
  boolean   yes or no
  choice    one of `options`, at least two: [{value, label, description}]
  multi     any of `options`, at least one
  text      free text
Each is required unless it says `required: false`. A request also carries
`version: 1` and a one-line `title`; `@RUNNER@ gate schema request` is its
schema. A request the runtime refuses keeps the ticket in the state that wrote
it, so a malformed question never reaches the person.

Routes are `{ to: <state>, when: { <question id>: <value> } }`, tried in order:
the first whose every key matches wins, and one without `when` is the
fallback. `to` must be a transition the gate declares, and a `text` question
cannot be routed on. A gate without routes asks the person the destination.

The answer is `{version: 1, request_digest: <the brief's digest>, answers}`
(`@RUNNER@ gate schema answer`). It is recorded under the work root at
runtime/gates/<ticket>/<state>.<visit>.answer.json, and from the state it moves
the ticket into, `{gate_answer}` is that file's path. Text between
`{if input.gate-answer.exists}` and `{endif}` is kept only once it is answered.
";

/// The example's machine.
const MACHINE: &str = "\
name: release-notes
version: 1
states:
  drafting:
    initial: true
    agent: claude-code
    agent_timeout: 30m
    description: Draft the notes, then the question the gate asks
    instructions: |
      Write the notes to runtime/notes/{task_id}.md and the question to
      runtime/gates/{task_id}.request.json (`@RUNNER@ gate schema request`).
      {if input.gate-answer.exists}
      Redraft as the last ruling, in {gate_answer}, asks.
      {endif}
  review:
    gating: true
    description: A person rules on the draft
    gate:
      display: runtime/notes/{task_id}.md
      request: runtime/gates/{task_id}.request.json
      routes:
        - { to: publishing, when: { approve: true } }
        - { to: drafting, when: { approve: false } }
  publishing:
    agent: claude-code
    agent_timeout: 30m
    description: Publish the notes as ruled
    instructions: |
      Publish the notes as the ruling in {gate_answer} says.
  completed:
    final: true
  cancelled:
    final: true
transitions:
  - { from: drafting, to: review }
  - { from: review, to: publishing }
  - { from: review, to: drafting }
  - { from: publishing, to: completed }
  - { from: \"*\", to: cancelled }
";

/// The request `drafting` writes for ticket `notes.1`.
const REQUEST: &str = r#"{"version": 1, "title": "Publish these release notes?",
 "summary": "The draft is the file shown above.",
 "questions": [
  {"id": "approve", "kind": "boolean", "prompt": "Publish them?"},
  {"id": "tone", "kind": "choice", "prompt": "Which tone?",
   "options": [{"value": "plain"}, {"value": "festive", "label": "Festive"}]},
  {"id": "channels", "kind": "multi", "prompt": "Where?", "required": false,
   "options": [{"value": "blog"}, {"value": "mail", "label": "Mail"}]},
  {"id": "note", "kind": "text", "prompt": "Anything to fix first?",
   "required": false}]}
"#;

/// The plan, with its one ticket already at the gate.
const PLAN: &str = "\
# Rhei: Release notes

## Tasks

### Task 1: Publish the release notes
**State:** review
";

/// The answer an agent would send, with the digest `gate show` printed.
const ANSWER: &str = r#"{"version": 1, "request_digest": "<digest from gate show>",
 "answers": {"approve": true, "tone": "plain", "channels": ["blog"]}}
"#;

/// The commands that check it, the runtime's own and then ephor's.
const CHECK: &str = "\
Check it, from the directory that holds the plan:
  @RUNNER@ validate notes.rhei.md
  @RUNNER@ gate show notes.rhei.md --task notes.1 --json
  @RUNNER@ gate answer notes.rhei.md --task notes.1 --from review \\
      --answers answer.json --dry-run --json
and, once a plan of this machine is about a matter in the feed:
  ephor work review --item <id> --json
  ephor work gate answer --item <id> --ticket <plan>.<n> --from review \\
      --answers answer.json --dry-run
";

/// The `gate` topic, its commands spelled with the bound runner
/// (§FS-020-guide.2).
pub fn gate(config: &WorkConfig) -> String {
    let text = format!(
        "{CONTRACT}\n\
         Worked example. states.yaml:\n{}\n\
         runtime/notes/notes.1.md: the draft, any text.\n\
         runtime/gates/notes.1.request.json, as drafting writes it:\n{}\n\
         notes.rhei.md:\n{}\n\
         answer.json:\n{}\n\
         {CHECK}",
        indent(MACHINE),
        indent(REQUEST),
        indent(PLAN),
        indent(ANSWER),
    );
    text.replace(RUNNER_SLOT, super::runner(config))
}

/// Indent a file of the example by two spaces, as the rest of the text is.
fn indent(file: &str) -> String {
    file.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// The example's files as the runtime reads them, for the test that runs the
/// runtime over them.
#[cfg(test)]
pub(crate) fn example() -> [(&'static str, &'static str); 5] {
    [
        ("states.yaml", MACHINE),
        ("runtime/gates/notes.1.request.json", REQUEST),
        ("runtime/notes/notes.1.md", "# Release notes\n\nA draft.\n"),
        ("notes.rhei.md", PLAN),
        ("answer.json", ANSWER),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The topic names the bound runner wherever it spells a command, and
    /// covers each part §FS-020-guide.2 lists.
    #[test]
    fn the_gate_topic_is_spelled_with_the_bound_runner() {
        let config = WorkConfig {
            runner: Some("acme-runtime".to_string()),
            ..WorkConfig::default()
        };
        let text = gate(&config);
        assert!(!text.contains(RUNNER_SLOT), "{text}");
        assert!(text.contains("acme-runtime gate schema request"), "{text}");
        assert!(
            text.contains("acme-runtime validate notes.rhei.md"),
            "{text}"
        );
        assert!(text.contains("ephor work gate answer"), "{text}");
        for part in [
            "gate:",
            "display:",
            "request:",
            "questions:",
            "boolean",
            "choice",
            "multi",
            "text",
            "routes:",
            "answer.json",
            "{gate_answer}",
        ] {
            assert!(text.contains(part), "{part} missing:\n{text}");
        }
        // Written for the default runner's name; a longer one widens only the
        // lines that spell it.
        let text = gate(&WorkConfig::default());
        assert!(
            text.lines().all(|line| line.chars().count() <= 80),
            "every line fits an 80-column screen:\n{text}"
        );
    }

    /// The example is one piece: the request it writes parses, every route
    /// names a question the request asks, and the answer answers it.
    #[test]
    fn the_worked_example_is_one_piece() {
        let request: serde_json::Value = serde_json::from_str(REQUEST).unwrap();
        let ids: Vec<&str> = request["questions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|question| question["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["approve", "tone", "channels", "note"]);
        let answer: serde_json::Value = serde_json::from_str(ANSWER).unwrap();
        for id in answer["answers"].as_object().unwrap().keys() {
            assert!(ids.contains(&id.as_str()), "{id}");
        }
        assert!(MACHINE.contains("when: { approve: true }"));
        assert!(PLAN.contains("**State:** review"));
    }

    /// The example under a real runtime: its validation, its reading of the
    /// brief and its dry run of the answer all accept it (§FS-020-guide.2).
    /// It needs that runtime, so it runs only when asked:
    /// `EPHOR_GUIDE_RUNTIME=<runner> cargo test -- --ignored the_example_holds`.
    #[test]
    #[ignore = "needs a runtime with the gate verbs; set EPHOR_GUIDE_RUNTIME"]
    fn the_example_holds_under_the_runtime_it_is_written_for() {
        let Some(runner) = std::env::var_os("EPHOR_GUIDE_RUNTIME") else {
            panic!("set EPHOR_GUIDE_RUNTIME to the runner to hold the example to");
        };
        let dir = tempfile::tempdir().unwrap();
        for (path, body) in example() {
            let path = dir.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        let run = |args: &[&str]| {
            let out = std::process::Command::new(&runner)
                .args(args)
                .current_dir(dir.path())
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            assert!(
                out.status.success(),
                "{args:?}: {stdout}{}",
                String::from_utf8_lossy(&out.stderr)
            );
            stdout
        };
        run(&["validate", "notes.rhei.md"]);
        let brief: serde_json::Value = serde_json::from_str(&run(&[
            "gate",
            "show",
            "notes.rhei.md",
            "--task",
            "notes.1",
            "--json",
        ]))
        .unwrap();
        assert_eq!(brief["source"], "request", "{brief}");
        let answer = ANSWER.replace("<digest from gate show>", brief["digest"].as_str().unwrap());
        std::fs::write(dir.path().join("answer.json"), answer).unwrap();
        let verdict: serde_json::Value = serde_json::from_str(&run(&[
            "gate",
            "answer",
            "notes.rhei.md",
            "--task",
            "notes.1",
            "--from",
            "review",
            "--answers",
            "answer.json",
            "--dry-run",
            "--json",
        ]))
        .unwrap();
        assert_eq!(verdict["ok"], true, "{verdict}");
        assert_eq!(verdict["to"], "publishing", "{verdict}");
    }
}
