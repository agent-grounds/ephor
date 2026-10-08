//! A matter's recorded conversation, walked once for both surfaces
//! (§AR-009-surfaces.1).
//!
//! The screen renders these as cards and `ephor thread` prints them as lines;
//! more importantly, both count messages the same way, so the index a reading
//! prints is the index a move takes (§REQ-002-parity.2). A move that addressed
//! messages by a different walk than the reading would be a command nobody can
//! aim.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::feed::model::Item;
use crate::feed::react::{self, ReactTarget};
use crate::feed::reply::{self, ReplyTarget};
use crate::feed::task::{self, Task};
use crate::work::runtime::results::Proposal;

use super::views;

/// One message, with both what a surface shows and what a move needs to act
/// on it. The targets are not part of the reading — they are a source's own
/// descriptors (§FS-001-forge-interface.1), meaningful only on the way back to
/// it — so they ride here and the view carries only whether they exist.
pub struct Message {
    pub thread: usize,
    pub author: String,
    pub when: Option<DateTime<Utc>>,
    pub text: String,
    pub reactions: Vec<views::Reaction>,
    pub react: Option<ReactTarget>,
    pub task: Option<Task>,
    /// The files its source named on it, without the source's id: no move
    /// hands a file back yet, so nothing here needs it
    /// (§FS-011-command-line.4).
    pub attachments: Option<Vec<views::Attachment>>,
}

/// The reply a run drafted, and where it would go (§FS-005-dispatch.13).
pub struct Draft {
    pub text: String,
    pub path: std::path::PathBuf,
    pub thread: usize,
    pub target: Option<ReplyTarget>,
    pub binding: Option<crate::replies::Binding>,
    pub stale_reason: Option<String>,
}

/// A matter's whole conversation: every message in order, and the draft
/// waiting under it where one is.
pub struct Conversation {
    pub messages: Vec<Message>,
    pub draft: Option<Draft>,
    pub pending_reply: Option<views::PendingReply>,
    pub reply_error: Option<String>,
    pub reply_diagnostics: Vec<String>,
    /// Its source declared it can settle the conversation at its source, so
    /// `s` is offered on it (§FS-004-quick-actions.2). Set by
    /// [`Session::conversation`](super::Session::conversation), which holds
    /// the feeds; a walk of the item alone cannot know it.
    pub settles: bool,
}

impl Conversation {
    /// Walk the matter's recorded threads. Empty where the source recorded
    /// none — which is not a failure, only a matter with nothing said on it.
    pub fn of(item: &Item, proposal: Option<Proposal>) -> Conversation {
        let threads = item
            .raw
            .get("threads")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut messages = Vec::new();
        for (index, thread) in threads.iter().enumerate() {
            for message in thread
                .get("messages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                messages.push(parse(index, message, &item.source));
            }
        }
        let saved = crate::replies::Store::inspect(&item.id);
        let reply_error = saved.as_ref().err().map(ToString::to_string);
        let record = saved
            .ok()
            .flatten()
            .unwrap_or_else(|| crate::replies::Record::new(item));
        let draft = proposal
            .filter(|proposal| !record.confirmed.contains(&proposal.path))
            .map(|proposal| draft_of(proposal, &threads, &item.source, &messages, item, &record));
        let pending_reply = record
            .intent
            .as_ref()
            .filter(|intent| intent.unresolved())
            .map(|intent| views::PendingReply {
                thread: intent.binding.thread,
                target: intent.binding.target.clone(),
                text: intent.text.clone(),
                status: if intent.held() { "held" } else { "pending" }.into(),
                retry: !intent.held(),
                note: intent.note.clone(),
                reason: None,
                resolutions: if intent.held() {
                    vec!["sent".into(), "not-sent".into()]
                } else {
                    vec![]
                },
            });
        Conversation {
            messages,
            draft,
            pending_reply,
            reply_error,
            reply_diagnostics: crate::replies::Store::recovery().diagnostics,
            settles: false,
        }
    }

    /// The reading (§FS-011-command-line.4).
    pub fn view(&self, item: &Item) -> views::Thread {
        views::Thread {
            item: item.id.clone(),
            project: item.project.clone(),
            source: item.source.clone(),
            title: item.title.clone(),
            url: item.url.clone(),
            messages: self
                .messages
                .iter()
                .map(|message| views::Message {
                    thread: message.thread,
                    author: message.author.clone(),
                    at: message.when,
                    text: message.text.clone(),
                    reactions: message.reactions.clone(),
                    can_react: message.react.is_some(),
                    task: message.task.as_ref().map(|task| views::MessageTask {
                        text: message.text.clone(),
                        resolved: task.resolved,
                        source: task.source.clone(),
                    }),
                    attachments: message.attachments.clone(),
                })
                .collect(),
            draft: self.draft.as_ref().map(|draft| views::Draft {
                text: draft.text.clone(),
                path: draft.path.clone(),
                thread: draft.thread,
                sendable: draft.target.is_some()
                    && draft.stale_reason.is_none()
                    && self.pending_reply.is_none()
                    && self.reply_error.is_none(),
                target: draft
                    .binding
                    .as_ref()
                    .and_then(|binding| binding.target.clone()),
                stale_reason: draft.stale_reason.clone(),
            }),
            pending_reply: self.pending_reply.clone(),
            reply_error: self.reply_error.clone(),
            reply_diagnostics: self.reply_diagnostics.clone(),
        }
    }
}

/// Bound drafts retain the request's position and opaque target; legacy words
/// remain copyable under the last shown thread (§FS-005-dispatch.13,
/// §FS-007-matters.4).
fn draft_of(
    proposal: Proposal,
    threads: &[Value],
    source: &str,
    messages: &[Message],
    item: &Item,
    record: &crate::replies::Record,
) -> Draft {
    let targets: Vec<Option<ReplyTarget>> = threads
        .iter()
        .map(|thread| reply::parse_target(thread, source))
        .collect();
    // Only threads that made it onto the screen: one with no messages is not
    // somewhere a reader can be shown anything.
    let shown = |index: usize| messages.iter().any(|msg| msg.thread == index);
    let thread = (0..threads.len())
        .filter(|index| shown(*index) && targets.get(*index).is_some_and(Option::is_some))
        .next_back()
        .or_else(|| (0..threads.len()).filter(|index| shown(*index)).next_back())
        .unwrap_or(0);
    let thread = proposal
        .binding
        .as_ref()
        .map(|binding| binding.thread)
        .unwrap_or(thread);
    let stale_reason = match &proposal.binding {
        Some(binding) => binding.freshness(item, record).err(),
        None => {
            Some("This draft is unbound: copy or edit it, type words or request a new draft".into())
        }
    };
    let target = proposal
        .binding
        .as_ref()
        .and_then(|binding| binding.reply_target());
    Draft {
        text: proposal.text,
        path: proposal.path,
        target,
        thread,
        binding: proposal.binding,
        stale_reason,
    }
}

fn parse(thread: usize, value: &Value, source: &str) -> Message {
    let reactions = value
        .get("reactions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|reaction| {
            let emoji = reaction.get("emoji").and_then(Value::as_str)?.to_string();
            let users = reaction
                .get("users")
                .and_then(Value::as_array)
                .map(|users| {
                    users
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|user| !user.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            Some(views::Reaction { emoji, users })
        })
        .collect();
    Message {
        thread,
        author: value
            .get("author")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        when: value
            .get("when")
            .and_then(Value::as_str)
            .and_then(|when| DateTime::parse_from_rfc3339(when).ok())
            .map(|when| when.with_timezone(&Utc)),
        text: value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        reactions,
        react: react::parse_target(value, source),
        task: task::parse(value, source),
        attachments: crate::matter::attachments_of(value)
            .map(|files| files.iter().map(views::Attachment::from).collect()),
    }
}
