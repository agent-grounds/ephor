//! E2E-068-work-states-names-the-projects-machine: `ephor work states` answers
//! for a project, and says which projects override the site's machine
//! (§FS-011-command-line.7, §FS-005-dispatch.6).
//!
//! The scenario is agent-grounds/ephor#259. A site sets one state machine for
//! every project, `work.states`, and a different one for project `demo`,
//! `projects.demo.work.states`. A new work root of `demo` receives demo's own
//! machine (§FS-005-dispatch.6), but the verb printed the site's, could not be
//! told the project, and said nothing about the override — and ephor's own
//! refusals told the reader to redirect that verb into demo's root, which
//! installs the wrong machine.
//!
//! Both machines are the one ephor ships with a marker line on top, so each is
//! valid and each is told apart by its first line. The runner is a name on no
//! PATH, so no agent runtime is asked anything; nothing reaches a network.

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::Path;
use std::process::{Output, Stdio};

use serde_json::{json, Value};
use support::*;

const GLOBAL: &str = "# FIXTURE MACHINE: global\n";
const DEMO: &str = "# FIXTURE MACHINE: project demo\n";

fn git(dir: &Path, args: &[&str]) {
    let status = git_command()
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} in {}", dir.display());
}

/// The machine ephor ships, as the verb prints it on a site that configures
/// none and overrides nothing.
fn shipped() -> String {
    let world = World::new();
    let out = world
        .ephor_raw()
        .args(["work", "states"])
        .output()
        .expect("ran");
    assert!(out.status.success(), "{}", text(&out.stderr));
    text(&out.stdout)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8")
}

fn run(world: &World, args: &[&str]) -> Output {
    world.ephor_raw().args(args).output().expect("ran")
}

fn first_line(out: &Output) -> String {
    text(&out.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn json_states(world: &World, args: &[&str]) -> Value {
    let out = run(world, args);
    assert!(
        out.status.success(),
        "{args:?} failed: {}{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("work states --json is JSON")
}

/// The report's site: `demo` is a git project whose branch checkouts sit one
/// directory per branch, `other` is registered with no work block of its own,
/// the site sets `work.states` to the global machine and `demo` overrides it.
fn site() -> World {
    let world = World::new();
    let machine = shipped();
    fs::write(
        world.path().join("global.states.yaml"),
        format!("{GLOBAL}{machine}"),
    )
    .unwrap();
    fs::write(
        world.path().join("demo.states.yaml"),
        format!("{DEMO}{machine}"),
    )
    .unwrap();

    let origin = world.path().join("origin");
    fs::create_dir_all(&origin).unwrap();
    git(&origin, &["init", "-q", "--initial-branch=main"]);
    git(&origin, &["config", "user.email", "t@example.com"]);
    git(&origin, &["config", "user.name", "t"]);
    fs::write(origin.join("README.md"), "the project\n").unwrap();
    git(&origin, &["add", "README.md"]);
    git(&origin, &["commit", "-q", "-m", "the project"]);
    let main = world.forest().join("main");
    fs::create_dir_all(world.forest()).unwrap();
    let status = git_command()
        .args(["clone", "-q"])
        .arg(&origin)
        .arg(&main)
        .status()
        .expect("git clones");
    assert!(status.success());

    world.register(json!({ "branch_root_template": "{project_root}/{branch}" }));
    let other = world.path().join("other");
    fs::create_dir_all(&other).unwrap();
    let mut registry = world.registry_doc();
    let mut row = registry["projects"][0].clone();
    row["id"] = json!("other");
    row["display_name"] = json!("Other");
    row["root"] = json!(other.to_string_lossy());
    row.as_object_mut().unwrap().remove("branch_root_template");
    registry["projects"].as_array_mut().unwrap().push(row);
    write_json(&world.registry_path(), &registry);

    world.configure(json!({
        "work": {
            "states": world.path().join("global.states.yaml").to_string_lossy(),
            "runner": "ephor-259-no-such-runner"
        },
        "projects": {
            PROJECT: { "work": {
                "states": world.path().join("demo.states.yaml").to_string_lossy()
            } },
            "other": { "providers": [] }
        }
    }));
    world
}

#[test]
#[ignore = "red until #259: work states takes no --project, names no override, and checkout's remedy is the bare form; implement removes this"]
fn work_states_answers_for_the_project_it_is_asked_about() {
    let world = site();

    // With --project, the project's own machine, said to be the project's.
    let demo = run(&world, &["work", "states", "--project", PROJECT]);
    assert_eq!(demo.status.code(), Some(0), "{}", text(&demo.stderr));
    assert_eq!(first_line(&demo), DEMO.trim_end());
    let said = json_states(&world, &["work", "states", "--project", PROJECT, "--json"]);
    assert_eq!(said["project"], json!(PROJECT), "{said}");
    assert_eq!(said["tier"], json!("project"), "{said}");
    assert_eq!(said["source"], json!("configured"), "{said}");
    assert_eq!(
        said["path"],
        json!(world.path().join("demo.states.yaml").to_string_lossy()),
        "{said}"
    );
    assert_eq!(said["states"], json!(text(&demo.stdout)), "{said}");

    // A registered project with no override gets the site's machine.
    let other = run(&world, &["work", "states", "--project", "other"]);
    assert_eq!(other.status.code(), Some(0), "{}", text(&other.stderr));
    assert_eq!(first_line(&other), GLOBAL.trim_end());
    let said = json_states(&world, &["work", "states", "--project", "other", "--json"]);
    assert_eq!(said["tier"], json!("site"), "{said}");
    assert_eq!(said["project"], json!("other"), "{said}");

    // An id ephor knows nothing about is refused, by name.
    let nope = run(&world, &["work", "states", "--project", "nope"]);
    assert_eq!(nope.status.code(), Some(2), "{}", text(&nope.stdout));
    assert!(
        text(&nope.stderr).contains("nope") && !text(&nope.stderr).contains("unexpected argument"),
        "the refusal does not name the unknown project: {}",
        text(&nope.stderr)
    );

    // --project is not a scope selector: the selectors stay refused.
    let selector = run(&world, &["work", "states", "--workspace", PROJECT]);
    assert_eq!(selector.status.code(), Some(2));
}

#[test]
#[ignore = "red until #259: bare work states names no project that overrides the site's machine; implement removes this"]
fn work_states_without_a_project_names_the_projects_that_override_it() {
    let world = site();

    let bare = run(&world, &["work", "states"]);
    assert!(bare.status.success(), "{}", text(&bare.stderr));
    // Standard output is the machine and nothing else: it is what gets redirected.
    assert_eq!(first_line(&bare), GLOBAL.trim_end());
    let global = fs::read_to_string(world.path().join("global.states.yaml")).unwrap();
    assert_eq!(text(&bare.stdout), global);
    let note = text(&bare.stderr);
    assert!(
        note.lines()
            .filter(|line| line.starts_with("note:"))
            .count()
            == 1
            && note.contains(PROJECT)
            && note.contains("ephor work states --project demo"),
        "standard error names no overriding project: {note:?}"
    );

    let said = json_states(&world, &["work", "states", "--json"]);
    assert_eq!(said["project"], Value::Null, "{said}");
    assert_eq!(said["tier"], json!("site"), "{said}");
    assert_eq!(said["source"], json!("configured"), "{said}");
    assert_eq!(
        said["overrides"],
        json!([{
            "project": PROJECT,
            "path": world.path().join("demo.states.yaml").to_string_lossy()
        }]),
        "{said}"
    );
    let asked = json_states(&world, &["work", "states", "--project", PROJECT, "--json"]);
    assert_eq!(asked["overrides"], said["overrides"], "{asked}");
}

#[test]
#[ignore = "red until #259: work states takes no --project, so it cannot print what a checkout installs; implement removes this"]
fn work_states_for_a_project_is_what_its_new_root_receives() {
    let world = site();

    let checkout = run(
        &world,
        &[
            "checkout",
            "--project",
            PROJECT,
            "--branch",
            "main",
            "--json",
        ],
    );
    assert!(checkout.status.success(), "{}", text(&checkout.stderr));
    let installed = world.forest().join("main/panta/states.yaml");
    let installed = fs::read(&installed).expect("checkout installed a machine");
    assert!(
        installed.starts_with(DEMO.as_bytes()),
        "checkout installed another machine"
    );

    let printed = run(&world, &["work", "states", "--project", PROJECT]);
    assert!(printed.status.success(), "{}", text(&printed.stderr));
    assert_eq!(
        printed.stdout, installed,
        "`work states --project demo` is not byte for byte what checkout installed"
    );
}

#[test]
#[ignore = "red until #259: the checkout refusal's remedy redirects the site's machine into demo's root; implement removes this"]
fn the_checkout_refusal_names_the_projects_machine() {
    let world = site();
    // The work root already holds a plan of the reader's own and no machine.
    world.file("main/panta/index.panta.md", "# Panta: main\n");
    world.file(
        "main/panta/hand.rhei.md",
        "# Rhei: hand\n\n### Task one: by hand\n**State:** pending\n\nwork\n",
    );

    let checkout = run(
        &world,
        &[
            "checkout",
            "--project",
            PROJECT,
            "--branch",
            "main",
            "--json",
        ],
    );
    assert!(checkout.status.success(), "{}", text(&checkout.stderr));
    let said = text(&checkout.stdout);
    assert!(said.contains("already holds plans"), "no refusal: {said}");
    assert!(
        said.contains("ephor work states --project demo >"),
        "the remedy would install the site's machine into demo's root: {said}"
    );
    assert!(!world.forest().join("main/panta/states.yaml").exists());
}

#[test]
#[ignore = "red until #259: with no site machine, work states prints the alphabetically first project's; implement removes this"]
fn with_no_site_machine_the_answer_is_the_shipped_one() {
    let world = World::new();
    let machine = shipped();
    for id in ["alpha", "beta"] {
        fs::write(
            world.path().join(format!("{id}.states.yaml")),
            format!("# FIXTURE MACHINE: project {id}\n{machine}"),
        )
        .unwrap();
    }
    let path = |id: &str| world.path().join(format!("{id}.states.yaml"));
    world.configure(json!({
        "projects": {
            "beta": { "providers": [], "work": { "states": path("beta").to_string_lossy() } },
            "alpha": { "providers": [], "work": { "states": path("alpha").to_string_lossy() } }
        }
    }));

    let bare = run(&world, &["work", "states"]);
    assert!(bare.status.success(), "{}", text(&bare.stderr));
    assert_eq!(
        text(&bare.stdout),
        machine,
        "bare work states is not the shipped machine"
    );

    let said = json_states(&world, &["work", "states", "--json"]);
    assert_eq!(said["source"], json!("shipped"), "{said}");
    assert_eq!(said["tier"], json!("shipped"), "{said}");
    assert_eq!(said["path"], Value::Null, "{said}");
    assert_eq!(
        said["overrides"],
        json!([
            { "project": "alpha", "path": path("alpha").to_string_lossy() },
            { "project": "beta", "path": path("beta").to_string_lossy() }
        ]),
        "{said}"
    );

    let beta = run(&world, &["work", "states", "--project", "beta"]);
    assert!(beta.status.success(), "{}", text(&beta.stderr));
    assert_eq!(first_line(&beta), "# FIXTURE MACHINE: project beta");
}

#[test]
#[ignore = "red until #259: work states --help offers no --project; implement removes this"]
fn the_help_says_whose_machine_it_prints() {
    let world = World::new();
    let help = run(&world, &["work", "states", "--help"]);
    assert!(help.status.success());
    let help = text(&help.stdout);
    assert!(help.contains("--project"), "{help}");
    assert!(
        !help.contains("Print the state machine ephor's tickets run under"),
        "the help still claims one machine for every ticket: {help}"
    );
}
