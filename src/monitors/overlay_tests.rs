use serde_json::json;

use super::*;
use crate::monitors::shape::read_declaration;

fn declared(entry: serde_json::Value) -> Declaration {
    read_declaration(&entry).unwrap()
}

fn described(entry: serde_json::Value) -> Description {
    let mut entry = entry;
    entry["id"] = json!("m");
    crate::monitors::shape::read_sample(&entry)
        .unwrap()
        .description
}

fn resolve_with(
    site: Option<serde_json::Value>,
    manifest: Option<serde_json::Value>,
    sample: Option<serde_json::Value>,
) -> Monitor {
    let site = site.map(declared);
    let manifest = manifest.map(declared);
    let sample = sample.map(described);
    resolve(
        "m",
        &Homes {
            site: site.as_ref(),
            manifest: manifest.as_ref(),
            sample: sample.as_ref(),
            first_kind: None,
            descriptions_only: false,
        },
    )
}

#[test]
fn the_site_wins_then_the_manifest_then_the_samples() {
    let monitor = resolve_with(
        Some(json!({"id": "m", "title": "Site title", "every": "1h"})),
        Some(
            json!({"id": "m", "title": "Manifest title", "unit": "%", "every": "1d", "timeout": 90}),
        ),
        Some(json!({"title": "Sample title", "unit": "ms", "better": "higher"})),
    );
    assert_eq!(monitor.title, "Site title");
    assert_eq!(monitor.unit.as_deref(), Some("%"));
    assert_eq!(monitor.better, Some(Better::Higher));
    assert_eq!(monitor.cadence.say(), "every 1h");
    assert_eq!(monitor.timeout, 90);
    assert_eq!(monitor.origins["title"], Home::Site);
    assert_eq!(monitor.origins["unit"], Home::Manifest);
    assert_eq!(monitor.origins["better"], Home::Sample);
    assert!(monitor.cadence_stated);
}

#[test]
fn the_defaults_fill_what_no_home_says() {
    let monitor = resolve_with(None, Some(json!({"id": "m"})), None);
    assert_eq!(monitor.title, "m");
    assert_eq!(monitor.cadence.say(), "every 10m");
    assert!(!monitor.cadence_stated);
    assert_eq!(monitor.timeout, 30);
    assert_eq!(monitor.drop_after, 1);
    assert_eq!(monitor.recover_after(&monitor.cadence), 2);
    let daily = Cadence::resolve(Some("1d"), None).unwrap();
    assert_eq!(monitor.recover_after(&daily), 1);
    let stated = resolve_with(None, Some(json!({"id": "m", "recover_after": 3})), None);
    assert_eq!(stated.recover_after(&daily), 3);
}

#[test]
fn healthy_warn_and_from_are_taken_whole_from_one_home() {
    let monitor = resolve_with(
        Some(json!({"id": "m", "healthy": {"max": 5}})),
        Some(json!({
            "id": "m", "kind": "number", "healthy": {"min": 1, "max": 9}, "warn": {"max": 3},
            "from": {"command": "measure", "cwd": "root"}
        })),
        None,
    );
    assert_eq!(
        monitor.healthy,
        Healthy::Said(
            Condition {
                max: Some(5.0),
                ..Condition::default()
            },
            Home::Site
        )
    );
    assert_eq!(monitor.warn.as_ref().unwrap().max, Some(3.0));
    assert_eq!(monitor.origins["warn"], Home::Manifest);
    assert_eq!(
        monitor.from,
        Some(Source::Command {
            command: "measure".into(),
            cwd: Some("root".into())
        })
    );
    let set_aside = resolve_with(
        Some(json!({"id": "m", "from": null, "warn": null})),
        Some(
            json!({"id": "m", "kind": "number", "healthy": {"max": 5}, "warn": {"max": 3}, "from": {"file": "x"}}),
        ),
        None,
    );
    assert_eq!(set_aside.from, None);
    assert_eq!(set_aside.warn, None);
    assert_eq!(set_aside.origins["from"], Home::Site);
}

#[test]
fn health_is_judged_in_order_and_null_stops_the_search() {
    let site_null = resolve_with(
        Some(json!({"id": "m", "healthy": null})),
        Some(json!({"id": "m", "kind": "number", "healthy": {"min": 80}, "warn": {"min": 90}})),
        Some(json!({"healthy": {"min": 70}})),
    );
    assert_eq!(site_null.judge(Some(false)), Judge::Kpi);
    assert_eq!(site_null.warn, None);
    assert!(site_null.is_readable());
    assert_eq!(site_null.words(Some(false), None), "KPI");

    let manifest = resolve_with(
        None,
        Some(json!({"id": "m", "kind": "number", "healthy": {"min": 80}})),
        Some(json!({"healthy": {"min": 70}})),
    );
    assert_eq!(
        manifest.judge(Some(false)).condition().unwrap().min,
        Some(80.0)
    );

    let brought = resolve_with(
        None,
        Some(json!({"id": "m", "kind": "number"})),
        Some(json!({"healthy": {"min": 70}})),
    );
    assert_eq!(
        brought.judge(Some(false)).condition().unwrap().min,
        Some(70.0)
    );
    assert_eq!(brought.origins["healthy"], Home::Sample);

    let brought_null = resolve_with(
        None,
        Some(json!({"id": "m", "kind": "boolean"})),
        Some(json!({"healthy": null})),
    );
    assert_eq!(brought_null.judge(Some(false)), Judge::Kpi);

    let by_ok = resolve_with(None, Some(json!({"id": "m", "kind": "boolean"})), None);
    assert_eq!(by_ok.judge(Some(false)), Judge::SampleOk);
    assert_eq!(by_ok.judge(None).condition().unwrap().is, Some(true));

    let number = resolve_with(None, Some(json!({"id": "m", "kind": "number"})), None);
    assert_eq!(number.judge(None), Judge::Kpi);
    assert_eq!(number.words(None, None), "KPI");

    let level = resolve_with(
        None,
        Some(json!({"id": "m", "levels": ["clean", "dirty"]})),
        None,
    );
    assert_eq!(
        level.judge(None).condition().unwrap().within,
        Some(vec!["clean".to_string()])
    );
    assert_eq!(level.words(None, None), "healthy when clean");
}

#[test]
fn the_kind_is_inferred_where_no_home_states_it() {
    let by_levels = resolve_with(None, Some(json!({"id": "m", "levels": ["a", "b"]})), None);
    assert_eq!(by_levels.kind, Some(Kind::Level));
    let by_is = resolve_with(
        None,
        Some(json!({"id": "m", "healthy": {"is": true}})),
        None,
    );
    assert_eq!(by_is.kind, Some(Kind::Boolean));
    let by_min = resolve_with(None, Some(json!({"id": "m", "healthy": {"min": 1}})), None);
    assert_eq!(by_min.kind, None);
    let by_sample = resolve_with(
        None,
        Some(json!({"id": "m"})),
        Some(json!({"kind": "series"})),
    );
    assert_eq!(by_sample.kind, Some(Kind::Series));
    let manifest = declared(json!({"id": "m"}));
    let by_first = resolve(
        "m",
        &Homes {
            manifest: Some(&manifest),
            first_kind: Some(Kind::Number),
            ..Homes::default()
        },
    );
    assert_eq!(by_first.kind, Some(Kind::Number));
    let stated = resolve_with(
        Some(json!({"id": "m", "kind": "number"})),
        Some(json!({"id": "m", "kind": "series"})),
        None,
    );
    assert_eq!(stated.kind, Some(Kind::Number));
}

#[test]
fn workflow_and_action_are_one_choice_across_homes() {
    let site_action = resolve_with(
        Some(json!({"id": "m", "action": "notify"})),
        Some(json!({"id": "m", "workflow": "fix-it"})),
        None,
    );
    assert_eq!(
        site_action.proposal,
        Some(Proposal::Action("notify".into()))
    );
    let manifest = resolve_with(
        Some(json!({"id": "m"})),
        Some(json!({"id": "m", "workflow": "fix-it"})),
        None,
    );
    assert_eq!(manifest.proposal, Some(Proposal::Workflow("fix-it".into())));
    let set_aside = resolve_with(
        Some(json!({"id": "m", "workflow": null})),
        Some(json!({"id": "m", "workflow": "fix-it"})),
        None,
    );
    assert_eq!(set_aside.proposal, None);
    assert_eq!(set_aside.origins["workflow"], Home::Site);
}

/// §FS-021-monitors.3.5: a warn needs a healthy to narrow, declared,
/// brought or its kind's default; whether its samples bring one is known
/// once one has been read, so until then it is read to find out.
#[test]
fn a_warn_with_no_healthy_to_narrow_is_a_fault() {
    let warned = || Some(json!({"id": "m", "kind": "number", "warn": {"max": 3}}));
    let monitor = resolve_with(None, warned(), Some(json!({"id": "m"})));
    assert_eq!(
        monitor.faults,
        vec!["warn has no healthy to narrow: declare a healthy, or drop the warn"]
    );
    assert!(!monitor.is_readable());
    assert!(resolve_with(None, warned(), None).is_readable());
    let brought = resolve_with(
        None,
        warned(),
        Some(json!({"id": "m", "healthy": {"max": 5}})),
    );
    assert!(brought.is_readable());
    let boolean = resolve_with(
        None,
        Some(json!({"id": "m", "kind": "boolean", "warn": {"is": true}})),
        None,
    );
    assert!(boolean.is_readable());
    let site_healthy = resolve_with(
        Some(json!({"id": "m", "healthy": {"max": 5}})),
        Some(json!({"id": "m", "kind": "number", "warn": {"max": 3}})),
        None,
    );
    assert!(site_healthy.is_readable());
}

#[test]
fn a_warn_laxer_than_healthy_is_a_fault_on_the_resolved_monitor() {
    let monitor = resolve_with(
        Some(json!({"id": "m", "healthy": {"max": 5}})),
        Some(json!({"id": "m", "kind": "number", "healthy": {"max": 10}, "warn": {"max": 7}})),
        None,
    );
    assert_eq!(
        monitor.faults,
        vec!["warn's max 7 lets through what healthy's max 5 refuses"]
    );
}

#[test]
fn a_condition_that_does_not_fit_the_resolved_kind_is_a_fault() {
    let monitor = resolve_with(
        Some(json!({"id": "m", "kind": "boolean"})),
        Some(json!({"id": "m", "healthy": {"min": 80}})),
        None,
    );
    assert_eq!(monitor.faults, vec!["healthy's min does not fit a boolean"]);
    let unknown = resolve_with(
        None,
        Some(json!({"id": "m", "kind": "number", "healthy": {"minimum": 80}})),
        None,
    );
    assert_eq!(
        unknown.faults,
        vec!["healthy's minimum is not a condition key: is, min, max, in or at_most"]
    );
}

#[test]
fn at_and_every_are_paired_on_the_resolved_monitor() {
    let manifest = json!({"id": "m", "at": "Mon 06:00", "from": {"command": "scripts/kpis.sh"}});
    let monitor = resolve_with(
        Some(json!({"id": "m", "every": "1d"})),
        Some(manifest.clone()),
        None,
    );
    assert_eq!(
        monitor.faults,
        vec!["at Mon 06:00 needs an every of whole weeks, and 1d is not one"]
    );
    let cleared = resolve_with(
        Some(json!({"id": "m", "every": "1d", "at": null})),
        Some(manifest),
        None,
    );
    assert!(cleared.is_readable());
    assert_eq!(cleared.cadence.say(), "every 1d");
}

#[test]
fn notes_from_both_declarations_are_kept() {
    let monitor = resolve_with(
        Some(json!({"id": "m", "heathy": {"min": 1}})),
        Some(json!({"id": "m", "kidn": "number"})),
        None,
    );
    assert_eq!(
        monitor.notes,
        vec!["unknown field heathy", "unknown field kidn"]
    );
}

#[test]
fn the_condition_is_said_with_the_readings_whole() {
    let monitor = resolve_with(
        None,
        Some(json!({"id": "m", "kind": "number", "of": 4, "healthy": {"min": 4}})),
        None,
    );
    assert_eq!(monitor.words(None, None), "healthy at 4/4");
    assert_eq!(monitor.words(None, Some(5.0)), "healthy ≥ 4/5");
}

#[test]
fn a_monitor_read_from_the_answer_that_brings_it_takes_only_the_sites_cadence() {
    // The manifest's `every` sets no cadence for a monitor that names no `from`.
    let brought = resolve_with(
        None,
        Some(json!({"id": "m", "kind": "number", "every": "1d", "at": "06:00"})),
        None,
    );
    assert!(!brought.cadence_stated);
    assert_eq!(brought.cadence, Cadence::default_every());
    assert_eq!(brought.origins.get("every"), None);
    assert_eq!(brought.origins.get("at"), None);
    // The site may state its own for freshness.
    let site = resolve_with(
        Some(json!({"id": "m", "every": "1h"})),
        Some(json!({"id": "m", "every": "1d", "at": "06:00"})),
        None,
    );
    assert!(site.cadence_stated);
    assert_eq!(site.cadence.say(), "every 1h");
    assert_eq!(site.origins["every"], Home::Site);
    // A monitor that names a `from` keeps the manifest's.
    let from = resolve_with(
        None,
        Some(json!({"id": "m", "every": "1d", "from": {"command": "scripts/kpis.sh"}})),
        None,
    );
    assert!(from.cadence_stated);
    assert_eq!(from.cadence.say(), "every 1d");
}

#[test]
fn a_row_that_trusts_only_descriptions_runs_no_command_of_the_manifest() {
    let manifest = |from: serde_json::Value| {
        declared(json!({"id": "m", "kind": "number", "every": "1h", "from": from}))
    };
    let site_from = declared(json!({"id": "m", "from": {"command": "scripts/site.sh"}}));
    let resolve_row = |site: Option<&Declaration>, manifest: &Declaration| {
        resolve(
            "m",
            &Homes {
                site,
                manifest: Some(manifest),
                descriptions_only: true,
                ..Homes::default()
            },
        )
    };
    let command = manifest(json!({"command": "scripts/kpis.sh"}));
    let not_run = resolve_row(None, &command);
    assert!(not_run.not_run);
    assert_eq!(not_run.from, None);
    assert!(!not_run.is_readable());
    assert!(not_run.faults.is_empty());
    assert_eq!(
        NOT_RUN,
        "not run: this registry row reads descriptions only"
    );
    // A file is data, and is read.
    let file = resolve_row(None, &manifest(json!({"file": "target/kpi.json"})));
    assert!(!file.not_run && file.is_readable());
    assert_eq!(file.cadence.say(), "every 1h");
    // A site declaration that names its own `from` is read as the site declares it.
    let site = resolve_row(Some(&site_from), &command);
    assert!(!site.not_run && site.is_readable());
    assert_eq!(
        site.from,
        Some(Source::Command {
            command: "scripts/site.sh".into(),
            cwd: None
        })
    );
    // A row that trusts the manifest fully runs its command.
    let full = resolve(
        "m",
        &Homes {
            manifest: Some(&command),
            ..Homes::default()
        },
    );
    assert!(!full.not_run && full.is_readable());
}

/// §FS-021-monitors.5.3: a file's path resolves against the place it is
/// read in, so one that leaves it, and a `repo:` that names no repository,
/// break a rule and are not read; a `..` that stays inside the place is read.
#[test]
fn a_from_that_leaves_the_checkout_is_a_declaration_error() {
    for from in [
        json!({"file": "/etc/kpi.json"}),
        json!({"file": "../kpi.json"}),
        json!({"file": "out/../../kpi.json"}),
        json!({"command": "sh kpis.sh", "cwd": "repo:.."}),
        json!({"file": "kpi.json", "cwd": "repo:."}),
    ] {
        let monitor = resolve_with(
            Some(json!({"id": "m", "kind": "number", "from": from.clone()})),
            None,
            None,
        );
        assert!(!monitor.is_readable(), "{from} is read: {monitor:?}");
    }
    // A `..` that stays inside the place leaves nothing.
    for file in ["target/../kpi.json", "a/b/../../kpi.json", "./kpi.json"] {
        let monitor = resolve_with(
            Some(json!({"id": "m", "kind": "number", "from": {"file": file}})),
            None,
            None,
        );
        assert!(monitor.is_readable(), "{file} is not read: {monitor:?}");
    }
    for from in [
        json!({"file": "out/kpi.json"}),
        json!({"file": "./kpi.json", "cwd": "repo:sub"}),
        json!({"command": "sh ../kpis.sh", "cwd": "root"}),
    ] {
        let monitor = resolve_with(
            Some(json!({"id": "m", "kind": "number", "from": from.clone()})),
            None,
            None,
        );
        assert!(monitor.is_readable(), "{from}: {:?}", monitor.faults);
    }
}
