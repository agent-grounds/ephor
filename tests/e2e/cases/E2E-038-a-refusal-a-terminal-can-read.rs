//! E2E-038-a-refusal-a-terminal-can-read: a dispatch that cannot make the
//! workspace says so in words a terminal renders.
//!
//! The scenario is §FS-011-command-line.11 end to end. A recipe names the
//! branch its work belongs on, so the dispatch makes that workspace before it
//! writes anything (§FS-005-dispatch.25, §FS-004-quick-actions.7) — and here
//! git will not, because the project's checkout has no remote to look the
//! branch up on. That refusal is the moment the reader most needs to act, and
//! it is where the checkout report used to arrive as markdown source: a `#`
//! heading folded onto the note's own prefix, a `##` heading whose whole text
//! was the repository's `.` path, and two rows of backticks around the four
//! lines git actually said.
//!
//! What this case holds ephor to: the note a terminal is handed carries no
//! heading marker and no fence, names the repository the way its declaration
//! names it for a person rather than by the path a program opens, keeps git's
//! own words indented underneath, and is the same text the `--json` reading
//! carries for that item — so the prose form and the machine form never know
//! different things (§REQ-002-parity.3).
//!
//! Every assertion here is about the **form** of what is printed. The
//! sentences are free to improve; what they may not do is arrive as source.

#[path = "../support.rs"]
mod support;

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::json;

use support::*;

/// What the project's one repository is called in the registry that declares
/// it. A role rather than a path, because that is the half of the ticket a
/// name answers.
const ROLE: &str = "the project";

/// A forge with one issue of the reader's. An issue has no branch, which is
/// what makes the recipe below mint one.
const ACME_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities)
    printf '{"issues":true}'
    ;;
  issues)
    printf '%s' '[
      { "key": "acme/widget#95", "title": "Durations read as seconds",
        "url": "https://acme.example/issue/95",
        "updated_at": "2026-07-30T12:00:00Z", "status": "open" }
    ]'
    ;;
  *)
    printf '[]'
    ;;
esac
"#;

/// A runtime that is never reached: the checkout is refused before any ticket
/// is written, and a case that left the runner off PATH would be proving that
/// instead.
const ACME_RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
exit 0
"#;

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

/// The project's main branch on disk and **no remote**: a repository git can
/// read and a branch git cannot look up, which is the refusal this case is
/// about.
fn main_checkout_with_no_remote(world: &World) {
    let main = world.forest().join("main");
    std::fs::create_dir_all(&main).expect("the main checkout");
    git(&main, &["init", "-q", "--initial-branch=main"]);
    std::fs::write(main.join("README.md"), "the project\n").expect("a file");
    git(&main, &["add", "README.md"]);
    git(&main, &["commit", "-q", "-m", "the project"]);
}

/// A world watching the forge, whose one repository the registry names for a
/// reader, and whose recipe says which branch its work belongs on.
fn watching() -> World {
    let world = World::new();
    world.stub("ephor-forge-acmeforge", ACME_FORGE);
    world.stub("acme-runtime", ACME_RUNTIME);
    main_checkout_with_no_remote(&world);

    world.register(json!({ "branch_root_template": "{project_root}/{branch}", "branches": [] }));
    // The registry's own word for the repository, which is what a report has
    // to reach for before it falls back to the path (§FS-011-command-line.11.2).
    let mut registry = world.registry_doc();
    registry["project_types"][0]["repos"][0]["role"] = json!(ROLE);
    write_json(&world.registry_path(), &registry);

    world.configure(json!({
        "projects": { PROJECT: { "providers": [
            { "provider": "acmeforge", "user": "you", "repos": ["widget"] }
        ] } },
        "work": {
            "runner": "acme-runtime",
            "recipes": [{
                "id": "fix-issue",
                "icon": "⛬",
                "description": "fix the issue",
                "state": "fix",
                "needs_checkout": true,
                "branch": "fix/issue-{number}",
                "when": { "kinds": ["issue"] },
                "brief": "Fix {title}."
            }]
        }
    }));
    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
}

/// Whether this line is a repository introduced by the directory a program
/// opens rather than by a name a person reads — `## .`, `.`, `.: …`.
fn names_a_repository_by_its_path(line: &str) -> bool {
    let bare = line.trim().trim_start_matches('#').trim();
    bare == "." || bare.starts_with(". ") || bare.starts_with(".:")
}

/// Everything §FS-011-command-line.11.1 forbids a terminal to be handed, said
/// once so both halves of this case ask it of their own surface.
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

/// The refusal a person reads: what git said, framed in ephor's own words
/// rather than in markdown source.
#[test]
fn a_dispatch_that_cannot_make_the_workspace_refuses_in_words_a_terminal_renders() {
    let world = watching();

    let out = world
        .ephor()
        .args(["work", "dispatch", "--project", PROJECT])
        .output()
        .expect("the dispatch runs");
    let said = String::from_utf8_lossy(&out.stderr).into_owned();

    // The scenario only means something while git is the one refusing.
    assert!(
        said.contains("git refused") || said.contains("fatal:"),
        "git did not refuse the checkout, so there is no refusal to judge:\n{said}"
    );

    carries_no_markup(&said, "the note a dispatch printed");

    // The repository is named the way its declaration names it for a person.
    assert!(
        said.contains(ROLE),
        "the refusal never names the repository `{ROLE}`:\n{said}"
    );

    // git's own words are kept, and kept underneath the line they belong to
    // rather than fenced off from it.
    let mut git_said = 0;
    for line in said.lines().filter(|line| line.contains("fatal:")) {
        git_said += 1;
        assert!(
            line.starts_with(' ') || line.starts_with('\t'),
            "git's message is not indented under the repository it refused: {line:?}\n{said}"
        );
    }
    assert!(git_said > 0, "git's own message is gone:\n{said}");
}

/// The same refusal a program reads. `says` on every other outcome of a
/// dispatch is a sentence; the refused one carried a whole document, and after
/// this it carries what the terminal was told, word for word
/// (§FS-011-command-line.7, §REQ-002-parity.3).
#[test]
fn the_machine_form_of_the_refusal_is_the_words_the_terminal_was_given() {
    let world = watching();

    let out = world
        .ephor()
        .args(["work", "dispatch", "--project", PROJECT, "--json"])
        .output()
        .expect("the dispatch runs");
    let view = shaped("work-dispatch", &out);
    assert_eq!(view["refused"], json!(1), "{view:#}");

    let items = view["items"].as_array().expect("the landings are a list");
    let refused = items
        .iter()
        .find(|item| item["outcome"] == "refused")
        .unwrap_or_else(|| panic!("no refused landing in {view:#}"));
    let says = refused["says"].as_str().expect("a refusal says something");

    carries_no_markup(says, "the `says` of a refused landing");
    assert!(
        says.contains(ROLE),
        "the machine form never names the repository `{ROLE}`: {says:?}"
    );

    // The same words, not a second telling of them.
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        said.contains(says.trim_end()),
        "the terminal and the reading carry different words\n\
         terminal:\n{said}\nreading:\n{says}"
    );
}
