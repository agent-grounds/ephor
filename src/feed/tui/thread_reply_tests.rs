//! Session/API and screen share recovery facts and actual moves
//! (§FS-007-matters.4, §FS-011-command-line.4, §FS-011-command-line.7).

use super::*;
use crate::{
    api::{
        act::Sending,
        reply::{tests::World, Resolution},
    },
    replies::{Status, Store},
};
use serde_json::{json, Value};
use std::fs;

fn text(screen: &mut ThreadScreen) -> String {
    screen.rebuild_lines(200);
    screen
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn view(world: &World, item: &Item) -> Value {
    let value = serde_json::to_value(world.session.conversation(item).view(item)).unwrap();
    assert!(
        crate::api::schema::holds("thread", &value).is_empty(),
        "{value}"
    );
    value
}

#[test]
fn bound_and_advancing_facts_match_api_schema_render_and_post_refusal() {
    let world = World::new();
    world.draft("€100 acceptance");
    let api = view(&world, &world.item);
    assert_eq!(api["draft"]["thread"], 0);
    assert_eq!(api["draft"]["target"], json!({"in_reply_to":"H"}));
    let mut screen =
        ThreadScreen::open_reading(world.item.clone(), world.session.conversation(&world.item))
            .unwrap();
    assert!(text(&mut screen).contains("Bound thread 0 target: {\"in_reply_to\":\"H\"}"));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('p')),
        Action::PostReply { .. }
    ));
    let mut advanced = world.item.clone();
    advanced.raw["threads"][0]["messages"]
        .as_array_mut()
        .unwrap()
        .push(
            json!({"author":"dana","when":"2026-10-07T11:00:00Z","text":"Now €150","mine":false}),
        );
    let api = view(&world, &advanced);
    assert_eq!(api["draft"]["sendable"], false);
    screen.reread_reading(world.session.conversation(&advanced));
    let rendered = text(&mut screen);
    let reason = api["draft"]["stale_reason"].as_str().unwrap();
    assert!(rendered.contains(reason));
    for fact in ["dana", "2026-10-07T11:00:00Z", "Now €150"] {
        assert!(reason.contains(fact));
    }
    assert!(!screen.footer().contains("p post reply"));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('p')),
        Action::SetMessage(_)
    ));
}

#[test]
fn saved_prepared_words_retry_and_unknown_hold_match_view_keys_and_actual_moves() {
    for held in [false, true] {
        for resolution in [Resolution::Sent, Resolution::NotSent] {
            let world = World::new();
            let path = world.draft("Thanks");
            let store = Store::site(&world.item.id, true).unwrap();
            let mut record = world.record(true);
            if held {
                let intent = record.intent.as_mut().unwrap();
                intent.status = Status::Held;
                intent.note = Some("Explicit unknown: check remote receipt".into());
            }
            store.save(&record).unwrap();
            drop(store);
            fs::write(path, "Edited unsent words").unwrap();
            let api = view(&world, &world.item);
            assert_eq!(api["pending_reply"]["text"], "Thanks");
            assert_eq!(api["pending_reply"]["target"], json!({"in_reply_to":"H"}));
            assert_eq!(api["pending_reply"]["retry"], !held);
            let absent = world.session.reply_item(&world.item.id).unwrap().unwrap();
            assert_eq!(world.session.recovery_rows().unwrap()[0].id, absent.id);
            let mut screen =
                ThreadScreen::open_reading(absent.clone(), world.session.conversation(&absent))
                    .unwrap();
            let rendered = text(&mut screen);
            assert!(
                rendered.contains("Thanks") && rendered.contains("target {\"in_reply_to\":\"H\"}")
            );
            let outcome = if held {
                assert!(rendered.contains("Explicit unknown: check remote receipt"));
                assert!(
                    screen.footer().contains("S resolve sent")
                        && screen.footer().contains("N resolve not-sent")
                );
                assert!(matches!(
                    screen.handle_key(KeyCode::Char('p')),
                    Action::SetMessage(_)
                ));
                let key = if resolution == Resolution::Sent {
                    'S'
                } else {
                    'N'
                };
                match screen.handle_key(KeyCode::Char(key)) {
                    Action::ResolveReply {
                        item,
                        resolution: selected,
                    } => {
                        assert_eq!(selected, resolution);
                        world
                            .session
                            .resolve_reply(&item, None, selected, Sending::Now)
                    }
                    _ => panic!("resolution key failed to reach the shared move"),
                }
            } else {
                assert!(screen.footer().contains("p retry saved send"));
                assert!(!screen.footer().contains("S resolve"));
                match screen.handle_key(KeyCode::Char('p')) {
                    Action::PostReply { item } => world.session.reply(&item, None, Sending::Now),
                    _ => panic!("retry key failed to reach the shared move"),
                }
            };
            assert!(outcome.ok, "{}", outcome.says);
            assert!(
                crate::api::schema::holds("outcome", &serde_json::to_value(&outcome).unwrap())
                    .is_empty()
            );
            assert_eq!(world.calls().len(), usize::from(!held));
            if !held {
                assert_eq!(world.calls()[0]["text"], "Thanks");
            }
            screen.reread_reading(world.session.conversation(&absent));
            assert!(screen.pending_reply.is_none());
            assert!(
                !screen.footer().contains("retry saved") && !screen.footer().contains("S resolve")
            );
        }
    }
}

#[test]
fn corruption_diagnostics_do_not_hide_valid_absent_recovery_or_offer_invalid_moves() {
    let world = World::new();
    world.draft("Thanks");
    let store = Store::site(&world.item.id, true).unwrap();
    store.save(&world.record(false)).unwrap();
    drop(store);
    fs::write(world.tmp.path().join("replies/unrelated.json"), "{broken").unwrap();
    assert_eq!(world.session.recovery_rows().unwrap().len(), 1);
    let api = view(&world, &world.item);
    assert!(api["reply_diagnostics"][0]
        .as_str()
        .unwrap()
        .contains("unrelated.json"));
    let mut screen =
        ThreadScreen::open_reading(world.item.clone(), world.session.conversation(&world.item))
            .unwrap();
    assert!(text(&mut screen).contains("unrelated.json"));
    assert!(screen.footer().contains("S resolve sent"));
    let valid = fs::read_dir(world.tmp.path().join("replies"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension().is_some_and(|ext| ext == "json")
                && path.file_name().unwrap() != "unrelated.json"
        })
        .unwrap();
    fs::write(valid, "{broken").unwrap();
    let api = view(&world, &world.item);
    assert!(api["reply_error"]
        .as_str()
        .unwrap()
        .contains("Invalid saved reply"));
    screen.reread_reading(world.session.conversation(&world.item));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('p')),
        Action::SetMessage(_)
    ));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('S')),
        Action::SetMessage(_)
    ));
    assert!(world.calls().is_empty());
}

#[test]
fn unbound_copy_and_edit_offer_survives_but_post_and_resolutions_refuse() {
    let world = World::new();
    let path = world.draft("Copyable words");
    let mut ledger: Value =
        serde_json::from_slice(&fs::read(world.tmp.path().join("work.json")).unwrap()).unwrap();
    ledger["entries"][&world.item.id]["dispatches"][0]
        .as_object_mut()
        .unwrap()
        .remove("reply_binding");
    fs::write(
        world.tmp.path().join("work.json"),
        serde_json::to_vec(&ledger).unwrap(),
    )
    .unwrap();
    let api = view(&world, &world.item);
    assert_eq!(api["draft"]["sendable"], false);
    let mut screen =
        ThreadScreen::open_reading(world.item.clone(), world.session.conversation(&world.item))
            .unwrap();
    assert!(text(&mut screen).contains("Copyable words"));
    assert!(screen.footer().contains("e edit reply"));
    assert!(!screen.footer().contains("p post reply"));
    match screen.handle_key(KeyCode::Char('e')) {
        Action::EditReply { path: offered, .. } => assert_eq!(offered, path),
        _ => panic!("unbound file lost editing"),
    }
    assert!(matches!(
        screen.handle_key(KeyCode::Char('p')),
        Action::SetMessage(_)
    ));
    fs::write(path, "").unwrap();
    screen.reread_reading(world.session.conversation(&world.item));
    assert!(screen.draft.is_none());
}

#[test]
fn withdrawal_restoration_and_routing_failure_offer_the_same_moves_as_session() {
    let mut world = World::new();
    world.draft("Thanks");
    let store = Store::site(&world.item.id, true).unwrap();
    store.save(&world.record(true)).unwrap();
    drop(store);
    fs::write(world.tmp.path().join("caps.json"), "{\"replies\":true}").unwrap();
    let api = view(&world, &world.item);
    assert_eq!(api["pending_reply"]["status"], "held");
    let mut screen =
        ThreadScreen::open_reading(world.item.clone(), world.session.conversation(&world.item))
            .unwrap();
    assert!(screen.footer().contains("S resolve sent"));
    assert!(!screen.footer().contains("p retry"));
    fs::remove_file(world.tmp.path().join("caps.json")).unwrap();
    screen.reread_reading(world.session.conversation(&world.item));
    assert!(screen.footer().contains("p retry saved send"));
    assert!(!screen.footer().contains("S resolve"));
    fs::write(world.tmp.path().join("fail-cap"), "").unwrap();
    screen.reread_reading(world.session.conversation(&world.item));
    assert!(!screen.footer().contains("p retry") && !screen.footer().contains("S resolve"));
    assert!(text(&mut screen).contains("cannot read declaration"));
    fs::remove_file(world.tmp.path().join("fail-cap")).unwrap();
    world.session.provider_blocks.get_mut("demo").unwrap()[0]["user"] = json!("another-account");
    let api = view(&world, &world.item);
    assert_eq!(api["pending_reply"]["retry"], false);
    assert!(api["pending_reply"]["reason"]
        .as_str()
        .unwrap()
        .contains("Original source"));
    screen.reread_reading(world.session.conversation(&world.item));
    assert!(!screen.footer().contains("p retry") && !screen.footer().contains("S resolve"));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('p')),
        Action::SetMessage(_)
    ));
    assert!(world.calls().is_empty());
}
