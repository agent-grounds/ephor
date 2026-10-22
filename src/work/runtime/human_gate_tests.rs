//! Gate choices, local materials and refused moves (§FS-005-dispatch.9.1).

use super::*;
use std::os::unix::fs::PermissionsExt;

struct World {
    dir: tempfile::TempDir,
    config: WorkConfig,
    path: PathBuf,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let runner = root.join("runtime-stub");
        std::fs::write(
            &runner,
            r#"#!/bin/sh
if [ "$1" = states ]; then
    cat machine.json
else
    printf '%s\n' "$@" > invocation
    if [ -f refuse ]; then
        echo 'callback refused: proposal changed' >&2
        exit 1
    fi
fi
"#,
        )
        .unwrap();
        std::fs::set_permissions(&runner, std::fs::Permissions::from_mode(0o755)).unwrap();
        let config = WorkConfig {
            runner: Some(runner.display().to_string()),
            ..WorkConfig::default()
        };
        let path = root.join("review.rhei.md");
        std::fs::write(&path, "# Rhei: Review\n**States:** review\n\n## Tasks\n\n### Task 1: Decide\n**State:** waiting\n\n[Proposal](proposal.md) [Online](https://example.invalid/private)\n").unwrap();
        std::fs::write(
            root.join("states.yaml"),
            "name: review\nstates:\n  waiting:\n    gating: true\n  accepted:\n    final: true\n",
        )
        .unwrap();
        std::fs::write(root.join("proposal.md"), "Private local assessment\n").unwrap();
        std::fs::write(root.join("machine.json"), serde_json::json!({
            "states": [
                {"name":"waiting", "gating":true,
                 "inputs":[{"name":"proposal", "path":"proposal.md"}, {"name":"missing", "path":"missing.md", "optional":true}],
                 "outputs":[{"name":"decision", "path":"decisions/{task_id}-{state}.md"}]},
                {"name":"accepted", "description":"Release the reviewed work"},
                {"name":"revise"}
            ],
            "transitions":[
                {"from":"waiting", "to":"accepted"},
                {"from":"waiting", "to":"revise", "exit_code":"nonzero"},
                {"from":"elsewhere", "to":"unrelated"}
            ]
        }).to_string()).unwrap();
        Self { dir, config, path }
    }

    fn review(&self) -> Review {
        review_plan(
            &self.config,
            self.dir.path(),
            &self.path,
            "review",
            Some("1"),
        )
        .unwrap()
    }

    fn item(&self) -> Item {
        Item {
            id: "rhei:review.1".to_string(),
            project: "demo".to_string(),
            source: "rhei".to_string(),
            kind: ItemKind::Task,
            role: None,
            title: "Decide".to_string(),
            url: None,
            state: Some("waiting".to_string()),
            needs_response: true,
            updated_at: chrono::Utc::now(),
            raw: serde_json::json!({"plan":self.path}),
        }
    }
}

#[test]
fn human_gate_choices_and_files_are_read_without_moving_or_sending() {
    let world = World::new();
    let review = world.review();
    let gate = &review.gates[0];
    assert_eq!(gate.ticket, "review.1");
    assert_eq!(gate.destinations.len(), 2);
    assert_eq!(
        gate.destinations[0].description.as_deref(),
        Some("Release the reviewed work")
    );
    assert!(gate.destinations[1]
        .condition
        .as_deref()
        .unwrap()
        .contains("nonzero"));
    let proposal = review
        .files
        .iter()
        .find(|file| file.path.ends_with("proposal.md"))
        .unwrap();
    assert_eq!(read_file(proposal).unwrap(), "Private local assessment\n");
    assert!(review
        .files
        .iter()
        .any(|file| file.path.ends_with("decisions/review.1-waiting.md")));
    let missing = review
        .files
        .iter()
        .find(|file| file.path.ends_with("missing.md"))
        .unwrap();
    assert!(missing.problem.is_some());
    assert!(read_file(missing).is_err());
    assert!(!review
        .files
        .iter()
        .any(|file| file.path.to_string_lossy().contains("https:")));
    assert!(!world.dir.path().join("invocation").exists());
}

#[test]
fn human_gate_transition_keeps_expected_state_machine_reason_and_runtime_refusal() {
    let world = World::new();
    let gate = world.review().gates.remove(0);
    let reason = "reviewed 'exactly'; $(touch injected)";
    transition(&world.config, &gate, "accepted", reason).unwrap();
    let invocation = std::fs::read_to_string(world.dir.path().join("invocation")).unwrap();
    assert!(invocation.contains("--task\nreview.1\n--from\nwaiting\n--to\naccepted\n"));
    assert!(invocation.contains(&format!("--result\n{reason}\n--state-machine\n")));
    assert!(!invocation.contains("Private local assessment"));
    assert!(!world.dir.path().join("injected").exists());
    assert!(!invocation.contains("--force"));
    std::fs::write(world.dir.path().join("refuse"), "").unwrap();
    let error = transition(&world.config, &gate, "accepted", reason).unwrap_err();
    assert!(error
        .to_string()
        .contains("callback refused: proposal changed"));
    assert_eq!(
        Plan::read(&world.path)
            .unwrap()
            .unwrap()
            .ticket("1")
            .unwrap()
            .state
            .as_deref(),
        Some("waiting")
    );
}

#[test]
fn human_gate_undeclared_move_and_stale_read_are_refused() {
    let world = World::new();
    let gate = world.review().gates.remove(0);
    assert!(transition(&world.config, &gate, "not-declared", "because").is_err());
    assert!(!world.dir.path().join("invocation").exists());
    let session = crate::api::Session {
        work_config: world.config.clone(),
        ..Default::default()
    };
    assert!(session
        .transition_work(&world.item(), "review.1", "waiting", "accepted", "  ")
        .is_err());
    let error = session
        .transition_work(
            &world.item(),
            "review.1",
            "old-state",
            "accepted",
            "because",
        )
        .unwrap_err();
    assert!(error.to_string().contains("changed"));
    std::fs::write(
        &world.path,
        "# Rhei: Review\n## Tasks\n### Task 1: Decide\n**State:** accepted\n",
    )
    .unwrap();
    assert!(session
        .human_review(&world.item())
        .unwrap()
        .gates
        .is_empty());
    assert!(session
        .transition_work(&world.item(), "review.1", "waiting", "accepted", "because")
        .is_err());
    assert!(!world.dir.path().join("invocation").exists());
}

#[test]
fn human_gate_missing_plan_and_invalid_machine_are_failures_not_empty_reviews() {
    let world = World::new();
    std::fs::write(world.dir.path().join("machine.json"), "not json").unwrap();
    assert!(review_plan(&world.config, world.dir.path(), &world.path, "review", None).is_err());
    std::fs::remove_file(&world.path).unwrap();
    assert!(source_plan(&world.item()).is_err());
}

#[test]
fn human_gate_directory_workspace_keeps_artifacts_under_its_execution_root() {
    let world = World::new();
    let own = world.dir.path().join("nested");
    std::fs::create_dir_all(own.join("tasks")).unwrap();
    let index = own.join("index.rhei.md");
    std::fs::write(&index, "# Rhei: Nested\n**States:** review\n").unwrap();
    std::fs::write(
        own.join("tasks/1.md"),
        "### Task 1: Decide\n**State:** waiting\n\n[Notes](../notes.md)\n",
    )
    .unwrap();
    std::fs::copy(
        world.dir.path().join("machine.json"),
        own.join("machine.json"),
    )
    .unwrap();
    std::fs::copy(
        world.dir.path().join("states.yaml"),
        own.join("states.yaml"),
    )
    .unwrap();
    std::fs::write(own.join("notes.md"), "Nested local notes").unwrap();
    let review = review_plan(&world.config, world.dir.path(), &index, "nested", None).unwrap();
    assert_eq!(review.gates[0].root, own);
    assert_eq!(
        review.gates[0].machine.as_deref(),
        Some(own.join("states.yaml").as_path())
    );
    assert!(review
        .files
        .iter()
        .any(|file| file.path == own.join("proposal.md")));
    let notes = review
        .files
        .iter()
        .find(|file| file.path.ends_with("notes.md"))
        .unwrap();
    assert_eq!(
        notes.path.canonicalize().unwrap(),
        own.join("notes.md").canonicalize().unwrap()
    );
    assert_eq!(read_file(notes).unwrap(), "Nested local notes");
}

#[test]
fn human_gate_file_reader_bounds_text_and_refuses_binary_directory_and_fifo() {
    let world = World::new();
    let file = Artifact {
        name: "long".to_string(),
        path: world.dir.path().join("long.md"),
        problem: None,
    };
    let content = format!("{}é", "x".repeat(FILE_LIMIT - 1));
    std::fs::write(&file.path, content).unwrap();
    assert!(read_file(&file)
        .unwrap()
        .ends_with("[Preview truncated at 256 KiB]"));
    std::fs::write(&file.path, b"binary\0").unwrap();
    assert!(read_file(&file).is_err());
    let directory = Artifact {
        path: world.dir.path().to_path_buf(),
        ..file.clone()
    };
    assert!(read_file(&directory).is_err());
    std::fs::remove_file(&file.path).unwrap();
    let path = std::ffi::CString::new(file.path.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    assert!(read_file(&file).is_err());
}

#[test]
fn human_gate_authored_work_is_reviewed_through_its_original_matter() {
    let world = World::new();
    Plan::create(
        &world.path,
        "review",
        "Review",
        "Local dossier",
        &plan::Ticket {
            id: "1".to_string(),
            title: "Decide".to_string(),
            state: "waiting".to_string(),
            prior: None,
            target: None,
            model: None,
            body: "[Proposal](proposal.md)".to_string(),
        },
    )
    .save()
    .unwrap();
    assert!(plan::task_store_plans_in(world.dir.path())
        .unwrap()
        .is_empty());
    let mut item = world.item();
    item.id = "forge:demo/17".to_string();
    item.source = "forge".to_string();
    item.kind = ItemKind::Pr;
    item.raw = serde_json::json!({});
    let entry = crate::work::ledger::Entry {
        project: item.project.clone(),
        title: item.title.clone(),
        url: None,
        root: world.dir.path().to_path_buf(),
        checkout: world.dir.path().to_path_buf(),
        branch: None,
        plan_id: "review".to_string(),
        plan: world.path.clone(),
        dispatches: Vec::new(),
        pool: None,
    };
    let mut ledger = crate::work::ledger::Ledger::default();
    ledger.entries.insert(item.id.clone(), entry);
    let dispatcher = crate::work::Dispatcher {
        reply_config: Default::default(),
        registry_doc: serde_json::json!({"projects":[]}),
        global: world.config.clone(),
        projects: Default::default(),
        organizations: Default::default(),
        placements: Default::default(),
        behind: Default::default(),
        rosters: Default::default(),
        workflows: Default::default(),
        actions: Vec::new(),
        project_actions: Default::default(),
        checkouts: Default::default(),
        notes: Vec::new(),
        journal: Default::default(),
        ledger,
    };
    let session = crate::api::Session {
        work_config: world.config.clone(),
        dispatcher: Some(dispatcher),
        ..Default::default()
    };
    let review = session.human_review(&item).unwrap();
    assert_eq!(review.gates.len(), 1);
    assert_eq!(review.gates[0].ticket, "review.1");
    session
        .transition_work(&item, "review.1", "waiting", "accepted", "reviewed locally")
        .unwrap();
    let invocation = std::fs::read_to_string(world.dir.path().join("invocation")).unwrap();
    assert!(invocation.contains("--task\nreview.1\n--from\nwaiting\n--to\naccepted\n"));
}

/// Finished blocked tickets are read beside the gates, never as one: after
/// every gate's materials, in plan order, verdict then result, a missing
/// result carrying its problem; a final ticket with another verdict, none, or a
/// cancellation adds nothing (§FS-005-dispatch.9.1, §FS-005-dispatch.9.2).
#[test]
fn finished_blocked_tickets_are_listed_after_the_gates_and_never_gated() {
    let world = World::new();
    let root = world.dir.path();
    let mut machine: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("machine.json")).unwrap()).unwrap();
    machine["states"][1]["final"] = true.into();
    machine["states"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"name": "cancelled", "final": true}));
    std::fs::write(root.join("machine.json"), machine.to_string()).unwrap();
    std::fs::write(
        &world.path,
        "# Rhei: Review\n**States:** review\n\n## Tasks\n\n\
         ### Task 0: Earlier\n**State:** accepted\n\n\
         ### Task 1: Decide\n**State:** waiting\n\n\
         ### Task 2: Other\n**State:** accepted\n\n\
         ### Task 3: Plain\n**State:** accepted\n\n\
         ### Task 4: Taken back\n**State:** cancelled\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("runtime/ephor")).unwrap();
    std::fs::create_dir_all(root.join("runtime/results")).unwrap();
    for (ticket, verdict) in [
        ("0", "blocked — VPN"),
        ("2", "blocked — VPN"),
        ("3", "done — ok"),
        ("4", "blocked — VPN"),
    ] {
        std::fs::write(
            root.join(format!("runtime/ephor/review.{ticket}.verdict.md")),
            format!("VERDICT: {verdict}\n"),
        )
        .unwrap();
    }
    std::fs::write(
        root.join("runtime/results/review.0.md"),
        "## Result\n\nVPN\n",
    )
    .unwrap();

    let review = review_plan(&world.config, root, &world.path, "review", None)
        .unwrap()
        .settled();
    assert_eq!(review.gates.len(), 1);
    assert_eq!(review.gates[0].ticket, "review.1");
    let names: Vec<_> = review.files.iter().map(|file| file.name.as_str()).collect();
    let history = names
        .iter()
        .position(|name| name.contains("finished blocked"))
        .unwrap();
    assert!(names[..history]
        .iter()
        .all(|name| name.starts_with("review.1 · ")));
    assert_eq!(
        names[history..],
        [
            "review.0 · finished blocked · verdict",
            "review.0 · finished blocked · result",
            "review.2 · finished blocked · verdict",
            "review.2 · finished blocked · result",
        ]
    );
    let files = &review.files[history..];
    assert!(files[..3].iter().all(|file| file.problem.is_none()));
    assert!(files[3].problem.is_some(), "the missing result says so");
}
