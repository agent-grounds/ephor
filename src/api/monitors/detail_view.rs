//! What one monitor's detail view adds to its row (§FS-021-monitors.19):
//! where it is read from and declared, its hold, the chart of the span under
//! today's condition, the episodes inside the window and the readings, and
//! why there is no matter where there is none.

use std::collections::BTreeSet;
use std::path::Path;

use chrono::{Local, Utc};

use super::views;
use super::{series_points, Built, Gathered};
use crate::monitors::condition::Judge;
use crate::monitors::declared::Declarations;
use crate::monitors::episode::{self, Sort};
use crate::monitors::history::Line;
use crate::monitors::known::Known;
use crate::monitors::model::{Health, Home, Kind, Point, Source, Value};
use crate::monitors::page::{Rank, Standing};
use crate::monitors::state::Record;
use crate::monitors::strip::{self, Span};
use crate::monitors::window::Window;
use crate::monitors::words;
use crate::seams::summons::{Place, Site};

/// Everything the detail view adds to the row (§FS-021-monitors.19), with
/// `runs_in` the checkout its command runs or its file is read in.
pub(super) fn detail(
    project: &str,
    known: &Known,
    gathered: &Gathered,
    built: Built,
    window: Window,
    runs_in: Option<&Path>,
) -> views::MonitorDetail {
    let Built {
        row,
        standing,
        lines,
        drawn,
        span,
        judge,
    } = built;
    let monitor = &known.monitor;
    let record = gathered.document.monitors.get(&monitor.id);
    let series: Vec<Point> = series_points(row.kind, record)
        .map(<[Point]>::to_vec)
        .unwrap_or_default();
    let windowed = !series.iter().any(|point| point.t.is_none());
    // §FS-021-monitors.19: the repository's points, a number's as a series'.
    let points = match (row.kind, record.and_then(Record::points)) {
        (Some(Kind::Number), Some(supplied)) => supplied.to_vec(),
        _ => series.clone(),
    };
    let from = monitor.from.clone().map(|source| {
        let cwd = match &source {
            Source::Command { cwd, .. } | Source::File { cwd, .. } => cwd.as_deref(),
        };
        // §FS-021-monitors.5.3: where the reader runs it, said without asking the disk.
        let place = cwd.map_or(Ok(Place::Root), Place::parse).ok();
        views::FromView {
            runs_in: runs_in
                .zip(place)
                .map(|(root, place)| Site::root(root).place_of(&place)),
            home: monitor.origins.get("from").copied(),
            source,
        }
    });
    views::MonitorDetail {
        window,
        windowed,
        refreshed_at: None,
        description: monitor.description.clone(),
        from,
        declared: declared_view(known, &gathered.declarations),
        hold: standing.status.then(|| views::HoldView {
            drop_after: monitor.drop_after,
            recover_after: monitor.recover_after(&known.cadence),
        }),
        chart: chart(&drawn, &span, &series, windowed, known, &judge),
        steps: steps(row.kind, monitor.levels.as_deref()),
        thresholds: thresholds(&judge, known),
        episodes: episodes(project, &monitor.id, record, &span),
        readings: readings(&lines, &span, windowed, known, (&judge, row.of)),
        points,
        row,
    }
}

/// Every home that says something of it, and the fields the site names
/// where the manifest also does (§FS-021-monitors.19).
fn declared_view(known: &Known, declarations: &Declarations) -> views::DeclaredView {
    let monitor = &known.monitor;
    let homes: BTreeSet<Home> = monitor.origins.values().copied().collect();
    let manifest = declarations
        .manifest
        .get(&monitor.id)
        .and_then(|declaration| serde_json::to_value(declaration).ok());
    let site_overrides = monitor
        .origins
        .iter()
        .filter(|(_, home)| **home == Home::Site)
        .filter(|(field, _)| {
            manifest
                .as_ref()
                .is_some_and(|manifest| manifest.get(**field).is_some())
        })
        .map(|(field, _)| field.to_string())
        .collect();
    views::DeclaredView {
        homes: homes.into_iter().collect(),
        site_overrides,
    }
}

/// Why a monitor has no open matter (§FS-021-monitors.19).
pub(super) fn no_matter(known: &Known, standing: &Standing) -> String {
    let monitor = &known.monitor;
    if !monitor.faults.is_empty() {
        return "none: a monitor whose declaration is in error is not read".to_string();
    }
    if monitor.not_run {
        return format!("none: {}", crate::monitors::overlay::NOT_RUN);
    }
    if !standing.status {
        return "none: a KPI is never a matter".to_string();
    }
    if standing.overdue {
        return "none: overdue is the watch's own failure, not the project's".to_string();
    }
    match standing.rank {
        Rank::Failing => "none yet: it drops after its hold".to_string(),
        _ => "none: nothing is open".to_string(),
    }
}

/// The chart's points: the span's lines judged under today's condition, or
/// a series' points (§FS-021-monitors.19).
fn chart(
    lines: &[Line],
    span: &Span,
    points: &[Point],
    windowed: bool,
    known: &Known,
    judge: &Judge,
) -> Vec<views::ChartPoint> {
    let monitor = &known.monitor;
    let levels = monitor.levels.as_deref();
    if !points.is_empty() {
        return points
            .iter()
            .filter(|point| !windowed || point.t.is_none_or(|t| span.holds(t.with_timezone(&Utc))))
            .map(|point| views::ChartPoint {
                t: point.t.map(|t| t.with_timezone(&Utc)),
                x: point.x.clone(),
                value: Some(point.v),
                health: crate::monitors::condition::band(
                    judge,
                    monitor.warn.as_ref(),
                    &Value::Number(point.v),
                    None,
                    levels,
                )
                .map(Health::from),
            })
            .collect();
    }
    lines
        .iter()
        .filter(|line| span.holds(line.t()))
        .map(|line| views::ChartPoint {
            t: Some(line.t()),
            x: None,
            value: strip::scaled(line, levels),
            health: crate::monitors::history::rejudged(line, judge, monitor.warn.as_ref(), levels),
        })
        .collect()
}

/// What the places of a boolean's or a level's chart are called, from place
/// 0 up, as [`strip::scaled`] places them: `no` and `yes`, or the levels in
/// the order declared.
fn steps(kind: Option<Kind>, levels: Option<&[String]>) -> Vec<String> {
    match kind {
        Some(Kind::Boolean) => [false, true]
            .map(|yes| words::value(&Value::Boolean(yes), None, None))
            .to_vec(),
        Some(Kind::Level) => levels.map(<[String]>::to_vec).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Today's `healthy` and `warn` bounds, as threshold lines.
fn thresholds(judge: &Judge, known: &Known) -> Vec<views::Threshold> {
    let mut lines = Vec::new();
    let conditions = [
        ("healthy", judge.condition()),
        ("warn", known.monitor.warn.as_ref()),
    ];
    for (of, condition) in conditions {
        let Some(condition) = condition else {
            continue;
        };
        for (bound, at) in [("min", condition.min), ("max", condition.max)] {
            if let Some(at) = at {
                lines.push(views::Threshold { at, of, bound });
            }
        }
    }
    lines
}

/// Every reading in the span, newest first, each with a note where it
/// was read out of cadence, failed, or was thinned (§FS-021-monitors.19).
fn readings(
    lines: &[Line],
    span: &Span,
    windowed: bool,
    known: &Known,
    (judge, of): (&Judge, Option<f64>),
) -> Vec<views::ReadingView> {
    let monitor = &known.monitor;
    let levels = monitor.levels.as_deref();
    // The whole the last reading is a fraction of, as the row says its value.
    let said = |value| words::value(value, monitor.unit.as_deref(), of);
    lines
        .iter()
        .rev()
        .filter(|line| !windowed || span.holds(line.t()))
        .map(|line| {
            let health =
                crate::monitors::history::rejudged(line, judge, monitor.warn.as_ref(), levels);
            let (value, note) = match line {
                Line::Reading {
                    v, out_of_cadence, ..
                } => (
                    said(v),
                    out_of_cadence.then(|| "read out of cadence".to_string()),
                ),
                Line::Marker { .. } => ("—".to_string(), Some("the read failed".to_string())),
                Line::Thinned { min, max, n, .. } => (
                    format!("{}–{}", said(min), said(max)),
                    Some(format!("{n} readings thinned")),
                ),
            };
            views::ReadingView {
                t: line.t(),
                value,
                health,
                note,
            }
        })
        .collect()
}

/// The episodes inside the span, open ones included, oldest first
/// (§FS-021-monitors.19).
fn episodes(
    project: &str,
    id: &str,
    record: Option<&Record>,
    span: &Span,
) -> Vec<views::EpisodeView> {
    let Some(record) = record else {
        return Vec::new();
    };
    let mut episodes: Vec<views::EpisodeView> = record
        .past
        .iter()
        .filter(|past| past.ended >= span.from)
        .map(|past| views::EpisodeView {
            sort: past.sort,
            start: past.named,
            end: Some(past.ended),
            matter: episode::key(project, id, past.sort, past.named),
            ended: Some(past.how.say(past.ended, &Local)),
        })
        .collect();
    let open = record
        .drop
        .as_ref()
        .map(|drop| (Sort::Drop, drop.named))
        .into_iter()
        .chain(
            record
                .unknown
                .as_ref()
                .map(|unknown| (Sort::Unknown, unknown.named)),
        );
    for (sort, named) in open {
        episodes.push(views::EpisodeView {
            sort,
            start: named,
            end: None,
            matter: episode::key(project, id, sort, named),
            ended: None,
        });
    }
    episodes.sort_by_key(|episode| episode.start);
    episodes
}
