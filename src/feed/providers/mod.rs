//! Provider registry. Adding a provider: create a module implementing
//! `Provider`, then add a match arm in `build_provider`.

mod custom_status;
pub mod forge;
pub(crate) mod github;
mod github_ci;
mod github_issues;
mod github_notifications;
mod github_prs;
mod github_threads;

use serde_json::Value;

use crate::feed::config::ActionConfig;
use crate::feed::model::Item;
use crate::feed::provider::{Provider, ProviderError};
use crate::forge::Forge;

pub fn build_provider(config: &Value) -> Result<Box<dyn Provider>, ProviderError> {
    let name = config
        .get("provider")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError("provider entry is missing 'provider'".to_string()))?;
    match name {
        "github-prs" => Ok(Box::new(github_prs::GithubPrs::from_config(config)?)),
        "github-ci" => Ok(Box::new(github_ci::GithubCi::from_config(config)?)),
        "github-issues" => Ok(Box::new(github_issues::GithubIssues::from_config(config)?)),
        "github-notifications" => Ok(Box::new(
            github_notifications::GithubNotifications::from_config(config)?,
        )),
        "github-threads" => Ok(Box::new(github_threads::GithubThreads::from_config(
            config,
        )?)),
        "custom-status" => Ok(Box::new(custom_status::CustomStatus::from_config(config)?)),
        // Anything else names a forge rather than a built-in provider: reach
        // it out of process (§FS-001-forge-interface.2). `ephor-forge-<name>`
        // on PATH, or an explicit "command". Chat and mail are among them:
        // they reach ephor through a gateway answering the `messages` row
        // (§FS-001-forge-interface.1), never through an adapter named here.
        _ => Ok(Box::new(forge::ForgeProvider::external(config)?)),
    }
}

/// The external source constructor's refusal of an unsupported command
/// binding (§FS-001-forge-interface.2). Construction cannot invoke a forge.
/// Built-in providers have their own command contracts; this does not judge
/// those or classify transport failures as binding refusals.
pub(crate) fn command_refusal(config: &Value) -> Option<ProviderError> {
    let name = config.get("provider").and_then(Value::as_str)?;
    if !built_in(name)
        && config
            .get("command")
            .is_some_and(|value| !value.is_string())
    {
        forge::ForgeProvider::external(config).err()
    } else {
        None
    }
}

/// A write one of ephor's own providers performs itself rather than through
/// the forge interface (§FS-001-forge-interface.1). Which provider it is
/// belongs down here with the providers; above this module a write is a
/// write, and the descriptor it came from is opaque (§REQ-001-boundary.5).
#[derive(Debug, Clone, PartialEq)]
pub struct NativeWrite(github::Target);

/// Whether a `react` or `reply` descriptor is one of ephor's own providers' to
/// carry out. A descriptor that is claimed and unusable is no target at all,
/// which is why claiming and reading are two questions.
pub fn claims_write(descriptor: &Value) -> bool {
    github::claims(descriptor)
}

/// The descriptor read back into a write, where it carries what one needs.
pub fn native_write(descriptor: &Value) -> Option<NativeWrite> {
    github::parse_target(descriptor).map(NativeWrite)
}

/// Post a reaction through the provider that claimed the descriptor.
pub fn post_reaction(write: &NativeWrite, content: &str) -> crate::error::Result<()> {
    github::react(&write.0, content)
}

/// Send a reply through the provider that claimed the descriptor.
pub fn post_reply(write: &NativeWrite, text: &str) -> crate::error::Result<()> {
    github::reply(&write.0, text)
}

/// Whether a provider fetches unscoped — asking nothing about any one project
/// and answering about all of them (§DA-002-fetch-attribution-split). A
/// property of the provider, so it is answered where the providers are.
pub fn is_shared(name: &str) -> bool {
    name == "github-notifications"
}

/// Whether a provider name is one ephor implements itself. The complement of
/// this is the forge case in `build_provider`, kept in one place because a
/// write has to make the same distinction and two copies of the list would
/// drift into a source that fetches one way and writes another.
pub(crate) fn built_in(name: &str) -> bool {
    matches!(
        name,
        "github-prs"
            | "github-ci"
            | "github-issues"
            | "github-notifications"
            | "github-threads"
            | "custom-status"
    )
}

/// Whether a source of this name could carry a pull request at all
/// (§FS-004-quick-actions.6.1).
///
/// Asked of the name, because a feed's slots are keyed by name and a slot that
/// failed has no provider left to ask. Keyed off the same list
/// `build_provider` matches, so a provider added there is answered for here:
/// of ephor's own, three emit one. Every other built-in reports something
/// else, and so does a project's own task store — whatever became of one of
/// those, it was never going to say whether a branch is under review. Anything
/// else is a forge, which declares pull requests among its capabilities: not
/// knowing is the blocking answer, exactly as it is for the branch itself.
pub fn may_carry_pull_requests(name: &str) -> bool {
    match name {
        "github-prs" | "github-ci" | "github-threads" => true,
        _ if built_in(name) => false,
        _ => crate::seams::tasks::Kind::parse(name).is_none(),
    }
}

/// The sources a move on one matter may go back to (§FS-001-forge-interface.9):
/// the entries of the project it is placed under, then the ones bound once for
/// the site.
///
/// Where a matter came from and where it was placed are two facts. A matter a
/// site source reported, and attribution placed under a project, is still
/// that source's to answer, so looking only among the project's own entries
/// found nothing and every move on it failed after the dry run said it would
/// go out.
#[derive(Debug, Clone, Default)]
pub struct Sources {
    /// The project the matter is placed under; empty in the unattributed
    /// bucket, which binds no source of its own.
    pub project: String,
    /// That project's own entries.
    pub own: Vec<Value>,
    /// The entries bound once for the site.
    pub site: Vec<Value>,
}

impl Sources {
    /// The entry named `source`, and the project a move through it is told
    /// (§FS-001-forge-interface.9). Where the project binds a source under
    /// the same name as the site does, the project's own is the one meant; a
    /// site source is told no project on a move, as it is on a fetch.
    pub fn find(&self, source: &str) -> Option<(&Value, &str)> {
        let named = |block: &&Value| block.get("provider").and_then(Value::as_str) == Some(source);
        match self.own.iter().find(named) {
            Some(block) => Some((block, self.project.as_str())),
            None => self.site.iter().find(named).map(|block| (block, "")),
        }
    }
}

/// The forge behind a source and the request to call it with, for the writes
/// that go back to it — a reaction, a ticked task, a reply. Fails where the
/// source is one of ephor's own providers: those reach their host directly,
/// and a caller that lands here with one has a descriptor it should have
/// handled itself.
///
/// The block's own timeout is honored the way a fetch honors it, since a forge
/// that needs a minute to be reached at all needs it for a write too.
pub fn forge_call(
    sources: &Sources,
    source: &str,
    defaults: &crate::feed::config::Defaults,
) -> Result<(Box<dyn Forge>, crate::forge::Request), ProviderError> {
    if built_in(source) {
        return Err(ProviderError(format!(
            "'{source}' is not reached through the forge interface"
        )));
    }
    let (block, project) = sources.find(source).ok_or_else(|| {
        ProviderError(match sources.project.as_str() {
            "" => format!("the site has no source named '{source}' anymore"),
            project => {
                format!("neither '{project}' nor the site has a source named '{source}' anymore")
            }
        })
    })?;
    let timeout = crate::feed::refresh::provider_timeout(block)
        .map(|timeout| timeout.as_secs())
        .unwrap_or(defaults.provider_timeout_seconds);
    let request = crate::forge::Request {
        config: block.clone(),
        project: project.to_string(),
        tickets: Vec::new(),
        user: defaults.github_user.clone(),
        timeout_seconds: timeout,
    };
    Ok((forge::ForgeProvider::external(block)?.into_forge(), request))
}

/// The quick actions the source that produced one item offers on it
/// (§FS-004-quick-actions.1). Only that source is asked — it is the one that
/// knows what the item means — wherever it is bound
/// (§FS-001-forge-interface.9), and a provider block that no longer builds
/// simply offers nothing, since a menu is not the place to report a broken
/// configuration.
pub fn quick_actions(sources: &Sources, item: &Item) -> Vec<ActionConfig> {
    sources
        .find(&item.source)
        .and_then(|(block, _)| build_provider(block).ok())
        .map(|provider| provider.quick_actions(item))
        .unwrap_or_default()
}

/// `serde(default)` for provider flags that are on unless switched off.
pub(crate) fn enabled() -> bool {
    true
}

pub(crate) use crate::seams::summons::quote as shell_quote;

pub(crate) use github::{
    gh_command, github_login, names_under, parse_github_time, restart_actions, show_failing_checks,
};

pub(crate) fn parse_config<T: serde::de::DeserializeOwned>(
    config: &Value,
) -> Result<T, ProviderError> {
    serde_json::from_value(config.clone())
        .map_err(|err| ProviderError(format!("invalid provider config: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feed::config::Defaults;
    use crate::feed::model::ItemKind;
    use crate::feed::provider::command_exists;
    use serde_json::json;

    /// A pull request whose gate is red — one row carrying its gate, which is
    /// what the CI source reports now (§FS-007-matters.5).
    fn failing_ci_item(source: &str) -> Item {
        Item {
            id: "github-ci:acme/widget#42".to_string(),
            project: "widget".to_string(),
            source: source.to_string(),
            kind: ItemKind::Pr,
            role: None,
            title: "Retry window".to_string(),
            url: None,
            state: None,
            needs_response: true,
            updated_at: chrono::Utc::now(),
            raw: json!({
                "repo": "acme/widget",
                "gate": { "repos": [{
                    "repo": "acme/widget", "passed": 1, "failed": 2, "running": 0
                }] }
            }),
        }
    }

    #[test]
    fn only_the_source_that_produced_the_item_is_asked() {
        let blocks = Sources {
            project: "widget".to_string(),
            own: vec![
                json!({ "provider": "custom-status", "command": "true" }),
                json!({ "provider": "github-ci", "repos": ["acme/widget"] }),
            ],
            site: Vec::new(),
        };
        // The github-ci block answers for its own item — where `gh` is
        // installed to answer at all (§FS-004-quick-actions.2): the failures
        // and both restarts (§FS-004-quick-actions.9).
        let offered = quick_actions(&blocks, &failing_ci_item("github-ci"));
        if command_exists("gh") {
            assert_eq!(
                offered
                    .iter()
                    .map(|action| action.description.as_str())
                    .collect::<Vec<_>>(),
                [
                    "see the CI failures",
                    "restart what failed",
                    "restart the whole gate"
                ]
            );
        } else {
            assert!(offered.is_empty());
        }
        // The same item attributed to another source asks that source, which
        // knows nothing about it.
        assert!(quick_actions(&blocks, &failing_ci_item("custom-status")).is_empty());
    }

    /// A move finds its source among the project's own entries first, then the
    /// site's, and a site source is told no project (§FS-001-forge-interface.9).
    #[test]
    fn a_move_finds_its_source_wherever_it_is_bound() {
        let sources = Sources {
            project: "widget".to_string(),
            own: vec![json!({ "provider": "acme", "repos": ["app"] })],
            site: vec![
                json!({ "provider": "acme", "repos": ["elsewhere"] }),
                json!({ "provider": "chatgw", "spool": "~/chat" }),
            ],
        };

        // On a shared name the project's own entry is the one meant.
        let (block, told) = sources.find("acme").unwrap();
        assert_eq!(block["repos"], json!(["app"]));
        assert_eq!(told, "widget");
        // Only the site binds this one, and it is told no project.
        let (block, told) = sources.find("chatgw").unwrap();
        assert_eq!(block["spool"], "~/chat");
        assert_eq!(told, "");
        assert!(sources.find("nobody").is_none());

        let (_, request) = forge_call(&sources, "chatgw", &Defaults::default()).unwrap();
        assert_eq!(request.project, "");
        assert_eq!(
            request.config,
            json!({ "provider": "chatgw", "spool": "~/chat" })
        );
        let (_, request) = forge_call(&sources, "acme", &Defaults::default()).unwrap();
        assert_eq!(request.project, "widget");
    }

    /// A source neither the project nor the site binds any more is named with
    /// both places it was looked for, and a matter in the bucket with the one.
    #[test]
    fn a_source_bound_nowhere_says_where_it_was_looked_for() {
        let placed = Sources {
            project: "widget".to_string(),
            ..Sources::default()
        };
        let err = forge_call(&placed, "chatgw", &Defaults::default())
            .err()
            .unwrap();
        assert_eq!(
            err.0,
            "neither 'widget' nor the site has a source named 'chatgw' anymore"
        );
        let err = forge_call(&Sources::default(), "chatgw", &Defaults::default())
            .err()
            .unwrap();
        assert_eq!(err.0, "the site has no source named 'chatgw' anymore");
    }

    /// The stubs that once answered to these names are gone, so each is now
    /// a forge like any other name: a gateway installed as
    /// `ephor-forge-<name>` answers for it (§FS-001-forge-interface.1).
    #[test]
    fn a_retired_chat_or_mail_name_resolves_as_a_forge() {
        for name in ["slack", "discord", "email"] {
            assert!(!built_in(name), "{name}");
            assert!(!is_shared(name), "{name}");
            let provider = build_provider(&json!({ "provider": name })).unwrap();
            assert_eq!(provider.name(), name);
            assert_eq!(
                provider.unavailable_reason(),
                Some(format!("`ephor-forge-{name}` is not on PATH"))
            );
        }
    }

    #[test]
    fn a_configured_value_survives_becoming_a_shell_word() {
        assert_eq!(shell_quote("ghe.example.com"), "'ghe.example.com'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }
}
