//! The person's own words and a new draft, from the thread screen: `r` types
//! a reply, `e` on a stale draft types one from its words, the status line asks
//! once and `y` sends through the move `ephor reply ID WORDS` makes, and `n`
//! asks for a new draft or says why none is offered (§FS-005-dispatch.13.2,
//! §REQ-002-parity.2).

use super::*;
use crate::api::act::Sending;
use crate::api::reply::tests::World;
use crate::api::views::Redraft;
use serde_json::{json, Value};
use std::fs;

const WORDS: &str = "It is €150 now, and Tuesday works.";

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

/// The stale screen, as it reads where the recipe that laid the draft still
/// applies.
fn offered(world: &World, row: &Item) -> ThreadScreen {
    let mut reading = world.session.conversation(row);
    let draft = reading.draft.as_mut().expect("a draft is waiting");
    draft.redraft_refused = None;
    draft.redraft = Some(Redraft {
        recipe: "answer".into(),
        command: format!(
            "ephor work dispatch --item {} --recipe answer --again",
            row.id
        ),
    });
    ThreadScreen::open_reading(row.clone(), reading).unwrap()
}

fn carries(value: &Value, words: &str) -> bool {
    match value {
        Value::String(text) => text == words,
        Value::Array(values) => values.iter().any(|value| carries(value, words)),
        Value::Object(values) => values.values().any(|value| carries(value, words)),
        _ => false,
    }
}

#[test]
fn r_types_asks_once_naming_the_thread_and_target_and_y_sends_through_the_move() {
    let world = World::new();
    world.draft("€100 accepted");
    let now = edited(&world);
    let mut screen = screen(&world, &now);
    match screen.handle_key(KeyCode::Char('r')) {
        Action::TypeReply { item, start } => {
            assert_eq!(item.id, now.id);
            assert!(start.is_empty(), "r starts from nothing: {start:?}");
        }
        _ => panic!("r starts a typed reply"),
    }
    let path = world.tmp.path().join("typed.md");
    let question = screen.typed(format!("{WORDS}\n"), path.clone());
    assert!(
        question.contains("thread 0") && question.contains("in_reply_to"),
        "the question names the thread and the target: {question}"
    );
    assert!(screen.is_picking(), "the question holds the keyboard");
    assert!(screen.footer().contains("y send"), "{}", screen.footer());
    let (item, words) = match screen.handle_key(KeyCode::Char('y')) {
        Action::SendTyped {
            item,
            words,
            path: typed,
        } => {
            assert_eq!(typed, path);
            (item, words)
        }
        _ => panic!("y sends the typed words"),
    };
    assert!(world.calls().is_empty(), "the screen sends nothing itself");
    assert!(!screen.is_picking(), "asked once");
    let outcome = world.session.reply(&item, Some(&words), Sending::Now);
    assert!(outcome.ok, "{}", outcome.says);
    let calls = world.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(
        carries(&calls[0], WORDS),
        "the trimmed words went: {calls:?}"
    );
}

#[test]
fn any_other_answer_keeps_the_words_unsent_and_nothing_typed_asks_nothing() {
    let world = World::new();
    world.draft("€100 accepted");
    let mut screen = screen(&world, &world.item);
    let path = world.tmp.path().join("typed.md");
    screen.typed(WORDS.into(), path.clone());
    match screen.handle_key(KeyCode::Char('n')) {
        Action::SetMessage(says) => assert!(says.contains("typed.md"), "{says}"),
        _ => panic!("any other key keeps the words where they were typed"),
    }
    assert!(!screen.is_picking());
    assert_eq!(
        screen.typed("  \n".into(), path),
        "Nothing typed — nothing sent"
    );
    assert!(
        !screen.is_picking(),
        "nothing typed is nothing to ask about"
    );
    assert!(world.calls().is_empty());
}

#[test]
fn e_on_a_stale_draft_starts_from_its_words_and_leaves_the_file() {
    let world = World::new();
    let path = world.draft("€100 accepted");
    let now = edited(&world);
    let mut screen = screen(&world, &now);
    assert!(
        screen.footer().contains("e reply from draft"),
        "{}",
        screen.footer()
    );
    match screen.handle_key(KeyCode::Char('e')) {
        Action::TypeReply { start, .. } => assert_eq!(start, "€100 accepted"),
        _ => panic!("e on a stale draft types a reply from its words"),
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), "€100 accepted");
}

#[test]
fn n_asks_for_the_new_draft_where_one_is_offered_and_says_why_where_not() {
    let world = World::new();
    world.draft("€100 accepted");
    let now = edited(&world);

    let mut refused = screen(&world, &now);
    let why = world
        .session
        .conversation(&now)
        .draft
        .unwrap()
        .redraft_refused;
    let why = why.expect("a stale draft with no new draft says why");
    assert!(why.starts_with("No new draft is offered"), "{why}");
    assert!(!refused.footer().contains("n new draft"));
    assert!(text(&mut refused).contains(&why));
    match refused.handle_key(KeyCode::Char('n')) {
        Action::SetMessage(says) => assert_eq!(says, why),
        _ => panic!("n says why no new draft is offered"),
    }

    let mut screen = offered(&world, &now);
    assert!(
        screen.footer().contains("n new draft"),
        "{}",
        screen.footer()
    );
    let card = text(&mut screen);
    assert!(
        card.contains("ephor work dispatch --item mail:k-B --recipe answer --again"),
        "{card}"
    );
    match screen.handle_key(KeyCode::Char('n')) {
        Action::DispatchWork { item, entry } => {
            assert_eq!(item.id, now.id);
            assert_eq!(entry, "answer", "the recipe that laid the draft");
        }
        _ => panic!("n hands the matter over again"),
    }
    assert!(world.calls().is_empty());
}

#[test]
fn n_on_a_current_draft_and_r_where_nothing_can_carry_a_reply_refuse() {
    let world = World::new();
    world.draft("€100 accepted");
    let mut current = screen(&world, &world.item);
    assert!(!current.footer().contains("n new draft"));
    assert!(matches!(
        current.handle_key(KeyCode::Char('n')),
        Action::SetMessage(_)
    ));
    let mut copy_only = world.item.clone();
    copy_only.raw["threads"][0]["reply"] = Value::Null;
    let mut screen = screen(&world, &copy_only);
    assert!(!screen.footer().contains("  r "), "{}", screen.footer());
    assert!(matches!(
        screen.handle_key(KeyCode::Char('r')),
        Action::SetMessage(_)
    ));
}
