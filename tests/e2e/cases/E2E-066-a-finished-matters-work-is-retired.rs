//! E2E-066-a-finished-matters-work-is-retired: `ephor work retire` puts away
//! the work a finished matter left behind (§FS-005-dispatch.36).
//!
//! The scenario is agent-grounds/grund#301 and #249 in miniature. An issue was
//! answered by a recipe plan in the project's shared work root and by a
//! workflow laid in a minted checkout; the fix merged from a branch; the issue
//! closed; the plan is still there with a ticket at `fix`. Retiring moves that
//! plan, byte for byte, into an archive under `$XDG_DATA_HOME/ephor/archive`,
//! leaves the runtime's lock files where they are
//! (§FS-005-dispatch.36.4), writes a record beside it and the ledger
//! (§FS-005-dispatch.36.5), and asks the runtime for no transition
//! (§FS-005-dispatch.16).
//!
//! Every guard has a case of its own that moves nothing: the forge's word on
//! closure and open pull requests (§FS-005-dispatch.36.1,
//! §FS-001-forge-interface.1), the witnesses (§FS-005-dispatch.36.2), the
//! runtime's exclusion (§FS-005-dispatch.36.3), the archive's placement and
//! the tracked-path check (§FS-005-dispatch.36.4). Then what comes after:
//! recovery (§FS-005-dispatch.36.5), a reopen (§FS-005-dispatch.5), forget
//! (§FS-005-dispatch.35, §FS-005-dispatch.35.1), the readers that report a
//! retired placement (§FS-005-dispatch.4, §FS-003-feed-categories.2), the
//! action (§FS-004-quick-actions.10) and the command's shape and help
//! (§FS-005-dispatch.36.6, §FS-011-command-line.10, §FS-011-command-line.10.1).
//!
//! The forge is `tests/e2e/fixtures/closure-forge/gh.py`, which answers the
//! GraphQL it is handed from a model this file writes, so the read is the
//! implementation's to phrase. The runtime is a stub that renders a workflow,
//! answers `runs --json` from a file, and logs every call.

#[path = "../support.rs"]
mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::*;

const FORGE: &str = include_str!("../fixtures/closure-forge/gh.py");

/// The grund#301 shape: closed by a merged pull request, its checkout gone.
const ISSUE: &str = "github-issues:acme/widget#301";
/// Another matter whose plan shares the root, and must not be touched.
const NEIGHBOUR: &str = "github-issues:acme/widget#302";
/// The grund#249 shape: the record names only a placeholder branch.
const PLACEHOLDER: &str = "github-issues:acme/widget#249";

/// The runtime: renders the one workflow, answers `runs --json` from
/// `runs.json` (stderr from `runs.stderr`, failing where `runs.fails` exists),
/// and logs every invocation to `runner.calls` so a case can say nothing was
/// moved through it.
const RUNNER: &str = r#"#!/usr/bin/env bash
set -euo pipefail
world="@WORLD@"
printf '%s\n' "$*" >> "$world/runner.calls"
verb="${1:-}"
if [ "$#" -gt 0 ]; then shift; fi
case "$verb" in
  templates) cat <<'JSON'
[{"name":"wf","version":"1.0.0","source":"user","path":"wf","description":"work an issue",
  "inputs":[{"name":"ticket","description":"the issue","type":"string","required":true,"default":null,"validate":null}]}]
JSON
  ;;
  instantiate)
    output=""
    while [ "$#" -gt 0 ]; do case "$1" in --output) output="$2"; shift 2;; *) shift;; esac; done
    mkdir -p "$output/tasks"
    printf '# Rhei: wf\n**States:** wf\n' > "$output/index.rhei.md"
    printf 'name: wf\nstates:\n  implementing:\n    agent: x\n  done:\n    final: true\n' > "$output/states.yaml"
    printf '### Task fix: fix the ticket\n**State:** implementing\n\nwork\n' > "$output/tasks/01-fix.md"
    echo "Instantiated wf into $output" ;;
  runs)
    if [ -f "$world/runs.stderr" ]; then cat "$world/runs.stderr" >&2; fi
    if [ -f "$world/runs.fails" ]; then exit 1; fi
    if [ -f "$world/runs.json" ]; then cat "$world/runs.json"; else echo '[]'; fi ;;
  *) exit 0 ;;
esac
"#;

/// Holds an exclusive `flock` on argv[1] until killed, touching argv[2] once
/// it has it — a run, as far as its lock is concerned.
const HOLD: &str = "import fcntl, pathlib, sys, time\n\
lock = open(sys.argv[1], 'a')\n\
fcntl.flock(lock, fcntl.LOCK_EX)\n\
pathlib.Path(sys.argv[2]).touch()\n\
time.sleep(120)\n";

static HELD: AtomicUsize = AtomicUsize::new(0);

fn python() -> String {
    let out = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .expect("python3 for the offline forge");
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

struct Site {
    world: World,
    python: String,
}

/// A lock held by another process, released when dropped.
struct Holder(Child);

impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Every file under the forest and the data directory, with the ledger: what a
/// refusal must leave as it found it.
type Snapshot = BTreeMap<PathBuf, Vec<u8>>;

impl Site {
    /// A project `demo` whose checkouts are worktrees of `demo/main`, an
    /// `answer` recipe writing into the shared root `demo/panta`, an
    /// `implement` workflow laid into a minted `fix/issue-<n>` checkout, and a
    /// forge where #301 and #302 are open.
    fn new() -> Site {
        let world = World::new();
        let python = python();
        fs::write(world.path().join("gh.py"), FORGE).unwrap();
        world.stub(
            "gh",
            "#!/bin/sh\nexec \"$CLOSURE_PY\" -I \"$CLOSURE_GH\" \"$@\"\n",
        );
        world.stub(
            "rhei",
            &RUNNER.replace("@WORLD@", &world.path().to_string_lossy()),
        );
        world.register(json!({ "branch_root_template": "{project_root}/{branch}" }));
        world.configure(json!({
            "defaults": { "github_user": "tester" },
            "projects": { PROJECT: {
                "providers": [{
                    "provider": "github-issues", "repos": ["acme/widget"],
                    "participating": false, "comments": false, "updated_within_days": 0
                }],
                "actions": [{
                    "id": "implement", "icon": "*", "description": "work this issue",
                    "workflow": "wf", "requires_checkout": true,
                    "branch": "fix/issue-{number}", "root": "{workspace}/panta",
                    "when": { "kinds": ["issue"] },
                    "inputs": { "ticket": "{repo}#{number}" }
                }],
                "work": {
                    "root": "{root}/panta",
                    "recipes": [{
                        "id": "answer", "icon": "a", "description": "answer",
                        "state": "fix", "needs_checkout": false,
                        "when": { "kinds": ["issue"] }, "brief": "{title}"
                    }]
                }
            }},
            "work": { "runner": "rhei" }
        }));
        let site = Site { world, python };
        let origin = site.world.path().join("origin");
        fs::create_dir_all(&origin).unwrap();
        site.git(&origin, &["init", "-q", "-b", "main"]);
        fs::write(origin.join("README.md"), "widget\n").unwrap();
        site.git(&origin, &["add", "."]);
        site.git(&origin, &["commit", "-qm", "init"]);
        let main = site.world.forest().join("main");
        site.git(
            site.world.path(),
            &[
                "clone",
                "-q",
                &origin.to_string_lossy(),
                &main.to_string_lossy(),
            ],
        );
        site.forge(json!({
            "login": "tester",
            "repos": { "acme/widget": {
                "issues": {
                    "301": { "title": "Remove the fallback", "state": "OPEN" },
                    "302": { "title": "Neighbour", "state": "OPEN" }
                },
                "pulls": {}
            }}
        }));
        site
    }

    fn git(&self, dir: &Path, args: &[&str]) -> String {
        let out = git_command()
            .current_dir(dir)
            .env("HOME", self.world.path())
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?} in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn forge_path(&self) -> PathBuf {
        self.world.path().join("forge.json")
    }

    fn forge(&self, model: Value) {
        write_json(&self.forge_path(), &model);
    }

    /// Change the forge, as time passing on GitHub would.
    fn on_forge(&self, change: impl FnOnce(&mut Value)) {
        let mut model = read_json(&self.forge_path());
        change(&mut model);
        self.forge(model);
    }

    fn issue(&self, number: u32, fields: Value) {
        self.on_forge(|model| {
            let slot = &mut model["repos"]["acme/widget"]["issues"][number.to_string()];
            if slot.is_null() {
                *slot = json!({});
            }
            for (key, value) in fields.as_object().unwrap() {
                slot[key] = value.clone();
            }
        });
    }

    fn pull(&self, number: u32, fields: Value) {
        self.on_forge(|model| {
            model["repos"]["acme/widget"]["pulls"][number.to_string()] = fields;
        });
    }

    /// The issue closed because pull request `pr` merged from `head`.
    fn closed_by_merge(&self, issue: u32, pr: u32, head: &str) {
        self.pull(
            pr,
            json!({
                "title": "the fix", "state": "MERGED",
                "mergedAt": "2026-10-01T23:29:00Z", "closedAt": "2026-10-01T23:29:00Z",
                "headRefName": head, "closes": [issue]
            }),
        );
        self.issue(
            issue,
            json!({ "state": "CLOSED", "closedAt": "2026-10-01T23:29:30Z", "closedBy": [pr] }),
        );
    }

    fn ephor(&self) -> Command {
        let mut command = self.world.ephor_raw();
        for key in ["GH_HOST", "GH_TOKEN", "GITHUB_TOKEN"] {
            command.env_remove(key);
        }
        command
            .env("XDG_DATA_HOME", self.world.path().join("data"))
            .env("XDG_CONFIG_HOME", self.world.path().join("config"))
            .env("XDG_CACHE_HOME", self.world.path().join("cache"))
            .env("CLOSURE_PY", &self.python)
            .env("CLOSURE_GH", self.world.path().join("gh.py"))
            .env("CLOSURE_FORGE", self.forge_path());
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.ephor().args(args).output().expect("ephor runs")
    }

    fn run_in(&self, dir: &Path, args: &[&str]) -> Output {
        self.ephor()
            .current_dir(dir)
            .args(args)
            .output()
            .expect("ephor runs")
    }

    fn ok(&self, args: &[&str]) -> Output {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "ephor {args:?} failed:\n{}",
            said(&out)
        );
        out
    }

    fn refresh(&self) {
        self.ok(&["refresh", PROJECT, "--quiet"]);
    }

    fn dispatch(&self, item: &str) {
        self.ok(&["work", "dispatch", "--item", item, "--recipe", "answer"]);
    }

    fn lay(&self, item: &str) {
        self.ok(&["work", "lay", "implement", "--item", item]);
    }

    /// `work retire <args> --json`, held to the shape it publishes.
    fn retire(&self, args: &[&str]) -> (Output, Value) {
        let mut all = vec!["work", "retire"];
        all.extend_from_slice(args);
        all.push("--json");
        let out = self.run(&all);
        (out.clone(), published(&out))
    }

    /// A retirement that must succeed.
    fn retired(&self, args: &[&str]) -> Value {
        let (out, value) = self.retire(args);
        assert!(
            out.status.success(),
            "work retire {args:?}:\n{}",
            said(&out)
        );
        assert_eq!(value["retired"], true, "{value:#}");
        assert!(value["refused"].is_null(), "{value:#}");
        value
    }

    fn ledger_path(&self) -> PathBuf {
        self.world.path().join("state/ephor/work.json")
    }

    fn ledger(&self) -> Value {
        read_json(&self.ledger_path())
    }

    fn entry(&self, item: &str) -> Value {
        self.ledger()["entries"][item].clone()
    }

    /// The recipe plan ephor wrote for `item`, a single file in the shared root.
    fn recipe_plan(&self, item: &str) -> PathBuf {
        PathBuf::from(self.entry(item)["plan"].as_str().expect("a recipe plan"))
    }

    /// The directory workspace the `implement` workflow laid for `item`.
    fn laid_plan(&self, item: &str) -> PathBuf {
        let entry = self.entry(item);
        let laid = entry["dispatches"]
            .as_array()
            .unwrap()
            .iter()
            .find(|dispatch| dispatch["recipe"] == "implement")
            .expect("a laid workflow")
            .clone();
        PathBuf::from(laid["root"].as_str().unwrap()).join(laid["plan"].as_str().unwrap())
    }

    fn checkout(&self, number: u32) -> PathBuf {
        self.world.forest().join(format!("fix/issue-{number}"))
    }

    fn remove_checkout(&self, number: u32) {
        let main = self.world.forest().join("main");
        let checkout = self.checkout(number);
        self.git(
            &main,
            &["worktree", "remove", "--force", &checkout.to_string_lossy()],
        );
    }

    fn shared_root(&self) -> PathBuf {
        self.world.forest().join("panta")
    }

    fn data(&self) -> PathBuf {
        self.world.path().join("data")
    }

    fn archive_root(&self) -> PathBuf {
        self.data().join("ephor/archive")
    }

    fn runner_calls(&self) -> String {
        fs::read_to_string(self.world.path().join("runner.calls")).unwrap_or_default()
    }

    /// Files under the forest (without `.git`) and the data directory, and the
    /// ledger.
    fn snapshot(&self) -> Snapshot {
        let mut files = Snapshot::new();
        walk(&self.world.forest(), &mut files);
        walk(&self.data(), &mut files);
        if let Ok(bytes) = fs::read(self.ledger_path()) {
            files.insert(self.ledger_path(), bytes);
        }
        files
    }

    /// A refusal: exit 1, the guard named, the sentence carrying `says`, nothing
    /// moved, nothing archived, the ledger as it was. A lock file the runtime
    /// would have created empty is the one thing a real attempt may leave
    /// (§FS-005-dispatch.36.3).
    fn refused(&self, args: &[&str], guard: &str, says: &[&str]) -> Value {
        let before = self.snapshot();
        let (out, value) = self.retire(args);
        assert_eq!(
            out.status.code(),
            Some(1),
            "a guard refuses with exit 1:\n{}",
            said(&out)
        );
        assert_eq!(value["retired"], false, "{value:#}");
        assert_eq!(value["refused"]["guard"], guard, "{value:#}");
        let sentence = value["refused"]["says"].as_str().unwrap_or_default();
        for part in says {
            assert!(
                sentence.contains(part),
                "the refusal should say {part:?}: {sentence:?}"
            );
        }
        untouched(&before, &self.snapshot());
        value
    }

    /// An exclusive lock on `lock`, held by another process until dropped.
    fn hold(&self, lock: &Path) -> Holder {
        fs::create_dir_all(lock.parent().unwrap()).unwrap();
        let ready = self
            .world
            .path()
            .join(format!("held-{}", HELD.fetch_add(1, Ordering::SeqCst)));
        let child = Command::new(&self.python)
            .args(["-I", "-c", HOLD])
            .arg(lock)
            .arg(&ready)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the lock holder starts");
        let holder = Holder(child);
        let deadline = Instant::now() + Duration::from_secs(20);
        while !ready.exists() {
            assert!(
                Instant::now() < deadline,
                "the holder never took {}",
                lock.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        holder
    }

    /// A hand-laid directory plan with one open task, whose ticket records
    /// `id` where one is given — the shape a person or another tool leaves in
    /// a checkout's work root.
    fn hand_laid(&self, root: &Path, name: &str, id: Option<&str>) -> PathBuf {
        let plan = root.join(name);
        fs::create_dir_all(plan.join("tasks")).unwrap();
        let front = match id {
            Some(id) => format!("\n---\nmetadata:\n  tasks:\n    fix:\n      id: \"{id}\"\n---\n"),
            None => String::new(),
        };
        fs::write(
            plan.join("index.rhei.md"),
            format!("# Rhei: {name}\n{front}\n**States:** wf\n"),
        )
        .unwrap();
        fs::write(
            plan.join("states.yaml"),
            "name: wf\nstates:\n  implementing:\n    agent: x\n  done:\n    final: true\n",
        )
        .unwrap();
        fs::write(
            plan.join("tasks/01-fix.md"),
            "### Task fix: something\n**State:** implementing\n\nwork\n",
        )
        .unwrap();
        plan
    }
}

fn said(out: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The `work-retire` shape, or a failure that quotes what was printed instead.
fn published(out: &Output) -> Value {
    if serde_json::from_slice::<Value>(&out.stdout).is_err() {
        panic!("`ephor work retire --json` printed no JSON:\n{}", said(out));
    }
    shaped("work-retire", out)
}

fn walk(dir: &Path, files: &mut Snapshot) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            if entry.file_name() != ".git" {
                walk(&path, files);
            }
        } else if kind.is_symlink() {
            let target = fs::read_link(&path).unwrap();
            files.insert(path, target.to_string_lossy().as_bytes().to_vec());
        } else {
            files.insert(path, fs::read(entry.path()).unwrap_or_default());
        }
    }
}

/// Everything that was there is there, byte for byte; anything new is an empty
/// lock file.
fn untouched(before: &Snapshot, after: &Snapshot) {
    for (path, bytes) in before {
        match after.get(path) {
            Some(now) => assert!(now == bytes, "{} changed", path.display()),
            None => panic!("{} is gone", path.display()),
        }
    }
    for (path, bytes) in after {
        if !before.contains_key(path) {
            let lock = path.extension().is_some_and(|ext| ext == "lock");
            assert!(
                lock && bytes.is_empty(),
                "{} appeared, and only an empty lock file may",
                path.display()
            );
        }
    }
}

/// Every file under `dir` whose name is `name`.
fn found(dir: &Path, name: &str) -> Vec<PathBuf> {
    let mut files = Snapshot::new();
    walk(dir, &mut files);
    files
        .into_keys()
        .filter(|path| path.file_name().is_some_and(|file| file == name))
        .collect()
}

fn plan_named<'a>(value: &'a Value, path: &Path) -> &'a Value {
    value["plans"]
        .as_array()
        .expect("plans")
        .iter()
        .find(|plan| {
            plan["plan"]
                .as_str()
                .is_some_and(|plan| Path::new(plan) == path)
        })
        .unwrap_or_else(|| panic!("no plan {} in {value:#}", path.display()))
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| match item.as_str() {
                    Some(text) => text.to_string(),
                    None => item.to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The forge's fields for an issue reopened after this run's clock: the forge
/// stamps what it does not set with its own wall clock, so a fixed instant would
/// fall behind the dispatch it must come after (§FS-005-dispatch.5).
fn reopened_now() -> Value {
    let at = chrono::Utc::now() + chrono::Duration::minutes(5);
    json!({
        "state": "OPEN",
        "closedAt": null,
        "updatedAt": at.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    })
}

/// grund#301 in miniature: #301 answered by a recipe plan in the shared root
/// beside #302's and a hand-laid directory workspace, and by a workflow laid in
/// `fix/issue-301`; the checkout removed; #301 closed by #331 merged from
/// `fix/issue-301`. The plan's lock file and a result of its own sit in the
/// root, as the runtime leaves them.
fn finished(keep_checkout: bool) -> Site {
    let site = Site::new();
    site.refresh();
    site.dispatch(ISSUE);
    site.dispatch(NEIGHBOUR);
    site.lay(ISSUE);
    let root = site.shared_root();
    site.hand_laid(&root, "hand-laid", None);
    let plan = site.recipe_plan(ISSUE);
    let plan_id = site.entry(ISSUE)["plan_id"].as_str().unwrap().to_string();
    fs::write(format!("{}.lock", plan.display()), "").unwrap();
    fs::create_dir_all(root.join("runtime/results")).unwrap();
    fs::write(
        root.join(format!("runtime/results/{plan_id}.answer-1.md")),
        "half an answer\n",
    )
    .unwrap();
    if !keep_checkout {
        site.remove_checkout(301);
    }
    site.closed_by_merge(301, 331, "fix/issue-301");
    site.refresh();
    site
}

/// grund#249 in miniature: the workflow was laid in `fix/issue-249`, but the
/// record says branch `fix` and checkout `demo/fix` — a directory of
/// checkouts. #249 closed by #289 merged from `fix/issue-249`. The workspace
/// carries the runtime's coordination files beside its content.
fn placeholder(keep_checkout: bool, closing_pr: bool) -> Site {
    let site = Site::new();
    site.issue(
        249,
        json!({ "title": "Sweep the fallbacks", "state": "OPEN" }),
    );
    site.refresh();
    site.lay(PLACEHOLDER);
    let plan = site.laid_plan(PLACEHOLDER);
    for (file, body) in [
        (".rhei/run.lock", ""),
        ("index.rhei.md.lock", ""),
        ("tasks/01-fix.md.lock", ""),
        (
            "runtime.state-transitions.log",
            "fix: queued -> implementing\n",
        ),
        ("runtime.state-transitions.log.lock", ""),
        ("runtime.notes.md", "a note\n"),
        ("runtime.notes.md.lock", ""),
        ("runtime/metrics/.lock", ""),
        ("runtime/metrics/usage.json", "{\"tokens\": 12}\n"),
        ("runtime/results/fix.md", "verdict: half done\n"),
    ] {
        let path = plan.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }
    let placeholder = site.world.forest().join("fix");
    let mut ledger = site.ledger();
    let entry = &mut ledger["entries"][PLACEHOLDER];
    entry["branch"] = json!("fix");
    entry["checkout"] = json!(placeholder);
    for dispatch in entry["dispatches"].as_array_mut().unwrap() {
        dispatch["branch"] = json!("fix");
        dispatch["checkout"] = json!(placeholder);
    }
    write_json(&site.ledger_path(), &ledger);
    if !keep_checkout {
        site.remove_checkout(249);
    }
    if closing_pr {
        site.closed_by_merge(249, 289, "fix/issue-249");
    } else {
        site.issue(
            249,
            json!({ "state": "CLOSED", "closedAt": "2026-10-03T08:00:00Z" }),
        );
    }
    site.refresh();
    site
}

// --- 1. The grund#301 shape ---------------------------------------------------

#[test]
fn a_finished_matters_plan_in_a_shared_root_moves_whole_and_its_lock_stays() {
    let site = finished(false);
    let root = site.shared_root();
    let plan = site.recipe_plan(ISSUE);
    let original = fs::read(&plan).unwrap();
    let machine = fs::read(root.join("states.yaml")).unwrap();
    let neighbour = site.recipe_plan(NEIGHBOUR);
    let mut kept = Snapshot::new();
    for path in [
        root.join("index.panta.md"),
        root.join("states.yaml"),
        neighbour,
    ] {
        kept.insert(path.clone(), fs::read(&path).unwrap());
    }
    walk(&root.join("runtime"), &mut kept);
    walk(&root.join("hand-laid"), &mut kept);

    let value = site.retired(&["--item", ISSUE]);

    assert_eq!(value["item"], ISSUE);
    assert_eq!(value["dry_run"], false);
    let archive = PathBuf::from(value["archive"].as_str().expect("the archive"));
    assert!(
        archive.starts_with(site.archive_root().join("demo")),
        "the archive is ephor's data, per project: {}",
        archive.display()
    );
    assert!(!plan.exists(), "the plan left the shared root");
    assert!(
        Path::new(&format!("{}.lock", plan.display())).exists(),
        "its lock file stays where the runtime expects it"
    );
    let name = plan.file_name().unwrap().to_string_lossy().into_owned();
    let moved = found(&archive, &name);
    assert_eq!(
        moved.len(),
        1,
        "one copy of the plan in {}",
        archive.display()
    );
    assert_eq!(fs::read(&moved[0]).unwrap(), original, "byte for byte");
    assert!(
        found(&archive, "states.yaml")
            .iter()
            .any(|copy| fs::read(copy).unwrap() == machine),
        "the root's machine is copied beside the plan"
    );
    for (path, bytes) in &kept {
        assert_eq!(
            &fs::read(path).unwrap(),
            bytes,
            "{} is untouched",
            path.display()
        );
    }
    let entry = plan_named(&value, &plan);
    assert_eq!(entry["outcome"], "moved", "{value:#}");
    assert_eq!(
        entry["open"],
        json!([{ "task": "answer-1", "state": "fix" }])
    );
    assert_eq!(entry["admitted_by_caller"], false);
    let laid = site.laid_plan(ISSUE);
    assert_eq!(plan_named(&value, &laid)["outcome"], "absent", "{value:#}");
    let asked = site.runner_calls();
    assert!(
        !asked.lines().any(|line| line.starts_with("transition")),
        "no transition is asked of the runtime:\n{asked}"
    );
}

#[test]
fn the_retirement_is_recorded_beside_the_archive_and_in_the_ledger() {
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    let plan_id = site.entry(ISSUE)["plan_id"].as_str().unwrap().to_string();
    let neighbour_id = site.entry(NEIGHBOUR)["plan_id"]
        .as_str()
        .unwrap()
        .to_string();
    // What the runtime keeps per task in the shared root, as it names it: a
    // snapshot-cache entry, a log and a spawn of the plan's ticket, and a log
    // of the neighbour's.
    let root = site.shared_root();
    let cache = root.join(format!(".rhei/cache/snapshots/{plan_id}.answer-1"));
    let log = root.join(format!("runtime/logs/task-{plan_id}.answer-1-fix.log"));
    let spawn = root.join(format!("runtime/spawns/task-{plan_id}.answer-1-fix.json"));
    let theirs = root.join(format!("runtime/logs/task-{neighbour_id}.answer-1-fix.log"));
    for file in [
        cache.join("review/fix/1/a/record.json"),
        log.clone(),
        spawn.clone(),
        theirs.clone(),
    ] {
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "kept\n").unwrap();
    }

    let value = site.retired(&["--item", ISSUE, "--why", "merged as #331"]);

    let archive = PathBuf::from(value["archive"].as_str().unwrap());
    let record_path = PathBuf::from(value["record"].as_str().expect("the record"));
    assert_eq!(record_path, archive.join("retirement.json"));
    let record = read_json(&record_path);
    assert_eq!(record["schema"], "ephor-retirement/1");
    assert_eq!(record["state"], "committed");
    assert_eq!(record["matter"]["id"], ISSUE);
    assert_eq!(record["matter"]["project"], PROJECT);
    assert_eq!(record["retired"]["why"], "merged as #331");
    assert_eq!(record["eligibility"]["issue"]["state"], "closed");
    assert_eq!(record["eligibility"]["merged"][0]["pr"], 331);
    let left = strings(&record["left"]).join("\n");
    assert!(
        left.contains(&format!(
            "{}.lock",
            plan.file_name().unwrap().to_string_lossy()
        )),
        "the lock file left behind is named:\n{left}"
    );
    assert!(
        left.contains(&format!("{plan_id}.answer-1.md")),
        "so is the plan's own result left in the root's runtime/results:\n{left}"
    );
    let named = strings(&record["left"]);
    for kept in [&cache, &log, &spawn] {
        assert!(
            named.iter().any(|said| Path::new(said) == kept.as_path()),
            "{} stays where the runtime put it, and the record names it:\n{left}",
            kept.display()
        );
        assert!(kept.exists(), "{} stays", kept.display());
    }
    assert!(
        !named.iter().any(|said| Path::new(said) == theirs.as_path()),
        "the neighbour's log is not this plan's:\n{left}"
    );

    let entry = site.entry(ISSUE);
    let retirements = entry["retirements"].as_array().expect("retirements");
    assert_eq!(retirements.len(), 1);
    assert_eq!(retirements[0]["record"], json!(record_path));
    assert_eq!(retirements[0]["archive"], json!(archive));
    let recipe = entry["dispatches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|dispatch| dispatch["recipe"] == "answer")
        .unwrap();
    assert!(recipe["retired_at"].is_string(), "{entry:#}");
    assert_eq!(
        recipe["root"],
        json!(site.shared_root()),
        "the placement stays as written"
    );
}

#[test]
fn a_retired_placement_reads_as_retired_and_no_longer_as_open_work() {
    let site = finished(false);
    let before = site.ok(&["work", "offers", "--item", ISSUE, "--json"]);
    let before = json_of(&before);
    assert_eq!(
        before["status"]["missing"], true,
        "the laid plan's checkout is gone"
    );
    assert!(
        before["status"]["tickets"]
            .as_array()
            .unwrap()
            .iter()
            .any(|ticket| ticket["id"] == "answer-1" && ticket["finished"] == false),
        "answer-1 is open work before: {before:#}"
    );

    let value = site.retired(&["--item", ISSUE]);
    let archive = value["archive"].as_str().unwrap().to_string();

    let list = site.ok(&["work", "list"]);
    let list = String::from_utf8_lossy(&list.stdout);
    assert!(list.contains("retired →"), "{list}");
    assert!(!list.contains("a plan is missing"), "{list}");
    let ours: Vec<&str> = list
        .lines()
        .filter(|line| line.contains("acme/widget#301") || line.contains("acme-widget-301"))
        .collect();
    assert!(!ours.is_empty(), "{list}");
    assert!(
        ours.iter().all(|line| !line.contains("answer · fix")),
        "no open ticket row is left for #301:\n{list}"
    );
    assert!(
        list.lines()
            .any(|line| line.contains("acme/widget#302") && line.contains("answer · fix")),
        "the neighbour's open row stays:\n{list}"
    );
    let after = site.ok(&["work", "offers", "--item", ISSUE, "--json"]);
    let text = String::from_utf8_lossy(&after.stdout).into_owned();
    let after = json_of(&after);
    assert_eq!(after["status"]["missing"], false, "{after:#}");
    assert!(
        text.contains(&archive),
        "the view names where it went:\n{text}"
    );
}

/// The rows `ephor work` prints about #301, which a retired placement's own
/// lines are among.
fn board_of_301(site: &Site) -> Vec<String> {
    let out = site.ok(&["work"]);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| line.contains("acme/widget#301") || line.contains("acme-widget-301"))
        .map(str::to_string)
        .collect()
}

/// A run of the runtime itself, as the stub logged it, apart from the runs
/// listing a read may ask for and the help a probe reads.
fn ran(calls: &str) -> Vec<&str> {
    calls
        .lines()
        .filter(|line| *line == "run" || line.starts_with("run "))
        .filter(|line| !line.contains("--help"))
        .collect()
}

#[test]
fn a_file_put_back_at_a_retired_plans_path_is_neither_shown_nor_run() {
    // Every reader asks the one rule whether a placement was retired, so a
    // file someone puts back where the plan was changes neither what the
    // board says nor what a run by name reaches (§FS-005-dispatch.4,
    // §FS-005-dispatch.36.5).
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    let value = site.retired(&["--item", ISSUE]);
    let to = PathBuf::from(
        plan_named(&value, &plan)["to"]
            .as_str()
            .expect("where the plan went"),
    );
    let board = board_of_301(&site);
    assert!(
        board.iter().any(|line| line.contains("retired →")),
        "{board:#?}"
    );
    let run = site.run(&["work", "run", "--item", ISSUE]);
    assert!(
        said(&run).contains(&format!("Nothing to run for {ISSUE}")),
        "{}",
        said(&run)
    );

    fs::copy(&to, &plan).unwrap();

    assert_eq!(board_of_301(&site), board, "the board says what it said");
    let again = site.run(&["work", "run", "--item", ISSUE]);
    assert_eq!(again.status.code(), run.status.code(), "{}", said(&again));
    assert_eq!(
        said(&again),
        said(&run),
        "and a run by name reaches nothing"
    );
    assert!(
        ran(&site.runner_calls()).is_empty(),
        "the runtime was never run:\n{}",
        site.runner_calls()
    );
}

#[test]
fn a_ticket_in_a_retired_plan_cannot_be_cancelled() {
    let site = finished(false);
    let value = site.retired(&["--item", ISSUE]);
    let archive = value["archive"].as_str().unwrap().to_string();
    let out = site.run(&[
        "work", "cancel", "--item", ISSUE, "answer-1", "--why", "late",
    ]);
    assert!(!out.status.success(), "{}", said(&out));
    let text = said(&out);
    assert!(text.contains("retired"), "{text}");
    assert!(text.contains(&archive), "{text}");
    assert!(
        !site
            .runner_calls()
            .lines()
            .any(|line| line.starts_with("transition")),
        "nothing was asked of the runtime"
    );
}

// --- 2. The grund#249 shape ---------------------------------------------------

#[test]
fn a_placeholder_is_discarded_and_a_directory_workspace_moves_its_content_and_keeps_its_locks() {
    let site = placeholder(true, true);
    let plan = site.laid_plan(PLACEHOLDER);
    let index = fs::read(plan.join("index.rhei.md")).unwrap();

    let value = site.retired(&["--item", PLACEHOLDER]);

    let eligibility = &value["eligibility"];
    let discarded = eligibility["discarded"].as_array().unwrap();
    assert!(
        discarded.iter().any(|branch| branch["name"] == "fix"),
        "the directory of checkouts is a placeholder: {eligibility:#}"
    );
    let found_branch = eligibility["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|branch| branch["name"] == "fix/issue-249")
        .unwrap_or_else(|| panic!("fix/issue-249 is a candidate: {eligibility:#}"));
    let from = strings(&found_branch["from"]);
    assert!(from.contains(&"worktree".to_string()), "{from:?}");
    assert!(from.contains(&"closing pr".to_string()), "{from:?}");
    assert!(
        eligibility["merged"]
            .as_array()
            .unwrap()
            .iter()
            .any(|pr| pr["pr"] == 289),
        "{eligibility:#}"
    );

    for stays in [
        ".rhei/run.lock",
        "index.rhei.md.lock",
        "tasks/01-fix.md.lock",
        "runtime.state-transitions.log.lock",
        "runtime.notes.md.lock",
        "runtime/metrics/.lock",
    ] {
        assert!(
            plan.join(stays).exists(),
            "{stays} stays where the runtime keeps it"
        );
    }
    for leaves in [
        "index.rhei.md",
        "states.yaml",
        "tasks/01-fix.md",
        "runtime.state-transitions.log",
        "runtime.notes.md",
        "runtime/metrics/usage.json",
        "runtime/results/fix.md",
    ] {
        assert!(
            !plan.join(leaves).exists(),
            "{leaves} moved with the content"
        );
    }
    let archive = PathBuf::from(value["archive"].as_str().unwrap());
    let moved = found(&archive, "index.rhei.md");
    assert_eq!(moved.len(), 1, "{}", archive.display());
    assert_eq!(fs::read(&moved[0]).unwrap(), index);
    assert!(
        !found(&archive, "item.json").is_empty(),
        "the lay record goes with its plan"
    );
}

#[test]
fn the_branch_form_reaches_the_same_matter() {
    let site = placeholder(true, true);
    let (out, value) = site.retire(&[
        "--project",
        PROJECT,
        "--branch",
        "fix/issue-249",
        "--dry-run",
    ]);
    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(value["item"], PLACEHOLDER, "{value:#}");
    assert_eq!(value["dry_run"], true);
    let out = site.run_in(
        &site.checkout(249),
        &["work", "retire", "--dry-run", "--json"],
    );
    let value = published(&out);
    assert_eq!(
        value["item"], PLACEHOLDER,
        "the checkout one stands in: {value:#}"
    );
}

// --- 3. One refusal per guard -------------------------------------------------

#[test]
fn an_open_pull_request_from_a_fork_refuses() {
    let site = finished(false);
    site.pull(
        340,
        json!({ "title": "my take", "state": "OPEN", "headRefName": "fix/issue-301",
                "headOwner": "someone" }),
    );
    site.refused(
        &["--item", ISSUE],
        "open-pull-request",
        &["#340", "someone:fix/issue-301", "still open"],
    );
}

#[test]
fn an_open_issue_whose_only_pull_request_closed_unmerged_refuses() {
    let site = Site::new();
    site.refresh();
    site.dispatch(ISSUE);
    site.pull(
        331,
        json!({ "title": "abandoned", "state": "CLOSED", "closedAt": "2026-10-01T23:29:00Z",
                "headRefName": "fix/issue-301", "closes": [301] }),
    );
    site.issue(301, json!({ "closedBy": [331] }));
    site.refresh();
    site.refused(&["--item", ISSUE], "eligibility", &["acme/widget#301"]);
}

#[test]
fn a_held_run_lock_refuses() {
    let site = finished(false);
    let lock = site.shared_root().join(".rhei/run.lock");
    let _held = site.hold(&lock);
    site.refused(
        &["--item", ISSUE],
        "live-root",
        &["held by a live run", "retire again when it ends"],
    );
}

#[test]
fn a_held_writer_lock_refuses() {
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    let _held = site.hold(Path::new(&format!("{}.lock", plan.display())));
    site.refused(&["--item", ISSUE], "live-root", &["held"]);
}

#[test]
fn a_forced_recovery_marker_refuses() {
    let site = finished(false);
    let marker = site.shared_root().join(".rhei/forced-recovery.json");
    fs::create_dir_all(marker.parent().unwrap()).unwrap();
    fs::write(&marker, "{}\n").unwrap();
    site.refused(&["--item", ISSUE], "live-root", &[]);
}

#[test]
fn a_pending_removal_marker_refuses() {
    let site = finished(false);
    let marker = site.shared_root().join(".rhei/pending-removal.json");
    fs::create_dir_all(marker.parent().unwrap()).unwrap();
    fs::write(&marker, "{}\n").unwrap();
    site.refused(&["--item", ISSUE], "live-root", &[]);
}

#[test]
fn a_root_the_runtime_reports_live_or_cannot_decide_refuses_though_its_lock_is_free() {
    let site = finished(false);
    let root = site.shared_root();
    let other = site.recipe_plan(NEIGHBOUR);
    write_json(
        &site.world.path().join("runs.json"),
        &json!([{
            "id": "r1", "pid": std::process::id(), "status": "running",
            "workspace": root, "plan": other, "started_at": "2026-11-12T14:00:00Z",
            "headless": true, "parallel": 1, "events": root.join("runtime/events.jsonl"),
            "exit_code": null
        }]),
    );
    site.refused(&["--item", ISSUE], "live-root", &[]);

    fs::write(site.world.path().join("runs.json"), "[]\n").unwrap();
    fs::write(
        site.world.path().join("runs.stderr"),
        format!(
            "warning: could not check r2 {}: the descriptor is unreadable\n",
            root.display()
        ),
    )
    .unwrap();
    site.refused(&["--item", ISSUE], "live-root", &[]);
}

#[test]
fn a_stale_running_descriptor_does_not_block() {
    let site = finished(false);
    let root = site.shared_root();
    write_json(
        &root.join("runtime/run.json"),
        &json!({ "id": "r0", "pid": 999_999, "status": "running", "workspace": root,
                 "started_at": "2026-10-01T00:00:00Z" }),
    );
    site.retired(&["--item", ISSUE]);
}

#[test]
fn a_foreign_witness_refuses_with_and_without_admit() {
    let site = finished(true);
    let root = site.checkout(301).join("panta");
    let stray = site.hand_laid(&root, "stray", Some(NEIGHBOUR));
    site.refused(&["--item", ISSUE], "witness", &["stray"]);
    site.refused(
        &["--item", ISSUE, "--admit", &stray.to_string_lossy()],
        "witness",
        &["stray"],
    );
}

#[test]
fn a_mixed_witness_refuses_even_with_admit() {
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    let text = fs::read_to_string(&plan).unwrap();
    let swapped = text.replacen(
        &format!("id: \"{ISSUE}\""),
        &format!("id: \"{NEIGHBOUR}\""),
        1,
    );
    assert_ne!(text, swapped, "the ticket records its matter");
    fs::write(&plan, swapped).unwrap();
    site.refused(&["--item", ISSUE], "witness", &[]);
    site.refused(
        &["--item", ISSUE, "--admit", &plan.to_string_lossy()],
        "witness",
        &[],
    );
}

#[test]
fn a_witness_less_plan_refuses_until_the_caller_admits_it() {
    let site = finished(true);
    let root = site.checkout(301).join("panta");
    let orphan = site.hand_laid(&root, "orphan", None);
    site.refused(&["--item", ISSUE], "witness", &["orphan", "--admit"]);

    let before = site.snapshot();
    let out = site.run_in(
        &site.checkout(301),
        &[
            "work",
            "retire",
            "--admit",
            &orphan.to_string_lossy(),
            "--json",
        ],
    );
    assert!(
        !out.status.success(),
        "--admit needs --item:\n{}",
        said(&out)
    );
    assert!(said(&out).contains("--item"), "{}", said(&out));
    untouched(&before, &site.snapshot());

    let value = site.retired(&["--item", ISSUE, "--admit", &orphan.to_string_lossy()]);
    let admitted = plan_named(&value, &orphan);
    assert_eq!(admitted["outcome"], "moved");
    assert_eq!(admitted["admitted_by_caller"], true);
    let record = read_json(Path::new(value["record"].as_str().unwrap()));
    let named = record["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plan| plan["admitted_by_caller"] == true)
        .unwrap_or_else(|| panic!("the record says who admitted it: {record:#}"))
        .clone();
    assert!(
        named["from"].as_str().unwrap().ends_with("orphan"),
        "{named:#}"
    );
}

#[test]
fn a_tracked_path_refuses() {
    let site = finished(true);
    let laid = site.laid_plan(ISSUE);
    let checkout = site.checkout(301);
    let task = laid.join("tasks/01-fix.md");
    site.git(&checkout, &["add", "-f", &task.to_string_lossy()]);
    site.git(&checkout, &["commit", "-qm", "track the plan"]);
    site.refused(&["--item", ISSUE], "tracked", &["01-fix.md"]);
}

#[test]
fn a_failed_forge_lookup_refuses_in_the_forges_words() {
    let site = finished(false);
    site.on_forge(|model| model["closure_fails"] = json!("HTTP 502: the forge is down"));
    site.refused(
        &["--item", ISSUE],
        "forge",
        &["did not say whether", "HTTP 502"],
    );
    site.on_forge(|model| {
        model["closure_fails"] = Value::Null;
        model["closure_errors"] = json!("Something went wrong while executing your query");
    });
    site.refused(&["--item", ISSUE], "forge", &["Something went wrong"]);
}

#[test]
fn a_list_the_forge_cut_short_refuses() {
    let site = finished(false);
    site.on_forge(|model| model["cut_short"] = json!(["fix/issue-301"]));
    site.refused(
        &["--item", ISSUE],
        "forge",
        &["fix/issue-301", "cut short", "nothing was moved"],
    );
}

#[test]
fn an_unreadable_plan_refuses() {
    let site = finished(true);
    let laid = site.laid_plan(ISSUE);
    fs::write(laid.join("index.rhei.md"), b"# Rhei: \xff\xfe broken\n").unwrap();
    site.refused(&["--item", ISSUE], "unreadable", &["index.rhei.md"]);
}

#[cfg(unix)]
#[test]
fn an_archive_reached_through_a_symlink_into_a_work_root_refuses() {
    let site = finished(false);
    let inside = site.shared_root().join("keep");
    fs::create_dir_all(&inside).unwrap();
    std::os::unix::fs::symlink(&inside, site.data()).unwrap();
    site.refused(&["--item", ISSUE], "archive", &[]);
}

#[test]
fn act_is_refused_by_name() {
    let site = finished(false);
    let before = site.snapshot();
    let out = site.run(&["work", "retire", "--item", ISSUE, "--act", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", said(&out));
    let outcome = shaped("outcome", &out);
    assert_eq!(outcome["ok"], false);
    assert!(
        said(&out).contains("work retire does not take --act"),
        "{}",
        said(&out)
    );
    untouched(&before, &site.snapshot());
}

#[test]
fn a_forge_without_the_capability_is_refused_in_its_own_sentence() {
    let world = World::new();
    world.stub(
        "ephor-forge-acmeforge",
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail
cat > /dev/null
case "${{1:?subcommand}}" in
  capabilities) printf '{{"pull_requests":true}}' ;;
  pull-requests)
    state="$(cat "{state}")"
    printf '[{{"id":"app/101","repo":"app","number":"101","title":"Widen the window","url":"https://acme.example/pr/101","branch":"you/101","updated_at":"2026-07-30T12:00:00Z","role":"author","state":"%s","cited":false,"threads":[]}}]' "$state" ;;
  *) printf '[]' ;;
esac
"#,
            state = world.path().join("acme.state").display()
        ),
    );
    world.stub(
        "rhei",
        &RUNNER.replace("@WORLD@", &world.path().to_string_lossy()),
    );
    fs::write(world.path().join("acme.state"), "open").unwrap();
    world.configure(json!({
        "projects": { PROJECT: {
            "providers": [{ "provider": "acmeforge", "user": "you", "repos": ["app"] }],
            "work": { "root": "{root}/panta", "recipes": [{
                "id": "answer", "icon": "a", "description": "answer", "state": "fix",
                "needs_checkout": false, "when": { "kinds": ["pr"] }, "brief": "{title}"
            }]}
        }},
        "work": { "runner": "rhei" }
    }));
    let data = world.path().join("data");
    let ephor = |args: &[&str]| -> Output {
        world
            .ephor_raw()
            .env("XDG_DATA_HOME", &data)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(ephor(&["refresh", PROJECT, "--quiet"]).status.success());
    let item = "acmeforge:app/101";
    assert!(
        world.has_matter(item),
        "the extension's pull request is a matter"
    );
    let out = ephor(&["work", "dispatch", "--item", item, "--recipe", "answer"]);
    assert!(out.status.success(), "{}", said(&out));
    fs::write(world.path().join("acme.state"), "merged").unwrap();
    assert!(ephor(&["refresh", PROJECT, "--quiet"]).status.success());

    let out = ephor(&["work", "retire", "--item", item, "--json"]);
    let value = published(&out);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert_eq!(value["refused"]["guard"], "forge", "{value:#}");
    let sentence = value["refused"]["says"].as_str().unwrap_or_default();
    assert!(sentence.contains("cannot say whether"), "{sentence}");
    assert!(sentence.contains("is over"), "{sentence}");
}

#[test]
fn an_open_pull_request_from_a_second_candidate_branch_refuses() {
    let site = finished(true);
    let mut ledger = site.ledger();
    let dispatches = ledger["entries"][ISSUE]["dispatches"]
        .as_array_mut()
        .unwrap();
    let mut second = dispatches
        .iter()
        .find(|dispatch| dispatch["recipe"] == "implement")
        .unwrap()
        .clone();
    let gone = site.world.forest().join("fix/issue-301-take-2");
    second["branch"] = json!("fix/issue-301-take-2");
    second["checkout"] = json!(gone);
    second["root"] = json!(gone.join("panta"));
    dispatches.push(second);
    write_json(&site.ledger_path(), &ledger);
    site.pull(
        340,
        json!({ "title": "second try", "state": "OPEN", "headRefName": "fix/issue-301-take-2" }),
    );
    site.refused(
        &["--item", ISSUE],
        "open-pull-request",
        &["#340", "fix/issue-301-take-2"],
    );
}

#[test]
fn an_open_pull_request_linked_to_the_issue_from_an_unrecorded_branch_refuses() {
    let site = finished(false);
    site.pull(
        341,
        json!({ "title": "follow-up", "state": "OPEN", "headRefName": "someone-elses-branch",
                "closes": [301] }),
    );
    site.issue(301, json!({ "closedBy": [331, 341] }));
    site.refused(&["--item", ISSUE], "open-pull-request", &["#341"]);
}

#[test]
fn a_placeholder_only_record_refuses_naming_pr_branch_until_the_caller_names_one() {
    let site = placeholder(false, false);
    site.refused(
        &["--item", PLACEHOLDER],
        "branch",
        &["'fix'", "directory of checkouts", "--pr-branch"],
    );
    let value = site.retired(&["--item", PLACEHOLDER, "--pr-branch", "fix/issue-249"]);
    let branch = value["eligibility"]["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|branch| branch["name"] == "fix/issue-249")
        .unwrap_or_else(|| panic!("{value:#}"))
        .clone();
    assert_eq!(strings(&branch["from"]), vec!["caller".to_string()]);
}

// --- 4. Recovery --------------------------------------------------------------

/// A retirement whose moves happened and whose ledger save did not: the
/// archive holds the plan, the record says `pending`, the ledger is the one
/// from before. Returns the first record's path.
fn interrupted(site: &Site) -> PathBuf {
    let ledger = fs::read(site.ledger_path()).unwrap();
    let value = site.retired(&["--item", ISSUE]);
    let record_path = PathBuf::from(value["record"].as_str().unwrap());
    let mut record = read_json(&record_path);
    record["state"] = json!("pending");
    write_json(&record_path, &record);
    fs::write(site.ledger_path(), ledger).unwrap();
    record_path
}

#[test]
fn a_pending_retirement_whose_save_never_happened_is_undone_then_retired_afresh() {
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    let first = interrupted(&site);
    assert!(!plan.exists());

    let value = site.retired(&["--item", ISSUE]);

    assert!(
        strings(&value["recovered"])
            .iter()
            .any(|said| said.contains(&first.to_string_lossy().into_owned())),
        "the pending record is named as recovered: {value:#}"
    );
    assert_eq!(read_json(&first)["state"], "rolled-back");
    let second = PathBuf::from(value["record"].as_str().unwrap());
    assert_ne!(second, first, "the retirement went on from a fresh reading");
    assert_eq!(read_json(&second)["state"], "committed");
    assert!(!plan.exists());
    let name = plan.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        found(&site.archive_root(), &name).len(),
        1,
        "one archived copy"
    );
    assert_eq!(
        site.entry(ISSUE)["retirements"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn a_pending_retirement_whose_save_happened_is_marked_committed() {
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    let value = site.retired(&["--item", ISSUE]);
    let record_path = PathBuf::from(value["record"].as_str().unwrap());
    let mut record = read_json(&record_path);
    record["state"] = json!("pending");
    write_json(&record_path, &record);

    let (_, again) = site.retire(&["--item", ISSUE]);

    assert!(
        strings(&again["recovered"])
            .iter()
            .any(|said| said.contains(&record_path.to_string_lossy().into_owned())),
        "{again:#}"
    );
    assert_eq!(read_json(&record_path)["state"], "committed");
    assert!(!plan.exists(), "nothing was moved back");
    let name = plan.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(found(&site.archive_root(), &name).len(), 1);
}

#[test]
fn a_restore_never_replaces_a_path_now_occupied() {
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    let first = interrupted(&site);
    let newer = "# Rhei: laid again by a sync after the crash\n";
    fs::write(&plan, newer).unwrap();

    let value = site.refused(&["--item", ISSUE], "recovery", &[]);

    assert!(
        strings(&value["refused"]["paths"])
            .iter()
            .any(|path| Path::new(path) == plan),
        "the occupied path is named as not restored: {value:#}"
    );
    assert_eq!(
        fs::read_to_string(&plan).unwrap(),
        newer,
        "the newer plan is kept"
    );
    let name = plan.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        found(&site.archive_root(), &name).len(),
        1,
        "the archived copy stays in the archive"
    );
    assert_ne!(read_json(&first)["state"], "committed");
}

#[test]
fn a_failed_ledger_save_rolls_every_move_back() {
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    let original = fs::read(&plan).unwrap();
    let ledger = fs::read(site.ledger_path()).unwrap();
    fs::create_dir_all(site.ledger_path().with_extension("json.tmp")).unwrap();

    let (out, value) = site.retire(&["--item", ISSUE]);

    assert!(!out.status.success(), "{}", said(&out));
    assert_eq!(value["retired"], false);
    assert_eq!(
        fs::read(&plan).unwrap(),
        original,
        "the plan is back, byte for byte"
    );
    assert_eq!(fs::read(site.ledger_path()).unwrap(), ledger);
    let records = found(&site.archive_root(), "retirement.json");
    assert_eq!(records.len(), 1, "the attempt is on record");
    assert_eq!(read_json(&records[0])["state"], "rolled-back");
    let name = plan.file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        found(&site.archive_root(), &name).is_empty(),
        "nothing stays in the archive"
    );
}

// --- 5. Reopen -----------------------------------------------------------------

#[test]
fn after_a_reopen_new_work_is_a_new_plan_that_points_at_the_archive() {
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    let value = site.retired(&["--item", ISSUE]);
    let archive = PathBuf::from(value["archive"].as_str().unwrap());
    let record = value["record"].as_str().unwrap().to_string();
    let mut archived = Snapshot::new();
    walk(&archive, &mut archived);

    site.issue(301, reopened_now());
    site.refresh();
    site.ok(&["work", "sync", "--project", PROJECT]);

    let reopened = fs::read_to_string(&plan).expect("a new plan where the old one was");
    assert!(
        reopened.contains(&record),
        "it names the record:\n{reopened}"
    );
    assert!(
        reopened.contains(&archive.to_string_lossy().into_owned()),
        "and where the earlier work is kept:\n{reopened}"
    );
    assert!(
        !reopened
            .lines()
            .any(|line| line.contains("**Prior:**") && line.contains("archive")),
        "nothing waits on the archive:\n{reopened}"
    );
    let mut now = Snapshot::new();
    walk(&archive, &mut now);
    assert_eq!(now, archived, "the archive is untouched");
}

#[test]
fn a_workflow_laid_after_a_reopen_takes_a_fresh_id_and_names_the_archive() {
    // A laid workflow is the other kind of new work after a retirement: its
    // dossier carries the paragraph a recipe plan's first ticket does, and it
    // takes a fresh id rather than the retired plan's (§FS-005-dispatch.5).
    let site = finished(false);
    let retired_plan = site.laid_plan(ISSUE);
    let value = site.retired(&["--item", ISSUE]);
    let archive = value["archive"].as_str().unwrap().to_string();
    let record = value["record"].as_str().unwrap().to_string();

    site.issue(301, reopened_now());
    site.refresh();
    site.lay(ISSUE);

    let entry = site.entry(ISSUE);
    let laid: Vec<&Value> = entry["dispatches"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|dispatch| dispatch["recipe"] == "implement")
        .collect();
    assert_eq!(laid.len(), 2, "{entry:#}");
    let (old, new) = (laid[0], laid[1]);
    assert!(old["retired_at"].is_string(), "{entry:#}");
    assert!(new["retired_at"].is_null(), "{entry:#}");
    assert_eq!(old["root"], new["root"], "laid in the same root: {entry:#}");
    let old_id = old["plan"].as_str().unwrap();
    let new_id = new["plan"].as_str().unwrap();
    assert_ne!(new_id, old_id, "a fresh id, not the retired plan's");
    assert_eq!(new_id, format!("{old_id}-2"));
    assert_eq!(
        retired_plan.file_name().unwrap().to_string_lossy(),
        old_id,
        "the retired dispatch still names where its plan was"
    );

    let root = PathBuf::from(new["root"].as_str().unwrap());
    let dossier = fs::read_to_string(root.join(".ephor").join(new_id).join("dossier.md"))
        .expect("the new plan's dossier");
    assert!(dossier.contains(&record), "it names the record:\n{dossier}");
    assert!(
        dossier.contains(&archive),
        "and where the earlier work is kept:\n{dossier}"
    );

    let board = board_of_301(&site);
    let retired_lines = board
        .iter()
        .filter(|line| line.contains("retired →"))
        .count();
    assert_eq!(retired_lines, 1, "one line per archive:\n{board:#?}");
    assert!(
        board.iter().any(|line| line.contains(new_id)),
        "the live plan is shown:\n{board:#?}"
    );

    let list = site.ok(&["work", "list", "--json"]);
    let rows = json_of(&list);
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["item"] == ISSUE)
        .expect("#301's row");
    let workflows = row["workflows"].as_array().expect("workflows");
    let by_plan = |id: &str| {
        workflows
            .iter()
            .find(|workflow| workflow["plan"] == id)
            .unwrap_or_else(|| panic!("no workflow {id} in {row:#}"))
    };
    assert!(by_plan(old_id)["retired_at"].is_string(), "{row:#}");
    assert!(by_plan(new_id).get("retired_at").is_none(), "{row:#}");
}

// --- 6. Forget -----------------------------------------------------------------

#[test]
fn forget_leaves_a_retired_entry_to_item_and_item_names_the_archive() {
    let site = finished(false);
    let value = site.retired(&["--item", ISSUE]);
    let archive = value["archive"].as_str().unwrap().to_string();

    site.ok(&["work", "forget", "--missing"]);
    assert!(
        site.entry(ISSUE).is_object(),
        "a readable archive is not missing"
    );
    site.ok(&["work", "forget", "--done"]);
    assert!(
        site.entry(ISSUE).is_object(),
        "answer-1 is still at fix in the archive, so the entry is not done"
    );

    let out = site.ok(&["work", "forget", "--item", ISSUE, "--json"]);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(site.entry(ISSUE).is_null());
    let forgot = json_of(&out);
    let retired_to: Vec<String> = forgot["forgot"][0]["plans"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|plan| plan["retired_to"].as_str().map(str::to_string))
        .collect();
    assert!(
        retired_to.iter().any(|to| to.starts_with(&archive)),
        "forget names where the plan was retired to:\n{text}"
    );
}

#[test]
fn a_deleted_archive_reads_as_missing_and_forget_missing_takes_it() {
    let site = finished(false);
    let value = site.retired(&["--item", ISSUE]);
    let archive = PathBuf::from(value["archive"].as_str().unwrap());
    fs::remove_dir_all(&archive).unwrap();

    let list = site.ok(&["work", "list"]);
    let list = String::from_utf8_lossy(&list.stdout);
    assert!(list.contains("archive missing"), "{list}");
    site.ok(&["work", "forget", "--missing"]);
    assert!(
        site.entry(ISSUE).is_null(),
        "--missing is the sweep for it now"
    );
}

// --- 7. Dry run ------------------------------------------------------------------

#[test]
fn a_dry_run_writes_nothing_not_even_a_lock_file() {
    let site = finished(false);
    let plan = site.recipe_plan(ISSUE);
    fs::remove_file(format!("{}.lock", plan.display())).unwrap();
    let before = site.snapshot();

    let (out, value) = site.retire(&["--item", ISSUE, "--dry-run"]);

    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["retired"], false, "a dry run retires nothing");
    assert_eq!(
        plan_named(&value, &plan)["outcome"],
        "would-move",
        "{value:#}"
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains(ISSUE));
    assert_eq!(site.snapshot(), before, "not one byte, not one lock file");
    assert!(!site.archive_root().exists());

    let prose = site.ok(&["work", "retire", "--item", ISSUE, "--dry-run"]);
    assert!(
        String::from_utf8_lossy(&prose.stdout).contains("would move"),
        "{}",
        said(&prose)
    );
}

// --- 8. Help, the action, and the shape -------------------------------------------

#[test]
fn work_retire_help_refuses_act_and_the_selectors() {
    let site = Site::new();
    let out = site.ok(&["work", "retire", "--help"]);
    let help = String::from_utf8_lossy(&out.stdout);
    for flag in [
        "--item",
        "--project",
        "--branch",
        "--admit",
        "--pr-branch",
        "--why",
        "--dry-run",
        "--json",
    ] {
        assert!(help.contains(flag), "the help lists {flag}:\n{help}");
    }
    assert!(help.contains("work retire does not take --act."), "{help}");
    assert!(help.contains("sweeps no set of projects"), "{help}");
    assert!(
        help.contains("work retire does not take --workspace"),
        "{help}"
    );
}

#[test]
fn the_retire_action_is_offered_on_a_finished_matter_and_asks_first() {
    let site = finished(false);
    let out = site.ok(&["actions", "--item", ISSUE, "--json"]);
    let actions = json_of(&out);
    let retire = actions["offers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|offer| offer["id"] == "retire")
        .unwrap_or_else(|| panic!("retire is offered on a finished matter: {actions:#}"))
        .clone();
    assert_eq!(retire["confirm"], true, "{retire:#}");

    // Where the entry stands on an open matter it is the row that says why,
    // never one that runs (§FS-004-quick-actions.10).
    let open = json_of(&site.ok(&["actions", "--item", NEIGHBOUR, "--json"]));
    assert!(
        open["offers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|offer| offer["id"] == "retire")
            .all(|offer| offer["gate"] == "blocked"),
        "an open matter is not offered retirement: {open:#}"
    );

    let before = site.snapshot();
    let out = site.run(&["actions", "run", "retire", "--item", ISSUE]);
    assert!(!out.status.success(), "it asks first:\n{}", said(&out));
    assert!(said(&out).contains("--yes"), "{}", said(&out));
    untouched(&before, &site.snapshot());

    let out = site.run(&["actions", "run", "retire", "--item", ISSUE, "--yes"]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(!site.recipe_plan(ISSUE).exists());
}

#[test]
fn the_branch_action_is_offered_where_the_branch_names_one_finished_matter() {
    let site = placeholder(true, true);
    let out = site.ok(&[
        "actions",
        "--project",
        PROJECT,
        "--branch",
        "fix/issue-249",
        "--json",
    ]);
    let actions = json_of(&out);
    assert!(
        actions["offers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|offer| offer["id"] == "retire"),
        "{actions:#}"
    );
}

/// The retire entry on one matter's menu, where the menu has one.
fn retire_entry(site: &Site, item: &str) -> Option<Value> {
    let actions = json_of(&site.ok(&["actions", "--item", item, "--json"]));
    actions["offers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|offer| offer["id"] == "retire")
        .cloned()
}

/// §FS-004-quick-actions.10: where retiring is not offered on a matter, the
/// entry stands with the note in the reader's terms — the item is still open,
/// or nothing recorded about it is left — and running it refuses with the same
/// note and moves nothing. An open matter with nothing recorded says nothing.
#[test]
fn where_retiring_is_not_offered_the_matter_menu_says_why() {
    let site = finished(false);
    site.issue(303, json!({ "title": "Untouched", "state": "OPEN" }));
    site.refresh();

    let open =
        retire_entry(&site, NEIGHBOUR).expect("an open matter with recorded work carries the note");
    assert_eq!(open["gate"], "blocked", "{open:#}");
    let note = open["refusal"].as_str().expect("the note");
    assert!(note.contains("still open"), "{open:#}");
    let before = site.snapshot();
    let out = site.run(&["actions", "run", "retire", "--item", NEIGHBOUR, "--yes"]);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains(note), "{}", said(&out));
    untouched(&before, &site.snapshot());

    site.retired(&["--item", ISSUE]);
    let done = retire_entry(&site, ISSUE)
        .expect("a finished matter with nothing left to retire carries the note");
    assert_eq!(done["gate"], "blocked", "{done:#}");
    assert!(
        done["refusal"]
            .as_str()
            .is_some_and(|note| note.contains("no recorded work")),
        "{done:#}"
    );

    assert_eq!(
        retire_entry(&site, "github-issues:acme/widget#303"),
        None,
        "an open matter with no recorded work has nothing to say about retiring"
    );
}

/// §FS-004-quick-actions.10: a branch carrying two matters offers neither,
/// and its note names each so the reader can retire each by its item.
#[test]
fn a_branch_carrying_two_matters_names_each_instead_of_offering() {
    let site = finished(true);
    let mut ledger = site.ledger();
    ledger["entries"][NEIGHBOUR]["branch"] = json!("fix/issue-301");
    write_json(&site.ledger_path(), &ledger);

    let actions = json_of(&site.ok(&[
        "actions",
        "--project",
        PROJECT,
        "--branch",
        "fix/issue-301",
        "--json",
    ]));
    let retire = actions["offers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|offer| offer["id"] == "retire")
        .unwrap_or_else(|| panic!("the branch row carries the note: {actions:#}"));
    assert_eq!(retire["gate"], "blocked", "{retire:#}");
    let note = retire["refusal"].as_str().expect("the note");
    for item in [ISSUE, NEIGHBOUR] {
        assert!(note.contains(item), "{item} is named: {note}");
    }
    assert!(note.contains("--item"), "{note}");
}

#[test]
fn the_published_shape_is_printed_by_schema_views() {
    let site = Site::new();
    let out = site.ok(&["schema", "views"]);
    let schema = json_of(&out);
    let text = schema.to_string();
    assert!(
        text.contains("work-retire"),
        "ephor schema views publishes work-retire"
    );
}
