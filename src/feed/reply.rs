//! Replies to a conversation: reading the target out of a thread's `reply`
//! descriptor, and sending one back through the provider that reported the
//! conversation.
//!
//! A channel that can carry a reply says so by putting a `reply` descriptor on
//! its thread, in the pattern a message uses for `react`
//! (§FS-007-matters.4). Whose descriptor it is, and how a reply is sent to it,
//! belongs to the source that wrote it (§REQ-001-boundary.5). A channel that
//! omits it is display-only: the surfaces offer no key, and a drafted answer
//! is what the reader copies (§FS-005-dispatch.13), which is a stated degrade
//! rather than a failure (§REQ-001-boundary.1).

use serde_json::Value;

use crate::error::{EphorError, Result};
use crate::feed::config::Defaults;
use crate::feed::providers::{self, forge_call, NativeWrite, Sources};
use crate::forge::{Forge, Request};

/// Where a posted reply goes. Parsed from a thread's `reply` descriptor.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplyTarget {
    /// A source ephor implements itself, which sends the reply directly.
    Native(NativeWrite),
    /// Any source reached through the forge interface: the descriptor is that
    /// implementation's own and goes back to it verbatim
    /// (§FS-001-forge-interface.1).
    Forge { source: String, target: Value },
}

/// Parse a thread's `reply` descriptor, if it has a usable one. `source` is
/// the item's source, which is who to hand a descriptor ephor does not
/// recognize back to.
pub fn parse_target(thread: &Value, source: &str) -> Option<ReplyTarget> {
    let reply = thread.get("reply").filter(|reply| !reply.is_null())?;
    if providers::claims_write(reply) {
        return providers::native_write(reply).map(ReplyTarget::Native);
    }
    Some(ReplyTarget::Forge {
        source: source.to_string(),
        target: reply.clone(),
    })
}

/// A reply that can go out: the words, the source that will carry them, and
/// everything that source needs, resolved and checked.
pub struct Prepared {
    text: String,
    carrier: Carrier,
    reconciliation: bool,
}

enum Carrier {
    Native(NativeWrite),
    Forge {
        forge: Box<dyn Forge>,
        request: Request,
        target: Value,
    },
}

#[cfg(test)]
#[path = "reply_tests.rs"]
mod outcome_tests;

/// Everything a reply does short of sending it (§FS-001-forge-interface.9):
/// the words are settled, the source that reported the conversation is found
/// wherever it is bound, and it is asked whether it can carry a reply at all.
/// A dry run is this and nothing more, so it refuses exactly where the send
/// would.
///
/// The descriptor on the thread says where a reply goes; the `replies`
/// capability says the forge can send one, and a reply goes only where both
/// hold (§FS-001-forge-interface.1). A forge that wrote a descriptor without
/// declaring the capability is refused by name rather than handed words it
/// never said it could deliver.
pub fn prepare(
    target: &ReplyTarget,
    text: &str,
    sources: &Sources,
    defaults: &Defaults,
) -> Result<Prepared> {
    let text = text.trim();
    if text.is_empty() {
        return Err(EphorError::Command("There is nothing to post".to_string()));
    }
    let command = |err: crate::feed::provider::ProviderError| EphorError::Command(err.to_string());
    let mut reconciliation = false;
    let carrier = match target {
        ReplyTarget::Native(write) => Carrier::Native(write.clone()),
        ReplyTarget::Forge { source, target } => {
            let (forge, request) = forge_call(sources, source, defaults).map_err(command)?;
            let capabilities = forge.capabilities().map_err(command)?;
            if !capabilities.replies {
                return Err(EphorError::Command(format!(
                    "{source} does not send replies"
                )));
            }
            reconciliation = capabilities.reply_reconciliation;
            Carrier::Forge {
                forge,
                request,
                target: target.clone(),
            }
        }
    };
    Ok(Prepared {
        text: text.to_string(),
        carrier,
        reconciliation,
    })
}

impl Prepared {
    /// Send it. The text is the reader's — edited or as it was drafted — and
    /// it goes out exactly as it stands (§FS-005-dispatch.13).
    pub fn send(self) -> Result<crate::forge::ReplyOutcome> {
        match self.carrier {
            Carrier::Native(write) => providers::post_reply(&write, &self.text)
                .map(|()| crate::forge::ReplyOutcome::Accepted),
            Carrier::Forge {
                forge,
                request,
                target,
            } => forge
                .reply(&request, &target, &self.text)
                .map_err(|err| EphorError::Command(err.to_string())),
        }
    }

    /// Save the prepared payload and original carrier context, before delivery
    /// (§FS-005-dispatch.13, §FS-001-forge-interface.9).
    pub fn payload(&self) -> (&str, Option<&Request>, bool) {
        let request = match &self.carrier {
            Carrier::Native(_) => None,
            Carrier::Forge { request, .. } => Some(request),
        };
        (&self.text, request, self.reconciliation)
    }

    /// Replay carries the original request context and bytes; capability checks
    /// use today's binding without preparing new words (§FS-005-dispatch.13).
    pub fn restore(&mut self, text: &str, saved: Option<&Request>) {
        self.text = text.to_string();
        if let (Carrier::Forge { request, .. }, Some(saved)) = (&mut self.carrier, saved) {
            *request = saved.clone();
        }
    }
}

/// Prepare the reply and send it.
pub fn post(
    target: &ReplyTarget,
    text: &str,
    sources: &Sources,
    defaults: &Defaults,
) -> Result<()> {
    match prepare(target, text, sources, defaults)?.send()? {
        crate::forge::ReplyOutcome::Accepted => Ok(()),
        crate::forge::ReplyOutcome::Unknown { note } => Err(EphorError::Command(note)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A descriptor one of ephor's own providers wrote is carried out by that
    /// provider, and the engine holds it without knowing whose it is.
    #[test]
    fn parse_target_reads_a_native_descriptor() {
        let descriptor = providers::github::target_json(Some("github.example.com"), "PR_1");
        let write = providers::native_write(&descriptor).expect("a usable descriptor");
        let thread = json!({ "messages": [], "reply": descriptor });
        assert_eq!(
            parse_target(&thread, "github-prs"),
            Some(ReplyTarget::Native(write))
        );
    }

    /// A descriptor ephor does not recognize belongs to the forge that wrote
    /// it and goes back there verbatim (§FS-001-forge-interface.1).
    #[test]
    fn an_unrecognized_descriptor_goes_back_to_its_forge() {
        let thread = json!({ "reply": { "kind": "review-thread", "id": "t-9" } });
        assert_eq!(
            parse_target(&thread, "forge"),
            Some(ReplyTarget::Forge {
                source: "forge".to_string(),
                target: json!({ "kind": "review-thread", "id": "t-9" }),
            })
        );
    }

    /// A channel that declared nothing carries nothing: no target, and so no
    /// key on any surface (§FS-007-matters.4).
    #[test]
    fn a_channel_that_declares_no_reply_has_no_target() {
        assert_eq!(
            parse_target(&json!({ "messages": [{ "author": "ada" }] }), "forge"),
            None
        );
        assert_eq!(parse_target(&json!({ "reply": null }), "forge"), None);
    }

    /// Nothing is posted for an empty reply — a proposal edited down to
    /// whitespace is a proposal withdrawn, not a blank comment.
    #[test]
    fn an_empty_reply_is_refused_before_any_provider_is_asked() {
        let target = ReplyTarget::Forge {
            source: "forge".to_string(),
            target: json!({ "id": "t-9" }),
        };
        let err = post(&target, "  \n\n", &Sources::default(), &Defaults::default()).unwrap_err();
        assert!(err.to_string().contains("nothing to post"), "{err}");
    }

    /// A forge that wrote a reply descriptor without declaring `replies` is
    /// refused by name, on a dry run exactly as on a send, and is never handed
    /// the words (§FS-001-forge-interface.9).
    #[cfg(unix)]
    #[test]
    fn a_forge_that_declared_no_replies_is_refused_before_the_words_go_out() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let mute = dir.path().join("ephor-forge-mute");
        std::fs::write(
            &mute,
            "#!/bin/sh\n\
             here=$(dirname \"$0\")\n\
             cat > /dev/null\n\
             echo \"$1\" >> \"$here/calls\"\n\
             case \"$1\" in\n\
               capabilities) printf '{\"conversation\":true}' ;;\n\
               *) printf '{}' ;;\n\
             esac\n",
        )
        .unwrap();
        std::fs::set_permissions(&mute, std::fs::Permissions::from_mode(0o755)).unwrap();
        let sources = Sources {
            project: String::new(),
            own: Vec::new(),
            site: vec![json!({ "provider": "mute", "command": mute.to_string_lossy() })],
        };
        let target = ReplyTarget::Forge {
            source: "mute".to_string(),
            target: json!({ "chat": "120363@g.us" }),
        };

        let dry = prepare(&target, "Friday works.", &sources, &Defaults::default())
            .err()
            .expect("the rehearsal refuses");
        let sent = post(&target, "Friday works.", &sources, &Defaults::default()).unwrap_err();
        assert_eq!(dry.to_string(), "mute does not send replies");
        assert_eq!(
            sent.to_string(),
            dry.to_string(),
            "and the move in the same words"
        );
        let calls = std::fs::read_to_string(dir.path().join("calls")).unwrap();
        assert_eq!(
            calls.lines().collect::<Vec<_>>(),
            ["capabilities", "capabilities"]
        );
    }
}
