//! Screen freshness follows displayed project snapshots, independently of
//! provider health and reader state (§FS-001-forge-interface.7.1).

use super::*;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::Terminal;
use serde_json::json;
use std::fs;

const WARNING: &str = "Displayed feed outdated: demo — newer cache available; press r to refresh";

fn stored(project: &str, title: &str, fetched: DateTime<Utc>) -> ProjectFeed {
    let item = Item {
        id: format!("fixture:{project}#1"),
        project: project.into(),
        source: "fixture".into(),
        kind: crate::feed::model::ItemKind::Pr,
        role: None,
        title: title.into(),
        url: None,
        state: Some("open".into()),
        needs_response: false,
        updated_at: Utc::now(),
        raw: json!({}),
    };
    let feed = ProjectFeed {
        project: project.into(),
        fetched_at: Some(fetched),
        model: cache::MODEL,
        providers: std::collections::BTreeMap::from([(
            "fixture".into(),
            cache::ProviderSlot {
                fetched_at: Some(fetched),
                ok: true,
                matters: vec![crate::matter::Matter::of_item(&item)],
                ..Default::default()
            },
        )]),
    };
    cache::store_feed(&feed).unwrap();
    feed
}

fn drawn(app: &mut App) -> (String, Buffer) {
    let mut terminal = Terminal::new(TestBackend::new(140, 40)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let text = (0..40)
        .map(|y| {
            (0..140)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, buffer)
}

fn displayed_row(buffer: &Buffer, title: &str) -> Vec<ratatui::buffer::Cell> {
    (0..40)
        .map(|y| (0..140).map(|x| buffer[(x, y)].clone()).collect::<Vec<_>>())
        .find(|row| {
            row.iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .contains(title)
        })
        .expect("the reader's row is drawn")
}

fn due_tick(app: &mut App) -> bool {
    app.ticked_at = std::time::Instant::now() - WORK_TICK - Duration::from_millis(1);
    app.tick()
}

/// The subprocess isolates every cache/job/work path without mutating the
/// parent test process's environment. Returns true in the supervising parent.
fn isolated(name: &str) -> bool {
    if std::env::var_os("EPHOR_244_UNIT_CHILD").is_none() {
        let root = tempfile::Builder::new()
            .prefix("ephor-244-unit-")
            .tempdir()
            .unwrap();
        fs::write(
            root.path().join("registry.json"),
            "{\"project_types\": [], \"hook_sets\": [], \"projects\": []}",
        )
        .unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("EPHOR_")
                || key.to_string_lossy().starts_with("XDG_")
            {
                child.env_remove(key);
            }
        }
        let output = child
            .args(["--exact", name, "--include-ignored", "--nocapture"])
            .env("EPHOR_244_UNIT_CHILD", "1")
            .env("HOME", root.path())
            .env("XDG_STATE_HOME", root.path().join("state"))
            .env("XDG_CONFIG_HOME", root.path().join("config"))
            .env("EPHOR_REGISTRY", root.path().join("registry.json"))
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains("running 1 test"),
            "isolated screen test:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return true;
    }
    false
}

#[test]
#[ignore = "red until #244: the screen warning follows the affected snapshot across ticks and unrelated absorption; implement removes this"]
fn issue_244_warning_tracks_the_displayed_snapshot_and_preserves_reader_state() {
    if isolated("feed::tui::cache_freshness_tests::issue_244_warning_tracks_the_displayed_snapshot_and_preserves_reader_state") {
        return;
    }
    let old_time = Utc::now() - chrono::Duration::minutes(1);
    stored("demo", "OLD cache row", old_time);
    stored("other", "Unrelated row", old_time);
    let seen_path = crate::paths::state_dir().join("seen.json");
    let seen = std::collections::BTreeMap::from([(
        "fixture:already-done".into(),
        cache::Mark::at(old_time),
    )]);
    cache::store_seen(&seen).unwrap();
    let seen_bytes = fs::read(&seen_path).unwrap();
    let config: StatusConfig = serde_json::from_value(json!({
        "projects": {"demo": {"providers": []}, "other": {"providers": []}}
    }))
    .unwrap();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.navigator.handle_key(&app.ctx, KeyCode::Char('j'));
    let (initial, buffer) = drawn(&mut app);
    assert!(!initial.contains("Displayed feed outdated:"));
    let selected_row = displayed_row(&buffer, "OLD cache row");
    assert!(
        selected_row
            .iter()
            .any(|cell| cell.bg == Color::Rgb(60, 60, 80)),
        "fixture must select OLD before observing a cache replacement"
    );
    assert!(
        !due_tick(&mut app),
        "unchanged caches do not request a redraw"
    );

    // Replacement occurs before the first changed-file observation, while
    // the App continues to hold the older displayed snapshot.
    stored(
        "demo",
        "NEW cache row",
        old_time + chrono::Duration::seconds(1),
    );
    let newer_bytes = fs::read(cache::feed_path("demo")).unwrap();
    let changed = due_tick(&mut app);
    let (outdated, buffer) = drawn(&mut app);
    assert!(
        outdated.contains(WARNING),
        "missing outdated-display warning for demo after a newer cache write:\n{outdated}"
    );
    assert!(changed, "the new warning must request a frame");
    assert_eq!(displayed_row(&buffer, "OLD cache row"), selected_row);
    assert!(!outdated.contains("NEW cache row"));
    assert_eq!(
        serde_json::to_value(&app.ctx.seen).unwrap(),
        serde_json::to_value(&seen).unwrap()
    );
    assert_eq!(fs::read(&seen_path).unwrap(), seen_bytes);
    assert_eq!(fs::read(cache::feed_path("demo")).unwrap(), newer_bytes);
    assert!(app
        .ctx
        .feeds
        .iter()
        .flat_map(|feed| feed.providers.values())
        .all(|provider| provider.ok && !provider.stale && provider.error.is_none()));
    assert!(
        !due_tick(&mut app),
        "an unchanged newer cache does not redraw repeatedly"
    );

    app.message = "Unrelated operation finished".into();
    stored(
        "other",
        "Updated unrelated row",
        old_time + chrono::Duration::seconds(2),
    );
    app.absorb("other").unwrap();
    assert!(
        drawn(&mut app).0.contains(WARNING),
        "another project's arrival/message cleared demo's warning"
    );
    // A malformed replacement must not retract already established evidence.
    fs::write(cache::feed_path("demo"), "{broken").unwrap();
    due_tick(&mut app);
    assert!(drawn(&mut app).0.contains(WARNING));
    fs::write(cache::feed_path("demo"), &newer_bytes).unwrap();
    app.absorb("demo").unwrap();
    let current = drawn(&mut app).0;
    assert!(current.contains("NEW cache row") && !current.contains("OLD cache row"));
    assert!(
        !current.contains("Displayed feed outdated:"),
        "caught-up snapshot still warned: {current}"
    );
    assert!(
        !due_tick(&mut app),
        "absorption must advance the observation baseline"
    );
    assert_eq!(fs::read(&seen_path).unwrap(), seen_bytes);

    // Absorption, not just initial load, establishes a displayed baseline.
    stored(
        "demo",
        "NEXT cache row",
        old_time + chrono::Duration::seconds(3),
    );
    due_tick(&mut app);
    assert!(drawn(&mut app).0.contains(WARNING));
    app.reload_feeds().unwrap();
    let current = drawn(&mut app).0;
    assert!(current.contains("NEXT cache row") && !current.contains("Displayed feed outdated:"));
}

/// A deterministic replacement in the initial-read window prevents a later
/// metadata baseline from silently claiming that NEW is already displayed.
#[test]
#[ignore = "red until #244: a cache replaced during the initial read still warns about the older displayed snapshot; implement removes this"]
fn issue_244_cache_replaced_during_initial_read_is_still_outdated() {
    if isolated("feed::tui::cache_freshness_tests::issue_244_cache_replaced_during_initial_read_is_still_outdated") {
        return;
    }
    let old_time = Utc::now() - chrono::Duration::minutes(1);
    stored("demo", "OLD cache row", old_time);
    cache::after_test_feed_read("demo", move || {
        stored(
            "demo",
            "NEW cache row",
            old_time + chrono::Duration::seconds(1),
        );
    });
    let config: StatusConfig = serde_json::from_value(json!({
        "projects": {"demo": {"providers": []}}
    }))
    .unwrap();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    let initial = drawn(&mut app).0;
    assert!(initial.contains("OLD cache row") && !initial.contains("NEW cache row"));
    assert!(
        fs::read_to_string(cache::feed_path("demo"))
            .unwrap()
            .contains("NEW cache row"),
        "fixture must replace the cache during its initial read"
    );
    due_tick(&mut app);
    let current = drawn(&mut app).0;
    assert!(
        current.contains(WARNING),
        "replacement during initial read lost its outdated-display warning:\n{current}"
    );
    assert!(current.contains("OLD cache row") && !current.contains("NEW cache row"));
}
