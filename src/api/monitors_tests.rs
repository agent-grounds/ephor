//! The Monitors page and the detail view, held to §FS-021-monitors.18 and
//! §FS-021-monitors.19 without a session: each project's declarations and a
//! store in a directory of the test's own.

use std::path::Path;

use serde_json::{json, Value as Json};

use super::*;
use crate::monitors::episode::Ending;
use crate::monitors::model::{Band, Value};
use crate::monitors::shape;
use crate::monitors::state::{Hold, Kept, OpenDrop, OpenUnknown, Past, Shown};

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-11-25T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn hours(hours: i64) -> DateTime<Utc> {
    now() - TimeDelta::hours(hours)
}

fn recent() -> TimeDelta {
    TimeDelta::days(7)
}

/// A record whose value `v` was read at `at`, holding `health` for two days.
fn read_at(at: DateTime<Utc>, v: Value, health: Option<Health>, band: Option<Band>) -> Record {
    let mut record = Record::new(at);
    record.held_from = Some(at);
    record.last_reading = Some(Kept { t: at, v, of: None });
    record.hold = Hold {
        health,
        since: Some(at - TimeDelta::days(2)),
        band,
        run: 1,
    };
    record
}

fn open_drop(named: DateTime<Utc>) -> OpenDrop {
    OpenDrop {
        named,
        moved: None,
        worst: None,
        judged_under: Judge::SampleOk,
        judged_words: "healthy ≥ 80%".into(),
        works_ended: Vec::new(),
        renewed: None,
        shown: Shown::default(),
    }
}

fn open_unknown(named: DateTime<Utc>) -> OpenUnknown {
    OpenUnknown {
        named,
        moved: None,
        cause: Cause::Overdue,
        works_ended: Vec::new(),
        renewed: None,
        shown: Shown::default(),
    }
}

fn past(ended: DateTime<Utc>) -> Past {
    Past {
        sort: Sort::Drop,
        named: ended - TimeDelta::hours(3),
        ended,
        how: Ending::Recovered {
            value: "81.0%".into(),
        },
        moved: None,
        cause: None,
        shown: Shown::default(),
    }
}

/// One project's monitors as the site declares them, over a store holding
/// `records` and `histories` under `dir`.
fn gather(
    dir: &Path,
    site: &[Json],
    records: Vec<(&str, Record)>,
    histories: Vec<(&str, Vec<Line>)>,
) -> Gathered {
    let store = Store::at(dir, "widget");
    store
        .update(|document| {
            for (id, record) in records {
                document.monitors.insert(id.to_string(), record);
            }
            Ok(())
        })
        .unwrap();
    for (id, lines) in histories {
        store.append(id, &lines).unwrap();
    }
    gathered(declared::read(None, site), store, None)
}

fn built(gathered: &Gathered, id: &str) -> Built {
    let known = &gathered.known[id];
    let record = gathered.document.monitors.get(id);
    let matter =
        matter_of("widget", id, record, recent(), now()).map(|(key, sort, finished)| MatterView {
            key,
            sort,
            state: if finished { RECENT } else { NEW },
            proposal: None,
            passed_over: None,
        });
    build(
        "widget",
        id,
        known,
        gathered,
        Window::Week,
        now(),
        recent(),
        matter,
    )
}

fn rows(gathered: &Gathered) -> Vec<MonitorRow> {
    gathered
        .known
        .keys()
        .map(|id| built(gathered, id).row)
        .collect()
}

/// Six monitors, one at each rank the page orders by.
fn six(dir: &Path) -> Gathered {
    let site = [
        json!({"id": "coverage", "title": "Line coverage", "unit": "%", "healthy": {"min": 80}, "every": "1h"}),
        json!({"id": "latency", "title": "Latency", "unit": "ms", "better": "lower", "healthy": {"max": 50}, "every": "1h"}),
        json!({"id": "lines", "title": "Lines of code", "healthy": null, "every": "1h"}),
        json!({"id": "green", "title": "Build green", "kind": "boolean", "every": "1h"}),
        json!({"id": "flaky", "title": "Flaky tests", "healthy": {"max": 0}, "every": "1h", "drop_after": 2}),
        json!({"id": "docs", "title": "Docs build", "kind": "boolean", "every": "1h"}),
    ];
    let mut coverage = read_at(
        hours(0),
        Value::Number(79.2),
        Some(Health::Down),
        Some(Band::Down),
    );
    coverage.drop = Some(open_drop(hours(30)));
    let mut green = read_at(
        hours(5),
        Value::Boolean(true),
        Some(Health::Ok),
        Some(Band::Ok),
    );
    green.unknown = Some(open_unknown(hours(3)));
    let records = vec![
        ("coverage", coverage),
        (
            "latency",
            read_at(
                hours(0),
                Value::Number(42.0),
                Some(Health::Ok),
                Some(Band::Ok),
            ),
        ),
        (
            "lines",
            read_at(hours(0), Value::Number(41_000.0), None, None),
        ),
        ("green", green),
        (
            "flaky",
            read_at(
                hours(0),
                Value::Number(2.0),
                Some(Health::Ok),
                Some(Band::Down),
            ),
        ),
        (
            "docs",
            read_at(
                hours(0),
                Value::Boolean(true),
                Some(Health::Warn),
                Some(Band::Warn),
            ),
        ),
    ];
    gather(dir, &site, records, Vec::new())
}

/// §FS-021-monitors.18.3, §FS-021-monitors.18.2: rows worst first, down
/// ahead of unknown, failing, warn and ok, KPIs last, and the header counts
/// each once, overdue a second time.
#[test]
fn rows_go_worst_first_and_the_header_counts_each_once() {
    let dir = tempfile::tempdir().unwrap();
    let gathered = six(dir.path());
    let group = group("widget".into(), rows(&gathered), Vec::new());
    let order: Vec<&str> = group.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(
        order,
        ["coverage", "green", "flaky", "docs", "latency", "lines"]
    );
    let glyphs: Vec<&str> = group.rows.iter().map(|row| row.shows.glyph).collect();
    assert_eq!(glyphs, ["✖", "?", "▼", "▲", "●", "◆"]);

    let mut counts = PageCounts::default();
    for row in &group.rows {
        counts.add(&row.shows);
    }
    assert_eq!(
        counts.say(),
        "1 down · 1 unknown · 1 failing · 1 warn · 1 ok · 1 KPIs · 1 overdue"
    );
    let page = views::Monitors {
        window: Window::Week,
        counts,
        refreshed_at: None,
        projects: vec![group],
        without: Vec::new(),
    };
    assert!(page.fails_check());
    assert_eq!(
        page.header(now()),
        "ephor — monitors   1 down · 1 unknown · 1 failing · 1 warn · 1 ok · 1 KPIs · 1 overdue   window 7d · never refreshed"
    );
}

/// §FS-021-monitors.21.2: `--check` fails on a status down or unknown, and
/// never on a KPI, on failing or warn, or on a monitor not yet read.
#[test]
fn check_reads_statuses_down_or_unknown_only() {
    let dir = tempfile::tempdir().unwrap();
    let gathered = six(dir.path());
    let kept: Vec<MonitorRow> = rows(&gathered)
        .into_iter()
        .filter(|row| !matches!(row.id.as_str(), "coverage" | "green"))
        .collect();
    let page = |rows| views::Monitors {
        window: Window::Week,
        counts: PageCounts::default(),
        refreshed_at: None,
        projects: vec![group("widget".into(), rows, Vec::new())],
        without: Vec::new(),
    };
    assert!(!page(kept).fails_check());

    let site = [json!({"id": "fresh", "every": "1h", "healthy": {"min": 1}})];
    let empty = tempfile::tempdir().unwrap();
    let unread = gather(empty.path(), &site, Vec::new(), Vec::new());
    let row = &rows(&unread)[0];
    assert_eq!(row.shows.health.text(), "not read yet");
    assert_eq!(row.freshness.state, "not-read-yet");
    assert!(!page(vec![row.clone()]).fails_check());
}

/// §FS-021-monitors.21.2: `--check` fails on a status unknown, overdue
/// included, and on a status whose declaration is in error, even one the
/// shape refuses; never on a KPI unknown or in error.
#[test]
fn check_fails_on_an_unknown_status_and_one_in_error_never_on_a_kpi() {
    let dir = tempfile::tempdir().unwrap();
    let gathered = six(dir.path());
    let page = |rows: Vec<MonitorRow>| views::Monitors {
        window: Window::Week,
        counts: PageCounts::default(),
        refreshed_at: None,
        projects: vec![group("widget".into(), rows, Vec::new())],
        without: Vec::new(),
    };
    let only = |id: &str| -> Vec<MonitorRow> {
        rows(&gathered)
            .into_iter()
            .filter(|row| row.id == id)
            .collect()
    };
    assert_eq!(only("green")[0].shows.counts, Some(Rank::Unknown));
    assert!(page(only("green")).fails_check(), "an overdue status");

    let site = [
        json!({"id": "cov", "unit": "%", "healthy": {"min": 80}, "every": "90s"}),
        json!({"id": "typo", "kind": "nubmer", "healthy": {"min": 80}}),
        json!({"id": "kpi", "kind": "number", "every": "90s"}),
        json!({"id": "lines", "healthy": null, "every": "1h"}),
    ];
    let other = tempfile::tempdir().unwrap();
    let mut lines = read_at(hours(48), Value::Number(41_000.0), None, None);
    lines.schedule.first_seen = hours(48);
    let faulted = gather(other.path(), &site, vec![("lines", lines)], Vec::new());
    let all = rows(&faulted);
    let row = |id: &str| all.iter().find(|row| row.id == id).unwrap().clone();
    for id in ["cov", "typo"] {
        assert!(row(id).shows.status, "{id} is a status in error");
        assert!(page(vec![row(id)]).fails_check(), "{id}");
    }
    assert_eq!(row("lines").shows.counts, Some(Rank::Unknown));
    for id in ["kpi", "lines"] {
        assert!(!row(id).shows.status, "{id} is a KPI");
        assert!(!page(vec![row(id)]).fails_check(), "{id}");
    }
}

/// §FS-021-monitors.18.3: the group holding the worst row goes first, then
/// by name.
#[test]
fn groups_go_by_their_worst_row_then_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let gathered = six(dir.path());
    let all = rows(&gathered);
    let ok: Vec<MonitorRow> = all
        .iter()
        .filter(|row| row.id == "latency")
        .cloned()
        .collect();
    let mut projects = vec![
        group("gadget".into(), ok.clone(), Vec::new()),
        group("widget".into(), all, Vec::new()),
        group("acme".into(), ok, Vec::new()),
    ];
    arrange(&mut projects);
    let names: Vec<&str> = projects
        .iter()
        .map(|group| group.project.as_str())
        .collect();
    assert_eq!(names, ["widget", "acme", "gadget"]);
}

/// §FS-021-monitors.18.4, §FS-021-monitors.21.5: a row's value, cadence,
/// condition and matter, and an overdue monitor's cause, since and unknown
/// matter.
#[test]
fn a_row_carries_what_its_columns_say() {
    let dir = tempfile::tempdir().unwrap();
    let gathered = six(dir.path());
    let coverage = built(&gathered, "coverage").row;
    assert_eq!(coverage.shows.value, "79.2%");
    assert_eq!(coverage.shows.every, "1h");
    assert_eq!(coverage.shows.condition, "healthy ≥ 80%");
    assert_eq!(coverage.shows.matter, "New");
    let matter = coverage.matter.unwrap();
    assert_eq!(matter.key, "monitor:widget/coverage@20261124T060000Z");
    assert_eq!(matter.sort, Sort::Drop);
    assert_eq!(coverage.condition.from, "site");
    assert_eq!(coverage.cadence.from, "site");
    assert_eq!(coverage.freshness.state, "fresh");

    let green = built(&gathered, "green").row;
    assert_eq!(green.freshness.state, "unknown");
    assert_eq!(green.freshness.cause, Some("overdue"));
    assert_eq!(green.freshness.since, Some(hours(4)));
    assert_eq!(green.shows.health.text(), "overdue 4h");
    assert_eq!(
        green.freshness.matter.as_deref(),
        Some("monitor:widget/green.unknown@20261125T090000Z")
    );
    assert_eq!(green.condition.from, "default");

    let lines = built(&gathered, "lines").row;
    assert_eq!(lines.condition.from, "kpi");
    assert_eq!(lines.shows.matter, "—");
}

/// §FS-021-monitors.18.3: the matter column takes the open drop's before
/// the open unknown one's, a finished one while it is still reported, and
/// none after.
#[test]
fn the_matter_is_the_drops_before_the_unknowns_then_a_recent_one() {
    let mut record = read_at(hours(0), Value::Number(1.0), Some(Health::Down), None);
    record.drop = Some(open_drop(hours(2)));
    record.unknown = Some(open_unknown(hours(1)));
    let (key, sort, finished) = matter_of("widget", "m", Some(&record), recent(), now()).unwrap();
    assert_eq!((sort, finished), (Sort::Drop, false));
    assert!(!key.contains(".unknown"));

    record.drop = None;
    let (_, sort, _) = matter_of("widget", "m", Some(&record), recent(), now()).unwrap();
    assert_eq!(sort, Sort::Unknown);

    record.unknown = None;
    record.past.push(past(hours(24)));
    let (_, _, finished) = matter_of("widget", "m", Some(&record), recent(), now()).unwrap();
    assert!(finished);
    assert!(matter_of("widget", "m", Some(&record), TimeDelta::hours(1), now()).is_none());
}

/// §FS-021-monitors.19, §FS-021-monitors.18.9: the chart judges each reading
/// under today's condition, the threshold lines are today's bounds, the last
/// readings go newest first with their notes, and the episodes are the
/// window's.
#[test]
fn the_detail_draws_the_window_under_todays_condition() {
    let dir = tempfile::tempdir().unwrap();
    let site = [json!({
        "id": "latency", "title": "Latency", "unit": "ms", "better": "lower",
        "healthy": {"max": 50}, "every": "1h",
        "from": {"command": "kpis.sh", "cwd": "repo:tools"}
    })];
    let mut record = read_at(
        hours(1),
        Value::Number(60.0),
        Some(Health::Down),
        Some(Band::Down),
    );
    record.past = vec![past(now() - TimeDelta::days(30)), past(hours(48))];
    record.drop = Some(open_drop(hours(1)));
    let lines = vec![
        Line::Reading {
            t: hours(3),
            v: Value::Number(40.0),
            health: Some(Health::Down),
            out_of_cadence: false,
        },
        Line::marker(hours(2)),
        Line::Reading {
            t: hours(1),
            v: Value::Number(60.0),
            health: Some(Health::Down),
            out_of_cadence: true,
        },
    ];
    let gathered = gather(
        dir.path(),
        &site,
        vec![("latency", record)],
        vec![("latency", lines)],
    );
    let known = &gathered.known["latency"];
    let built = built(&gathered, "latency");
    let detail = detail(
        "widget",
        known,
        &gathered,
        built,
        Window::Week,
        Some(Path::new("/w/widget")),
    );

    assert_eq!(detail.title(), "ephor — monitors › widget › Latency");
    assert_eq!(detail.window_says(), "window 7d");
    assert_eq!(detail.chart.len(), 3);
    assert_eq!(detail.chart[0].value, Some(40.0));
    assert_eq!(
        detail.chart[0].health,
        Some(Health::Ok),
        "40 ms is healthy today"
    );
    assert_eq!(detail.chart[2].health, Some(Health::Down));
    assert_eq!(detail.thresholds.len(), 1);
    assert_eq!(
        (
            detail.thresholds[0].at,
            detail.thresholds[0].of,
            detail.thresholds[0].bound
        ),
        (50.0, "healthy", "max")
    );
    let notes: Vec<Option<&str>> = detail
        .readings
        .iter()
        .map(|reading| reading.note.as_deref())
        .collect();
    assert_eq!(
        notes,
        [Some("read out of cadence"), Some("the read failed"), None]
    );
    assert_eq!(detail.readings[0].value, "60 ms");
    assert_eq!(
        detail.episodes.len(),
        2,
        "the episode ended 30 days ago is outside"
    );
    assert!(
        detail.episodes[1].end.is_none(),
        "the open drop ends nothing yet"
    );
    assert!(detail.no_matter.is_none());
    let hold = detail.hold.as_ref().unwrap();
    assert!(hold
        .says()
        .starts_with("drop after 1 failing reading · recover after "));
    let from = detail.from.as_ref().unwrap();
    assert_eq!(from.runs_in.as_deref(), Some(Path::new("/w/widget/tools")));
    assert_eq!(from.home, Some(Home::Site));
}

/// §FS-021-monitors.19: a series drawn by `x` labels has no time to set a
/// window on, and charts its points.
#[test]
fn a_labelled_series_has_no_window() {
    let dir = tempfile::tempdir().unwrap();
    let site = [
        json!({"id": "bench", "kind": "series", "unit": "ms", "healthy": {"max": 10}, "every": "1h"}),
    ];
    let mut record = read_at(
        hours(0),
        Value::Number(5.0),
        Some(Health::Ok),
        Some(Band::Ok),
    );
    let point = |x: &str, v: f64| Point {
        t: None,
        x: Some(x.into()),
        v,
    };
    record.last_sample = Some(crate::monitors::model::Sample {
        id: "bench".into(),
        points: Some(vec![point("a", 12.0), point("b", 4.0), point("c", 5.0)]),
        ..Default::default()
    });
    let gathered = gather(dir.path(), &site, vec![("bench", record)], Vec::new());
    let built = built(&gathered, "bench");
    assert_eq!(built.row.shows.strip.len(), strip::CELLS);
    assert_eq!(built.row.shows.change.says, "▾ 7 ms");
    let detail = detail(
        "widget",
        &gathered.known["bench"],
        &gathered,
        built,
        Window::Week,
        None,
    );
    assert!(!detail.windowed);
    assert_eq!(detail.window_says(), "window n/a: series");
    let charted: Vec<Option<Health>> = detail.chart.iter().map(|point| point.health).collect();
    assert_eq!(
        charted,
        [Some(Health::Down), Some(Health::Ok), Some(Health::Ok)]
    );
    assert_eq!(detail.points.len(), 3);
}

/// §FS-021-monitors.19, §FS-021-monitors.21.5: the site's condition over the
/// manifest's says what it overrode, and the detail names both homes.
#[test]
fn a_site_override_names_what_the_manifest_said() {
    let dir = tempfile::tempdir().unwrap();
    let declarations = Declarations {
        site: shape::read_declarations(
            &[json!({"id": "coverage", "healthy": {"min": 80}})],
            Home::Site,
        ),
        manifest: shape::read_declarations(
            &[json!({
                "id": "coverage", "title": "Line coverage", "unit": "%",
                "healthy": {"min": 75}, "every": "1h", "from": {"file": "coverage.json"}
            })],
            Home::Manifest,
        ),
        ..Declarations::default()
    };
    let gathered = gathered(declarations, Store::at(dir.path(), "widget"), None);
    let built = built(&gathered, "coverage");
    let condition = &built.row.condition;
    assert_eq!(condition.from, "site");
    assert_eq!(condition.says, "healthy ≥ 80%");
    assert_eq!(
        condition.overrides,
        [json!({"healthy": {"min": 75.0}, "from": "manifest"})]
    );
    assert_eq!(built.row.cadence.from, "manifest");
    let detail = detail(
        "widget",
        &gathered.known["coverage"],
        &gathered,
        built,
        Window::Week,
        None,
    );
    assert_eq!(detail.declared.homes, [Home::Site, Home::Manifest]);
    assert_eq!(detail.declared.site_overrides, ["healthy"]);
    assert_eq!(detail.no_matter.as_deref(), Some("none: nothing is open"));
}

/// §FS-021-monitors.19: why a monitor has no matter, for one not run, a
/// KPI, and one failing toward its drop.
#[test]
fn the_detail_says_why_there_is_no_matter() {
    let dir = tempfile::tempdir().unwrap();
    let gathered = six(dir.path());
    let why = |id: &str| {
        let built = built(&gathered, id);
        detail(
            "widget",
            &gathered.known[id],
            &gathered,
            built,
            Window::Week,
            None,
        )
        .no_matter
        .unwrap()
    };
    assert_eq!(why("lines"), "none: a KPI is never a matter");
    assert_eq!(why("flaky"), "none yet: it drops after its hold");
    let mut known = gathered.known["latency"].clone();
    known.monitor.not_run = true;
    let standing = built(&gathered, "latency").standing;
    assert!(no_matter(&known, &standing).starts_with("none: "));
    let freshness = freshness_view("widget", "latency", &known, None, &standing);
    assert_eq!(freshness.state, "not-run");
}
