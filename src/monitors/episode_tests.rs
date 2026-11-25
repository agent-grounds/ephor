use super::*;
use crate::monitors::test_zone::{utc, Shifting};

#[test]
fn an_episode_is_named_to_the_second_and_never_goes_back() {
    let at = utc("2026-11-25T06:00:00.750Z");
    assert_eq!(name(at, None), utc("2026-11-25T06:00:00Z"));
    assert_eq!(
        name(at, Some(utc("2026-11-25T05:00:00Z"))),
        utc("2026-11-25T06:00:00Z")
    );
    assert_eq!(
        name(at, Some(utc("2026-11-25T06:00:00Z"))),
        utc("2026-11-25T06:00:01Z")
    );
    assert_eq!(
        name(at, Some(utc("2026-11-25T07:00:00Z"))),
        utc("2026-11-25T07:00:01Z")
    );
}

#[test]
fn a_key_carries_no_colon_after_its_source() {
    let named = utc("2026-11-25T06:00:00Z");
    assert_eq!(id(named), "20261125T060000Z");
    assert_eq!(
        key("acme", "coverage", Sort::Drop, named),
        "monitor:acme/coverage@20261125T060000Z"
    );
    assert_eq!(
        key("acme", "coverage", Sort::Unknown, named),
        "monitor:acme/coverage.unknown@20261125T060000Z"
    );
    let key = key("acme", "coverage", Sort::Unknown, named);
    assert_eq!(key.rsplit_once(':').unwrap().0, "monitor");
}

#[test]
fn an_ending_says_its_time_on_the_local_clock() {
    let at = utc("2026-11-25T05:02:00Z");
    assert_eq!(
        Ending::Recovered {
            value: "81.0%".into()
        }
        .say(at, &Shifting),
        "recovered at Nov 25 06:02 (81.0%)"
    );
    assert_eq!(
        Ending::ReadAgain {
            value: "yes".into()
        }
        .say(at, &Utc),
        "read again at Nov 25 05:02 (yes)"
    );
    assert_eq!(
        Ending::Unwatched {
            why: Unwatched::Kpi
        }
        .say(at, &Utc),
        "no longer watched at Nov 25 05:02: nothing judges it any more"
    );
    for why in [
        Unwatched::Gone,
        Unwatched::KindChanged,
        Unwatched::InError,
        Unwatched::NotRun,
    ] {
        assert!(!why.reason().is_empty());
    }
}

#[test]
fn a_movement_names_its_reason() {
    assert_eq!(
        Movement::LevelWorse {
            from: "degraded".into(),
            to: "failed".into()
        }
        .say(),
        "level worse: degraded → failed"
    );
    assert_eq!(
        Movement::ConditionChanged {
            from: "healthy ≥ 80%".into(),
            to: "healthy ≥ 85%".into()
        }
        .say(),
        "condition changed: healthy ≥ 80% → healthy ≥ 85%"
    );
    assert_eq!(Movement::StillDown.say(), "still down after its work ended");
    assert_eq!(
        Movement::StillUnknown {
            cause: "unreadable".into()
        }
        .say(),
        "still unreadable after its work ended"
    );
    assert!(Movement::StillDown.renews());
    assert!(!Movement::LevelWorse {
        from: String::new(),
        to: String::new()
    }
    .renews());
}

#[test]
fn an_ending_keeps_its_shape_in_the_store() {
    let ending = Ending::Unwatched {
        why: Unwatched::KindChanged,
    };
    let json = serde_json::to_value(&ending).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"how": "unwatched", "why": "kind_changed"})
    );
    assert_eq!(serde_json::from_value::<Ending>(json).unwrap(), ending);
}
