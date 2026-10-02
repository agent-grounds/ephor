//! E2E-041-every-idle-checkout-cleaned: each project's own clean verb, asked
//! of every branch checkout nobody is holding (§FS-017-clean).
//!
//! The scenario is a machine whose branch checkouts each keep the build they
//! made, until the disk is full. Ephor knows every checkout and which of them a
//! live run holds; it does not know how a project un-builds, so it asks the
//! project's verb and measures what came back (§FS-017-clean.1).
//!
//! Which checkouts are touched is the point. The main checkout belongs to
//! `ephor update` and is said once and left. A checkout whose tree a live run
//! holds is passed over with the run named, and so is one whose project binds
//! no verb there — no guessed fallback (§FS-017-clean.2). A verb that fails is
//! reported on its row and the sweep goes on (§FS-017-clean.3). And nothing is
//! summoned without `--act` (§FS-011-command-line.10).

#[path = "../support.rs"]
mod support;

use std::fs;
use std::path::PathBuf;

use serde_json::{json, Value};

use support::*;

/// The checkouts, named for what is supposed to happen to each. The failing
/// one is listed first, so a sweep that stopped at it would leave the ones
/// after it untouched.
const FAILS: &str = "fails/here";
const CLEANS: &str = "cleans/here";
const PARKS: &str = "parks/here";
const BARE: &str = "no/verb";
const BUSY: &str = "busy/tree";

/// What every checkout's build leaves behind: enough to measure.
const BUILD: usize = 512 * 1024;

fn checkout(world: &World, branch: &str) -> PathBuf {
    world.forest().join(branch)
}

/// A checkout on disk with a build in it, and optionally its own clean verb.
fn built(world: &World, branch: &str, verb: Option<&str>) {
    let dir = checkout(world, branch);
    fs::create_dir_all(dir.join("target")).expect("the checkout");
    fs::write(dir.join("target/blob"), vec![7u8; BUILD]).expect("the build");
    if let Some(body) = verb {
        world.script(&format!("{branch}/clean.sh"), body);
    }
}

fn has_build(world: &World, branch: &str) -> bool {
    checkout(world, branch).join("target/blob").exists()
}

/// The registry row: one checkout per branch under the project root, the main
/// one among them.
fn registered(world: &World, branches: &[&str]) {
    let declared: Vec<Value> = branches
        .iter()
        .map(|branch| json!({ "id": branch, "branch": branch, "active": true }))
        .collect();
    world.register(json!({
        "branch_root_template": "{project_root}/{branch}",
        "branches": declared
    }));
}

/// A run holding a checkout's tree, the way the runtime holds one
/// (§FS-005-dispatch.24): the work root's lock, kept while the handle lives.
fn a_live_run_in(world: &World, branch: &str) -> fs::File {
    let root = checkout(world, branch).join("panta");
    fs::create_dir_all(root.join(".rhei")).expect("the work root");
    fs::write(root.join("index.rhei.md"), "# Rhei: held\n").expect("a plan");
    fs::write(root.join(".rhei/run.lock"), "").expect("the lock file");
    let holder = fs::File::open(root.join(".rhei/run.lock")).expect("open the lock");
    holder.lock().expect("hold it");
    holder
}

fn row<'a>(reading: &'a Value, branch: &str) -> &'a Value {
    reading["checkouts"]
        .as_array()
        .expect("checkouts")
        .iter()
        .find(|row| row["branch"] == branch)
        .unwrap_or_else(|| panic!("no row for {branch}:\n{reading:#}"))
}

/// A machine with five branch checkouts and a main one, each with a build.
fn a_machine_full_of_builds() -> World {
    let world = World::new();
    registered(&world, &["main", FAILS, CLEANS, PARKS, BARE, BUSY]);
    let removes = "#!/bin/sh\nrm -rf target\n";
    built(&world, "main", Some(removes));
    built(
        &world,
        FAILS,
        Some("#!/bin/sh\necho 'cannot clean' >&2\nexit 3\n"),
    );
    // It is told what a summons about a branch is told, and runs in the
    // checkout (§FS-017-clean.1).
    built(
        &world,
        CLEANS,
        Some(
            "#!/bin/sh\nprintf '%s|%s|%s\\n' \"$EPHOR_PROJECT\" \"$EPHOR_BRANCH\" \"$PWD\" \
             > \"$HOME/told.txt\"\nrm -rf target\n",
        ),
    );
    built(&world, PARKS, Some("#!/bin/sh\nexit 75\n"));
    built(&world, BARE, None);
    built(&world, BUSY, Some(removes));
    world
}

/// The whole of it: nothing is summoned without the word, and under the word
/// exactly the idle checkouts with a verb are cleaned.
#[test]
fn the_sweep_reports_first_and_cleans_only_what_nobody_is_holding() {
    let world = a_machine_full_of_builds();
    let _run = a_live_run_in(&world, BUSY);

    // Without --act: what would be summoned where, and no bytes.
    let held = world
        .ephor_raw()
        .args(["clean", "--json"])
        .output()
        .expect("ran");
    assert!(
        held.status.success(),
        "a sweep that only reported failed:\n{}{}",
        String::from_utf8_lossy(&held.stdout),
        String::from_utf8_lossy(&held.stderr)
    );
    let reading = shaped("clean", &held);
    assert_eq!(reading["gated"], true, "{reading:#}");
    assert!(reading["says"].as_str().unwrap().contains("--act"));
    assert!(reading.get("reclaimed_bytes").is_none(), "{reading:#}");
    assert_eq!(reading["would_clean"], 3, "{reading:#}");
    let would = row(&reading, CLEANS);
    assert_eq!(would["outcome"], "would-clean");
    assert_eq!(would["command"], "./clean.sh");
    assert_eq!(
        would["cwd"],
        checkout(&world, CLEANS).to_string_lossy().as_ref()
    );
    assert!(would.get("reclaimed_bytes").is_none());
    for branch in ["main", FAILS, CLEANS, PARKS, BARE, BUSY] {
        assert!(
            has_build(&world, branch),
            "{branch} was touched by a dry run"
        );
    }

    // The prose says the main checkout once, and the reason for each pass-over.
    let prose = world.ephor_raw().args(["clean"]).output().expect("ran");
    let prose = String::from_utf8_lossy(&prose.stdout).to_string();
    assert_eq!(
        prose
            .matches("the main branch's checkout is `ephor update`'s")
            .count(),
        1,
        "{prose}"
    );
    assert!(prose.contains("would run `./clean.sh`"), "{prose}");

    // With the word: the failing verb is reported and the sweep goes on.
    let acted = world
        .ephor_raw()
        .args(["clean", "--act", "--json"])
        .output()
        .expect("ran");
    assert_eq!(
        acted.status.code(),
        Some(1),
        "a failed verb makes the run exit 1:\n{}",
        String::from_utf8_lossy(&acted.stdout)
    );
    let reading = shaped("clean", &acted);
    assert!(reading.get("gated").is_none(), "{reading:#}");

    let failed = row(&reading, FAILS);
    assert_eq!(failed["outcome"], "failed", "{reading:#}");
    assert!(failed["says"].as_str().unwrap().contains("failed (3)"));
    assert!(has_build(&world, FAILS));

    let cleaned = row(&reading, CLEANS);
    assert_eq!(cleaned["outcome"], "cleaned", "{reading:#}");
    assert!(!has_build(&world, CLEANS));
    let reclaimed = cleaned["reclaimed_bytes"].as_u64().unwrap();
    assert!(reclaimed >= BUILD as u64, "{reading:#}");
    assert_eq!(
        cleaned["before_bytes"].as_u64().unwrap() - cleaned["after_bytes"].as_u64().unwrap(),
        reclaimed
    );
    assert!(reading["reclaimed_bytes"].as_u64().unwrap() >= reclaimed);
    assert!(cleaned["says"].as_str().unwrap().contains("KiB reclaimed"));
    let told = fs::read_to_string(world.path().join("told.txt")).expect("the verb ran");
    assert_eq!(
        told.trim(),
        format!("{PROJECT}|{CLEANS}|{}", checkout(&world, CLEANS).display())
    );

    assert_eq!(row(&reading, PARKS)["outcome"], "parked", "{reading:#}");

    let bare = row(&reading, BARE);
    assert_eq!(bare["outcome"], "passed-over");
    assert!(bare["says"]
        .as_str()
        .unwrap()
        .contains("no clean verb declared"));
    assert!(has_build(&world, BARE));

    let busy = row(&reading, BUSY);
    assert_eq!(busy["outcome"], "passed-over");
    assert!(busy["says"]
        .as_str()
        .unwrap()
        .contains("a live run holds this checkout"));
    assert!(has_build(&world, BUSY));

    // The main checkout is not a row and is not touched.
    assert!(reading["checkouts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["branch"] != "main"));
    assert!(has_build(&world, "main"));
}

/// The binding is the check verbs': site configuration over manifest over
/// probe, resolved in the checkout (§FS-017-clean.1).
#[test]
fn the_site_wins_over_the_manifest_and_the_manifest_over_the_probe() {
    let world = World::new();
    const ALL: &str = "all/three";
    registered(&world, &["main", ALL]);
    built(&world, ALL, Some("#!/bin/sh\ntouch probe-ran\n"));
    world.script(
        &format!("{ALL}/ci/clean.sh"),
        "#!/bin/sh\ntouch manifest-ran\n",
    );
    world.file(
        &format!("{ALL}/ephor.json"),
        r#"{ "clean": "./ci/clean.sh" }"#,
    );
    world.configure(json!({ "projects": { PROJECT: { "clean": "touch site-ran" } } }));
    let ran = |marker: &str| checkout(&world, ALL).join(marker).exists();
    let clean = |world: &World| {
        let out = world
            .ephor_raw()
            .args(["clean", "--act", "--json"])
            .output()
            .expect("ran");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        shaped("clean", &out)
    };

    let reading = clean(&world);
    assert_eq!(row(&reading, ALL)["command"], "touch site-ran");
    assert!(ran("site-ran") && !ran("manifest-ran") && !ran("probe-ran"));

    world.configure(json!({}));
    let reading = clean(&world);
    assert_eq!(row(&reading, ALL)["command"], "./ci/clean.sh");
    assert!(ran("manifest-ran") && !ran("probe-ran"));

    fs::remove_file(checkout(&world, ALL).join("ephor.json")).unwrap();
    let reading = clean(&world);
    assert_eq!(row(&reading, ALL)["command"], "./clean.sh");
    assert!(ran("probe-ran"));
}

/// No verb, no guess: the *cleanable* rung says so in the words a passed-over
/// checkout does, and holds once the project binds one (§FS-017-clean.1).
#[test]
fn the_cleanable_rung_says_no_clean_verb_is_declared() {
    let world = World::new();
    // What answered is read from the last refresh's cache.
    world.ephor().args(["refresh", PROJECT]).assert().success();
    let cleanable = |world: &World| -> Value {
        let out = world
            .ephor_raw()
            .args(["capabilities", "--json"])
            .output()
            .expect("ran");
        let reading = shaped("capabilities", &out);
        reading["projects"][0].clone()
    };

    let without = cleanable(&world);
    let missing = without["missing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|rung| rung["rung"] == "cleanable")
        .unwrap_or_else(|| panic!("cleanable is not missing:\n{without:#}"));
    assert!(missing["why"]
        .as_str()
        .unwrap()
        .contains("no clean verb declared"));

    world.script("clean.sh", "#!/bin/sh\n");
    let with = cleanable(&world);
    assert!(
        with["held"]
            .as_array()
            .unwrap()
            .iter()
            .any(|rung| rung == "cleanable"),
        "{with:#}"
    );
}
