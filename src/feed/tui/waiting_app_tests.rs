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

    // A screen opened from Waiting comes back to Waiting, and the board
    // opened over it closes back to it.
    app.screen = review_screen(item.clone());
    app.back();
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
    app.screen = review_screen(item.clone());
    app.back();
    assert!(matches!(app.screen, Screen::Navigator));

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
