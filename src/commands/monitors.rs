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
    println!("{}", page.header(now));
    if page.projects.is_empty() {
        println!();
        println!("{NO_MONITORS}");
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
        .map(|group| group.project.chars().count())
        .chain(page.rows().map(|row| row.title.chars().count()))
        .max()
        .unwrap_or(0)
        .max(24);
    let mut rows = rows.into_iter();
    for group in &page.projects {
        println!();
        let heading: Vec<String> = match group.rows.is_empty() {
            true => Vec::new(),
            false => COLUMNS.iter().map(|name| name.to_string()).collect(),
        };
        println!("  {}", line(&group.project, &heading, title, &widths));
        for row in &group.rows {
            let cells = rows.next().unwrap_or_default();
            println!(
                "{} {}",
                row.shows.glyph,
                line(&row.title, &cells, title, &widths)
            );
        }
        for note in &group.notes {
            println!("  note: {note}");
        }
    }
    for project in &page.without {
        println!();
        println!("  {project} has no monitors");
    }
    if !page.is_empty() {
        println!();
        println!("  {FOOTNOTE}");
    }
}

/// One line of the table: the title padded, then each column padded but
/// the last.
fn line(title: &str, cells: &[String], title_width: usize, widths: &[usize; 6]) -> String {
    let mut said = pad(title, title_width + 2);
    for (at, (cell, width)) in cells.iter().zip(widths).enumerate() {
        match at + 1 == widths.len() {
            true => said.push_str(cell),
            false => said.push_str(&pad(cell, width + 2)),
        }
    }
    said.trim_end().to_string()
}

fn pad(text: &str, width: usize) -> String {
    let short = width.saturating_sub(text.chars().count());
    format!("{text}{}", " ".repeat(short))
}

/// A row's cells after its title.
fn cells(row: &MonitorRow) -> [String; 6] {
    let shows = &row.shows;
    [
        shows.value.clone(),
        shows.every.clone(),
        shows.health.state.clone(),
        shows
            .health
            .lasted
            .clone()
            .unwrap_or_else(|| "—".to_string()),
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
    let since = row.since.map(|since| format!(" · since {}", when(since)));
    println!(
        "{} {}{}",
        shows.glyph,
        shows.health.text(),
        since.unwrap_or_default()
    );
    println!("value: {} · {}", shows.value, row.condition_says());
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
        println!("points: {}", detail.points.len());
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
