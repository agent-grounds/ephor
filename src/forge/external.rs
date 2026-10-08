//! §FS-001-forge-interface.2, out-of-process transport: a forge implemented as
//! an executable rather than as Rust.
//!
//! The executable is named for the forge — `ephor-forge-<name>`, resolved on
//! PATH, following the convention `git`, `gh`, and `kubectl` use — or named
//! outright with `"command"`. ephor runs it once per capability:
//!
//! ```text
//!     ephor-forge-<name> capabilities   <<< '{"config":…,"project":…}'
//!     ephor-forge-<name> pull-requests  <<< '{"config":…,"tickets":[…],…}'
//!     ephor-forge-<name> issues         <<< '{"config":…,"tickets":[…],…}'
//!     ephor-forge-<name> notices        <<< '{"config":…,"tickets":[…],…}'
//!     ephor-forge-<name> messages       <<< '{"config":…,"project":…,…}'
//!     ephor-forge-<name> failures       <<< '{"config":…,"repo":…,"number":…}'
//!     ephor-forge-<name> restart        <<< '{"config":…,"repo":…,"number":…,"scope":…}'
//!     ephor-forge-<name> react          <<< '{"config":…,"target":…,"emoji":…}'
//!     ephor-forge-<name> resolve-task   <<< '{"config":…,"target":…}'
//!     ephor-forge-<name> reply          <<< '{"config":…,"target":…,"text":…}'
//!     ephor-forge-<name> settle         <<< '{"config":…,"target":"<id>"}'
//! ```
//!
//! The [`Request`] goes in on stdin, the answer comes back as JSON on stdout,
//! and stderr is the implementation's own diagnostics. Calls are coarse on
//! purpose: `pull-requests` returns conversation and gate inline, so a refresh
//! costs two spawns rather than one per pull request. `failures` is the one
//! call a refresh never makes — it is asked when a reader opens a red gate, so
//! it may take as long as the forge needs.

use std::process::Command;

use serde_json::{json, Value};

use super::{Capabilities, Conversation, Forge, Issue, Notice, PullRequest, Request, Restarted};
use crate::feed::gate::Failure;
use crate::feed::provider::{command_exists, run_json_stdin, ProviderError};

/// Every subcommand the transport runs, as the protocol spells it
/// (§FS-001-forge-interface.2). One list, so the published schema can be held
/// to it: a subcommand added here and not described there is a move a gateway
/// author has no way to learn about.
pub const SUBCOMMANDS: [&str; 11] = [
    "capabilities",
    "pull-requests",
    "issues",
    "notices",
    "messages",
    "failures",
    "restart",
    "react",
    "resolve-task",
    "reply",
    "settle",
];

/// One literal executable binding, with its provenance retained for diagnostics
/// (§FS-001-forge-interface.2).
pub struct ExternalForge {
    name: String,
    command: String,
    explicit_command: bool,
}

impl ExternalForge {
    /// `command` names one literal executable; only absence selects the
    /// `ephor-forge-<name>` PATH convention (§FS-001-forge-interface.2).
    pub fn new(name: impl Into<String>, command: Option<String>) -> Self {
        let name = name.into();
        let explicit_command = command.is_some();
        let command = command.unwrap_or_else(|| format!("ephor-forge-{name}"));
        ExternalForge {
            name,
            command,
            explicit_command,
        }
    }

    /// Append the protocol subcommand without splitting or evaluating the
    /// executable string, including paths containing spaces. Transport failures
    /// also identify an explicit binding for calls made without an availability
    /// check (§FS-001-forge-interface.2).
    fn call(
        &self,
        subcommand: &str,
        request: &Request,
        extra: Value,
    ) -> Result<Value, ProviderError> {
        debug_assert!(
            SUBCOMMANDS.contains(&subcommand),
            "`{subcommand}` is run and not listed in SUBCOMMANDS"
        );
        let mut payload = serde_json::to_value(request).unwrap_or_else(|_| json!({}));
        if let (Some(target), Some(source)) = (payload.as_object_mut(), extra.as_object()) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }

        let mut command = Command::new(&self.command);
        command.arg(subcommand);
        // Unprefixed: every failure from the transport already opens with
        // `ephor-forge-<name> <subcommand>`, and naming it again here put the
        // command in the message three times before the reader reached the
        // error itself.
        run_json_stdin(
            command,
            Some(serde_json::to_string(&payload).unwrap_or_default()),
            std::time::Duration::from_secs(request.timeout_seconds),
            false,
        )
        .map_err(|err| {
            if self.explicit_command {
                ProviderError(self.explicit_command_failure(&err.0))
            } else {
                err
            }
        })
    }

    /// Keep the transport's reason beside the explicit source/field binding
    /// and the executable-string remedy (§FS-001-forge-interface.2).
    fn explicit_command_failure(&self, reason: &str) -> String {
        format!(
            "source '{}' field 'command': {reason}; 'command' names one literal executable, \
             inline arguments are unsupported; use a wrapper executable for fixed arguments",
            self.name
        )
    }

    fn decode<T: serde::de::DeserializeOwned>(
        &self,
        subcommand: &str,
        value: Value,
    ) -> Result<T, ProviderError> {
        serde_json::from_value(value).map_err(|err| {
            ProviderError(format!(
                "{} {subcommand}: output does not match the forge interface: {err}",
                self.command
            ))
        })
    }
}

impl Forge for ExternalForge {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn available(&self) -> bool {
        command_exists(&self.command)
    }

    /// An out-of-process forge is missing exactly one thing, and naming it is
    /// the difference between an install command and an afternoon: the
    /// configuration names a forge, so an omitted executable is inferred.
    /// An explicit binding instead names the source's `command` field and the
    /// wrapper remedy for fixed arguments (§FS-001-forge-interface.2).
    fn unavailable_reason(&self) -> Option<String> {
        Some(if self.explicit_command {
            self.explicit_command_failure(&format!(
                "executable `{}` could not be found",
                self.command
            ))
        } else {
            format!("`{}` is not on PATH", self.command)
        })
    }

    /// The probe is a real process launch, so it fails for every reason a
    /// fetch does — the executable crashed, the VPN is down, the host refused
    /// the connection. Each of those is reported as itself; answering
    /// "declared nothing" instead would describe a working extension behind an
    /// unreachable host as a broken one.
    fn capabilities(&self) -> Result<Capabilities, ProviderError> {
        // No source's block is in hand yet, and the probe never depends on
        // one; an empty object keeps `config` the object the schema says it
        // always is (§FS-001-forge-interface.2).
        let probe = Request {
            config: json!({}),
            project: String::new(),
            tickets: Vec::new(),
            user: None,
            timeout_seconds: 15,
        };
        let value = self.call("capabilities", &probe, json!({}))?;
        self.decode("capabilities", value)
    }

    fn pull_requests(&self, request: &Request) -> Result<Vec<PullRequest>, ProviderError> {
        let value = self.call("pull-requests", request, json!({}))?;
        self.decode("pull-requests", value)
    }

    fn issues(&self, request: &Request) -> Result<Vec<Issue>, ProviderError> {
        let value = self.call("issues", request, json!({}))?;
        self.decode("issues", value)
    }

    fn notices(&self, request: &Request) -> Result<Vec<Notice>, ProviderError> {
        let value = self.call("notices", request, json!({}))?;
        self.decode("notices", value)
    }

    fn messages(&self, request: &Request) -> Result<Vec<Conversation>, ProviderError> {
        let value = self.call("messages", request, json!({}))?;
        self.decode("messages", value)
    }

    fn failures(
        &self,
        request: &Request,
        repo: &str,
        number: &str,
    ) -> Result<Vec<Failure>, ProviderError> {
        let value = self.call(
            "failures",
            request,
            json!({ "repo": repo, "number": number }),
        )?;
        self.decode("failures", value)
    }

    fn restart(
        &self,
        request: &Request,
        repo: &str,
        number: &str,
        scope: crate::feed::gate::Scope,
    ) -> Result<Restarted, ProviderError> {
        let value = self.call(
            "restart",
            request,
            json!({ "repo": repo, "number": number, "scope": scope.name() }),
        )?;
        // A struct deserializes from a sequence as readily as from a map, so
        // an implementation that answered `[]` — the shape every *listing*
        // subcommand returns — would decode into "asked nothing" and read as a
        // restart that found nothing to do. The one answer here that must not
        // be inventable is that one (§FS-001-forge-interface.1).
        if !value.is_object() {
            return Err(ProviderError(format!(
                "{} restart: expected an object saying what it asked for, got {value}",
                self.command
            )));
        }
        self.decode("restart", value)
    }

    fn react(&self, request: &Request, target: &Value, emoji: &str) -> Result<(), ProviderError> {
        self.call(
            "react",
            request,
            json!({ "target": target, "emoji": emoji }),
        )?;
        Ok(())
    }

    fn resolve_task(&self, request: &Request, target: &Value) -> Result<(), ProviderError> {
        self.call("resolve-task", request, json!({ "target": target }))?;
        Ok(())
    }

    // §FS-001-forge-interface.2: typed outcomes, unchanged target/text requests.
    fn reply(
        &self,
        request: &Request,
        target: &Value,
        text: &str,
    ) -> Result<super::ReplyOutcome, ProviderError> {
        super::ReplyOutcome::from_wire(self.call(
            "reply",
            request,
            json!({ "target": target, "text": text }),
        )?)
    }

    /// The conversation's own id goes out as `target`, a string where every
    /// other write hands back an object; the answer carries nothing ephor
    /// reads, and a refusal is the exit and stderr (§FS-001-forge-interface.2).
    fn settle(&self, request: &Request, id: &str) -> Result<(), ProviderError> {
        self.call("settle", request, json!({ "target": id }))?;
        Ok(())
    }
}
