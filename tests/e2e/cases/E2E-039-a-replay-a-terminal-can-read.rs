//! E2E-039-a-replay-a-terminal-can-read: the replay a person watches is prose,
//! and the document it is also told as stays where it was declared to be.
//!
//! The same scenario as `E2E-038-a-refusal-a-terminal-can-read.rs`, for the
//! other report. §FS-011-command-line.11 is written about *reports* and not
//! about any one command, so it already bound the replay; the replay had
//! simply not been brought under it. `ephor rebase` printed its markdown
//! straight to the terminal — a `#` headline, a `##` section whose whole text
//! was the repository's `.` path, and two rows of backticks around what git
//! said — and so did the sweep that wraps it.
//!
//! What this case holds ephor to, in both commands. What a terminal is handed
//! carries no heading marker and no fence (§FS-011-command-line.11.1); every
//! repository is called what its declaration calls it for a person rather than
//! by the directory a program opens, and the one repository at the root of its
//! checkout is never introduced by a full stop (§FS-011-command-line.11.2);
//! git's own words are kept, indented under the line they belong to. And a
//! replay nested inside the sweep's own report takes the form of the document
//! carrying it (§FS-011-command-line.11.1).
//!
//! What it holds ephor to keeping. The markdown form was never the terminal's
//! and is not lost: the file `--report` writes and the `report` field of
//! `--json` carry the document unchanged, and `repos[].repo` goes on carrying
//! the `.` a program opens a directory with (§FS-011-command-line.7,
//! §REQ-002-parity.4).
//!
//! Every assertion here is about the **form** of what is printed. The
//! sentences are free to improve; what they may not do is arrive as source.

#[path = "../support.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;

use support::*;

/// What the project's one repository is called in the registry that declares
/// it. A role rather than a path, because that is the half of the ticket a
/// name answers (§FS-011-command-line.11.2).
const ROLE: &str = "the project";

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

fn commit(dir: &Path, file: &str, contents: &str, message: &str) {
    std::fs::write(dir.join(file), contents).expect("write the file");
    git(dir, &["add", file]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// A clone of the project's one repository, on `branch`, where the registry's
/// `branch_root_template` puts a checkout of it.
fn checkout_of(world: &World, origin: &Path, at: &str, branch: &str) -> PathBuf {
    let path = world.forest().join(at);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the workspace area");
    let status = Command::new("git")
        .args(["clone", "-q", "--branch", branch])
        .arg(origin)
        .arg(&path)
        .stderr(Stdio::null())
        .status()
        .expect("git clones");
    assert!(status.success(), "cloning {branch} into {at} failed");
    git(&path, &["config", "user.email", "t@example.com"]);
    git(&path, &["config", "user.name", "t"]);
    path
}

/// The world this case is about: one project of one repository at the root of
/// its checkout, named by its role; a branch checkout with uncommitted work,
/// which is the disposition that puts git's own words in a report; and a
/// nested branch checkout whose replay will stop in a conflict, which is the
/// one that gets a replay nested inside the sweep's own report.
struct Scene {
    world: World,
    dirty: PathBuf,
}

fn scene() -> Scene {
    let world = World::new();

    let origin = world.path().join("origin.git");
    let seed = world.path().join("seed");
    std::fs::create_dir_all(&seed).expect("the seed checkout");
    git(&seed, &["init", "-q", "--initial-branch=main"]);
    commit(&seed, "README.md", "the project\n", "the project");
    commit(&seed, "shared.txt", "one\n", "one");
    git(
        &seed,
        &["init", "-q", "--bare", origin.to_str().expect("a path")],
    );
    git(
        &seed,
        &["remote", "add", "origin", origin.to_str().expect("a path")],
    );
    git(&seed, &["push", "-q", "-u", "origin", "main"]);

    // A branch with work on it, and a branch whose work is on the same line
    // `main` then moved — the conflict the sweep stops on.
    git(&seed, &["checkout", "-q", "-b", "fix/x"]);
    commit(&seed, "work.txt", "work\n", "work");
    git(&seed, &["push", "-q", "-u", "origin", "fix/x"]);
    git(&seed, &["checkout", "-q", "-b", "fix/issue-9", "main"]);
    commit(&seed, "shared.txt", "ours\n", "ours");
    git(&seed, &["push", "-q", "-u", "origin", "fix/issue-9"]);
    git(&seed, &["checkout", "-q", "main"]);
    commit(&seed, "shared.txt", "theirs\n", "main moves");
    git(&seed, &["push", "-q", "origin", "main"]);

    checkout_of(&world, &origin, "main", "main");
    let dirty = checkout_of(&world, &origin, "fix-x", "fix/x");
    // Uncommitted work: nothing is touched here, and git's own `M README.md`
    // is what the report quotes.
    std::fs::write(dirty.join("README.md"), "the project\nuncommitted\n").expect("a change");
    let nested = checkout_of(&world, &origin, "fix/issue-9", "fix/issue-9");
    git(&nested, &["fetch", "-q", "origin"]);

    world.register(json!({
        "root": world.forest().to_string_lossy(),
        "main_branch": "main",
        "branch_root_template": "{project_root}/{branch}",
        "branches": []
    }));
    // The registry's own word for the repository, which is what a report has
    // to reach for before it falls back to the path (§FS-011-command-line.11.2).
    let mut registry = world.registry_doc();
    registry["project_types"][0]["repos"][0]["role"] = json!(ROLE);
    registry["project_types"][0]["repos"][0]["default_branch"] = json!("main");
    write_json(&world.registry_path(), &registry);

    // A sweep reports on the checkouts a reading reached, so the world has to
    // have been read once before it can be swept.
    world.ephor().args(["refresh", PROJECT]).assert().success();

    Scene { world, dirty }
}

/// Whether this line is a repository introduced by the directory a program
/// opens rather than by a name a person reads — `## .`, `.`, `.: …`.
fn names_a_repository_by_its_path(line: &str) -> bool {
    let bare = line.trim().trim_start_matches('#').trim();
    bare == "." || bare.starts_with(". ") || bare.starts_with(".:")
}

/// Everything §FS-011-command-line.11.1 forbids a terminal to be handed, and
/// §FS-011-command-line.11.2 forbids any report to call a repository.
fn carries_no_markup(text: &str, what: &str) {
    for line in text.lines() {
        assert!(
            !line.trim_end().starts_with('#'),
            "{what} carries a markdown heading a terminal does not render: {line:?}\n{text}"
        );
        assert!(
            !line.trim_start().starts_with("```"),
            "{what} carries a fence: {line:?}\n{text}"
        );
        assert!(
            !names_a_repository_by_its_path(line),
            "{what} names a repository by its `.` path: {line:?}\n{text}"
        );
    }
    assert!(
        !text.contains("```"),
        "{what} carries a fence somewhere in a line:\n{text}"
    );
}

/// What the document a program stores has to go on being: the headline, the
/// per-repository heading under the repository's own name, and the fence
/// around what git said (§FS-011-command-line.11.1, §FS-011-command-line.7).
fn is_the_markdown_document(text: &str, what: &str) {
    assert!(
        text.lines().any(|line| line.starts_with("# ")),
        "{what} lost its markdown headline:\n{text}"
    );
    assert!(
        text.lines().any(|line| line.starts_with("## ")),
        "{what} lost its per-repository heading:\n{text}"
    );
}

/// The replay a person watches, and the two surfaces that keep the document.
#[test]
fn a_replay_reaches_a_terminal_as_prose_and_a_file_as_the_document() {
    let scene = scene();
    let world = &scene.world;
    let report = world.path().join("runtime/rebase.md");

    let out = world
        .ephor()
        .args(["rebase", "--project", PROJECT, "--checkout"])
        .arg(&scene.dirty)
        .arg("--report")
        .arg(&report)
        .output()
        .expect("the replay runs");
    let printed = String::from_utf8_lossy(&out.stdout).into_owned();

    // The scenario only means something while git has something to say.
    assert!(
        printed.contains("README.md"),
        "git said nothing about the uncommitted work, so there is no quotation \
         to judge:\n{printed}"
    );

    carries_no_markup(&printed, "what `ephor rebase` printed");
    assert!(
        printed.contains(ROLE),
        "the replay never names the repository `{ROLE}`:\n{printed}"
    );
    // git's own words are kept, and kept underneath the line they belong to
    // rather than fenced off from it.
    for line in printed.lines().filter(|line| line.contains("README.md")) {
        assert!(
            line.starts_with(' ') || line.starts_with('\t'),
            "git's message is not indented under the repository it is about: \
             {line:?}\n{printed}"
        );
    }

    // And the document is where it was declared to be, unchanged in kind.
    let written = std::fs::read_to_string(&report).expect("the report file");
    is_the_markdown_document(&written, "the `--report` file");
    assert!(
        written.contains("## the project — fix/x"),
        "the document stopped naming the repository for its reader:\n{written}"
    );
    assert!(
        written.contains("```"),
        "the document stopped fencing what git said:\n{written}"
    );
}

/// The same replay a program reads: the `report` field is the document, and
/// the machine form goes on carrying the path a program opens a directory
/// with (§FS-011-command-line.7, §REQ-002-parity.4).
#[test]
fn the_machine_form_of_a_replay_keeps_the_document_and_the_path() {
    let scene = scene();

    let out = scene
        .world
        .ephor()
        .args(["rebase", "--project", PROJECT, "--checkout"])
        .arg(&scene.dirty)
        .arg("--json")
        .output()
        .expect("the replay runs");
    let view = json_of(&out);

    let report = view["report"]
        .as_str()
        .expect("a reading carries the report");
    is_the_markdown_document(report, "the `report` field of a reading");
    assert!(
        report.contains("## the project — fix/x"),
        "the `report` field stopped naming the repository for its reader:\n{report}"
    );
    assert_eq!(
        view["repos"][0]["repo"],
        json!("."),
        "the machine form stopped carrying the path a program opens: {view:#}"
    );
}

/// The sweep that wraps the replay has the same two readers and the same
/// rule: its own report is prose on a terminal (§FS-011-command-line.11.1).
#[test]
fn the_sweeps_own_report_reaches_a_terminal_as_prose() {
    let scene = scene();

    let dry = scene
        .world
        .ephor()
        .args(["rebase", "--workspace", PROJECT])
        .output()
        .expect("the sweep runs");
    let said = String::from_utf8_lossy(&dry.stdout).into_owned();

    assert!(
        said.contains("would be replayed"),
        "the sweep found no branch checkout to report on:\n{said}"
    );
    carries_no_markup(&said, "what the sweep printed");
}

/// And one more thing the sweep has to settle, because it carries a report
/// inside its own: a nested replay takes the form of the document carrying
/// it, not the form its own first reader would have asked for
/// (§FS-011-command-line.11.1).
#[test]
fn a_replay_nested_in_a_sweep_follows_the_document_carrying_it() {
    let scene = scene();
    let world = &scene.world;
    let report = world.path().join("runtime/sweep.md");

    let acted = world
        .ephor()
        .args(["rebase", "--workspace", PROJECT, "--act"])
        .arg("--report")
        .arg(&report)
        .output()
        .expect("the sweep runs");
    let printed = String::from_utf8_lossy(&acted.stdout).into_owned();

    assert!(
        printed.contains("conflict"),
        "no checkout stopped in a conflict, so no replay is nested in the \
         sweep's report:\n{printed}"
    );
    carries_no_markup(&printed, "what the acting sweep printed");
    assert!(
        printed.contains(ROLE),
        "the nested replay never names the repository `{ROLE}`:\n{printed}"
    );

    // The document the sweep writes is still a document, nested replay and
    // all.
    let written = std::fs::read_to_string(&report).expect("the sweep's report file");
    is_the_markdown_document(&written, "the sweep's `--report` file");
    assert!(
        written.contains("## the project — fix/issue-9"),
        "the nested replay stopped naming its repository for its reader:\n{written}"
    );
}
