//! The interface opens on the Waiting screen, and the overview is one key away
//! (§FS-019-waiting.4, §FS-019-waiting.5), driven through the shell itself in
//! an isolated home.

use super::cache_freshness_tests::{drawn, isolated, stored};
use super::*;
use serde_json::json;

fn review_screen(item: Item) -> Screen {
    Screen::HumanGate(HumanGateScreen::new(
        item,
        crate::work::runtime::human_gate::Review::default(),
    ))
}

/// `esc` on the review that is open, as the event loop sends it: the key goes
/// to the screen, and the action it answers is applied the way `apply`
/// applies `Back` and `OpenWork` (§FS-019-waiting.4).
fn esc_on_review(app: &mut App) -> Action {
    let Screen::HumanGate(review) = &mut app.screen else {
        panic!("a review is open");
    };
    assert!(
        review.footer().contains(match app.home {
            Home::Waiting => "esc waiting",
            Home::Overview => "esc overview",
        }),
        "{}",
        review.footer()
    );
    let action = review.handle_key(KeyCode::Esc);
    if matches!(action, Action::Back) {
        app.back();
    }
    action
}

#[test]
fn the_interface_opens_on_waiting_and_the_overview_is_one_key_away() {
    if isolated(
        "feed::tui::waiting_app_tests::the_interface_opens_on_waiting_and_the_overview_is_one_key_away",
    ) {
        return;
    }
    stored("demo", "Durations read as seconds", Utc::now());
    let config: StatusConfig =
        serde_json::from_value(json!({ "projects": { "demo": { "providers": [] } } })).unwrap();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.go_home_to(start_home(
        &config.defaults,
        &crate::cli::TuiArgs::default(),
    ));
    app.freshen_waiting();

    // The first screen: nothing on the reader, the one matter new, and the
    // overview line saying how much the feed holds (§FS-019-waiting.3).
    let (text, _) = drawn(&mut app);
    assert!(
        text.contains("ephor — waiting   0 on you · 1 new · 0 running"),
        "{text}"
    );
    assert!(text.contains("Durations read as seconds"), "{text}");
    assert!(
        text.contains("overview: 1 matter · 1 changed since you looked"),
        "{text}"
    );
    let item = app.ctx.items().remove(0);

    // Esc on the floor does nothing.
    assert!(matches!(app.waiting.handle_key(KeyCode::Esc), Action::None));

    // A gate review opened from Waiting comes back to Waiting on `esc`, by
    // the key itself, and the board opened over it closes back to it.
    app.open_human_review(item.clone());
    assert!(
        matches!(app.screen, Screen::HumanGate(_)),
        "{}",
        app.message
    );
    assert!(matches!(esc_on_review(&mut app), Action::Back));
    assert!(matches!(app.screen, Screen::Waiting));
    assert!(app.global_key(KeyCode::Char(';')));
    assert!(matches!(app.screen, Screen::Operations(_)));
    app.back();
    assert!(matches!(app.screen, Screen::Waiting));

    // `O` is the overview, from anywhere; and the screen esc returns to is
    // now the overview.
    assert!(app.global_key(KeyCode::Char('O')));
    assert!(matches!(app.screen, Screen::Navigator));
    assert!(drawn(&mut app).0.contains("O waiting"));
    app.open_human_review(item.clone());
    assert!(matches!(esc_on_review(&mut app), Action::Back));
    assert!(matches!(app.screen, Screen::Navigator));

    // One opened from the work screen still goes back to the work screen, and
    // a reload keeps where it goes.
    let mut review = HumanGateScreen::new(
        item.clone(),
        crate::work::runtime::human_gate::Review::default(),
    );
    assert!(review.footer().contains("esc work"), "{}", review.footer());
    assert!(matches!(
        review.handle_key(KeyCode::Esc),
        Action::OpenWork(_)
    ));
    app.open_human_review(item.clone());
    app.open_human_review(item.clone());
    let Screen::HumanGate(review) = &app.screen else {
        panic!("the reload keeps the review open");
    };
    assert_eq!(review.back, Return::Overview);
    app.back();

    // `O` on the overview comes back to Waiting, and from a screen opened
    // over Waiting it goes to the overview.
    assert!(app.global_key(KeyCode::Char('O')));
    assert!(matches!(app.screen, Screen::Waiting));
    app.screen = review_screen(item);
    assert!(app.global_key(KeyCode::Char('O')));
    assert!(matches!(app.screen, Screen::Navigator));
}

/// The start screen is the reader's setting, and `--overview` overrides it
/// for one launch (§FS-019-waiting.5).
#[test]
fn the_start_screen_is_the_setting_or_the_flag() {
    let waiting = crate::feed::config::Defaults::default();
    let overview = crate::feed::config::Defaults {
        start_screen: crate::feed::config::StartScreen::Overview,
        ..Default::default()
    };
    let plain = crate::cli::TuiArgs::default();
    let flagged = crate::cli::TuiArgs { overview: true };
    assert_eq!(start_home(&waiting, &plain), Home::Waiting);
    assert_eq!(start_home(&overview, &plain), Home::Overview);
    assert_eq!(start_home(&waiting, &flagged), Home::Overview);
}

/// A new matter is processed where it is shown, through the same calls the
/// command makes (§FS-019-waiting.7.1, §FS-019-waiting.7.2,
/// §FS-019-waiting.7.4): where nothing hands it over and the runtime's
/// `triage` cannot be had, the row proposes `triage` and says it is not
/// available; the processing screen says why; and a dismissal with a reason
/// takes it out of New.
#[test]
fn a_new_matter_is_proposed_processed_or_dismissed_from_waiting() {
    if isolated(
        "feed::tui::waiting_app_tests::a_new_matter_is_proposed_processed_or_dismissed_from_waiting",
    ) {
        return;
    }
    stored("demo", "Durations read as seconds", Utc::now());
    let config: StatusConfig =
        serde_json::from_value(json!({ "projects": { "demo": { "providers": [] } } })).unwrap();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.go_home_to(Home::Waiting);
    app.freshen_waiting();
    let (text, _) = drawn(&mut app);
    assert!(text.contains("→ triage · not available"), "{text}");
    assert!(text.contains("A process all"), "{text}");
    let item = app.ctx.items().remove(0);

    // Enter on the row opens the processing screen over it.
    assert!(matches!(
        app.waiting.handle_key(KeyCode::Enter),
        Action::ProcessMatter { entry: None, .. }
    ));
    app.open_processing(item.clone(), None, false);
    let Screen::Process(screen) = &mut app.screen else {
        panic!("the processing screen opens");
    };
    // The screen says why it is not available: here, before any runtime,
    // the project has no registry row to place work in.
    let shown = screen.text();
    assert!(
        shown.contains("no entry hands this matter over, so the runtime's triage is proposed"),
        "{shown}"
    );
    assert!(shown.contains("demo has no registry row"), "{shown}");
    assert!(matches!(
        screen.handle_key(KeyCode::Enter),
        Action::SetMessage(_)
    ));
    app.back();
    assert!(matches!(app.screen, Screen::Waiting));

    // `x`, then a reason: the call `ephor work dismiss` makes. An empty one
    // is refused and records nothing.
    assert!(!app.ctx.dismiss(&item, "  ").ok);
    let outcome = app.ctx.dismiss(&item, "a duplicate of #1");
    assert!(outcome.ok, "{}", outcome.says);
    app.processed();
    app.freshen_waiting();
    let (text, _) = drawn(&mut app);
    assert!(
        text.contains("ephor — waiting   0 on you · 0 new · 0 running"),
        "{text}"
    );
    assert!(!text.contains("Durations read as seconds"), "{text}");
}
