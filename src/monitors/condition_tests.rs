use super::*;

fn levels() -> Vec<String> {
    ["clean", "degraded", "failed"].map(String::from).to_vec()
}

fn number(min: Option<f64>, max: Option<f64>) -> Condition {
    Condition {
        min,
        max,
        ..Condition::default()
    }
}

fn at_most(level: &str) -> Condition {
    Condition {
        at_most: Some(level.into()),
        ..Condition::default()
    }
}

fn within(names: &[&str]) -> Condition {
    Condition {
        within: Some(names.iter().map(|name| name.to_string()).collect()),
        ..Condition::default()
    }
}

#[test]
fn a_value_meets_every_key_the_condition_holds() {
    let band = number(Some(40.0), Some(60.0));
    assert!(meets(&band, &Value::Number(40.0), None));
    assert!(meets(&band, &Value::Number(60.0), None));
    assert!(!meets(&band, &Value::Number(39.9), None));
    assert!(!meets(&band, &Value::Number(60.1), None));
    assert!(!meets(&band, &Value::Boolean(true), None));
    let yes = Condition {
        is: Some(true),
        ..Condition::default()
    };
    assert!(meets(&yes, &Value::Boolean(true), None));
    assert!(!meets(&yes, &Value::Boolean(false), None));
    let levels = levels();
    let level = |name: &str| Value::Level(name.into());
    assert!(meets(&at_most("degraded"), &level("clean"), Some(&levels)));
    assert!(meets(
        &at_most("degraded"),
        &level("degraded"),
        Some(&levels)
    ));
    assert!(!meets(
        &at_most("degraded"),
        &level("failed"),
        Some(&levels)
    ));
    assert!(!meets(
        &at_most("degraded"),
        &level("unheard"),
        Some(&levels)
    ));
    assert!(meets(
        &within(&["clean", "failed"]),
        &level("failed"),
        Some(&levels)
    ));
    assert!(!meets(
        &within(&["clean"]),
        &level("degraded"),
        Some(&levels)
    ));
}

#[test]
fn a_reading_is_down_warn_or_ok() {
    let healthy = Judge::Condition(number(None, Some(5.0)));
    let warn = number(None, Some(0.0));
    let band = |v: f64| super::band(&healthy, Some(&warn), &Value::Number(v), None, None);
    assert_eq!(band(0.0), Some(Band::Ok));
    assert_eq!(band(3.0), Some(Band::Warn));
    assert_eq!(band(5.0), Some(Band::Warn));
    assert_eq!(band(6.0), Some(Band::Down));
    assert_eq!(
        super::band(&Judge::Kpi, None, &Value::Number(1.0), Some(false), None),
        None
    );
}

#[test]
fn the_samples_ok_judges_where_nothing_else_does() {
    let value = Value::Number(3.0);
    assert_eq!(
        band(&Judge::SampleOk, None, &value, Some(true), None),
        Some(Band::Ok)
    );
    assert_eq!(
        band(&Judge::SampleOk, None, &value, Some(false), None),
        Some(Band::Down)
    );
    assert_eq!(band(&Judge::SampleOk, None, &value, None, None), None);
    let warn = number(None, Some(2.0));
    assert_eq!(
        band(&Judge::SampleOk, Some(&warn), &value, Some(true), None),
        Some(Band::Warn)
    );
}

#[test]
fn a_kind_has_its_default() {
    assert_eq!(
        kind_default(Some(Kind::Boolean), None).unwrap().is,
        Some(true)
    );
    let levels = levels();
    assert_eq!(
        kind_default(Some(Kind::Level), Some(&levels))
            .unwrap()
            .within,
        Some(vec!["clean".to_string()])
    );
    assert_eq!(kind_default(Some(Kind::Level), None), None);
    assert_eq!(kind_default(Some(Kind::Number), None), None);
    assert_eq!(kind_default(Some(Kind::Series), None), None);
    assert_eq!(kind_default(None, None), None);
}

#[test]
fn a_condition_implies_a_kind_only_where_its_keys_decide() {
    assert_eq!(
        implied_kind(&Condition {
            is: Some(false),
            ..Condition::default()
        }),
        Some(Kind::Boolean)
    );
    assert_eq!(implied_kind(&at_most("x")), Some(Kind::Level));
    assert_eq!(implied_kind(&within(&["x"])), Some(Kind::Level));
    assert_eq!(implied_kind(&number(Some(1.0), None)), None);
}

#[test]
fn a_condition_that_does_not_fit_its_kind_is_a_fault() {
    let levels = levels();
    assert_eq!(
        faults(
            "healthy",
            &number(Some(80.0), None),
            Some(Kind::Boolean),
            None
        ),
        vec!["healthy's min does not fit a boolean"]
    );
    assert_eq!(
        faults("warn", &at_most("clean"), Some(Kind::Number), None),
        vec!["warn's at_most does not fit a number"]
    );
    assert!(faults(
        "healthy",
        &number(Some(1.0), None),
        Some(Kind::Series),
        None
    )
    .is_empty());
    assert_eq!(
        faults(
            "healthy",
            &Condition {
                is: Some(true),
                min: Some(1.0),
                ..Condition::default()
            },
            None,
            None
        ),
        vec!["healthy mixes is and min, which fit no one kind"]
    );
    assert_eq!(
        faults(
            "healthy",
            &within(&["clean", "spotless"]),
            Some(Kind::Level),
            Some(&levels)
        ),
        vec!["healthy's in names spotless, which its levels do not hold: clean, degraded, failed"]
    );
    let unknown = Condition {
        unknown: vec!["minimum".into()],
        ..Condition::default()
    };
    assert_eq!(
        faults("healthy", &unknown, Some(Kind::Number), None),
        vec!["healthy's minimum is not a condition key: is, min, max, in or at_most"]
    );
}

#[test]
fn a_condition_no_value_can_meet_is_a_fault() {
    let levels = levels();
    assert_eq!(
        faults(
            "healthy",
            &number(Some(90.0), Some(80.0)),
            Some(Kind::Number),
            None
        ),
        vec!["healthy can never be met: its min 90 is above its max 80"]
    );
    let none = Condition {
        within: Some(vec!["failed".into()]),
        at_most: Some("degraded".into()),
        ..Condition::default()
    };
    assert_eq!(
        faults("healthy", &none, Some(Kind::Level), Some(&levels)),
        vec!["healthy can never be met: none of failed is at most degraded"]
    );
}

#[test]
fn a_warn_laxer_than_healthy_is_a_fault_where_it_can_be_decided() {
    assert_eq!(
        laxer(&number(None, Some(5.0)), &number(None, Some(7.0))).unwrap(),
        "warn's max 7 lets through what healthy's max 5 refuses"
    );
    assert_eq!(
        laxer(&number(Some(80.0), None), &number(Some(70.0), None)).unwrap(),
        "warn's min 70 lets through what healthy's min 80 refuses"
    );
    assert_eq!(
        laxer(&number(None, Some(5.0)), &number(None, Some(0.0))),
        None
    );
    assert_eq!(
        laxer(&number(Some(80.0), None), &number(None, Some(95.0))),
        None
    );
    assert_eq!(laxer(&at_most("degraded"), &within(&["clean"])), None);
}

#[test]
fn a_judge_keeps_its_shape_in_the_store() {
    let judge = Judge::Condition(number(Some(80.0), None));
    let json = serde_json::to_value(&judge).unwrap();
    assert_eq!(json, serde_json::json!({"condition": {"min": 80.0}}));
    assert_eq!(serde_json::from_value::<Judge>(json).unwrap(), judge);
    assert_eq!(
        serde_json::to_value(Judge::Kpi).unwrap(),
        serde_json::json!("kpi")
    );
    assert!(Judge::Kpi.is_kpi());
    assert_eq!(Judge::SampleOk.condition(), None);
}

#[test]
fn two_judges_are_the_same_when_they_accept_the_same_values() {
    use serde_json::json;
    let levels: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
    let judge = |json| Judge::Condition(serde_json::from_value::<Condition>(json).unwrap());
    let level = |a, b| same_judge(&judge(a), &judge(b), Some(Kind::Level), Some(&levels));
    assert!(level(json!({"in": ["a", "b"]}), json!({"in": ["b", "a"]})));
    assert!(level(json!({"in": ["a", "b"]}), json!({"at_most": "b"})));
    assert!(!level(json!({"in": ["a", "b"]}), json!({"in": ["a"]})));
    let boolean = |a, b| same_judge(&judge(a), &judge(b), Some(Kind::Boolean), None);
    assert!(boolean(json!({"is": true}), json!({"is": true})));
    assert!(!boolean(json!({"is": true}), json!({"is": false})));
    let number = |a, b| same_judge(&judge(a), &judge(b), Some(Kind::Number), None);
    assert!(number(json!({"min": 80}), json!({"min": 80.0})));
    assert!(!number(json!({"min": 80}), json!({"min": 85})));
    assert!(!same_judge(&Judge::SampleOk, &Judge::Kpi, None, None));
    assert!(same_judge(&Judge::SampleOk, &Judge::SampleOk, None, None));
}
