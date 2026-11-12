//! The current task shared by the badge and one-task location
//! (§FS-005-dispatch.9). These are display facts, never scheduling decisions.

use std::collections::BTreeSet;

use super::TicketStatus;

/// Runtime facts retained for the work reading (§FS-005-dispatch.9).
/// Prerequisites are indices into the same status snapshot, resolved within
/// each plan so two plans with the same local task id cannot order each other.
#[derive(Debug, Clone, Default)]
pub(crate) struct TicketReading {
    pub waiting_on: Option<String>,
    pub polling: bool,
    pub prior: Vec<usize>,
}

/// The real gate retains the manual action (§FS-005-dispatch.9), even when
/// a poll elsewhere in the plan also names a person.
pub(super) fn gate(tickets: &[TicketStatus]) -> Option<usize> {
    tickets
        .iter()
        .position(|ticket| !ticket.finished && ticket.waiting)
}

/// Person waits lead the reading (§FS-005-dispatch.9). Keep real gates first
/// so their manual action remains beside the ticket it moves. Otherwise keep
/// the existing last-open order unless its unfinished prerequisites establish
/// running or polling work that it cannot displace. A claim proves neither.
pub(super) fn current(tickets: &[TicketStatus]) -> Option<usize> {
    gate(tickets)
        .or_else(|| {
            tickets
                .iter()
                .position(|ticket| ticket.person_wait().is_some())
        })
        .or_else(|| {
            let last = tickets.iter().rposition(|ticket| !ticket.finished)?;
            if tickets[last].running {
                return Some(last);
            }
            active_prior(tickets, last).or(Some(last))
        })
}

/// Follow only unfinished prerequisite chains (§FS-005-dispatch.9), stopping
/// at finished tasks. A live slot is stronger evidence than a polling state;
/// equally evidenced prerequisites retain the existing reverse-plan order.
/// Cycles in an unreadable readiness graph cannot make this reading loop.
fn active_prior(tickets: &[TicketStatus], at: usize) -> Option<usize> {
    let mut pending = tickets[at].reading.prior.clone();
    let mut seen = BTreeSet::from([at]);
    let mut active = Vec::new();
    while let Some(prior) = pending.pop() {
        if !seen.insert(prior) {
            continue;
        }
        let ticket = &tickets[prior];
        if ticket.finished {
            continue;
        }
        if ticket.running || ticket.reading.polling {
            active.push(prior);
        }
        pending.extend(&ticket.reading.prior);
    }
    active
        .into_iter()
        .max_by_key(|&prior| (tickets[prior].running, prior))
}
