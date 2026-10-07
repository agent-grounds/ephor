//! Passive collection errors are visible beside valid saved absent-feed rows
//! (§FS-005-dispatch.13, §FS-011-command-line.4).

use super::*;
use crate::{
    api::{reply::tests::World, OrgInfo},
    feed::cache::ProjectFeed,
    replies::Store,
};
use std::fs;

#[test]
fn valid_absent_recovery_and_corrupt_diagnostic_survive_all_navigator_modes_and_api_counts() {
    let mut world = World::new();
    let store = Store::site(&world.item.id, true).unwrap();
    store.save(&world.record(false)).unwrap();
    drop(store);
    fs::write(world.tmp.path().join("replies/unrelated.json"), "{broken").unwrap();
    world.session.projects = vec!["demo".into()];
    world.session.orgs = vec![OrgInfo {
        id: "root".into(),
        name: "Root".into(),
        root: None,
    }];
    world
        .session
        .project_org
        .insert("demo".into(), "root".into());
    world.session.feeds = vec![ProjectFeed {
        project: "demo".into(),
        ..Default::default()
    }];
    world.session.recompute_stats();
    assert_eq!(world.session.stats["demo"], (1, 1, 1));
    assert_eq!(world.session.items()[0].id, world.item.id);
    let mut navigator = NavigatorState::new();
    navigator.rebuild(&world.session);
    assert!(navigator
        .stream_entries
        .iter()
        .any(|entry| matches!(entry, Entry::Item(row) if row.item.id == world.item.id)));
    assert!(navigator
        .stream_entries
        .iter()
        .any(|entry| matches!(entry, Entry::Org(text) if text.contains("unrelated.json"))));
    assert!(navigator
        .project_entries
        .iter()
        .any(|entry| matches!(entry, Entry::Org(text) if text.contains("unrelated.json"))));
    navigator.mode = Mode::Detail;
    navigator.detail_project = 0;
    // Enter/drill-in also calls this direct path, outside a full rebuild.
    navigator.rebuild_detail(&world.session);
    assert!(navigator
        .detail_entries
        .iter()
        .any(|entry| matches!(entry, Entry::Item(row) if row.item.id == world.item.id)));
    assert!(navigator
        .detail_entries
        .iter()
        .any(|entry| matches!(entry, Entry::Org(text) if text.contains("unrelated.json"))));
}
