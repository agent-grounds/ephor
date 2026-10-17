//! In-process carriers retain typed outcomes and the unchanged request seam
//! (§FS-001-forge-interface.1, §FS-001-forge-interface.2).

use super::*;
use crate::feed::provider::ProviderError;
use crate::forge::{Capabilities, ReplyOutcome};
use serde_json::json;
use std::sync::{Arc, Mutex};

struct InProcess {
    outcome: ReplyOutcome,
    calls: Arc<Mutex<Vec<(Request, Value, String)>>>,
}
impl Forge for InProcess {
    fn name(&self) -> String {
        "memory".into()
    }
    fn capabilities(&self) -> std::result::Result<Capabilities, ProviderError> {
        Ok(Capabilities {
            replies: true,
            reply_reconciliation: true,
            ..Default::default()
        })
    }
    fn reply(
        &self,
        request: &Request,
        target: &Value,
        text: &str,
    ) -> std::result::Result<ReplyOutcome, ProviderError> {
        self.calls
            .lock()
            .unwrap()
            .push((request.clone(), target.clone(), text.into()));
        Ok(self.outcome.clone())
    }
}

struct DefaultReply;
impl Forge for DefaultReply {
    fn name(&self) -> String {
        "default".into()
    }
    fn capabilities(&self) -> std::result::Result<Capabilities, ProviderError> {
        Ok(Default::default())
    }
}

#[test]
fn real_prepared_inprocess_send_returns_accepted_unknown_and_default_refusal() {
    for outcome in [
        ReplyOutcome::Accepted,
        ReplyOutcome::Unknown {
            note: "check remote operation".into(),
        },
    ] {
        let calls = Arc::new(Mutex::new(vec![]));
        let forge = InProcess {
            outcome: outcome.clone(),
            calls: calls.clone(),
        };
        let request = Request {
            config: json!({"provider":"memory","user":"original"}),
            project: String::new(),
            tickets: vec![],
            user: None,
            timeout_seconds: 1,
        };
        let target = json!({"opaque":"H"});
        let prepared = Prepared {
            text: "Thanks".into(),
            carrier: Carrier::Forge {
                forge: Box::new(forge),
                request: request.clone(),
                target: target.clone(),
            },
            reconciliation: true,
        };
        assert_eq!(prepared.send().unwrap(), outcome);
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            serde_json::to_value(&calls[0].0).unwrap(),
            serde_json::to_value(&request).unwrap()
        );
        assert_eq!(calls[0].1, target);
        assert_eq!(calls[0].2, "Thanks");
    }
    let request = Request {
        config: json!({}),
        project: String::new(),
        tickets: vec![],
        user: None,
        timeout_seconds: 1,
    };
    assert!(DefaultReply
        .reply(&request, &json!({}), "Thanks")
        .unwrap_err()
        .to_string()
        .contains("does not support"));
    assert!(!DefaultReply.capabilities().unwrap().reply_reconciliation);
}

/// A refusal is one outcome whichever transport carries it: the in-process
/// `Refused` and the executable's `{"status":"refused"}` decode alike, and
/// `post` fails with the shared sentence rather than succeeding
/// (§FS-001-forge-interface.2, §FS-005-dispatch.13.3).
#[cfg(unix)]
#[test]
fn a_refusal_reaches_the_same_outcome_in_process_and_through_an_executable() {
    use std::os::unix::fs::PermissionsExt;
    let note = "descriptor is no longer fresh";
    let dir = tempfile::tempdir().unwrap();
    let command = dir.path().join("ephor-forge-refusing");
    std::fs::write(
        &command,
        "#!/bin/sh\n\
         cat > /dev/null\n\
         case \"$1\" in\n\
           capabilities) printf '{\"replies\":true,\"reply_reconciliation\":true}' ;;\n\
           *) printf '{\"status\":\"refused\",\"note\":\"descriptor is no longer fresh\"}' ;;\n\
         esac\n",
    )
    .unwrap();
    std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o755)).unwrap();
    let request = Request {
        config: json!({"provider":"refusing"}),
        project: String::new(),
        tickets: vec![],
        user: None,
        timeout_seconds: 5,
    };
    let send = |forge: Box<dyn Forge>| {
        Prepared {
            text: "Yes, €100 works.".into(),
            carrier: Carrier::Forge {
                forge,
                request: request.clone(),
                target: json!({"in_reply_to":"H"}),
            },
            reconciliation: true,
        }
        .send()
        .unwrap()
    };
    let refused = ReplyOutcome::Refused { note: note.into() };
    let in_process = send(Box::new(InProcess {
        outcome: refused.clone(),
        calls: Arc::new(Mutex::new(vec![])),
    }));
    let executable = send(Box::new(crate::forge::external::ExternalForge::new(
        "refusing",
        Some(command.to_string_lossy().into_owned()),
    )));
    assert_eq!(in_process, refused);
    assert_eq!(executable, in_process);

    let sources = Sources {
        project: String::new(),
        own: Vec::new(),
        site: vec![json!({ "provider": "refusing", "command": command.to_string_lossy() })],
    };
    let target = ReplyTarget::Forge {
        source: "refusing".into(),
        target: json!({"in_reply_to":"H"}),
    };
    let err = post(
        &target,
        "Yes, €100 works.",
        &sources,
        &crate::feed::config::Defaults::default(),
    )
    .expect_err("a refusal is never reported as success");
    assert_eq!(err.to_string(), not_sent(note));
    assert_eq!(err.to_string(), format!("Reply not sent: {note}"));
}
