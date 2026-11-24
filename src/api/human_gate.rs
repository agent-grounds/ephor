//! The same local gate review and owner decision on both surfaces
//! (§FS-005-dispatch.9.1), and a prepared gate's answer (§FS-005-dispatch.9.3).

use crate::error::{EphorError, Result};
use crate::feed::model::Item;
use crate::work::runtime::gate::{self, Verdict};
use crate::work::runtime::human_gate::{self, Gate, Review};

use super::Session;

impl Session {
    /// A matter's review, each gate with its brief where the bound runtime
    /// prepares one (§FS-005-dispatch.9.3).
    pub fn human_review(&self, item: &Item) -> Result<Review> {
        let mut review = self.gates_of(item)?;
        gate::brief_review(&self.work_config, &mut review);
        Ok(review)
    }

    /// Whether the bound runtime prepares gate briefs, asked of it once
    /// (§AR-005-capabilities.4): the one fact a surface needs to choose the
    /// form over the review (§FS-005-dispatch.9.3).
    pub fn gate_briefs(&self) -> bool {
        gate::briefs(&self.work_config)
    }

    /// Send one answer document to the runtime, unchanged, as a dry run or
    /// for real, and hand back its verdict (§FS-005-dispatch.9.3). The gate is
    /// read again first, as a transition rereads it; what the answer means is
    /// the runtime's to decide (§DA-012-the-runtime-routes-a-gate-answer).
    pub fn answer_gate(
        &self,
        item: &Item,
        ticket: &str,
        from: &str,
        document: &serde_json::Value,
        dry_run: bool,
    ) -> Result<Verdict> {
        if let Some(reason) = gate::refusal(&self.work_config) {
            return Err(EphorError::Command(reason));
        }
        let review = self.gates_of(item)?;
        let gate = one_gate(&review, ticket, from)?;
        gate::answer(&self.work_config, gate, document, dry_run)
    }

    /// The gates and files of a matter's review, without asking for briefs.
    fn gates_of(&self, item: &Item) -> Result<Review> {
        let mut review = Review::default();
        if let Some((path, plan_id, task)) = human_gate::source_plan(item)? {
            let root = item
                .raw
                .get("task_root")
                .and_then(|value| value.as_str())
                .map(std::path::Path::new)
                .or_else(|| path.parent())
                .ok_or_else(|| EphorError::Command("the task has no execution root".to_string()))?;
            review.extend(human_gate::review_plan(
                &self.work_config,
                root,
                &path,
                &plan_id,
                Some(&task),
            )?);
        }
        if let Some(entry) = self
            .dispatcher
            .as_ref()
            .and_then(|dispatcher| dispatcher.ledger.entries.get(&item.id))
        {
            let recorded = crate::work::recorded_plans(entry);
            if !recorded.unread.is_empty() {
                return Err(EphorError::Command(format!(
                    "{} recorded plan(s) could not be read",
                    recorded.unread.len()
                )));
            }
            for plan in recorded.plans {
                review.extend(human_gate::review_plan(
                    &self.work_config,
                    &plan.root,
                    &plan.path,
                    &plan.plan_id,
                    None,
                )?);
            }
        }
        // History after every plan's gates (§FS-005-dispatch.9.2).
        Ok(review.settled())
    }

    pub fn transition_work(
        &self,
        item: &Item,
        ticket: &str,
        from: &str,
        to: &str,
        result: &str,
    ) -> Result<()> {
        if result.trim().is_empty() {
            return Err(EphorError::Command(
                "record a reason for this decision".to_string(),
            ));
        }
        let review = self.gates_of(item)?;
        let gate = one_gate(&review, ticket, from)?;
        // A typed gate is answered in its form, not moved by a reason alone:
        // the runtime would refuse, so the sentence names the way that works
        // (§FS-005-dispatch.9.3). Only the gate being moved is briefed.
        let typed = gate::briefs(&self.work_config)
            && gate::show(&self.work_config, gate).is_ok_and(|brief| brief.asks_its_own());
        if typed {
            return Err(EphorError::Command(format!(
                "'{ticket}' asks questions of its own at '{from}'; answer them in its form, \
                 or with `ephor work gate answer --item {} --ticket {ticket} --from {from} \
                 --answers <file|->`",
                item.id
            )));
        }
        human_gate::transition(&self.work_config, gate, to, result)
    }
}

/// The one gate a move names, still in the state the reader saw
/// (§FS-005-dispatch.9.1).
fn one_gate<'a>(review: &'a Review, ticket: &str, from: &str) -> Result<&'a Gate> {
    let matches: Vec<_> = review
        .gates
        .iter()
        .filter(|gate| gate.ticket == ticket)
        .collect();
    let [gate] = matches.as_slice() else {
        return Err(EphorError::Command(format!(
            "'{ticket}' is not one unambiguous human gate; reopen the review"
        )));
    };
    if gate.state != from {
        return Err(EphorError::Command(format!(
            "'{ticket}' changed from '{from}' to '{}'; reopen the review",
            gate.state
        )));
    }
    Ok(gate)
}
