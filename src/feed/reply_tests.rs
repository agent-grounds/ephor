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
