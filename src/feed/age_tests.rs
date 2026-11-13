//! A project's age, judged from its bindings (§FS-011-command-line.12).

use std::collections::BTreeMap;

use chrono::TimeDelta;

use super::*;
use crate::feed::cache::ProviderSlot;

fn at(minutes: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000, 0).unwrap() + TimeDelta::minutes(minutes)
}

/// A history asked at `asked`, answered at `answered`.
fn history(capability: Option<&str>, asked: Option<i64>, answered: Option<i64>) -> History {
    History {
        capability: capability.map(String::from),
        asked_at: asked.map(at),
        answered_at: answered.map(at),
        ..History::default()
    }
}

fn slot(histories: Vec<History>) -> ProviderSlot {
    ProviderSlot {
        histories,
        ..ProviderSlot::default()
    }
}

fn feed(fetched: Option<i64>, slots: Vec<(&str, ProviderSlot)>) -> ProjectFeed {
    ProjectFeed {
        project: "graal".to_string(),
        fetched_at: fetched.map(at),
        model: crate::feed::cache::MODEL,
        revision: 1,
        providers: slots
            .into_iter()
            .map(|(name, slot)| (name.to_string(), slot))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// gdev's pull requests asked at minute 0, its issues at minute 10.
fn gdev() -> ProviderSlot {
    slot(vec![
        history(Some("pull-requests"), Some(0), Some(0)),
        history(Some("issues"), Some(10), Some(10)),
    ])
}

/// The oldest asking over the project's own histories is its age: a refresh
/// that asked only issues leaves the project as old as its pull requests.
#[test]
fn a_project_is_as_old_as_its_oldest_history() {
    let cache = feed(Some(10), vec![("gdev", gdev())]);
    assert_eq!(age(Some(&cache), &["gdev"]), Some(at(0)));
}

/// A source that keeps failing is still stamped asked by every refresh, so it
/// ages like any other and never holds the project past its TTL; the failure
/// is the slot's to say.
#[test]
fn a_failing_source_ages_by_when_it_was_asked() {
    let failing = slot(vec![History {
        error: Some("acme: HTTP 503".to_string()),
        stale: true,
        ..history(None, Some(10), Some(0))
    }]);
    let cache = feed(Some(10), vec![("acme", failing)]);
    assert_eq!(age(Some(&cache), &["acme"]), Some(at(10)));
    assert!(!never_asked(Some(&cache), &["acme"]));
}

/// A capability the source did not declare has no history, and does not age
/// the project: only what each bound source declared counts.
#[test]
fn an_undeclared_capability_does_not_count() {
    // gdev declared pull requests and issues and nothing else.
    let cache = feed(Some(10), vec![("gdev", gdev())]);
    let own = own(Some(&cache), &["gdev"]);
    let capabilities: Vec<_> = own
        .iter()
        .map(|own| own.history.and_then(|h| h.capability.as_deref()))
        .collect();
    assert_eq!(capabilities, [Some("pull-requests"), Some("issues")]);
    assert!(!never_asked(Some(&cache), &["gdev"]));
}

/// A history never asked makes the project read never fetched, however fresh
/// the rest of it is.
#[test]
fn a_history_never_asked_reads_never_fetched() {
    let mut gdev = gdev();
    gdev.histories.push(history(Some("notices"), None, None));
    let cache = feed(Some(10), vec![("gdev", gdev)]);
    assert_eq!(age(Some(&cache), &["gdev"]), None);
    assert!(never_asked(Some(&cache), &["gdev"]));
}

/// "Never asked" is judged from the bindings: a bound source the cache holds
/// no history for has not been asked, which is what a cache rebuilt by a model
/// change and refreshed for one source looks like.
#[test]
fn a_bound_source_with_no_history_reads_never_asked() {
    let cache = feed(Some(10), vec![("gdev", gdev())]);
    assert_eq!(age(Some(&cache), &["gdev", "acme"]), None);
    assert!(never_asked(Some(&cache), &["gdev", "acme"]));
    let unasked: Vec<&str> = own(Some(&cache), &["gdev", "acme"])
        .iter()
        .filter(|own| own.history.is_none())
        .map(|own| own.source)
        .collect();
    assert_eq!(unasked, ["acme"]);
    // No cache at all is every bound source never asked.
    assert_eq!(age(None, &["gdev"]), None);
    assert!(never_asked(None, &["gdev"]));
}

/// A site source's history is not the project's: `status` refetches only the
/// project's own sources, so an old site history must not age it. A task
/// store, found rather than bound, is the project's own.
#[test]
fn site_histories_do_not_count_and_task_stores_do() {
    let site = slot(vec![history(None, Some(-600), Some(-600))]);
    let store = slot(vec![history(None, Some(5), Some(5))]);
    let cache = feed(
        Some(10),
        vec![("gdev", gdev()), ("gmail", site), ("rhei", store)],
    );
    assert_eq!(age(Some(&cache), &["gdev"]), Some(at(0)));
    let sources: Vec<&str> = own(Some(&cache), &["gdev"])
        .iter()
        .map(|own| own.source)
        .collect();
    assert_eq!(sources, ["gdev", "gdev", "rhei"]);

    let older = slot(vec![history(None, Some(-5), Some(-5))]);
    let cache = feed(Some(10), vec![("gdev", gdev()), ("rhei", older)]);
    assert_eq!(age(Some(&cache), &["gdev"]), Some(at(-5)));
}

/// A project whose refresh asks nothing of its own — no source, no task
/// store — takes its age from the last refresh run that asked it, so it does
/// not read never fetched forever.
#[test]
fn a_project_with_no_histories_takes_its_refresh_stamp() {
    let site = slot(vec![history(None, Some(-600), Some(-600))]);
    let cache = feed(Some(10), vec![("gmail", site)]);
    assert_eq!(age(Some(&cache), &[]), Some(at(10)));
    assert!(!never_asked(Some(&cache), &[]));
    assert_eq!(age(Some(&feed(None, vec![])), &[]), None);
    assert_eq!(age(None, &[]), None);
}

/// The TTL is judged on the age, by the one rule.
#[test]
fn younger_than_reads_the_age() {
    let now = Utc::now();
    let recent = |minutes: i64| History {
        asked_at: Some(now - TimeDelta::minutes(minutes)),
        answered_at: Some(now - TimeDelta::minutes(minutes)),
        ..History::default()
    };
    let cache = ProjectFeed {
        fetched_at: Some(now),
        providers: BTreeMap::from([
            ("gdev".to_string(), slot(vec![recent(1)])),
            ("acme".to_string(), slot(vec![recent(30)])),
        ]),
        ..ProjectFeed::default()
    };
    assert!(younger_than(Some(&cache), &["gdev"], 600));
    assert!(!younger_than(Some(&cache), &["gdev", "acme"], 600));
    assert!(!younger_than(None, &[], 600));
}

/// The bindings are the project's own blocks' sources, each once.
#[test]
fn bound_names_each_source_once() {
    let config: ProjectFeedConfig = serde_json::from_value(serde_json::json!({
        "providers": [
            { "provider": "gdev", "block": "a" },
            { "provider": "gdev", "block": "b" },
            { "provider": "acme" }
        ]
    }))
    .unwrap();
    assert_eq!(bound(&config), ["gdev", "acme"]);
}
