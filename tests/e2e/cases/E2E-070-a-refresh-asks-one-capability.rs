//! E2E-070-a-refresh-asks-one-capability: a ticket registered a moment ago is
//! one `issues` call away from the feed (§FS-011-command-line.9.2).
//!
//! The scenario is issue #263's. A project watches `gdev`, a forge extension
//! answering for both its pull requests and its tickets through two
//! configuration blocks, and `acme`, a second forge that reports pull requests
//! only. A branch is registered with the ticket `GR-45248`, and the next step is
//! to lay work on it. Before this change the only way to bring the ticket into
//! the feed was a refresh of the whole project — every pull request with its
//! threads and gate, every branch gate, every notice, from every source — and
//! `work lay` refused the item until all of it had answered
//! (§FS-001-forge-interface.8).
//!
//! What this case pins:
//!
//! - a plain refresh asks and writes exactly what it did before
//!   (`plain_refresh_is_unchanged`, which passes before the change and must
//!   stay green after it);
//! - `refresh --source gdev --only issues` asks gdev's `issues` alone, leaves
//!   every other answer as it was, and does not make the project read fresh
//!   (§FS-001-forge-interface.6.1, §FS-011-command-line.12), so an unheld
//!   rebase sweep still refetches it before it reads (§FS-004-quick-actions.6.1);
//! - a narrowed refresh over identical answers yields identical matters, so
//!   work does not move (§FS-005-dispatch.5);
//! - a history never asked is not an empty answer: matters it would have
//!   reported are not gone, and read marks are not pruned for them, judged from
//!   the project's bindings rather than from what the cache holds;
//! - every refusal of the new flags is decided before any data call.
//!
//! Every forge call lands in one log (`tests/e2e/fixtures/capability-forge.py`),
//! so "what was asked" is read off the log rather than inferred from the feed.

#[path = "../support.rs"]
mod support;

use std::collections::BTreeMap;
use std::process::Output;
use std::thread::sleep;
use std::time::Duration;

use serde_json::{json, Value};

use support::*;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/e2e/fixtures/capability-forge.py"
);
const TICKET: &str = "GR-45248";
const TICKET_ITEM: &str = "gdev:GR-45248";
const GDEV_PR: &str = "gdev:G/7";
const ACME_PR: &str = "acme:app/9";
const ORG: &str = "foundation";

/// The two forges as stubs on `PATH`, each handing its calls to the fixture
/// with its own name, the shared log and the shared control file.
fn install_forges(world: &World) {
    for source in ["gdev", "acme"] {
        world.stub(
            &format!("ephor-forge-{source}"),
            &format!(
                "#!/usr/bin/env bash\nexec python3 '{FIXTURE}' {source} '{}' '{}' \"$@\"\n",
                log_path(world).display(),
                world.path().join("control.json").display(),
            ),
        );
    }
}

/// gdev twice — two blocks that report the same pull request — and acme once,
/// all bound to the project.
fn bound_blocks() -> Value {
    json!([
        { "provider": "gdev", "block": "a", "repos": ["G"] },
        { "provider": "gdev", "block": "b", "repos": ["G"] },
        { "provider": "acme", "repos": ["app"] }
    ])
}

/// The project in an organization, so the rebase sweep can be pointed at it,
/// watching gdev and acme under `ttl` seconds.
fn watching(ttl: u64) -> World {
    let world = World::new();
    world.organize(ORG, "Foundation");
    install_forges(&world);
    world.configure(settings(ttl));
    world
}

/// What [`watching`] configures.
fn settings(ttl: u64) -> Value {
    json!({
        "defaults": { "ttl_seconds": ttl },
        "projects": { PROJECT: { "providers": bound_blocks() } },
        "work": { "recipes": [{
            "id": "answer",
            "description": "answer the review",
            "when": { "kinds": ["pr"] },
            "needs_checkout": false,
            "state": "fix",
            "brief": "Answer {title}."
        }] }
    })
}

/// A runtime offering one workflow about a ticket, which `work lay` can lay
/// down as a dry run: what a reader does next with a ticket just imported.
const TICKET_RUNTIME: &str = r#"#!/usr/bin/env bash
set -euo pipefail
verb="$1"; shift
case "$verb" in
  templates)
    cat <<'JSON'
[ { "name": "work-ticket", "version": "1.0.0", "source": "user",
    "path": "work-ticket", "description": "Work one ticket.",
    "inputs": [ { "name": "ticket", "description": "The ticket.",
      "type": "string", "required": true, "default": null, "validate": null,
      "format": null, "positional": 1, "items": null, "properties": null } ] } ]
JSON
    ;;
  instantiate)
    output=""; dry=""
    while [ "$#" -gt 0 ]; do
      case "$1" in
        --output) output="$2"; shift 2 ;;
        --dry-run) dry=yes; shift ;;
        *) shift ;;
      esac
    done
    [ -n "$dry" ] || { echo "only a dry run is expected here" >&2; exit 1; }
    echo "would render work-ticket into $output"
    ;;
  *) echo "unexpected: $verb" >&2; exit 1 ;;
esac
"#;

fn log_path(world: &World) -> std::path::PathBuf {
    world.path().join("forge.log")
}

/// Every forge call since the log was last cleared, one line each:
/// `<source> <block> <subcommand> [tickets]`.
fn calls(world: &World) -> Vec<String> {
    std::fs::read_to_string(log_path(world))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn clear(world: &World) {
    std::fs::write(log_path(world), "").expect("the call log clears");
}

fn fail(world: &World, sources: &[&str]) {
    write_json(
        &world.path().join("control.json"),
        &json!({ "fail": sources }),
    );
}

/// The subcommands one block asked, in the order it asked them.
fn asked_by(calls: &[String], source: &str, block: &str) -> Vec<String> {
    calls
        .iter()
        .filter_map(|line| {
            let mut words = line.split(' ');
            (words.next() == Some(source) && words.next() == Some(block))
                .then(|| words.next().unwrap_or("").to_string())
        })
        .collect()
}

fn run(world: &World, args: &[&str]) -> Output {
    world.ephor_raw().args(args).output().expect("ephor runs")
}

fn said(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Register a branch carrying the ticket, which is how the ticket reaches the
/// forge's `issues` (§FS-001-forge-interface.2).
fn register_ticket(world: &World) {
    let mut registry = world.registry_doc();
    registry["projects"][0]["branches"] = json!([{
        "id": "fix-gr-45248",
        "branch": format!("fix/{TICKET}"),
        "ticket": TICKET,
        "active": true
    }]);
    write_json(&world.registry_path(), &registry);
}

/// The feed's rows for the project, as a program reads them.
fn rows(world: &World) -> Vec<Value> {
    let output = run(world, &["feed", "--workspace", PROJECT, "--json"]);
    assert!(output.status.success(), "{}", said(&output));
    json_of(&output)
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("feed --json is a list: {}", said(&output)))
}

fn ids(rows: &[Value]) -> Vec<String> {
    let mut ids: Vec<String> = rows
        .iter()
        .map(|row| row["id"].as_str().unwrap_or("").to_string())
        .collect();
    ids.sort();
    ids
}

/// What `feed` says about the project's age on the error stream
/// (§FS-011-command-line.12).
fn age_line(world: &World) -> String {
    let output = run(world, &["feed", "--workspace", PROJECT]);
    assert!(output.status.success(), "{}", said(&output));
    stderr(&output)
}

fn refresh_narrowed(world: &World, extra: &[&str]) -> Output {
    let mut args = vec!["refresh", PROJECT];
    args.extend_from_slice(extra);
    run(world, &args)
}

/// The read marks the reader holds, by matter key.
fn marks(world: &World) -> BTreeMap<String, Value> {
    let path = world.path().join("state/ephor/seen.json");
    serde_json::from_value(read_json(&path)).expect("seen.json is a map")
}

/// Make the cache on disk one written before #263: model 2, the last model
/// that kept one answer per source. A model change drops the file's contents
/// on load and the cache is rebuilt, so what it held does not matter — only
/// that it is older (§FS-001-forge-interface.6.1).
fn age_the_cache_model(world: &World) {
    let path = world
        .path()
        .join("state/ephor/feed")
        .join(format!("{PROJECT}.json"));
    let mut cache = read_json(&path);
    cache["model"] = json!(2);
    write_json(&path, &cache);
}

fn sweep(world: &World) -> Output {
    run(world, &["rebase", "--org", ORG, "--act"])
}

/// A plain refresh asks what it asked before #263, in the same order per
/// block, writes the same rows, prints the same JSON fields and exits the same
/// — and a `status` inside the TTL asks nothing, failing source and all
/// (§FS-011-command-line.9.2: "a plain `ephor refresh` … asks and writes
/// exactly what it did before"). This passes before the change and is the
/// compatibility the change must keep.
#[test]
fn plain_refresh_is_unchanged() {
    let world = watching(600);
    fail(&world, &["acme"]);

    let output = run(&world, &["refresh", PROJECT, "--json"]);
    assert_eq!(output.status.code(), Some(4), "{}", said(&output));

    // The calls: each gdev block asks every capability gdev declares, in the
    // forge interface's order; acme stops at its first failure. Blocks run
    // concurrently, so the order is pinned per block and the whole as a set.
    let made = calls(&world);
    for block in ["a", "b"] {
        assert_eq!(
            asked_by(&made, "gdev", block),
            ["pull-requests", "branch-gates", "issues", "notices"],
            "gdev block {block}: {made:#?}"
        );
    }
    assert_eq!(
        asked_by(&made, "acme", "-"),
        ["capabilities", "pull-requests"]
    );
    assert_eq!(
        asked_by(&made, "gdev", "-"),
        ["capabilities", "capabilities"],
        "{made:#?}"
    );
    assert_eq!(made.len(), 12, "no call beyond these: {made:#?}");

    // The JSON: exactly today's fields, and no `answers` on a plain run
    // (§FS-011-command-line.7).
    let reading = json_of(&output);
    let keys = |value: &Value| -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .unwrap_or_else(|| panic!("an object: {value:#}"))
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    };
    assert_eq!(
        keys(&reading),
        ["degraded", "lost", "notes", "projects", "refreshed"],
        "{reading:#}"
    );
    let project = &reading["projects"][0];
    assert_eq!(
        keys(project),
        ["failures", "items", "lost", "notes", "project"],
        "{reading:#}"
    );
    assert_eq!(reading["refreshed"], 1, "{reading:#}");
    assert_eq!(reading["degraded"], 1, "{reading:#}");
    assert_eq!(project["project"], PROJECT, "{reading:#}");
    assert_eq!(project["items"], 1, "{reading:#}");
    let failures = project["failures"].as_array().expect("failures");
    assert_eq!(failures.len(), 1, "{reading:#}");
    assert!(failures[0].as_str().unwrap_or("").contains("acme"));

    // The rows: the two blocks' reports of one pull request are one row, and
    // the first block's report wins the tie: a slot is read by source, then
    // block, then capability, whichever run wrote it (§FS-001-forge-interface.6.1).
    let feed = rows(&world);
    assert_eq!(ids(&feed), [GDEV_PR], "{feed:#?}");
    assert_eq!(feed[0]["title"], "Retry window (block a)", "{feed:#?}");

    // Inside the TTL, `status` reads the cache and asks nobody, the failing
    // source included (§FS-011-command-line.12).
    clear(&world);
    let status = run(&world, &["status", PROJECT]);
    assert!(status.status.success(), "{}", said(&status));
    assert!(calls(&world).is_empty(), "{:#?}", calls(&world));
}

/// The reproducer. A ticket registered on a branch reaches the feed through
/// one `issues` call to gdev, and nothing else is asked: no pull request, no
/// branch gate, no notice, no acme (§FS-011-command-line.9.2). Everything not
/// asked keeps its answer and its age, so the project still reads as old as
/// its pull requests (§FS-011-command-line.12) and an unheld sweep refetches
/// it before it reads (§FS-004-quick-actions.6.1).
#[test]
fn registered_issue_import_skips_pr_work() {
    let world = watching(3);
    let first = run(&world, &["refresh", PROJECT, "--quiet"]);
    assert!(first.status.success(), "{}", said(&first));
    let acme_before = world.feed()["providers"]["acme"].clone();
    assert!(!acme_before.is_null(), "acme answered: {:#}", world.feed());

    register_ticket(&world);
    let refused = run(&world, &["actions", "--item", TICKET_ITEM]);
    assert_eq!(
        refused.status.code(),
        Some(2),
        "the ticket is not in the feed yet: {}",
        said(&refused)
    );

    // Past the TTL of the full refresh, so whether the narrowed one makes the
    // project read fresh is visible.
    sleep(Duration::from_secs(4));
    clear(&world);
    let narrowed = refresh_narrowed(&world, &["--source", "gdev", "--only", "issues", "--json"]);
    assert_eq!(narrowed.status.code(), Some(0), "{}", said(&narrowed));

    // Only gdev's probe and gdev's `issues`, carrying the registered ticket.
    let made = calls(&world);
    for line in &made {
        assert!(
            line == "gdev - capabilities"
                || line == &format!("gdev a issues {TICKET}")
                || line == &format!("gdev b issues {TICKET}"),
            "a narrowed refresh asked more than gdev's issues: {made:#?}"
        );
    }
    for block in ["a", "b"] {
        assert_eq!(asked_by(&made, "gdev", block), ["issues"], "{made:#?}");
    }

    // The JSON says what was asked (§FS-011-command-line.7).
    let reading = json_of(&narrowed);
    let answers = reading["projects"][0]["answers"]
        .as_array()
        .unwrap_or_else(|| panic!("a narrowed refresh lists its answers: {reading:#}"));
    assert!(
        answers.iter().any(|answer| answer["source"] == "gdev"
            && answer["capability"] == "issues"
            && answer["ok"] == true),
        "{reading:#}"
    );
    assert!(
        answers
            .iter()
            .all(|answer| answer["source"] == "gdev" && answer["capability"] == "issues"),
        "only gdev's issues were asked: {reading:#}"
    );

    // The ticket can be acted on, and nothing else left the feed.
    let actions = run(&world, &["actions", "--item", TICKET_ITEM]);
    assert!(actions.status.success(), "{}", said(&actions));
    // Work can be laid on it: the step this import was for
    // (§FS-001-forge-interface.8).
    world.stub("ticket-runtime", TICKET_RUNTIME);
    let mut runtime = settings(3);
    runtime["work"]["runner"] = json!("ticket-runtime");
    world.configure(runtime);
    let laid = run(
        &world,
        &[
            "work",
            "lay",
            "work-ticket",
            "--item",
            TICKET_ITEM,
            "--set",
            &format!("ticket={TICKET}"),
            "--dry-run",
        ],
    );
    assert!(laid.status.success(), "{}", said(&laid));
    assert_eq!(ids(&rows(&world)), [ACME_PR, GDEV_PR, TICKET_ITEM]);

    // acme was not asked, so its answer is exactly what it was.
    assert_eq!(world.feed()["providers"]["acme"], acme_before);

    // The project is as old as its oldest history — gdev's pull requests and
    // acme, both from the first refresh — so `feed` still says so.
    let age = age_line(&world);
    assert!(
        age.contains(&format!("{PROJECT} fetched")) && age.contains("past its"),
        "a refresh that asked only issues made the project read fresh: {age:?}"
    );

    // And the sweep reads the same age: an unheld sweep refetches the project
    // before it reads, so gdev's pull requests are asked again.
    clear(&world);
    let swept = sweep(&world);
    let made = calls(&world);
    assert!(
        made.iter()
            .any(|line| line.starts_with("gdev a pull-requests")),
        "the sweep trusted pull requests nobody asked for again: {made:#?}\n{}",
        said(&swept)
    );
}

/// A narrowed refresh over the same answers is the same feed: the same rows,
/// the same keys and fingerprints, so work laid on a matter does not move
/// (§FS-001-forge-interface.6.1, §FS-005-dispatch.5).
#[test]
fn narrowed_equals_full() {
    let world = watching(600);
    register_ticket(&world);
    let full = run(&world, &["refresh", PROJECT, "--quiet"]);
    assert!(full.status.success(), "{}", said(&full));
    let dispatched = run(&world, &["work", "dispatch", "--item", GDEV_PR, "--json"]);
    assert!(dispatched.status.success(), "{}", said(&dispatched));
    assert_eq!(json_of(&dispatched)["opened"], 1, "{}", said(&dispatched));
    let before = rows(&world);

    let narrowed = refresh_narrowed(&world, &["--source", "gdev", "--only", "issues"]);
    assert_eq!(narrowed.status.code(), Some(0), "{}", said(&narrowed));
    assert_eq!(
        rows(&world),
        before,
        "the narrowed refresh changed the feed"
    );

    let synced = run(&world, &["work", "sync", "--dry-run", "--json"]);
    assert!(synced.status.success(), "{}", said(&synced));
    let synced = json_of(&synced);
    assert_eq!(synced["reopened"], 0, "{synced:#}");
    assert_eq!(synced["opened"], 0, "{synced:#}");
}

/// A capability never asked is not an empty answer. On a cache rebuilt by the
/// model change, a refresh of gdev's `issues` alone leaves gdev's pull requests
/// and acme unknown: work does not move, read marks are kept, the project reads
/// never fetched, and `status` refetches it (§FS-011-command-line.12; the read
/// marks are §FS-007-matters.5's).
#[test]
fn never_asked_is_not_gone() {
    let world = watching(600);
    register_ticket(&world);
    let full = run(&world, &["refresh", PROJECT, "--quiet"]);
    assert!(full.status.success(), "{}", said(&full));
    let dispatched = run(&world, &["work", "dispatch", "--item", GDEV_PR, "--json"]);
    assert!(dispatched.status.success(), "{}", said(&dispatched));
    let marked = run(&world, &["mark-read", "--all"]);
    assert!(marked.status.success(), "{}", said(&marked));
    assert!(marks(&world).contains_key(GDEV_PR) && marks(&world).contains_key(ACME_PR));

    age_the_cache_model(&world);
    clear(&world);
    let narrowed = refresh_narrowed(&world, &["--source", "gdev", "--only", "issues"]);
    assert_eq!(narrowed.status.code(), Some(0), "{}", said(&narrowed));
    assert!(
        calls(&world)
            .iter()
            .all(|line| line == "gdev - capabilities" || line.contains(" issues")),
        "{:#?}",
        calls(&world)
    );

    // Absent because never asked is not moved and not gone.
    let synced = run(&world, &["work", "sync", "--dry-run", "--json"]);
    assert!(synced.status.success(), "{}", said(&synced));
    assert_eq!(json_of(&synced)["reopened"], 0, "{}", said(&synced));

    let marked = run(&world, &["mark-read", "--all"]);
    assert!(marked.status.success(), "{}", said(&marked));
    let kept = marks(&world);
    assert!(
        kept.contains_key(GDEV_PR) && kept.contains_key(ACME_PR),
        "mark-read pruned the read marks of matters nobody asked about: {kept:#?}"
    );

    let age = age_line(&world);
    assert!(
        age.contains(&format!("{PROJECT} never fetched")),
        "a project with never-asked histories read as fetched: {age:?}"
    );

    clear(&world);
    let status = run(&world, &["status", PROJECT]);
    assert!(status.status.success(), "{}", said(&status));
    assert!(
        calls(&world)
            .iter()
            .any(|line| line.starts_with("gdev a pull-requests")),
        "status read a project with a never-asked history as fresh: {:#?}",
        calls(&world)
    );
}

/// "Never asked" is judged from the project's bindings, not from what the
/// cache holds. On a rebuilt cache, `--source gdev` alone asks all of gdev, but
/// acme is bound and has no history: the project must not read fresh, the
/// sweep must not skip its refetch, and acme's read marks stay
/// (§FS-011-command-line.12, §FS-004-quick-actions.6.1).
#[test]
fn a_bound_source_with_no_history_is_never_asked() {
    let world = watching(600);
    let full = run(&world, &["refresh", PROJECT, "--quiet"]);
    assert!(full.status.success(), "{}", said(&full));
    let marked = run(&world, &["mark-read", "--all"]);
    assert!(marked.status.success(), "{}", said(&marked));

    age_the_cache_model(&world);
    clear(&world);
    let narrowed = refresh_narrowed(&world, &["--source", "gdev"]);
    assert_eq!(narrowed.status.code(), Some(0), "{}", said(&narrowed));
    let made = calls(&world);
    assert!(
        made.iter().all(|line| line.starts_with("gdev ")),
        "--source gdev asked another source: {made:#?}"
    );
    for block in ["a", "b"] {
        assert_eq!(
            asked_by(&made, "gdev", block),
            ["pull-requests", "branch-gates", "issues", "notices"],
            "--source gdev asks every capability of gdev: {made:#?}"
        );
    }

    let age = age_line(&world);
    assert!(
        age.contains(&format!("{PROJECT} never fetched")),
        "acme is bound and was never asked, so the project is not fresh: {age:?}"
    );

    let marked = run(&world, &["mark-read", "--all"]);
    assert!(marked.status.success(), "{}", said(&marked));
    assert!(
        marks(&world).contains_key(ACME_PR),
        "mark-read pruned acme's read mark though acme was never asked: {:#?}",
        marks(&world)
    );

    clear(&world);
    let swept = sweep(&world);
    assert!(
        calls(&world)
            .iter()
            .any(|line| line.starts_with("acme - pull-requests")),
        "the sweep skipped its refetch with a bound source never asked: {:#?}\n{}",
        calls(&world),
        said(&swept)
    );
}

/// Every refusal of the narrowing flags exits 2 with its reason and is decided
/// before any data call (§FS-011-command-line.9.2). `--source` alone on a site
/// source still refreshes it whole and asks no other site source.
#[test]
fn narrowing_is_refused_before_any_data_call() {
    let world = watching(600);
    // A native provider beside the two forges: it has one history and nothing
    // finer to ask. It is never called here, so it needs no stub.
    let mut providers = bound_blocks();
    providers
        .as_array_mut()
        .expect("a list")
        .push(json!({ "provider": "github-prs", "repos": ["acme/app"] }));
    world.configure(json!({ "projects": { PROJECT: { "providers": providers } } }));

    let refuses = |args: &[&str], names: &str, probes: bool| {
        clear(&world);
        let output = refresh_narrowed(&world, args);
        let text = said(&output);
        assert_eq!(output.status.code(), Some(2), "{args:?} is refused: {text}");
        assert!(
            !stderr(&output).contains("unexpected argument"),
            "{args:?} is refused for its reason, not as an unknown flag: {text}"
        );
        assert!(
            stderr(&output).contains(names),
            "{args:?}'s refusal names {names:?}: {text}"
        );
        let made = calls(&world);
        assert!(
            made.iter()
                .all(|line| probes && line.ends_with(" capabilities")),
            "{args:?} made a data call before refusing: {made:#?}"
        );
    };

    refuses(&["--only", "issues"], "--source", false);
    for name in ["capabilities", "failures", "restart", "everything"] {
        refuses(&["--source", "gdev", "--only", name], name, false);
    }
    refuses(
        &["--source", "github-prs", "--only", "pull-requests"],
        "github-prs",
        false,
    );
    refuses(&["--source", "nosuch"], "nosuch", false);
    // acme declares no issues; knowing that takes its probe and nothing more.
    refuses(&["--source", "acme", "--only", "issues"], "issues", true);

    // A site binding: `--only` is refused, `--source` alone asks it whole.
    let site = World::new();
    install_forges(&site);
    site.configure(json!({
        "sources": [
            { "provider": "gdev", "block": "a", "repos": ["G"] },
            { "provider": "acme", "repos": ["app"] }
        ],
        "projects": { PROJECT: { "providers": [] } }
    }));
    clear(&site);
    let output = refresh_narrowed(&site, &["--source", "gdev", "--only", "notices"]);
    assert_eq!(output.status.code(), Some(2), "{}", said(&output));
    assert!(
        !stderr(&output).contains("unexpected argument") && stderr(&output).contains("gdev"),
        "{}",
        said(&output)
    );
    assert!(calls(&site).is_empty(), "{:#?}", calls(&site));

    clear(&site);
    let whole = refresh_narrowed(&site, &["--source", "gdev"]);
    assert_eq!(whole.status.code(), Some(0), "{}", said(&whole));
    let made = calls(&site);
    assert_eq!(
        asked_by(&made, "gdev", "a"),
        ["pull-requests", "branch-gates", "issues", "notices"],
        "{made:#?}"
    );
    assert!(
        made.iter().all(|line| line.starts_with("gdev ")),
        "a site source named alone asked another site source: {made:#?}"
    );
}

/// A plan the project keeps in its own checkout: one open task.
const PLAN: &str = "# Rhei: the retry window\n\n## Tasks\n\n\
### Task 1: Widen the retry window\n**State:** pending\n";

/// A narrowed refresh asks only the source it names: it reads no task store
/// and probes no pool's headroom, both of which a plain refresh does
/// (§FS-011-command-line.9.2, §FS-005-dispatch.29), and what it says it asked
/// is the published shape (§FS-011-command-line.7).
#[test]
fn a_narrowed_refresh_reads_no_store_and_probes_no_pool() {
    let world = World::new();
    install_forges(&world);
    let probed = world.path().join("headroom.log");
    world.stub(
        "north-headroom",
        &format!(
            "#!/usr/bin/env bash\necho probed >> '{}'\n{}",
            probed.display(),
            answering(
                json!({ "v": 1, "data": { "windows": [
                    { "name": "session", "remaining": 0.5, "resets_at": "2026-09-05T18:30:00Z" }
                ] } }),
                0
            )
            .trim_start_matches("#!/usr/bin/env bash\n")
        ),
    );
    world.configure(json!({
        "projects": { PROJECT: { "providers": bound_blocks() } },
        "work": { "headroom": { "north": "north-headroom" } }
    }));
    world.file("panta/window.rhei.md", PLAN);

    // The plain refresh reads the store and asks the pool: the control.
    let plain = run(&world, &["refresh", PROJECT, "--quiet"]);
    assert!(plain.status.success(), "{}", said(&plain));
    assert!(world.has_matter("rhei:window.1"), "{:#?}", world.matters());
    let asked_pool = || {
        std::fs::read_to_string(&probed)
            .unwrap_or_default()
            .lines()
            .count()
    };
    assert_eq!(asked_pool(), 1, "a plain refresh probes the pool");
    let store_before = world.feed()["providers"]["rhei"].clone();

    // The store moves on; a narrowed run must not see it.
    world.file(
        "panta/window.rhei.md",
        &PLAN.replace("**State:** pending", "**State:** completed"),
    );
    for narrowing in [
        &["--source", "gdev"][..],
        &["--source", "gdev", "--only", "issues"],
    ] {
        let mut args = narrowing.to_vec();
        args.push("--json");
        let narrowed = refresh_narrowed(&world, &args);
        assert_eq!(narrowed.status.code(), Some(0), "{}", said(&narrowed));
        let reading = shaped("refresh", &narrowed);
        let answers = reading["projects"][0]["answers"]
            .as_array()
            .unwrap_or_else(|| panic!("{narrowing:?} lists its answers: {reading:#}"));
        assert!(
            !answers.is_empty() && answers.iter().all(|answer| answer["source"] == "gdev"),
            "{narrowing:?} answered for gdev alone: {reading:#}"
        );
        assert_eq!(asked_pool(), 1, "{narrowing:?} probed a pool's headroom");
        assert_eq!(
            world.feed()["providers"]["rhei"],
            store_before,
            "{narrowing:?} read the task store"
        );
        assert!(world.has_matter("rhei:window.1"), "{narrowing:?}");
    }
}

/// Only what a run asked is re-stamped (§FS-011-command-line.12). The site
/// sources' write at the end of a refresh lands in every project's file, but
/// stamps `fetched_at` only on the projects the run asked: refreshing one
/// project no longer makes every other read as just fetched.
#[test]
fn a_site_write_stamps_only_the_projects_the_run_asked() {
    const OTHER: &str = "other";
    let world = World::new();
    install_forges(&world);
    let mut registry = world.registry_doc();
    let mut other = registry["projects"][0].clone();
    other["id"] = json!(OTHER);
    other["root"] = json!(world.path().join(OTHER).to_string_lossy());
    std::fs::create_dir_all(world.path().join(OTHER)).expect("the other project's root");
    registry["projects"]
        .as_array_mut()
        .expect("a list")
        .push(other);
    write_json(&world.registry_path(), &registry);
    world.configure(json!({
        "sources": [{ "provider": "gdev", "block": "a", "repos": ["G"] }],
        "projects": {
            PROJECT: { "providers": [] },
            OTHER: { "providers": [] }
        }
    }));
    let cache = |project: &str| {
        read_json(
            &world
                .path()
                .join("state/ephor/feed")
                .join(format!("{project}.json")),
        )
    };

    let everything = run(&world, &["refresh", "--quiet"]);
    assert!(everything.status.success(), "{}", said(&everything));
    let before = cache(OTHER);
    assert!(before["fetched_at"].is_string(), "{before:#}");

    sleep(Duration::from_millis(50));
    let one = run(&world, &["refresh", PROJECT, "--quiet"]);
    assert!(one.status.success(), "{}", said(&one));
    let after = cache(OTHER);
    assert_ne!(
        after["revision"], before["revision"],
        "the site's write landed in the other project's file: {after:#}"
    );
    assert_eq!(
        after["fetched_at"], before["fetched_at"],
        "refreshing {PROJECT} made {OTHER} read as just fetched"
    );
    assert_ne!(
        cache(PROJECT)["fetched_at"],
        before["fetched_at"],
        "the project the run asked is stamped"
    );
}
