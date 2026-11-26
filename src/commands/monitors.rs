//! `ephor monitors` and `ephor monitors show` (§FS-021-monitors.21): the
//! Monitors page as a table and one monitor's detail view, rendered from the
//! session's readings. Presentation only (§AR-009-surfaces.4): what each
//! row says is the reading's, and only the layout is decided here.

use std::process::ExitCode;

use chrono::{DateTime, Local, Utc};

use super::emit;
use crate::api::monitor_views::{MonitorDetail, MonitorRow, Monitors, NO_MONITORS};
use crate::api::Session;
use crate::cli::{MonitorsArgs, MonitorsCommand, MonitorsShowArgs};
use crate::error::{EphorError, Result};
use crate::feed::commands::known_project;
use crate::feed::config::load_config;
use crate::feed::render::Style;
use crate::monitors::page::Rank;
use crate::monitors::strip::Way;
use crate::scope::Projects;

/// The footnote under the table (§FS-021-monitors.21.1).
const FOOTNOTE: &str =
    "change: over the window, or over the last 24 readings when those reach further back";

/// The table's columns after the title (§FS-021-monitors.21.1).
const COLUMNS: [&str; 6] = ["value", "every", "health", "since", "change", "condition"];

/// `ephor monitors` (§FS-021-monitors.21.1, §FS-021-monitors.21.2): the
/// page, `4` under `--check` where a status it prints is down or unknown.
/// It reads the store and the cache and fetches nothing
/// (§FS-021-monitors.21.4).
pub fn monitors(args: &MonitorsArgs, scope: &Projects) -> Result<ExitCode> {
    if let Some(MonitorsCommand::Show(show)) = &args.command {
        return monitor(show);
    }
    let config = load_config()?;
    for project in &args.project {
        scope.admit(project)?;
        known_project(&config, project)?;
    }
    let mut session = Session::open_over(&config, scope)?;
    let page = session.monitors(args.window, &args.project);
    let failed = args.check && page.fails_check();
    if args.json {
        emit(&page);
    } else {
        print_page(&page, Utc::now());
    }
    Ok(match failed {
        true => ExitCode::from(4),
        false => ExitCode::SUCCESS,
    })
}

fn print_page(page: &Monitors, now: DateTime<Utc>) {
    for line in page_lines(page, now, &Style::detect()) {
        println!("{line}");
    }
}

/// The page as lines of text, coloured by `style`: the glyph and the
/// health by the row's rank and the change by `better`
/// (§FS-021-monitors.3.7, §FS-021-monitors.18.6); the glyph and the strip
/// carry the same in shape, so the table reads without colour.
fn page_lines(page: &Monitors, now: DateTime<Utc>, style: &Style) -> Vec<String> {
    let mut lines = vec![page.header(now)];
    if page.projects.is_empty() {
        lines.push(String::new());
        lines.push(NO_MONITORS.to_string());
    }
    let rows: Vec<[String; 6]> = page.rows().map(cells).collect();
    let mut widths = COLUMNS.map(str::len);
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let title = page
        .projects
        .iter()
        .map(|group| clipped(&group.project).chars().count())
        .chain(page.rows().map(|row| clipped(&row.title).chars().count()))
        .max()
        .unwrap_or(0)
        .max(TITLE - 1);
    let mut rows = rows.into_iter();
    for group in &page.projects {
        lines.push(String::new());
        let heading: Vec<String> = match group.rows.is_empty() {
            true => Vec::new(),
            false => COLUMNS.iter().map(|name| name.to_string()).collect(),
        };
        let plain = |_: usize, cell: &str| cell.to_string();
        lines.push(format!(
            "  {}",
            line(&clipped(&group.project), &heading, title, &widths, plain)
        ));
        for row in &group.rows {
            let cells = rows.next().unwrap_or_default();
            let paint = |at: usize, cell: &str| match at {
                2 => ranked(style, row.shows.rank, cell),
                4 => match row.shows.change.way {
                    Way::Better => style.green(cell),
                    Way::Worse => style.amber(cell),
                    Way::Plain => cell.to_string(),
                },
                _ => cell.to_string(),
            };
            lines.push(format!(
                "{} {}",
                ranked(style, row.shows.rank, row.shows.glyph),
                line(&clipped(&row.title), &cells, title, &widths, paint)
            ));
        }
        for note in &group.notes {
            lines.push(format!("  note: {note}"));
        }
    }
    for project in &page.without {
        lines.push(String::new());
        lines.push(format!("  {project} has no monitors"));
    }
    if !page.is_empty() {
        lines.push(String::new());
        lines.push(format!("  {FOOTNOTE}"));
    }
    lines
}

/// The colour a rank is shown in: down red, failing and warn amber, ok
/// green, and unknown and a KPI dim (§FS-021-monitors.3.7).
fn ranked(style: &Style, rank: Rank, text: &str) -> String {
    match rank {
        Rank::Down => style.red(text),
        Rank::Failing | Rank::Warn => style.amber(text),
        Rank::Ok => style.green(text),
        Rank::Unknown | Rank::Kpi => style.dim(text),
    }
}

/// The widest a title is printed, `…` standing for the rest, so one long
/// title does not push every row of the page out of line.
const TITLE: usize = 25;

fn clipped(title: &str) -> String {
    match title.chars().count() > TITLE {
        true => title.chars().take(TITLE - 1).chain(['…']).collect(),
        false => title.to_string(),
    }
}

/// One line of the table: the title padded, then each column padded but
/// the last, each cell painted by `paint` after it is measured.
fn line(
    title: &str,
    cells: &[String],
    title_width: usize,
    widths: &[usize; 6],
    paint: impl Fn(usize, &str) -> String,
) -> String {
    let mut said = pad(title, title_width + 2);
    for (at, (cell, width)) in cells.iter().zip(widths).enumerate() {
        said.push_str(&paint(at, cell));
        if at + 1 < widths.len() {
            said.push_str(&" ".repeat((width + 2).saturating_sub(cell.chars().count())));
        }
    }
    said.trim_end().to_string()
}

fn pad(text: &str, width: usize) -> String {
    let short = width.saturating_sub(text.chars().count());
    format!("{text}{}", " ".repeat(short))
}

/// A row's cells after its title: its value, its cadence, its health and
/// how long that has held, its change, and its condition with its notes
/// (§FS-021-monitors.21.1).
fn cells(row: &MonitorRow) -> [String; 6] {
    let shows = &row.shows;
    [
        shows.value.clone(),
        shows.every.clone(),
        row.health_says(),
        shows.held.clone().unwrap_or_else(|| "—".to_string()),
        shows.change.says.clone(),
        // §FS-021-monitors.4.6, §FS-021-monitors.7.1: the row's notes follow its condition.
        std::iter::once(row.condition_says())
            .chain(row.notes_say(when))
            .collect::<Vec<_>>()
            .join(" · "),
    ]
}

/// `ephor monitors show <project>/<id>` (§FS-021-monitors.21.3).
fn monitor(args: &MonitorsShowArgs) -> Result<ExitCode> {
    let Some((project, id)) = args
        .monitor
        .split_once('/')
        .filter(|(project, id)| !project.is_empty() && !id.is_empty())
    else {
        return Err(EphorError::Command(format!(
            "'{}' does not name a monitor: write it as <project>/<id>.",
            args.monitor
        )));
    };
    let config = load_config()?;
    known_project(&config, project)?;
    let mut session = Session::open_over(&config, &Projects::every())?;
    let detail = session
        .monitor(project, id, args.window)
        .map_err(EphorError::Command)?;
    if args.json {
        emit(&detail);
    } else {
        print_detail(&detail, Utc::now());
    }
    Ok(ExitCode::SUCCESS)
}

fn when(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

fn print_detail(detail: &MonitorDetail, now: DateTime<Utc>) {
    let row = &detail.row;
    let shows = &row.shows;
    println!(
        "{}   {} · {}",
        detail.title(),
        detail.window_says(),
        crate::api::monitor_views::refreshed(detail.refreshed_at, now)
    );
    println!();
    println!("{}", detail.health_says(when));
    if let Some(sentence) = &shows.health.sentence {
        println!("{sentence}");
    }
    println!("value: {}", shows.value);
    println!("condition: {}", row.condition.parts_say());
    if let Some(said) = &row.detail {
        println!("detail: {said}");
    }
    for note in row.notes_say(when) {
        println!("note: {note}");
    }
    if let Some(description) = &detail.description {
        println!("about: {description}");
    }
    if let Some(from) = &detail.from {
        let source = match &from.source {
            crate::monitors::model::Source::Command { command, .. } => format!("command {command}"),
            crate::monitors::model::Source::File { file, .. } => format!("file {file}"),
        };
        let runs = from
            .runs_in
            .as_ref()
            .map(|dir| format!(" · in {}", dir.display()))
            .unwrap_or_default();
        println!("from: {source}{runs}");
    }
    println!("declared: {}", detail.declared.says());
    println!("cadence: {}", row.cadence_says(when));
    if let Some(freshness) = row.freshness.says(when) {
        println!("freshness: {freshness}");
    }
    if let Some(hold) = &detail.hold {
        println!("hold: {}", hold.says());
    }
    let strip: String = shows.strip.iter().map(|cell| cell.glyph()).collect();
    println!("strip: {strip}  change {}", shows.change.says);
    if !detail.points.is_empty() {
        println!();
        println!("points ({})", detail.points.len());
        for point in &detail.points {
            let at = match (&point.t, &point.x) {
                (Some(t), _) => when(t.with_timezone(&Utc)),
                (None, Some(x)) => x.clone(),
                (None, None) => "—".to_string(),
            };
            println!("  {at:<16}  {}", row.say(point.v));
        }
    }
    println!();
    println!("episodes ({})", detail.episodes.len());
    for episode in &detail.episodes {
        let end = episode.end.map_or_else(|| "open".to_string(), when);
        println!(
            "  {:<7} {} → {}  {}",
            match episode.sort {
                crate::monitors::episode::Sort::Drop => "drop",
                crate::monitors::episode::Sort::Unknown => "unknown",
            },
            when(episode.start),
            end,
            episode.matter
        );
    }
    println!();
    println!("readings ({})", detail.readings.len());
    for reading in &detail.readings {
        let health = reading.health.map_or("—", |health| health.name());
        let note = reading
            .note
            .as_ref()
            .map(|note| format!("  {note}"))
            .unwrap_or_default();
        let said = format!(
            "  {}  {:<12} {:<8}{note}",
            when(reading.t),
            reading.value,
            health
        );
        println!("{}", said.trim_end());
    }
    println!();
    println!("matter: {}", detail.matter_says());
}

#[cfg(test)]
#[path = "monitors_tests.rs"]
mod tests;
