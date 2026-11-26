//! E2E-083-the-monitors-page-as-a-table: `ephor monitors` prints the
//! Monitors page as a table, worst first under the header's counts, with the
//! columns `value every health since change condition` and the footnote on
//! what a change covers (§FS-021-monitors.21.1, §FS-021-monitors.18.2,
//! §FS-021-monitors.18.3). With no monitors it says how to declare one, and a
//! project named with `--project` that has none says so. `--json` prints the
//! published `monitors` view (§FS-021-monitors.21.5). `--check` exits 4 while
//! a status is down or unknown, or its declaration is in error, and 0 once
//! none is, and never for a KPI, unknown or in error
//! (§FS-021-monitors.21.2). Printing the page reads the store and runs no
//! monitor, so an answer that changed shows only after a refresh reads it
//! (§FS-021-monitors.21.4).

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

const NONE: &str = "no monitors — ephor guide monitors says how to declare one";
const FOOTNOTE: &str =
    "change: over the window, or over the last 24 readings when those reach further back";

fn world() -> World {
    let world = World::new();
    world.configure(json!({ "projects": { PROJECT: {
        "providers": [],
        "monitors": [
            { "id": "coverage", "title": "Line coverage", "unit": "%",
              "healthy": { "min": 80 }, "every": "1h",
              "from": { "command": "scripts/kpis.sh" } },
            { "id": "lines", "title": "Lines of code", "every": "1h",
              "from": { "command": "scripts/kpis.sh" } },
            { "id": "green", "title": "Build green", "kind": "boolean", "every": "1h",
              "from": { "command": "scripts/kpis.sh" } }
        ]
    } } }));
    world
}

fn answer(world: &World, coverage: f64) {
    world.script(
        "scripts/kpis.sh",
        &format!(
            "#!/bin/sh\nprintf '%s' '{{\"v\":1,\"monitors\":[{{\"id\":\"coverage\",\"value\":{coverage}}},{{\"id\":\"lines\",\"value\":1200}},{{\"id\":\"green\",\"value\":true}}]}}' > \"$EPHOR_ANSWER\"\n"
        ),
    );
}

fn refresh_now(world: &World) {
    world
        .ephor()
        .args(["refresh", PROJECT, "--monitors-now"])
        .assert()
        .success();
}

fn page(world: &World, args: &[&str]) -> Value {
    let output = world
        .ephor()
        .arg("monitors")
        .args(args)
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    shaped("monitors", &output)
}

fn table(world: &World, args: &[&str]) -> String {
    let output = world.ephor().arg("monitors").args(args).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn ids(page: &Value) -> Vec<&str> {
    page["projects"][0]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect()
}

#[test]
fn with_no_monitors_the_page_says_how_to_declare_one() {
    let world = World::new();
    let printed = table(&world, &[]);
    let header = printed.lines().next().unwrap();
    assert!(
        header.starts_with(
            "ephor — monitors   0 down · 0 unknown · 0 failing · 0 warn · 0 ok · 0 KPIs   window 7d · "
        ),
        "{printed}"
    );
    assert!(printed.contains(NONE), "{printed}");
    assert!(!printed.contains(FOOTNOTE), "{printed}");

    let empty = page(&world, &[]);
    assert_eq!(empty["projects"], json!([]));
    assert_eq!(empty["counts"]["down"], json!(0));
    assert_eq!(empty["window"], json!("7d"));

    world.ephor().args(["monitors", "--check"]).assert().code(0);

    let named = table(&world, &["--project", PROJECT]);
    assert!(
        named.contains(&format!("{PROJECT} has no monitors")),
        "{named}"
    );
    let named = page(&world, &["--project", PROJECT]);
    assert_eq!(named["without"], json!([PROJECT]));
}

#[test]
fn the_page_puts_the_worst_first_and_counts_each_monitor_once() {
    let world = world();
    answer(&world, 79.2);
    refresh_now(&world);

    let read = page(&world, &[]);
    assert_eq!(read["projects"][0]["project"], json!(PROJECT));
    assert_eq!(ids(&read), ["coverage", "green", "lines"]);
    let counts = &read["counts"];
    assert_eq!(
        (
            &counts["down"],
            &counts["ok"],
            &counts["kpis"],
            &counts["overdue"]
        ),
        (&json!(1), &json!(1), &json!(1), &json!(0))
    );
    let coverage = &read["projects"][0]["rows"][0];
    assert_eq!(coverage["health"], json!("down"));
    assert_eq!(coverage["value"], json!(79.2));
    assert_eq!(coverage["shows"]["value"], json!("79.2%"));
    assert_eq!(coverage["shows"]["every"], json!("1h"));
    assert_eq!(coverage["shows"]["matter"], json!("New"));
    assert_eq!(coverage["cadence"]["from"], json!("site"));
    assert_eq!(coverage["freshness"]["state"], json!("fresh"));
    assert_eq!(coverage["condition"]["from"], json!("site"));
    assert_eq!(coverage["condition"]["healthy"], json!({ "min": 80.0 }));
    assert!(coverage["matter"]["key"]
        .as_str()
        .unwrap()
        .starts_with(&format!("monitor:{PROJECT}/coverage@")));
    assert_eq!(
        read["projects"][0]["rows"][2]["shows"]["rank"],
        json!("kpi")
    );
    assert_eq!(
        read["projects"][0]["rows"][0]["shows"]["strip"]
            .as_array()
            .unwrap()
            .len(),
        24
    );

    let printed = table(&world, &[]);
    let lines: Vec<&str> = printed.lines().collect();
    assert!(
        lines[0].starts_with("ephor — monitors   1 down · 0 unknown · 0 failing · 0 warn · 1 ok · 1 KPIs   window 7d · refreshed "),
        "{printed}"
    );
    let heading = lines
        .iter()
        .find(|line| line.starts_with(&format!("  {PROJECT} ")))
        .unwrap_or_else(|| panic!("no project heading:\n{printed}"));
    let columns: Vec<&str> = heading.split_whitespace().collect();
    assert_eq!(
        columns,
        [
            PROJECT,
            "value",
            "every",
            "health",
            "since",
            "change",
            "condition"
        ]
    );
    let row = lines
        .iter()
        .find(|line| line.starts_with("✖ Line coverage"))
        .unwrap_or_else(|| panic!("no coverage row:\n{printed}"));
    for said in ["79.2%", "1h", "down", "healthy ≥ 80%"] {
        assert!(row.contains(said), "{said} is not on the row: {row}");
    }
    let kpi = lines
        .iter()
        .find(|line| line.starts_with("◆ Lines of code"))
        .unwrap();
    assert!(kpi.contains("KPI"), "{kpi}");
    assert!(printed.contains(&format!("  {FOOTNOTE}")), "{printed}");
}

#[test]
fn check_exits_4_while_a_status_is_down_and_never_for_a_kpi() {
    let world = world();
    answer(&world, 79.2);
    refresh_now(&world);
    world.ephor().args(["monitors", "--check"]).assert().code(4);
    world
        .ephor()
        .args(["monitors", "--check", "--json"])
        .assert()
        .code(4);

    answer(&world, 91.0);
    refresh_now(&world);
    let read = page(&world, &[]);
    assert_eq!(read["counts"]["down"], json!(0), "{read}");
    world.ephor().args(["monitors", "--check"]).assert().code(0);
}

/// Every time the store keeps put months back, as if nothing had read the
/// project since.
fn age(value: &mut Value) {
    match value {
        Value::String(text) if text.len() > 10 && text.as_bytes()[10] == b'T' => {
            *text = "2026-01-01T00:00:00Z".to_string();
        }
        Value::Array(values) => values.iter_mut().for_each(age),
        Value::Object(fields) => fields.values_mut().for_each(age),
        _ => {}
    }
}

fn aged(world: &World) {
    let path = world
        .path()
        .join("state/ephor/monitors")
        .join(PROJECT)
        .join("monitors.json");
    let mut kept = read_json(&path);
    age(&mut kept["monitors"]);
    write_json(&path, &kept);
}

fn unreadable(world: &World) {
    world.script("scripts/kpis.sh", "#!/bin/sh\nexit 3\n");
}

#[test]
fn check_exits_4_while_a_status_is_unknown_and_never_for_a_kpi_unknown() {
    let world = world();
    answer(&world, 91.0);
    refresh_now(&world);
    world.ephor().args(["monitors", "--check"]).assert().code(0);
    aged(&world);
    unreadable(&world);
    refresh_now(&world);
    let read = page(&world, &[]);
    assert_eq!(read["counts"]["down"], json!(0), "{read}");
    assert_eq!(read["counts"]["unknown"], json!(3), "{read}");
    world.ephor().args(["monitors", "--check"]).assert().code(4);

    let kpi = World::new();
    kpi.configure(json!({ "projects": { PROJECT: {
        "providers": [],
        "monitors": [
            { "id": "lines", "title": "Lines of code", "every": "1h",
              "from": { "command": "scripts/kpis.sh" } }
        ]
    } } }));
    kpi.script(
        "scripts/kpis.sh",
        "#!/bin/sh\nprintf '%s' '{\"v\":1,\"monitors\":[{\"id\":\"lines\",\"value\":1200}]}' > \"$EPHOR_ANSWER\"\n",
    );
    refresh_now(&kpi);
    aged(&kpi);
    unreadable(&kpi);
    refresh_now(&kpi);
    let read = page(&kpi, &[]);
    assert_eq!(read["counts"]["unknown"], json!(1), "{read}");
    assert_eq!(
        read["projects"][0]["rows"][0]["shows"]["status"],
        json!(false)
    );
    kpi.ephor().args(["monitors", "--check"]).assert().code(0);
}

#[test]
fn check_exits_4_on_a_status_whose_declaration_is_in_error_and_never_on_a_kpi() {
    let world = World::new();
    world.manifest(json!({
        "version": 1,
        "monitors": [
            { "id": "cov", "unit": "%", "healthy": { "min": 80 }, "every": "90s",
              "from": { "command": "scripts/kpis.sh" } }
        ]
    }));
    let read = page(&world, &[]);
    let row = &read["projects"][0]["rows"][0];
    assert_eq!(row["id"], json!("cov"), "{read}");
    assert_eq!(row["shows"]["status"], json!(true), "{read}");
    assert_eq!(row["shows"]["health"]["state"], json!("not read"), "{read}");
    assert_eq!(read["counts"]["unknown"], json!(1), "{read}");
    world.ephor().args(["monitors", "--check"]).assert().code(4);

    world.manifest(json!({
        "version": 1,
        "monitors": [
            { "id": "lines", "kind": "number", "every": "90s",
              "from": { "command": "scripts/kpis.sh" } }
        ]
    }));
    let read = page(&world, &[]);
    assert_eq!(read["counts"]["unknown"], json!(1), "{read}");
    world.ephor().args(["monitors", "--check"]).assert().code(0);
}

#[test]
fn printing_the_page_runs_no_monitor() {
    let world = world();
    answer(&world, 79.2);
    refresh_now(&world);
    answer(&world, 95.0);
    for _ in 0..2 {
        let read = page(&world, &[]);
        assert_eq!(read["projects"][0]["rows"][0]["value"], json!(79.2));
    }
}

#[test]
fn the_window_and_the_project_are_arguments() {
    let world = world();
    answer(&world, 79.2);
    refresh_now(&world);

    let read = page(&world, &["--window", "30d", "--project", PROJECT]);
    assert_eq!(read["window"], json!("30d"));
    assert_eq!(ids(&read), ["coverage", "green", "lines"]);
    let printed = table(&world, &["--window", "1y"]);
    assert!(
        printed.lines().next().unwrap().contains("window 1y"),
        "{printed}"
    );

    world
        .ephor()
        .args(["monitors", "--window", "2d"])
        .assert()
        .code(2);
    world
        .ephor()
        .args(["monitors", "--project", "ghost"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("ghost"));
}
