use std::path::Path;
use std::process::Command;

use serde_json::{json, Value as Json};

use crate::monitors::condition::band;
use crate::monitors::model::{Band, Home, Value};
use crate::monitors::overlay::{resolve, Homes};
use crate::monitors::shape::{read_declarations, read_sample};
use crate::work::recipe::WorkConfig;

const MANIFEST_SCHEMA: &str = include_str!("../../../assets/ephor-manifest.schema.json");
const ANSWER_SCHEMA: &str = include_str!("../../../assets/ephor-answer.schema.json");

/// How many lines one screen gives the explanation before the example
/// (§FS-020-guide.1).
const SCREEN: usize = 48;

/// The topic as `ephor guide monitors` prints it.
fn topic() -> String {
    crate::api::guide::guide(&WorkConfig::default(), Some("monitors"))
        .unwrap()
        .text
}

/// The worked example, as the text gives it to the reader who copies it.
struct Example {
    script: String,
    manifest: String,
    answer: String,
}

/// Takes the example out of the text itself, so the text and what the tests
/// hold to the protocol cannot drift apart: each file is the block indented
/// by two spaces after the line that names it, up to the commands that check
/// it.
fn example(text: &str) -> Example {
    let (_, after) = text
        .split_once("Worked example.")
        .expect("the topic ends with its worked example");
    let (after, _) = after
        .split_once("Check it")
        .expect("the example ends with the commands that check it");
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut intro = String::new();
    let mut body: Option<Vec<&str>> = None;
    let mut flush = |intro: &mut String, body: &mut Option<Vec<&str>>| {
        if let Some(mut lines) = body.take() {
            while lines.last() == Some(&"") {
                lines.pop();
            }
            blocks.push((std::mem::take(intro), lines.join("\n") + "\n"));
        }
    };
    for line in after.lines() {
        if let Some(code) = line.strip_prefix("  ") {
            body.get_or_insert_with(Vec::new).push(code);
        } else if line.is_empty() && body.is_some() {
            body.as_mut().unwrap().push("");
        } else {
            flush(&mut intro, &mut body);
            intro.push_str(line);
            intro.push(' ');
        }
    }
    flush(&mut intro, &mut body);
    let [(script_said, script), (manifest_said, manifest), (answer_said, answer)] =
        <[(String, String); 3]>::try_from(blocks).expect("three files in the example");
    assert!(script_said.contains("scripts/kpis.sh"), "{script_said}");
    assert!(manifest_said.contains("ephor.json"), "{manifest_said}");
    assert!(answer_said.contains("script writes:"), "{answer_said}");
    Example {
        script,
        manifest,
        answer,
    }
}

/// A validator for one of a published schema's `$defs`, the strict shape each
/// entry of its `monitors` is checked against on its own
/// (§FS-021-monitors.8.4).
fn entry_validator(schema: &str, definition: &str) -> jsonschema::Validator {
    let schema: Json = serde_json::from_str(schema).unwrap();
    let rooted = json!({
        "$schema": schema["$schema"],
        "$defs": schema["$defs"],
        "$ref": format!("#/$defs/{definition}"),
    });
    jsonschema::validator_for(&rooted).unwrap()
}

/// Every way `value` breaks the whole published `schema`.
fn whole(schema: &str, value: &Json) -> Vec<String> {
    let schema: Json = serde_json::from_str(schema).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    validator
        .iter_errors(value)
        .map(|error| format!("{}: {error}", error.instance_path))
        .collect()
}

/// A checkout holding the example's script and manifest, and its answer file
/// beside it rather than in it, as a summons is given one.
fn checkout(example: &Example) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("checkout");
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::write(root.join("scripts/kpis.sh"), &example.script).unwrap();
    std::fs::write(root.join("ephor.json"), &example.manifest).unwrap();
    dir
}

/// The checkout the text describes before the answer it shows: 1.4.0 in
/// VERSION, its heading in CHANGELOG.md, three TODO lines under src/, and
/// the nightly job's result and when it ran in target/nightly.status.
fn as_described(root: &Path) {
    let file = |relative: &str, body: &str| {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    };
    file("VERSION", "1.4.0\n");
    file("CHANGELOG.md", "# Changelog\n\n## 1.4.0\n\n- Monitors.\n");
    file("src/lib.rs", "// TODO: one\nfn a() {}\n// TODO: two\n");
    file("src/deep/mod.rs", "fn b() {}\n// TODO: three\n");
    file(
        "target/nightly.status",
        "passed 2026-11-26T05:40:00+01:00\n",
    );
}

/// Runs the script as a summons would: in the checkout, with the ids it reads
/// in EPHOR_MONITORS and the file to answer in EPHOR_ANSWER
/// (§FS-021-monitors.5.4). It is run by `sh` rather than executed, so a test
/// thread forking meanwhile cannot hold the file just written busy; ephor's
/// own run of it is E2E-086's.
fn answer_of(dir: &Path, reads: &str) -> Json {
    let answer = dir.join("answer.json");
    let out = Command::new("sh")
        .arg("scripts/kpis.sh")
        .current_dir(dir.join("checkout"))
        .env("HOME", dir)
        .env("EPHOR_MONITORS", reads)
        .env("EPHOR_ANSWER", &answer)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let text = std::fs::read_to_string(&answer).unwrap();
    let parsed: Json = serde_json::from_str(&text).unwrap_or_else(|err| {
        panic!("the script's answer is not JSON: {err}\n{text}");
    });
    // As ephor reads it: parsed and held to the envelope schema whole.
    crate::seams::answer::parse(&text, "kpis.sh", &dir.join("checkout")).unwrap();
    parsed
}

/// The answer holds to the published answer schema whole, and each sample to
/// the strict shape one sample is checked against (§FS-021-monitors.8.2).
fn holds_as_an_answer(answer: &Json) {
    let problems = whole(ANSWER_SCHEMA, answer);
    assert!(problems.is_empty(), "{problems:?}\n{answer}");
    let sample = entry_validator(ANSWER_SCHEMA, "monitorSample");
    for entry in answer["monitors"].as_array().unwrap() {
        assert!(sample.is_valid(entry), "{entry}");
    }
}

/// The explanation fits one screen of 80 columns and 48 lines, and the
/// worked example comes after it; the example's lines fit 80 columns too
/// (§FS-020-guide.1).
#[test]
fn the_explanation_fits_one_screen_before_the_example() {
    let text = topic();
    let (explanation, _) = text
        .split_once("Worked example.")
        .expect("the topic ends with its example");
    let lines = explanation.trim_end().lines().count();
    assert!(
        lines <= SCREEN,
        "the explanation is {lines} lines, more than one screen of {SCREEN}:\n{explanation}"
    );
    for line in text.lines() {
        assert!(line.chars().count() <= 80, "wider than 80 columns: {line}");
    }
}

/// The topic covers each part §FS-020-guide.6 lists, in the words a reader
/// will meet in ephor's own output, and what a reader trying it out stumbled
/// on: when a read is stale, where `timeout` goes, that a sample brings a
/// monitor of its own, and what validation does not check. Each part is
/// looked for with the text's line breaks read as spaces.
#[test]
fn the_topic_covers_what_the_guide_promises() {
    let text = topic().split_whitespace().collect::<Vec<_>>().join(" ");
    for part in [
        // The four kinds and their values (§FS-021-monitors.2).
        "boolean true or false",
        "number a number, `unit`",
        "level one of `levels`, best first",
        "series `points`, all {t, v} with t a time as `at` is, or all {x, v}",
        "a series' value is its last v",
        "a condition is on the value, never the fraction",
        // Conditions, warn, and who decides health (§FS-021-monitors.3).
        "{\"is\": true} for a boolean",
        "both, inclusive, for a number or series",
        "{\"in\": [\"clean\"]} or {\"at_most\": \"degraded\"} for a level",
        "`warn`, stricter and needing a `healthy` (a kind's default counts), must also hold",
        "the first of the site's `healthy` (the person's own), the manifest's",
        "the sample's `ok` (false is down), the kind's",
        "`\"healthy\": null` makes any monitor a KPI",
        // Where a monitor is declared, and one a sample brings (§FS-021-monitors.4).
        "ephor.json `monitors`; only `id` is required",
        "A sample for an id nothing declares is a monitor too",
        // The two bindings, exit codes and environment (§FS-021-monitors.5).
        "with none, the answer that brings its id",
        "{\"command\": \"scripts/kpis.sh\"} runs in a shell",
        "{\"file\": \"target/kpi/coverage.json\"}",
        "those about an item empty; EPHOR_MONITORS, the ids this run reads",
        "EPHOR_ANSWER, the file to write",
        "Exit 0 is a read; 75 is parked",
        "30 s gone (`timeout` beside `from`, whole seconds, sets another)",
        "never climbing out by `..`",
        "Never run `ephor refresh` in it",
        // The answer and a sample (§FS-021-monitors.8).
        "{\"v\": 1, \"monitors\": [...]}",
        "`title`, `kind`, `unit`, `better`, `levels`, `healthy`, `warn`, `description`",
        "makes that monitor alone unreadable",
        "NaN and Infinity are not JSON",
        "leave `value` out",
        // When a read is stale (§FS-021-monitors.7.1).
        "A read is stale, not a reading",
        "its series' last point is the one held, or its file is unchanged",
        "what a job leaves needs the job's `at` and cadence",
        // Cadence (§FS-021-monitors.6).
        "`every` is a whole number and m, h, d or w",
        "`at` pins the slots to the local clock",
        "Cadence is a floor, not a promise",
        "the site's to arrange",
        // The commands that check it, and what validation leaves to them.
        "ephor validate --manifest ephor.json each declaration's shape",
        "ephor refresh <project> reads what is due; --monitors-now, all",
        "ephor monitors show <project>/<id> its value and health, why a read failed",
        "ephor schema manifest",
        "ephor schema answer",
        "the name `ephor list` gives the project",
        "Validate holds each declaration to its shape alone",
        "a monitor that breaks one is not read",
        "noted the same way and set aside, never refused",
    ] {
        assert!(text.contains(part), "{part} missing:\n{}", topic());
    }
}

/// The commands that check the result close the worked example, as the
/// `gate` topic's do, and the topic ends with them (§FS-020-guide.6).
#[test]
fn the_example_ends_with_the_commands_that_check_it() {
    let text = topic();
    let (_, example) = text.split_once("Worked example.").unwrap();
    let (files, check) = example.split_once("Check it").unwrap();
    assert!(files.contains("scripts/kpis.sh") && files.contains("ephor.json"));
    assert!(check.trim_end().ends_with("never refused."), "{check}");
}

/// The protocol is ephor's, so the text reads nothing of the site's, names no
/// runtime, and carries no schemas beside it (§FS-020-guide.6).
#[test]
fn the_text_is_the_same_on_every_machine_and_names_no_runtime() {
    let bound = WorkConfig {
        runner: Some("acme-runtime".to_string()),
        ..WorkConfig::default()
    };
    let view = crate::api::guide::guide(&bound, Some("monitors")).unwrap();
    assert_eq!(view.text, topic());
    assert_eq!(view.topic.as_deref(), Some("monitors"));
    assert!(!view.text.contains("acme-runtime"), "{}", view.text);
    let unbound = WorkConfig::default();
    let shipped = crate::work::runtime::runner(&unbound);
    assert!(!view.text.contains(shipped), "{}", view.text);
    assert_eq!(view.schemas, None);
    assert_eq!(view.schemas_unavailable, None);
}

/// The example's manifest is one `ephor validate --manifest` accepts: it holds
/// to the published manifest schema whole, each declaration to the strict
/// shape one is checked against, and ephor reads three monitors of three kinds
/// from it with nothing noted and no rule broken (§FS-021-monitors.4.1,
/// §FS-021-monitors.4.6).
#[test]
fn the_examples_manifest_declares_three_monitors_ephor_reads() {
    let example = example(&topic());
    let value: Json = serde_json::from_str(&example.manifest).unwrap();
    let problems = whole(MANIFEST_SCHEMA, &value);
    assert!(problems.is_empty(), "{problems:?}");
    let declaration = entry_validator(MANIFEST_SCHEMA, "monitor");
    for entry in value["monitors"].as_array().unwrap() {
        assert!(declaration.is_valid(entry), "{entry}");
    }

    let manifest = crate::manifest::parse(&example.manifest, "ephor.json").unwrap();
    assert_eq!(manifest.refused_monitors(), Vec::<String>::new());
    let declared = read_declarations(&manifest.monitors, Home::Manifest);
    assert!(declared.refused.is_empty(), "{:?}", declared.refused);
    let said: Vec<(String, String, String)> = declared
        .declarations
        .iter()
        .map(|declaration| {
            let monitor = resolve(
                &declaration.id,
                &Homes {
                    manifest: Some(declaration),
                    ..Homes::default()
                },
            );
            assert!(monitor.is_readable(), "{monitor:?}");
            assert!(monitor.notes.is_empty(), "{:?}", monitor.notes);
            assert!(monitor.is_status(None), "{}", monitor.id);
            (
                monitor.id.clone(),
                monitor.words(None, None),
                monitor.cadence.say(),
            )
        })
        .collect();
    // What the paragraph after the manifest says of each.
    assert_eq!(
        said,
        [
            ("releasable", "healthy when yes", "every 10m"),
            ("todo-markers", "healthy ≤ 20 · warn above 10", "every 4h"),
            (
                "nightly",
                "healthy: passed or flaky · warn: flaky",
                "every 1d at 06:00"
            ),
        ]
        .map(|(id, words, cadence)| (
            id.to_string(),
            words.to_string(),
            cadence.to_string()
        ))
    );
}

/// The example's script, run in the checkout the text describes, writes the
/// answer the text shows, which holds to the published answer schema and
/// gives each monitor a sample its kind reads (§FS-021-monitors.5.4,
/// §FS-021-monitors.8.1).
#[test]
fn the_examples_script_writes_the_answer_the_text_shows() {
    let example = example(&topic());
    let dir = checkout(&example);
    as_described(&dir.path().join("checkout"));
    let answer = answer_of(dir.path(), "releasable,todo-markers,nightly");
    holds_as_an_answer(&answer);
    let shown: Json = serde_json::from_str(&example.answer).unwrap();
    assert_eq!(answer, shown, "the script writes what the text shows");

    let manifest = crate::manifest::parse(&example.manifest, "ephor.json").unwrap();
    let declared = read_declarations(&manifest.monitors, Home::Manifest);
    let bands: Vec<(String, Value, Option<Band>)> = answer["monitors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let sample = read_sample(entry).unwrap();
            let homes = Homes {
                manifest: declared.get(&sample.id),
                ..Homes::default()
            };
            let checked = crate::monitors::sample::check(&sample, &homes).unwrap();
            let monitor = &checked.monitor;
            let judged = band(
                &monitor.judge(sample.ok),
                monitor.warn.as_ref(),
                &checked.value,
                sample.ok,
                monitor.levels.as_deref(),
            );
            (sample.id, checked.value, judged)
        })
        .collect();
    assert_eq!(
        bands,
        [
            (
                "releasable".to_string(),
                Value::Boolean(true),
                Some(Band::Ok)
            ),
            (
                "todo-markers".to_string(),
                Value::Number(3.0),
                Some(Band::Ok)
            ),
            (
                "nightly".to_string(),
                Value::Level("passed".to_string()),
                Some(Band::Ok)
            ),
        ]
    );
}

/// The bands the paragraph after the manifest names: a count is ok up to 10,
/// warn to 20 and down above, and the nightly build ok when passed, warn when
/// flaky and down when failed (§FS-021-monitors.3.3).
#[test]
fn the_examples_bands_are_the_ones_the_text_says() {
    let example = example(&topic());
    let manifest = crate::manifest::parse(&example.manifest, "ephor.json").unwrap();
    let declared = read_declarations(&manifest.monitors, Home::Manifest);
    let band_of = |id: &str, value: Value| {
        let monitor = resolve(
            id,
            &Homes {
                manifest: declared.get(id),
                ..Homes::default()
            },
        );
        band(
            &monitor.judge(None),
            monitor.warn.as_ref(),
            &value,
            None,
            monitor.levels.as_deref(),
        )
    };
    let count = |n: f64| band_of("todo-markers", Value::Number(n));
    assert_eq!(
        [count(10.0), count(11.0), count(20.0), count(21.0)],
        [
            Some(Band::Ok),
            Some(Band::Warn),
            Some(Band::Warn),
            Some(Band::Down)
        ]
    );
    let level = |name: &str| band_of("nightly", Value::Level(name.to_string()));
    assert_eq!(
        [level("passed"), level("flaky"), level("failed")],
        [Some(Band::Ok), Some(Band::Warn), Some(Band::Down)]
    );
    assert_eq!(
        band_of("releasable", Value::Boolean(false)),
        Some(Band::Down)
    );
}

/// In a checkout with none of what the script reads, it still answers in the
/// published shape: releasable says why it is false, the count is 0, and the
/// nightly sample carries no value, which is an unreadable read of nightly
/// alone, never the answer refused (§FS-021-monitors.8.2).
#[test]
fn a_value_the_script_cannot_give_is_left_out_and_costs_that_monitor_alone() {
    let example = example(&topic());
    let dir = checkout(&example);
    let answer = answer_of(dir.path(), "nightly");
    holds_as_an_answer(&answer);
    assert_eq!(
        answer["monitors"],
        json!([
            {"id": "releasable", "value": false, "detail": "no CHANGELOG.md entry for VERSION"},
            {"id": "todo-markers", "value": 0},
            {"id": "nightly", "detail": "the nightly job left no target/nightly.status"}
        ])
    );
    let manifest = crate::manifest::parse(&example.manifest, "ephor.json").unwrap();
    let declared = read_declarations(&manifest.monitors, Home::Manifest);
    for entry in answer["monitors"].as_array().unwrap() {
        let sample = read_sample(entry).unwrap();
        let homes = Homes {
            manifest: declared.get(&sample.id),
            ..Homes::default()
        };
        let read = crate::monitors::sample::check(&sample, &homes);
        assert_eq!(read.is_err(), sample.id == "nightly", "{entry}: {read:?}");
    }
    let text = topic();
    assert!(
        text.contains("that read of nightly alone is unreadable"),
        "{text}"
    );
}

/// The script as the text prints it, two-space indent and all: the lines
/// after the example's first sentence that name it.
fn printed_script(text: &str) -> String {
    let (_, after) = text.split_once("Worked example.").unwrap();
    let lines: Vec<&str> = after
        .lines()
        .skip_while(|line| !line.ends_with("kinds:"))
        .skip(1)
        .take_while(|line| line.is_empty() || line.starts_with("  "))
        .collect();
    lines.join("\n").trim_end().to_string() + "\n"
}

/// A reader who copies the script with the indent the text prints it in
/// still has one that writes the answer the text shows: it writes with
/// `printf`, so no line of it has to start a line (§FS-020-guide.1).
#[test]
fn the_script_copied_with_its_indent_writes_the_same_answer() {
    let text = topic();
    let printed = printed_script(&text);
    assert!(printed.starts_with("  #!/bin/sh\n"), "{printed}");
    let mut example = example(&text);
    let shown: Json = serde_json::from_str(&example.answer).unwrap();
    example.script = printed;
    let dir = checkout(&example);
    as_described(&dir.path().join("checkout"));
    let answer = answer_of(dir.path(), "releasable,todo-markers,nightly");
    holds_as_an_answer(&answer);
    assert_eq!(answer, shown);
}

/// What the closing paragraph says validation leaves to the refresh: each
/// mistake it names fits the declaration's shape, so `ephor validate
/// --manifest` passes it, and the monitor as its homes resolve it is not
/// read; a key ephor does not know, a `timeout` inside `from` among them, is
/// noted and set aside, and `timeout` beside `from` is a whole number of
/// seconds (§FS-021-monitors.4.6, §FS-021-monitors.3.5, §FS-021-monitors.5.5).
#[test]
fn validation_holds_the_shape_alone_and_the_resolved_monitor_the_rules() {
    let from = json!({"command": "scripts/kpis.sh"});
    let read = |declaration: Json| {
        let manifest = json!({"v": 1, "monitors": [declaration]}).to_string();
        let manifest = crate::manifest::parse(&manifest, "ephor.json").unwrap();
        let declared = read_declarations(&manifest.monitors, Home::Manifest);
        let monitor = declared.declarations.first().map(|declaration| {
            resolve(
                &declaration.id,
                &Homes {
                    manifest: Some(declaration),
                    ..Homes::default()
                },
            )
        });
        (manifest.refused_monitors(), monitor)
    };
    for broken in [
        json!({"id": "m", "kind": "number", "healthy": {"is": true}, "from": from}),
        json!({"id": "m", "kind": "number", "warn": {"max": 10}, "from": from}),
        json!({"id": "m", "kind": "number", "healthy": {"max": 5}, "warn": {"max": 7}, "from": from}),
        json!({"id": "m", "kind": "number", "every": "5h", "at": "06:00", "from": from}),
    ] {
        let (refused, monitor) = read(broken.clone());
        assert_eq!(refused, Vec::<String>::new(), "validation passes {broken}");
        let monitor = monitor.unwrap();
        assert!(!monitor.is_readable(), "{broken} is read: {monitor:?}");
    }
    for (unknown, note) in [
        (
            json!({"id": "m", "kind": "number", "heathy": {"min": 1}, "from": from}),
            "unknown field heathy",
        ),
        (
            json!({"id": "m", "kind": "number", "from": {"command": "s", "timeout": 3}}),
            "unknown field from.timeout",
        ),
    ] {
        let (refused, monitor) = read(unknown.clone());
        assert_eq!(refused, Vec::<String>::new(), "validation passes {unknown}");
        let monitor = monitor.unwrap();
        assert!(monitor.is_readable(), "{unknown}: {monitor:?}");
        assert!(
            monitor.notes.iter().any(|said| said.contains(note)),
            "{unknown}: {:?}",
            monitor.notes
        );
    }
    let (refused, monitor) = read(json!({"id": "m", "from": from, "timeout": 120}));
    assert!(refused.is_empty() && monitor.unwrap().is_readable());
    let (refused, _) = read(json!({"id": "m", "from": from, "timeout": "2m"}));
    assert_eq!(refused.len(), 1, "a timeout that is not whole seconds");
}
