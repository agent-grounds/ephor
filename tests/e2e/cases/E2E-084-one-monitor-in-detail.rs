//! E2E-084-one-monitor-in-detail: `ephor monitors show <project>/<id>` prints
//! one monitor's detail view: its health and since, its value and condition
//! and where each part came from, its cadence and hold, its readings, its
//! episodes and its matter (§FS-021-monitors.21.3, §FS-021-monitors.19).
//! `--json` prints the published `monitor` view, with the site's override of
//! the manifest's condition said beside it (§FS-021-monitors.21.5). An id
//! the project does not have is refused, naming the ones it has, and so is a
//! name that is not `<project>/<id>`; the scope selectors are refused,
//! because it is about one monitor.

#[path = "../support.rs"]
mod support;

use serde_json::{json, Value};

use support::*;

fn world() -> World {
    let world = World::new();
    world.manifest(json!({
        "version": 1,
        "monitors": [
            { "id": "coverage", "title": "Line coverage", "unit": "%",
              "healthy": { "min": 75 }, "every": "1h",
              "from": { "command": "scripts/kpis.sh" } }
        ]
    }));
    world.configure(json!({ "projects": { PROJECT: {
        "providers": [],
        "monitors": [
            { "id": "coverage", "healthy": { "min": 80 } },
            { "id": "lines", "title": "Lines of code", "every": "1h",
              "from": { "command": "scripts/kpis.sh" } }
        ]
    } } }));
    world.script(
        "scripts/kpis.sh",
        "#!/bin/sh\nprintf '%s' '{\"v\":1,\"monitors\":[{\"id\":\"coverage\",\"value\":79.2,\"detail\":\"src/ is 71%\"},{\"id\":\"lines\",\"value\":1200}]}' > \"$EPHOR_ANSWER\"\n",
    );
    world
        .ephor()
        .args(["refresh", PROJECT, "--monitors-now"])
        .assert()
        .success();
    world
}

fn shown(world: &World, monitor: &str) -> Value {
    let output = world
        .ephor()
        .args(["monitors", "show", monitor, "--json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    shaped("monitor", &output)
}

#[test]
fn the_detail_says_where_each_part_came_from() {
    let world = world();
    let detail = shown(&world, &format!("{PROJECT}/coverage"));
    assert_eq!(detail["id"], json!("coverage"));
    assert_eq!(detail["title"], json!("Line coverage"));
    assert_eq!(detail["health"], json!("down"));
    assert_eq!(detail["value"], json!(79.2));
    assert_eq!(detail["detail"], json!("src/ is 71%"));
    assert_eq!(detail["window"], json!("7d"));
    assert_eq!(detail["windowed"], json!(true));
    let condition = &detail["condition"];
    assert_eq!(condition["from"], json!("site"));
    assert_eq!(condition["healthy"], json!({ "min": 80.0 }));
    assert_eq!(
        condition["overrides"],
        json!([{ "healthy": { "min": 75.0 }, "from": "manifest" }])
    );
    assert_eq!(detail["cadence"]["from"], json!("manifest"));
    assert_eq!(detail["declared"]["homes"], json!(["site", "manifest"]));
    assert_eq!(detail["declared"]["site_overrides"], json!(["healthy"]));
    assert_eq!(detail["hold"]["drop_after"], json!(1));
    assert_eq!(
        detail["thresholds"],
        json!([{ "at": 80.0, "of": "healthy", "bound": "min" }])
    );
    assert_eq!(detail["readings"].as_array().unwrap().len(), 1);
    assert_eq!(detail["readings"][0]["value"], json!("79.2%"));
    assert_eq!(detail["chart"][0]["health"], json!("down"));
    let episodes = detail["episodes"].as_array().unwrap();
    assert_eq!(episodes.len(), 1, "{episodes:?}");
    assert!(episodes[0].get("end").is_none(), "the drop is still open");
    let matter = &detail["matter"];
    assert_eq!(matter["state"], json!("New"));
    assert_eq!(matter["sort"], json!("drop"));
    assert_eq!(episodes[0]["matter"], matter["key"]);
    assert!(matter["proposal"]["entry"].is_string(), "{matter}");
    assert!(detail.get("no_matter").is_none());

    let kpi = shown(&world, &format!("{PROJECT}/lines"));
    assert_eq!(kpi["shows"]["rank"], json!("kpi"));
    assert!(kpi.get("hold").is_none(), "a KPI has no hold");
    assert_eq!(kpi["no_matter"], json!("none: a KPI is never a matter"));
}

#[test]
fn the_detail_as_text() {
    let world = world();
    let output = world
        .ephor()
        .args([
            "monitors",
            "show",
            &format!("{PROJECT}/coverage"),
            "--window",
            "30d",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let printed = String::from_utf8(output.stdout).unwrap();
    let first = printed.lines().next().unwrap();
    assert!(
        first.starts_with(&format!(
            "ephor — monitors › {PROJECT} › Line coverage   window 30d · "
        )),
        "{printed}"
    );
    for said in [
        "✖ down",
        "value: 79.2% · healthy ≥ 80% · src/ is 71%",
        "from: command scripts/kpis.sh",
        "declared: site, manifest · the site overrides healthy",
        "cadence: every 1h · last read ",
        "hold: drop after 1 failing reading",
        "episodes (1)",
        "readings (1)",
        &format!("matter: monitor:{PROJECT}/coverage@"),
    ] {
        assert!(printed.contains(said), "`{said}` is not said:\n{printed}");
    }
}

#[test]
fn an_id_the_project_does_not_have_is_refused_naming_the_ones_it_has() {
    let world = world();
    world
        .ephor()
        .args(["monitors", "show", &format!("{PROJECT}/branches")])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("it has: coverage, lines"));
    let output = world
        .ephor()
        .args(["monitors", "show", &format!("{PROJECT}/branches"), "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let refused = shaped("outcome", &output);
    assert_eq!(refused["ok"], json!(false));

    world
        .ephor()
        .args(["monitors", "show", "coverage"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("<project>/<id>"));
    world
        .ephor()
        .args(["monitors", "show", "ghost/coverage"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("ghost"));
    world
        .ephor()
        .args([
            "--org",
            "acme",
            "monitors",
            "show",
            &format!("{PROJECT}/coverage"),
        ])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--org"));
}
