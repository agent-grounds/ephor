//! Pages and detail views made from a store of the test's own, for the
//! screen and the command to draw (§FS-021-monitors.18, §FS-021-monitors.19,
//! §REQ-003-test-isolation): a second project whose monitors are late or
//! unknown for each cause, and one number's and one boolean's detail.

use super::*;

/// Every monitor of `project` in `gathered`, as rows built at the tests' now.
fn rows_of(project: &str, gathered: &Gathered) -> Vec<MonitorRow> {
    gathered
        .known
        .iter()
        .map(|(id, known)| {
            let record = gathered.document.monitors.get(id);
            let matter =
                matter_of(project, id, record, recent(), now()).map(|(key, sort, finished)| {
                    MatterView {
                        key,
                        sort,
                        state: if finished { RECENT } else { NEW },
                        proposal: None,
                        passed_over: None,
                    }
                });
            build(
                project,
                id,
                known,
                gathered,
                Window::Week,
                now(),
                recent(),
                matter,
            )
            .row
        })
        .collect()
}

/// A record whose last reading was ok five hours ago, and whose last try, an
/// hour ago, found `found`: late for that cause, and by now unknown.
fn tried(found: Found) -> Record {
    let mut record = read_at(
        hours(5),
        Value::Number(81.0),
        Some(Health::Ok),
        Some(Band::Ok),
    );
    record.schedule.anchor = Some(hours(5));
    record.schedule.last_read = Some(hours(1));
    record.last_attempt = Some(Attempt {
        at: hours(1),
        found,
    });
    record
}

/// Project `tools`: a monitor unknown for each cause a read can have, and
/// one not read yet (§FS-021-monitors.7.2, §FS-021-monitors.18.7).
fn causes(dir: &Path) -> Gathered {
    let site = [
        json!({"id": "unreadable", "title": "Advisories", "healthy": {"min": 80}, "every": "1h"}),
        json!({"id": "parked", "title": "Benchmarks", "healthy": {"min": 80}, "every": "1h"}),
        json!({"id": "stale", "title": "Coverage of tools", "healthy": {"min": 80}, "every": "1h"}),
        json!({"id": "fresh", "title": "Doc links", "healthy": {"min": 1}, "every": "1h"}),
    ];
    let unreadable = tried(Found::Unreadable {
        why: "scripts/kpis.sh exited 3".into(),
    });
    let stale = tried(Found::Stale { measured: None });
    let records = vec![
        ("unreadable", unreadable),
        ("parked", tried(Found::Parked)),
        ("stale", stale),
    ];
    let store = Store::at(dir, "tools");
    store
        .update(|document| {
            for (id, record) in records {
                document.monitors.insert(id.to_string(), record);
            }
            Ok(())
        })
        .unwrap();
    gathered(declared::read(None, &site), store, None)
}

/// Two projects: `widget`'s six monitors, one at each rank, and `tools`'
/// monitors unknown for each cause, the worse project first
/// (§FS-021-monitors.18.3). `dir` holds both stores.
pub(crate) fn page_of_projects(dir: &Path) -> views::Monitors {
    let widget = group("widget".into(), rows(&six(&dir.join("widget"))), Vec::new());
    let tools = group(
        "tools".into(),
        rows_of("tools", &causes(&dir.join("tools"))),
        vec!["ephor.json could not be read: expected value at line 1".into()],
    );
    let mut projects = vec![tools, widget];
    arrange(&mut projects);
    let mut counts = PageCounts::default();
    for row in projects.iter().flat_map(|group| &group.rows) {
        counts.add(&row.shows);
    }
    views::Monitors {
        window: Window::Week,
        counts,
        refreshed_at: Some(hours(0) - TimeDelta::minutes(4)),
        projects,
        without: Vec::new(),
    }
}

/// `widget`'s latency in detail: three readings, one a failed read and the
/// last read out of cadence, under `healthy ≤ 50 ms` and `warn ≤ 40 ms`, with
/// an open drop (§FS-021-monitors.19).
pub(crate) fn latency_detail(dir: &Path) -> views::MonitorDetail {
    let site = [json!({
        "id": "latency", "title": "Latency", "unit": "ms", "better": "lower",
        "healthy": {"max": 50}, "warn": {"max": 40}, "every": "1h",
        "from": {"command": "kpis.sh"}
    })];
    let mut record = read_at(
        hours(1),
        Value::Number(60.0),
        Some(Health::Down),
        Some(Band::Down),
    );
    record.drop = Some(open_drop(hours(1)));
    let lines = vec![
        Line::Reading {
            t: hours(3),
            v: Value::Number(30.0),
            health: Some(Health::Ok),
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
        dir,
        &site,
        vec![("latency", record)],
        vec![("latency", lines)],
    );
    let built = built(&gathered, "latency");
    let known = &gathered.known["latency"];
    detail(
        "widget",
        known,
        &gathered,
        built,
        Window::Week,
        Some(Path::new("/w/widget")),
    )
}

/// `widget`'s build in detail: a boolean that was green, went red and came
/// back, with no matter open (§FS-021-monitors.19).
pub(crate) fn green_detail(dir: &Path) -> views::MonitorDetail {
    let site = [json!({"id": "green", "title": "Build green", "kind": "boolean", "every": "1h"})];
    let record = read_at(
        hours(0),
        Value::Boolean(true),
        Some(Health::Ok),
        Some(Band::Ok),
    );
    let reading = |ago: i64, yes: bool| Line::Reading {
        t: hours(ago),
        v: Value::Boolean(yes),
        health: Some(if yes { Health::Ok } else { Health::Down }),
        out_of_cadence: false,
    };
    let lines = vec![
        reading(4, true),
        reading(3, false),
        reading(2, false),
        reading(0, true),
    ];
    let gathered = gather(dir, &site, vec![("green", record)], vec![("green", lines)]);
    let built = built(&gathered, "green");
    detail(
        "widget",
        &gathered.known["green"],
        &gathered,
        built,
        Window::Week,
        None,
    )
}

/// §FS-021-monitors.18.7: the second project's health column names each
/// cause and how long it has run, and the first not yet read.
#[test]
fn each_cause_is_named_in_the_health_column() {
    let dir = tempfile::tempdir().unwrap();
    let page = page_of_projects(dir.path());
    let names: Vec<&str> = page
        .projects
        .iter()
        .map(|group| group.project.as_str())
        .collect();
    assert_eq!(names, ["widget", "tools"], "the worse project first");
    let health = |id: &str| {
        page.rows()
            .find(|row| row.id == id)
            .unwrap()
            .shows
            .health
            .text()
    };
    assert!(
        health("unreadable").starts_with("unreadable "),
        "{}",
        health("unreadable")
    );
    assert!(
        health("parked").starts_with("parked "),
        "{}",
        health("parked")
    );
    assert!(health("stale").starts_with("stale "), "{}", health("stale"));
    assert_eq!(health("fresh"), "not read yet");
}

/// §FS-021-monitors.19: a boolean's chart places are named `no` and `yes`,
/// and the cursor says a point by its name, its time and its health; a
/// number's says its value in its unit, and a failed read says it has none.
#[test]
fn the_cursor_says_a_point_in_the_rows_words() {
    let dir = tempfile::tempdir().unwrap();
    let at = |at: DateTime<Utc>| at.format("%H:%M").to_string();
    let green = green_detail(&dir.path().join("green"));
    assert_eq!(green.steps, ["no", "yes"]);
    let last = green.chart.last().unwrap();
    assert_eq!(green.cursor_says(last, at), "12:00  yes  ok");
    assert_eq!(green.cursor_says(&green.chart[1], at), "09:00  no  down");

    let latency = latency_detail(&dir.path().join("latency"));
    assert!(latency.steps.is_empty());
    assert_eq!(
        latency.cursor_says(&latency.chart[0], at),
        "09:00  30 ms  ok"
    );
    assert_eq!(
        latency.cursor_says(&latency.chart[1], at),
        "10:00  —  unknown"
    );
    assert_eq!(
        latency.cursor_says(&latency.chart[2], at),
        "11:00  60 ms  down"
    );
    let bounds: Vec<(f64, &str)> = latency
        .thresholds
        .iter()
        .map(|line| (line.at, line.of))
        .collect();
    assert_eq!(bounds, [(50.0, "healthy"), (40.0, "warn")]);
}
