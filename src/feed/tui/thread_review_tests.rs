//! A stale draft's review and its ways on, on the thread screen as on the API
//! it renders (§FS-005-dispatch.13.2, §FS-011-command-line.4).

use super::*;
use crate::api::reply::tests::World;
use serde_json::json;
use std::fs;

const STALE: &str = "Draft is stale: dana edited [0] after it was drafted";

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

/// The world's row after dana edited the message the draft answers.
fn edited(world: &World) -> Item {
    let mut row = world.item.clone();
    row.raw["threads"][0]["messages"][0]["text"] = json!("€150?");
    row
}

fn screen(world: &World, row: &Item) -> ThreadScreen {
    ThreadScreen::open_reading(row.clone(), world.session.conversation(row)).unwrap()
}

#[test]
fn the_card_shows_the_review_the_api_carries() {
    let world = World::new();
    world.draft("€100 accepted");
    let now = edited(&world);
    let api = serde_json::to_value(world.session.conversation(&now).view(&now)).unwrap();
    assert!(
        crate::api::schema::holds("thread", &api).is_empty(),
        "{api}"
    );
    let since = api["draft"]["since"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(since.len(), 1, "one edit: {api}");
    assert_eq!(since[0]["change"], "edited");
    assert_eq!(since[0]["message"], 0);
    assert_eq!(since[0]["before"], "€100?");
    assert_eq!(since[0]["text"], "€150?");
    assert_eq!(api["draft"]["stale_reason"], STALE);
    let card = text(&mut screen(&world, &now));
    assert!(card.contains("Since the draft"), "{card}");
    assert!(card.contains("dana edited [0]"), "{card}");
    assert!(
        card.contains("- €100?") && card.contains("+ €150?"),
        "{card}"
    );
}

#[test]
fn on_a_stale_draft_p_refuses_e_and_r_type_and_n_asks_again() {
    let world = World::new();
    let path = world.draft("€100 accepted");
    let now = edited(&world);
    let mut screen = screen(&world, &now);
    let footer = screen.footer();
    assert!(!footer.contains("p post reply"), "{footer}");
    assert!(footer.contains("  r "), "r is taught: {footer}");
    match screen.handle_key(KeyCode::Char('p')) {
        Action::SetMessage(reason) => assert_eq!(reason, STALE),
        _ => panic!("p on a stale draft refuses with the stale reason"),
    }
    let words = fs::read_to_string(&path).unwrap();
    let e = screen.handle_key(KeyCode::Char('e'));
    assert!(
        !matches!(&e, Action::EditReply { path: opened, .. } if opened == &path),
        "e on a stale draft starts a typed reply rather than opening the draft file"
    );
    assert!(
        !matches!(e, Action::None | Action::SetMessage(_)),
        "e on a stale draft starts a typed reply from its words"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        words,
        "the draft file stays"
    );
    assert!(
        !matches!(
            screen.handle_key(KeyCode::Char('r')),
            Action::None | Action::SetMessage(_)
        ),
        "r starts a typed reply"
    );
    assert!(
        !matches!(screen.handle_key(KeyCode::Char('n')), Action::None),
        "n asks for a new draft, or says why none is offered"
    );
    assert!(world.calls().is_empty(), "no key here sends anything");
}

#[test]
fn on_a_fresh_draft_e_and_p_are_unchanged_and_r_types_a_reply() {
    let world = World::new();
    let path = world.draft("€100 accepted");
    let mut screen = screen(&world, &world.item);
    assert!(matches!(
        screen.handle_key(KeyCode::Char('p')),
        Action::PostReply { .. }
    ));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('e')),
        Action::EditReply { path: opened, .. } if opened == path
    ));
    assert!(screen.footer().contains("  r "), "{}", screen.footer());
    assert!(
        !matches!(
            screen.handle_key(KeyCode::Char('r')),
            Action::None | Action::SetMessage(_)
        ),
        "r starts a typed reply"
    );
    assert!(world.calls().is_empty());
}
