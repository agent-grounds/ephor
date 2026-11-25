use serde_json::json;

use super::*;
use crate::monitors::test_zone::utc;

fn command(text: &str) -> Source {
    Source::Command {
        command: text.into(),
        cwd: None,
    }
}

fn from(text: &str) -> Owner {
    Owner::From(command(text))
}

fn answered(source: Owner, entries: serde_json::Value) -> Answer {
    Answer {
        source,
        outcome: Outcome::Answered(entries.as_array().unwrap().clone()),
    }
}

struct Fixture {
    routes: BTreeMap<String, Route>,
    owners: BTreeMap<String, Owner>,
    due: BTreeSet<String>,
}

impl Fixture {
    fn new(routes: &[(&str, Route)], due: &[&str]) -> Fixture {
        Fixture {
            routes: routes
                .iter()
                .map(|(id, route)| (id.to_string(), route.clone()))
                .collect(),
            owners: BTreeMap::new(),
            due: due.iter().map(|id| id.to_string()).collect(),
        }
    }

    fn route(&self, plain: bool, answers: &[Answer]) -> Routed {
        let watched = Watched {
            routes: &self.routes,
            owners: &self.owners,
            due: &self.due,
            plain,
        };
        route(&watched, answers)
    }
}

fn value(delivery: Option<&Delivery>) -> serde_json::Value {
    match delivery {
        Some(Delivery::Sample(sample)) => serde_json::to_value(sample.value.clone()).unwrap(),
        other => panic!("not a sample: {other:?}"),
    }
}

fn unreadable(delivery: Option<&Delivery>) -> &str {
    match delivery {
        Some(Delivery::Unreadable(why)) => why,
        other => panic!("not unreadable: {other:?}"),
    }
}

#[test]
fn a_from_gives_its_due_monitors_their_samples_and_drops_the_rest() {
    let fixture = Fixture::new(
        &[
            ("a", Route::From(command("measure"))),
            ("b", Route::From(command("measure"))),
        ],
        &["a"],
    );
    let routed = fixture.route(
        true,
        &[answered(
            from("measure"),
            json!([{"id": "a", "value": 1}, {"id": "b", "value": 2}]),
        )],
    );
    assert_eq!(value(routed.deliveries.get("a")), json!(1.0));
    assert!(!routed.deliveries.contains_key("b"));
}

#[test]
fn a_sample_for_a_from_monitor_in_another_answer_is_ignored() {
    let fixture = Fixture::new(&[("a", Route::From(command("measure")))], &["a"]);
    let routed = fixture.route(
        true,
        &[
            answered(Owner::CustomStatus, json!([{"id": "a", "value": 9}])),
            answered(from("measure"), json!([{"id": "a", "value": 1}])),
        ],
    );
    assert_eq!(value(routed.deliveries.get("a")), json!(1.0));
    assert!(!routed.owners.contains_key("a"));
}

#[test]
fn an_answer_taken_for_a_monitor_must_carry_exactly_one_sample() {
    let fixture = Fixture::new(
        &[
            ("a", Route::From(command("m"))),
            ("b", Route::From(command("m"))),
            ("c", Route::From(command("m"))),
        ],
        &["a", "b", "c"],
    );
    let routed = fixture.route(
        true,
        &[answered(
            from("m"),
            json!([{"id": "b", "value": 1}, {"id": "b", "value": 2}, {"id": "c", "value": []}, {"value": 3}]),
        )],
    );
    assert_eq!(unreadable(routed.deliveries.get("a")), "no sample for a");
    assert_eq!(unreadable(routed.deliveries.get("b")), "two samples for b");
    assert_eq!(
        unreadable(routed.deliveries.get("c")),
        "the sample's value is a list, not true, false, a number or a level"
    );
    assert_eq!(
        routed.strays,
        vec!["a sample with no id is read for no monitor"]
    );
}

#[test]
fn a_failed_answer_is_unreadable_and_a_parked_one_parked_for_what_it_was_taken_for() {
    let fixture = Fixture::new(
        &[
            ("a", Route::From(command("one"))),
            ("b", Route::From(command("two"))),
            ("c", Route::From(command("two"))),
        ],
        &["a", "b"],
    );
    let routed = fixture.route(
        true,
        &[
            Answer {
                source: from("one"),
                outcome: Outcome::Failed("exited 2".into()),
            },
            Answer {
                source: from("two"),
                outcome: Outcome::Parked,
            },
        ],
    );
    assert_eq!(unreadable(routed.deliveries.get("a")), "exited 2");
    assert_eq!(routed.deliveries.get("b"), Some(&Delivery::Parked));
    assert!(!routed.deliveries.contains_key("c"));
}

#[test]
fn the_first_answer_to_bring_a_monitor_owns_it() {
    let fixture = Fixture::new(
        &[
            ("declared", Route::Brought),
            ("named", Route::From(command("x"))),
        ],
        &[],
    );
    let routed = fixture.route(
        true,
        &[
            answered(from("x"), json!([{"id": "declared", "value": 1}, {"id": "stray", "value": true}])),
            answered(Owner::CustomStatus, json!([{"id": "declared", "value": 2}, {"id": "stray", "value": false}, {"id": "own", "value": 5}])),
        ],
    );
    assert_eq!(value(routed.deliveries.get("declared")), json!(1.0));
    assert_eq!(value(routed.deliveries.get("stray")), json!(true));
    assert_eq!(value(routed.deliveries.get("own")), json!(5.0));
    assert_eq!(routed.owners["declared"], from("x"));
    assert_eq!(routed.owners["stray"], from("x"));
    assert_eq!(routed.owners["own"], Owner::CustomStatus);
    assert_eq!(routed.brought.len(), 3);
}

#[test]
fn an_owned_monitor_is_read_whenever_its_owner_is_taken() {
    let mut fixture = Fixture::new(&[("named", Route::From(command("x")))], &[]);
    fixture.owners.insert("kept".into(), from("x"));
    fixture.owners.insert("status".into(), Owner::CustomStatus);
    let routed = fixture.route(
        true,
        &[
            answered(Owner::CustomStatus, json!([{"id": "kept", "value": 9}])),
            answered(from("x"), json!([])),
        ],
    );
    assert_eq!(
        unreadable(routed.deliveries.get("status")),
        "no sample for status"
    );
    assert_eq!(
        unreadable(routed.deliveries.get("kept")),
        "no sample for kept"
    );
    assert!(!routed.brought.contains("kept"));
}

#[test]
fn an_owner_whose_from_nothing_names_is_let_go() {
    let mut fixture = Fixture::new(&[], &[]);
    fixture.owners.insert("orphan".into(), from("gone"));
    let routed = fixture.route(
        true,
        &[answered(
            Owner::CustomStatus,
            json!([{"id": "orphan", "value": 1}]),
        )],
    );
    assert_eq!(routed.owners["orphan"], Owner::CustomStatus);
    assert_eq!(value(routed.deliveries.get("orphan")), json!(1.0));
}

#[test]
fn a_declared_monitor_no_answer_carries_is_unreadable_on_a_plain_refresh() {
    let fixture = Fixture::new(&[("wanted", Route::Brought)], &[]);
    let routed = fixture.route(true, &[answered(Owner::CustomStatus, json!([]))]);
    assert_eq!(
        unreadable(routed.deliveries.get("wanted")),
        "no answer carries wanted"
    );
    let chosen = fixture.route(false, &[answered(Owner::CustomStatus, json!([]))]);
    assert!(chosen.deliveries.is_empty());
}

#[test]
fn a_monitor_nothing_declared_is_forgotten_after_30_days() {
    let brought = utc("2026-10-26T00:00:00Z");
    assert!(!forgotten(false, brought, utc("2026-11-24T23:59:59Z")));
    assert!(forgotten(false, brought, utc("2026-11-25T00:00:00Z")));
    assert!(!forgotten(true, brought, utc("2027-11-25T00:00:00Z")));
}

#[test]
fn a_brought_monitor_takes_its_answers_cadence() {
    let x = command("x");
    let y = command("y");
    let hourly = Cadence::resolve(Some("1h"), None).unwrap();
    let daily = Cadence::resolve(Some("1d"), None).unwrap();
    let declared = [
        (&x, daily),
        (&x, hourly),
        (&y, Cadence::resolve(Some("1m"), None).unwrap()),
    ];
    assert_eq!(
        brought_cadence(Some(&Owner::From(x.clone())), declared),
        hourly
    );
    assert_eq!(
        brought_cadence(Some(&Owner::CustomStatus), declared),
        Cadence::default_every()
    );
    assert_eq!(brought_cadence(None, declared), Cadence::default_every());
}

#[test]
fn an_owner_keeps_its_shape_in_the_store() {
    assert_eq!(
        serde_json::to_value(Owner::CustomStatus).unwrap(),
        json!("custom_status")
    );
    let owner = from("x");
    let json = serde_json::to_value(&owner).unwrap();
    assert_eq!(json, json!({"from": {"command": "x"}}));
    assert_eq!(serde_json::from_value::<Owner>(json).unwrap(), owner);
}

#[test]
fn a_run_names_the_monitors_it_reads() {
    let mut fixture = Fixture::new(
        &[
            ("due", Route::From(command("x"))),
            ("waiting", Route::From(command("x"))),
            ("other", Route::From(command("y"))),
            ("brought", Route::Brought),
            ("held", Route::Brought),
            ("broken", Route::NotRead),
        ],
        &["due", "other"],
    );
    fixture.owners.insert("held".into(), Owner::CustomStatus);
    fixture.owners.insert("stray".into(), from("x"));
    let reads = |plain: bool, source: &Owner| {
        let watched = Watched {
            routes: &fixture.routes,
            owners: &fixture.owners,
            due: &fixture.due,
            plain,
        };
        reads(&watched, source).into_iter().collect::<Vec<_>>()
    };
    assert_eq!(reads(true, &from("x")), vec!["brought", "due", "stray"]);
    assert_eq!(reads(false, &from("x")), vec!["due", "stray"]);
    assert_eq!(reads(true, &Owner::CustomStatus), vec!["brought", "held"]);
    assert_eq!(reads(false, &from("y")), vec!["other"]);
}

#[test]
fn a_monitor_not_read_takes_no_sample_from_any_answer() {
    let fixture = Fixture::new(&[("broken", Route::NotRead)], &["broken"]);
    let routed = fixture.route(
        true,
        &[answered(
            Owner::CustomStatus,
            json!([{"id": "broken", "value": 1}]),
        )],
    );
    assert!(routed.deliveries.is_empty());
    assert!(routed.owners.is_empty());
    assert!(routed.brought.is_empty());
}

#[test]
fn a_monitor_is_routed_by_what_its_declaration_resolves_to() {
    use crate::monitors::overlay::{resolve, Homes};
    use crate::monitors::shape::read_declaration;
    let monitor = |entry: serde_json::Value| {
        let declaration = read_declaration(&entry).unwrap();
        resolve(
            "m",
            &Homes {
                manifest: Some(&declaration),
                ..Homes::default()
            },
        )
    };
    assert_eq!(
        Route::of(&monitor(json!({"id": "m", "from": {"command": "x"}}))),
        Route::From(command("x"))
    );
    assert_eq!(Route::of(&monitor(json!({"id": "m"}))), Route::Brought);
    let broken = monitor(json!({"id": "m", "kind": "boolean", "healthy": {"min": 1}}));
    assert_eq!(Route::of(&broken), Route::NotRead);
}
