//! A source's explicit executable binding cannot become a different forge.
//! §FS-001-forge-interface.2 requires absence-only convention, refusal of
//! non-string commands before invocation, and literal executable strings.
//! Ported from issue #186's gateway/wrapper/stale-forge reproducer; argv logs
//! distinguish refusal from silently running an unconfigured executable.
//! Construction has no CLI surface of its own: the final tests call the
//! shared factories directly to pin refusal before a capability probe.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};
use std::fs;
use std::process::Output;
use support::*;

const SOURCE: &str = "mail-me";
const GATEWAY: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$EPHOR_COMMAND_LOG/gateway.argv"
/bin/cat >/dev/null
[ "$1" = forge ] || exit 64
case "$2" in
  capabilities) printf '{"messages":true,"failures":true}' ;;
  *) printf '[]' ;;
esac
"#;
const CONVENTIONAL: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$EPHOR_COMMAND_LOG/conventional.argv"
/bin/cat >/dev/null
case "$1" in
  capabilities) printf '{"messages":true,"failures":true}' ;;
  *) printf '[]' ;;
esac
"#;
const WRAPPER: &str = r#"#!/bin/sh
exec "$EPHOR_COMMAND_LOG/fakebin/gateway" forge "$@"
"#;
const SEED_GATEWAY: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$EPHOR_COMMAND_LOG/gateway.argv"
/bin/cat >/dev/null
[ "$1" = forge ] || exit 64
case "$2" in
  capabilities) printf '{"pull_requests":true,"failures":true}' ;;
  pull-requests)
    printf '%s' '[{"id":"app/1","repo":"app","number":"1","title":"A change","updated_at":"2026-10-07T10:00:00Z","role":"author","state":"open","cited":false,"threads":[]}]' ;;
  *) printf '[]' ;;
esac
"#;

#[derive(Clone, Copy, Debug)]
enum Binding {
    Project,
    Site,
}

fn fixture(conventional: bool) -> World {
    let world = World::new();
    world.stub("gateway", GATEWAY);
    world.stub("gateway-wrapper", WRAPPER);
    if conventional {
        world.stub("ephor-forge-mail-me", CONVENTIONAL);
    }
    world
}

fn configure(world: &World, binding: Binding, command: Option<Value>) {
    let mut source = json!({ "provider": SOURCE });
    if let Some(command) = command {
        source["command"] = command;
    }
    world.configure(match binding {
        Binding::Project => json!({ "projects": { PROJECT: { "providers": [source] } } }),
        Binding::Site => json!({ "sources": [source] }),
    });
}

fn run(world: &World, args: &[&str]) -> Output {
    world
        .ephor()
        // Only our stubs can resolve; the fixture uses /bin/sh and /bin/cat.
        .env("PATH", world.path().join("fakebin"))
        .env("EPHOR_COMMAND_LOG", world.path())
        .env("XDG_CONFIG_HOME", world.path().join("config"))
        .env("XDG_CACHE_HOME", world.path().join("cache"))
        .env("NO_COLOR", "1")
        .args(args)
        .output()
        .expect("run ephor")
}

fn log(world: &World, name: &str) -> String {
    fs::read_to_string(world.path().join(name)).unwrap_or_default()
}

fn evidence(world: &World, output: &Output) -> String {
    format!(
        "exit: {:?}\nstdout: {}\nstderr: {}\ngateway argv: {:?}\nconventional argv: {:?}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        log(world, "gateway.argv"),
        log(world, "conventional.argv"),
    )
}

fn assert_refused(world: &World, output: &Output, explanation: bool) {
    let diagnostic = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = diagnostic.to_lowercase();
    let observed = evidence(world, output);
    println!("{observed}");
    // Check both logs even when the exit code or diagnostic is wrong, so the
    // failure shows a successful invocation of the wrong forge too.
    assert!(
        !output.status.success()
            && diagnostic.contains(SOURCE)
            && lower.contains("command")
            && log(world, "gateway.argv").is_empty()
            && log(world, "conventional.argv").is_empty()
            && (!explanation
                || (lower.contains("executable")
                    && lower.contains("argument")
                    && lower.contains("wrapper"))),
        "expected a source/command refusal before invoking either forge{}; {observed}",
        if explanation {
            " explaining literal executable semantics and the wrapper remedy"
        } else {
            ""
        },
    );
}

fn array_refusal(binding: Binding, conventional: bool, args: &[&str]) {
    let world = fixture(conventional);
    if args[0] == "failures" {
        // This action reads the feed before resolving its explicit source.
        // Seed a real matter through the wrapper and demonstrate that the
        // action works before changing the binding. Attribution puts the
        // site's pull request in this project's feed too.
        world.register(json!({ "territory": ["app"] }));
        world.stub("gateway", SEED_GATEWAY);
        configure(
            &world,
            binding,
            Some(json!(world.path().join("fakebin/gateway-wrapper"))),
        );
        let seeded = run(&world, &["refresh"]);
        assert!(seeded.status.success(), "{}", evidence(&world, &seeded));
        assert_eq!(world.matter("mail-me:app/1")["title"], "A change");
        let control = run(&world, args);
        assert!(control.status.success(), "{}", evidence(&world, &control));
        assert!(log(&world, "gateway.argv").contains("forge failures\n"));
        // Clear the preparation log; only the action after the unsupported
        // binding is installed may contribute to the refusal assertion.
        fs::remove_file(world.path().join("gateway.argv")).unwrap();
    }
    configure(
        &world,
        binding,
        Some(json!([world.path().join("fakebin/gateway"), "forge"])),
    );
    assert_refused(&world, &run(&world, args), false);
}

#[test]
fn project_array_without_conventional_executable_is_refused_by_field() {
    array_refusal(Binding::Project, false, &["refresh", PROJECT]);
}

#[test]
fn project_array_with_stale_conventional_executable_never_invokes_it() {
    array_refusal(Binding::Project, true, &["refresh", PROJECT]);
}

#[test]
fn site_array_without_conventional_executable_is_refused_by_field() {
    array_refusal(Binding::Site, false, &["refresh"]);
}

#[test]
fn site_array_with_stale_conventional_executable_never_invokes_it() {
    array_refusal(Binding::Site, true, &["refresh"]);
}

#[test]
fn doctor_refuses_project_array_before_capability_probe() {
    array_refusal(Binding::Project, true, &["doctor", "--skip-self"]);
}

#[test]
fn doctor_refuses_site_array_before_capability_probe() {
    array_refusal(Binding::Site, true, &["doctor", "--skip-self"]);
}

// A site's command refusal is its own failure (§FS-001-forge-interface.2,
// §FS-001-forge-interface.6), even when every project source answers and the
// shared source has never placed a row in any project cache.
fn doctor_with_healthy_project_refuses_new_site_binding(as_json: bool) {
    let world = fixture(true);
    let healthy = world.stub(
        "healthy-forge",
        r#"#!/bin/sh
printf '%s\n' "$*" >> "$EPHOR_COMMAND_LOG/healthy.argv"
/bin/cat >/dev/null
case "$1" in
  capabilities) printf '{"messages":true}' ;;
  *) printf '[]' ;;
esac
"#,
    );
    world.configure(json!({
        "projects": { PROJECT: { "providers": [{
            "provider": "healthy", "command": healthy
        }] } },
        "sources": [{
            "provider": SOURCE,
            "command": [world.path().join("fakebin/gateway"), "forge"]
        }],
    }));
    let args = if as_json {
        vec!["doctor", "--skip-self", "--json"]
    } else {
        vec!["doctor", "--skip-self"]
    };
    let output = run(&world, &args);
    assert_refused(&world, &output, false);
    let observed = evidence(&world, &output);
    assert_eq!(output.status.code(), Some(4), "{observed}");
    assert_eq!(
        log(&world, "healthy.argv"),
        "capabilities\nmessages\n",
        "{observed}"
    );
    if as_json {
        let report: Value = serde_json::from_slice(&output.stdout).expect("doctor's JSON report");
        assert_eq!(report["projects"][0]["project"], PROJECT, "{observed}");
        assert_eq!(report["projects"][0]["health"], "well", "{observed}");
        assert_eq!(
            report["projects"][0]["sources"],
            json!({
                "configured": 1, "asked": 1, "answering": 1
            }),
            "{observed}"
        );
        assert_eq!(report["projects"][0]["silent"], json!([]), "{observed}");
        assert_eq!(report["shared_sources"]["health"], "degraded", "{observed}");
        let silent = report["shared_sources"]["silent"]
            .as_array()
            .expect("shared refusals");
        assert_eq!(silent.len(), 1, "{observed}");
        assert_eq!(silent[0]["source"], SOURCE, "{observed}");
        assert!(
            silent[0]["why"].as_str().unwrap().contains("command"),
            "{observed}"
        );
        assert_eq!(silent[0]["unreachable"], false, "{observed}");
    } else {
        let report = String::from_utf8_lossy(&output.stdout);
        assert!(report.contains("1/1 source(s) answering"), "{observed}");
        assert!(
            report.contains(SOURCE) && report.contains("command"),
            "{observed}"
        );
        assert!(report.contains("degraded —"), "{observed}");
        assert!(
            !report.contains("well — everything asked answered"),
            "{observed}"
        );
    }
}

#[test]
fn doctor_text_retains_new_site_command_refusal_with_healthy_project() {
    doctor_with_healthy_project_refuses_new_site_binding(false);
}

#[test]
fn doctor_json_retains_new_site_command_refusal_with_healthy_project() {
    doctor_with_healthy_project_refuses_new_site_binding(true);
}

fn failures_args() -> [&'static str; 9] {
    [
        "failures",
        "--project",
        PROJECT,
        "--source",
        SOURCE,
        "--repo",
        "app",
        "--number",
        "1",
    ]
}

#[test]
fn source_directed_action_refuses_project_array_before_invocation() {
    array_refusal(Binding::Project, true, &failures_args());
}

#[test]
fn source_directed_action_refuses_site_array_before_invocation() {
    array_refusal(Binding::Site, true, &failures_args());
}

// Compatibility controls already pass: they hold the refusal fix to the
// existing transport instead of substituting for a failing regression.
#[test]
fn wrapper_for_fixed_arguments_still_reaches_gateway() {
    for binding in [Binding::Project, Binding::Site] {
        let world = fixture(true);
        configure(
            &world,
            binding,
            Some(json!(world.path().join("fakebin/gateway-wrapper"))),
        );
        let output = run(&world, &["refresh"]);
        let observed = evidence(&world, &output);
        println!("{binding:?}: {observed}");
        assert!(output.status.success(), "{observed}");
        assert_eq!(
            log(&world, "gateway.argv"),
            "forge capabilities\nforge messages\n",
            "{observed}"
        );
        assert_eq!(log(&world, "conventional.argv"), "", "{observed}");
    }
}

#[test]
fn absent_command_uses_conventional_executable() {
    for binding in [Binding::Project, Binding::Site] {
        let world = fixture(true);
        configure(&world, binding, None);
        let output = run(&world, &["refresh"]);
        let observed = evidence(&world, &output);
        println!("{binding:?}: {observed}");
        assert!(output.status.success(), "{observed}");
        assert_eq!(
            log(&world, "conventional.argv"),
            "capabilities\nmessages\n",
            "{observed}"
        );
        assert_eq!(log(&world, "gateway.argv"), "", "{observed}");
    }
}

#[test]
fn literal_executable_path_containing_spaces_still_works() {
    for binding in [Binding::Project, Binding::Site] {
        let world = fixture(true);
        let path = world.stub("gateway forge", WRAPPER);
        configure(&world, binding, Some(json!(path)));
        let output = run(&world, &["refresh"]);
        let observed = evidence(&world, &output);
        println!("{binding:?}: {observed}");
        assert!(output.status.success(), "{observed}");
        assert_eq!(
            log(&world, "gateway.argv"),
            "forge capabilities\nforge messages\n",
            "{observed}"
        );
        assert_eq!(log(&world, "conventional.argv"), "", "{observed}");
    }
}

#[test]
fn nonexistent_argument_bearing_literal_explains_command_and_wrapper() {
    let world = fixture(true);
    let nonexistent = format!("{} forge", world.path().join("fakebin/gateway").display());
    assert!(!std::path::Path::new(&nonexistent).exists());
    configure(&world, Binding::Project, Some(json!(nonexistent)));
    assert_refused(&world, &run(&world, &["refresh", PROJECT]), true);
}

#[test]
fn all_non_string_json_types_fail_in_shared_source_construction() {
    use ephor::feed::config::Defaults;
    use ephor::feed::providers::{build_provider, forge_call, Sources};

    let mut failures = Vec::new();
    for value in [json!([]), json!({}), json!(42), json!(true), Value::Null] {
        let block = json!({ "provider": SOURCE, "command": value });
        let result = build_provider(&block).err().map(|error| error.0);
        println!("build_provider command={value}: {result:?}");
        if !result.is_some_and(|message| message.contains(SOURCE) && message.contains("command")) {
            failures.push(format!(
                "build_provider accepted or misidentified command={value}"
            ));
        }
        // The project override must also win over a valid site binding when
        // the action factory resolves the source (§FS-001-forge-interface.9).
        for binding in [Binding::Project, Binding::Site] {
            let sources = match binding {
                Binding::Project => Sources {
                    project: PROJECT.into(),
                    own: vec![block.clone()],
                    site: vec![json!({ "provider": SOURCE, "command": "valid-site-wrapper" })],
                },
                Binding::Site => Sources {
                    project: PROJECT.into(),
                    own: vec![],
                    site: vec![block.clone()],
                },
            };
            let result = forge_call(&sources, SOURCE, &Defaults::default())
                .err()
                .map(|error| error.0);
            println!("forge_call {binding:?} command={value}: {result:?}");
            if !result
                .is_some_and(|message| message.contains(SOURCE) && message.contains("command"))
            {
                failures.push(format!(
                    "forge_call {binding:?} accepted or misidentified command={value}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
