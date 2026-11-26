//! The Monitors page driven through the shell itself in an isolated home
//! (§FS-021-monitors.18.1, §FS-021-monitors.18.10, §REQ-003-test-isolation):
//! `M` from any screen and back, `p` processing in place, and `r` starting
//! a refresh of one project beneath the screen.

use super::cache_freshness_tests::{drawn, isolated};
use super::*;
use crate::api::monitors::tests::fixtures::drop_in_store;
use serde_json::json;

/// A project `demo` with one site-declared monitor, `coverage`, whose drop
/// is open in the store and whose matter is in the feed.
fn demo() -> (StatusConfig, Item) {
    let config: StatusConfig = serde_json::from_value(json!({
        "projects": {"demo": {
            "providers": [],
            "monitors": [{"id": "coverage", "title": "Line coverage", "unit": "%",
                          "healthy": {"min": 80}, "every": "1h"}]
        }}
    }))
    .unwrap();
    let at = Utc::now() - chrono::TimeDelta::minutes(30);
    let item = drop_in_store("demo", "coverage", at);
    let feed = ProjectFeed {
        project: "demo".into(),
        fetched_at: Some(at),
        model: cache::MODEL,
        revision: at.timestamp_millis().max(0) as u64,
        providers: std::collections::BTreeMap::from([(
            crate::monitors::watch::SLOT.to_string(),
            cache::ProviderSlot {
                fetched_at: Some(at),
                ok: true,
                matters: vec![crate::matter::Matter::of_item(&item)],
                ..Default::default()
            },
        )]),
    };
    cache::store_feed(&feed).unwrap();
    (config, item)
}

fn key(app: &mut App, code: KeyCode) {
    if app.global_key(code) {
        return;
    }
    let action = match &mut app.screen {
        Screen::Monitors(page) => page.handle_key(code),
        Screen::Operations(board) => board.handle_key(code),
        Screen::Burn(page) => page.handle_key(code),
        Screen::Waiting => app.waiting.handle_key(code),
        Screen::Process(screen) => screen.handle_key(code),
        _ => panic!("a screen this test drives"),
    };
    match action {
        Action::Back => app.back(),
        Action::ReadMonitors => {
            app.reload_monitors();
        }
        Action::ReadMonitor => app.read_monitor(),
        Action::ProcessMatter {
            item,
            entry,
            choosing,
        } => app.open_processing(item, entry, choosing),
        Action::None => {}
        Action::SetMessage(message) => app.message = message,
        _ => panic!("an action this test applies"),
    }
}

/// §FS-021-monitors.18.1: `M` opens the page from Waiting and from the
/// operations board, and `M` or `esc` closes it back to where the reader
/// was; the burn page opened over it is the same modal layer.
#[test]
fn m_opens_the_page_from_any_screen_and_closes_it_back() {
    if isolated(
        "feed::tui::monitors_app_tests::m_opens_the_page_from_any_screen_and_closes_it_back",
    ) {
        return;
    }
    let (config, _) = demo();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.go_home_to(Home::Waiting);
    app.freshen_waiting();

    key(&mut app, KeyCode::Char('M'));
    assert!(matches!(app.screen, Screen::Monitors(_)));
    let (text, _) = drawn(&mut app);
    assert!(text.contains("ephor — monitors   1 down"), "{text}");
    assert!(text.contains("Line coverage"), "{text}");
    key(&mut app, KeyCode::Char('M'));
    assert!(matches!(app.screen, Screen::Waiting));

    key(&mut app, KeyCode::Char('M'));
    key(&mut app, KeyCode::Esc);
    assert!(matches!(app.screen, Screen::Waiting));

    // Over the board or the burn page, it closes back to that page, and
    // that page then closes back to where it opened.
    key(&mut app, KeyCode::Char(';'));
    assert!(matches!(app.screen, Screen::Operations(_)));
    key(&mut app, KeyCode::Char('M'));
    assert!(matches!(app.screen, Screen::Monitors(_)));
    key(&mut app, KeyCode::Char('M'));
    assert!(
        matches!(app.screen, Screen::Operations(_)),
        "M closes back to the board"
    );
    key(&mut app, KeyCode::Char('$'));
    assert!(matches!(app.screen, Screen::Burn(_)));
    key(&mut app, KeyCode::Char('M'));
    assert!(matches!(app.screen, Screen::Monitors(_)));
    key(&mut app, KeyCode::Esc);
    assert!(
        matches!(app.screen, Screen::Burn(_)),
        "esc closes back to the burn page"
    );
    key(&mut app, KeyCode::Esc);
    assert!(
        matches!(app.screen, Screen::Waiting),
        "and the burn page back to where it opened"
    );

    // From the overview too, and back to it.
    app.go_home_to(Home::Overview);
    key(&mut app, KeyCode::Char('M'));
    assert!(matches!(app.screen, Screen::Monitors(_)));
    key(&mut app, KeyCode::Char('M'));
    assert!(matches!(app.screen, Screen::Navigator));
}

/// §FS-021-monitors.18.10, §FS-019-waiting.7.2: `p` on the row opens the
/// processing screen Waiting's `Enter` opens, and `esc` comes back to the
/// page rather than to Waiting; the detail's `p` does the same.
#[test]
fn p_processes_the_matter_in_place() {
    if isolated("feed::tui::monitors_app_tests::p_processes_the_matter_in_place") {
        return;
    }
    let (config, item) = demo();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.go_home_to(Home::Waiting);
    app.freshen_waiting();
    key(&mut app, KeyCode::Char('M'));

    key(&mut app, KeyCode::Char('p'));
    assert!(
        matches!(app.screen, Screen::Process(_)),
        "p opens processing: {}",
        app.message
    );
    key(&mut app, KeyCode::Esc);
    assert!(
        matches!(app.screen, Screen::Monitors(_)),
        "esc comes back to the page"
    );

    key(&mut app, KeyCode::Enter);
    let Screen::Monitors(page) = &app.screen else {
        panic!("the page is open");
    };
    assert!(page.detail().is_some(), "{}", app.message);
    let (text, _) = drawn(&mut app);
    assert!(
        text.contains("ephor — monitors › demo › Line coverage"),
        "{text}"
    );
    assert!(
        text.contains(&format!("matter: {} · New", item.id)),
        "{text}"
    );
    key(&mut app, KeyCode::Char('p'));
    assert!(matches!(app.screen, Screen::Process(_)), "{}", app.message);
    // A move confirmed comes back to the page too (§FS-021-monitors.18.10).
    app.processed();
    assert!(matches!(app.screen, Screen::Monitors(_)));
    key(&mut app, KeyCode::Esc);
    key(&mut app, KeyCode::Esc);
    assert!(matches!(app.screen, Screen::Waiting));
}

/// §FS-021-monitors.18.10, §FS-019-waiting.7.2: processing opened from the
/// page is about the one matter the row names: it says nothing of Waiting's
/// new rows, offers no `n` through them, names the page `esc` goes back to,
/// and leaves no message of the page's beside it.
#[test]
fn processing_from_the_page_is_about_its_one_matter() {
    if isolated("feed::tui::monitors_app_tests::processing_from_the_page_is_about_its_one_matter") {
        return;
    }
    let (config, item) = demo();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.go_home_to(Home::Waiting);
    app.freshen_waiting();
    key(&mut app, KeyCode::Char('M'));
    app.message = "p: an earlier row's refusal".to_string();
    key(&mut app, KeyCode::Char('p'));
    let Screen::Process(screen) = &app.screen else {
        panic!("p opens processing: {}", app.message);
    };
    assert!(!screen.title().ends_with(" new"), "{}", screen.title());
    let footer = screen.footer();
    assert!(footer.ends_with("esc monitors"), "{footer}");
    assert!(!footer.contains("n next"), "{footer}");
    assert!(app.message.is_empty(), "{}", app.message);

    // Waiting's own processing still goes back to Waiting.
    app.go_home_to(Home::Waiting);
    app.open_processing(item, None, false);
    let Screen::Process(screen) = &app.screen else {
        panic!("Enter opens processing: {}", app.message);
    };
    assert!(screen.title().contains("1 of 1 new"), "{}", screen.title());
    assert!(
        screen.footer().ends_with("esc waiting"),
        "{}",
        screen.footer()
    );
}

/// §FS-021-monitors.18.10: the page goes back only with the processing
/// screen it was opened under. Once that screen is left by another road,
/// a later processing screen, `A`'s preview included, comes back to
/// Waiting rather than to a page from before.
#[test]
fn a_page_left_behind_does_not_come_back_later() {
    if isolated("feed::tui::monitors_app_tests::a_page_left_behind_does_not_come_back_later") {
        return;
    }
    let (config, _) = demo();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.go_home_to(Home::Waiting);
    app.freshen_waiting();
    key(&mut app, KeyCode::Char('M'));
    key(&mut app, KeyCode::Char('p'));
    assert!(matches!(app.screen, Screen::Process(_)), "{}", app.message);
    // A move that lands on the floor without `back`, as settling does.
    app.screen = app.home_screen();
    let view = app.ctx.process_all(&[], true);
    app.preview_process_all(view);
    app.back();
    assert!(
        matches!(app.screen, Screen::Waiting),
        "esc from A's preview is Waiting"
    );
    let view = app.ctx.process_all(&[], true);
    app.preview_process_all(view);
    app.processed();
    assert!(matches!(app.screen, Screen::Waiting));
}

/// §FS-021-monitors.18.10: `r` refreshes the row's project and reads every
/// monitor of it now, beneath the screen, as `ephor refresh demo
/// --monitors-now` does; a second `r` while it runs is refused.
#[test]
fn r_refreshes_the_rows_project_beneath_the_screen() {
    if isolated("feed::tui::monitors_app_tests::r_refreshes_the_rows_project_beneath_the_screen") {
        return;
    }
    let (config, _) = demo();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.go_home_to(Home::Waiting);
    key(&mut app, KeyCode::Char('M'));
    let Screen::Monitors(page) = &mut app.screen else {
        panic!("the page is open");
    };
    let Action::RefreshMonitors(project) = page.handle_key(KeyCode::Char('r')) else {
        panic!("r asks for a refresh");
    };
    assert_eq!(project, "demo");
    app.refresh_monitors(&config, &project);
    let refresh = app
        .refresh
        .as_ref()
        .expect("a refresh runs beneath the screen");
    assert_eq!(refresh.progress(), "Refreshing demo --monitors-now (1/1)…");
    assert!(
        matches!(app.screen, Screen::Monitors(_)),
        "the page stays up"
    );
    app.refresh_monitors(&config, &project);
    assert_eq!(app.message, "Already refreshing");
    let began = std::time::Instant::now();
    while app.refresh.is_some() && began.elapsed() < Duration::from_secs(60) {
        app.collect_refresh().unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(app.refresh.is_none(), "the refresh lands");
    assert!(matches!(app.screen, Screen::Monitors(_)));
    assert!(app.message.contains("--monitors-now"), "{}", app.message);
}

/// The screen as an `width` by `height` terminal shows it, one line a row.
fn drawn_at(app: &mut App, width: u16, height: u16) -> Vec<String> {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..height)
        .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
        .collect()
}

/// §FS-021-monitors.18.2, §FS-001-forge-interface.7: where the header does
/// not fit beside a refresh's progress, the progress keeps a line of its
/// own and the header's counts and window are said above the first
/// project, overdue included; where it fits, both share the title bar.
#[test]
fn a_refresh_in_flight_is_said_beside_a_header_too_wide_for_both() {
    if isolated(
        "feed::tui::monitors_app_tests::a_refresh_in_flight_is_said_beside_a_header_too_wide_for_both",
    ) {
        return;
    }
    let (config, _) = demo();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.go_home_to(Home::Waiting);
    key(&mut app, KeyCode::Char('M'));
    app.refresh_monitors(&config, "demo");
    let progress = "Refreshing demo --monitors-now (1/1)…";

    let lines = drawn_at(&mut app, 80, 24);
    let shown = lines.join("\n");
    assert!(lines[0].starts_with(" ephor — monitors   "), "{shown}");
    assert!(!lines[0].contains("1 down"), "{shown}");
    assert!(
        lines[..2].iter().any(|line| line.contains(progress)),
        "{shown}"
    );
    let counts = lines
        .iter()
        .position(|line| line.contains("1 down · 0 unknown"));
    let window = lines
        .iter()
        .position(|line| line.contains("window 7d · refreshed"));
    let project = lines
        .iter()
        .position(|line| line.trim_start().starts_with("demo"));
    assert!(counts.is_some() && window.is_some(), "{shown}");
    assert!(counts < project && window < project, "{shown}");

    let lines = drawn_at(&mut app, 200, 24);
    assert!(
        lines[0].contains("ephor — monitors   1 down") && lines[0].contains(progress),
        "{}",
        lines.join("\n")
    );

    let began = std::time::Instant::now();
    while app.refresh.is_some() && began.elapsed() < Duration::from_secs(60) {
        app.collect_refresh().unwrap();
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// §FS-021-monitors.20.1, §FS-021-monitors.18.10: on an 80-column terminal
/// a footer too long for one line goes on to the next rather than off the
/// edge, so Waiting's `M opens them` and every key the page names are seen.
#[test]
fn a_footer_too_long_for_the_screen_goes_on_to_another_line() {
    if isolated(
        "feed::tui::monitors_app_tests::a_footer_too_long_for_the_screen_goes_on_to_another_line",
    ) {
        return;
    }
    let (config, _) = demo();
    let mut app = App::load(&config, &crate::scope::Projects::every()).unwrap();
    app.go_home_to(Home::Waiting);
    app.freshen_waiting();
    let shown = drawn_at(&mut app, 80, 24).join("\n");
    assert!(shown.contains("M opens them"), "{shown}");
    assert!(shown.contains("q quit"), "{shown}");
    key(&mut app, KeyCode::Char('M'));
    let shown = drawn_at(&mut app, 80, 24).join("\n");
    assert!(shown.contains("esc/M back"), "{shown}");
    key(&mut app, KeyCode::Enter);
    let shown = drawn_at(&mut app, 80, 24).join("\n");
    assert!(shown.contains("M close"), "{shown}");
}
