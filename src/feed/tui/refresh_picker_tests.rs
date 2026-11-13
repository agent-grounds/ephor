//! `R` on the feed screen asks what `ephor refresh --source/--only` asks
//! (§FS-011-command-line.9.2, §REQ-002-parity.1).

use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::feed::cache::ProviderSlot;
use crate::feed::history::History;

fn config() -> StatusConfig {
    serde_json::from_value(json!({
        "sources": [{ "provider": "gmail" }, { "provider": "gdev", "block": "site" }],
        "projects": {
            "graal": { "providers": [
                { "provider": "gdev", "block": "a" },
                { "provider": "gdev", "block": "b" },
                { "provider": "acme" },
                { "provider": "github-prs", "repos": ["acme/app"] }
            ] },
            "other": { "providers": [] }
        }
    }))
    .unwrap()
}

/// graal's cache: gdev declared pull requests and issues.
fn feeds() -> Vec<ProjectFeed> {
    let history = |capability: &str| History {
        capability: Some(capability.to_string()),
        ..History::default()
    };
    vec![ProjectFeed {
        project: "graal".to_string(),
        providers: BTreeMap::from([(
            "gdev".to_string(),
            ProviderSlot {
                histories: vec![history("pull-requests"), history("issues")],
                ..ProviderSlot::default()
            },
        )]),
        ..ProjectFeed::default()
    }]
}

fn picker(over: &[&str]) -> RefreshPicker {
    let over: Vec<String> = over.iter().map(|project| project.to_string()).collect();
    RefreshPicker::over(&config(), &feeds(), &over).expect("sources are bound here")
}

fn listed(picker: &RefreshPicker) -> Vec<(&str, Vec<&str>)> {
    picker
        .sources
        .iter()
        .map(|source| (source.name.as_str(), source.capabilities.clone()))
        .collect()
}

/// The sources are the projects' own bindings, then the site's; each offers
/// one capability at a time only where the command line would take `--only`
/// for it: a forge reached out of process, bound by every project in view,
/// with the capabilities its cache says it declared — or all five where no
/// refresh recorded any.
#[test]
fn the_picker_offers_what_the_command_line_would_take() {
    assert_eq!(
        listed(&picker(&["graal"])),
        [
            ("gdev", vec!["pull-requests", "issues"]),
            ("acme", FETCHES.to_vec()),
            ("github-prs", vec![]),
            ("gmail", vec![]),
        ]
    );
    // Over both projects, gdev reaches `other` through the site's binding,
    // where `--only` is refused: only its whole answer is offered.
    let both = picker(&["graal", "other"]);
    assert_eq!(listed(&both)[0], ("gdev", vec![]));
    // Over a project that binds nothing, only the site's sources, by name.
    let bare = picker(&["other"]);
    let names: Vec<&str> = listed(&bare).iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["gdev", "gmail"]);
}

/// A source, then its whole answer or one capability: the choice is the
/// request `--source NAME [--only CAP]` makes.
#[test]
fn choosing_a_source_then_an_answer_is_the_narrowed_request() {
    let mut picker = picker(&["graal"]);
    assert!(matches!(
        picker.handle_key(KeyCode::Enter),
        PickerOutcome::Stay
    ));
    // Down to `issues`: whole answer, pull-requests, issues.
    picker.handle_key(KeyCode::Char('j'));
    picker.handle_key(KeyCode::Char('j'));
    match picker.handle_key(KeyCode::Enter) {
        PickerOutcome::Refresh(narrowed) => assert_eq!(
            narrowed,
            Narrowed {
                source: "gdev".to_string(),
                only: Some("issues"),
            }
        ),
        _ => panic!("a capability chosen refreshes it"),
    }

    // Back out, to acme, and its whole answer.
    let mut picker = super::tests::picker(&["graal"]);
    picker.handle_key(KeyCode::Char('j'));
    picker.handle_key(KeyCode::Enter);
    picker.handle_key(KeyCode::Char('h'));
    assert!(picker.answer.is_none(), "h goes back to the sources");
    picker.handle_key(KeyCode::Enter);
    match picker.handle_key(KeyCode::Enter) {
        PickerOutcome::Refresh(narrowed) => {
            assert_eq!(narrowed.flags(), "--source acme");
        }
        _ => panic!("the whole answer refreshes the source"),
    }

    // The cursor stays on the list, and Esc closes.
    let mut picker = super::tests::picker(&["graal"]);
    for _ in 0..10 {
        picker.handle_key(KeyCode::Char('j'));
    }
    assert_eq!(picker.selected, 3);
    assert!(matches!(
        picker.handle_key(KeyCode::Esc),
        PickerOutcome::Close
    ));
}

/// The narrowed run names itself in the flags `ephor refresh` takes, so the
/// line it leaves says what the command line would.
#[test]
fn a_narrowed_request_reads_as_its_flags() {
    let narrowed = Narrowed {
        source: "gdev".to_string(),
        only: Some("issues"),
    };
    assert_eq!(narrowed.flags(), "--source gdev --only issues");
}

/// `R` on the feed screen opens the picker; `r` is still the whole refresh.
#[test]
fn r_refreshes_everything_and_capital_r_picks() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = super::super::tests::ctx_with_branch(tmp.path(), None);
    let mut navigator = super::super::NavigatorState::new();
    assert!(matches!(
        navigator.handle_key(&ctx, KeyCode::Char('r')),
        super::super::Action::Refresh
    ));
    assert!(matches!(
        navigator.handle_key(&ctx, KeyCode::Char('R')),
        super::super::Action::PickRefresh
    ));
}

/// The screen's run is the command line's request: it resolves the same way
/// and refuses the same choices, before any data call, and a refused run
/// writes nothing and says why (§FS-011-command-line.9.2).
#[test]
fn a_refused_choice_is_refused_before_anything_is_asked() {
    if super::super::cache_freshness_tests::isolated(
        "feed::tui::refresh_picker::tests::a_refused_choice_is_refused_before_anything_is_asked",
    ) {
        return;
    }
    let config: StatusConfig = serde_json::from_value(json!({
        "sources": [{ "provider": "gmail" }],
        "projects": { "demo": { "providers": [] } }
    }))
    .unwrap();
    let over = vec!["demo".to_string()];
    for (narrowed, says) in [
        (
            Narrowed {
                source: "nosuch".to_string(),
                only: None,
            },
            "nosuch",
        ),
        (
            Narrowed {
                source: "gmail".to_string(),
                only: Some("issues"),
            },
            "site",
        ),
    ] {
        let mut run =
            crate::feed::refresh::BackgroundRefresh::start(&config, &over, Some(narrowed.clone()))
                .unwrap();
        let started = std::time::Instant::now();
        while !run.done() {
            run.collect();
            assert!(started.elapsed() < std::time::Duration::from_secs(10));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        run.finish();
        let summary = run.summary();
        assert!(
            summary.starts_with("Not refreshed — ") && summary.contains(says),
            "{narrowed:?}: {summary}"
        );
        assert!(
            !crate::feed::cache::feed_path("demo").exists(),
            "a refused run wrote the cache"
        );
    }
}
