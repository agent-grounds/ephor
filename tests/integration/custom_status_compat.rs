//! Compatibility at the custom-status reader, adapter and retained-cache
//! boundaries (§FS-006-project-interface.4, §AR-006-matters.4).

use super::*;
use ephor::feed::model::{Item, ItemKind};

/// An integration target is a downstream crate: this is the unchanged public
/// literal from the review, without a constructor or struct update hiding a
/// new required field (§FS-006-project-interface.4).
#[test]
fn custom_status_answer_public_matter_construction_remains_compatible() {
    use ephor::seams::answer::Matter;
    use serde_json::Map;

    let matter = Matter {
        key: "poll:fixed".to_string(),
        kind: Some("status".to_string()),
        title: None,
        state: Some("waiting".to_string()),
        terminal: Some(false),
        url: None,
        repo: None,
        number: None,
        branch: None,
        time: None,
        refs: Vec::new(),
        reasons: Vec::new(),
        data: Map::new(),
    };
    assert_eq!(matter.terminal, Some(false));
}

/// The custom adapter's list overlay must not change what another consumer
/// reads from the public envelope (§FS-006-project-interface.4).
#[test]
fn custom_status_answer_shared_reader_keeps_passthrough_separate() {
    let data = json!({"refs": ["data-ref"], "reasons": ["data-reason"], "_ephor": null});
    let input = json!({"v": 1, "matters": [
        {"key": "empty", "refs": [], "reasons": [], "data": data},
        {"key": "omitted", "data": data}
    ]});
    let answer = ephor::seams::answer::parse(&input.to_string(), "check", Path::new(".")).unwrap();
    for matter in answer.matters {
        assert!(matter.refs.is_empty());
        assert!(matter.reasons.is_empty());
        assert_eq!(serde_json::Value::Object(matter.data), data);
    }
}

/// The unchanged public Item counterexample keeps legacy finality inference
/// through Item/Matter conversion and serialization (§FS-003-feed-categories.2).
#[test]
fn custom_status_answer_public_item_passthrough_cannot_decide_finality() {
    let item = Item {
        id: "legacy".to_string(),
        project: "demo".to_string(),
        source: "custom-status".to_string(),
        kind: ItemKind::Status,
        role: None,
        title: "legacy JSON".to_string(),
        url: None,
        state: Some("waiting".to_string()),
        needs_response: false,
        updated_at: chrono::Utc::now(),
        raw: json!({
            "_ephor": {
                "custom_status_answer": {"terminal": true, "time_supplied": false}
            }
        }),
    };
    assert!(
        !item.is_finished(),
        "legacy JSON passthrough decided finality"
    );
    assert!(
        !ephor::matter::Matter::of_item(&item).is_finished(),
        "legacy JSON passthrough decided retained-matter finality"
    );
    let reloaded: Item = serde_json::from_value(serde_json::to_value(&item).unwrap()).unwrap();
    assert!(!reloaded.is_finished());
    assert_eq!(reloaded.raw, item.raw);

    let mut closed = item;
    closed.state = Some("done".to_string());
    closed.raw["_ephor"]["custom_status_answer"]["terminal"] = json!(false);
    assert!(closed.is_finished());
    assert!(ephor::matter::Matter::of_item(&closed).is_finished());

    closed.source = "another-provider".to_string();
    closed.raw["_ephor_custom_status"] = json!({"answer": {"terminal": false}});
    assert!(closed.is_finished());
    assert!(ephor::matter::Matter::of_item(&closed).is_finished());
}

fn retained_matters(tmp: &Path) -> Vec<ephor::matter::Matter> {
    let cache: ephor::feed::cache::ProjectFeed =
        serde_json::from_value(custom_answer_cache(tmp)).unwrap();
    assert_eq!(cache.model, ephor::feed::cache::MODEL);
    cache.providers["custom-status"].matters.clone()
}

/// Replay actual answer metadata through format: json. Even an exact copy of
/// the adapter's record becomes inert input at the legacy ingestion boundary,
/// remains inert after cache reload, and cannot retain refresh-time activity
/// (§FS-006-project-interface.4, §FS-003-feed-categories.2).
#[test]
fn custom_status_answer_legacy_json_cannot_replay_answer_provenance() {
    let tmp = tempdir();
    let (input, reporter) = write_custom_answer_fixture(
        tmp.path(),
        &json!({
            "v": 1,
            "matters": [
                {"key": "open", "state": "waiting", "terminal": true},
                {"key": "closed", "state": "done", "terminal": false}
            ]
        }),
    );
    refresh_custom_answer(tmp.path());
    let answers = retained_matters(tmp.path());
    assert!(answers[0].is_finished());
    assert!(!answers[1].is_finished());
    let rows: Vec<_> = answers
        .iter()
        .map(|matter| {
            let mut raw = matter.raw.clone();
            raw["status"] = json!(matter.state);
            raw["title"] = json!(matter.key.as_str());
            raw["needs_response"] = json!(true);
            raw
        })
        .collect();
    replace_custom_answer(&input, &json!(rows));
    make_executable(
        &reporter,
        "#!/usr/bin/env bash\nset -euo pipefail\ncat answer.json\n",
    );
    let config_path = tmp.path().join("status.json");
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    config["projects"]["demo"]["providers"][0]["format"] = json!("json");
    fs::write(config_path, config.to_string()).unwrap();

    let mut previous = None;
    for _ in 0..2 {
        refresh_custom_answer(tmp.path());
        let matters = retained_matters(tmp.path());
        assert_eq!(matters.len(), 2);
        for (index, matter) in matters.iter().enumerate() {
            let expected = index == 1;
            assert_eq!(matter.is_finished(), expected);
            assert_eq!(matter.as_item().is_finished(), expected);
            assert_eq!(matter.needs_response, !expected);
            assert_eq!(matter.raw["_ephor"], rows[index]["_ephor"]);
            assert_eq!(
                matter.raw["_ephor_custom_status"]["passthrough"],
                rows[index]["_ephor_custom_status"]
            );
            assert!(matter.raw["_ephor_custom_status"].get("answer").is_none());
        }
        if let Some(previous) = previous {
            assert!(
                matters[0].updated_at > previous,
                "legacy refresh stopped being activity"
            );
        }
        previous = Some(matters[0].updated_at);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Every non-object JSON shape survives the schema reader, typed-list
/// overlay, provenance insertion and cache reload. All three typed terminal
/// cases remain authoritative beside conflicting data, and omission on a
/// later refresh retains supplied activity (§FS-006-project-interface.4,
/// §FS-003-feed-categories.2).
#[test]
fn custom_status_answer_non_object_metadata_survives_cache() {
    let tmp = tempdir();
    let shapes = vec![
        json!("unrelated-passthrough"),
        json!(null),
        json!(true),
        json!(false),
        json!(42),
        json!(-3),
        json!(1.5),
        json!([]),
        json!(["nested", {"terminal": true}]),
    ];
    let mut rows = Vec::new();
    for shape in &shapes {
        for (terminal, state) in [
            (Some(true), "waiting"),
            (Some(false), "done"),
            (None, "waiting"),
            (None, "done"),
        ] {
            for explicit_lists in [true, false] {
                let origin = if explicit_lists {
                    json!({"answer": {
                        "terminal": state == "waiting", "time_supplied": false
                    }, "passthrough": shape})
                } else {
                    shape.clone()
                };
                let mut row = json!({
                    "key": format!("shape:{}", rows.len()),
                    "state": state,
                    "time": "2026-09-01T00:00:00Z",
                    "data": {
                        "terminal": state == "waiting",
                        "refs": ["passthrough-ref"],
                        "reasons": ["passthrough-reason"],
                        "_ephor": shape,
                        "_ephor_custom_status": origin
                    }
                });
                if let Some(terminal) = terminal {
                    row["terminal"] = json!(terminal);
                }
                if explicit_lists {
                    row["refs"] = json!([]);
                    row["reasons"] = json!([]);
                }
                rows.push(row);
            }
        }
    }
    let (input, _) = write_custom_answer_fixture(tmp.path(), &json!({"v": 1, "matters": rows}));
    for _ in 0..2 {
        refresh_custom_answer(tmp.path());
        let matters = retained_matters(tmp.path());
        assert_eq!(matters.len(), rows.len());
        for (matter, row) in matters.iter().zip(&rows) {
            assert_eq!(matter.raw["_ephor"], row["data"]["_ephor"]);
            assert!(matter.raw.as_object().unwrap().contains_key("_ephor"));
            assert_eq!(
                matter.raw["_ephor_custom_status"]["passthrough"],
                row["data"]["_ephor_custom_status"]
            );
            for field in ["refs", "reasons"] {
                assert_eq!(
                    &matter.raw[field],
                    row.get(field).unwrap_or(&row["data"][field])
                );
            }
            let expected = row
                .get("terminal")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(row["state"] == "done");
            assert_eq!(matter.is_finished(), expected, "{}", row["key"]);
            assert_eq!(matter.as_item().is_finished(), expected, "{}", row["key"]);
            assert_eq!(matter.updated_at.to_rfc3339(), "2026-09-01T00:00:00+00:00");
        }
        for row in &mut rows {
            row.as_object_mut().unwrap().remove("time");
        }
        replace_custom_answer(&input, &json!({"v": 1, "matters": rows}));
    }
}

/// A summary cannot impersonate a typed matter by supplying either metadata
/// spelling; it remains an unfinished, refresh-timed status line
/// (§FS-006-project-interface.4, §FS-003-feed-categories.2).
#[test]
fn custom_status_answer_summary_cannot_supply_matter_provenance() {
    let tmp = tempdir();
    let data = json!({
        "_ephor": {"custom_status_answer": {"terminal": true, "time_supplied": false}},
        "_ephor_custom_status": {"answer": {"terminal": true, "time_supplied": false}}
    });
    write_custom_answer_fixture(
        tmp.path(),
        &json!({
            "v": 1, "summary": "still waiting", "needs_response": true, "data": data
        }),
    );
    let mut previous = None;
    for _ in 0..2 {
        refresh_custom_answer(tmp.path());
        let matter = retained_matters(tmp.path()).remove(0);
        assert!(!matter.is_finished());
        assert!(!matter.as_item().is_finished());
        assert!(matter.needs_response);
        assert_eq!(matter.raw["_ephor"], data["_ephor"]);
        assert_eq!(
            matter.raw["_ephor_custom_status"]["passthrough"],
            data["_ephor_custom_status"]
        );
        if let Some(previous) = previous {
            assert!(matter.updated_at > previous);
        }
        previous = Some(matter.updated_at);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
