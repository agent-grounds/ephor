//! §FS-001-forge-interface.2, out-of-process transport, end to end.
//!
//! The extension below is a real shell script — no Rust, no compilation — and
//! it is the whole implementation of a forge. If this test passes, `jq` over a
//! vendor CLI is a complete way to add one.

mod common;

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use common::*;

/// A forge extension in bash. It reads the request on stdin, answers each
/// subcommand with JSON on stdout, and decides for itself which messages are
/// the user's — the one identity question policy cannot answer for it.
const FAKE_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
request="$(cat)"
me=$(printf '%s' "$request" | jq -r '.config.user')

case "${1:?subcommand}" in
  capabilities)
    printf '{"pull_requests":true,"conversation":true,"gate":true,"issues":true,"tasks":true,"replies":true}'
    ;;
  pull-requests)
    jq -n --arg me "$me" '[
      {
        id: "app/101", repo: "app", number: "101",
        title: "Widen the retry window",
        url: "https://forge.example/pr/101",
        branch: "you/ABC-42-retry",
        updated_at: "2026-07-30T12:00:00Z",
        role: "author", state: "open:needs_work", cited: false,
        threads: [ { messages: [
          { author: "Other Dev", text: "Please widen it.", when: "2026-07-30T11:00:00Z", mine: false },
          { author: $me,        text: "Done.",            when: "2026-07-30T11:30:00Z", mine: true  }
        ] } ],
        gate: { repos: [ { repo: "app", passed: 5, failed: 1, running: 0 },
                         { repo: "plugins", passed: 2, failed: 0, running: 1 } ] }
      },
      {
        id: "plugins/202", repo: "plugins", number: "202",
        title: "Cache the resolver",
        url: "https://forge.example/pr/202",
        branch: "someone/cache",
        updated_at: "2026-07-29T12:00:00Z",
        role: "reviewer", state: "open:mentioned", cited: true,
        threads: [ { messages: [
          { author: "Other Dev", text: "what do you think?", when: "2026-07-29T11:00:00Z", mine: false }
        ] } ]
      },
      {
        id: "app/303", repo: "app", number: "303",
        title: "Bump the parser",
        url: "https://forge.example/pr/303",
        updated_at: "2026-07-28T12:00:00Z",
        role: "author", state: "open", cited: false,
        threads: [
          { messages: [
            { author: "Robot", text: "Read the performance policy.", mine: false },
            { author: "Robot", text: "I acknowledge the performance impact.", mine: false,
              task: { state: "open", comment: "c-1" } } ] },
          { messages: [
            { author: "Robot", text: "Security review needed?", mine: false },
            { author: "Robot", text: "Security requirements satisfied.", mine: false,
              task: { state: "resolved", comment: "c-2" } } ] }
        ]
      },
      {
        id: "app/404", repo: "app", number: "404",
        title: "Every box ticked",
        url: "https://forge.example/pr/404",
        updated_at: "2026-07-27T12:00:00Z",
        role: "author", state: "open", cited: false,
        threads: [ { messages: [
          { author: "Robot", text: "Read the performance policy.", mine: false },
          { author: "Robot", text: "I acknowledge the performance impact.", mine: false,
            task: { state: "resolved", comment: "c-3" } } ] } ]
      }
    ]'
    ;;
  resolve-task)
    log="$(printf '%s' "$request" | jq -r '.config.resolved_log // ""')"
    if [ -n "$log" ]; then
      printf '%s' "$request" | jq -c '.target' > "$log"
    fi
    printf '{}'
    ;;
  reply)
    log="$(printf '%s' "$request" | jq -r '.config.replied_log // ""')"
    if [ -n "$log" ]; then
      printf '%s' "$request" | jq -c '{target, text}' > "$log"
    fi
    printf '{}'
    ;;
  issues)
    jq -n --argjson tickets "$(printf '%s' "$request" | jq '.tickets')" '[
      $tickets[] | {
        key: ., title: "Retry window is too narrow",
        status: "In Progress",
        url: ("https://tracker.example/browse/" + .),
        updated_at: "2026-07-29T09:12:00Z",
        messages: [ { author: "Other Dev", text: "Reproduced on staging.",
                      when: "2026-07-28T10:00:00Z", mine: false } ]
      } ]'
    ;;
  *)
    echo "unknown subcommand: $1" >&2
    exit 2
    ;;
esac
"#;

fn extension_env(tmp: &Path) -> Vec<(String, String)> {
    vec![
        (
            "XDG_STATE_HOME".to_string(),
            tmp.join("state").to_string_lossy().into_owned(),
        ),
        (
            "EPHOR_STATUS_CONFIG".to_string(),
            tmp.join("status.json").to_string_lossy().into_owned(),
        ),
        (
            "EPHOR_REGISTRY".to_string(),
            tmp.join("workspaces.json").to_string_lossy().into_owned(),
        ),
    ]
}

fn write_fixture(tmp: &Path) {
    let template = write_template(tmp);
    let project_root = tmp.join("demo");
    fs::create_dir_all(&project_root).unwrap();

    write_registry(
        &tmp.join("workspaces.json"),
        &json!({
            "project_types": base_project_types(&template),
            "hook_sets": [],
            "projects": [{
                "id": "demo",
                "type": "monorepo",
                "display_name": "Demo",
                "root": project_root.to_string_lossy(),
                "main_branch": "main",
                "branches": [
                    { "id": "demo-ticket", "branch": "you/ABC-42-retry", "active": true, "ticket": "ABC-42" }
                ]
            }]
        }),
    );

    fs::write(
        tmp.join("status.json"),
        serde_json::to_string_pretty(&json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "projects": {
                "demo": {
                    // Names no built-in provider, so ephor resolves
                    // `ephor-forge-demoforge` on PATH. Every other key is
                    // opaque to ephor and handed to the extension.
                    "providers": [
                        { "provider": "demoforge", "user": "dev", "repos": ["app", "plugins"] }
                    ]
                }
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

fn path_with_extension(tmp: &Path) -> String {
    let fake_bin = tmp.join("fakebin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("ephor-forge-demoforge"), FAKE_FORGE);
    format!(
        "{}:{}",
        fake_bin.to_string_lossy(),
        std::env::var("PATH").unwrap_or_default()
    )
}

#[test]
fn a_bash_extension_is_a_complete_forge_implementation() {
    let tmp = tempdir();
    write_fixture(tmp.path());
    let path = path_with_extension(tmp.path());

    let mut cmd = ephor_cmd();
    cmd.env("PATH", &path);
    for (key, value) in extension_env(tmp.path()) {
        cmd.env(key, value);
    }
    cmd.args(["refresh", "demo"]).assert().success();

    let cache: Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("state/ephor/feed/demo.json")).unwrap(),
    )
    .unwrap();
    let items = cache["providers"]["demoforge"]["matters"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        items.len(),
        5,
        "four pull requests and one issue: {items:#?}"
    );

    let authored = items
        .iter()
        .find(|i| i["key"] == "demoforge:app/101")
        .unwrap();
    // Policy ran over out-of-process data exactly as it would in process: a
    // needs_work verdict stands even though the user had the last word in the
    // thread — answering a comment does not clear a review.
    assert_eq!(authored["role"], "author");
    assert_eq!(authored["needs_response"], true);
    assert_eq!(authored["raw"]["branch"], "you/ABC-42-retry");
    assert_eq!(authored["raw"]["gate"]["repos"][1]["running"], 1);
    assert_eq!(
        authored["raw"]["threads"][0]["messages"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    // Cited, and the last word is not the user's: unanswered.
    let reviewing = items
        .iter()
        .find(|i| i["key"] == "demoforge:plugins/202")
        .unwrap();
    assert_eq!(reviewing["role"], "reviewer");
    assert_eq!(reviewing["needs_response"], true);

    // Bot checklists: two threads nobody of the user's ever spoke in, so the
    // last word can never be theirs. One box is open and one is ticked, and
    // the open one is the whole reason this awaits an answer
    // (§FS-003-feed-categories.4). The descriptors survive verbatim into the
    // item, which is what the thread screen draws and ticks from.
    let checklists = items
        .iter()
        .find(|i| i["key"] == "demoforge:app/303")
        .unwrap();
    assert_eq!(checklists["needs_response"], true);
    assert_eq!(
        checklists["raw"]["threads"][0]["messages"][1]["task"],
        json!({ "state": "open", "comment": "c-1" })
    );
    assert_eq!(
        checklists["raw"]["threads"][1]["messages"][1]["task"],
        json!({ "state": "resolved", "comment": "c-2" })
    );

    // And with every box ticked it is done, though a robot had the last word
    // in every thread and the user never wrote a line.
    let ticked = items
        .iter()
        .find(|i| i["key"] == "demoforge:app/404")
        .unwrap();
    assert_eq!(ticked["needs_response"], false, "{ticked:#?}");

    // The registry's active branch ticket reached the extension and came back
    // as an issue, with the same message shape as a conversation.
    let issue = items
        .iter()
        .find(|i| i["key"] == "demoforge:ABC-42")
        .unwrap();
    // Issues are their own category (§FS-003-feed-categories.1), and an
    // implementation that does not report a role is reporting the user's own.
    assert_eq!(issue["kind"], "issue");
    assert_eq!(issue["role"], "author");
    assert_eq!(issue["title"], "ABC-42 Retry window is too narrow");
    assert_eq!(issue["state"], "in progress");
    assert_eq!(issue["needs_response"], true);

    // The gate renders on the row like any other forge's.
    let mut cmd = ephor_cmd();
    cmd.env("PATH", &path);
    for (key, value) in extension_env(tmp.path()) {
        cmd.env(key, value);
    }
    cmd.args(["feed"])
        .assert()
        .success()
        .stdout(predicates::str::contains("✓7 ✗1 ⋯1"));
}

/// An extension that is not installed, or that answers rubbish, must degrade
/// to a provider warning — never take the refresh down.
#[test]
fn a_broken_extension_degrades_to_a_provider_warning() {
    let tmp = tempdir();
    write_fixture(tmp.path());
    let fake_bin = tmp.path().join("fakebin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(
        &fake_bin.join("ephor-forge-demoforge"),
        "#!/usr/bin/env bash\nprintf 'not json'\n",
    );
    let path = format!(
        "{}:{}",
        fake_bin.to_string_lossy(),
        std::env::var("PATH").unwrap_or_default()
    );

    let mut cmd = ephor_cmd();
    cmd.env("PATH", &path);
    for (key, value) in extension_env(tmp.path()) {
        cmd.env(key, value);
    }
    // Exit 3 = every provider failed; the run itself still completes.
    let output = cmd.args(["refresh", "demo"]).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("demoforge"), "{stderr}");
}

/// A forge that answers correctly, but slower than the shared default ceiling.
const SLOW_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cat >/dev/null
case "${1:?subcommand}" in
  capabilities)
    printf '{"pull_requests":true,"conversation":false,"gate":false,"issues":false}'
    ;;
  pull-requests)
    sleep 3
    jq -n '[ { id: "app/303", repo: "app", number: "303", title: "Slow but real",
               url: "https://forge.example/pr/303", branch: "you/slow",
               updated_at: "2026-07-30T12:00:00Z", role: "author",
               state: "open", cited: false, threads: [] } ]'
    ;;
  *) exit 2 ;;
esac
"#;

/// Install the slow forge and point a one-provider `demo` project at it.
/// `block_timeout` is the provider block's own `timeout_seconds`, if any.
fn slow_forge_fixture(tmp: &Path, block_timeout: Option<u64>) -> String {
    write_fixture(tmp);
    let mut provider = json!({ "provider": "slowforge", "user": "dev" });
    if let Some(seconds) = block_timeout {
        provider["timeout_seconds"] = json!(seconds);
    }
    fs::write(
        tmp.join("status.json"),
        serde_json::to_string_pretty(&json!({
            // One second: shorter than the forge, so only the block's own
            // ceiling can let it finish.
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 1 },
            "projects": { "demo": { "providers": [provider] } }
        }))
        .unwrap(),
    )
    .unwrap();

    let fake_bin = tmp.join("slowbin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("ephor-forge-slowforge"), SLOW_FORGE);
    format!(
        "{}:{}",
        fake_bin.to_string_lossy(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn refresh_slow_forge(tmp: &Path, path: &str) -> Value {
    let mut cmd = ephor_cmd();
    cmd.env("PATH", path);
    for (key, value) in extension_env(tmp) {
        cmd.env(key, value);
    }
    cmd.args(["refresh", "demo"]).output().unwrap();
    serde_json::from_str(&fs::read_to_string(tmp.join("state/ephor/feed/demo.json")).unwrap())
        .unwrap()
}

/// A provider block raises its own ceiling past `provider_timeout_seconds`.
/// A forge behind a VPN is slower than a local `gh` call, and sizing the
/// shared default for it would delay every other provider's failure.
#[test]
fn a_provider_block_raises_its_own_timeout() {
    let tmp = tempdir();
    let path = slow_forge_fixture(tmp.path(), Some(30));
    let slot = &refresh_slow_forge(tmp.path(), &path)["providers"]["slowforge"];

    assert_eq!(slot["ok"], true, "{slot:#?}");
    let items = slot["matters"].as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 1, "{items:#?}");
    assert_eq!(items[0]["key"], "slowforge:app/303");
}

/// The companion: without a block of its own, the same forge is held to the
/// default and times out. This is what proves the test above measures the
/// override rather than a ceiling that was generous all along.
#[test]
fn without_an_override_the_default_timeout_still_applies() {
    let tmp = tempdir();
    let path = slow_forge_fixture(tmp.path(), None);
    let slot = &refresh_slow_forge(tmp.path(), &path)["providers"]["slowforge"];

    assert_eq!(slot["ok"], false, "{slot:#?}");
    let error = slot["error"].as_str().unwrap_or_default();
    assert!(error.contains("timed out"), "{error}");
}

/// An extension that was never installed must say so by name. "missing tool
/// or secret" sends the reader looking for a credential, when what is missing
/// is an executable whose name appears nowhere in the configuration — the
/// provider block names the *forge*, and ephor derives the command from it.
#[test]
fn an_uninstalled_extension_names_the_executable_it_wanted() {
    let tmp = tempdir();
    write_fixture(tmp.path());
    // An empty directory: nothing named `ephor-forge-demoforge` anywhere.
    let empty_bin = tmp.path().join("emptybin");
    fs::create_dir_all(&empty_bin).unwrap();

    let mut cmd = ephor_cmd();
    cmd.env("PATH", empty_bin.to_string_lossy().into_owned());
    for (key, value) in extension_env(tmp.path()) {
        cmd.env(key, value);
    }
    cmd.args(["refresh", "demo"]).output().unwrap();

    let cache: Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("state/ephor/feed/demo.json")).unwrap(),
    )
    .unwrap();
    let error = cache["providers"]["demoforge"]["error"]
        .as_str()
        .unwrap_or_default();
    assert!(
        error.contains("ephor-forge-demoforge") && error.contains("PATH"),
        "diagnostic must name the executable and where it looked: {error}"
    );
}

/// Install a forge that fails a given way, and return its refreshed cache
/// slot plus the exit code and stderr of the refresh that produced it.
fn refresh_with_forge(script: &str) -> (Value, i32, String) {
    let tmp = tempdir();
    write_fixture(tmp.path());
    let fake_bin = tmp.path().join("failbin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("ephor-forge-demoforge"), script);
    let path = format!(
        "{}:{}",
        fake_bin.to_string_lossy(),
        std::env::var("PATH").unwrap_or_default()
    );

    let mut cmd = ephor_cmd();
    cmd.env("PATH", &path);
    for (key, value) in extension_env(tmp.path()) {
        cmd.env(key, value);
    }
    let output = cmd.args(["refresh", "demo"]).output().unwrap();
    let cache: Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("state/ephor/feed/demo.json")).unwrap(),
    )
    .unwrap();
    (
        cache["providers"]["demoforge"].clone(),
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// A destination that cannot be reached is its own condition, not a generic
/// warning: nothing about the setup is wrong, the items on screen are last
/// known values, and the fix is a network rather than a config edit.
#[test]
fn an_unreachable_destination_is_reported_as_unreachable() {
    let (slot, _code, stderr) = refresh_with_forge(
        "#!/usr/bin/env bash\n\
         echo 'java.net.UnknownHostException: bitbucket.example.com' >&2\n\
         exit 1\n",
    );

    assert_eq!(slot["ok"], false, "{slot:#?}");
    assert_eq!(slot["unreachable"], true, "{slot:#?}");
    assert!(
        stderr.contains("unreachable"),
        "the refresh must say so on stderr: {stderr}"
    );
}

/// The converse: a failure the reader has to fix must not be filed under
/// "the network is down", or they will wait for it to heal on its own.
#[test]
fn a_broken_extension_is_not_reported_as_unreachable() {
    let (slot, _code, stderr) = refresh_with_forge(
        "#!/usr/bin/env bash\necho 'bad configuration: unknown project key' >&2\nexit 1\n",
    );

    assert_eq!(slot["ok"], false, "{slot:#?}");
    // Absent or false — a healthy-shaped slot does not carry the marker.
    assert!(!slot["unreachable"].as_bool().unwrap_or(false), "{slot:#?}");
    assert!(!stderr.contains("unreachable"), "{stderr}");
}

/// Losing every provider is exit 3, as it always was.
#[test]
fn losing_every_provider_still_exits_three() {
    let (_slot, code, stderr) =
        refresh_with_forge("#!/usr/bin/env bash\necho 'exploded' >&2\nexit 1\n");

    assert_eq!(code, 3, "{stderr}");
    assert!(
        stderr.contains("error:") && stderr.contains("demoforge"),
        "the failure must name the provider: {stderr}"
    );
}

/// The case that used to pass silently: one provider dies, another survives,
/// and the refresh reports success. That is how a forge stays uninstalled for
/// months — the timer running this sees exit 0 every time, and the section of
/// the feed it should have filled just looks like a quiet week.
#[test]
fn a_partly_lost_refresh_fails_explicitly() {
    let tmp = tempdir();
    write_fixture(tmp.path());
    let fake_bin = tmp.path().join("mixedbin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("ephor-forge-goodforge"), FAKE_FORGE);
    make_executable(
        &fake_bin.join("ephor-forge-deadforge"),
        "#!/usr/bin/env bash\necho 'exploded' >&2\nexit 1\n",
    );
    fs::write(
        tmp.path().join("status.json"),
        serde_json::to_string_pretty(&json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "projects": { "demo": { "providers": [
                { "provider": "goodforge", "user": "dev", "repos": ["app"] },
                { "provider": "deadforge", "user": "dev", "repos": ["app"] }
            ] } }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut cmd = ephor_cmd();
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            fake_bin.to_string_lossy(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    for (key, value) in extension_env(tmp.path()) {
        cmd.env(key, value);
    }
    let output = cmd.args(["refresh", "demo"]).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(4),
        "a refresh that lost a provider must not report success: {stderr}"
    );
    assert!(
        stderr.contains("deadforge"),
        "the lost provider must be named: {stderr}"
    );

    // The survivor still delivered: an explicit failure must not cost the
    // providers that worked.
    let cache: Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("state/ephor/feed/demo.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cache["providers"]["goodforge"]["ok"], true);
    assert!(!cache["providers"]["goodforge"]["matters"]
        .as_array()
        .unwrap()
        .is_empty());
}

/// A shared source is a lost provider like any other
/// (§FS-001-forge-interface.6). It is the leg that reads the forge's own
/// notice list — the completeness capability — so a run that lost it and still
/// exited 0 is exactly how a source stays dark indefinitely: whatever runs the
/// refresh on a timer sees success and nobody is told.
#[test]
fn a_lost_shared_source_makes_the_run_fail_even_when_every_project_answered() {
    let tmp = tempdir();
    write_fixture(tmp.path());
    let fake_bin = tmp.path().join("sharedbin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("ephor-forge-goodforge"), FAKE_FORGE);
    // A site-level source that asks nothing about any one project, and fails
    // whatever it is asked.
    make_executable(
        &fake_bin.join("ephor-forge-deadgateway"),
        "#!/usr/bin/env bash
echo 'the gateway is not running' >&2
exit 1
",
    );
    fs::write(
        tmp.path().join("status.json"),
        serde_json::to_string_pretty(&json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "sources": [{ "provider": "deadgateway" }],
            "projects": { "demo": { "providers": [
                { "provider": "goodforge", "user": "dev", "repos": ["app"] }
            ] } }
        }))
        .unwrap(),
    )
    .unwrap();

    let mut cmd = ephor_cmd();
    cmd.env(
        "PATH",
        format!(
            "{}:{}",
            fake_bin.to_string_lossy(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    for (key, value) in extension_env(tmp.path()) {
        cmd.env(key, value);
    }
    let output = cmd.args(["refresh", "demo"]).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(4),
        "losing the shared leg must not report success: {stderr}"
    );
    assert!(
        stderr.contains("sources:"),
        "the lost shared source must be named: {stderr}"
    );

    // The project's own source still delivered.
    let cache: Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("state/ephor/feed/demo.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cache["providers"]["goodforge"]["ok"], true);
}

/// The `capabilities` probe is the forge's first call, so it is where an
/// unreachable host shows up. Its error must be reported as itself — it used
/// to be replaced with "declared no capabilities", which describes a working
/// extension behind a downed VPN as a malformed one.
#[test]
fn a_failing_capabilities_probe_reports_its_own_error() {
    let (slot, _code, _stderr) = refresh_with_forge(
        "#!/usr/bin/env bash\n\
         if [ \"$1\" = capabilities ]; then\n\
           echo 'connection refused by gateway' >&2\n\
           exit 1\n\
         fi\n\
         printf '[]'\n",
    );

    let error = slot["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("connection refused"),
        "the probe's real error must survive: {error}"
    );
    assert_eq!(slot["unreachable"], true, "{slot:#?}");
}

/// §FS-004-quick-actions.5: a ticked box goes back out over the same
/// transport, and the descriptor the implementation wrote for itself arrives
/// verbatim. ephor read `state` out of it and understood nothing else — which
/// is what lets a forge name its tasks however its API does.
#[test]
fn ticking_a_task_reaches_the_extension_verbatim() {
    use ephor::forge::external::ExternalForge;
    use ephor::forge::{Forge, Request};

    let tmp = tempdir();
    let extension = tmp.path().join("ephor-forge-demoforge");
    make_executable(&extension, FAKE_FORGE);
    let log = tmp.path().join("resolved.json");

    let forge = ExternalForge::new("demoforge", Some(extension.to_string_lossy().into_owned()));
    assert!(
        forge.capabilities().unwrap().tasks,
        "an extension declares that it can tick"
    );

    let target = json!({ "state": "open", "repo": "app", "pull_request": "303", "comment": "c-1" });
    forge
        .resolve_task(
            &Request {
                config: json!({ "user": "dev", "resolved_log": log.to_string_lossy() }),
                project: "demo".to_string(),
                tickets: Vec::new(),
                user: Some("dev".to_string()),
                timeout_seconds: 10,
            },
            &target,
        )
        .unwrap();

    let sent: Value = serde_json::from_str(&fs::read_to_string(&log).unwrap()).unwrap();
    assert_eq!(sent, target);
}

/// §FS-005-dispatch.13: an answer a run drafted goes out over the same
/// transport, in the reader's own words, with the thread descriptor the
/// implementation wrote for itself arriving verbatim. ephor never posts on its
/// own — this is the call a person made.
#[test]
fn a_reply_reaches_the_extension_with_the_words_and_the_thread() {
    use ephor::forge::external::ExternalForge;
    use ephor::forge::{Forge, Request};

    let tmp = tempdir();
    let extension = tmp.path().join("ephor-forge-demoforge");
    make_executable(&extension, FAKE_FORGE);
    let log = tmp.path().join("replied.json");

    let forge = ExternalForge::new("demoforge", Some(extension.to_string_lossy().into_owned()));
    assert!(
        forge.capabilities().unwrap().replies,
        "an extension declares that its channels can carry a reply"
    );

    let target = json!({ "kind": "conversation", "repo": "app", "pull_request": "101" });
    forge
        .reply(
            &Request {
                config: json!({ "user": "dev", "replied_log": log.to_string_lossy() }),
                project: "demo".to_string(),
                tickets: Vec::new(),
                user: Some("dev".to_string()),
                timeout_seconds: 10,
            },
            &target,
            "Widened to 30s — see the test in retry_test.rs.",
        )
        .unwrap();

    let sent: Value = serde_json::from_str(&fs::read_to_string(&log).unwrap()).unwrap();
    assert_eq!(sent["target"], target);
    assert_eq!(
        sent["text"],
        "Widened to 30s — see the test in retry_test.rs."
    );
}

/// An implementation that reads task state without writing it declares no
/// tasks capability, and the refusal names the forge rather than failing
/// somewhere further in (§FS-001-forge-interface.1).
#[test]
fn an_extension_that_cannot_tick_says_so() {
    use ephor::forge::external::ExternalForge;
    use ephor::forge::{Forge, Request};

    let tmp = tempdir();
    let extension = tmp.path().join("ephor-forge-readonly");
    make_executable(
        &extension,
        "#!/usr/bin/env bash\n\
         set -euo pipefail\n\
         cat >/dev/null\n\
         case \"$1\" in\n\
           capabilities) printf '{\"pull_requests\":true,\"conversation\":true}' ;;\n\
           *) echo \"unknown subcommand: $1\" >&2; exit 2 ;;\n\
         esac\n",
    );

    let forge = ExternalForge::new("readonly", Some(extension.to_string_lossy().into_owned()));
    assert!(!forge.capabilities().unwrap().tasks);

    let error = forge
        .resolve_task(
            &Request {
                config: json!({}),
                project: "demo".to_string(),
                tickets: Vec::new(),
                user: None,
                timeout_seconds: 10,
            },
            &json!({ "state": "open" }),
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("readonly"), "{error}");
}

/// A source bound once for the site: one pull request with a red gate and a
/// conversation, in a repository only the `demo` project's territory claims.
/// Every call that carries the source's own block is logged with the project
/// it was told, so a move can be traced back to the source that reported it.
const SITE_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
request="$(cat)"
log="$(printf '%s' "$request" | jq -r '.config.log // ""')"
if [ -n "$log" ]; then
  printf '%s' "$request" | jq -c --arg call "$1" '{call: $call, project}' >> "$log"
fi
case "${1:?subcommand}" in
  capabilities)
    printf '{"pull_requests":true,"conversation":true,"gate":true,"failures":true,"restart":true,"reactions":true,"tasks":true,"replies":true}'
    ;;
  pull-requests)
    printf '%s' '[
      { "id": "acme/widget/7", "repo": "acme/widget", "number": "7",
        "title": "Widen the retry window",
        "updated_at": "2026-08-01T12:00:00Z",
        "role": "author", "state": "open", "cited": false,
        "threads": [ { "reply": { "thread": "t7" }, "messages": [
          { "author": "Ada", "text": "does the window reset per attempt?",
            "when": "2026-08-01T11:00:00Z", "mine": false,
            "react": { "subject": "c1" } },
          { "author": "Bo", "text": "add a test for the reset",
            "when": "2026-08-01T11:30:00Z", "mine": false,
            "task": { "id": "t1", "state": "open" } } ] } ],
        "gate": { "repos": [ { "repo": "acme/widget", "passed": 4, "failed": 1, "running": 0 } ] } }
    ]'
    ;;
  failures) printf '%s' '[ { "job": "gate / integration" } ]' ;;
  restart) printf '{"asked":1}' ;;
  react|resolve-task|reply) printf '{}' ;;
  *) printf '[]' ;;
esac
"#;

/// Where a matter came from and where it was placed are two facts
/// (§AR-008-pipeline.2): a site source's pull request that attribution placed
/// under `demo` is still that source's to answer. Every move back on it —
/// the failures, a restart, a reaction, a ticked task, a reply — reaches the
/// site source, which is told no project, as it is on a fetch.
#[test]
fn every_move_on_a_site_sources_matter_goes_back_to_that_source() {
    let tmp = tempdir();
    write_fixture(tmp.path());
    let registry = tmp.path().join("workspaces.json");
    let mut doc: Value = serde_json::from_str(&fs::read_to_string(&registry).unwrap()).unwrap();
    doc["projects"][0]["territory"] = json!(["acme/widget"]);
    write_registry(&registry, &doc);
    let log = tmp.path().join("calls.jsonl");
    fs::write(
        tmp.path().join("status.json"),
        serde_json::to_string_pretty(&json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "sources": [{ "provider": "sitegw", "log": log.to_string_lossy() }],
            "projects": { "demo": { "providers": [] } }
        }))
        .unwrap(),
    )
    .unwrap();
    let fake_bin = tmp.path().join("sitebin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("ephor-forge-sitegw"), SITE_FORGE);
    let path = format!(
        "{}:{}",
        fake_bin.to_string_lossy(),
        std::env::var("PATH").unwrap_or_default()
    );
    let run = |args: &[&str]| {
        let mut cmd = ephor_cmd();
        cmd.env("PATH", &path);
        for (key, value) in extension_env(tmp.path()) {
            cmd.env(key, value);
        }
        cmd.args(args).output().unwrap()
    };
    let outcome = |args: &[&str]| -> Value {
        let output = run(args);
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
            panic!(
                "`ephor {}` printed no outcome: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            )
        })
    };

    let refreshed = run(&["refresh"]);
    assert!(
        refreshed.status.success(),
        "{}",
        String::from_utf8_lossy(&refreshed.stderr)
    );
    let cache: Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("state/ephor/feed/demo.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        cache["providers"]["sitegw"]["matters"][0]["key"], "sitegw:acme/widget/7",
        "attribution placed the site source's matter under demo: {cache:#?}"
    );
    let item = "sitegw:acme/widget/7";

    for args in [
        vec!["failures", "--item", item, "--json"],
        vec!["restart", "--item", item, "--scope", "failed", "--json"],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "`ephor {}` did not reach the site source: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for args in [
        vec!["react", item, "THUMBS_UP", "--message", "0", "--json"],
        vec!["tick", item, "--message", "1", "--json"],
    ] {
        let said = outcome(&args);
        assert_eq!(said["ok"], true, "`ephor {}`: {said}", args.join(" "));
    }

    // The rehearsal resolves the same source and asks it whether it can carry
    // a reply, then stops short of sending — and says where it would go.
    let rehearsed = outcome(&["reply", item, "it", "resets", "--dry-run", "--json"]);
    assert_eq!(rehearsed["ok"], true, "{rehearsed}");
    assert!(
        rehearsed["says"]
            .as_str()
            .unwrap_or_default()
            .contains("sitegw"),
        "the dry run names the source the reply would go through: {rehearsed}"
    );
    let replied = outcome(&["reply", item, "it", "resets", "--json"]);
    assert_eq!(replied["ok"], true, "{replied}");

    let calls: Vec<Value> = fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let moves: Vec<&Value> = calls
        .iter()
        .filter(|call| !matches!(call["call"].as_str(), Some("pull-requests" | "issues")))
        .collect();
    assert_eq!(
        moves
            .iter()
            .map(|call| call["call"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["failures", "restart", "react", "resolve-task", "reply"],
        "each move reached the site source once, and the rehearsal sent nothing: {calls:#?}"
    );
    assert!(
        calls.iter().all(|call| call["project"] == ""),
        "a site source is told no project, on a move as on a fetch: {calls:#?}"
    );
}

/// A site source whose host stopped answering after a good refresh: the
/// failure is the network's, the one the request names.
const FLAKY_SITE_FORGE: &str = r#"#!/usr/bin/env bash
set -euo pipefail
request="$(cat)"
down="$(printf '%s' "$request" | jq -r '.config.down // ""')"
case "${1:?subcommand}" in
  capabilities) printf '{"pull_requests":true}' ;;
  pull-requests)
    if [ -n "$down" ] && [ -e "$down" ]; then
      echo 'connection refused by the gateway' >&2
      exit 1
    fi
    printf '%s' '[
      { "id": "acme/widget/7", "repo": "acme/widget", "number": "7",
        "title": "Widen the retry window",
        "updated_at": "2026-08-01T12:00:00Z", "role": "author", "state": "open" }
    ]'
    ;;
  *) printf '[]' ;;
esac
"#;

/// A site source that stops answering keeps its last-good rows in the
/// project it placed them under, and the slot says why, and which kind of
/// failure it was, the way a project's own source does
/// (§AR-008-pipeline.5). Without it the rows went stale with no word on
/// the screen as to what had happened.
#[test]
fn a_lost_site_source_says_why_beside_the_rows_it_left() {
    let tmp = tempdir();
    write_fixture(tmp.path());
    let registry = tmp.path().join("workspaces.json");
    let mut doc: Value = serde_json::from_str(&fs::read_to_string(&registry).unwrap()).unwrap();
    doc["projects"][0]["territory"] = json!(["acme/widget"]);
    write_registry(&registry, &doc);
    let down = tmp.path().join("down");
    fs::write(
        tmp.path().join("status.json"),
        serde_json::to_string_pretty(&json!({
            "defaults": { "ttl_seconds": 600, "provider_timeout_seconds": 10 },
            "sources": [{ "provider": "sitegw", "down": down.to_string_lossy() }],
            "projects": { "demo": { "providers": [] } }
        }))
        .unwrap(),
    )
    .unwrap();
    let fake_bin = tmp.path().join("sitebin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("ephor-forge-sitegw"), FLAKY_SITE_FORGE);
    let path = format!(
        "{}:{}",
        fake_bin.to_string_lossy(),
        std::env::var("PATH").unwrap_or_default()
    );
    let run = |args: &[&str]| {
        let mut cmd = ephor_cmd();
        cmd.env("PATH", &path);
        for (key, value) in extension_env(tmp.path()) {
            cmd.env(key, value);
        }
        cmd.args(args).output().unwrap()
    };
    let slot = || -> Value {
        let cache: Value = serde_json::from_str(
            &fs::read_to_string(tmp.path().join("state/ephor/feed/demo.json")).unwrap(),
        )
        .unwrap();
        cache["providers"]["sitegw"].clone()
    };

    assert!(run(&["refresh"]).status.success());
    assert_eq!(slot()["ok"], true, "{:#?}", slot());

    fs::write(&down, "").unwrap();
    let lost = run(&["refresh"]);
    assert!(
        !lost.status.success(),
        "losing the site source fails the run"
    );
    let slot = slot();
    assert_eq!(slot["ok"], false, "{slot:#?}");
    assert_eq!(slot["stale"], true, "the last-good row is kept: {slot:#?}");
    assert_eq!(slot["unreachable"], true, "{slot:#?}");
    assert!(
        slot["error"]
            .as_str()
            .is_some_and(|error| error.contains("connection refused")),
        "the slot keeps the reason: {slot:#?}"
    );

    let status = run(&["status", "demo", "--cached"]);
    let stderr = String::from_utf8_lossy(&status.stderr);
    assert!(
        stderr.contains("[sitegw]") && stderr.contains("unreachable: "),
        "the project's view says which source went quiet, and how: {stderr}"
    );
}

/// A shared source is fetched once per site, not per project
/// (§AR-008-pipeline.1). So a refresh of one project keeps the rows a site
/// source placed there, and only a source taken out of `sources` loses them.
#[test]
fn a_site_sources_rows_stay_until_it_leaves_sources() {
    let tmp = tempdir();
    write_fixture(tmp.path());
    let registry = tmp.path().join("workspaces.json");
    let mut doc: Value = serde_json::from_str(&fs::read_to_string(&registry).unwrap()).unwrap();
    doc["projects"][0]["territory"] = json!(["acme/widget"]);
    write_registry(&registry, &doc);
    let configure = |sources: Value| {
        fs::write(
            tmp.path().join("status.json"),
            serde_json::to_string_pretty(&json!({
                // Every cached read is past its time, so `status` asks the
                // project's own sources again.
                "defaults": { "ttl_seconds": 0, "provider_timeout_seconds": 10 },
                "sources": sources,
                "projects": { "demo": { "providers": [] } }
            }))
            .unwrap(),
        )
        .unwrap();
    };
    let fake_bin = tmp.path().join("sitebin");
    fs::create_dir_all(&fake_bin).unwrap();
    make_executable(&fake_bin.join("ephor-forge-sitegw"), FLAKY_SITE_FORGE);
    let path = format!(
        "{}:{}",
        fake_bin.to_string_lossy(),
        std::env::var("PATH").unwrap_or_default()
    );
    let run = |args: &[&str]| {
        let mut cmd = ephor_cmd();
        cmd.env("PATH", &path);
        for (key, value) in extension_env(tmp.path()) {
            cmd.env(key, value);
        }
        cmd.args(args).output().unwrap()
    };
    let slot = || -> Value {
        let cache: Value = serde_json::from_str(
            &fs::read_to_string(tmp.path().join("state/ephor/feed/demo.json")).unwrap(),
        )
        .unwrap();
        cache["providers"]["sitegw"].clone()
    };
    let keys = |slot: &Value| -> Vec<String> {
        slot["matters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|matter| matter["key"].as_str().map(String::from))
            .collect()
    };

    configure(json!([{ "provider": "sitegw" }]));
    assert!(run(&["refresh"]).status.success());
    assert_eq!(keys(&slot()), ["sitegw:acme/widget/7"], "{:#?}", slot());

    // The project's own sources are asked again; the site source is not, and
    // what it placed here is still here.
    let status = run(&["status", "demo"]);
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert_eq!(
        keys(&slot()),
        ["sitegw:acme/widget/7"],
        "a refresh of one project keeps a site source's rows: {:#?}",
        slot()
    );

    configure(json!([]));
    assert!(run(&["refresh"]).status.success());
    assert_eq!(
        slot(),
        Value::Null,
        "a source no longer in `sources` leaves nothing behind"
    );
}
