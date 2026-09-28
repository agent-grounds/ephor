//! E2E-032-the-checkout-a-project-declared: the command a project bound is
//! what makes its branch workspaces, whoever asked for one
//! (§FS-004-quick-actions.7, §FS-006-project-interface.8).
//!
//! The scenario is a site whose repository is large enough that a whole tree
//! per branch is not on: it binds the checkout command ephor documents for
//! exactly that, and the command makes `$EPHOR_WORKSPACE` as a sparse slice
//! and leaves a marker naming who made it. Everything below is one question —
//! did that command run?
//!
//! The reader's key runs it, and that is the half that already works. The two
//! halves that do not are the ones a site cannot work around: the workspace a
//! dispatch mints for a branch-less issue, which is the only maker there is
//! for a matter nobody has cut a branch for, and `ephor checkout` typed by
//! name. A site that declared a sparse slice gets a whole tree from both,
//! silently, and the agent that then runs there stands in a tree the site
//! deliberately did not want it to see.
//!
//! What this case holds ephor to: a dispatch about a branch-less issue leaves
//! the command's marker and the command's slice, with the work store and the
//! plan inside it; `ephor checkout` by name leaves the same; `--from` is
//! refused by name, because the base is then the command's to decide; a second
//! ask does not call the command again and says the workspace is already
//! there; a workspace the command did not actually make is named rather than
//! dispatched into; and with nothing bound every one of those answers is
//! git's, exactly as it was.

#[path = "../support.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use predicates::prelude::*;
use serde_json::json;

use support::*;

/// A forge with one matter of the reader's: an issue, which has no branch
/// until somebody cuts one, and so is the case the dispatch has to mint a
/// workspace for.
const ACME_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${1:?subcommand}" in
  capabilities)
    printf '{"issues":true,"pull_requests":false}'
    ;;
  issues)
    printf '%s' '[
      { "key": "acme/widget#95", "title": "Durations read as seconds",
        "url": "https://acme.example/issue/95",
        "updated_at": "2026-09-20T12:00:00Z", "status": "open" }
    ]'
    ;;
  *)
    printf '[]'
    ;;
esac
"#;

/// The branch the shipped `implement` recipe's template renders for that
/// issue, and the workspace it belongs in (§FS-005-dispatch.25).
const MINTED: &str = "fix/issue-95";

/// That issue in the feed, for a case that asks about the one matter rather
/// than sweeping.
const ITEM: &str = "acmeforge:acme/widget#95";

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} in {}", dir.display());
}

/// The project as a person has it: one repository published on `main`, cloned
/// into the `main` workspace, with the branch checkouts one directory per
/// branch beside it. `src/` is what the site's slice keeps; `README.md` and
/// `huge/` are what it leaves out, so a whole tree is visible as a whole tree.
fn project() -> World {
    let world = World::new();

    let origin = world.path().join("origin");
    std::fs::create_dir_all(origin.join("src")).expect("the remote");
    std::fs::create_dir_all(origin.join("huge")).expect("the part left out");
    git(&origin, &["init", "-q", "--initial-branch=main"]);
    git(&origin, &["config", "user.email", "t@example.com"]);
    git(&origin, &["config", "user.name", "t"]);
    std::fs::write(origin.join("README.md"), "the project\n").expect("a file");
    std::fs::write(origin.join("src/main.rs"), "fn main() {}\n").expect("a file");
    std::fs::write(origin.join("huge/blob.txt"), "left out of a slice\n").expect("a file");
    git(&origin, &["add", "-A"]);
    git(&origin, &["commit", "-q", "-m", "the project"]);

    let main = world.forest().join("main");
    std::fs::create_dir_all(main.parent().expect("a parent")).expect("the project root");
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(&origin)
        .arg(&main)
        .status()
        .expect("git clones");
    assert!(status.success());
    git(&main, &["config", "user.email", "t@example.com"]);
    git(&main, &["config", "user.name", "t"]);

    world.stub("ephor-forge-acmeforge", ACME_FORGE);
    world.register(json!({
        "branch_root_template": "{project_root}/{branch}",
        "branches": []
    }));
    watching(&world, json!({}));
    world
}

/// Watch the forge, with `extra` merged into the project's own configuration —
/// which is where a checkout command is bound, and the only key any of this
/// turns on (§FS-006-project-interface.1).
fn watching(world: &World, extra: serde_json::Value) {
    let mut project = json!({
        "providers": [{ "provider": "acmeforge", "user": "you", "repos": ["widget"] }]
    });
    if let (Some(base), Some(overlay)) = (project.as_object_mut(), extra.as_object()) {
        for (key, value) in overlay {
            base.insert(key.clone(), value.clone());
        }
    }
    world.configure(json!({ "projects": { PROJECT: project } }));
}

/// The site's own checkout, as §FS-006-project-interface.8 describes one: it
/// makes `$EPHOR_WORKSPACE`, as a sparse slice rather than a whole tree, and
/// leaves a marker naming who made it. It also appends a line per call, which
/// is how a case asks how many times it was asked.
fn bind_site_checkout(world: &World) -> PathBuf {
    let log = world.path().join("checkout-calls.log");
    let command = world.stub(
        "site-checkout",
        &format!(
            "#!/usr/bin/env bash\n\
             set -euo pipefail\n\
             : \"${{EPHOR_WORKSPACE:?}}\" \"${{EPHOR_BRANCH:?}}\"\n\
             printf '%s\\n' \"$EPHOR_BRANCH\" >> {log}\n\
             git -C \"$PWD/main\" worktree add --quiet -B \"$EPHOR_BRANCH\" \"$EPHOR_WORKSPACE\" main\n\
             git -C \"$EPHOR_WORKSPACE\" sparse-checkout set --no-cone src\n\
             printf 'made by the project checkout command\\n' > \"$EPHOR_WORKSPACE/.made-by-site-checkout\"\n",
            log = log.display(),
        ),
    );
    watching(
        world,
        json!({ "checkout": { "command": command.to_string_lossy() } }),
    );
    log
}

/// How many times the bound command was asked to make a workspace.
fn calls(log: &Path) -> usize {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

/// The marker the command leaves, and the slice it makes: `src/` kept, the
/// rest of the repository left out. Asserted together, because either one
/// alone could be true of a tree somebody else made.
fn made_by_the_site(workspace: &Path) {
    assert!(
        workspace.join(".made-by-site-checkout").is_file(),
        "the project's checkout command did not make {}: {:?}",
        workspace.display(),
        listing(workspace)
    );
    assert!(
        workspace.join("src/main.rs").is_file(),
        "the slice the command makes keeps src/: {:?}",
        listing(workspace)
    );
    for left_out in ["README.md", "huge"] {
        assert!(
            !workspace.join(left_out).exists(),
            "{left_out} is outside the slice the command makes, so a whole tree was made \
             instead: {:?}",
            listing(workspace)
        );
    }
}

/// What is in a directory, for a failure that has to say what it saw instead.
fn listing(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![format!("{} is not there", dir.display())];
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The one plan a dispatch wrote, inside the store it wrote it into.
fn plan_in(store: &Path) -> PathBuf {
    let plans: Vec<PathBuf> = std::fs::read_dir(store)
        .unwrap_or_else(|err| panic!("no store at {}: {err}", store.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".rhei.md"))
        .collect();
    assert_eq!(
        plans.len(),
        1,
        "one dispatch writes one plan into {}: {:?}",
        store.display(),
        listing(store)
    );
    plans.into_iter().next().expect("the plan")
}

/// The reproducer, at the surface a site cannot work around. `ephor actions
/// run checkout` is not offered about an issue with no branch — correctly,
/// since there is no workspace to make until a branch is named — so the
/// dispatch is the *only* maker of this workspace, and a site that declared a
/// checkout has no other way to get the one it declared.
#[test]
fn a_dispatch_mints_the_workspace_with_the_command_the_project_bound() {
    let world = project();
    let log = bind_site_checkout(&world);

    world.ephor().args(["refresh", PROJECT]).assert().success();
    world.ephor().args(["work", "dispatch"]).assert().success();

    let workspace = world.forest().join(MINTED);
    made_by_the_site(&workspace);
    assert_eq!(
        calls(&log),
        1,
        "the command makes the workspace exactly once"
    );
}

/// And the workspace a bound command made is still owed the store, because
/// the plan the dispatch is about to write lands in it
/// (§FS-004-quick-actions.7.1). A workspace handed back with nowhere for a
/// plan to land would have traded one silence for another.
#[test]
fn the_workspace_a_bound_command_made_holds_the_store_and_the_plan() {
    let world = project();
    bind_site_checkout(&world);

    world.ephor().args(["refresh", PROJECT]).assert().success();
    world.ephor().args(["work", "dispatch"]).assert().success();

    let workspace = world.forest().join(MINTED);
    made_by_the_site(&workspace);
    let store = workspace.join("panta");
    assert!(
        store.is_dir(),
        "no work store in the workspace the command made: {:?}",
        listing(&workspace)
    );
    let plan = plan_in(&store);
    assert!(
        plan.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains("95")),
        "the plan in the store is the one about this issue: {}",
        plan.display()
    );
}

/// The other half of the report: `ephor checkout` typed by name bypasses the
/// bound command too, so this is not a dispatch-only slip. One operation for
/// every caller means the command the reader's key runs is the command the
/// command line runs (§FS-004-quick-actions.7).
#[test]
fn ephor_checkout_by_name_makes_the_workspace_the_command_makes() {
    let world = project();
    let log = bind_site_checkout(&world);

    world
        .ephor()
        .args([
            "checkout",
            "--project",
            PROJECT,
            "--branch",
            "spike/by-hand",
        ])
        .assert()
        .success();

    made_by_the_site(&world.forest().join("spike/by-hand"));
    assert_eq!(calls(&log), 1);
}

/// What a branch is grown from is the bound command's to decide, and ephor has
/// nothing to pass it a base through — so `--from` is refused naming the input
/// it came in on rather than parsed and ignored (§FS-004-quick-actions.7.4).
#[test]
fn from_is_refused_where_the_project_bound_a_checkout_command() {
    let world = project();
    let log = bind_site_checkout(&world);

    world
        .ephor()
        .args([
            "checkout",
            "--project",
            PROJECT,
            "--branch",
            "spike/based",
            "--from",
            "main",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--from"));

    assert_eq!(calls(&log), 0, "nothing was asked to make anything");
    assert!(
        !world.forest().join("spike/based").exists(),
        "a refusal left a directory behind"
    );
}

/// The command is asked only where the workspace is absent
/// (§FS-006-project-interface.8). A directory that is there is never handed
/// back to a command written to create it: the second ask reports the
/// workspace as already checked out, and the command's own record of how many
/// times it ran still says once.
#[test]
fn a_second_ask_does_not_call_the_command_again() {
    let world = project();
    let log = bind_site_checkout(&world);

    world
        .ephor()
        .args(["checkout", "--project", PROJECT, "--branch", "spike/twice"])
        .assert()
        .success();
    assert_eq!(calls(&log), 1);

    world
        .ephor()
        .args(["checkout", "--project", PROJECT, "--branch", "spike/twice"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already checked out"));

    assert_eq!(
        calls(&log),
        1,
        "the command was handed a workspace it had already made"
    );
}

/// *Verified* is more than *the directory is there*
/// (§FS-006-project-interface.8). This is the report's sharpest sentence made
/// into a case: a command that exits 0 having made a directory and no
/// repository is the checkout not made, the declared repository is named, and
/// nothing is dispatched behind it — where today the dispatch would succeed,
/// the workspace would exist, and ephor's own verification would pass.
#[test]
fn a_workspace_the_command_did_not_make_is_named_and_nothing_is_dispatched() {
    let world = project();
    let log = world.path().join("hollow-calls.log");
    let command = world.stub(
        "hollow-checkout",
        &format!(
            "#!/usr/bin/env bash\n\
             set -euo pipefail\n\
             printf '%s\\n' \"$EPHOR_BRANCH\" >> {log}\n\
             mkdir -p \"$EPHOR_WORKSPACE\"\n\
             exit 0\n",
            log = log.display(),
        ),
    );
    watching(
        &world,
        json!({ "checkout": { "command": command.to_string_lossy() } }),
    );

    world.ephor().args(["refresh", PROJECT]).assert().success();
    // Asked about the one matter, because a refusal is this command's answer
    // only where a caller asked about one: a sweep steps over what it cannot
    // reach and says so in its tally (§FS-005-dispatch.12). What the checkout
    // refuses is the same either way, and it is the same call the report behind
    // this ticket made.
    world
        .ephor()
        .args(["work", "dispatch", "--item", ITEM])
        .assert()
        .failure()
        .stderr(predicate::str::contains(MINTED));

    let workspace = world.forest().join(MINTED);
    assert!(
        !workspace.join("panta").exists(),
        "a plan was given somewhere to land in a workspace that was not made: {:?}",
        listing(&workspace)
    );
    assert!(
        !workspace.join("src").exists(),
        "ephor's git filled in a tree the command did not make: {:?}",
        listing(&workspace)
    );
    assert_eq!(calls(&log), 1, "the command was asked once");

    // And it holds on the attempt after that. The directory the refusal left
    // behind is what a workspace is resolved from, so a second dispatch is
    // where *there* and *made* part company: read as checked out, it would mint
    // nothing, ask nobody, and put a store and a plan in a tree holding none of
    // the project's repositories — the same silence one attempt later.
    world
        .ephor()
        .args(["work", "dispatch", "--item", ITEM])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "the repository at its root not on disk there",
        ));

    assert_eq!(
        calls(&log),
        1,
        "the command was handed back the directory its own refusal left behind"
    );
    assert!(
        !workspace.join("panta").exists(),
        "the second dispatch wrote a store behind a checkout that was not made: {:?}",
        listing(&workspace)
    );
    assert!(
        !workspace.join("src").exists(),
        "ephor's git filled in a tree the command did not make: {:?}",
        listing(&workspace)
    );
}

/// A dry run makes nothing and says what it would have made, naming the maker:
/// a note implying ephor's git where the project bound its own command would be
/// describing a run nobody is about to make (§FS-005-dispatch.25). Asserted as
/// the whole sentence rather than a substring — the report behind this ticket
/// read a run of twenty-six spaces in the middle of it, which every substring
/// match in the suite stepped straight over.
#[test]
fn the_dry_run_names_the_command_that_would_make_the_workspace() {
    let world = project();
    let log = bind_site_checkout(&world);
    let command = world.path().join("fakebin").join("site-checkout");

    world.ephor().args(["refresh", PROJECT]).assert().success();
    world
        .ephor()
        .args(["work", "dispatch", "--item", ITEM, "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "note: {MINTED} is not checked out — the dispatch would make {} first, with \
             {PROJECT}'s own checkout command (`{}`).",
            world.forest().join(MINTED).display(),
            command.display(),
        )));

    assert_eq!(calls(&log), 0, "a dry run makes nothing");
    assert!(
        !world.forest().join(MINTED).exists(),
        "a dry run left a workspace behind"
    );
}

/// And the half that must not change with any of it. A project with no
/// checkout command bound keeps git's answer: the whole tree the registry
/// describes, its store, and its plan (§FS-004-quick-actions.7).
#[test]
fn with_nothing_bound_the_answer_is_still_gits() {
    let world = project();

    world.ephor().args(["refresh", PROJECT]).assert().success();
    world.ephor().args(["work", "dispatch"]).assert().success();

    let workspace = world.forest().join(MINTED);
    assert!(
        workspace.join("README.md").is_file(),
        "{:?}",
        listing(&workspace)
    );
    assert!(workspace.join("huge/blob.txt").is_file());
    assert!(!workspace.join(".made-by-site-checkout").exists());
    assert!(plan_in(&workspace.join("panta")).is_file());

    world
        .ephor()
        .args(["checkout", "--project", PROJECT, "--branch", "spike/plain"])
        .assert()
        .success();
    assert!(world.forest().join("spike/plain/README.md").is_file());
}
