//! The Monitors page and the detail view as the screen draws them and as its
//! keys move them (§FS-021-monitors.18, §FS-021-monitors.19), over pages the
//! API's own fixtures build in a directory of the test's own.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::style::Color;
use ratatui::Terminal;
use serde_json::json;

use super::MonitorsScreen;
use crate::api::monitor_views::{self as views, NO_MONITORS};
use crate::api::monitors::tests::fixtures::{green_detail, latency_detail, page_of_projects};
use crate::api::monitors::tests::page_of_six;
use crate::feed::model::{Item, ItemKind};
use crate::feed::tui::monitors_draw::{Columns, Plot};
use crate::feed::tui::Action;
use crate::monitors::window::Window;

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-25T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn screen(page: views::Monitors) -> MonitorsScreen {
    MonitorsScreen::new(page, BTreeMap::new(), Window::Week, now())
}

fn render(screen: &mut MonitorsScreen, width: u16, height: u16) -> (String, Buffer) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| screen.draw(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let text = (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, buffer)
}

/// The colour of the first cell that draws `symbol`.
fn colour_of(buffer: &Buffer, symbol: &str) -> Color {
    let area = buffer.area;
    (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .map(|at| &buffer[at])
        .find(|cell| cell.symbol() == symbol)
        .unwrap_or_else(|| panic!("{symbol} is drawn"))
        .fg
}

fn item(id: &str) -> Item {
    Item {
        id: id.into(),
        project: "widget".into(),
        source: "monitor".into(),
        kind: ItemKind::Status,
        role: None,
        title: "Line coverage dropped".into(),
        url: None,
        state: Some("down".into()),
        needs_response: false,
        updated_at: now(),
        raw: json!({}),
    }
}

/// §FS-021-monitors.18.2, §FS-021-monitors.18.3, §FS-021-monitors.18.4,
/// §FS-021-monitors.18.7: the header counts every monitor; projects come
/// worst first; each row carries its glyph and health in its rank's colour,
/// its strip and its change; and the second project's health column names
/// each cause and its note is said under it.
#[test]
fn the_page_groups_its_projects_worst_first_with_every_health_and_cause() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(page_of_projects(dir.path()));
    assert!(
        screen
            .title()
            .starts_with(" ephor — monitors   1 down · 4 unknown"),
        "{}",
        screen.title()
    );
    assert!(
        screen.title().ends_with("window 7d · refreshed 4m ago"),
        "{}",
        screen.title()
    );
    let (text, buffer) = render(&mut screen, 200, 40);
    let widget = text.find("  widget").expect("widget is a group");
    let tools = text.find("  tools").expect("tools is a group");
    assert!(widget < tools, "the worse project first:\n{text}");
    for said in [
        "Line coverage",
        "79.2%",
        "healthy ≥ 80%",
        "overdue 4h",
        "failing 1 of 2",
        "not read yet",
        "KPI",
        "New",
        "matter",
        "last 7d",
        "note: ephor.json could not be read",
        "change: over the window",
    ] {
        assert!(text.contains(said), "{said} on the page:\n{text}");
    }
    for cause in ["unreadable ", "parked ", "stale "] {
        assert!(text.contains(cause), "{cause} on the page:\n{text}");
    }
    assert_eq!(colour_of(&buffer, "✖"), Color::Red);
    assert_eq!(colour_of(&buffer, "▼"), Color::Yellow);
    assert_eq!(colour_of(&buffer, "▲"), Color::Yellow);
    assert_eq!(colour_of(&buffer, "●"), Color::Green);
    assert_eq!(colour_of(&buffer, "◆"), Color::DarkGray);
    assert_eq!(colour_of(&buffer, "?"), Color::DarkGray);
}

/// §FS-021-monitors.18.5, §FS-021-monitors.18.9: the strip is drawn cell by
/// cell in each cell's health, a boolean's by its health, low while down;
/// over a week the build's four readings fall in one cell, which takes the
/// worse of them.
#[test]
fn the_strip_is_drawn_in_each_cells_health() {
    let dir = tempfile::tempdir().unwrap();
    let mut page = page_of_six(dir.path());
    let green = green_detail(&dir.path().join("green")).row;
    page.projects[0].rows[1] = green;
    let mut screen = screen(page);
    let (text, buffer) = render(&mut screen, 200, 20);
    let row = text
        .lines()
        .find(|line| line.contains("Build green"))
        .unwrap();
    assert!(row.contains(" ▁  1 drop"), "the strip is drawn: {row}");
    let y = text
        .lines()
        .position(|line| line.contains("Build green"))
        .unwrap() as u16;
    let colours: Vec<Color> = (0..200)
        .map(|x| &buffer[(x, y)])
        .filter(|cell| "▁█".contains(cell.symbol()))
        .map(|cell| cell.fg)
        .collect();
    assert_eq!(colours, [Color::Red], "the cell is down");
}

/// §FS-021-monitors.18.2: with no monitors the page keeps its header, every
/// count at zero, and says how to declare one.
#[test]
fn an_empty_page_says_how_to_declare_a_monitor() {
    let mut screen = screen(views::Monitors {
        window: Window::Week,
        counts: Default::default(),
        refreshed_at: None,
        projects: Vec::new(),
        without: Vec::new(),
    });
    assert_eq!(
        screen.title(),
        " ephor — monitors   0 down · 0 unknown · 0 failing · 0 warn · 0 ok · 0 KPIs   \
         window 7d · never refreshed"
    );
    let (text, _) = render(&mut screen, 120, 10);
    assert!(text.contains(NO_MONITORS), "{text}");
    assert!(matches!(screen.handle_key(KeyCode::Enter), Action::None));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('p')),
        Action::None
    ));
}

/// §FS-021-monitors.18.8: `w` cycles 6h, 1d, 7d, 30d, 90d and 1y, asking
/// the shell for the page again each time.
#[test]
fn w_cycles_the_window_and_asks_for_the_page_again() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(page_of_six(dir.path()));
    let mut seen = Vec::new();
    for _ in 0..6 {
        assert!(matches!(
            screen.handle_key(KeyCode::Char('w')),
            Action::ReadMonitors
        ));
        seen.push(screen.window.label());
    }
    assert_eq!(seen, ["30d", "90d", "1y", "6h", "1d", "7d"]);
}

/// §FS-021-monitors.18.10, §FS-021-monitors.19: `Enter` asks for the row's
/// detail, `]` and `[` step through the page's order and ask again, and
/// `esc` comes back to the page with the cursor where it was.
#[test]
fn enter_opens_the_detail_and_brackets_step_through_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(page_of_six(dir.path()));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('j')),
        Action::None
    ));
    assert!(matches!(
        screen.handle_key(KeyCode::Enter),
        Action::ReadMonitor
    ));
    assert_eq!(screen.wanted(), Some(("widget".into(), "green".into())));
    screen.show(green_detail(&dir.path().join("green")));
    assert!(screen
        .title()
        .starts_with(" ephor — monitors › widget › Build green   window 7d"));
    assert!(
        screen.footer().contains("[/] previous/next"),
        "{}",
        screen.footer()
    );

    assert!(matches!(
        screen.handle_key(KeyCode::Char(']')),
        Action::ReadMonitor
    ));
    assert_eq!(screen.wanted(), Some(("widget".into(), "flaky".into())));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('[')),
        Action::ReadMonitor
    ));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('[')),
        Action::ReadMonitor
    ));
    assert_eq!(screen.wanted(), Some(("widget".into(), "coverage".into())));
    assert!(
        matches!(screen.handle_key(KeyCode::Char('[')), Action::None),
        "nothing before the first"
    );

    assert!(matches!(screen.handle_key(KeyCode::Esc), Action::None));
    assert!(screen.detail().is_none());
    assert_eq!(screen.selected(), 0);
    assert!(
        screen.footer().contains("enter open"),
        "{}",
        screen.footer()
    );
    assert!(matches!(screen.handle_key(KeyCode::Esc), Action::Back));
}

/// §FS-021-monitors.19: `←` and `→` walk the chart's points, and the line
/// under the chart says the one the cursor is on.
#[test]
fn the_arrows_move_the_charts_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(page_of_six(dir.path()));
    screen.handle_key(KeyCode::Enter);
    let detail = green_detail(&dir.path().join("green"));
    let last = detail.chart.len() - 1;
    screen.show(detail);
    assert_eq!(screen.cursor, last, "the cursor starts at the newest");
    screen.handle_key(KeyCode::Right);
    assert_eq!(screen.cursor, last, "nothing after the newest");
    screen.handle_key(KeyCode::Left);
    assert_eq!(screen.cursor, last - 1);
    let (text, _) = render(&mut screen, 120, 40);
    let cursor = text
        .lines()
        .find(|line| line.starts_with("cursor: "))
        .unwrap();
    assert!(cursor.contains("  no  down"), "{cursor}");
}

/// §FS-021-monitors.18.10: `p` processes an open matter in place, by the
/// move `Enter` makes on Waiting, and says why there is nothing to process
/// on any other row; `r` asks for the row's project now; `o` opens the
/// sample's url or says it has none.
#[test]
fn p_r_and_o_act_on_the_selected_row() {
    let dir = tempfile::tempdir().unwrap();
    let page = page_of_six(dir.path());
    let key = page
        .rows()
        .next()
        .unwrap()
        .matter
        .as_ref()
        .unwrap()
        .key
        .clone();
    let mut screen = screen(page.clone());
    match screen.handle_key(KeyCode::Char('p')) {
        Action::SetMessage(said) => assert!(said.contains("not in the feed yet"), "{said}"),
        _ => panic!("a matter not in the feed is named"),
    }
    let mut screen = MonitorsScreen::new(
        page,
        BTreeMap::from([(key.clone(), item(&key))]),
        Window::Week,
        now(),
    );
    match screen.handle_key(KeyCode::Char('p')) {
        Action::ProcessMatter {
            item,
            entry,
            choosing,
        } => {
            assert_eq!(item.id, key);
            assert!(
                entry.is_none() && !choosing,
                "the proposal, as Enter on Waiting"
            );
        }
        _ => panic!("p processes the open matter"),
    }
    match screen.handle_key(KeyCode::Char('r')) {
        Action::RefreshMonitors(project) => assert_eq!(project, "widget"),
        _ => panic!("r refreshes the row's project"),
    }
    match screen.handle_key(KeyCode::Char('o')) {
        Action::SetMessage(said) => assert_eq!(said, "o: the last answer carried no url"),
        _ => panic!("o says there is no url"),
    }
    screen.handle_key(KeyCode::Char('G'));
    match screen.handle_key(KeyCode::Char('p')) {
        Action::SetMessage(said) => assert_eq!(said, "p: a KPI has no matter"),
        _ => panic!("a KPI has no matter"),
    }
    screen.handle_key(KeyCode::Char('k'));
    match screen.handle_key(KeyCode::Char('p')) {
        Action::SetMessage(said) => assert_eq!(said, "p: no matter is open for this monitor"),
        _ => panic!("an ok monitor has no matter"),
    }
}

/// §FS-021-monitors.18.10: `o` opens the url the last sample carried.
#[test]
fn o_opens_the_samples_url() {
    let dir = tempfile::tempdir().unwrap();
    let mut page = page_of_six(dir.path());
    page.projects[0].rows[0].url = Some("https://ci.example/coverage".into());
    let mut screen = screen(page);
    match screen.handle_key(KeyCode::Char('o')) {
        Action::OpenUrl(Some(url)) => assert_eq!(url, "https://ci.example/coverage"),
        _ => panic!("o opens the url"),
    }
}

/// §FS-021-monitors.19: the chart draws today's `healthy` bound in red and
/// `warn` in amber as lines across it, names them under it, and labels both
/// axes; a failed read leaves a gap rather than a line through it.
#[test]
fn the_chart_draws_the_thresholds_across_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(page_of_six(dir.path()));
    screen.handle_key(KeyCode::Char('G'));
    screen.handle_key(KeyCode::Char('k'));
    screen.handle_key(KeyCode::Enter);
    screen.show(latency_detail(&dir.path().join("latency")));
    let (text, buffer) = render(&mut screen, 120, 48);
    assert!(
        text.contains("bounds:  ── healthy max 50 ms  ── warn max 40 ms"),
        "{text}"
    );
    let run = |colour: Color| {
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .filter(|x| {
                        let cell = &buffer[(*x, y)];
                        cell.fg == colour
                            && cell
                                .symbol()
                                .chars()
                                .all(|c| ('\u{2800}'..='\u{28FF}').contains(&c))
                    })
                    .count()
            })
            .max()
            .unwrap_or(0)
    };
    assert!(
        run(Color::Red) >= 40,
        "the healthy bound crosses the chart:\n{text}"
    );
    assert!(
        run(Color::Yellow) >= 40,
        "the warn bound crosses the chart:\n{text}"
    );
    assert!(text.contains("ms"), "the y axis says the unit:\n{text}");

    let detail = latency_detail(&dir.path().join("again"));
    let plot = Plot::of(&detail);
    assert!(!plot.stepped);
    assert_eq!(plot.points[1], None, "the failed read is a gap");
    assert!(plot.segments(&detail).is_empty(), "no line crosses the gap");
    assert_eq!(plot.alone().len(), 2);
    assert!(plot.y[0] < 30.0 && plot.y[1] > 60.0, "{:?}", plot.y);
}

/// §FS-021-monitors.19: a boolean is drawn stepped, its y axis naming its
/// states, so it reads without colour.
#[test]
fn a_boolean_is_drawn_stepped_with_its_states_on_the_axis() {
    let dir = tempfile::tempdir().unwrap();
    let detail = green_detail(dir.path());
    let plot = Plot::of(&detail);
    assert!(plot.stepped);
    assert_eq!(plot.y_labels, ["no", "yes"]);
    assert_eq!(plot.y, [0.0, 1.0]);
    let segments = plot.segments(&detail);
    let (step, _) = &segments[0];
    assert_eq!(step.len(), 3, "it holds, then steps");
    assert_eq!(step[1].1, step[0].1, "held until the next reading");
    assert_eq!(step[1].0, step[2].0, "then it steps");
}

/// §FS-021-monitors.18.4: a narrow screen gives up the condition, then
/// the strip, before it cuts a title past twelve characters; a wide one
/// draws every column.
#[test]
fn a_narrow_screen_gives_up_columns_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let page = page_of_projects(dir.path());
    let wide = Columns::fit(&page, 220);
    assert!(wide.strip && wide.condition.is_some(), "{wide:?}");
    let narrow = Columns::fit(&page, 80);
    assert!(narrow.condition.is_none(), "{narrow:?}");
    assert!(narrow.title >= 12, "{narrow:?}");
}

/// §FS-021-monitors.18.4, §FS-021-monitors.21.1: at 80 by 24 the page and
/// the detail view draw without overlap, a long title clipped, the health
/// column whole and the window still said.
#[test]
fn the_page_and_the_detail_fit_80_by_24() {
    let dir = tempfile::tempdir().unwrap();
    let mut page = page_of_projects(dir.path());
    page.projects[0].rows[0].title =
        "Line coverage of every crate in the workspace, integration tests included".into();
    let mut screen = screen(page);
    let (text, _) = render(&mut screen, 80, 22);
    assert!(text.contains("window 7d · refreshed 4m ago"), "{text}");
    let first = text.lines().find(|line| line.starts_with('✖')).unwrap();
    assert!(first.contains('…'), "{first}");
    assert!(!text.contains("integration tests included"), "{text}");
    let flaky = text
        .lines()
        .find(|line| line.contains("Flaky tests"))
        .unwrap();
    assert!(flaky.contains("failing 1 of 2"), "{flaky}");
    for _ in 0..20 {
        screen.handle_key(KeyCode::Char('j'));
        render(&mut screen, 80, 22);
    }
    screen.handle_key(KeyCode::Char('g'));
    screen.handle_key(KeyCode::Enter);
    screen.show(latency_detail(&dir.path().join("latency")));
    let (text, _) = render(&mut screen, 80, 22);
    assert!(text.lines().next().unwrap().starts_with("✖ "), "{text}");
    assert!(text.contains("bounds:  ── healthy max 50 ms"), "{text}");
    for _ in 0..40 {
        screen.handle_key(KeyCode::Char('j'));
        render(&mut screen, 80, 22);
    }
    let (text, _) = render(&mut screen, 80, 22);
    assert!(
        text.contains("readings (3)"),
        "scrolled to the readings:\n{text}"
    );
    render(&mut screen, 20, 5);
}
