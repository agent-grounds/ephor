//! The same local gate review and owner decision on both surfaces (§FS-005-dispatch.9.1).

use crate::error::{EphorError, Result};
use crate::feed::model::Item;
use crate::work::runtime::human_gate::{self, Review};

use super::Session;

impl Session {
    pub fn human_review(&self, item: &Item) -> Result<Review> {
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
        let review = self.human_review(item)?;
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
        human_gate::transition(&self.work_config, gate, to, result)
    }
}
