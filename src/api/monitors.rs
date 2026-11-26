//! The Monitors page and one monitor's detail view, below both surfaces
//! (§FS-021-monitors.18, §FS-021-monitors.19, §AR-009-surfaces.1).
//!
//! One derivation, so the page `M` opens and what `ephor monitors --json`
//! prints cannot come apart (§REQ-002-parity.2). It reads the store, the
//! declarations and the cache, and fetches, runs and writes nothing
//! (§FS-021-monitors.21.1). Where each monitor stands is the core's
//! [`page::standing`], and what its strip and change cover is
//! [`strip`]'s; this module only gathers what they judge from and says it.

use std::collections::BTreeMap;

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
use crate::monitors::model::{Declaration, Home, Kind, Point, Stated, Value};
use crate::monitors::overlay::Healthy;
use crate::monitors::page::{self, Rank, Standing};
use crate::monitors::state::{Document, Record};
use crate::monitors::store::Store;
use crate::monitors::strip::{self, Span};
use crate::monitors::window::Window;
use crate::monitors::words;

use detail_view::detail;

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
            // §FS-021-monitors.17.3: a manifest that cannot be read is said,
            // even where nothing else tells that the project has monitors.
            if rows.is_empty() && gathered.notes.is_empty() {
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
            return Err(match (has.is_empty(), gathered.notes.is_empty()) {
                // §FS-021-monitors.17.3: what kept its monitors from being read.
                (true, false) => format!(
                    "{project} has no monitor '{id}' to show: {}.",
                    gathered.notes.join("; ")
                ),
                (true, true) => format!(
                    "{project} has no monitors — ephor guide monitors says how to declare one."
                ),
                (false, _) => format!(
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
    // Titles by their letters, whatever their case (§FS-021-monitors.18.3).
    rows.sort_by_cached_key(|row| (row.shows.rank, row.title.to_lowercase(), row.id.clone()));
    MonitorProject {
        project,
        rows,
        notes,
    }
}

/// The groups, the one holding the worst row first, then by name
/// (§FS-021-monitors.18.3).
fn arrange(projects: &mut [MonitorProject]) {
    // A group that holds only notes goes after every group with a row.
    let worst = |group: &MonitorProject| {
        let worst = group.rows.first().map(|row| row.shows.rank);
        (worst.is_none(), worst)
    };
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
            strip::change(
                &drawn,
                &span,
                monitor,
                &judge,
                monitor.recover_after(&known.cadence),
            ),
        ),
    };
    let of = last.and_then(|kept| kept.of).or(monitor.of);
    // §FS-021-monitors.7.4: a value that no longer holds, or that ephor no
    // longer reads, is not shown as one.
    let holds = standing.rank != Rank::Unknown
        && !matches!(standing.freshness, Some(Freshness::Unknown { .. }));
    let shows = Shows {
        rank: standing.rank,
        glyph: standing.rank.glyph(),
        value: last.filter(|_| holds).map_or_else(
            || "—".to_string(),
            |kept| words::value(&kept.v, monitor.unit.as_deref(), of),
        ),
        every: known.cadence.column(),
        condition: monitor.words(sample.and_then(|sample| sample.ok), of),
        health: standing.column.clone(),
        word: word(&standing),
        held: held(&standing, now),
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
        detail: sample.and_then(|sample| sample.detail.clone()),
        url: sample.and_then(|sample| sample.url.clone()),
        notes: monitor.notes.clone(),
        skew: record.and_then(|record| record.skew),
        no_matter: matter
            .is_none()
            .then(|| detail_view::no_matter(known, &standing)),
        matter,
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

/// The health as one word, or what keeps a monitor from having one
/// (§FS-021-monitors.18.7).
fn word(standing: &Standing) -> String {
    match standing.column.state.as_str() {
        unread @ ("not run" | "not read" | "not read yet") => unread.to_string(),
        _ => standing.rank.word().to_string(),
    }
}

/// How long the health has held: as the health column says it where that
/// is the health, `ok 12d` or `ok 25 points`, else since it began.
fn held(standing: &Standing, now: DateTime<Utc>) -> Option<String> {
    let health = standing.health?;
    match &standing.column.lasted {
        Some(lasted) if standing.column.state == health.name() => Some(lasted.clone()),
        _ => standing.since.map(|since| page::lasted(now - since)),
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
    // Why that read failed, or when its stale sample was measured, before the
    // monitor is late too, so a first read that fails says why at once.
    match last.map(|attempt| &attempt.found) {
        Some(freshness::Found::Unreadable { why }) => view.why = Some(why.clone()),
        Some(freshness::Found::Stale { measured }) => view.measured = *measured,
        _ => {}
    }
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
    let from = ConditionView::judge_from(judge, healthy_home);
    let warn_from = monitor
        .warn
        .as_ref()
        .and(monitor.origins.get("warn").copied());
    let parts = condition_parts(known, judge, declarations, of, (&from, warn_from));
    ConditionView {
        healthy: judge.condition().cloned(),
        warn: monitor.warn.clone(),
        from,
        warn_from,
        says: monitor.words(sample_ok, of),
        overrides,
        parts,
    }
}

/// `healthy` and `warn` in words, each with its home and the manifest's
/// part it overrides (§FS-021-monitors.19); a KPI's `warn` is not said
/// (§FS-021-monitors.3.5).
fn condition_parts(
    known: &Known,
    judge: &Judge,
    declarations: &Declarations,
    of: Option<f64>,
    (from, warn_from): (&str, Option<Home>),
) -> Vec<views::ConditionPart> {
    let monitor = &known.monitor;
    let terms = monitor.terms(of);
    let manifest = declarations.manifest.get(&monitor.id);
    let overridden = |field: &str| {
        monitor.origins.get(field) == Some(&Home::Site) && from_manifest(manifest, field)
    };
    let manifest_healthy = manifest.and_then(|declaration| match &declaration.healthy {
        Stated::Said(condition) => Some(condition),
        _ => None,
    });
    let mut parts = vec![views::ConditionPart {
        part: "healthy",
        says: words::healthy(judge, &terms),
        from: from.to_string(),
        over: overridden("healthy").then(|| {
            match manifest.map(|declaration| &declaration.healthy) {
                Some(Stated::Said(condition)) => {
                    words::healthy(&Judge::Condition(condition.clone()), &terms)
                }
                _ => "KPI".to_string(),
            }
        }),
    }];
    if let (Some(warn), false) = (&monitor.warn, judge.is_kpi()) {
        parts.push(views::ConditionPart {
            part: "warn",
            says: words::warn(warn, judge.condition(), &terms),
            from: warn_from.map_or("—", views::home_name).to_string(),
            over: overridden("warn").then(|| match manifest.map(|declaration| &declaration.warn) {
                Some(Stated::Said(condition)) => words::warn(condition, manifest_healthy, &terms),
                _ => "none".to_string(),
            }),
        });
    }
    parts
}

/// Whether the manifest's declaration names `field`.
fn from_manifest(manifest: Option<&Declaration>, field: &str) -> bool {
    let Some(declaration) = manifest else {
        return false;
    };
    match field {
        "healthy" => !declaration.healthy.is_unsaid(),
        "warn" => !declaration.warn.is_unsaid(),
        _ => false,
    }
}

mod detail_view;

#[cfg(test)]
#[path = "monitors_tests.rs"]
pub(crate) mod tests;
