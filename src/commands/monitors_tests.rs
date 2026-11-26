//! The page as the command draws it (§FS-021-monitors.21.1): its columns,
//! its clipped titles and its colours.

use chrono::{DateTime, Utc};

use super::*;
use crate::api::monitors::tests::page_of_six;
use crate::monitors::strip::Change;

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-25T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn row<'a>(lines: &'a [String], title: &str) -> &'a str {
    lines
        .iter()
        .find(|line| line.contains(title))
        .unwrap_or_else(|| panic!("no row for {title} in {lines:#?}"))
}

/// §FS-021-monitors.21.1: `health` says the health, with the page's health
/// column beside it where that says more, and `since` how long it has held.
#[test]
fn the_health_and_since_columns_say_the_health_and_how_long_it_held() {
    let dir = tempfile::tempdir().unwrap();
    let lines = page_lines(&page_of_six(dir.path()), now(), &Style::colored(false));
    let heading = row(&lines, "widget");
    assert!(
        heading.contains("value  ") && heading.contains("health") && heading.contains("since"),
        "{heading}"
    );
    let words = |title: &str| -> Vec<String> {
        row(&lines, title)
            .split("  ")
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .map(str::to_string)
            .collect()
    };
    assert_eq!(
        words("Line coverage")[..5],
        ["✖ Line coverage", "79.2%", "1h", "down", "2d"]
    );
    assert_eq!(
        words("Build green")[..5],
        ["? Build green", "—", "1h", "unknown (overdue 4h)", "3h"]
    );
    assert_eq!(words("Flaky tests")[3..5], ["failing 1 of 2", "2d"]);
    assert_eq!(words("Docs build")[3..5], ["warn", "2d"]);
    assert_eq!(words("Lines of code")[3..5], ["KPI", "—"]);
}

/// A title longer than 25 characters is clipped with `…`, so one long title
/// does not push every row of the page out of line.
#[test]
fn a_long_title_is_clipped_and_the_columns_stay_in_line() {
    let dir = tempfile::tempdir().unwrap();
    let mut page = page_of_six(dir.path());
    page.projects[0].rows[0].title =
        "Pull requests merged through the queue without a retry, weekly".into();
    let lines = page_lines(&page, now(), &Style::colored(false));
    let long = row(&lines, "Pull requests");
    assert!(long.starts_with("✖ Pull requests merged thr…  "), "{long}");
    let starts: Vec<usize> = ["79.2%", "42 ms", "41000"]
        .iter()
        .map(|value| {
            let line = row(&lines, value);
            line[..line.find(value).unwrap()].chars().count()
        })
        .collect();
    assert_eq!(starts, [29, 29, 29], "{lines:#?}");
}

/// §FS-021-monitors.3.7, §FS-021-monitors.18.6: the glyph and the health
/// are coloured by the rank, and the change by `better`, only where colour
/// is wanted; the glyph says the same in shape without it.
#[test]
fn the_glyph_health_and_change_are_coloured_only_where_colour_is_wanted() {
    let dir = tempfile::tempdir().unwrap();
    let mut page = page_of_six(dir.path());
    for row in &mut page.projects[0].rows {
        if row.id == "latency" {
            row.shows.change = Change {
                says: "▾ 2 ms".into(),
                way: Way::Better,
            };
        }
        if row.id == "flaky" {
            row.shows.change = Change {
                says: "▴ 2".into(),
                way: Way::Worse,
            };
        }
    }
    let lines = page_lines(&page, now(), &Style::colored(true));
    let red = |text: &str| format!("\x1b[31m{text}\x1b[0m");
    let green = |text: &str| format!("\x1b[32m{text}\x1b[0m");
    let amber = |text: &str| format!("\x1b[33m{text}\x1b[0m");
    let dim = |text: &str| format!("\x1b[2m{text}\x1b[0m");
    let coverage = row(&lines, "Line coverage");
    assert!(coverage.starts_with(&red("✖")), "{coverage:?}");
    assert!(coverage.contains(&red("down")), "{coverage:?}");
    let latency = row(&lines, "Latency");
    assert!(latency.starts_with(&green("●")), "{latency:?}");
    assert!(latency.contains(&green("▾ 2 ms")), "{latency:?}");
    let flaky = row(&lines, "Flaky tests");
    assert!(flaky.starts_with(&amber("▼")), "{flaky:?}");
    assert!(flaky.contains(&amber("▴ 2")), "{flaky:?}");
    assert!(row(&lines, "Docs build").starts_with(&amber("▲")));
    assert!(row(&lines, "Build green").starts_with(&dim("?")));
    assert!(row(&lines, "Lines of code").starts_with(&dim("◆")));

    let plain = page_lines(&page, now(), &Style::colored(false));
    assert!(
        plain.iter().all(|line| !line.contains('\x1b')),
        "{plain:#?}"
    );
    let column = |lines: &[String], title: &str| {
        let line = row(lines, title);
        let bare = line.replace("\x1b[32m", "").replace("\x1b[31m", "");
        let bare = bare.replace("\x1b[33m", "").replace("\x1b[2m", "");
        bare.replace("\x1b[0m", "")
    };
    assert_eq!(
        column(&lines, "Latency"),
        column(&plain, "Latency"),
        "colour adds no width"
    );
}
