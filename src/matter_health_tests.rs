//! A monitor's matter moves on its health and not its number
//! (§FS-021-monitors.13.1), and says why in its own sentences wherever
//! movement is read: the resurfaced row and the stale work
//! (§FS-021-monitors.13.2, §FS-005-dispatch.5).

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset, Utc};

use crate::feed::model::Item;
use crate::matter::Matter;
use crate::monitors::condition::Judge;
use crate::monitors::matters;
use crate::monitors::model::Value;
use crate::monitors::state::{Document, OpenDrop, Record, Shown};
use crate::work::ledger::Snapshot;

fn at(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .unwrap()
        .with_timezone(&Utc)
}

fn release(level: &str, worst: &str, words: &str) -> Item {
    let mut record = Record::new(at("2026-11-20T06:00:00Z"));
    record.drop = Some(OpenDrop {
        named: at("2026-11-25T06:00:00Z"),
        moved: None,
        worst: Some(worst.into()),
        judged_under: Judge::SampleOk,
        judged_words: words.into(),
        works_ended: Vec::new(),
        renewed: None,
        shown: Shown {
            title: "Release".into(),
            value: Some(Value::Level(level.into())),
            condition: words.into(),
            ..Shown::default()
        },
    });
    let mut document = Document::default();
    document.monitors.insert("release".into(), record);
    let zone = FixedOffset::east_opt(0).unwrap();
    let now = at("2026-11-25T07:00:00Z");
    matters::items("widget", &document, &BTreeMap::new(), now, 7, &zone).remove(0)
}

#[test]
fn a_value_moving_inside_its_band_is_not_movement() {
    let was = release("degraded", "degraded", "healthy when ok");
    let now = release("degraded, slower", "degraded", "healthy when ok");
    assert_ne!(was.title, now.title);
    let (before, after) = (Matter::of_item(&was), Matter::of_item(&now));
    assert_eq!(after.fingerprint.resurfacing(&before.fingerprint), None);
    assert!(Snapshot::of(&was).changes(&Snapshot::of(&now)).is_empty());
}

#[test]
fn a_worse_level_and_a_changed_condition_say_why() {
    let was = release("degraded", "degraded", "healthy when ok");
    let now = release("failed", "failed", "healthy when green");
    let (before, after) = (Matter::of_item(&was), Matter::of_item(&now));
    assert_eq!(
        after.fingerprint.resurfacing(&before.fingerprint).as_deref(),
        Some("⟳ level worse: degraded → failed, condition changed: healthy when ok → healthy when green")
    );
    assert_eq!(
        Snapshot::of(&was).changes(&Snapshot::of(&now)),
        vec![
            "level worse: degraded → failed",
            "condition changed: healthy when ok → healthy when green"
        ]
    );
}

/// Ledgers and caches written before a print carried health read as before,
/// and every other matter's print carries none.
#[test]
fn prints_without_health_read_as_before() {
    let old: Snapshot = serde_json::from_value(serde_json::json!({
        "updated_at": "2026-11-25T06:00:00Z", "state": "down"
    }))
    .unwrap();
    assert_eq!(old.health, None);
    let now = Snapshot::of(&release("failed", "failed", "healthy when ok"));
    assert!(old.changes(&now).is_empty());
    let mut other = release("failed", "failed", "healthy when ok");
    other.source = "custom-status".into();
    let json = serde_json::to_value(Snapshot::of(&other)).unwrap();
    assert!(json.get("health").is_none(), "{json}");
    let print = serde_json::to_value(Matter::of_item(&other).fingerprint).unwrap();
    assert!(print.get("health").is_none(), "{print}");
}
