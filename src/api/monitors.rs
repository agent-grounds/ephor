//! The Monitors page and one monitor's detail view, below both surfaces
//! (§FS-021-monitors.18, §FS-021-monitors.19, §AR-009-surfaces.1).
//!
//! One derivation, so the page `M` opens and what `ephor monitors --json`
//! prints cannot come apart (§REQ-002-parity.2). It reads the store, the
//! declarations and the cache, and fetches, runs and writes nothing
//! (§FS-021-monitors.21.1). Where each monitor stands is the core's
//! [`page::standing`], and what its strip and change cover is
//! [`strip`]'s; this module only gathers what they judge from and says it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::{DateTime, Local, TimeDelta, Utc};
use serde_json::{json, Value as Json};

use super::monitor_views::{
    self as views, CadenceView, ConditionView, FreshnessView, MatterView, MonitorProject,
    MonitorRow, PageCounts, Shows,
};
use crate::monitors::condition::Judge;
use crate::monitors::declared::{self, Declarations};
use crate::monitors::episode::{self, Sort};
use crate::monitors::freshness::{self, Cause, Freshness};
use crate::monitors::history::{self, Line};
use crate::monitors::known::{self, Known};
use crate::monitors::model::{Health, Home, Kind, Point, Source, Value};
use crate::monitors::overlay::Healthy;
use crate::monitors::page::{self, Rank, Standing};
use crate::monitors::state::{Document, Record};
use crate::monitors::store::Store;
use crate::monitors::strip::{self, Span};
use crate::monitors::window::Window;
use crate::monitors::words;
use crate::seams::summons::{Place, Site};

/// What one project's monitors are judged from.
struct Gathered {
    declarations: Declarations,
    document: Document,
    known: BTreeMap<String, Known>,
    store: Store,
    slot_error: Option<String>,
    notes: Vec<String>,
}

/// One row, and what the detail view draws beside it.
struct Built {
    row: MonitorRow,
    standing: Standing,
    /// Ephor's own lines, which the readings list.
    lines: Vec<Line>,
    /// What the strip and the chart draw: a number's supplied points before
    /// ephor's own lines (§FS-021-monitors.2.2).
    drawn: Vec<Line>,
    span: Span,
    judge: Judge,
}

/// A monitor's last readings the detail view lists (§FS-021-monitors.19).
const LAST_READINGS: usize = 10;

impl super::Session {
    /// The Monitors page (§FS-021-monitors.18): every monitor of the
    /// projects read, or of those `only` names, grouped by project, the
    /// worst project first and the worst monitor first within it
    /// (§FS-021-monitors.18.3), under the header's counts
    /// (§FS-021-monitors.18.2).
    pub fn monitors(&mut self, window: Window, only: &[String]) -> views::Monitors {
        let now = Utc::now();
        let chosen: Vec<String> = self
            .projects
            .iter()
            .filter(|project| only.is_empty() || only.contains(project))
            .cloned()
            .collect();
        let mut projects = Vec::new();
        let mut without = Vec::new();
        let mut counts = PageCounts::default();
        for project in chosen {
            let gathered = self.gather(&project);
            let rows: Vec<MonitorRow> = gathered
                .known
                .iter()
                .map(|(id, known)| self.built(&project, id, known, &gathered, window, now).row)
                .collect();
            if rows.is_empty() {
                if only.contains(&project) {
                    without.push(project);
                }
                continue;
            }
            for row in &rows {
                counts.add(&row.shows);
            }
            projects.push(group(project, rows, gathered.notes));
        }
        arrange(&mut projects);
        views::Monitors {
            window,
            counts,
            refreshed_at: self.refreshed_at(),
            projects,
            without,
        }
    }

    /// One monitor's detail view (§FS-021-monitors.19,
    /// §FS-021-monitors.21.3). An id the project does not have is refused,
    /// naming the ones it has.
    pub fn monitor(
        &mut self,
        project: &str,
        id: &str,
        window: Window,
    ) -> Result<views::MonitorDetail, String> {
        let now = Utc::now();
        let gathered = self.gather(project);
        let Some(known) = gathered.known.get(id) else {
            let has: Vec<&str> = gathered.known.keys().map(String::as_str).collect();
            return Err(match has.is_empty() {
                true => format!(
                    "{project} has no monitors — ephor guide monitors says how to declare one."
                ),
                false => format!(
                    "{project} has no monitor '{id}' (it has: {}).",
                    has.join(", ")
                ),
            });
        };
        let built = self.built(project, id, known, &gathered, window, now);
        let runs_in = self.placement(project).map(declared::checkout);
        let mut detail = detail(project, known, &gathered, built, window, runs_in.as_deref());
        detail.refreshed_at = self.refreshed_at();
        if let Some(matter) = detail
            .row
            .matter
            .as_mut()
            .filter(|matter| matter.state == NEW)
        {
            self.propose(matter);
        }
        Ok(detail)
    }

    /// The newest fetch among the projects read, as Waiting's header says
    /// it (§FS-019-waiting.3).
    fn refreshed_at(&self) -> Option<DateTime<Utc>> {
        self.feeds.iter().filter_map(|feed| feed.fetched_at).max()
    }

    /// Reads one project's declarations and store.
    fn gather(&self, project: &str) -> Gathered {
        let site = self
            .config
            .projects
            .get(project)
            .map(|config| config.monitors.as_slice())
            .unwrap_or_default();
        let declarations = declared::read(self.placement(project), site);
        let slot_error = self
            .feed(project)
            .and_then(|feed| feed.monitor_slot_error())
            .map(str::to_string);
        gathered(declarations, Store::of(project), slot_error)
    }

    fn recent(&self) -> TimeDelta {
        TimeDelta::days(i64::try_from(self.recent_days).unwrap_or(7))
    }

    /// One monitor's row, with its matter's state as the feed knows it.
    fn built(
        &self,
        project: &str,
        id: &str,
        known: &Known,
        gathered: &Gathered,
        window: Window,
        now: DateTime<Utc>,
    ) -> Built {
        let record = gathered.document.monitors.get(id);
        let recent = self.recent();
        let matter = matter_of(project, id, record, recent, now).map(|(key, sort, finished)| {
            let state = match (finished, self.item(&key)) {
                (true, _) => RECENT,
                (false, Some(item)) if self.dismissed(&item) => "dismissed",
                (false, Some(item)) if self.has_work(&item) => "running",
                (false, _) => NEW,
            };
            MatterView {
                key,
                sort,
                state,
                proposal: None,
                passed_over: None,
            }
        });
        build(project, id, known, gathered, window, now, recent, matter)
    }

    /// The work a New matter would be handed, and why the monitor's own
    /// `action` is not it where it is passed over (§FS-021-monitors.16.1).
    fn propose(&mut self, matter: &mut MatterView) {
        let Some(item) = self.item(&matter.key) else {
            return;
        };
        matter.proposal = Some(self.proposal_of(&item));
        matter.passed_over = self
            .menu_with(&super::read::Subject::Item(&item), false)
            .ok()
            .and_then(|menu| {
                menu.into_iter()
                    .find(|entry| entry.action.id == super::monitor_menu::PASSED_OVER)
                    .and_then(|entry| entry.action.withheld)
            });
    }
}

/// The matter state of an open matter nothing has touched.
pub const NEW: &str = "New";
/// The matter state of a finished matter still reported.
pub const RECENT: &str = "Recent";

/// What one project's declarations and store hold.
fn gathered(declarations: Declarations, store: Store, slot_error: Option<String>) -> Gathered {
    let mut notes = Vec::new();
    if let Some(error) = &declarations.manifest_error {
        notes.push(format!("the manifest could not be read: {error}"));
    }
    let document = store.load().unwrap_or_else(|err| {
        notes.push(format!("the monitor store could not be read: {err}"));
        Document::default()
    });
    let known = known::all(&declarations, &document);
    Gathered {
        declarations,
        document,
        known,
        store,
        slot_error,
        notes,
    }
}

/// One project's group, its rows worst first (§FS-021-monitors.18.3).
fn group(project: String, mut rows: Vec<MonitorRow>, notes: Vec<String>) -> MonitorProject {
    rows.sort_by(|a, b| (a.shows.rank, &a.title, &a.id).cmp(&(b.shows.rank, &b.title, &b.id)));
    MonitorProject {
        project,
        rows,
        notes,
    }
}

/// The groups, the one holding the worst row first, then by name
/// (§FS-021-monitors.18.3).
fn arrange(projects: &mut [MonitorProject]) {
    let worst = |group: &MonitorProject| group.rows.first().map(|row| row.shows.rank);
    projects.sort_by(|a, b| (worst(a), &a.project).cmp(&(worst(b), &b.project)));
}

/// One monitor's row (§FS-021-monitors.18.4), judged at `now`.
#[allow(clippy::too_many_arguments)]
fn build(
    project: &str,
    id: &str,
    known: &Known,
    gathered: &Gathered,
    window: Window,
    now: DateTime<Utc>,
    recent: TimeDelta,
    matter: Option<MatterView>,
) -> Built {
    let monitor = &known.monitor;
    let record = gathered.document.monitors.get(id);
    let lines = gathered.store.history(id).unwrap_or_default();
    let facts = page::Facts {
        monitor,
        cadence: &known.cadence,
        record,
        slot_error: gathered.slot_error.as_deref(),
        recent,
    };
    let standing = page::standing(&facts, now, &Local);
    let sample = record.and_then(|record| record.last_sample.as_ref());
    let judge = monitor.judge(sample.and_then(|sample| sample.ok));
    let last = record.and_then(|record| record.last_reading.as_ref());
    let kind = monitor
        .kind
        .or(record.and_then(|record| record.kind))
        .or(last.map(|kept| kind_of(&kept.v)));
    let points = series_points(kind, record);
    // §FS-021-monitors.2.2: a number's points are drawn before its first reading.
    let drawn = match (kind, record.and_then(Record::points)) {
        (Some(Kind::Number), Some(supplied)) => {
            history::handed_over(supplied, &lines, &judge, monitor.warn.as_ref())
        }
        _ => lines.clone(),
    };
    let span = strip::span(&drawn, window, now);
    let (cells, change) = match points {
        Some(points) => (
            strip::series(points, monitor, &judge),
            strip::series_change(points, monitor),
        ),
        None => (
            strip::strip(&drawn, &span, monitor, &judge),
            strip::change(&drawn, &span, monitor, &judge),
        ),
    };
    let of = last.and_then(|kept| kept.of).or(monitor.of);
    let shows = Shows {
        rank: standing.rank,
        glyph: standing.rank.glyph(),
        value: last.map_or_else(
            || "—".to_string(),
            |kept| words::value(&kept.v, monitor.unit.as_deref(), of),
        ),
        every: known.cadence.column(),
        condition: monitor.words(sample.and_then(|sample| sample.ok), of),
        health: standing.column.clone(),
        counts: standing.counts,
        overdue: standing.overdue,
        status: standing.status,
        span,
        strip: cells,
        change,
        matter: matter.as_ref().map_or("—", |matter| matter.state),
    };
    let row = MonitorRow {
        project: project.to_string(),
        id: id.to_string(),
        title: monitor.title.clone(),
        kind,
        value: last.map(|kept| kept.v.clone()),
        of,
        unit: monitor.unit.clone(),
        better: monitor.better,
        health: standing.health,
        since: standing.since,
        shows,
        cadence: cadence_view(known),
        freshness: freshness_view(project, id, known, record, &standing, now),
        condition: condition_view(
            known,
            &judge,
            &gathered.declarations,
            of,
            sample.and_then(|s| s.ok),
        ),
        matter,
        detail: sample.and_then(|sample| sample.detail.clone()),
        url: sample.and_then(|sample| sample.url.clone()),
    };
    Built {
        row,
        standing,
        lines,
        drawn,
        span,
        judge,
    }
}

/// The matter the row's matter column and `p` take: the open drop's before
/// the open unknown one's, and otherwise the finished one still reported,
/// as its key, its sort, and whether it is finished
/// (§FS-021-monitors.18.3, §FS-021-monitors.18.4).
fn matter_of(
    project: &str,
    id: &str,
    record: Option<&Record>,
    recent: TimeDelta,
    now: DateTime<Utc>,
) -> Option<(String, Sort, bool)> {
    let record = record?;
    let open = record
        .drop
        .as_ref()
        .map(|drop| (Sort::Drop, drop.named))
        .or_else(|| {
            record
                .unknown
                .as_ref()
                .map(|unknown| (Sort::Unknown, unknown.named))
        });
    let (sort, named, finished) = match open {
        Some((sort, named)) => (sort, named, false),
        None => {
            let past = record
                .past
                .iter()
                .filter(|past| past.ended >= now - recent)
                .max_by_key(|past| past.ended)?;
            (past.sort, past.named, true)
        }
    };
    Some((episode::key(project, id, sort, named), sort, finished))
}

/// Everything the detail view adds to the row (§FS-021-monitors.19), with
/// `runs_in` the checkout its command runs or its file is read in.
fn detail(
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
    let no_matter = row.matter.is_none().then(|| no_matter(known, &standing));
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
        thresholds: thresholds(&judge, known),
        episodes: episodes(project, &monitor.id, record, &span),
        readings: readings(&lines, &span, windowed, known, &judge),
        points,
        no_matter,
        row,
    }
}

/// The kind a value reads as, for a monitor no home states one of
/// (§FS-021-monitors.2).
fn kind_of(value: &Value) -> Kind {
    match value {
        Value::Boolean(_) => Kind::Boolean,
        Value::Number(_) => Kind::Number,
        Value::Level(_) => Kind::Level,
    }
}

/// A series' points, where it has any (§FS-021-monitors.2.4).
fn series_points(kind: Option<Kind>, record: Option<&Record>) -> Option<&[Point]> {
    (kind == Some(Kind::Series))
        .then(|| record.and_then(Record::points))
        .flatten()
        .filter(|points| !points.is_empty())
}

fn cadence_view(known: &Known) -> CadenceView {
    let origins = &known.monitor.origins;
    let from = origins
        .get("every")
        .or_else(|| origins.get("at"))
        .map_or("default", |home| views::home_name(*home));
    CadenceView {
        every: known.cadence.say_every(),
        at: known.cadence.at.map(|at| at.say()),
        from: from.to_string(),
        says: known.cadence.say(),
        grace: page::lasted(known.cadence.grace()),
    }
}

/// Its freshness as the machine forms carry it (§FS-021-monitors.21.5).
fn freshness_view(
    project: &str,
    id: &str,
    known: &Known,
    record: Option<&Record>,
    standing: &Standing,
    now: DateTime<Utc>,
) -> FreshnessView {
    let monitor = &known.monitor;
    if !monitor.faults.is_empty() {
        return FreshnessView {
            state: "not-read",
            ..FreshnessView::default()
        };
    }
    if monitor.not_run {
        return FreshnessView {
            state: "not-run",
            ..FreshnessView::default()
        };
    }
    let (Some(record), Some(fresh)) = (record, standing.freshness.as_ref()) else {
        return FreshnessView {
            state: "not-read-yet",
            ..FreshnessView::default()
        };
    };
    let basis = record.basis();
    let late_from = freshness::late_from(&basis, &known.cadence, &Local);
    let holds_until = freshness::holds_until(&basis, &known.cadence, &Local);
    // §FS-021-monitors.19, §FS-021-monitors.9.1: the schedule's last read and
    // next slot, a failed or out-of-cadence read included, not the reading
    // the value holds from.
    let last = record.last_attempt.as_ref();
    let mut view = FreshnessView {
        state: "fresh",
        last_read: last.map(|attempt| attempt.at).or(record.schedule.last_read),
        last_found: last.map(|attempt| attempt.found.name()),
        next_slot: record.schedule.next_slot(&known.cadence, now, &Local),
        holds_until,
        ..FreshnessView::default()
    };
    let cause = match fresh {
        Freshness::NotReadYet { .. } => {
            view.state = "not-read-yet";
            None
        }
        Freshness::Holds { .. } => None,
        Freshness::Late { cause, .. } => {
            view.state = "late";
            Some(cause)
        }
        Freshness::Unknown { cause, .. } => {
            view.state = "unknown";
            view.matter = record
                .unknown
                .as_ref()
                .map(|unknown| episode::key(project, id, Sort::Unknown, unknown.named));
            Some(cause)
        }
    };
    if let Some(cause) = cause {
        view.cause = Some(cause.name());
        view.since = late_from;
        match cause {
            Cause::Unreadable { why } => view.why = Some(why.clone()),
            Cause::Stale { measured } => view.measured = *measured,
            Cause::Parked { .. } | Cause::Overdue => {}
        }
    }
    view
}

/// The condition, the home of each part, and what the site's overrides
/// replaced (§FS-021-monitors.21.5).
fn condition_view(
    known: &Known,
    judge: &Judge,
    declarations: &Declarations,
    of: Option<f64>,
    sample_ok: Option<bool>,
) -> ConditionView {
    let monitor = &known.monitor;
    let healthy_home = match &monitor.healthy {
        Healthy::Said(_, home) | Healthy::Null(home) => Some(*home),
        Healthy::Unsaid => None,
    };
    let manifest = declarations
        .manifest
        .get(&monitor.id)
        .and_then(|declaration| serde_json::to_value(declaration).ok());
    let overrides = ["healthy", "warn"]
        .into_iter()
        .filter(|field| monitor.origins.get(field) == Some(&Home::Site))
        .filter_map(|field| {
            let said = manifest.as_ref()?.get(field)?.clone();
            Some(json!({ field: said, "from": "manifest" }))
        })
        .collect::<Vec<Json>>();
    ConditionView {
        healthy: judge.condition().cloned(),
        warn: monitor.warn.clone(),
        from: ConditionView::judge_from(judge, healthy_home),
        warn_from: monitor
            .warn
            .as_ref()
            .and(monitor.origins.get("warn").copied()),
        says: monitor.words(sample_ok, of),
        overrides,
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
fn no_matter(known: &Known, standing: &Standing) -> String {
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

/// The last readings in the span, newest first, each with a note where it
/// was read out of cadence, failed, or was thinned (§FS-021-monitors.19).
fn readings(
    lines: &[Line],
    span: &Span,
    windowed: bool,
    known: &Known,
    judge: &Judge,
) -> Vec<views::ReadingView> {
    let monitor = &known.monitor;
    let levels = monitor.levels.as_deref();
    let said = |value| words::value(value, monitor.unit.as_deref(), monitor.of);
    lines
        .iter()
        .rev()
        .filter(|line| !windowed || span.holds(line.t()))
        .take(LAST_READINGS)
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

#[cfg(test)]
#[path = "monitors_tests.rs"]
mod tests;
