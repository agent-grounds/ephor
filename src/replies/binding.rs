//! Capture and compare the conversation answered by one request
//! (§FS-005-dispatch.4, §FS-005-dispatch.13).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

use super::Record;
use crate::feed::{config::Defaults, model::Item, providers::Sources};
use crate::forge::Request;

/// Provenance never refreshed underneath a proposal (§FS-005-dispatch.13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub row: String,
    pub source: String,
    pub project: String,
    pub config: Value,
    pub context: String,
    pub user: Option<String>,
    pub thread: usize,
    pub messages: Vec<Value>,
    pub target: Option<Value>,
    pub generation: u64,
    pub path: PathBuf,
}

/// Only authorship, timestamp, words and ownership change a baseline;
/// reactions and task state do not (§FS-005-dispatch.13).
pub fn fingerprints(thread: &Value) -> Vec<Value> {
    thread
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|message| {
            json!({
                "author": message.get("author").cloned().unwrap_or(Value::Null),
                "when": message.get("when").cloned().unwrap_or(Value::Null),
                "text": message.get("text").cloned().unwrap_or(Value::Null),
                "mine": message.get("mine").cloned().unwrap_or(Value::Bool(false)),
            })
        })
        .collect()
}

/// Recorded shown threads, with no network reading (§FS-005-dispatch.13).
pub fn threads(item: &Item) -> &[Value] {
    item.raw
        .get("threads")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

impl Binding {
    /// Last shown sendable thread, or last shown thread for copy-only work
    /// (§FS-005-dispatch.13). Typed sends subsequently require a descriptor.
    pub fn capture(
        item: &Item,
        sources: &Sources,
        defaults: &Defaults,
        record: &Record,
        path: PathBuf,
    ) -> Option<Self> {
        let shown = |thread: &&Value| !fingerprints(thread).is_empty();
        let (index, thread) = threads(item)
            .iter()
            .enumerate()
            .filter(|(_, thread)| {
                shown(thread) && crate::feed::reply::parse_target(thread, &item.source).is_some()
            })
            .next_back()
            .or_else(|| {
                threads(item)
                    .iter()
                    .enumerate()
                    .filter(|(_, thread)| shown(thread))
                    .next_back()
            })?;
        let (config, context) = sources
            .find(&item.source)
            .map(|(config, context)| (config.clone(), context.to_string()))
            .unwrap_or((Value::Null, item.project.clone()));
        let mut binding = Self {
            row: item.id.clone(),
            source: item.source.clone(),
            project: item.project.clone(),
            config,
            context,
            user: defaults.reply_user(),
            thread: index,
            messages: fingerprints(thread),
            target: thread
                .get("reply")
                .filter(|value| !value.is_null())
                .cloned(),
            generation: 0,
            path,
        };
        binding.generation = record.generation(&binding);
        Some(binding)
    }

    /// Local generations belong to a thread's saved first message and position,
    /// never the whole row (§FS-005-dispatch.13).
    pub fn key(&self) -> String {
        json!([self.thread, self.messages.first()]).to_string()
    }

    /// The advancing message is named even when no target identity survived
    /// (§FS-005-dispatch.13).
    pub fn label(message: &Value) -> String {
        format!(
            "{} {}: {}",
            message["author"].as_str().unwrap_or("unknown author"),
            message["when"].as_str().unwrap_or("unknown time"),
            message["text"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(180)
                .collect::<String>()
        )
    }

    /// Compare only this bound thread. Missing, reordered or ambiguous identity
    /// refuses safely; unrelated thread activity is immaterial (§FS-005-dispatch.13).
    pub fn freshness(&self, item: &Item, record: &Record) -> Result<(), String> {
        if self.row != item.id || self.source != item.source {
            return Err("Draft is stale: its row or source changed".into());
        }
        if record.generation(self) != self.generation {
            let words = record
                .accepted
                .get(&self.key())
                .map(|accepted| accepted.text.as_str())
                .unwrap_or("a reply");
            return Err(format!(
                "Draft is stale: an accepted own send advanced this thread: {words}"
            ));
        }
        let all: Vec<Vec<Value>> = threads(item).iter().map(fingerprints).collect();
        if self.messages.is_empty()
            || all
                .iter()
                .filter(|messages| messages.starts_with(&self.messages))
                .count()
                != 1
        {
            return Err("Draft is stale: bound thread is missing, reordered or ambiguous".into());
        }
        let Some(messages) = all
            .get(self.thread)
            .filter(|messages| messages.starts_with(&self.messages))
        else {
            return Err("Draft is stale: bound thread moved or disappeared".into());
        };
        if let Some(newer) = messages.get(self.messages.len()) {
            return Err(format!("Draft is stale: {}", Self::label(newer)));
        }
        if threads(item)[self.thread]
            .get("reply")
            .filter(|value| !value.is_null())
            != self.target.as_ref()
        {
            return Err(
                "Draft is stale: the bound reply target changed; refresh and request a new draft"
                    .into(),
            );
        }
        Ok(())
    }

    /// Recovery must find the original configured binding and scope, including
    /// an original site source now shadowed by a project (§FS-001-forge-interface.9).
    pub fn routing(&self, sources: &Sources, defaults: &Defaults) -> Result<(), String> {
        let Some((config, context)) = sources.find(&self.source) else {
            return Err(format!(
                "Original source binding {} is no longer configured",
                self.source
            ));
        };
        if config != &self.config || context != self.context || defaults.reply_user() != self.user {
            return Err("Original source binding/account or project/site context changed; check the channel before resolving".into());
        }
        Ok(())
    }

    /// The saved native or opaque forge descriptor, without interpretation here
    /// (§FS-001-forge-interface.1).
    pub fn reply_target(&self) -> Option<crate::feed::reply::ReplyTarget> {
        crate::feed::reply::parse_target(&json!({"reply": self.target}), &self.source)
    }

    /// The exact source context prepared for the operation (§FS-001-forge-interface.9).
    pub fn matches_request(&self, request: &Request) -> bool {
        request.config == self.config
            && request.project == self.context
            && request.user == self.user
    }
}
