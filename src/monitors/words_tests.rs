use super::*;

fn condition(json: serde_json::Value) -> Condition {
    serde_json::from_value(json).unwrap()
}

fn said(json: serde_json::Value, terms: Terms) -> String {
    healthy(&Judge::Condition(condition(json)), &terms)
}

fn unit(unit: &str) -> Terms<'_> {
    Terms {
        unit: Some(unit),
        ..Terms::default()
    }
}

#[test]
fn a_numbers_healthy_is_said_as_the_values_it_accepts() {
    assert_eq!(
        said(serde_json::json!({"min": 80}), unit("%")),
        "healthy ≥ 80%"
    );
    assert_eq!(
        said(serde_json::json!({"max": 5}), unit("count")),
        "healthy ≤ 5"
    );
    assert_eq!(
        said(serde_json::json!({"min": 40, "max": 60}), unit("ms")),
        "healthy 40–60 ms"
    );
    assert_eq!(
        said(serde_json::json!({"max": 2.5}), unit("$")),
        "healthy ≤ $2.5"
    );
    assert_eq!(
        said(serde_json::json!({"min": 1}), Terms::default()),
        "healthy ≥ 1"
    );
}

#[test]
fn a_condition_that_leaves_one_value_is_said_with_at() {
    assert_eq!(
        said(serde_json::json!({"max": 0}), unit("count")),
        "healthy at 0"
    );
    let fraction = Terms {
        of: Some(4.0),
        ..Terms::default()
    };
    assert_eq!(
        said(serde_json::json!({"min": 4}), fraction),
        "healthy at 4/4"
    );
    assert_eq!(
        said(serde_json::json!({"min": 3}), fraction),
        "healthy ≥ 3/4"
    );
    assert_eq!(
        said(serde_json::json!({"min": 7, "max": 7}), unit("ms")),
        "healthy at 7 ms"
    );
    // Without a floor, {"max": 0} leaves every negative number.
    assert_eq!(
        said(serde_json::json!({"max": 0}), Terms::default()),
        "healthy ≤ 0"
    );
}

#[test]
fn a_boolean_and_a_level_are_said_as_what_they_accept() {
    assert_eq!(
        said(serde_json::json!({"is": true}), Terms::default()),
        "healthy when yes"
    );
    assert_eq!(
        said(serde_json::json!({"is": false}), Terms::default()),
        "healthy when no"
    );
    let levels: Vec<String> = ["published", "pending", "failed"]
        .map(String::from)
        .to_vec();
    let terms = Terms {
        levels: Some(&levels),
        ..Terms::default()
    };
    assert_eq!(
        said(serde_json::json!({"in": ["published"]}), terms),
        "healthy when published"
    );
    assert_eq!(
        said(serde_json::json!({"at_most": "published"}), terms),
        "healthy when published"
    );
    assert_eq!(
        said(serde_json::json!({"at_most": "pending"}), terms),
        "healthy: published or pending"
    );
    assert_eq!(
        said(serde_json::json!({"in": ["failed", "published"]}), terms),
        "healthy: published or failed"
    );
}

#[test]
fn warn_is_said_as_the_values_it_marks() {
    let warn = |json, terms| super::warn(&condition(json), None, &terms);
    assert_eq!(
        warn(serde_json::json!({"max": 0}), unit("count")),
        "warn above 0"
    );
    assert_eq!(
        warn(serde_json::json!({"min": 90}), unit("%")),
        "warn below 90%"
    );
    assert_eq!(
        warn(serde_json::json!({"min": 40, "max": 60}), unit("ms")),
        "warn outside 40–60 ms"
    );
    let levels: Vec<String> = ["fast", "slow", "stuck"].map(String::from).to_vec();
    let terms = Terms {
        levels: Some(&levels),
        ..Terms::default()
    };
    let healthy = condition(serde_json::json!({"at_most": "slow"}));
    assert_eq!(
        super::warn(
            &condition(serde_json::json!({"in": ["fast"]})),
            Some(&healthy),
            &terms
        ),
        "warn: slow"
    );
}

#[test]
fn together_they_read_as_one_condition() {
    let judge = Judge::Condition(condition(serde_json::json!({"max": 5})));
    let warn = condition(serde_json::json!({"max": 0}));
    assert_eq!(
        super::condition(&judge, Some(&warn), &unit("count")),
        "healthy ≤ 5 · warn above 0"
    );
    assert_eq!(
        super::condition(&Judge::Kpi, Some(&warn), &unit("count")),
        "KPI"
    );
    assert_eq!(
        super::condition(&judge, None, &unit("count")),
        "healthy ≤ 5"
    );
    assert_eq!(
        super::condition(&Judge::SampleOk, None, &Terms::default()),
        "healthy as its sample says"
    );
}

#[test]
fn a_value_is_said_in_its_terms() {
    assert_eq!(value(&Value::Boolean(true), None, None), "yes");
    assert_eq!(value(&Value::Boolean(false), None, None), "no");
    assert_eq!(
        value(&Value::Level("degraded".into()), None, None),
        "degraded"
    );
    assert_eq!(value(&Value::Number(79.2), Some("%"), None), "79.2%");
    assert_eq!(value(&Value::Number(81.0), Some("%"), None), "81.0%");
    assert_eq!(value(&Value::Number(12.0), None, Some(15.0)), "12/15");
    assert_eq!(value(&Value::Number(40.0), Some("ms"), None), "40 ms");
    assert_eq!(value(&Value::Number(3.0), Some("count"), None), "3");
    assert_eq!(number(-0.0), "0");
    assert_eq!(number(82.4), "82.4");
}
