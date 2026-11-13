//! How old a project's cache is, and which of its histories were never asked,
//! judged from the project's **bindings** rather than from what the cache
//! holds (§FS-011-command-line.12).
//!
//! This is the one rule. `status` reads it to decide whether to refetch, `feed`
//! to say a project is past its TTL, the status header to say how old it is,
//! the rebase sweep to decide whether it can tell what is under review
//! (§FS-004-quick-actions.6.1), and `mark-read` to decide whether a missing
//! matter is gone or only unknown (§FS-007-matters.5). Two readers with two
//! rules would make one cache fresh to one and stale to the other.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::feed::cache::ProjectFeed;
use crate::feed::config::ProjectFeedConfig;
use crate::feed::history::History;

/// The sources a project binds itself, by name and in configuration order.
pub fn bound(project_config: &ProjectFeedConfig) -> Vec<&str> {
    let mut names: Vec<&str> = Vec::new();
    for name in project_config
        .providers
        .iter()
        .filter_map(|block| block.get("provider").and_then(Value::as_str))
    {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// One history of the project's own, or a bound source the cache holds no
/// history for.
#[derive(Debug, Clone, Copy)]
pub struct Own<'a> {
    pub source: &'a str,
    /// None where a bound source has no history in the cache: it was never
    /// asked, which is not the same as having answered with nothing.
    pub history: Option<&'a History>,
}

impl Own<'_> {
    /// When a refresh last asked it; None where it never was.
    pub fn asked_at(&self) -> Option<DateTime<Utc>> {
        self.history.and_then(|history| history.asked_at)
    }

    /// Whether its last asking answered completely.
    pub fn answered(&self) -> bool {
        self.history.is_some_and(History::answered)
    }
}

/// The project's own histories (§FS-011-command-line.12): every history of
/// each source it binds — every capability that source declared, a bound
/// source with none in the cache standing as one never asked — and of each
/// task store it was read from. A site source's histories are not the
/// project's: `status` refetches only its own sources, so an old one would make
/// every `status` refetch a project its own refresh can never make fresh.
pub fn own<'a>(feed: Option<&'a ProjectFeed>, bound: &[&'a str]) -> Vec<Own<'a>> {
    let mut own = Vec::new();
    for &source in bound {
        match feed.and_then(|feed| feed.providers.get(source)) {
            Some(slot) if !slot.histories.is_empty() => {
                own.extend(slot.histories.iter().map(|history| Own {
                    source,
                    history: Some(history),
                }))
            }
            _ => own.push(Own {
                source,
                history: None,
            }),
        }
    }
    // A task store is found rather than bound (§FS-006-project-interface.7):
    // what the cache holds of one is the project's own.
    for (name, slot) in feed.into_iter().flat_map(|feed| &feed.providers) {
        if !bound.contains(&name.as_str()) && crate::seams::tasks::Kind::parse(name).is_some() {
            own.extend(slot.histories.iter().map(|history| Own {
                source: name.as_str(),
                history: Some(history),
            }));
        }
    }
    own
}

/// A project's age (§FS-011-command-line.12): the oldest time a refresh asked
/// any of its own histories. None — *never fetched* — where any of them was
/// never asked. A project whose refresh asks nothing of its own, which is a
/// legal configuration, takes the time a refresh run last asked it.
pub fn age(feed: Option<&ProjectFeed>, bound: &[&str]) -> Option<DateTime<Utc>> {
    let own = own(feed, bound);
    if own.is_empty() {
        return feed.and_then(|feed| feed.fetched_at);
    }
    own.iter()
        .map(Own::asked_at)
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .min()
}

/// Whether any of the project's own histories was never asked: what it would
/// have reported is unknown rather than absent (§FS-011-command-line.12).
pub fn never_asked(feed: Option<&ProjectFeed>, bound: &[&str]) -> bool {
    own(feed, bound)
        .iter()
        .any(|history| history.asked_at().is_none())
}

/// Whether the cache is younger than `ttl` by the project's age: what `status`
/// decides a refetch on, and what `feed` names past it.
pub fn younger_than(feed: Option<&ProjectFeed>, bound: &[&str], ttl: u64) -> bool {
    age(feed, bound).is_some_and(|asked| (Utc::now() - asked).num_seconds() < ttl as i64)
}

#[cfg(test)]
#[path = "age_tests.rs"]
mod tests;
