//! The shared send, recovery and checked-resolution move (§FS-005-dispatch.13,
//! §FS-011-command-line.4). Surfaces supply intent, never delivery policy.

use super::{act::Sending, session::Session, views};
use crate::error::Result;
use crate::feed::model::Item;
use crate::forge::ReplyOutcome;
use crate::replies::storage::{error, ReplyStorage};
use crate::replies::{Binding, Intent, Record, Status, Store};

/// A checked local decision does not change the forge's ledger
/// (§FS-005-dispatch.13, §FS-011-command-line.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Resolution {
    Sent,
    NotSent,
}

impl Session {
    /// One shared move for both surfaces, holding the row lock until its durable
    /// outcome is recorded (§FS-005-dispatch.13).
    pub(crate) fn reply_move(
        &self,
        item: &Item,
        words: Option<&str>,
        resolution: Option<Resolution>,
        sending: Sending,
    ) -> views::Outcome {
        match self.perform_reply(item, words, resolution, sending) {
            Ok(says) => views::Outcome::ok(says),
            Err(err) => views::Outcome::refused(err.to_string()),
        }
    }

    /// Check and record a held outcome without sending or accepting words
    /// (§FS-005-dispatch.13).
    pub fn resolve_reply(
        &self,
        item: &Item,
        words: Option<&str>,
        resolution: Resolution,
        sending: Sending,
    ) -> views::Outcome {
        self.reply_move(item, words, Some(resolution), sending)
    }

    /// A saved unresolved row remains addressable after disappearing from the
    /// feed; malformed records are never treated as absence (§FS-001-forge-interface.9).
    pub fn reply_item(&self, row: &str) -> Result<Option<Item>> {
        if let Some(item) = self.item(row) {
            return Ok(Some(item));
        }
        Ok(Store::inspect(row)?
            .filter(|record| record.intent.as_ref().is_some_and(Intent::unresolved))
            .map(|record| record.row))
    }

    /// Both inbox readings can retain saved rows without altering the carrier's
    /// cached feed or reporting a synthetic provider (§FS-011-command-line.4).
    pub fn recovery_rows(&self) -> Result<Vec<Item>> {
        Ok(Store::pending_rows()?
            .into_iter()
            .filter(|row| self.item(&row.id).is_none())
            .collect())
    }

    /// Recovery precedes proposal freshness and uses the saved operation;
    /// new sends capture today's words and provenance (§FS-005-dispatch.13).
    fn perform_reply(
        &self,
        item: &Item,
        words: Option<&str>,
        resolution: Option<Resolution>,
        sending: Sending,
    ) -> Result<String> {
        let store = Store::site(&item.id, sending == Sending::Now)?;
        self.perform_reply_locked(item, words, resolution, sending, &store)
    }

    /// A locked persistence seam shared by all outcomes, so fault tests can
    /// exercise actual send/confirmation ordering (§FS-005-dispatch.13).
    pub(crate) fn perform_reply_locked(
        &self,
        item: &Item,
        words: Option<&str>,
        resolution: Option<Resolution>,
        sending: Sending,
        store: &dyn ReplyStorage,
    ) -> Result<String> {
        let mut record = store.read()?.unwrap_or_else(|| Record::new(item));
        if let Some(resolution) = resolution {
            if words.is_some() {
                return Err(error(
                    "Resolution accepts no words and never sends; check the channel first",
                ));
            }
            let intent = record
                .intent
                .as_ref()
                .filter(|intent| intent.unresolved())
                .ok_or_else(|| error("Only a held reply outcome can be resolved"))?;
            // A withdrawn declaration holds recovery on the original binding.
            // A read failure is not evidence of a withdrawn promise, and an
            // eligible currently declaring carrier still requires replay
            // (§FS-005-dispatch.13, §FS-001-forge-interface.9).
            if !intent.held() {
                let sources = self.sources_for(&intent.binding.project);
                intent
                    .binding
                    .routing(&sources, &self.config.defaults)
                    .map_err(error)?;
                let target = intent
                    .binding
                    .reply_target()
                    .ok_or_else(|| error("Saved target is unusable"))?;
                if crate::feed::reply::prepare(
                    &target,
                    &intent.text,
                    &sources,
                    &self.config.defaults,
                )?
                .payload()
                .2
                {
                    return Err(error("Only a held outcome can be resolved; retry the saved send on this reconciling carrier"));
                }
            }
            if sending == Sending::Dry {
                return Ok(format!(
                    "would resolve saved send as {} at target {}:\n\n{}",
                    match resolution {
                        Resolution::Sent => "sent",
                        Resolution::NotSent => "not-sent",
                    },
                    target_label(&intent.binding),
                    intent.text
                ));
            }
            record.intent.as_mut().unwrap().status = Status::Held;
            match resolution {
                Resolution::Sent => record.confirm()?,
                Resolution::NotSent => record.release()?,
            }
            store.save(&record)?;
            return Ok(match resolution {
                Resolution::Sent => retire(&record, "Saved send resolved as sent"),
                Resolution::NotSent => {
                    "Saved send resolved as not-sent; a future send requires the ordinary checks"
                        .into()
                }
            });
        }

        if let Some(intent) = record.intent.as_ref().filter(|intent| intent.unresolved()) {
            if words.is_some_and(|words| words.trim() != intent.text) {
                return Err(error(
                    "Changed words cannot replace the pending saved reply payload",
                ));
            }
            if intent.held() {
                return Err(error(format!("Reply outcome is held. {} Check the channel and resolve with `ephor reply {} --resolve sent|not-sent`; local resolution does not resolve a gateway ledger",
                    intent.note.as_deref().unwrap_or("Delivery is uncertain."), item.id)));
            }
            intent
                .binding
                .routing(
                    &self.sources_for(&intent.binding.project),
                    &self.config.defaults,
                )
                .map_err(error)?;
            let target = intent
                .binding
                .reply_target()
                .ok_or_else(|| error("The saved send has no usable target"))?;
            let mut prepared = crate::feed::reply::prepare(
                &target,
                &intent.text,
                &self.sources_for(&intent.binding.project),
                &self.config.defaults,
            )?;
            if !intent.reconciliation || !prepared.payload().2 {
                return Err(error("Current carrier does not declare reconciliation: outcome held. Check the channel and use --resolve sent|not-sent"));
            }
            prepared.restore(&intent.text, intent.request.as_ref());
            if sending == Sending::Dry {
                return Ok(format!(
                    "would retry saved send to {} through {} at target {}:\n\n{}",
                    record.row.title,
                    intent.binding.source,
                    target_label(&intent.binding),
                    intent.text
                ));
            }
            // Already durably pending: do not create a new operation or rebuild
            // its context when recovering (§FS-005-dispatch.13).
            return finish(prepared.send(), store, &mut record);
        }

        let sources = self.sources_for(&item.project);
        let (binding, text, draft) = match words {
            Some(words) => {
                let binding = Binding::capture(
                    item,
                    &sources,
                    &self.config.defaults,
                    &record,
                    Default::default(),
                )
                .ok_or_else(|| error("This channel declared no way to send a reply"))?;
                (binding, words.trim().to_string(), None)
            }
            None => {
                let proposal =
                    crate::work::Dispatcher::latest_proposal(item)?.ok_or_else(|| {
                        error("No reply has been drafted here — give the words to send some")
                    })?;
                if record.confirmed.contains(&proposal.path) {
                    return Err(error("This drafted reply has already been sent"));
                }
                let binding = proposal.binding.ok_or_else(|| {
                    error(format!(
                        "This draft is unbound; copy the words from {} or request a new draft",
                        proposal.path.display()
                    ))
                })?;
                binding.freshness(item, &record).map_err(error)?;
                (binding, proposal.text, Some(proposal.path))
            }
        };
        binding
            .routing(&sources, &self.config.defaults)
            .map_err(error)?;
        let target = binding.reply_target().ok_or_else(|| error(match &draft {
            Some(path) => format!("This channel declared no way to send a reply — the words are at {}", path.display()),
            None => "This channel declared no way to send a reply; the typed words were not written anywhere".into(),
        }))?;
        let prepared =
            crate::feed::reply::prepare(&target, &text, &sources, &self.config.defaults)?;
        let (text, request, reconciliation) = prepared.payload();
        if reconciliation
            && record
                .accepted
                .values()
                .any(|accepted| accepted.target == binding.target)
        {
            return Err(error(
                "The confirmed reply descriptor is unchanged; refresh before an intentional repeat",
            ));
        }
        if sending == Sending::Dry {
            return Ok(format!(
                "would send to {} through {} at target {}:\n\n{text}",
                item.title,
                item.source,
                target_label(&binding)
            ));
        }
        record.row = item.clone();
        record.intent = Some(Intent {
            binding,
            text: text.to_string(),
            request: request.cloned(),
            reconciliation,
            draft,
            status: Status::Pending,
            note: None,
        });
        store.save(&record)?;
        finish(prepared.send(), store, &mut record)
    }
}

/// Carrier errors retain durable pending; explicit unknown becomes held. Known
/// acceptance is synced before retiring its proposal (§FS-005-dispatch.13).
fn finish(
    outcome: Result<ReplyOutcome>,
    store: &dyn ReplyStorage,
    record: &mut Record,
) -> Result<String> {
    match outcome? {
        ReplyOutcome::Accepted => {
            record.confirm()?;
            store.save(record)?;
            Ok(retire(record, "↩ reply posted"))
        }
        ReplyOutcome::Unknown { note } => {
            let intent = record
                .intent
                .as_mut()
                .ok_or_else(|| error("No saved reply for carrier outcome"))?;
            intent.status = Status::Held;
            intent.note = Some(note.clone());
            store.save(record)?;
            Err(error(format!(
                "Reply outcome unknown: {note}. Check the channel and use --resolve sent|not-sent"
            )))
        }
    }
}

/// Failure to move the copy cannot undo a durable confirmation (§FS-005-dispatch.13).
fn retire(record: &Record, says: &str) -> String {
    match record
        .intent
        .as_ref()
        .and_then(|intent| intent.draft.as_ref())
    {
        Some(path) => match crate::work::runtime::results::mark_path_posted(path) {
            Ok(()) => says.into(),
            Err(err) => format!("{says} ({err})"),
        },
        None => says.into(),
    }
}

/// Rehearsal and readings show the actual opaque descriptor (§FS-011-command-line.4).
fn target_label(binding: &Binding) -> String {
    binding
        .target
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "no target".into())
}
