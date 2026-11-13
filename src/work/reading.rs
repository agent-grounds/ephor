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
/// so their manual action remains beside the ticket it moves. After them, an
/// unfinished task the live run holds leads, wherever it sits in the plan: a
/// supervising parent need not be a prerequisite of its later steps to be what
/// the run is doing (§FS-005-dispatch.15.2). When several are held, the last
/// in plan order is named, the same reverse-plan order the rest of this
/// reading keeps; a finished task never leads, whatever its stale slot says.
/// Otherwise keep the existing last-open order unless its unfinished
/// prerequisites establish polling work that it cannot displace. A claim
/// proves neither.
pub(super) fn current(tickets: &[TicketStatus]) -> Option<usize> {
    gate(tickets)
        .or_else(|| {
            tickets
                .iter()
                .position(|ticket| ticket.person_wait().is_some())
        })
        .or_else(|| {
            tickets
                .iter()
                .rposition(|ticket| !ticket.finished && ticket.running)
        })
        .or_else(|| {
            let last = tickets.iter().rposition(|ticket| !ticket.finished)?;
            polling_prior(tickets, last).or(Some(last))
        })
}

/// Follow only unfinished prerequisite chains (§FS-005-dispatch.9), stopping
/// at finished tasks, for the polling state nearest the end of the plan; held
/// work was already read from the live slot (§FS-005-dispatch.15.2). Cycles in
/// an unreadable readiness graph cannot make this reading loop.
fn polling_prior(tickets: &[TicketStatus], at: usize) -> Option<usize> {
    let mut pending = tickets[at].reading.prior.clone();
    let mut seen = BTreeSet::from([at]);
    let mut polling = None;
    while let Some(prior) = pending.pop() {
        if !seen.insert(prior) {
            continue;
        }
        let ticket = &tickets[prior];
        if ticket.finished {
            continue;
        }
        if ticket.reading.polling {
            polling = polling.max(Some(prior));
        }
        pending.extend(&ticket.reading.prior);
    }
    polling
}
