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
use crate::api::monitor_views::{self as views, NO_MONITORS, TITLE};
use crate::api::monitors::tests::fixtures::{green_detail, latency_detail, page_of_projects};
use crate::api::monitors::tests::page_of_six;
use crate::feed::model::{Item, ItemKind};
use crate::feed::tui::monitors_chart::Plot;
use crate::feed::tui::monitors_page::{Columns, Shape};
use crate::feed::tui::Action;
use crate::monitors::model::Health;
use crate::monitors::strip::{Cell, Mark};
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
            .title(200)
            .starts_with(" ephor — monitors   1 down · 4 unknown"),
        "{}",
        screen.title(200)
    );
    assert!(
        screen.title(200).ends_with("window 7d · refreshed 4m ago"),
        "{}",
        screen.title(200)
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
        screen.title(200),
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
        .title(200)
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
    // §FS-021-monitors.19: every other row says why in the words its detail
    // view's matter line uses, which the model says once.
    let said = |screen: &mut MonitorsScreen| match screen.handle_key(KeyCode::Char('p')) {
        Action::SetMessage(said) => said,
        _ => panic!("p says why there is nothing to process"),
    };
    screen.handle_key(KeyCode::Char('j'));
    assert!(
        said(&mut screen).contains(".unknown@20261125T090000Z is not in the feed yet"),
        "the overdue build's unknown matter is New"
    );
    screen.handle_key(KeyCode::Char('j'));
    assert_eq!(said(&mut screen), "p: none yet: it drops after its hold");
    screen.handle_key(KeyCode::Char('G'));
    assert_eq!(said(&mut screen), "p: none: a KPI is never a matter");
    screen.handle_key(KeyCode::Char('k'));
    assert_eq!(said(&mut screen), "p: none: nothing is open");
}

/// §FS-021-monitors.18.10, §FS-021-monitors.13.2: `p` processes only a New
/// matter, and says what a running, dismissed or finished one leaves to do:
/// a dismissed one comes back when its monitor moves, and only a finished
/// one waits for the next drop.
#[test]
fn p_says_what_a_matter_past_new_leaves_to_do() {
    let dir = tempfile::tempdir().unwrap();
    let page = page_of_six(dir.path());
    let said = |state: &'static str| {
        let mut page = page.clone();
        page.projects[0].rows[0].matter.as_mut().unwrap().state = state;
        match screen(page).handle_key(KeyCode::Char('p')) {
            Action::SetMessage(said) => said,
            _ => panic!("p says why there is nothing to process"),
        }
    };
    assert_eq!(
        said("running"),
        "p: work is already running on this matter; Waiting shows the run"
    );
    assert_eq!(
        said("dismissed"),
        "p: the matter is dismissed; it comes back if the monitor moves"
    );
    assert_eq!(
        said("Recent"),
        "p: the matter is Recent; the next drop opens a new one"
    );
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
        text.contains("bounds:  ── healthy ≤ 50 ms  ── warn above 40 ms"),
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
    let ((hold, _), (step, _)) = (&segments[0], &segments[1]);
    assert_eq!(hold[1].1, hold[0].1, "held until the next reading");
    assert_eq!(hold[1], step[0], "then it steps");
    assert_eq!(step[1].0, step[0].0, "straight up or down");
}

/// A boolean read four times by `x` label, ok, ok, down, ok, on the chart's
/// scale: `yes` on top.
fn flips(dir: &std::path::Path, labels: &[&str]) -> views::MonitorDetail {
    let mut detail = green_detail(dir);
    detail.windowed = false;
    let healths = [Health::Ok, Health::Ok, Health::Down, Health::Ok, Health::Ok];
    detail.chart = labels
        .iter()
        .zip(healths)
        .map(|(x, health)| views::ChartPoint {
            t: None,
            x: Some((*x).to_string()),
            value: Some(if health == Health::Ok { 1.0 } else { 0.0 }),
            health: Some(health),
        })
        .collect();
    detail
}

fn opened(detail: views::MonitorDetail) -> (MonitorsScreen, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(page_of_six(dir.path()));
    screen.handle_key(KeyCode::Enter);
    screen.show(detail);
    (screen, dir)
}

/// §FS-021-monitors.19: a step holds the value it read in that reading's
/// health, and only the step itself is drawn in the worse of the two, so an
/// ok plateau before a drop is green, as the cursor line on it says.
#[test]
fn a_step_holds_in_its_readings_health_and_steps_in_the_worse() {
    let dir = tempfile::tempdir().unwrap();
    let detail = flips(dir.path(), &["a", "b", "c", "d"]);
    let plot = Plot::of(&detail);
    let healths: Vec<Option<Health>> = plot.segments(&detail).iter().map(|(_, h)| *h).collect();
    let (ok, down) = (Some(Health::Ok), Some(Health::Down));
    assert_eq!(healths, [ok, ok, ok, down, down, down]);

    let (mut screen, _dir) = opened(detail);
    let (text, buffer) = render(&mut screen, 100, 40);
    let row = text
        .lines()
        .position(|line| line.starts_with("yes"))
        .unwrap();
    let colours: Vec<Color> = (0..buffer.area.width)
        .map(|x| &buffer[(x, u16::try_from(row).unwrap())])
        .filter(|cell| {
            cell.symbol()
                .chars()
                .all(|c| ('\u{2801}'..='\u{28FF}').contains(&c))
        })
        .map(|cell| cell.fg)
        .collect();
    let green = colours.iter().filter(|fg| **fg == Color::Green).count();
    let red = colours.iter().filter(|fg| **fg == Color::Red).count();
    assert!(
        green > 2 * red + 20,
        "the plateaus before the drop are green:\n{text}"
    );
}

/// §FS-021-monitors.19: the strip of health lies under the chart's plot,
/// one cell per column over the same span, so a flip on the chart sits over
/// the cell that turns; the change has a line of its own.
#[test]
fn the_strip_of_health_lines_up_under_the_chart() {
    let dir = tempfile::tempdir().unwrap();
    let (mut screen, _dir) = opened(flips(dir.path(), &["a", "b", "c", "d"]));
    let (text, _) = render(&mut screen, 100, 40);
    let lines: Vec<&str> = text.lines().collect();
    let corner = lines
        .iter()
        .find_map(|line| line.chars().position(|c| c == '└'))
        .unwrap();
    let at = lines
        .iter()
        .position(|line| line.starts_with("health:"))
        .unwrap();
    let cells: Vec<char> = lines[at].chars().skip(corner + 1).collect();
    assert_eq!(cells.len(), 100 - corner - 1, "{text}");
    let glyph = |health| {
        Cell {
            mark: Mark::Read,
            value: None,
            health: Some(health),
            height: None,
        }
        .health_glyph()
    };
    let wide = cells.len();
    assert!(
        cells[..wide * 6 / 10]
            .iter()
            .all(|c| *c == glyph(Health::Ok)),
        "{text}"
    );
    assert!(
        cells[wide * 7 / 10..wide * 95 / 100]
            .iter()
            .all(|c| *c == glyph(Health::Down)),
        "{text}"
    );
    assert!(lines[at + 1].trim_start().starts_with("change "), "{text}");
}

/// §FS-021-monitors.19: the y axis is labelled in round numbers in the
/// row's unit, never a float's every digit, and starts at zero rather than
/// below it where nothing read is negative.
#[test]
fn the_y_axis_says_round_numbers() {
    use crate::feed::tui::monitors_chart::axis;
    assert_eq!(axis([3.9, 4.2, 4.8].into_iter()), [3.0, 4.0, 5.0]);
    assert_eq!(axis([0.1, 0.3].into_iter()), [0.0, 0.2, 0.4]);
    assert_eq!(axis([0.0, 10.0].into_iter()), [0.0, 10.0, 20.0]);
    assert_eq!(axis([-4.0, 4.0].into_iter()), [-5.0, 0.0, 5.0]);
    let dir = tempfile::tempdir().unwrap();
    let mut detail = latency_detail(dir.path());
    detail.thresholds.clear();
    for (point, value) in detail.chart.iter_mut().zip([3.7, 3.9, 4.6]) {
        point.value = Some(value);
    }
    assert_eq!(Plot::of(&detail).y_labels, ["3 ms", "4 ms", "5 ms"]);
}

/// §FS-021-monitors.19: a series' middle x label is drawn at the middle of
/// the axis, so it is said only where a point stands there.
#[test]
fn a_series_middle_label_names_the_point_at_the_middle() {
    let dir = tempfile::tempdir().unwrap();
    let even = Plot::of(&flips(dir.path(), &["a", "b", "c", "d"]));
    assert_eq!(even.x_labels, ["a", "d"]);
    let odd = Plot::of(&flips(dir.path(), &["a", "b", "c", "d", "e"]));
    assert_eq!(odd.x_labels, ["a", "c", "e"]);
}

/// §FS-021-monitors.19: the episodes, readings and points are laid out once
/// per reading of the monitor and width, not once per frame.
#[test]
fn the_detail_is_laid_out_once_per_reading_and_width() {
    let dir = tempfile::tempdir().unwrap();
    let (mut screen, _dir) = opened(latency_detail(dir.path()));
    let laid = |screen: &MonitorsScreen| {
        screen
            .drawn
            .as_ref()
            .and_then(|drawn| drawn.sections.as_ref())
            .map(|(width, lines)| (*width, lines.as_ptr()))
    };
    render(&mut screen, 120, 40);
    let first = laid(&screen).unwrap();
    render(&mut screen, 120, 40);
    assert_eq!(laid(&screen), Some(first), "the same lines, not laid again");
    render(&mut screen, 90, 40);
    assert_eq!(laid(&screen).map(|(width, _)| width), Some(90));
    screen.show(latency_detail(&dir.path().join("again")));
    assert!(laid(&screen).is_none(), "a new reading is laid afresh");
}

/// §FS-021-monitors.18.4: a narrow screen gives up the condition, then
/// the strip, before it cuts a title past twelve characters; a wide one
/// draws every column.
#[test]
fn a_narrow_screen_gives_up_columns_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let page = page_of_projects(dir.path());
    let shapes: Vec<Shape> = (60..=240)
        .rev()
        .map(|width| Columns::fit(&page, width).shape)
        .collect();
    assert_eq!(shapes[0], Shape::One);
    let under = shapes.iter().position(|shape| *shape == Shape::Under);
    let split = shapes.iter().position(|shape| *shape == Shape::Split);
    assert!(under.is_some() && under < split, "{shapes:?}");
    assert!(shapes[split.unwrap()..]
        .iter()
        .all(|shape| *shape == Shape::Split));
    let narrow = Columns::fit(&page, 80);
    assert_eq!(narrow.shape, Shape::Split);
    assert_eq!(narrow.title, TITLE.min(narrow.title), "{narrow:?}");
}

/// §FS-021-monitors.18.4, §FS-021-monitors.4.5, §FS-021-monitors.4.6: an
/// 80-column page keeps every column: a row too wide for one line goes on
/// to a second, where the strip, the change and the whole condition are
/// said, a not-read row's sentence and the row's notes included; titles
/// are clipped only past the page's 25 characters, nothing is drawn past
/// the edge, and the cursor's row is lit on both of its lines.
#[test]
fn an_80_column_page_puts_what_one_line_cannot_hold_on_a_second() {
    let dir = tempfile::tempdir().unwrap();
    let mut page = page_of_six(dir.path());
    let sentence = "not read: its healthy says is, which a number does not take";
    page.projects[0].rows[0].shows.health.sentence = Some(sentence.into());
    page.projects[0].rows[0].notes = vec!["unknown field heathy".into()];
    page.projects[0].rows[1].title = "Build green on every supported platform".into();
    let cell = |height: u8, health| Cell {
        mark: Mark::Read,
        value: Some(f64::from(height)),
        health: Some(health),
        height: Some(height),
    };
    page.projects[0].rows[0].shows.strip = vec![cell(0, Health::Down); 24];
    page.projects[0].rows[0].shows.strip[23] = cell(7, Health::Ok);
    let mut screen = screen(page);
    let (text, buffer) = render(&mut screen, 80, 24);
    assert!(
        text.lines().all(|line| line.chars().count() <= 80),
        "{text}"
    );
    let lines: Vec<&str> = text.lines().collect();
    let first = lines.iter().position(|line| line.starts_with('✖')).unwrap();
    let second = lines[first + 1];
    assert!(
        second.contains('▁') || second.contains('█'),
        "the strip:\n{text}"
    );
    let said: String = lines[first + 1..]
        .iter()
        .take_while(|line| line.starts_with("    "))
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(said.contains(sentence), "{text}");
    assert!(said.contains("unknown field heathy"), "{text}");
    assert!(text.contains("Build green on every sup…"), "{text}");
    assert!(text.contains("last 7d"), "the strip's heading:\n{text}");
    let lit = |y: usize| buffer[(2, u16::try_from(y).unwrap())].bg;
    let next = lines.iter().position(|line| line.starts_with('?')).unwrap();
    assert!(
        (first..next).all(|y| lit(y) == lit(first)),
        "every line of the cursor's row"
    );
    assert_ne!(lit(first), lit(next), "the next row is not lit");

    // Scrolled to the last row, both of its lines are on screen.
    screen.handle_key(KeyCode::Char('G'));
    let (text, _) = render(&mut screen, 80, 10);
    let lines: Vec<&str> = text.lines().collect();
    let last = lines.iter().position(|line| line.starts_with('◆')).unwrap();
    assert!(
        lines.get(last + 1).is_some_and(|line| line.contains("KPI")),
        "{text}"
    );
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
    assert!(first.contains("Line coverage of every c…"), "{first}");
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
    assert!(text.contains("bounds:  ── healthy ≤ 50 ms"), "{text}");
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

/// §FS-021-monitors.4.6, §FS-021-monitors.7.1: a row's notes follow its
/// condition on the page, as `ephor monitors` prints them, so a misspelt
/// `healthy` is seen on the row and not only in the detail view.
#[test]
fn a_rows_notes_follow_its_condition_on_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let mut page = page_of_six(dir.path());
    page.projects[0].rows[0].notes = vec!["unknown field heathy".into()];
    let mut screen = screen(page);
    let (text, _) = render(&mut screen, 240, 20);
    let row = text
        .lines()
        .find(|line| line.contains("Line coverage"))
        .unwrap();
    assert!(
        row.contains("healthy ≥ 80% · unknown field heathy"),
        "{row}"
    );
}

/// §FS-021-monitors.18.5: where a row's last 24 readings reach further back
/// than the window, the strip's heading says so.
#[test]
fn the_strip_heading_names_the_readings_that_set_a_span() {
    let dir = tempfile::tempdir().unwrap();
    let mut page = page_of_six(dir.path());
    page.projects[0].rows[3].shows.span.by_readings = true;
    let (text, _) = render(&mut screen(page), 240, 20);
    assert!(text.contains("last 7d or 24 readings"), "{text}");
}
