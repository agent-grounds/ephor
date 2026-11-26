//! A live run with nothing it can do is paused, read from the runtime's facts
//! alone (§FS-019-waiting.1.3.1): the lock, the run's witness, the runner's own
//! ready set, and each parked ticket's state and recorded next attempt.

use std::os::unix::fs::PermissionsExt;

use super::*;

/// 2100-01-01T00:00:00Z: a next attempt still ahead whenever the suite runs.
const AHEAD: i64 = 4_102_444_800;

/// 2000-01-01T00:00:00Z: a next attempt that has come.
const PAST: i64 = 946_684_800;

/// A runner that lists nothing beyond the plans on disk, answers its ready set
/// with the root's `ready.json` and fails it where there is none, and leaves
/// `asked-ready` behind whenever the ready set was asked for.
const RUNNER: &str = r#"#!/bin/sh
case "$1" in
  list)
    case " $* " in
      *" --ready "*)
        : > "$2/asked-ready"
        [ -f "$2/ready.json" ] || { echo 'no ready set here' >&2; exit 1; }
        cat "$2/ready.json" ;;
      *) printf '[]' ;;
    esac ;;
  *) exit 2 ;;
esac
"#;

/// The waits a paused run tells apart: a gate, a poll that declares whose
/// answer it awaits, one that declares nobody, and an agent step the run has
/// not taken up.
const MACHINE: &str = concat!(
    "name: m\n",
    "states:\n",
    "  implement:\n    agent: x\n",
    "  needs-human:\n    gating: true\n",
    "  holding:\n    program: [\"bash\", \"ship-pr.sh\", \"hold\"]\n",
    "    poll:\n      interval: 30m\n      waiting_on: a release tag\n",
    "  checks:\n    program: [\"bash\", \"collect.sh\"]\n",
    "    poll:\n      interval: 30m\n",
    "  done:\n    final: true\n",
);

struct Root {
    _dir: tempfile::TempDir,
    root: PathBuf,
    config: WorkConfig,
    group: RootPlans,
}

impl Root {
    /// One root, one plan: `tickets` as `(id, state)`, and the runtime's
    /// record of each poll's next attempt as `(id, state, at)`, written the
    /// way the runtime writes it.
    fn new(tickets: &[(&str, &str)], next: &[(&str, &str, &str)]) -> Root {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("panta");
        fs::create_dir_all(root.join(".rhei")).unwrap();
        fs::write(root.join("states.yaml"), MACHINE).unwrap();
        let runner = dir.path().join("runtime-stub");
        fs::write(&runner, RUNNER).unwrap();
        fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
        // §REQ-003-test-isolation.5: publish the runner only after it can execute.
        crate::seams::summons::settle_executable(&runner);
        let mut front = String::new();
        if !next.is_empty() {
            front.push_str("---\nmetadata:\n  tasks:\n");
            for (id, state, at) in next {
                front.push_str(&format!(
                    "    {id}:\n      pollNextAttemptAt:\n        {state}: {at}\n"
                ));
            }
            front.push_str("---\n\n");
        }
        let tasks: String = tickets
            .iter()
            .map(|(id, state)| format!("### Task {id}: do {id}\n**State:** {state}\n\nwork\n\n"))
            .collect();
        let plan = root.join("widget-42.rhei.md");
        fs::write(
            &plan,
            format!("# Rhei: t\n**States:** m\n\n{front}## Tasks\n\n{tasks}"),
        )
        .unwrap();
        Root {
            group: RootPlans {
                root: root.clone(),
                plans: vec![PlanRef {
                    project: "widget".to_string(),
                    plan_id: "widget-42".to_string(),
                    path: plan,
                    item: Some("forge:widget/42".to_string()),
                    title: "Widen the retry window".to_string(),
                }],
            },
            config: WorkConfig {
                runner: Some(runner.display().to_string()),
                ..WorkConfig::default()
            },
            root,
            _dir: dir,
        }
    }

    /// What the runner answers for its ready set: `[]` for none.
    fn ready(self, answer: &str) -> Root {
        fs::write(self.root.join("ready.json"), answer).unwrap();
        self
    }

    /// The board's one operation on this root while a run holds its lock.
    fn live(&self) -> Operation {
        fs::write(self.root.join(LOCK), "").unwrap();
        let holder = fs::File::open(self.root.join(LOCK)).unwrap();
        holder.lock().unwrap();
        let board = board(&self.config, std::slice::from_ref(&self.group));
        drop(holder);
        assert_eq!(board.operations.len(), 1, "{:?}", board.operations);
        let op = board.operations[0].clone();
        assert!(op.live);
        op
    }

    fn asked_ready(&self) -> bool {
        self.root.join("asked-ready").exists()
    }
}

fn at(secs: i64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(secs, 0)
}

/// A run whose one open ticket polls a release tag half an hour out, and
/// whose runner says nothing is ready, is paused on that poll: what it
/// declares it waits on, and when the runtime will look again.
#[test]
fn a_poll_with_its_next_attempt_ahead_pauses_the_run() {
    let root = Root::new(
        &[("ship", "holding")],
        &[("ship", "holding", &AHEAD.to_string())],
    )
    .ready("[]");
    let op = root.live();
    let paused = op.paused.clone().expect("the run is paused");
    assert_eq!(
        paused.waits,
        vec![Parked {
            plan_id: "widget-42".to_string(),
            ticket: "ship".to_string(),
            state: "holding".to_string(),
            kind: ParkedKind::Poll,
            waiting_on: Some("a release tag".to_string()),
            next_attempt: at(AHEAD),
        }]
    );
    assert_eq!(paused.next_look(), at(AHEAD));
    // Live all the same: paused says what the run is doing, not whether it is
    // there (§FS-005-dispatch.15).
    assert_eq!(op.state(), "paused");
    assert_eq!(op.badges()[0], "paused");
}

/// A gate pauses a run with no next look: nothing but a person moves it.
#[test]
fn a_gate_pauses_the_run_with_no_next_look() {
    let root = Root::new(&[("specify", "needs-human")], &[]).ready("[]");
    let paused = root.live().paused.expect("the run is paused");
    assert_eq!(paused.waits.len(), 1);
    assert_eq!(paused.waits[0].kind, ParkedKind::Gate);
    assert_eq!(paused.waits[0].ticket, "specify");
    assert_eq!(paused.waits[0].waiting_on, None);
    assert_eq!(paused.next_look(), None);
}

/// Gates first, then the polls, each in the walk's order; the next look is
/// the earliest next attempt. A poll that declares nobody is still a wait,
/// and a step the ready set does not hold is neither a wait nor a reason
/// against pausing: it is held back by the work before it.
#[test]
fn a_mix_lists_the_gates_first_and_looks_next_at_the_earliest_poll() {
    let later = AHEAD + 3600;
    let root = Root::new(
        &[
            ("collect", "checks"),
            ("ship", "holding"),
            ("specify", "needs-human"),
            ("implement", "implement"),
            ("old", "done"),
        ],
        &[
            ("collect", "checks", &later.to_string()),
            // The runtime may write the seconds as a string of digits.
            ("ship", "holding", &format!("'{AHEAD}'")),
        ],
    )
    .ready("[]");
    let paused = root.live().paused.expect("the run is paused");
    let waits: Vec<(&str, ParkedKind, Option<&str>)> = paused
        .waits
        .iter()
        .map(|wait| (wait.ticket.as_str(), wait.kind, wait.waiting_on.as_deref()))
        .collect();
    assert_eq!(
        waits,
        vec![
            ("specify", ParkedKind::Gate, None),
            ("collect", ParkedKind::Poll, None),
            ("ship", ParkedKind::Poll, Some("a release tag")),
        ]
    );
    assert_eq!(paused.next_look(), at(AHEAD));
}

/// Ready work is a run about to take a turn, however much else is parked.
#[test]
fn ready_work_keeps_the_run_running() {
    let root = Root::new(
        &[("specify", "needs-human"), ("implement", "implement")],
        &[],
    )
    .ready(r#"[{"id": "widget-42.implement", "state": "implement"}]"#);
    let op = root.live();
    assert_eq!(op.paused, None);
    assert_eq!(op.state(), "running");
    assert!(root.asked_ready());
}

/// A poll whose next attempt has come is a run about to take a turn, and with
/// nothing else parked the runner is not even asked.
#[test]
fn a_poll_that_is_due_keeps_the_run_running() {
    let root = Root::new(
        &[("ship", "holding")],
        &[("ship", "holding", &PAST.to_string())],
    )
    .ready("[]");
    assert_eq!(root.live().paused, None);
    assert!(!root.asked_ready());
}

/// A poll that is due keeps the run between turns even beside a gate: the
/// run is about to take it up, and the runner is not asked.
#[test]
fn a_poll_that_is_due_beside_a_gate_keeps_the_run_running() {
    let root = Root::new(
        &[("specify", "needs-human"), ("ship", "holding")],
        &[("ship", "holding", &PAST.to_string())],
    )
    .ready("[]");
    assert_eq!(root.live().paused, None);
    assert!(!root.asked_ready());
}

/// A poll with no next attempt recorded is not parked, and it is no reason
/// against pausing either: whether it is about to run is the ready set's to
/// say. Beside a gate, with nothing ready, the run waits on the gate alone.
#[test]
fn a_poll_with_nothing_recorded_beside_a_gate_leaves_the_ready_set_to_judge() {
    let root = Root::new(&[("specify", "needs-human"), ("ship", "holding")], &[]).ready("[]");
    let paused = root.live().paused.expect("the run is paused");
    assert_eq!(paused.waits.len(), 1);
    assert_eq!(paused.waits[0].ticket, "specify");
    let ready = Root::new(&[("specify", "needs-human"), ("ship", "holding")], &[])
        .ready(r#"[{"id": "widget-42.ship", "state": "holding"}]"#);
    assert_eq!(ready.live().paused, None);
}

/// A poll with no next attempt recorded, or one written in no spelling the
/// runtime writes, is not known to be waiting: the run reads as it did.
#[test]
fn a_poll_with_no_readable_next_attempt_keeps_the_run_running() {
    let silent = Root::new(&[("ship", "holding")], &[]).ready("[]");
    assert_eq!(silent.live().paused, None);
    let clocked = Root::new(
        &[("ship", "holding")],
        &[("ship", "holding", "'2100-01-01T00:00:00Z'")],
    )
    .ready("[]");
    assert_eq!(clocked.live().paused, None);
}

/// A runner that cannot answer for its ready set has not said that nothing is
/// ready (§FS-019-waiting.1.3.1).
#[test]
fn a_ready_set_nobody_answered_keeps_the_run_running() {
    let root = Root::new(&[("specify", "needs-human")], &[]);
    assert_eq!(root.live().paused, None);
    assert!(root.asked_ready());
    let garbled = Root::new(&[("specify", "needs-human")], &[]).ready("not json");
    assert_eq!(garbled.live().paused, None);
}

/// A run holding a ticket is at work, whatever else is parked, and the runner
/// is not asked.
#[test]
fn a_run_holding_a_ticket_is_not_paused() {
    let root = Root::new(
        &[("specify", "needs-human"), ("implement", "implement")],
        &[],
    )
    .ready("[]");
    fs::create_dir_all(root.root.join("runtime")).unwrap();
    fs::write(
        root.root.join(JOURNAL),
        "2026-10-21T10:00:00Z  implement  start@implement  runtime/logs/task-implement-implement.log\n",
    )
    .unwrap();
    let op = root.live();
    assert!(op
        .tickets
        .iter()
        .any(|ticket| matches!(ticket.doing, Doing::Running)));
    assert_eq!(op.paused, None);
    assert!(!root.asked_ready());
}

/// A root no run holds is never paused: its gate waits on the reader with no
/// run behind it (§FS-005-dispatch.15).
#[test]
fn a_root_no_run_holds_is_not_paused() {
    let root = Root::new(&[("specify", "needs-human")], &[]).ready("[]");
    let board = board(&root.config, std::slice::from_ref(&root.group));
    let op = &board.operations[0];
    assert!(!op.live);
    assert_eq!(op.paused, None);
    assert_eq!(op.state(), "waiting on you");
    assert!(!root.asked_ready());
}

/// The runtime's record is read under the task's own id, and under the
/// plan-qualified one where that is all there is.
#[test]
fn the_next_attempt_is_read_under_either_spelling_of_the_task() {
    let root = Root::new(
        &[("ship", "holding")],
        &[("widget-42.ship", "holding", &AHEAD.to_string())],
    );
    let plan = Plan::read(&root.group.plans[0].path).unwrap().unwrap();
    assert_eq!(
        plan.poll_next_attempt("widget-42", "ship", "holding"),
        Some(AHEAD)
    );
    assert_eq!(plan.poll_next_attempt("widget-42", "ship", "checks"), None);
    assert_eq!(
        plan.poll_next_attempt("widget-42", "other", "holding"),
        None
    );
}
