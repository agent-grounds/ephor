//! Fresh processes pin the demonstrated permission fault and operation reuse
//! (§FS-005-dispatch.13, §FS-001-forge-interface.2, §FS-011-command-line.4,
//! §FS-011-command-line.7). No live carrier is used.

use super::*;
use support::shaped;

fn record(world: &MailWorld) -> (std::path::PathBuf, Value) {
    let path = fs::read_dir(world.world.path().join("state/ephor/replies"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "json"))
        .unwrap();
    let value = read_json(&path);
    (path, value)
}

#[cfg(unix)]
#[test]
fn drafted_and_typed_unknown_hold_across_directory_permission_restoration_refresh_and_restart() {
    use std::os::unix::fs::PermissionsExt;
    // Root bypasses this actual permission obstruction. Writer-stage tests
    // remain unconditional; execute this target as a regular user as well.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("permission regression requires a non-root user; skipped");
        return;
    }
    for drafted in [false, true] {
        for choice in ["sent", "not-sent"] {
            let world = MailWorld::new(true);
            let draft = drafted.then(|| world.drafted("Thanks"));
            let fixture = world.world.path().join("reply-forge.py");
            let source = fs::read_to_string(&fixture).unwrap();
            let needle = "    if consume(\"unknown\"):\n";
            assert!(source.contains(needle));
            fs::write(&fixture, source.replace(needle,
                "    if consume(\"unknown\"):\n        os.chmod(root / 'state/ephor/replies', 0o555)\n"
            )).unwrap();
            world.flag("unknown");
            let first = world.reply(if drafted { None } else { Some("Thanks") });
            let directory = world.world.path().join("state/ephor/replies");
            // Restore before assertions so a failing assertion cannot leave an
            // undeletable scratch directory or affect later scenarios.
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
            let diagnostic = says(&first);
            assert!(!first.status.success());
            assert!(
                diagnostic.contains("delivery unknown") && diagnostic.contains("Permission denied"),
                "{diagnostic}"
            );
            let (path, raw) = record(&world);
            assert_eq!(raw["version"], 2);
            assert!(raw["evidence"].is_string());
            assert_eq!(
                raw["intent"]["status"], "pending",
                "actual replacement was obstructed"
            );
            assert_eq!(world.requests().len(), 1);
            assert_eq!(world.delivered(), 0);
            if let Some(path) = &draft {
                fs::write(path, "Edited words").unwrap();
            }
            world.incoming();
            world.refresh();
            let thread = world.thread();
            assert_eq!(thread["pending_reply"]["status"], "held");
            assert_eq!(thread["pending_reply"]["retry"], false);
            assert_eq!(thread["pending_reply"]["target"], json!({"in_reply_to":H}));
            assert_eq!(thread["pending_reply"]["text"], "Thanks");
            assert!(thread["pending_reply"]["note"]
                .as_str()
                .unwrap()
                .contains("delivery unknown"));
            assert_eq!(
                thread["pending_reply"]["resolutions"],
                json!(["sent", "not-sent"])
            );
            let before = tree(&world.world.path().join("state/ephor"));
            let dry = world.run(&["reply", ITEM, "--dry-run", "--json"]);
            assert!(!dry.status.success());
            assert_eq!(shaped("outcome", &dry)["ok"], false);
            let dry_decision =
                world.ok(&["reply", ITEM, "--resolve", choice, "--dry-run", "--json"]);
            assert_eq!(shaped("outcome", &dry_decision)["ok"], true);
            assert_eq!(tree(&world.world.path().join("state/ephor")), before);
            let retry = world.reply(if drafted { None } else { Some("Thanks") });
            assert!(!retry.status.success() && says(&retry).contains("held"));
            assert_eq!(world.requests().len(), 1);
            assert_eq!(read_json(&path), raw, "refused retry did not rewrite state");
            world.flag("absent");
            world.refresh();
            let feed = shaped("feed", &world.ok(&["feed", "--json"]));
            assert!(feed
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["id"] == ITEM));
            assert_eq!(world.thread()["pending_reply"]["retry"], false);
            let gateway = tree(&world.world.path().join("mail"));
            let resolution = world.ok(&["reply", ITEM, "--resolve", choice, "--json"]);
            assert_eq!(shaped("outcome", &resolution)["ok"], true);
            assert_eq!(read_json(&path)["intent"]["status"], choice);
            assert_eq!(world.requests().len(), 1);
            assert_eq!(tree(&world.world.path().join("mail")), gateway);
            if let Some(path) = draft {
                if choice == "sent" {
                    assert!(!path.exists());
                } else {
                    assert_eq!(fs::read_to_string(path).unwrap(), "Edited words");
                }
            }
        }
    }
}

#[test]
fn ordinary_reconciliation_keeps_one_receipt_and_deliberate_repeat_gets_another() {
    for drafted in [false, true] {
        let world = MailWorld::new(true);
        if drafted {
            world.drafted("Thanks");
        }
        let words = if drafted { None } else { Some(" Thanks ") };
        world.uncertain(words);
        let (_, pending) = record(&world);
        assert_eq!(pending["version"], 2);
        let operation = pending["evidence"].clone();
        assert!(operation.is_string());
        world.incoming();
        world.refresh();
        let before = tree(&world.world.path().join("state/ephor"));
        world.ok(&["reply", ITEM, "--dry-run"]);
        assert_eq!(tree(&world.world.path().join("state/ephor")), before);
        let out = world.reply(words);
        assert!(out.status.success(), "{}", says(&out));
        let (_, confirmed) = record(&world);
        assert_eq!(confirmed["evidence"], operation);
        assert_eq!(confirmed["intent"]["status"], "sent");
        assert_eq!(world.wires()[0], world.wires()[1]);
        assert_eq!(world.requests()[1]["target"], json!({"in_reply_to":H}));
        assert_eq!(world.delivered(), 1);
        assert_eq!(world.requests().len(), 2);
        world.ok(&["reply", ITEM, "Thanks"]);
        let (_, repeat) = record(&world);
        assert_ne!(repeat["evidence"], operation);
        assert_eq!(repeat["intent"]["status"], "sent");
        assert_eq!(
            world.requests()[2]["target"],
            json!({"in_reply_to":reply_world::M})
        );
        assert_eq!(world.delivered(), 2);
        assert_eq!(repeat["confirmed"], confirmed["confirmed"]);
        if drafted {
            assert!(world.thread()["draft"].is_null());
        }
    }
}

#[test]
fn legacy_pending_is_read_without_mutation_then_upgraded_before_saved_replay() {
    for drafted in [false, true] {
        let world = MailWorld::new(true);
        if drafted {
            world.drafted("Thanks");
        }
        let words = if drafted { None } else { Some("Thanks") };
        world.uncertain(words);
        let (path, mut saved) = record(&world);
        // Model an existing version-1 site operation, with no new-format field.
        saved["version"] = json!(1);
        saved.as_object_mut().unwrap().remove("evidence");
        write_json(&path, &saved);
        let before = tree(&world.world.path().join("state/ephor"));
        assert_eq!(world.thread()["pending_reply"]["retry"], true);
        world.ok(&["reply", ITEM, "--dry-run"]);
        assert_eq!(tree(&world.world.path().join("state/ephor")), before);
        world.incoming();
        world.refresh();
        assert!(world.reply(words).status.success());
        let snapshot = world.lines("before.jsonl").pop().unwrap();
        let upgraded = snapshot["records"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(
            upgraded["version"], 2,
            "old version-1 Store readers reject this version"
        );
        assert!(upgraded["evidence"].is_string());
        assert_eq!(upgraded["intent"], saved["intent"]);
        assert_eq!(world.wires()[0], world.wires()[1]);
        assert_eq!(world.delivered(), 1);
    }
}
