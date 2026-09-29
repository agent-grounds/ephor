//! E2E-037-a-plan-named-before-the-digest: the carry-over happens where ephor
//! is entitled to write, and nowhere else.
//!
//! A plan named before the digest was part of the stem is moved to the name
//! its matter's id renders now (§FS-005-dispatch.3.1). The question this case
//! is about is *when*. Doing it wherever the ledger is read made `ephor work
//! list` rename files in every project, and made a sweep the width gate was
//! holding move eight files across two projects while it printed that nothing
//! had been written — which is the one thing `--act` exists to promise
//! (§FS-011-command-line.10), and the one thing a dry run exists to promise
//! (§FS-005-dispatch.26).
//!
//! So the reading verbs move nothing, and the verbs that write carry the root
//! over first and say what moved. Three things hang off that and are here too:
//! a dry run over a root that is behind still has to report the ticket the
//! real dispatch will write, because a dry run that lies is no better than one
//! that writes; a carry-over that cannot finish leaves the record and the disk
//! agreeing and stops only the matter it is about; and two records of one plan
//! file — the state the collision this digest fixes actually leaves behind —
//! are refused by name rather than resolved by sorting (§FS-005-dispatch.3).

#[path = "../support.rs"]
mod support;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use predicates::prelude::*;
use serde_json::json;
use support::*;

/// A second project beside the first, so a bare sweep is a sweep above one
/// project and the width gate can fire.
const OTHER: &str = "far";

/// A world watching two projects, each with one matter already dispatched —
/// one plan file, one ledger entry, one digested stem apiece.
fn two_projects() -> World {
    let world = World::new();
    world.file("status.txt", "demo is well\n");

    let far = world.path().join(OTHER);
    std::fs::create_dir_all(&far).expect("the other forest");
    std::fs::write(far.join("status.txt"), "far is well\n").expect("its status");

    let mut registry = world.registry_doc();
    let mut row = registry["projects"][0].clone();
    row["id"] = json!(OTHER);
    row["display_name"] = json!("Far");
    row["root"] = json!(far.to_string_lossy());
    registry["projects"]
        .as_array_mut()
        .expect("projects")
        .push(row);
    write_json(&world.registry_path(), &registry);

    let watching =
        json!({ "providers": [{ "provider": "custom-status", "command": "cat status.txt" }] });
    world.configure(json!({
        "work": {
            "recipes": [{
                "id": "look",
                "description": "look at it",
                "brief": "look at {title}",
                "when": { "kinds": ["status"] }
            }]
        },
        "projects": { PROJECT: watching.clone(), OTHER: watching }
    }));
    world.ephor().args(["refresh"]).assert().success();
    for project in [PROJECT, OTHER] {
        world
            .ephor()
            .args(["work", "dispatch", "--project", project])
            .assert()
            .success();
    }
    world
}

/// Ephor's own record of what it handed over, and of what each plan is called.
fn ledger(world: &World) -> PathBuf {
    world.path().join("state/ephor/work.json")
}

/// One matter whose root is named before the digest.
struct Behind {
    id: String,
    was: String,
    now: String,
    root: PathBuf,
}

/// Put both roots back the way an ephor that named plans without the digest
/// left them: the plan, a result and an unposted reply at the stem with the
/// digest cut off, and a ledger recording that stem.
fn rewind(world: &World) -> Vec<Behind> {
    let path = ledger(world);
    let mut record = read_json(&path);
    let mut behind = Vec::new();
    for (id, entry) in record["entries"].as_object_mut().expect("entries") {
        let now = entry["plan_id"].as_str().expect("a plan id").to_string();
        let was = now
            .rsplit_once('-')
            .expect("a digested stem ends in its digest")
            .0
            .to_string();
        let root = PathBuf::from(entry["root"].as_str().expect("a root"));
        std::fs::rename(
            root.join(format!("{now}.rhei.md")),
            root.join(format!("{was}.rhei.md")),
        )
        .expect("the plan goes back to its old name");
        for (dir, name) in [
            ("runtime/results", format!("{was}.look-1.md")),
            ("runtime/ephor", format!("{was}.reply.md")),
        ] {
            std::fs::create_dir_all(root.join(dir)).expect("the directory");
            std::fs::write(root.join(dir).join(name), "what the run left\n").expect("the file");
        }
        entry["plan_id"] = json!(was);
        entry["plan"] = json!(root.join(format!("{was}.rhei.md")).to_string_lossy());
        behind.push(Behind {
            id: id.clone(),
            was,
            now,
            root,
        });
    }
    write_json(&path, &record);
    behind
}

/// Everything a verb that moves nothing has to leave exactly as it found it:
/// every file in both work roots, and the ledger's own bytes and modification
/// time.
#[derive(Debug, PartialEq, Eq)]
struct Untouched {
    files: Vec<String>,
    record: String,
    written: std::time::SystemTime,
}

fn walk(root: &Path, at: &Path, into: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(at) else {
        return;
    };
    for found in entries.flatten() {
        let path = found.path();
        match path.is_dir() {
            true => walk(root, &path, into),
            false => into.push(
                path.strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned(),
            ),
        }
    }
}

fn untouched(world: &World, behind: &[Behind]) -> Untouched {
    let mut files = Vec::new();
    for root in behind.iter().map(|one| &one.root).collect::<BTreeSet<_>>() {
        walk(world.path(), root, &mut files);
    }
    files.sort();
    let path = ledger(world);
    Untouched {
        files,
        record: std::fs::read_to_string(&path).expect("the ledger"),
        written: std::fs::metadata(&path)
            .expect("the ledger")
            .modified()
            .expect("a modification time"),
    }
}

/// The matter in the first project, which the cases below act on while the
/// second project stands by as the unrelated one.
fn first<'a>(world: &World, behind: &'a [Behind]) -> &'a Behind {
    behind
        .iter()
        .find(|one| one.root.starts_with(world.forest()))
        .expect("the first project's matter")
}

/// A command that writes nothing may not rename a plan, and a sweep the width
/// gate is holding may not either (§FS-011-command-line.10,
/// §FS-005-dispatch.26).
#[test]
fn a_reading_verb_and_a_held_sweep_move_nothing() {
    let world = two_projects();
    let behind = rewind(&world);
    let before = untouched(&world, &behind);

    for reading in [
        vec!["work", "list"],
        vec!["work", "list", "--json"],
        vec!["feed"],
    ] {
        world.ephor().args(&reading).assert().success();
        assert_eq!(
            untouched(&world, &behind),
            before,
            "`ephor {}` moved something",
            reading.join(" ")
        );
    }

    // Two projects and no `--act`: the sweep reports. Renaming eight files
    // across both while printing that nothing was written is the worst of
    // both answers.
    world
        .ephor()
        .args(["work", "dispatch", "--again"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Nothing was written"));
    assert_eq!(
        untouched(&world, &behind),
        before,
        "a sweep the width gate was holding carried plans over"
    );

    world
        .ephor()
        .args([
            "work",
            "dispatch",
            "--project",
            PROJECT,
            "--again",
            "--dry-run",
        ])
        .assert()
        .success();
    assert_eq!(
        untouched(&world, &behind),
        before,
        "a dry run carried plans over"
    );
}

/// A dry run over a root that is behind reports the ticket the real dispatch
/// will actually write (§FS-005-dispatch.26, §FS-005-dispatch.3.1).
///
/// The stem it recomputes holds no file yet, because nothing has been carried
/// over; reading only that name would promise a first ticket in a fresh plan
/// where an append to the plan already there is what is due. So it reads the
/// plan the record still names, and says the move it is reporting across.
#[test]
fn a_dry_run_over_a_root_that_is_behind_reports_the_append_it_would_make() {
    let world = two_projects();
    let behind = rewind(&world);
    let one = first(&world, &behind);
    let before = untouched(&world, &behind);

    let promised = json_of(
        &world
            .ephor_raw()
            .args([
                "work",
                "dispatch",
                "--project",
                PROJECT,
                "--item",
                &one.id,
                "--again",
                "--dry-run",
                "--json",
            ])
            .output()
            .expect("ran"),
    );
    // Reporting the truth is only half of it: the other half is that it is
    // still a report (§FS-005-dispatch.26).
    assert_eq!(
        untouched(&world, &behind),
        before,
        "the dry run bought its honest answer by writing"
    );
    let promised_row = &promised["items"][0];
    assert_eq!(promised_row["outcome"], json!("would-open"), "{promised:#}");
    assert!(
        promised["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|note| {
                let note = note.as_str().unwrap_or_default();
                note.contains(&one.was) && note.contains(&one.now)
            }),
        "the dry run did not say the carry-over it was reporting across: {promised:#}"
    );

    let made = json_of(
        &world
            .ephor_raw()
            .args([
                "work",
                "dispatch",
                "--project",
                PROJECT,
                "--item",
                &one.id,
                "--again",
                "--json",
            ])
            .output()
            .expect("ran"),
    );
    let made_row = &made["items"][0];
    assert_eq!(
        promised_row["ticket"], made_row["ticket"],
        "the dry run promised a different ticket than the dispatch wrote:\n{promised:#}\n{made:#}"
    );
    assert_eq!(
        promised_row["plan"], made_row["plan"],
        "the dry run promised a different plan than the dispatch wrote:\n{promised:#}\n{made:#}"
    );
    // An append, not a first ticket: the plan the record named already had one.
    assert_eq!(made_row["ticket"], json!("look-2"), "{made:#}");
}

/// What a carry-over moved is said in prose and in `--json` alike
/// (§FS-005-dispatch.3.1, §REQ-002-parity).
#[test]
fn what_a_carry_over_moved_is_said_in_prose_and_in_json() {
    let world = two_projects();
    let behind = rewind(&world);
    let one = first(&world, &behind);
    let said = format!(
        "carried the plan of {} over from {} to {} in {}",
        one.id,
        one.was,
        one.now,
        one.root.display()
    );

    let reading = json_of(
        &world
            .ephor_raw()
            .args([
                "work",
                "dispatch",
                "--project",
                PROJECT,
                "--item",
                &one.id,
                "--again",
                "--json",
            ])
            .output()
            .expect("ran"),
    );
    assert!(
        reading["notes"]
            .as_array()
            .expect("notes")
            .iter()
            .any(|note| note.as_str() == Some(said.as_str())),
        "the reading does not carry what moved: {reading:#}"
    );

    let behind = rewind(&world);
    let one = first(&world, &behind);
    world
        .ephor()
        .args([
            "work",
            "dispatch",
            "--project",
            PROJECT,
            "--item",
            &one.id,
            "--again",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "carried the plan of {} over from {} to {}",
            one.id, one.was, one.now
        )));
}

/// A carry-over that cannot finish puts back what it moved, stops only the
/// matter it is about, and leaves no record naming a file that is not there
/// (§FS-005-dispatch.3.1, §FS-005-dispatch.4).
#[cfg(unix)]
#[test]
fn a_carry_over_that_fails_leaves_the_record_and_the_disk_agreeing() {
    use std::os::unix::fs::PermissionsExt;

    let world = two_projects();
    let behind = rewind(&world);
    let one = first(&world, &behind);
    let other = behind
        .iter()
        .find(|entry| entry.root != one.root)
        .expect("the other project's matter");
    let results = one.root.join("runtime/results");
    let shut = |mode: u32| {
        std::fs::set_permissions(&results, std::fs::Permissions::from_mode(mode))
            .expect("the results directory's mode");
    };

    shut(0o500);
    world
        .ephor()
        .args(["work", "dispatch", "--act", "--again"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Cannot carry"));

    // The matter whose root could not be carried over is exactly as it was,
    // and its record names the file that is there.
    let record = read_json(&ledger(&world));
    assert_eq!(
        record["entries"][&one.id]["plan_id"],
        json!(one.was),
        "{record:#}"
    );
    assert!(
        one.root.join(format!("{}.rhei.md", one.was)).is_file(),
        "the plan was left somewhere other than where the record says"
    );
    assert!(
        results.join(format!("{}.look-1.md", one.was)).is_file(),
        "the result the rename could not move was not put back"
    );
    // And the unrelated project was carried over anyway: one unreachable
    // directory is no reason to leave every other project behind.
    assert_eq!(
        record["entries"][&other.id]["plan_id"],
        json!(other.now),
        "{record:#}"
    );

    shut(0o700);
    world
        .ephor()
        .args(["work", "dispatch", "--act", "--again"])
        .assert()
        .success();
    let record = read_json(&ledger(&world));
    assert_eq!(
        record["entries"][&one.id]["plan_id"],
        json!(one.now),
        "the root was not carried over once it could be: {record:#}"
    );
    assert!(one.root.join(format!("{}.rhei.md", one.now)).is_file());
    assert!(results.join(format!("{}.look-1.md", one.now)).is_file());
}

/// Two ledger entries recorded at one plan file are refused by name — both
/// matters and the file — and nothing about them is moved
/// (§FS-005-dispatch.3, §FS-005-dispatch.3.1).
///
/// This is the state the collision this digest fixes actually leaves on disk:
/// an older ephor wrote both matters' tickets into one file and recorded both
/// entries at it. Giving the file to whichever matter sorts first would leave
/// the other's record naming a file that is not there, which is the one
/// outcome a rename must not produce. Splitting it is a person's job.
#[test]
fn two_records_of_one_plan_file_are_refused_by_name() {
    let world = two_projects();
    let behind = rewind(&world);
    let one = first(&world, &behind);
    let other = behind
        .iter()
        .find(|entry| entry.root != one.root)
        .expect("the other project's matter");
    let shared = one.root.join(format!("{}.rhei.md", one.was));

    let path = ledger(&world);
    let mut record = read_json(&path);
    record["entries"][&other.id]["root"] = json!(one.root.to_string_lossy());
    record["entries"][&other.id]["plan_id"] = json!(one.was);
    record["entries"][&other.id]["plan"] = json!(shared.to_string_lossy());
    write_json(&path, &record);
    let before = untouched(&world, &behind);

    let refused = world
        .ephor()
        .args(["work", "dispatch", "--act", "--again"])
        .assert()
        .get_output()
        .clone();
    let says = String::from_utf8_lossy(&refused.stdout).into_owned()
        + &String::from_utf8_lossy(&refused.stderr);
    for named in [
        one.id.as_str(),
        other.id.as_str(),
        &shared.to_string_lossy(),
    ] {
        assert!(
            says.contains(named),
            "the refusal does not name {named}:\n{says}"
        );
    }
    assert_eq!(
        untouched(&world, &behind),
        before,
        "the refusal moved something"
    );
    let record = read_json(&path);
    for entry in [one, other] {
        let plan = record["entries"][&entry.id]["plan"]
            .as_str()
            .expect("a recorded plan");
        assert!(
            Path::new(plan).is_file(),
            "the refusal left {} naming a file that is not there: {plan}",
            entry.id
        );
    }
}
