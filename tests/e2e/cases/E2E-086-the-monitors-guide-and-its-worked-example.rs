//! E2E-086-the-monitors-guide-and-its-worked-example: `ephor guide monitors`
//! explains the monitor protocol in ephor's own words, the same on every
//! machine, and under `--json` carries the text and no schemas
//! (§FS-020-guide.6, §FS-020-guide.4). Its worked example works: the script
//! and the manifest, copied out of the text into a checkout, are a manifest
//! `ephor validate --manifest` accepts and a script whose answer ephor reads,
//! three monitors of three kinds from one command (§FS-021-monitors.4.1,
//! §FS-021-monitors.5.1); and where the script has no value for one, that
//! monitor alone is unreadable, and `ephor monitors show` says why at once
//! (§FS-021-monitors.8.2, §FS-021-monitors.19). The nightly job's result
//! carries the time it ran, so a night it does not run is a stale read, not
//! a reading (§FS-021-monitors.7.1).
//!
//! The site binds a runtime, so the case can see that the text names none.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

fn guide(world: &World, args: &[&str]) -> (String, Option<Value>) {
    let out = world
        .ephor()
        .args(["guide", "monitors"])
        .args(args)
        .output()
        .expect("ephor guide monitors");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = args.contains(&"--json").then(|| shaped("guide", &out));
    (String::from_utf8_lossy(&out.stdout).into_owned(), json)
}

/// The file the example names on the line `header` begins: the block after
/// it indented by two spaces, as the reader copies it out of the text.
fn file_after(text: &str, header: &str) -> String {
    let start = text
        .lines()
        .position(|line| line.starts_with(header))
        .unwrap_or_else(|| panic!("no line begins {header}:\n{text}"));
    let mut lines: Vec<&str> = text
        .lines()
        .skip(start)
        .skip_while(|line| !line.ends_with(':'))
        .skip(1)
        .take_while(|line| line.is_empty() || line.starts_with("  "))
        .map(|line| line.strip_prefix("  ").unwrap_or(line))
        .collect();
    while lines.last() == Some(&"") {
        lines.pop();
    }
    lines.join("\n") + "\n"
}

/// The checkout the text describes before the answer it shows.
fn as_described(world: &World) {
    world.file("VERSION", "1.4.0\n");
    world.file("CHANGELOG.md", "# Changelog\n\n## 1.4.0\n\n- Monitors.\n");
    world.file("src/lib.rs", "// TODO: one\nfn a() {}\n// TODO: two\n");
    world.file("src/deep/mod.rs", "fn b() {}\n// TODO: three\n");
    world.file(
        "target/nightly.status",
        "passed 2026-11-26T05:40:00+01:00\n",
    );
}

/// The nightly job, run a few minutes ago: its result and when it ran, so
/// the next read of nightly finds a measurement newer than an interval.
fn nightly_ran(world: &World, result: &str) -> String {
    let ran = (chrono::Utc::now() - chrono::TimeDelta::minutes(5))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    world.file("target/nightly.status", &format!("{result} {ran}\n"));
    ran
}

/// Runs the example's script once as ephor summons it, in the checkout with
/// the summons' two variables, retrying while a thread that forked as the
/// file was written still holds it busy (the `settle` of the harness, with a
/// checkout and an answer file the script needs).
fn probe(world: &World, script: &std::path::Path) -> Value {
    let answer = world.path().join("probe-answer.json");
    for _ in 0..20 {
        let run = std::process::Command::new(script)
            .current_dir(world.forest())
            .env("HOME", world.path())
            .env("EPHOR_MONITORS", "releasable,todo-markers,nightly")
            .env("EPHOR_ANSWER", &answer)
            .output();
        match run {
            Err(err) if err.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Ok(out) => {
                assert!(out.status.success(), "{out:?}");
                return read_json(&answer);
            }
            Err(err) => panic!("the example's script does not run: {err}"),
        }
    }
    panic!("the example's script stayed busy")
}

/// `ephor monitors show` of one monitor, as text.
fn show_text(world: &World, id: &str) -> String {
    let out = world
        .ephor()
        .args(["monitors", "show", &format!("{PROJECT}/{id}")])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn shown(world: &World, id: &str) -> Value {
    let out = world
        .ephor()
        .args(["monitors", "show", &format!("{PROJECT}/{id}"), "--json"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    shaped("monitor", &out)
}

/// The topic is ephor's: listed among the topics, spelled the same whatever
/// runtime the site binds, and carrying no schemas, since `ephor schema
/// manifest` and `ephor schema answer` print them (§FS-020-guide.6).
#[test]
fn the_monitors_topic_names_no_runtime_and_carries_no_schemas() {
    let world = World::new();
    let (unbound, _) = guide(&world, &[]);
    world.configure(json!({"work": {"runner": "acme-runtime"}}));
    let (text, view) = guide(&world, &["--json"]);
    let view = view.unwrap();
    assert_eq!(view["topic"], "monitors");
    assert_eq!(view["text"].as_str().unwrap(), unbound);
    assert!(!unbound.contains("acme-runtime"), "{unbound}");
    assert!(view.get("schemas").is_none(), "{view}");
    assert!(view.get("schemas_unavailable").is_none(), "{view}");
    assert!(
        text.starts_with('{'),
        "--json prints the view alone: {text}"
    );
    for line in unbound.lines() {
        assert!(line.chars().count() <= 80, "wider than 80 columns: {line}");
    }
    let flat = unbound.split_whitespace().collect::<Vec<_>>().join(" ");
    for part in [
        "EPHOR_MONITORS",
        "EPHOR_ANSWER",
        "Never run `ephor refresh` in it",
        "ephor validate --manifest ephor.json",
        "ephor monitors show <project>/<id>",
        "Worked example.",
        "Validate holds each declaration to its shape alone",
    ] {
        assert!(flat.contains(part), "{part} missing:\n{unbound}");
    }
    let (listed, _) = guide(&World::new(), &[]);
    assert_eq!(listed, unbound);
}

/// The example, copied out of the text into a checkout, is a manifest ephor
/// accepts and a script whose answer ephor reads: one run of the one command
/// gives the boolean, the number and the level their readings, each healthy
/// by its own condition (§FS-021-monitors.5.1, §FS-021-monitors.3.6).
#[test]
fn the_worked_example_is_read_as_the_text_says() {
    let world = World::new();
    let (text, _) = guide(&world, &[]);
    let script = world.script("scripts/kpis.sh", &file_after(&text, "Worked example."));
    world.file("ephor.json", &file_after(&text, "ephor.json, at the root"));
    as_described(&world);
    let shows = file_after(&text, "With 1.4.0 in VERSION");
    assert_eq!(
        probe(&world, &script),
        serde_json::from_str::<Value>(&shows).unwrap(),
        "the script writes what the text shows"
    );

    let manifest = world.forest().join("ephor.json");
    let out = world
        .ephor()
        .args(["validate", "--manifest", manifest.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");

    // Never read, so each is due at once: a plain refresh reads all three,
    // the nightly result the job left minutes ago among them.
    let ran = nightly_ran(&world, "passed");
    world.ephor().args(["refresh", PROJECT]).assert().success();
    for (id, kind, value, cadence) in [
        ("releasable", "boolean", json!(true), "default"),
        ("todo-markers", "number", json!(3.0), "manifest"),
        ("nightly", "level", json!("passed"), "manifest"),
    ] {
        let monitor = shown(&world, id);
        assert_eq!(monitor["kind"], kind, "{monitor}");
        assert_eq!(monitor["health"], "ok", "{monitor}");
        assert_eq!(monitor["value"], value, "{monitor}");
        assert_eq!(monitor["freshness"]["last_found"], "reading", "{monitor}");
        assert_eq!(monitor["cadence"]["from"], cadence, "{monitor}");
        assert_eq!(monitor["from"]["home"], "manifest", "{monitor}");
        assert_eq!(
            monitor["from"]["source"]["command"], "scripts/kpis.sh",
            "{monitor}"
        );
    }

    // A night the job does not run: read again, nightly brings what it
    // brought before, which is stale and no reading; the others are read.
    world
        .ephor()
        .args(["refresh", PROJECT, "--monitors-now"])
        .assert()
        .success();
    let nightly = shown(&world, "nightly");
    assert_eq!(nightly["freshness"]["last_found"], "stale", "{nightly}");
    let measured = nightly["freshness"]["measured"].as_str().unwrap();
    assert_eq!(
        chrono::DateTime::parse_from_rfc3339(measured).unwrap(),
        chrono::DateTime::parse_from_rfc3339(&ran).unwrap(),
        "{nightly}"
    );
    assert_eq!(nightly["health"], "ok", "the last health holds: {nightly}");
    assert!(show_text(&world, "nightly").contains("(stale: measured "));
    for id in ["releasable", "todo-markers"] {
        let monitor = shown(&world, id);
        assert_eq!(monitor["freshness"]["last_found"], "reading", "{monitor}");
    }
}

/// Where the nightly job left no status, the script leaves the value out
/// rather than writing something JSON is not, and that read is of nightly
/// alone: the others are read from the same answer (§FS-021-monitors.8.2).
#[test]
fn a_value_the_script_cannot_give_costs_that_monitor_alone() {
    let world = World::new();
    let (text, _) = guide(&world, &[]);
    let script = world.script("scripts/kpis.sh", &file_after(&text, "Worked example."));
    world.file("ephor.json", &file_after(&text, "ephor.json, at the root"));
    as_described(&world);
    std::fs::remove_file(world.forest().join("target/nightly.status")).unwrap();
    let answer = probe(&world, &script);
    assert!(answer["monitors"][2].get("value").is_none(), "{answer}");

    world
        .ephor()
        .args(["refresh", PROJECT, "--monitors-now"])
        .assert()
        .success();
    let nightly = shown(&world, "nightly");
    assert_eq!(
        nightly["freshness"]["last_found"], "unreadable",
        "{nightly}"
    );
    assert!(nightly.get("value").is_none(), "{nightly}");
    // Not read yet, and saying why from that first read.
    assert_eq!(nightly["freshness"]["state"], "not-read-yet", "{nightly}");
    assert_eq!(
        nightly["freshness"]["why"], "the sample has no value",
        "{nightly}"
    );
    let said = show_text(&world, "nightly");
    assert!(said.contains("(failed: the sample has no value)"), "{said}");
    for id in ["releasable", "todo-markers"] {
        let monitor = shown(&world, id);
        assert_eq!(monitor["freshness"]["last_found"], "reading", "{monitor}");
        assert_eq!(monitor["health"], "ok", "{monitor}");
    }
}
