//! E2E-085-the-monitors-page-on-a-terminal: `ephor tui` on a real 80 by 24
//! terminal. Waiting's overview line names the monitors
//! (§FS-021-monitors.20.1); `M` opens the Monitors page over it,
//! with the header's counts and the rows worst first, and `M` closes it back
//! (§FS-021-monitors.18.1). `Enter` opens the row's detail view with its chart's
//! cursor and its strip of health, `]` steps to the next monitor in the page's
//! order, and `esc` comes back to the page (§FS-021-monitors.19,
//! §FS-021-monitors.18.10). Python's pty and pyte are the test-only scaffold;
//! the ephor binary is the real path under test.

#[path = "../support.rs"]
mod support;

use serde_json::json;

use support::*;

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
    world.script(
        "scripts/kpis.sh",
        "#!/bin/sh\nprintf '%s' '{\"v\":1,\"monitors\":[{\"id\":\"coverage\",\"value\":79.2},{\"id\":\"lines\",\"value\":1200},{\"id\":\"green\",\"value\":true}]}' > \"$EPHOR_ANSWER\"\n",
    );
    world
        .ephor()
        .args(["refresh", PROJECT, "--monitors-now"])
        .assert()
        .success();
    world
}

/// Drive `ephor tui` through `steps` on an 80 by 24 terminal and return each
/// step's screen by name, with the driver's exit status.
#[cfg(unix)]
fn drive(world: &World, steps: serde_json::Value) -> (Vec<(String, String)>, Option<i32>) {
    let ephor = world.ephor_raw();
    let mut driver = std::process::Command::new("python3");
    // The removals too, so the `ephor` the driver starts inherits no `GIT_*`
    // (§REQ-003-test-isolation.3.3).
    for (key, value) in ephor.get_envs() {
        match value {
            Some(value) => driver.env(key, value),
            None => driver.env_remove(key),
        };
    }
    let output = driver
        .args(["-u", "-c", include_str!("../monitors_screen.py")])
        .arg(ephor.get_program())
        .args(["80", "24"])
        .arg(steps.to_string())
        .output()
        .expect("Python 3 runs the terminal scenario");
    let printed = String::from_utf8_lossy(&output.stdout);
    let screens = printed
        .split("=== ")
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (name, screen) = part.split_once('\n').unwrap_or((part, ""));
            (name.to_string(), screen.to_string())
        })
        .collect();
    assert!(
        output.status.code() == Some(0),
        "the terminal scenario ended with {:?}:\n{printed}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    (screens, output.status.code())
}

fn screen<'a>(screens: &'a [(String, String)], name: &str) -> &'a str {
    &screens
        .iter()
        .find(|(each, _)| each == name)
        .unwrap_or_else(|| panic!("no screen {name}: {screens:?}"))
        .1
}

#[cfg(unix)]
#[test]
fn m_opens_the_monitors_page_and_enter_its_detail_on_an_80_by_24_terminal() {
    let world = world();
    let (screens, status) = drive(
        &world,
        json!([
            ["waiting", "", "monitors: 1 down"],
            ["page", "M", "j/k move  enter open"],
            ["detail", "\r", "cursor:"],
            ["next", "]", "› Build green"],
            ["back", "\u{1b}", "j/k move  enter open"],
            ["closed", "M", "ephor — waiting"],
            ["quit", "q", null]
        ]),
    );
    assert_eq!(status, Some(0));

    let page = screen(&screens, "page");
    assert!(
        page.contains("ephor — monitors   1 down · 0 unknown · 0 failing · 0 warn · 1 ok"),
        "{page}"
    );
    let rows: Vec<&str> = page
        .lines()
        .filter(|line| {
            ["Line coverage", "Build green", "Lines of code"]
                .iter()
                .any(|t| line.contains(t))
        })
        .collect();
    assert_eq!(rows.len(), 3, "{page}");
    assert!(
        rows[0].starts_with("✖ Line coverage") && rows[0].contains("79.2%"),
        "{page}"
    );
    assert!(rows[1].starts_with("● Build green"), "{page}");
    assert!(rows[2].starts_with("◆ Lines of code"), "{page}");
    assert!(
        page.lines().all(|line| line.chars().count() <= 80),
        "nothing is drawn past the terminal's edge:\n{page}"
    );

    let detail = screen(&screens, "detail");
    assert!(
        detail.contains(&format!("ephor — monitors › {PROJECT} › Line coverage")),
        "{detail}"
    );
    assert!(detail.contains("health:"), "{detail}");
    assert!(detail.contains("matter:"), "{detail}");

    let next = screen(&screens, "next");
    assert!(
        next.contains(&format!("ephor — monitors › {PROJECT} › Build green")),
        "] steps to the next monitor in the page's order:\n{next}"
    );

    let back = screen(&screens, "back");
    assert!(!back.contains(" › "), "esc comes back to the page:\n{back}");
    let closed = screen(&screens, "closed");
    assert!(
        !closed.contains("ephor — monitors"),
        "M closes the page:\n{closed}"
    );
}
