//! Controlled carrier startup, distinct from lost acknowledgement
//! (§FS-005-dispatch.13, §FS-001-forge-interface.2, §FS-001-forge-interface.6).
//! Delay only the selected command in one world; never override its timeout.

use super::*;

#[test]
fn healthy_startup_messages_refresh_keeps_the_conversation() {
    for delayed in [false, true] {
        let world = MailWorld::new(false);
        world.incoming();
        if delayed {
            world.delay_startup("messages");
        }
        let out = world.run(&["refresh", "demo"]);
        assert!(
            out.status.success(),
            "healthy messages must answer (delayed={delayed}): {}; startup: {:?}",
            says(&out),
            world.lines("startup.jsonl")
        );
        let thread = world.thread();
        assert!(thread.to_string().contains(NEW_WORDS));
        assert!(world.requests().is_empty());
        assert_eq!(world.delivered(), 0);
        if delayed {
            assert_eq!(world.lines("startup.jsonl"), phases("messages"));
        } else {
            eprintln!("undelayed messages control passed");
        }
    }
}

#[test]
fn healthy_startup_reply_records_and_delivers_once() {
    for delayed in [false, true] {
        let world = MailWorld::new(true);
        if delayed {
            world.delay_startup("reply");
        }
        let out = world.reply(Some("Thanks"));
        assert_eq!(
            world.requests().len(),
            1,
            "healthy reply must reach the carrier (delayed={delayed}): {}; startup: {:?}",
            says(&out),
            world.lines("startup.jsonl")
        );
        assert!(out.status.success(), "{}", says(&out));
        assert_eq!(world.delivered(), 1);
        assert_eq!(world.requests()[0]["target"], json!({"in_reply_to": H}));
        assert_eq!(world.requests()[0]["text"], "Thanks");
        assert_eq!(world.requests()[0]["project"], "demo");
        if delayed {
            assert_eq!(world.lines("startup.jsonl"), phases("reply"));
        } else {
            eprintln!("undelayed reply control passed");
        }
    }
}

fn phases(command: &str) -> Vec<Value> {
    vec![
        json!({"command": command, "phase": "started"}),
        json!({"command": command, "phase": "ready"}),
    ]
}

#[test]
fn healthy_startup_site_capture_retains_recovery_and_resolution() {
    fix_cases::site_context_with_startup_delay(None);
    eprintln!("undelayed site capture control passed");
    fix_cases::site_context_with_startup_delay(Some("reply"));
}

#[test]
fn healthy_startup_site_replacement_retains_recovery_and_resolution() {
    fix_cases::site_context_with_startup_delay(None);
    eprintln!("undelayed site replacement control passed");
    fix_cases::site_context_with_startup_delay(Some("messages"));
}

#[test]
fn healthy_startup_moved_placement_retains_original_routing() {
    fix_cases::moved_placement_with_startup_delay(false);
    eprintln!("undelayed moved placement control passed");
    fix_cases::moved_placement_with_startup_delay(true);
}
