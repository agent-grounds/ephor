//! Dispatch: what ephor watches, it can hand to an agent runtime
//! (§FS-005-dispatch).
//!
//! The feed says what is happening; this says what is being done about it.
//! An item plus a recipe becomes a ticket in a plan, written into the
//! checkout the item's branch resolves to, carrying the dossier of everything
//! ephor already knew. Afterwards ephor keeps the ledger and reads the work's
//! state back out of the plan — never out of its own memory.

pub mod commands;
pub mod dossier;
pub mod headroom;
pub mod hold;
pub mod ledger;
pub mod private;
mod ranking;
pub mod recipe;
pub mod runtime;
pub mod spend;
pub mod sweeps;
pub mod workflow;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::branches::{Placement, WorkspaceState};
use crate::capabilities::{CapabilitySet, Rung};
use crate::error::{EphorError, Result};
use crate::feed::config::{ActionConfig, StatusConfig};
use crate::feed::model::Item;
use crate::paths::for_shell;

use dossier::Subject;
use hold::{Flight, Hold};
use ledger::{Dispatch, Entry, Judged, Ledger, Snapshot};
use recipe::{HandList, OrganizationWorkConfig, ProjectWorkConfig, Recipe, WorkConfig};
use runtime::plan::{self, Plan, Ticket, WorkRoot};

/// What one dispatch did.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// A plan was created and the first ticket written into it.
    Opened {
        plan: PathBuf,
        ticket: String,
        recipe: String,
    },
    /// The item had moved, so a ticket was appended after the last one
    /// (§FS-005-dispatch.5).
    Reopened {
        plan: PathBuf,
        ticket: String,
        recipe: String,
        changes: Vec<String>,
    },
    /// The work still answers the item as it is.
    Current,
    /// A recipe's deterministic opening move finished, so there was nothing
    /// left to hand over (§FS-005-dispatch.12): a clean rebase is a done
    /// thing, not a ticket.
    Settled { move_name: String },
    /// The item moved, but nothing applies to it any more — it was merged,
    /// closed, or answered. The work is over; the ledger keeps saying so.
    Dormant { changes: Vec<String> },
    /// A workflow the runtime offers laid down a plan of its own beside the
    /// item's (§FS-005-dispatch.19).
    Laid {
        plan: PathBuf,
        plan_id: String,
        workflow: String,
        entry: String,
    },
    /// A sweep reached work about a matter a source the site lists as
    /// private reported, and wrote nothing (§FS-018-private-sources.3).
    /// `hold` is always [`Hold::Private`]. Only a sweep is told this: a
    /// person's named move writes as it always did.
    PassedOver { recipe: String, hold: Hold },
}

impl Outcome {
    /// What was written, without saying whether it happened: the caller knows
    /// whether this was a dry run and a second "opened" from here would
    /// contradict it.
    pub fn describe(&self) -> String {
        match self {
            Outcome::Opened {
                plan,
                ticket,
                recipe,
            } => format!("{recipe} → {}#{ticket}", plan.display()),
            // A ticket appended to a plan that exists is "reopened" whether or
            // not the item moved — asking for something else is one way to
            // reopen work. With nothing to say about the item, say nothing.
            Outcome::Reopened {
                plan,
                ticket,
                recipe,
                changes,
            } if changes.is_empty() => format!("{recipe} → {}#{ticket}", plan.display()),
            Outcome::Reopened {
                plan,
                ticket,
                recipe,
                changes,
            } => format!(
                "{recipe} → {}#{ticket} ({})",
                plan.display(),
                changes.join("; ")
            ),
            Outcome::Current => "already current".to_string(),
            Outcome::Settled { move_name } => {
                format!("{move_name} finished — nothing to hand over")
            }
            Outcome::Dormant { changes } => {
                format!("{} — no recipe applies to it now", changes.join("; "))
            }
            Outcome::Laid {
                plan,
                workflow,
                entry,
                ..
            } => format!("{entry} ({workflow}) → {}", plan.display()),
            Outcome::PassedOver { hold, .. } => hold.says(),
        }
    }
}

/// One ticket of an item's work, as the plan currently has it.
#[derive(Debug, Clone)]
pub struct TicketStatus {
    pub id: String,
    pub recipe: String,
    pub title: String,
    pub state: Option<String>,
    pub finished: bool,
    /// Taken back rather than finished: the ticket sits in the machine's
    /// abandonment state (§FS-005-dispatch.16). Finished too, since that
    /// state is final — this says which kind of over it is.
    pub cancelled: bool,
    /// The runtime has stopped on this ticket and a person has to answer it
    /// (§FS-005-dispatch.9).
    pub waiting: bool,
    /// Who claimed the ticket, where anyone has — a claimed ticket is not a
    /// run's to advance (§FS-005-dispatch.15).
    pub assignee: Option<String>,
    /// The execution line the ticket carries, where it carries one
    /// (§FS-005-dispatch.14).
    pub pinned: Option<plan::Pin>,
    /// What the review left behind, where the work reached one — or, for a
    /// ticket taken back, the reason the reader gave (§FS-005-dispatch.16).
    pub verdict: Option<String>,
    /// When ephor asked for it, from the ledger's record of the dispatch
    /// (§FS-005-dispatch.18). None for a ticket ephor did not dispatch — one
    /// written into the plan by hand, or by the machine for itself: nothing
    /// knows when that was asked for, so nothing claims to.
    pub asked: Option<DateTime<Utc>>,
    /// A live run has this ticket in hand right now, read from the run's own
    /// record of itself (§FS-005-dispatch.15.2, §FS-005-dispatch.23). Open and
    /// being worked on are different facts and the row says which.
    pub running: bool,
    /// A run is live on this ticket's root and busy elsewhere: it will get its
    /// turn without anyone doing anything (§FS-005-dispatch.15).
    pub queued: bool,
}

/// What one cancel did (§FS-005-dispatch.16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cancelled {
    pub ticket: String,
    /// The state it was taken back from.
    pub from: String,
    pub plan: PathBuf,
    /// Open tickets ordered after it, which will not start while it stands
    /// cancelled — named, never moved: cancelling them too is the reader's
    /// call.
    pub left_waiting: Vec<String>,
}

impl Cancelled {
    /// One line for a message: what was cancelled, and what that leaves
    /// waiting.
    pub fn describe(&self) -> String {
        match self.left_waiting.is_empty() {
            true => format!("⊘ {} cancelled", self.ticket),
            false => format!(
                "⊘ {} cancelled — {} ordered after it and will not start while it stands cancelled",
                self.ticket,
                match self.left_waiting.len() {
                    1 => format!("{} is", self.left_waiting[0]),
                    _ => format!("{} are", self.left_waiting.join(", ")),
                }
            ),
        }
    }
}

/// What a cancel says when the reader says nothing: the runtime records why
/// a ticket ended where it did and refuses a terminal move that says
/// nothing, and this is the truth of a reason left blank — never a reason
/// invented on the reader's behalf.
pub const CANCELLED_UNSAID: &str = "Cancelled from ephor; no reason was given.";

/// An item's work as it stands: read from the plan every time
/// (§FS-005-dispatch.4).
#[derive(Debug, Clone)]
pub struct WorkStatus {
    pub project: String,
    pub root: PathBuf,
    /// The plan's id inside the root, for naming it to the runtime.
    pub plan_id: String,
    /// The checkout the runtime is run from.
    pub checkout: PathBuf,
    pub plan: PathBuf,
    /// Every committed plan placement for this matter — the ones ephor wrote
    /// and the ones its workflows laid beside them (§FS-005-dispatch.35). The
    /// singular fields above remain the latest recipe placement for
    /// compatibility (§FS-005-dispatch.4, §REQ-002-parity.4).
    pub plans: Vec<RecordedPlan>,
    /// A plan the ledger points at is gone — reported, never repaired. True
    /// for a recipe plan ephor wrote and lost, and for a laid plan the record
    /// names that nobody can read (§FS-005-dispatch.35).
    pub missing: bool,
    /// How many laid plans the record named could not be read
    /// (§FS-005-dispatch.35). A count of open tickets cannot tell an entry
    /// whose work is finished from one whose plan is unreadable — both are
    /// zero — so what may be forgotten is read from this beside it.
    pub unread_workflows: usize,
    pub tickets: Vec<TicketStatus>,
    /// How many plans a workflow laid down beside this matter's own
    /// (§FS-005-dispatch.19). A count rather than the plans themselves: what
    /// each one is doing is the operations board's answer, read from the plan
    /// files there like every other operation (§FS-005-dispatch.15).
    pub workflows: usize,
    /// What has happened to the item since the last dispatch.
    pub changes: Vec<String>,
    /// How to move the waiting ticket on by hand, where one is waiting. Built
    /// by the runtime module, since the words are the runner's
    /// (§REQ-001-boundary.5).
    pub advance: Option<String>,
    /// Minutes of silence worth noting on the live run holding this work — the
    /// badge the board carries, on the row the reader is already looking at
    /// (§FS-005-dispatch.23). None where no run is live here, and on one
    /// writing normally.
    pub quiet: Option<u64>,
    /// The one task a report with room for one line should name, and the plan
    /// it is in (§FS-005-dispatch.35). None where nothing is open, which is
    /// what tells a matter that is over from a matter no recipe applies to.
    pub open_at: Option<OpenWork>,
}

/// Where a matter's work still stands, for a sentence that has room for one
/// (§FS-005-dispatch.35): the plan holding the task, and what that task is
/// at. The plan is named because it is the thing to go and open — a matter
/// whose work is a laid workflow plan has no other file that says anything.
#[derive(Debug, Clone)]
pub struct OpenWork {
    pub plan: PathBuf,
    pub ticket: String,
    pub state: String,
}

/// One committed plan placement from the ledger's normalized reading
/// (§FS-005-dispatch.15.1).
#[derive(Debug, Clone)]
pub struct RecordedPlan {
    pub root: PathBuf,
    pub checkout: PathBuf,
    pub branch: Option<String>,
    pub plan_id: String,
    pub path: PathBuf,
    /// The name the record gave a plan a workflow laid down, where this is
    /// one (§FS-005-dispatch.35). `None` is a plan ephor wrote itself — the
    /// one difference the two are told apart by, since a plan ephor promised
    /// and a plan it only asked a runtime for are not gone in the same sense.
    pub laid: Option<String>,
}

/// Every plan the record says is one matter's work, resolved against the disk
/// (§FS-005-dispatch.35). A matter's work is the plan ephor wrote itself and
/// every one a workflow laid beside it, and no surface gets to read a
/// narrower list than another (§FS-005-dispatch.30).
#[derive(Debug, Clone, Default)]
pub struct RecordedWork {
    /// The plans that could be read, in the record's own order: the recipe
    /// placements first, then what the workflows laid.
    pub plans: Vec<RecordedPlan>,
    /// How many laid plans the record named that nobody could read. Counted
    /// rather than listed because the only question asked of them is whether
    /// there are any: an entry with one of these is not an entry whose work
    /// is over, it is an entry whose work cannot be found.
    pub unread: usize,
}

impl WorkStatus {
    pub fn stale(&self) -> bool {
        !self.changes.is_empty()
    }

    pub fn open_tickets(&self) -> usize {
        self.tickets.iter().filter(|t| !t.finished).count()
    }

    /// Work that has stopped and is waiting on a person. The one thing in here
    /// that is nobody else's to move (§FS-005-dispatch.9).
    pub fn waiting(&self) -> Option<&TicketStatus> {
        self.tickets.iter().find(|ticket| ticket.waiting)
    }

    /// Whether there is nothing here to read at all: a plan the record named
    /// could not be read, and no other plan of this matter's answered
    /// (§FS-005-dispatch.35).
    ///
    /// Narrower than [`WorkStatus::missing`], which says only that *some*
    /// plan the record named could not be read. Since a matter's work is
    /// every plan the record says is its own, the two came apart: one laid
    /// plan gone and another going is a matter that is missing a plan and
    /// still has work to show. A caller asking "is there anything to say
    /// about this matter" asks this one; a caller reporting on the record
    /// asks `missing`.
    pub fn unreadable(&self) -> bool {
        self.missing && self.tickets.is_empty()
    }

    /// A row that has something to say and a plan that could not be read says
    /// both (§FS-005-dispatch.30): the second is a fact about the record and
    /// never a reason to withhold the first, so the row and the sentence
    /// `work sync` writes about one matter cannot name different facts.
    fn warned(&self, said: String) -> String {
        match self.missing {
            true => format!("{said}  ⚠ a plan is missing"),
            false => said,
        }
    }

    /// One line for a row that has room for one: what the work is doing, or
    /// what it decided, and whether the item has moved under it. `verdict` is
    /// how much of the verdict's own sentence fits where this is going.
    pub fn badge(&self, verdict_width: usize) -> String {
        // Nothing here to read — not merely a plan the record named that
        // nobody could (§FS-005-dispatch.35). Where another plan of this
        // matter's did answer, what it says leads and the unreadable one is
        // said beside it.
        if self.unreadable() {
            return "⚠ plan missing".to_string();
        }
        // Work that is entirely workflows has no ticket to badge; what it has
        // is said on the rows beneath (§FS-005-dispatch.19).
        if self.tickets.is_empty() && self.workflows > 0 {
            return match self.workflows {
                1 => "⛬ 1 workflow".to_string(),
                many => format!("⛬ {many} workflows"),
            };
        }
        // A question for a person leads: everything else in the badge is the
        // runtime telling you what it is doing, and this is it telling you it
        // has stopped.
        if let Some(waiting) = self.waiting() {
            // A ticket the machine opened for itself has no recipe — its
            // "recipe" falls back to its own id, and saying that twice is
            // noise where the point is the question.
            return self.warned(match waiting.recipe == waiting.id {
                true => format!("⚠ waiting on you · {}", waiting.id),
                false => format!("⚠ {} · waiting on you · {}", waiting.recipe, waiting.id),
            });
        }
        let mut badge = match self.tickets.iter().rev().find(|ticket| !ticket.finished) {
            Some(open) => format!(
                "⚙ {} · {}",
                open.recipe,
                open.state.as_deref().unwrap_or("?")
            ),
            None => match self.tickets.last() {
                // Taken back is a different kind of over from finished, and
                // the row says which (§FS-005-dispatch.16).
                Some(last) if last.cancelled => format!("⊘ {} · cancelled", last.recipe),
                Some(last) => match &last.verdict {
                    // The verdict's own sentence, cut where a row ends: the
                    // rest of it is in the artifact, one keystroke away.
                    Some(verdict) => {
                        format!("✓ {} · {}", last.recipe, clamp(verdict, verdict_width))
                    }
                    None => format!("✓ {}", last.recipe),
                },
                None => "· no tickets".to_string(),
            },
        };
        badge = self.warned(badge);
        if self.stale() {
            badge.push_str(&format!("  ⟳ {}", self.changes.join("; ")));
        }
        badge
    }

    /// The rows this work stands on beneath the matter it is about
    /// (§FS-005-dispatch.23): one per open ticket, the parked one first
    /// (§FS-005-dispatch.9), and — where nothing is open — one for what the
    /// last ticket decided. `verdict` is how much of a verdict's own sentence
    /// fits on a row.
    pub fn lines(&self, verdict_width: usize) -> Vec<WorkLine> {
        // As the badge reads it: nothing to read is one fact, a plan the
        // record named that nobody could read is another, and a matter with
        // both a going plan and an unreadable one gets a row for each
        // (§FS-005-dispatch.35).
        if self.unreadable() {
            return vec![WorkLine::said(Tone::Waiting, "⚠", "plan missing")];
        }
        // Work that is entirely workflows has no ticket of its own; what it
        // has is said on the rows beneath (§FS-005-dispatch.19).
        if self.tickets.is_empty() && self.workflows > 0 {
            let said = match self.workflows {
                1 => "1 workflow".to_string(),
                many => format!("{many} workflows"),
            };
            return vec![WorkLine::said(Tone::Going, "⛬", said)];
        }
        let mut lines: Vec<WorkLine> = Vec::new();
        let open = self.tickets.iter().filter(|ticket| !ticket.finished);
        for ticket in open.clone().filter(|ticket| ticket.waiting) {
            // A ticket the machine opened for itself has no recipe — its
            // "recipe" falls back to its own id, and saying that twice is
            // noise where the point is the question.
            let said = match ticket.recipe == ticket.id {
                true => format!("waiting on you · {}", ticket.id),
                false => format!("{} · waiting on you · {}", ticket.recipe, ticket.id),
            };
            lines.push(WorkLine::of(Tone::Waiting, "⚠", said, ticket));
        }
        for ticket in open.filter(|ticket| !ticket.waiting) {
            let mut said = format!(
                "{} · {}",
                ticket.recipe,
                ticket.state.as_deref().unwrap_or("?")
            );
            // Open and being worked on right now are different facts, and the
            // row says which (§FS-005-dispatch.23) — in the board's own words,
            // because this is that reading narrowed to one matter
            // (§FS-005-dispatch.15).
            let (tone, marker) = match (ticket.running, ticket.queued) {
                (true, _) => {
                    // A live run that has gone silent wears the badge it wears
                    // on the board: a long tool call is legitimately quiet, so
                    // it is a badge and never a verdict (§FS-005-dispatch.15).
                    if let Some(minutes) = self.quiet {
                        said = format!("{said} · quiet {minutes}m");
                    }
                    (Tone::Running, "▶")
                }
                (false, true) => {
                    said = format!("{said} · queued");
                    (Tone::Going, "⚙")
                }
                (false, false) => (Tone::Going, "⚙"),
            };
            lines.push(WorkLine::of(tone, marker, said, ticket));
        }
        // Nothing open: what the last one decided, on one line. The rest of
        // the record is the work screen's (§FS-005-dispatch.18).
        if lines.is_empty() {
            lines.push(match self.tickets.last() {
                // Taken back is a different kind of over from finished, and
                // the row says which (§FS-005-dispatch.16).
                Some(last) if last.cancelled => WorkLine::of(
                    Tone::Over,
                    "⊘",
                    format!("{} · cancelled", last.recipe),
                    last,
                ),
                Some(last) => {
                    // The verdict's own sentence, cut where a row ends: the
                    // rest of it is in the artifact, one keystroke away.
                    let said = match &last.verdict {
                        Some(verdict) => {
                            format!("{} · {}", last.recipe, clamp(verdict, verdict_width))
                        }
                        None => last.recipe.clone(),
                    };
                    WorkLine::of(Tone::Over, "✓", said, last)
                }
                None => WorkLine::said(Tone::Over, "·", "no tickets"),
            });
        }
        if self.missing {
            lines.push(WorkLine::said(Tone::Waiting, "⚠", "a plan is missing"));
        }
        if self.stale() {
            lines.push(WorkLine::said(
                Tone::Stale,
                "⟳",
                format!("since that was asked: {}", self.changes.join("; ")),
            ));
        }
        lines
    }
}

/// How a work line reads at a glance (§FS-005-dispatch.23). The tones the work
/// screen already spells its tickets in, so the tree and the screen behind `w`
/// cannot say the same ticket two different ways.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A live run has it in hand right now (§FS-005-dispatch.23).
    Running,
    /// Open, and nothing is working it this moment.
    Going,
    /// It has stopped and a person has to answer it (§FS-005-dispatch.9).
    Waiting,
    /// Finished, or taken back (§FS-005-dispatch.16).
    Over,
    /// The item has moved under the work (§FS-005-dispatch.5).
    Stale,
}

/// One row of a matter's work, beneath the row the matter is on
/// (§FS-005-dispatch.23).
#[derive(Debug, Clone)]
pub struct WorkLine {
    pub tone: Tone,
    pub marker: &'static str,
    pub said: String,
    /// The ticket this row *is*, where it is one — what cancelling here takes
    /// back (§FS-005-dispatch.16). None on a row that is a summary rather than
    /// a ticket, and on one whose ticket is already over.
    pub ticket: Option<String>,
    /// When ephor asked for it, where the ledger knows (§FS-005-dispatch.18).
    pub asked: Option<DateTime<Utc>>,
}

impl WorkLine {
    fn of(tone: Tone, marker: &'static str, said: String, ticket: &TicketStatus) -> WorkLine {
        WorkLine {
            tone,
            marker,
            said,
            ticket: (!ticket.finished).then(|| ticket.id.clone()),
            asked: ticket.asked,
        }
    }

    fn said(tone: Tone, marker: &'static str, said: impl Into<String>) -> WorkLine {
        WorkLine {
            tone,
            marker,
            said: said.into(),
            ticket: None,
            asked: None,
        }
    }
}

/// What the work behind one menu entry is doing, for the row that could start
/// it again (§FS-005-dispatch.21). Three answers, because the runtime schedules
/// one run per execution root: the run holds this entry's work, it parked a
/// question this entry opened, or it is live on the root and will reach it
/// (§FS-005-dispatch.15).
#[derive(Debug, Clone)]
pub enum WorkGoing {
    Running {
        root: PathBuf,
        /// The run holding this particular recorded root, where one names
        /// itself (§FS-005-dispatch.20, §FS-005-dispatch.30).
        identity: Option<runtime::watch::RunIdentity>,
        /// The ticket the run holds and the state it is in, in the words the
        /// board already uses.
        doing: String,
    },
    /// A ticket this entry opened that the machine parks for a person: it is
    /// *waiting on you* (§FS-005-dispatch.9, §FS-005-dispatch.20), and §21's
    /// word for it is §15's, never *queued*, which would promise a turn that
    /// never comes.
    ///
    /// Marked whether or not a run still holds the root. A run with nobody at
    /// its terminal waits at a human gate rather than exiting, and one that
    /// exited leaves the question standing all the same — either way this is
    /// open work about this subject, and a second dispatch laid beside it is
    /// exactly the mistake §21 exists to prevent.
    Waiting {
        root: PathBuf,
        identity: Option<runtime::watch::RunIdentity>,
        /// The ticket the question is in, and the state the machine parked it
        /// in — the plan is where the answer belongs (§FS-005-dispatch.9).
        ticket: String,
        state: String,
        plan: PathBuf,
    },
    Queued {
        root: PathBuf,
        identity: Option<runtime::watch::RunIdentity>,
    },
}

impl WorkGoing {
    pub fn root(&self) -> &std::path::Path {
        match self {
            WorkGoing::Running { root, .. }
            | WorkGoing::Waiting { root, .. }
            | WorkGoing::Queued { root, .. } => root,
        }
    }

    pub fn identity(&self) -> Option<&runtime::watch::RunIdentity> {
        match self {
            WorkGoing::Running { identity, .. }
            | WorkGoing::Waiting { identity, .. }
            | WorkGoing::Queued { identity, .. } => identity.as_ref(),
        }
    }
}

/// One item's work root, read once and asked about many times
/// (§FS-005-dispatch.15.1).
///
/// The lock probe, the states document, the plan and the journal answer the
/// same way for every row of one menu, so they are read here and handed to
/// each — the rule that an answer is resolved once and reused
/// (§AR-005-capabilities.1). Nothing is remembered past the menu: this is a
/// reading of the world, taken when the menu was assembled.
pub struct WorkAt<'a> {
    entry: &'a Entry,
    roots: Vec<WorkAtRoot>,
}

/// One of an item's committed work roots, read once for every menu entry
/// (§FS-005-dispatch.15.1).
struct WorkAtRoot {
    root: PathBuf,
    /// The run lock is held: something is running on this root right now.
    live: bool,
    /// The root's own state machine, where it has a readable one. Finality and
    /// gating are its words: with none to say them nothing here is judged over,
    /// and nothing is judged a question for a person either
    /// (§FS-005-dispatch.15).
    machine: Option<WorkRoot>,
    /// The matter's own plan.
    plan: Option<(PathBuf, Plan)>,
    /// What answers for which tickets the run has in hand — the run's own
    /// stream where the binding writes one, the journal otherwise
    /// (§FS-005-dispatch.15.2) — and when the root's lock was born, which
    /// only the journal's reading needs. Read once for every ticket asked
    /// about (§FS-005-dispatch.15).
    witness: Option<runtime::watch::Witness>,
    lock_born: Option<std::time::SystemTime>,
    /// What the live run calls itself, from the descriptor beside its lock
    /// (§FS-005-dispatch.20).
    identity: Option<runtime::watch::RunIdentity>,
}

impl WorkAt<'_> {
    pub fn root(&self) -> &std::path::Path {
        &self.entry.root
    }

    pub fn live(&self) -> bool {
        self.roots.iter().any(|root| root.live)
    }

    /// What the work one entry hands over is doing right now
    /// (§FS-005-dispatch.21), off the reading already taken.
    ///
    /// Three answers, ranked as the board ranks them: what waits on the reader
    /// stands ahead of anything else its work is doing (§FS-005-dispatch.9),
    /// then the ticket a run holds, then the queue the root's run will reach.
    /// This is the board's reading narrowed to one row, not a second reading.
    pub fn going(&self, action: &str) -> Option<WorkGoing> {
        let mut waiting: Option<WorkGoing> = None;
        let mut running: Option<WorkGoing> = None;
        let mut queued: Option<WorkGoing> = None;
        for at in &self.roots {
            // `judge` is the machine that answers for the plan the ticket is
            // in, never assumed to be the root's (§FS-005-dispatch.28).
            let mut consider = |judge: Option<&WorkRoot>,
                                plan_id: &str,
                                path: &std::path::Path,
                                ticket: &plan::PlanTicket| {
                let Some(machine) = judge else {
                    return hold_at(at, plan_id, ticket, &mut running, &mut queued);
                };
                let state = ticket.state.as_deref().unwrap_or("?");
                if ticket
                    .state
                    .as_deref()
                    .is_some_and(|state| machine.is_final(state))
                {
                    return;
                }
                if ticket
                    .state
                    .as_deref()
                    .is_some_and(|state| machine.is_gating(state))
                {
                    if waiting.is_none() {
                        waiting = Some(WorkGoing::Waiting {
                            root: at.root.clone(),
                            identity: at.identity.clone(),
                            ticket: format!("{plan_id}.{}", ticket.id),
                            state: state.to_string(),
                            plan: path.to_path_buf(),
                        });
                    }
                    return;
                }
                hold_at(at, plan_id, ticket, &mut running, &mut queued);
            };

            let mine: BTreeSet<&str> = self
                .entry
                .dispatches
                .iter()
                .filter(|dispatch| {
                    dispatch.recipe == action
                        && !dispatch.is_workflow()
                        && canonical(dispatch.root.as_ref().unwrap_or(&self.entry.root))
                            == canonical(&at.root)
                })
                .map(|dispatch| dispatch.ticket.as_str())
                .collect();
            if let Some((path, plan)) = at.plan.as_ref() {
                for ticket in plan.tickets() {
                    if mine.contains(ticket.id.as_str()) {
                        consider(at.machine.as_ref(), &self.entry.plan_id, path, &ticket);
                    }
                }
            }

            for dispatch in self.entry.dispatches.iter().filter(|dispatch| {
                dispatch.recipe == action
                    && dispatch.is_workflow()
                    && canonical(dispatch.root.as_ref().unwrap_or(&self.entry.root))
                        == canonical(&at.root)
            }) {
                let Some(name) = dispatch.plan.as_deref() else {
                    continue;
                };
                let Some(laid) = runtime::workflow::laid(&at.root.join(name)) else {
                    continue;
                };
                let Ok(Some(plan)) = Plan::read(&laid.path) else {
                    continue;
                };
                let own = plan::own_machine(&laid.path);
                let judge = match &own {
                    Ok(Some(store)) => Some(store),
                    Ok(None) => at.machine.as_ref(),
                    Err(_) => None,
                };
                for ticket in plan.tickets() {
                    consider(judge, &laid.plan_id, &laid.path, &ticket);
                }
            }
        }
        waiting.or(running).or(queued)
    }
}

fn hold_at(
    at: &WorkAtRoot,
    plan_id: &str,
    ticket: &plan::PlanTicket,
    running: &mut Option<WorkGoing>,
    queued: &mut Option<WorkGoing>,
) {
    if !at.live {
        return;
    }
    let state = ticket.state.as_deref().unwrap_or("?");
    if at.witness.as_ref().is_some_and(|witness| {
        witness.holds(&at.root, at.lock_born, plan_id, &ticket.id, Some(state))
    }) {
        if running.is_none() {
            *running = Some(WorkGoing::Running {
                root: at.root.clone(),
                identity: at.identity.clone(),
                doing: format!("{plan_id}.{} [{state}]", ticket.id),
            });
        }
        return;
    }
    if queued.is_none() {
        *queued = Some(WorkGoing::Queued {
            root: at.root.clone(),
            identity: at.identity.clone(),
        });
    }
}

/// How the ticket says who this went to (§FS-005-dispatch.29). Ephor's own
/// words, and named as ephor's, so a reader can tell what the runtime was
/// asked from what ephor decided before asking it.
fn chose(said: &str) -> String {
    format!("**Who this went to.** {said}.")
}

/// What a dispatch pins on its ticket, and what it records there about the
/// choosing (§FS-005-dispatch.14, §FS-005-dispatch.29).
#[derive(Debug, Clone, Default)]
pub struct Pinned {
    /// The runtime's own execution line, where the choice has one.
    pub target: Option<String>,
    /// A model with no carrier of its own, for the same reason.
    pub model: Option<String>,
    /// What choosing among the pin's hands had to say, in ephor's own words.
    /// Written into the ticket's body beside the brief rather than as a field
    /// of the runtime's plan language, which is the runtime's
    /// (§REQ-001-boundary.1). None where there was nothing to choose among.
    pub said: Option<String>,
    /// The pool the chosen hand's work is bought against, remembered on the
    /// entry so a start that fails here can be recorded against what refused
    /// it (§FS-005-dispatch.29).
    pub pool: Option<String>,
}

/// The first pre-image of one path touched by an unsaved hand-off
/// (§FS-005-dispatch.4). Directory images are used only for workflow output
/// and carried-file trees; an existing work root journals its shared files
/// individually so unrelated runtime work is never rolled back.
enum PathImage {
    Missing,
    File(Vec<u8>),
    Directory(Vec<TreeImage>),
    #[cfg(unix)]
    Symlink {
        target: PathBuf,
        referent: Option<(PathBuf, Vec<u8>)>,
    },
}

enum TreeImage {
    Directory(PathBuf),
    File(PathBuf, Vec<u8>),
    #[cfg(unix)]
    Symlink(PathBuf, PathBuf),
}

impl PathImage {
    fn capture(path: &std::path::Path) -> std::io::Result<PathImage> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(PathImage::Missing);
            }
            Err(err) => return Err(err),
        };
        if metadata.file_type().is_symlink() {
            #[cfg(unix)]
            {
                let target = std::fs::read_link(path)?;
                // Keep the exact link text, but separately retain the bytes a
                // write through that link can change. Canonicalizing only the
                // referent avoids treating the link itself as a directory and
                // also follows a relative or chained destination to the file
                // the hand-off would actually touch (§FS-005-dispatch.4).
                let referent = match std::fs::canonicalize(path) {
                    Ok(referent) if std::fs::metadata(&referent)?.is_file() => {
                        Some((referent.clone(), std::fs::read(referent)?))
                    }
                    Ok(_) => None,
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                    Err(err) => return Err(err),
                };
                return Ok(PathImage::Symlink { target, referent });
            }
            #[cfg(not(unix))]
            return std::fs::read(path).map(PathImage::File);
        }
        if metadata.is_file() {
            return std::fs::read(path).map(PathImage::File);
        }
        let mut entries = Vec::new();
        capture_tree(path, path, &mut entries)?;
        Ok(PathImage::Directory(entries))
    }

    /// Whether the path already holds exactly this image, so putting it back
    /// is nothing to do (§FS-005-dispatch.4).
    ///
    /// A hand-off journals a path before it touches it and unwinds every path
    /// it journalled, so most of what a rollback puts back never moved. The
    /// restore below removes and rewrites, which fails where the directory is
    /// the very thing that went wrong — and then the reader is told the
    /// rollback failed about a file that is sitting exactly where it belongs.
    /// Only the two shapes a rename produces are recognised; a directory or a
    /// symlink is restored as before.
    fn unchanged_at(&self, path: &std::path::Path) -> bool {
        match self {
            PathImage::Missing => std::fs::symlink_metadata(path)
                .is_err_and(|err| err.kind() == std::io::ErrorKind::NotFound),
            PathImage::File(bytes) => {
                std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
                    && std::fs::read(path).is_ok_and(|have| have == *bytes)
            }
            _ => false,
        }
    }

    fn restore(&self, path: &std::path::Path) -> std::io::Result<()> {
        if self.unchanged_at(path) {
            return Ok(());
        }
        remove_path(path)?;
        match self {
            PathImage::Missing => Ok(()),
            PathImage::File(bytes) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, bytes)
            }
            #[cfg(unix)]
            PathImage::Symlink { target, referent } => {
                if let Some((referent, bytes)) = referent {
                    if let Some(parent) = referent.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(referent, bytes)?;
                }
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::os::unix::fs::symlink(target, path)
            }
            PathImage::Directory(entries) => {
                std::fs::create_dir_all(path)?;
                for entry in entries {
                    match entry {
                        TreeImage::Directory(relative) => {
                            std::fs::create_dir_all(path.join(relative))?;
                        }
                        TreeImage::File(relative, bytes) => {
                            let target = path.join(relative);
                            if let Some(parent) = target.parent() {
                                std::fs::create_dir_all(parent)?;
                            }
                            std::fs::write(target, bytes)?;
                        }
                        #[cfg(unix)]
                        TreeImage::Symlink(relative, target) => {
                            let link = path.join(relative);
                            if let Some(parent) = link.parent() {
                                std::fs::create_dir_all(parent)?;
                            }
                            std::os::unix::fs::symlink(target, link)?;
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

fn capture_tree(
    root: &std::path::Path,
    dir: &std::path::Path,
    entries: &mut Vec<TreeImage>,
) -> std::io::Result<()> {
    for found in std::fs::read_dir(dir)? {
        let path = found?.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            #[cfg(unix)]
            entries.push(TreeImage::Symlink(relative, std::fs::read_link(&path)?));
            #[cfg(not(unix))]
            entries.push(TreeImage::File(relative, std::fs::read(&path)?));
        } else if metadata.is_dir() {
            entries.push(TreeImage::Directory(relative));
            capture_tree(root, &path, entries)?;
        } else {
            entries.push(TreeImage::File(relative, std::fs::read(&path)?));
        }
    }
    Ok(())
}

fn remove_path(path: &std::path::Path) -> std::io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

#[derive(Default)]
struct Journal {
    ledger: Option<Ledger>,
    paths: Vec<(PathBuf, PathImage)>,
    seen: BTreeSet<PathBuf>,
    /// Hold the row through hand-off ledger commit/rollback (§FS-005-dispatch.4).
    reply_locks: BTreeMap<String, crate::replies::Store>,
}

impl Journal {
    fn begin(&mut self, ledger: &Ledger) {
        self.ledger.get_or_insert_with(|| ledger.clone());
    }

    fn remember(&mut self, path: &std::path::Path) -> Result<()> {
        let path = path.to_path_buf();
        if self.seen.contains(&path) {
            return Ok(());
        }
        let image = PathImage::capture(&path).map_err(|err| {
            EphorError::Command(format!("Cannot journal {}: {err}", path.display()))
        })?;
        self.seen.insert(path.clone());
        self.paths.push((path, image));
        Ok(())
    }

    fn rollback(mut self) -> (Ledger, Option<(PathBuf, std::io::Error)>) {
        let mut failed = None;
        for (path, image) in self.paths.drain(..).rev() {
            if let Err(err) = image.restore(&path) {
                if failed.is_none() {
                    failed = Some((path, err));
                }
            }
        }
        (self.ledger.take().unwrap_or_default(), failed)
    }
}

/// One ledger entry whose recorded plan name is not the stem its own id
/// renders now (§FS-005-dispatch.3.1).
struct Behind {
    id: String,
    was: String,
    now: String,
    root: PathBuf,
}

/// A carry-over that was not made: the matters it holds back, and what the
/// reader is told about them (§FS-005-dispatch.3).
struct Refused {
    matters: Vec<String>,
    says: String,
}

/// What one carry-over pass moved, and what stood in its way
/// (§FS-005-dispatch.3.1).
#[derive(Default)]
struct CarriedOver {
    moved: Vec<String>,
    refused: Vec<Refused>,
}

impl CarriedOver {
    /// Everything that stood in the way, said together — and what was carried
    /// over anyway, because a rename nobody was told about reads as work that
    /// vanished (§REQ-002-parity).
    fn stopped(&self) -> String {
        let mut says = self
            .refused
            .iter()
            .map(|one| one.says.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        if !self.moved.is_empty() {
            says.push_str(&format!(
                "\n\nEverything else was carried over and is committed: {}.",
                self.moved.join("; ")
            ));
        }
        says
    }
}

/// Reads the work configuration, offers recipes, writes tickets, and keeps the
/// ledger.
pub struct Dispatcher {
    /// The configured source identity used at hand-off, including in-process
    /// callers that never load a status file (§FS-005-dispatch.4).
    reply_config: StatusConfig,
    registry_doc: Value,
    global: WorkConfig,
    projects: BTreeMap<String, ProjectWorkConfig>,
    /// The ceiling each organization's projects share, by organization id
    /// (§FS-005-dispatch.24). Which projects those are is the registry's to
    /// say, so this half of the tier is only the numbers.
    organizations: BTreeMap<String, OrganizationWorkConfig>,
    placements: BTreeMap<String, Option<Placement>>,
    /// What the checkout of each (project, branch) says about itself —
    /// both distances, from one fold — measured on demand.
    behind: BTreeMap<(String, String), recipe::Facts>,
    /// Who can be asked, per work root — the roster is read from the runtime's
    /// merged settings, and a sweep asks about the same handful of roots over
    /// and over (§FS-005-dispatch.14).
    rosters: BTreeMap<PathBuf, runtime::roster::Roster>,
    /// What the runtime offers, per place asked — a sweep asks the same
    /// handful of roots over and over (§FS-005-dispatch.19).
    workflows: BTreeMap<PathBuf, runtime::workflow::Offered>,
    /// The person's own entries, and the ones they wrote for one project:
    /// two of the three homes a workflow entry may live in
    /// (§FS-005-dispatch.19). Kept because a sweep has to find the entry
    /// that asked to run itself without a menu being open
    /// (§FS-005-dispatch.28).
    actions: Vec<ActionConfig>,
    project_actions: BTreeMap<String, Vec<ActionConfig>>,
    /// The checkout command each project bound, where it bound one
    /// (§FS-006-project-interface.8). Held because a dry run says what it would
    /// have made and by whom, and a dry run makes nothing at all: the maker is
    /// a fact about the project rather than about the run
    /// (§FS-005-dispatch.25).
    checkouts: BTreeMap<String, crate::feed::config::CheckoutConfig>,
    /// What the reader should know about the hands this dispatcher resolved,
    /// each said once (§FS-006-project-interface.9).
    notes: Vec<String>,
    /// Work-root pre-images retained until the ledger replacement commits
    /// the whole hand-off (§FS-005-dispatch.4).
    journal: Journal,
    pub ledger: Ledger,
}

impl Dispatcher {
    /// Read the configuration and the ledger, and write nothing.
    ///
    /// Every reading command answers from the recorded plan name and is
    /// self-consistent before anything is carried over, so the carry-over
    /// belongs to the verbs entitled to write in those roots rather than to
    /// this (§FS-005-dispatch.3.1): loading it here made `work list` move
    /// files, and made a sweep the `--act` gate was holding rename plans in
    /// every project while it printed that nothing had been written
    /// (§FS-011-command-line.10, §FS-005-dispatch.26).
    pub fn load(config: &StatusConfig) -> Result<Dispatcher> {
        let dispatcher = Dispatcher {
            reply_config: config.clone(),
            registry_doc: crate::feed::commands::load_registry_doc()?,
            global: config.work.clone(),
            projects: config
                .projects
                .iter()
                .map(|(id, project)| (id.clone(), project.work.clone()))
                .collect(),
            organizations: config
                .organizations
                .iter()
                .map(|(id, organization)| (id.clone(), organization.work.clone()))
                .collect(),
            placements: BTreeMap::new(),
            behind: BTreeMap::new(),
            rosters: BTreeMap::new(),
            workflows: BTreeMap::new(),
            actions: config.actions.clone(),
            project_actions: config
                .projects
                .iter()
                .map(|(id, project)| (id.clone(), project.actions.clone()))
                .collect(),
            checkouts: config
                .projects
                .iter()
                .filter_map(|(id, project)| {
                    project
                        .checkout
                        .as_ref()
                        .map(|checkout| (id.clone(), checkout.clone()))
                })
                .collect(),
            notes: Vec::new(),
            journal: Journal::default(),
            ledger: ledger::load()?,
        };
        Ok(dispatcher)
    }

    /// What the reader should know about who got this work, each note once.
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// The recipes offered on a project: shipped, then configuration's three
    /// scopes outward in — the site's, the organization the registry places
    /// this project in, then the project's own (§FS-005-dispatch.1). The
    /// membership is read where the ceilings and the root read it, so a project
    /// no registry row places in an organization is simply offered the tier
    /// above and its own (§REQ-001-boundary.2).
    pub fn recipes(&self, project: &str) -> Vec<Recipe> {
        let per_organization = self
            .organization_work(project)
            .map(|work| work.recipes.as_slice())
            .unwrap_or_default();
        let per_project = self
            .projects
            .get(project)
            .map(|work| work.recipes.as_slice())
            .unwrap_or_default();
        recipe::resolve(&self.global.recipes, per_organization, per_project)
    }

    /// The recipes that apply to one item. A `branch` or a `root` template
    /// requiring a field this matter has not got does not serve it, so it is
    /// withheld from dispatch selection rather than selected and refused
    /// (§FS-005-dispatch.25) — another matter can carry the field and render
    /// the same template.
    pub fn offers(&mut self, item: &Item) -> Vec<Recipe> {
        if item.is_blocked() {
            return Vec::new();
        }
        let facts = self.facts(item);
        let placement = self.placement(&item.project).cloned();
        self.offered(item, &facts, placement.as_ref())
    }

    /// [`Dispatcher::offers`] on facts and a placement the caller already
    /// holds, for a reading that may not measure: whether a stale draft's
    /// recipe would still be dispatched is this test and no other
    /// (§FS-005-dispatch.13.2).
    pub fn offered(
        &self,
        item: &Item,
        facts: &recipe::Facts,
        placement: Option<&Placement>,
    ) -> Vec<Recipe> {
        if item.is_blocked() {
            return Vec::new();
        }
        let mut offers = recipe::applicable(&self.recipes(&item.project), item, facts);
        let Some(placement) = placement else {
            return offers;
        };
        offers.retain(|recipe| {
            recipe.branch.as_deref().is_none_or(|template| {
                crate::branches::why_not_served(placement, item, template).is_none()
            }) && recipe.root.as_deref().is_none_or(|template| {
                crate::branches::why_root_not_served(placement, item, template).is_none()
            })
        });
        offers
    }

    /// Why the recipe named `wanted` is not among this matter's
    /// [`offers`](Self::offers): the sentence `work dispatch --item <id>
    /// --recipe <r>` refuses with (§FS-005-dispatch.27.1), naming the recipe,
    /// the matter, and what kept them apart. It is asked of the recipes and the
    /// facts `offers` decided on, so the reason given is the one that decided.
    ///
    /// Every field that refused is named, `kinds` and `behind` included.
    /// The offers reading leaves those out because nobody asked about that
    /// recipe there; here somebody did. A name no recipe of the project carries
    /// was weighed against nothing, so it is told it is not configured rather
    /// than that it does not apply, beside the recipes that are, and pointed to
    /// `ephor work ask` (§FS-005-dispatch.10) — unless it is one of the
    /// project's workflow entries, which is pointed to `ephor work lay`.
    pub fn why_not_offered(&mut self, item: &Item, wanted: &str) -> String {
        let recipes = self.recipes(&item.project);
        let Some(recipe) = recipes.iter().find(|recipe| recipe.id == wanted) else {
            // Configured, but as a workflow entry: `work lay` is its door, not `work ask`.
            let entry = self
                .workflow_actions(&item.project)
                .is_ok_and(|entries| entries.iter().any(|entry| entry.id == wanted));
            if entry {
                return format!(
                    "'{wanted}' is a workflow entry of project '{}', not a recipe, so `work \
                     dispatch` cannot hand {} to it; `ephor work lay --item {} {wanted}` lays it",
                    item.project, item.id, item.id
                );
            }
            let configured: Vec<&str> = recipes.iter().map(|recipe| recipe.id.as_str()).collect();
            return format!(
                "no recipe '{wanted}' is configured for project '{}', so {} cannot be handed to \
                 it (it has: {}); `ephor work ask --item {}` asks for work no recipe describes",
                item.project,
                item.id,
                configured.join(", "),
                item.id
            );
        };
        // In the order `offers` and `Recipe::matches` refuse in.
        let why = if let Some(reason) = item.blocking_reason() {
            Some(format!("it is {reason}"))
        } else if let Some(reason) = recipe.reserved() {
            Some(reason)
        } else if item.is_finished() {
            Some("the matter is finished, and no recipe applies to finished work".to_string())
        } else {
            let facts = self.facts(item);
            let placement = self.placement(&item.project);
            let reasons: Vec<String> = recipe
                .withheld(item, &facts, placement)
                .into_iter()
                .map(|refusal| refusal.reason)
                .collect();
            (!reasons.is_empty()).then(|| reasons.join("; "))
        };
        match why {
            Some(why) => format!("recipe '{wanted}' does not apply to {}: {why}", item.id),
            // Not reached: `offers` withholds a recipe for exactly the reasons
            // above, read off the same recipes and the same facts, so a recipe
            // none of them refused would have been offered. Still a sentence
            // naming both, never an empty one.
            None => format!("recipe '{wanted}' is not among the offers for {}", item.id),
        }
    }

    /// What a selector needs to know about the checkout
    /// (§FS-004-quick-actions.6). Measured once per branch and remembered for
    /// the sweep: a dispatch over a whole feed asks about the same handful of
    /// branches over and over, and each answer is several git calls.
    pub fn facts(&mut self, item: &Item) -> recipe::Facts {
        let Some(placement) = self.placement(&item.project).cloned() else {
            return recipe::Facts::default();
        };
        let checkout = placement.checkout(item);
        let Some(branch) = checkout.branch.clone() else {
            return recipe::Facts::default();
        };
        // A branch nobody has checked out cannot be measured, and guessing
        // would offer work about a tree that is not on the machine.
        if !matches!(checkout.state, WorkspaceState::Ready) {
            return recipe::Facts::default();
        }
        let key = (item.project.clone(), branch);
        if let Some(facts) = self.behind.get(&key) {
            return *facts;
        }
        // Summed across the workspace's forest, like the inbox's own count:
        // one repository trailing is the workspace trailing
        // (§AR-004-forest.1). Both distances come off the one standing fold,
        // so a recipe asking about either is answered from one measurement
        // (§FS-004-quick-actions.8). A distance to a base nobody named is not
        // a fact anything acts on (§FS-004-quick-actions.6), so on a project
        // with no main branch only `behind` is nulled — the other distance is
        // to the branch's own copy, and needs no base — the same split the
        // rows and their menus make of the same measurement.
        let standing = placement.forest(&checkout.workspace).standing();
        let facts = recipe::Facts {
            behind: placement
                .main_branch
                .is_some()
                .then(|| standing.staleness().total())
                .flatten(),
            behind_upstream: standing.behind_upstream(),
        };
        self.behind.insert(key, facts);
        facts
    }

    /// What a recipe would actually ask for about this item — the brief with
    /// the item's own words in it, which is what a reader has to see before
    /// pressing the key, not the template it came from. Falls back to the
    /// template where the item cannot be placed; the refusal that follows
    /// says why better than a blank line would.
    ///
    /// And falls back the same way where the file a recipe keeps its brief in
    /// cannot be read (§FS-005-dispatch.34.3): a menu row is a row, and a
    /// refusal in the slot where the words go is worse than words that are out
    /// of date. The dry run is not this — it promises what the real dispatch
    /// would do, and refuses where that would refuse.
    pub fn brief(&mut self, item: &Item, recipe: &Recipe) -> String {
        let Ok(site) = self.site(item, recipe) else {
            return recipe.brief.clone().unwrap_or_default();
        };
        match dossier::brief(recipe, &site.values) {
            Ok(brief) => brief.text,
            Err(_) => dossier::render(recipe.brief.as_deref().unwrap_or_default(), &site.values),
        }
    }

    /// Who does one action on one project, in the order §FS-005-dispatch.14
    /// sets (§FS-006-project-interface.9). `picked` is what the reader chose
    /// for this dispatch alone — the first of the seven steps, which nothing
    /// feeds yet: choosing at the moment of asking belongs to the picker, and
    /// this is the signature it will call.
    pub fn hand(
        &mut self,
        project: &str,
        action: &str,
        picked: Option<&HandList>,
        pinned: Option<&HandList>,
        root: &std::path::Path,
    ) -> runtime::roster::Choice {
        self.ensure_roster(root);
        // What the pools have been reported to be, as of now: read from the
        // record the last refresh left, never fetched here. Probing is
        // fetching, and a network call between a reader and a ticket is what
        // keeping it under `refresh` avoids (§FS-005-dispatch.29).
        let evidence = headroom::Evidence::read(&self.global, &self.ledger, Utc::now());
        runtime::roster::resolve(
            &self.rosters[root],
            &self.global,
            self.projects.get(project),
            action,
            picked,
            pinned,
            &evidence,
        )
    }

    /// What every pool this site can reach says about itself right now
    /// (§FS-005-dispatch.29): the pools the roster reaches and the pools a
    /// verb is bound for, each with its effective remaining or the reason it
    /// is unknown. What `capabilities` and `status` report, and read from the
    /// same evidence the selection rule reads — one answer, two surfaces.
    pub fn pools(&mut self, root: Option<&std::path::Path>) -> Vec<headroom::Standing> {
        let roster = match root {
            Some(root) => {
                self.ensure_roster(root);
                self.rosters[root].clone()
            }
            None => runtime::roster::roster(&self.global, None),
        };
        let evidence = headroom::Evidence::read(&self.global, &self.ledger, Utc::now());
        let mut names = headroom::pools_of(&roster);
        for pool in self.global.headroom.keys() {
            if !names.iter().any(|have| have == pool) {
                names.push(pool.clone());
            }
        }
        names
            .into_iter()
            .map(|pool| {
                let bound = self.global.headroom.contains_key(&pool);
                evidence.standing(&pool, bound)
            })
            .collect()
    }

    /// The hands a picker may offer about this project's work, against the
    /// work root the dispatch will use (§FS-005-dispatch.14): the roster's,
    /// already without what the project's narrowing excludes. Empty where
    /// the roster is — with no runtime bound there is nothing to pick from,
    /// and the picker is simply not offered.
    pub fn pickable(
        &mut self,
        project: &str,
        root: &std::path::Path,
    ) -> Vec<runtime::roster::Hand> {
        self.ensure_roster(root);
        runtime::roster::pickable(&self.rosters[root], self.projects.get(project))
    }

    fn ensure_roster(&mut self, root: &std::path::Path) {
        if !self.rosters.contains_key(root) {
            let roster = runtime::roster::roster(&self.global, Some(root));
            // Where this root's overlay was found travels as a note beside the
            // work, never as a refusal: the deprecated name produces exactly
            // the roster its home produces, so nothing a reader had is taken
            // away (§FS-006-project-interface.12, §FS-005-dispatch.14).
            for note in roster.notes.clone() {
                self.note_once(&note);
            }
            self.rosters.insert(root.to_path_buf(), roster);
        }
    }

    /// Where an item's work root is, without making it: the ledger's answer
    /// where the item has work, and the same template `site` resolves at
    /// dispatch otherwise (§FS-006-project-interface.7) — so a surface asking
    /// "who would get this" resolves against the root the dispatch will use.
    ///
    /// `branch` is the template carried by the entry that would be dispatched,
    /// where it carries one: the root is then inside the workspace that
    /// template names, exactly as [`Dispatcher::site_for`] resolves it. A
    /// caller with no entry in hand passes `None` and gets the matter's own
    /// (§FS-005-dispatch.25).
    pub fn work_root_of(&mut self, item: &Item, branch: Option<&str>) -> Option<PathBuf> {
        if let Some(entry) = self.ledger.entries.get(&item.id) {
            return Some(entry.root.clone());
        }
        self.work_root_for(item, branch, None)
    }

    /// The root a particular recipe or entry would select. The optional
    /// override is one whole template above the configured tiers and is
    /// rendered after branch placement (§FS-005-dispatch.6.1,
    /// §FS-005-dispatch.25).
    pub fn work_root_for(
        &mut self,
        item: &Item,
        branch: Option<&str>,
        root: Option<&str>,
    ) -> Option<PathBuf> {
        let template = root
            .map(str::to_string)
            .unwrap_or_else(|| self.root_template(&item.project));
        let placement = self.placement(&item.project)?.clone();
        let checkout = match (placement.own_branch(item), branch) {
            (Some(_), _) | (None, None) => placement.own_checkout(item),
            (None, Some(template)) => {
                let minted = crate::branches::minted(&placement, item, template).ok()?;
                // Nor where the dispatch would refuse to make a private
                // matter's workspace (§FS-018-private-sources.2).
                if let WorkspaceState::Missing(target) = &minted.state {
                    if private::unmade(&self.global, &item.source, None, template, target).is_some()
                    {
                        return None;
                    }
                }
                minted
            }
        };
        let subject = Subject {
            item,
            checkout: &checkout,
            root: &placement.root,
            organization: placement.organization.as_ref(),
        };
        // A template the dispatch would refuse on has no preview to give: the
        // surface asking "who would get this" is told nothing rather than a
        // path with a gap in it (§FS-005-dispatch.6.1). The private rung first
        // and alone, as the dispatch asks it (§FS-018-private-sources.2).
        private::root(&self.global, &item.source, &placement.root, |private| {
            subject.work_root(private)
        })
        .unwrap_or_else(|| subject.work_root(&template))
        .ok()
    }

    /// Every organization work block written over an organization no registry
    /// row places a project inside, said in the note the sweep carries
    /// (§FS-005-dispatch.24). Such a key reaches nothing at all — the ceiling
    /// bounds nobody, the root places nothing, and a recipe written there is
    /// offered to no project — so the reader hears it where the thing they
    /// meant to write would have been read. It is said for reaching nobody
    /// rather than for bounding nobody, because a block carrying only recipes
    /// must not be announced as a ceiling its author never wrote. It asks the
    /// same membership [`Dispatcher::organization_of_each_project`] resolves
    /// the ceilings through, so a key that is refusing starts is never
    /// announced here.
    fn ceilings_over_nobody(&self) -> Vec<String> {
        crate::registry::organizations_over_nobody(&self.registry_doc, self.organizations.keys())
            .into_iter()
            .map(|organization| {
                format!(
                    "organizations.{organization}: no registry row places a project in it, \
                     so what is written there reaches nothing"
                )
            })
            .collect()
    }

    /// Which organization each project belongs to, as the registry declares
    /// it (§FS-005-dispatch.24). Membership is identity and lives in the
    /// registry row; the ceiling over it is a binding and lives in the site's
    /// configuration, so this reads the registry and writes nothing back to
    /// it (§REQ-001-boundary.2).
    fn organization_of_each_project(&self) -> BTreeMap<String, String> {
        crate::registry::organization_of_each_project(&self.registry_doc)
    }

    /// Every execution root beneath the watch, with the plans it holds
    /// (§FS-005-dispatch.15): the ledger's dispatches — those plans carry the
    /// matter behind them — merged with what enumerating the work roots
    /// finds, so a plan ephor never wrote is watched exactly as one it did.
    /// The walk is bounded by the registry, never by the disk: each project's
    /// checkout and each branch workspace already resolved
    /// (§FS-005-dispatch.15.1).
    pub fn work_roots(&mut self) -> Vec<runtime::watch::RootPlans> {
        let ids: Vec<String> = crate::registry::array_field(&self.registry_doc, "projects")
            .iter()
            .map(|project| crate::registry::id_of(project).to_string())
            .collect();
        let placements: Vec<Placement> = ids
            .iter()
            .filter_map(|id| self.placement(id).cloned())
            .collect();
        enumerate_roots(
            &self.global,
            &self.organizations,
            &self.projects,
            &placements,
            &self.ledger,
        )
    }

    /// Every working tree a live run holds right now, over every root beneath
    /// the watch (§FS-005-dispatch.24) — what a sweep that writes into
    /// checkouts reads before it writes in any of them. The rebase sweep and
    /// `clean` ask it here rather than each assembling the reading, so the
    /// guard they promise is one guard (§FS-017-clean.2).
    pub fn live_checkouts(&mut self) -> BTreeMap<PathBuf, PathBuf> {
        let roots = self.work_roots();
        live_checkouts(&self.global, &roots, &self.ledger)
    }

    /// What this ticket pins, and what the reader is told about it. Refuses
    /// where the choice cannot stand, so nothing is written and no opening
    /// move is made under a hand that may not have it
    /// (§FS-006-project-interface.9).
    fn pin(
        &mut self,
        item: &Item,
        recipe: &Recipe,
        picked: Option<&HandList>,
        root: &std::path::Path,
    ) -> Result<Pinned> {
        // One spelling per recipe: a hand is the checkable name for exactly
        // what `target`/`model` spell raw, and a recipe carrying both would
        // have one of them silently lose (§FS-006-project-interface.9).
        if recipe.hand.is_some() && (recipe.target.is_some() || recipe.model.is_some()) {
            return Err(EphorError::Command(format!(
                "recipe '{}' names both a hand and the runtime's own execution identity \
                 (target/model) — say one or the other",
                recipe.id
            )));
        }
        // A recipe spelling the runtime's own execution identity has pinned
        // itself — the second step — and the tables below do not displace it;
        // only the reader's own pick, the first step, does
        // (§FS-005-dispatch.14). A project that narrows the roster binds it
        // all the same: a selector no hand named is not authorized by a list
        // of names.
        if picked.is_none()
            && recipe.hand.is_none()
            && (recipe.target.is_some() || recipe.model.is_some())
        {
            if let Some(why) = runtime::roster::refuse_unnamed(
                self.projects.get(&item.project),
                &format!(
                    "recipe '{}' pins the runtime's own execution identity",
                    recipe.id
                ),
            ) {
                return Err(EphorError::Command(why));
            }
            return Ok(Pinned {
                target: recipe.target.clone(),
                model: recipe.model.clone(),
                ..Pinned::default()
            });
        }
        let choice = self.hand(
            &item.project,
            &recipe.id,
            picked,
            recipe.hand.as_ref(),
            root,
        );
        if let runtime::roster::Choice::Refused(why) = choice {
            return Err(EphorError::Command(why));
        }
        if let Some(note) = choice.note() {
            self.note_once(note);
        }
        let (target, model) = choice.pin();
        Ok(Pinned {
            target,
            model,
            said: choice.said().map(str::to_string),
            pool: choice.pool().map(str::to_string),
        })
    }

    /// Say one thing about who got the work once, however many tickets raise
    /// it (§FS-006-project-interface.9).
    fn note_once(&mut self, note: &str) {
        if !self.notes.iter().any(|have| have == note) {
            self.notes.push(note.to_string());
        }
    }

    /// The one flag spelling a run over this entry's plan may carry
    /// (§FS-005-dispatch.14): the chosen hand the plan language could not
    /// spell — an agent and no model — resolved again at the moment the run
    /// is invoked, the same moment the runtime reads its own configuration.
    /// `status` is the same entry's, as [`Dispatcher::status_of`] just read
    /// it — the plan is not read twice for one run.
    ///
    /// Flags are per-run, and what they can touch differs by what a ticket
    /// carries (§FS-005-dispatch.14): one with the full execution line is
    /// resolved from that line alone, the flags invisible to it, while one
    /// pinning a model alone would take its carrier from them; a claimed
    /// ticket is not the run's to advance at all (§FS-005-dispatch.15). So
    /// the flags ride only where they can re-aim nothing: every ticket the
    /// run would advance resolves to the same spelling, and none pins a
    /// model. None otherwise — and where a hand wanted flags the run cannot
    /// carry, the reader is told it went unbound.
    pub fn run_hand(
        &mut self,
        entry: &Entry,
        status: &WorkStatus,
    ) -> Option<runtime::roster::HandFlags> {
        // Nothing readable to resolve a hand from — not merely a plan the
        // record named that nobody could read (§FS-005-dispatch.35). A
        // matter's work is every plan the record says is its own, so one
        // unreadable plan beside a readable one leaves tasks this run would
        // advance, and the flags they resolve to still ride
        // (§FS-005-dispatch.14). That the record named a plan nobody could
        // read is said rather than swallowed, as every other thing that
        // keeps a hand off a run is.
        if status.tickets.is_empty() {
            return None;
        }
        if status.missing {
            self.note_once(
                "a plan this matter's record names could not be read; the hand was resolved \
                 from the plans that could",
            );
        }
        let recipes = self.recipes(&entry.project);
        let mut wants: Vec<Option<runtime::roster::HandFlags>> = Vec::new();
        for ticket in &status.tickets {
            // Not this run's to advance: finished, or claimed by somebody
            // (§FS-005-dispatch.15) — flags can re-aim neither.
            if ticket.finished || ticket.assignee.is_some() {
                continue;
            }
            match ticket.pinned {
                // Its own full line: the runtime resolves it from the line
                // alone, and a run's flags are invisible to it.
                Some(plan::Pin::Target) => continue,
                // A model alone would take its carrier from the flags, so its
                // presence keeps them off this run.
                Some(plan::Pin::Model) => {
                    wants.push(None);
                    continue;
                }
                None => {}
            }
            // The recipe the ticket was dispatched under, already resolved by
            // `status_of` — its own id where no dispatch recorded one.
            let action = &ticket.recipe;
            let pinned = recipes
                .iter()
                .find(|recipe| &recipe.id == action)
                .and_then(|recipe| recipe.hand.clone());
            let choice = self.hand(&entry.project, action, None, pinned.as_ref(), &entry.root);
            // At dispatch a refusal blocks the ticket; here the ticket
            // already exists, so the run goes unflagged and the reason is
            // said rather than swallowed (§FS-006-project-interface.9).
            if let runtime::roster::Choice::Refused(why) = &choice {
                self.note_once(why);
            }
            if let Some(note) = choice.note() {
                let note = note.to_string();
                self.note_once(&note);
            }
            wants.push(choice.flags());
        }
        let first = wants.iter().flatten().next()?.clone();
        if wants.iter().all(|want| want.as_ref() == Some(&first)) {
            return Some(first);
        }
        self.note_once(&format!(
            "the open tickets of {} do not agree on one hand — the run carries no \
             agent flags, and each ticket runs as it stands",
            entry.plan.display()
        ));
        None
    }

    /// The hand riding a run of one item's plan, and what the reader should
    /// be told about it — everything a surface that runs a single plan needs
    /// to know before it cedes the terminal (§FS-005-dispatch.14). The same
    /// resolution `work run` makes, over this item's entry and the plan as it
    /// stands right now, so the key and the command line cannot come apart
    /// (§FS-005-dispatch.12). One plan is one group: such a run advances no
    /// other plan, so there is nothing for its flags to contradict and no
    /// grouping to do.
    ///
    /// The notes come back rather than piling up: what an earlier keystroke
    /// was told does not silence this run's own answer, and does not travel
    /// with it either.
    pub fn run_hand_for(
        &mut self,
        item: &str,
    ) -> (Option<runtime::roster::HandFlags>, Vec<String>) {
        let said = std::mem::take(&mut self.notes);
        let hand = self.ledger.entries.get(item).cloned().and_then(|entry| {
            let status = self.status_of(&entry, None);
            self.run_hand(&entry, &status)
        });
        let notes = std::mem::replace(&mut self.notes, said);
        (hand, notes)
    }

    /// The hand riding one run over one work root (§FS-005-dispatch.14).
    ///
    /// A run asked for by name is one run over everything the record says
    /// that root's matters have open — the matter's own plan and every one a
    /// workflow laid beside it (§FS-005-dispatch.30) — so the flags have to
    /// answer for all of them. Resolved exactly as [`Dispatcher::run_hand`]
    /// resolves one entry's tickets, and for the same reason: flags are
    /// per-run, so where two matters want different ones the run carries
    /// none and the reader is told rather than having one of them re-aimed.
    pub fn run_hand_over(&mut self, due: &Due) -> Option<runtime::roster::HandFlags> {
        let mut wants: Vec<Option<runtime::roster::HandFlags>> = Vec::new();
        for item in &due.items {
            let Some(entry) = self.ledger.entries.get(item).cloned() else {
                continue;
            };
            let status = self.status_of(&entry, None);
            wants.push(self.run_hand(&entry, &status));
        }
        let first = wants.iter().flatten().next()?.clone();
        if wants.iter().all(|want| want.as_ref() == Some(&first)) {
            return Some(first);
        }
        self.note_once(&format!(
            "the open work in {} does not agree on one hand — the run carries no agent \
             flags, and each ticket runs as it stands",
            due.root.display()
        ));
        None
    }

    fn placement(&mut self, project: &str) -> Option<&Placement> {
        self.placements
            .entry(project.to_string())
            .or_insert_with(|| Placement::load(&self.registry_doc, project))
            .as_ref()
    }

    /// The states YAML installed into a work root that has none: the project's
    /// own, the global one, or the machine ephor ships.
    fn states_yaml(&self, project: &str) -> Result<String> {
        states_yaml(&self.global, self.projects.get(project))
    }

    fn root_template(&self, project: &str) -> String {
        root_template(
            &self.global,
            self.organization_work(project),
            self.projects.get(project),
        )
    }

    /// The work configuration written over the organization this project's
    /// registry row places it in, where one is written there
    /// (§FS-005-dispatch.6.1). The membership is the registry's and the
    /// binding is the site configuration's, which is the one direction this
    /// ever reads in (§REQ-001-boundary.2).
    fn organization_work(&self, project: &str) -> Option<&OrganizationWorkConfig> {
        let (organization, _) = crate::registry::organization_of(&self.registry_doc, project)?;
        self.organizations.get(&organization)
    }

    /// Where an item's work belongs, refusing where it would not run
    /// (§FS-005-dispatch.6).
    fn site(&mut self, item: &Item, recipe: &Recipe) -> Result<Site> {
        let mut site = self.site_for(
            item,
            &recipe.id,
            recipe.needs_checkout,
            recipe.branch.as_deref(),
            recipe.root.as_deref(),
        )?;
        // Previewed briefs and committed hand-offs name the same next request's
        // output; a workflow chooses its own request stem (§FS-005-dispatch.13).
        let plan_id = plan::plan_id(&item.id);
        let prior_path = self
            .recorded_plan_behind(&item.id, &plan_id)
            .map(|(_, path)| path)
            .unwrap_or_else(|| plan::plan_path_in(&site.dir, &plan_id));
        let prior = Plan::read(&prior_path)?;
        let ticket = next_ticket_id(
            self.ledger.entries.get(&item.id),
            prior.as_ref(),
            &recipe.id,
        );
        site.values.insert(
            std::borrow::Cow::Borrowed("reply"),
            runtime::results::request_reply_path(&site.dir, &plan_id, &ticket)
                .to_string_lossy()
                .into_owned(),
        );
        Ok(site)
    }

    /// The same, for an entry that is not a recipe: a workflow says what it
    /// needs on disk through its own `requires_checkout`
    /// (§FS-005-dispatch.19).
    ///
    /// `branch` is the entry's template for the branch its work belongs on,
    /// where it carries one (§FS-005-dispatch.25). It applies only to a matter
    /// with no branch of its own, and saying it means the work needs the
    /// checkout — so it decides where the work goes without resolving anything
    /// the matter already answered. `entry` is the recipe or entry carrying
    /// it, named where the template is refused.
    ///
    /// Two answers come out of it, because it is asked two questions
    /// (§FS-005-dispatch.25): [`Site::checkout`] is where the work *runs* —
    /// where the matter's code lives right now, the project's main branch
    /// included — and [`Site::dir`] is the work root it *writes*, which
    /// resolves through the matter's own placement, where the main branch is
    /// never a matter's own. They differ on exactly one matter: one the
    /// registry matched to the main branch and nothing else.
    fn site_for(
        &mut self,
        item: &Item,
        entry: &str,
        needs_checkout: bool,
        branch: Option<&str>,
        root: Option<&str>,
    ) -> Result<Site> {
        // An entry or recipe answer wins as one whole template; broader
        // configuration is consulted only when it is absent
        // (§FS-005-dispatch.6.1, §FS-005-dispatch.28).
        let template = root
            .map(str::to_string)
            .unwrap_or_else(|| self.root_template(&item.project));
        // A project ephor cannot place has nowhere to put the work, and the
        // ladder owns that sentence (§AR-005-capabilities.2).
        let placement = self.placement(&item.project).cloned().ok_or_else(|| {
            EphorError::Command(
                CapabilitySet::unknown(&item.project)
                    .refusal(&[Rung::Observable])
                    .unwrap_or_else(|| format!("{} cannot be placed", item.project)),
            )
        })?;
        // Where the matter's code lives right now, main branch included, and
        // the forest root where nothing resolves (§FS-005-dispatch.13,
        // §AR-004-forest.1) — which is what lets work about a conversation
        // run without the checkout-able rung (§FS-006-project-interface.10).
        // This is the read resolution: work that only reads the change (no
        // branch template, no needs_checkout) runs here unchanged, whether
        // or not the matter merely resembles the main branch by word match.
        let mut checkout = placement.checkout(item);
        // Saying which branch the work belongs on says that it needs the
        // checkout: the template is about where the change will be edited
        // (§FS-005-dispatch.25).
        let needs_checkout = needs_checkout || branch.is_some();
        // The matter's own branch always wins — but the project's main
        // branch is never a matter's own (§FS-005-dispatch.25). Only work
        // that edits the change asks this: a template supplies the branch a
        // matter has none of, and never displaces the one the forge recorded
        // or the registry matched; with no template, editing work has
        // nowhere to go. This is [`crate::branches::placed_through`] with
        // the refusal kept: a surface asking where the work would go falls
        // back to the matter's own placement, and the dispatch that would
        // write there refuses instead.
        let mut mint = None;
        let mut named = None;
        if needs_checkout && placement.own_branch(item).is_none() {
            match branch {
                Some(template) => {
                    checkout = crate::branches::minted(&placement, item, template)
                        .map_err(EphorError::Command)?;
                    named = Some(checkout.workspace.clone());
                    if let WorkspaceState::Missing(target) = &checkout.state {
                        // A private matter's workspace is never made: it would be a
                        // checkout of the project's named from the matter, made by
                        // the project's own command with the matter in hand. Refused
                        // here, before anything runs (§FS-018-private-sources.2).
                        let refused = private::unmade(
                            &self.global,
                            &item.source,
                            Some(entry),
                            template,
                            target,
                        );
                        if let Some(why) = refused {
                            return Err(EphorError::Command(why));
                        }
                        mint = Some(target.clone());
                    }
                }
                None => checkout = placement.own_checkout(item),
            }
        }
        // Only for work that edits the change. A review or a reply runs in
        // the project's own checkout and fetches what it needs.
        if needs_checkout {
            // Every branch workspace of this project that resolves as present
            // is asked the half-made question, not only the one a `branch`
            // template minted: the bare directory a bound command leaves
            // behind is the same directory whether nobody had cut the branch
            // or the matter owns it, and one surface of this contract refusing
            // a workspace the other dispatches into is the contradiction the
            // question exists to close (§FS-006-project-interface.8). The
            // project's own checkout is never among them — it is not a branch
            // workspace and was never the command's to make.
            named = named.or_else(|| placement.branch_workspace(&checkout));
            let wanted = checkout.branch.as_deref().unwrap_or("?");
            match &checkout.state {
                // A workspace this dispatch is about to make is not a missing
                // one: it is named, it is this matter's, and making it is the
                // move that comes after the last refusal (§FS-005-dispatch.25).
                WorkspaceState::Missing(_) if mint.is_some() => {}
                WorkspaceState::Missing(target) => {
                    return Err(EphorError::Command(format!(
                        "{}: branch {} is not checked out ({} is missing). Make it with:\n  \
                         ephor checkout --item {}",
                        item.project,
                        wanted,
                        target.display(),
                        item.id
                    )));
                }
                // The one checkout is standing on other code. There is no
                // workspace to make, so the refusal names both ways out
                // rather than offering a checkout that cannot be made
                // (§FS-005-dispatch.3).
                WorkspaceState::Elsewhere(head) => {
                    return Err(EphorError::Command(format!(
                        "{}: branch {} is not checked out — {} is standing on {}, and it is \
                         the only checkout this project has. Put the branch there:\n  \
                         git -C {} switch {}\n\
                         or give '{}' a branch_root_template in the registry, so its branches \
                         get workspaces of their own.",
                        item.project,
                        wanted,
                        placement.root.display(),
                        head,
                        placement.root.display(),
                        wanted,
                        item.project,
                    )));
                }
                // The matter is on no branch and no entry said which one its
                // work belongs on, so there is no workspace for work that
                // edits the change — and the project root of a project whose
                // checkouts are one per branch holds no change to edit. This
                // used to be written there anyway; it is refused now, which is
                // what the menu has always done (§FS-005-dispatch.25). A
                // matter matched only to the project's main branch is on no
                // branch by the same rule, and the refusal names what it
                // declined rather than calling the matter unmatched.
                WorkspaceState::Unmatched => {
                    let clause = match placement
                        .matched(item)
                        .filter(|matched| placement.is_main_branch(&matched.branch))
                    {
                        Some(matched) => format!(
                            "matched {}, this project's main branch — the trunk every workspace \
                             is grown from, not a branch of its own",
                            matched.branch
                        ),
                        None => "is on no branch".to_string(),
                    };
                    return Err(EphorError::Command(format!(
                        "{}: {} {clause}, and this work edits the change. Give the entry a \
                         'branch' template naming the branch it belongs on, so dispatch makes \
                         the workspace:\n  \"branch\": \"fix/issue-{{number}}\"\n\
                         or hand over work that reads the change instead of editing it.",
                        item.project, item.id,
                    )));
                }
                WorkspaceState::Ready => {}
            }
        }
        let subject = Subject {
            item,
            checkout: &checkout,
            root: &placement.root,
            organization: placement.organization.as_ref(),
        };
        let values = subject.placeholders();
        // Laying the plan uses the placement checkout, except where this
        // entry's branch template mints a workspace. The read-resolution
        // checkout above may be the project's main branch; main is not the
        // matter's own placement root for read-only work
        // (§FS-005-dispatch.25).
        let placed = match (placement.own_branch(item), branch) {
            (Some(_), _) | (None, None) => placement.own_checkout(item),
            (None, Some(template)) => {
                crate::branches::minted(&placement, item, template).map_err(EphorError::Command)?
            }
        };
        let laid = Subject {
            item,
            checkout: &placed,
            root: &placement.root,
            organization: placement.organization.as_ref(),
        };
        // A matter a source the site lists as private reported goes under the
        // person's own root, and no other rung is asked — not the entry's or
        // the recipe's, not a branch workspace's (§FS-018-private-sources.2).
        // Refusing here is what keeps a work root that reaches above the
        // project from becoming a directory called `{org_root}`, or a path
        // with the organization's segment simply missing
        // (§FS-005-dispatch.6.1).
        let dir = private::root(&self.global, &item.source, &placement.root, |private| {
            laid.work_root(private)
        })
        .unwrap_or_else(|| laid.work_root(&template))
        .map_err(EphorError::Command)?;
        Ok(Site {
            dir,
            dossier: subject.dossier(),
            metadata: subject.metadata(),
            values,
            runtime_root: placement.root.clone(),
            checkout: checkout.clone(),
            mint,
            named,
        })
    }

    /// Make the workspace a `branch` template named, where it named one that is
    /// not on disk (§FS-005-dispatch.25).
    ///
    /// Called after every refusal and before the first write, so a refusal
    /// still leaves nothing behind — and never on a dry run, which is the
    /// caller's to decide because it is the caller that knows it is one.
    /// It is `ephor checkout`'s own operation ([`crate::checkout::make`]), so
    /// the workspace a dispatch makes and the workspace a reader's key makes
    /// are the same thing (§FS-004-quick-actions.7) — including the project's
    /// own checkout command, where one is bound, which the operation summons
    /// rather than each of its callers (§FS-006-project-interface.8). For a
    /// matter nobody has cut a branch for this is the only maker there is, so a
    /// binding this path did not honour was a binding a site could not reach at
    /// all (§FS-005-dispatch.25).
    fn mint(&mut self, item: &Item, site: &Site) -> Result<()> {
        // A workspace that is there is not minted, and on a bound-command
        // project *there* is not *made*: the refusal is what the maker would
        // have said had the directory not hidden the ask from it
        // (§FS-006-project-interface.8).
        if let Some(why) = self.half_made(item, site) {
            return Err(EphorError::Command(why));
        }
        if site.mint.is_none() {
            return Ok(());
        }
        let branch = site.checkout.branch.clone().unwrap_or_default();
        let placement = self
            .placement(&item.project)
            .cloned()
            .ok_or_else(|| EphorError::Command(format!("{} cannot be placed", item.project)))?;
        let (made, source) = crate::checkout::make(&crate::checkout::Ask {
            placement: &placement,
            project: &item.project,
            branch: &branch,
            // Nothing inside ephor says what a branch is grown from: the
            // project's main branch answers, or the bound command decides
            // (§FS-004-quick-actions.7.4).
            from: None,
            selected_root: Some(&site.dir),
            about: Some(item),
        })?;
        // A repository the checkout refused is the checkout's own refusal, in
        // the checkout's own words, and nothing is dispatched behind it.
        if let Some(why) = made.refusal(&source) {
            return Err(EphorError::Command(why));
        }
        if let Some(note) = made.store.as_ref().and_then(|store| store.note.clone()) {
            self.note_once(&note);
        }
        Ok(())
    }

    /// Why the workspace a `branch` template named is a directory that is there
    /// without being one, where it is (§FS-006-project-interface.8).
    ///
    /// [`crate::branches::minted`] answers *checked out* by asking the
    /// filesystem for a directory, so the bare directory a bound command left
    /// behind without making the workspace reads as checked out: nothing is
    /// minted, the maker is never entered, and a store and a plan would land in
    /// a tree holding none of the project's repositories — which is the silence
    /// this whole contract is about, one attempt later. So the question the
    /// maker would have answered is asked here instead, in the maker's own
    /// words ([`crate::checkout::half_made`]).
    ///
    /// Only where the project bound a command. Where nothing is bound, ephor's
    /// git completes a half-made workspace as it always has
    /// (§FS-004-quick-actions.7), and only the command may decide what a
    /// workspace of a project that bound one is.
    fn half_made(&mut self, item: &Item, site: &Site) -> Option<String> {
        // A workspace this dispatch is about to mint is absent, and every
        // repository of it with it: there is nothing half-made about it.
        if site.mint.is_some() {
            return None;
        }
        let target = site.named.clone()?;
        let bound = self.checkouts.get(&item.project)?.clone();
        let forest = self.placement(&item.project)?.forest(&target);
        crate::checkout::half_made(&item.project, &bound, &target, &forest)
    }

    /// The deterministic opening move a recipe declares, made before the
    /// ticket costs a model (§FS-005-dispatch.12).
    ///
    /// It is the same implementation the reader's key runs
    /// (§FS-004-quick-actions.6) — two of them would eventually disagree about
    /// what a clean rebase is. Where it finishes there is nothing to dispatch;
    /// where it stops, the repository is left standing in the conflict and the
    /// report of where it got to becomes the ticket's opening.
    fn opening(&mut self, item: &Item, recipe: &Recipe) -> Result<Opening> {
        let Some(name) = recipe.opens_with.as_deref() else {
            return Ok(Opening::None);
        };
        if name != recipe::OPENING_REBASE {
            return Err(EphorError::Command(format!(
                "recipe '{}' opens with '{name}', which ephor does not know (it knows: {}).",
                recipe.id,
                recipe::OPENING_REBASE
            )));
        }
        let placement = self
            .placement(&item.project)
            .cloned()
            .ok_or_else(|| EphorError::Command(format!("{} cannot be placed", item.project)))?;
        let checkout = placement.checkout(item);
        // Nothing on disk to replay, or nothing to replay onto: the move does
        // not apply, and the ticket is written as it would have been.
        if !matches!(checkout.state, WorkspaceState::Ready) {
            return Ok(Opening::None);
        }
        let Some(base) = placement.main_branch.clone() else {
            return Ok(Opening::None);
        };
        let forest = placement.forest(&checkout.workspace);
        if forest.repos.is_empty() {
            return Ok(Opening::None);
        }
        // The opening move a recipe declares is the rebase onto the project's
        // main branch; a replay onto the branch's own copy is the reader's
        // move and has its own entry (§FS-004-quick-actions.8).
        // The ticket this dispatch is about to write is who comes for the
        // conflict, so it is left where the replay stopped
        // (§FS-005-dispatch.12).
        let outcome = crate::git::rebase(
            &forest,
            &crate::git::Onto::Base(base),
            crate::git::Stopped::Leave,
        );
        // What the replay measured is now stale: the branch it was offered for
        // has moved under the cached answer.
        if let Some(branch) = checkout.branch.clone() {
            self.behind.remove(&(item.project.clone(), branch));
        }
        if outcome.conflicted().is_empty() && outcome.stuck().is_empty() {
            return Ok(Opening::Finished);
        }
        // What is handed over becomes a paragraph of the ticket this dispatch
        // is about to write, so it is flattened by the rule the plan language
        // has for an embedded document rather than carried in as one
        // (§FS-005-dispatch.3, §FS-011-command-line.11.1).
        Ok(Opening::Stopped(outcome.in_a_body()))
    }

    /// Hand an item to the runtime under one recipe. Opens the plan when the
    /// item has none, and appends to it when it has. `picked` is what the
    /// reader chose for this dispatch alone — the first of the seven steps,
    /// made at the moment of dispatch and spent by it: nothing records it,
    /// and the next dispatch resolves from the second step down
    /// (§FS-005-dispatch.14).
    pub fn dispatch(
        &mut self,
        item: &Item,
        recipe: &Recipe,
        picked: Option<&HandList>,
        dry_run: bool,
    ) -> Result<Outcome> {
        if let Some(reason) = item.blocking_reason() {
            return Err(EphorError::Command(format!(
                "{} is {reason}; finish those prerequisite tickets before handing it over",
                item.id
            )));
        }
        // A root named before the digest is carried over before this dispatch
        // recomputes a stem in it, so no lookup is made against a name the
        // disk has not caught up to — and only here, where the verb is
        // entitled to write in that root. A dry run promises rather than moves
        // and reads the record instead, below (§FS-005-dispatch.3.1,
        // §FS-005-dispatch.26).
        match dry_run {
            false => self.carry_over_before_writing(Some(&item.id))?,
            true => {
                if let Some(why) = self.carry_over_refusal(&item.id) {
                    return Err(EphorError::Command(why));
                }
            }
        }
        let mut site = self.site(item, recipe)?;
        // Who does it, before anything is written and before the opening move
        // is made: a refusal leaves nothing behind
        // (§FS-006-project-interface.9).
        let Pinned {
            target,
            model,
            said,
            pool,
        } = self.pin(item, recipe, picked, &site.dir)?;
        let states = self.states_yaml(&item.project)?;
        let plan_id = plan::plan_id(&item.id);
        // A request owns both its binding and its output, even before that output
        // exists (§FS-005-dispatch.13).
        let prior_plan = Plan::read(&plan::plan_path_in(&site.dir, &plan_id))?;
        let request_ticket = next_ticket_id(
            self.ledger.entries.get(&item.id),
            prior_plan.as_ref(),
            &recipe.id,
        );
        let reply_path = runtime::results::request_reply_path(&site.dir, &plan_id, &request_ticket);
        site.values.insert(
            std::borrow::Cow::Borrowed("reply"),
            reply_path.to_string_lossy().into_owned(),
        );
        // What the ticket will actually ask for, read here: beside the hand
        // and the machine, and on this side of the mint, so a rendered path
        // with no file behind it leaves no workspace, no work root and no plan
        // (§FS-005-dispatch.34). The dry run reaches this too, because it
        // promises what the real dispatch would do (§FS-005-dispatch.34.3).
        let asked = dossier::brief(recipe, &site.values).map_err(EphorError::Command)?;

        // Where a machine is already in force, it answers before anything is
        // written: a recipe naming a state it does not have is refused, and a
        // refusal should leave nothing behind.
        let undeclared = |root: &WorkRoot| {
            EphorError::Command(format!(
                "recipe '{}' starts in state '{}', which the machine '{}' in {} does not declare (it has: {}).",
                recipe.id,
                recipe.state,
                root.machine,
                root.dir.display(),
                root.state_names().join(", ")
            ))
        };
        // A state that waits on files an earlier state writes is not one a
        // fresh ticket can start in: there is no earlier state, so the work
        // would be written and then sit there unrunnable — which is the thing
        // this whole check exists to prevent (§FS-005-dispatch.6).
        let unopenable = |root: &WorkRoot| {
            let openable = root.openable_states();
            let instead = match openable.is_empty() {
                true => "no state of it opens without one".to_string(),
                false => format!("states that open without one: {}", openable.join(", ")),
            };
            EphorError::Command(format!(
                "recipe '{}' starts in state '{}', which the machine '{}' in {} declares inputs for. \
                 A fresh ticket has no earlier state to have written them, so it would never run \
                 — {instead}. Give the recipe a 'state' in the work configuration.",
                recipe.id,
                recipe.state,
                root.machine,
                root.dir.display(),
            ))
        };
        let vet = |root: &WorkRoot| match () {
            _ if !root.declares(&recipe.state) => Err(undeclared(root)),
            _ if root.needs_input(&recipe.state) => Err(unopenable(root)),
            _ => Ok(()),
        };
        if dry_run {
            // A machine already in force is read without creating anything, and
            // it answers here too: a dry run that promises a ticket the real
            // dispatch would refuse is the most misleading promise of the set
            // (§FS-005-dispatch.6). Where no root exists yet there is nothing
            // to consult, and what the run promises is where the ticket would
            // go.
            if let Some(existing) = WorkRoot::open(&site.dir)? {
                vet(&existing)?;
            }
            // And so does the workspace: a directory that is there without
            // being one is a state the real dispatch refuses, so promising the
            // ticket behind it would be that same misleading promise
            // (§FS-006-project-interface.8).
            if let Some(why) = self.half_made(item, &site) {
                return Err(EphorError::Command(why));
            }
            // A dry run makes nothing, so it says what it would have made:
            // the branch, and the workspace the plan path below is inside
            // (§FS-005-dispatch.25).
            if let Some(target) = &site.mint {
                // Naming the maker, because the two answers hold different
                // things and a report that implied ephor's git where the
                // project bound its own command would be describing a run
                // nobody is about to make (§FS-006-project-interface.8).
                let note = match self.checkouts.get(&item.project) {
                    Some(checkout) => format!(
                        "{} is not checked out — the dispatch would make {} first, with {}'s own \
                         checkout command (`{}`).",
                        site.checkout.branch.as_deref().unwrap_or("?"),
                        target.display(),
                        item.project,
                        checkout.command,
                    ),
                    None => format!(
                        "{} is not checked out — the dispatch would make {} first.",
                        site.checkout.branch.as_deref().unwrap_or("?"),
                        target.display()
                    ),
                };
                self.note_once(&note);
            }
            let path = plan::plan_path_in(&site.dir, &plan_id);
            // What the next real dispatch would append to, over a root it has
            // not carried over yet: the plan the record still names
            // (§FS-005-dispatch.3.1). The move is named too, because the path
            // this report prints is not the path the plan is at today.
            let behind = match path.is_file() {
                true => None,
                false => self.recorded_plan_behind(&item.id, &plan_id),
            };
            if let Some((was, _)) = &behind {
                let note = format!(
                    "the plan of {} is still named {was} in {}; the dispatch would carry it \
                     over to {plan_id} first",
                    item.id,
                    site.dir.display()
                );
                self.note_once(&note);
            }
            let existing = match &behind {
                Some((_, recorded)) => Plan::read(recorded)?,
                None => Plan::read(&path)?,
            };
            let ticket = next_ticket_id(
                self.ledger.entries.get(&item.id),
                existing.as_ref(),
                &recipe.id,
            );
            return Ok(match existing {
                Some(_) => Outcome::Reopened {
                    plan: path,
                    ticket,
                    recipe: recipe.id.clone(),
                    changes: self
                        .ledger
                        .entries
                        .get(&item.id)
                        .map(|entry| entry.changes_since(item))
                        .unwrap_or_default(),
                },
                None => Outcome::Opened {
                    plan: path,
                    ticket,
                    recipe: recipe.id.clone(),
                },
            });
        }

        // A workspace that is there without being one is refused ahead of the
        // opening move, not only ahead of the mint: the move replays commits in
        // the workspace, so a dispatch that is going to refuse this tree must
        // refuse it before the first thing it does to it — which is what
        // [`Dispatcher::mint`]'s own guard says of itself and, asked from
        // inside the mint alone, is not yet true of this path
        // (§FS-006-project-interface.8).
        if let Some(why) = self.half_made(item, &site) {
            return Err(EphorError::Command(why));
        }
        // The deterministic move first, and the work starts where it stopped
        // (§FS-005-dispatch.12). Before the machine is consulted and before
        // anything is written, so a clean move leaves no plan behind either.
        let opening = self.opening(item, recipe)?;
        if matches!(opening, Opening::Finished) {
            return Ok(Outcome::Settled {
                move_name: recipe
                    .opens_with
                    .clone()
                    .unwrap_or_else(|| recipe.id.clone()),
            });
        }

        // The machine answers before the workspace is made, not after. Where
        // the work root is already there it is the machine that root declares;
        // where a `branch` template would mint the workspace there is no root
        // to open — the directory does not exist yet — so what is vetted is
        // the machine `ensure` installs below. Either way the refusal lands on
        // the same side of the mint as the hand and the inputs
        // (§FS-005-dispatch.25).
        match WorkRoot::open(&site.dir)? {
            Some(existing) => vet(&existing)?,
            None => vet(&WorkRoot::proposed(&site.dir, &states)?)?,
        }

        // The workspace a `branch` template named, made now: after the hand,
        // the machine and the opening move have all had their chance to refuse,
        // and before the work root below is the first thing written
        // (§FS-005-dispatch.25).
        self.begin_handoff();
        let reply_binding = self.capture_reply(item, reply_path.clone())?;
        self.journal_work_root(&site.dir)?;
        self.mint(item, &site)?;
        let root = WorkRoot::ensure(&site.dir, &states)?;
        // Read back rather than assumed: a workspace the mint just made can
        // come with a machine of the runtime's own, which `ensure` leaves
        // standing (§FS-006-project-interface.7), and that is the one the work
        // will actually run under.
        //
        // Which makes this the one refusal that outlives the mint: the machine
        // vetted above was the one ephor would have installed, and the runner
        // installed another inside the workspace this dispatch has just made.
        // The workspace is named rather than left for the reader to find
        // (§FS-005-dispatch.25) — and nothing further is made behind it,
        // because the next dispatch opens this root and refuses on the same
        // machine before minting anything.
        vet(&root).map_err(|why| match &site.mint {
            Some(target) => EphorError::Command(format!(
                "{why} The workspace {} was made before that machine could be read, and is \
                 still there; dispatching here again refuses on it without making anything \
                 further.",
                target.display()
            )),
            None => why,
        })?;
        let path = root.plan_path(&plan_id);
        self.journal.remember(&path)?;
        let mut brief = asked.text;
        // Which words this ticket got: the rendered path and a hash of the
        // bytes as read, on the ticket rather than in the dossier the next
        // reopen rewrites (§FS-005-dispatch.34.2, §FS-005-dispatch.8).
        let metadata: Vec<(&'static str, String)> = site
            .metadata
            .iter()
            .cloned()
            .chain(
                asked
                    .instruction
                    .iter()
                    .flat_map(dossier::Instruction::metadata),
            )
            .collect();
        // What is handed over is the situation rather than the request to
        // reproduce it: the repository is standing in what this report
        // describes (§FS-005-dispatch.12).
        if let Opening::Stopped(report) = &opening {
            brief = format!("{brief}\n\n{report}");
        }
        // And who it went to, where there was a choice to make: the plan
        // itself says who and why, so a reader who was not there can read both
        // (§FS-005-dispatch.29).
        if let Some(said) = &said {
            brief = format!("{brief}\n\n{}", chose(said));
        }
        let changes = self
            .ledger
            .entries
            .get(&item.id)
            .map(|entry| entry.changes_since(item))
            .unwrap_or_default();

        let (outcome, ticket_id) = match Plan::read(&path)? {
            None => {
                let ticket_id = request_ticket.clone();
                let ticket = Ticket {
                    id: ticket_id.clone(),
                    title: format!("{} — {}", recipe.description, item.title),
                    state: recipe.state.clone(),
                    prior: None,
                    target: target.clone(),
                    model: model.clone(),
                    body: brief,
                };
                let mut plan =
                    Plan::create(&path, &root.machine, &item.title, &site.dossier, &ticket);
                plan.set_metadata(&ticket_id, &metadata);
                plan.save()?;
                (
                    Outcome::Opened {
                        plan: path.clone(),
                        ticket: ticket_id.clone(),
                        recipe: recipe.id.clone(),
                    },
                    ticket_id,
                )
            }
            Some(mut existing) => {
                let ticket_id = request_ticket.clone();
                // The last ticket that is about *this* matter, not simply the
                // last one: a ticket ordered after work about something else
                // waits that work out (§FS-005-dispatch.5).
                let prior = existing
                    .last_ticket_about(&plan_id, &item.id)
                    .map(|ticket| ticket.id);
                let mut body = String::new();
                if !changes.is_empty() {
                    body.push_str(&format!(
                        "Since the previous ticket: {}. The item above has been \
                         rewritten to what it is now.\n\n",
                        changes.join("; ")
                    ));
                }
                body.push_str(&brief);
                existing.set_dossier(&site.dossier);
                existing.append(&Ticket {
                    id: ticket_id.clone(),
                    title: format!("{} — {}", recipe.description, item.title),
                    state: recipe.state.clone(),
                    prior,
                    target: target.clone(),
                    model: model.clone(),
                    body,
                });
                existing.set_metadata(&ticket_id, &metadata);
                existing.save()?;
                (
                    Outcome::Reopened {
                        plan: path.clone(),
                        ticket: ticket_id.clone(),
                        recipe: recipe.id.clone(),
                        changes: changes.clone(),
                    },
                    ticket_id,
                )
            }
        };

        let entry = self.ledger.entries.entry(item.id.clone()).or_insert(Entry {
            project: item.project.clone(),
            title: item.title.clone(),
            url: item.url.clone(),
            root: root.dir.clone(),
            checkout: site.checkout.workspace.clone(),
            branch: site.checkout.branch.clone(),
            plan_id: plan_id.clone(),
            plan: path.clone(),
            dispatches: Vec::new(),
            pool: pool.clone(),
        });
        entry.retain_dispatch_placements();
        entry.title = item.title.clone();
        entry.url = item.url.clone();
        entry.root = root.dir.clone();
        entry.checkout = site.checkout.workspace.clone();
        entry.branch = site.checkout.branch.clone();
        entry.plan = path;
        entry.pool = pool;
        entry.dispatches.push(Dispatch {
            reply_binding,
            reply_path: Some(reply_path),
            ticket: ticket_id,
            recipe: recipe.id.clone(),
            at: Utc::now(),
            // A ticket goes into the item's own plan, which the entry already
            // names (§FS-005-dispatch.3).
            plan: None,
            root: Some(root.dir.clone()),
            checkout: Some(site.checkout.workspace.clone()),
            branch: site.checkout.branch.clone(),
            pools: Vec::new(),
            snapshot: Snapshot::of(item),
        });
        Ok(outcome)
    }

    /// What the runtime offers about this project's work
    /// (§FS-005-dispatch.19). Asked at the project's own root, because a
    /// project keeps workflows of its own beside its checkout — and asked
    /// once successfully per root, since a sweep asks about the same handful
    /// over and over. A failed operation is returned and never cached as an
    /// empty answer.
    pub fn workflows(&mut self, project: &str) -> Result<runtime::workflow::Offered> {
        let Some(at) = self
            .placement(project)
            .map(|placement| placement.root.clone())
        else {
            return Ok(runtime::workflow::Offered::default());
        };
        if let Some(offered) = self.workflows.get(&at) {
            return Ok(offered.clone());
        }
        let offered = runtime::workflow::offered(&self.global, &at)?;
        self.workflows.insert(at, offered.clone());
        Ok(offered)
    }

    /// The entries written beside the workflows this project can reach — the
    /// third home an entry may live in (§FS-005-dispatch.19), and the only
    /// one that travels with the workflow itself. Each comes back with where
    /// its workflow was found, because that is where the entry ranks in the
    /// menu. An entry that would not parse is reported rather than dropped
    /// (§FS-004-quick-actions.3).
    pub fn workflow_entries(
        &mut self,
        project: &str,
    ) -> Result<Vec<(runtime::workflow::Source, ActionConfig)>> {
        let offered = self.workflows(project)?;
        let mut entries = Vec::new();
        for workflow in &offered.workflows {
            match crate::work::workflow::beside(workflow) {
                Ok(Some(entry)) => entries.push((workflow.source, entry)),
                Ok(None) => {}
                Err(why) => self.note_once(&why),
            }
        }
        Ok(entries)
    }

    /// Every workflow entry this project has, in the menu's own provenance
    /// order (§FS-005-dispatch.19): what travels with the workflow, then the
    /// project's own, then the person's, an entry a narrower home repeats
    /// replacing the one it displaces.
    ///
    /// The same assembly the menu makes, and deliberately the same one: a
    /// sweep that ranked the three homes differently from the screen would
    /// lay down an entry the reader never saw offered (§AR-009-surfaces.1).
    /// What it leaves out is the menu's gating — which is a question about a
    /// keystroke, and a sweep answers it by laying the entry down and
    /// reporting the refusal (§FS-005-dispatch.28).
    pub fn workflow_actions(&mut self, project: &str) -> Result<Vec<ActionConfig>> {
        let beside = self.workflow_entries(project)?;
        let from = |want: runtime::workflow::Source| -> Vec<ActionConfig> {
            beside
                .iter()
                .filter(|(source, _)| *source == want)
                .map(|(_, entry)| entry.clone())
                .collect()
        };
        let offers: Vec<ActionConfig> = self
            .placement(project)
            .and_then(Placement::manifest)
            .map(|manifest| {
                manifest
                    .offers
                    .iter()
                    .map(crate::manifest::Offer::action)
                    .collect()
            })
            .unwrap_or_default();
        let configured: Vec<ActionConfig> = self
            .actions
            .iter()
            .chain(
                self.project_actions
                    .get(project)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
            )
            .cloned()
            .collect();
        let mut entries = crate::api::offers::merge(vec![
            from(runtime::workflow::Source::Runtime),
            [offers, from(runtime::workflow::Source::Project)].concat(),
            [configured, from(runtime::workflow::Source::Person)].concat(),
        ]);
        entries.retain(|entry| entry.workflow.is_some());
        Ok(entries)
    }

    /// The workflow entries offered on one matter, in that same order
    /// (§FS-005-dispatch.19) — the selector, in the language every other
    /// entry is selected by.
    ///
    /// Finished work is never among them, exactly as it is never among a
    /// recipe's offers ([`recipe::Recipe::matches`]) and never on the menu
    /// (§FS-005-dispatch.6): a merged pull request with a red gate stays in
    /// the feed as news, and handing news to an agent is asking it to invent
    /// something to do.
    pub fn workflow_offers(&mut self, item: &Item) -> Result<Vec<ActionConfig>> {
        if item.is_finished() || item.is_blocked() {
            return Ok(Vec::new());
        }
        let facts = self.facts(item);
        let mut entries = self.workflow_actions(&item.project)?;
        let placement = self.placement(&item.project).cloned();
        entries.retain(|entry| {
            entry.matches(item, &facts)
                && entry.branch.as_deref().is_none_or(|template| {
                    placement.as_ref().is_none_or(|placement| {
                        crate::branches::why_not_served(placement, item, template).is_none()
                    })
                })
        });
        Ok(entries)
    }

    /// The exact plan the newest matching workflow-action dispatch already
    /// laid for this unchanged matter, where it is still on disk
    /// (§FS-005-dispatch.19).
    ///
    /// The newest matching record owns the answer even when an older one would
    /// still match the current snapshot. A moved matter or a missing plan is
    /// not evidence to fall back past: either means the action may be laid
    /// again by its existing path.
    pub fn repeated_workflow(&self, item: &Item, entry_id: &str) -> Option<PathBuf> {
        let (project, plan_id, root, ambiguous_legacy_identity) = {
            let entry = self.ledger.entries.get(&item.id)?;
            let dispatch = entry
                .dispatches
                .iter()
                .rev()
                .find(|dispatch| dispatch.is_workflow() && dispatch.recipe == entry_id)?;
            if !dispatch.snapshot.changes(&Snapshot::of(item)).is_empty() {
                return None;
            }
            let plan_id = dispatch.plan.clone()?;
            let ambiguous_legacy_identity = dispatch.root.is_none()
                && entry
                    .dispatches
                    .iter()
                    .filter(|candidate| {
                        candidate.is_workflow()
                            && candidate.recipe == entry_id
                            && candidate.plan.as_deref() == Some(plan_id.as_str())
                    })
                    .count()
                    > 1;
            (
                entry.project.clone(),
                plan_id,
                dispatch.root.clone(),
                ambiguous_legacy_identity,
            )
        };

        // New records retain the selected dispatch's own root, so a missing
        // newest plan never falls back to older work with the same id.
        if let Some(root) = root {
            return runtime::workflow::laid(&root.join(&plan_id)).map(|found| found.path);
        }
        if ambiguous_legacy_identity {
            return None;
        }

        // A later dispatch can move the item-level ledger entry to another
        // work root. Find the selected dispatch's plan in the same bounded
        // recorded-work reading the board and named starts use, instead of
        // resolving every historical dispatch below that mutable root
        // (§FS-005-dispatch.19). The old ledger shape has no per-dispatch
        // root, so more than one distinct plan with this identity is not
        // positive evidence: guessing one could refuse with somebody else's
        // plan.
        let mut found = BTreeMap::new();
        let placements: Vec<Placement> =
            crate::registry::array_field(&self.registry_doc, "projects")
                .iter()
                .filter(|candidate| crate::registry::id_of(candidate) == project.as_str())
                .filter_map(|project| {
                    Placement::load(&self.registry_doc, crate::registry::id_of(project))
                })
                .collect();
        for plan in enumerate_roots(
            &self.global,
            &self.organizations,
            &self.projects,
            &placements,
            &self.ledger,
        )
        .into_iter()
        .flat_map(|root| root.plans)
        {
            if plan.project == project
                && plan.plan_id == plan_id
                && plan.item.as_deref().is_none_or(|id| id == item.id)
            {
                found.entry(canonical(&plan.path)).or_insert(plan.path);
            }
        }
        if found.len() == 1 {
            found.into_values().next()
        } else {
            None
        }
    }

    /// The ids of this project's workflow entries that asked to run
    /// themselves (§FS-005-dispatch.28). What the due sweep matches a laid
    /// plan's laying entry against: silence is the key, so an id that is not
    /// here is nobody's to start.
    pub fn autorun_workflows(&mut self, project: &str) -> Result<BTreeSet<String>> {
        Ok(self
            .workflow_actions(project)?
            .into_iter()
            .filter(|entry| entry.workflow.as_ref().is_some_and(|ask| ask.autorun))
            .map(|entry| entry.id)
            .collect())
    }

    /// Everything one workflow entry would write, with nothing written yet
    /// (§FS-005-dispatch.19): which workflow, every input answered and where
    /// its answer came from, and the plan it would lay down. A refusal here
    /// leaves nothing behind, which is the point of resolving before writing.
    pub fn laying(
        &mut self,
        item: &Item,
        entry: &ActionConfig,
        typed: &BTreeMap<String, String>,
        picked: Option<&HandList>,
    ) -> Result<Laying> {
        self.laying_with_values(item, entry, typed, &serde_json::Map::new(), false, picked)
    }

    /// Everything one workflow entry would write, including explicitly
    /// loaded values files.
    pub fn laying_with_values(
        &mut self,
        item: &Item,
        entry: &ActionConfig,
        typed: &BTreeMap<String, String>,
        file_values: &serde_json::Map<String, Value>,
        values_file_supplied: bool,
        picked: Option<&HandList>,
    ) -> Result<Laying> {
        if let Some(reason) = item.blocking_reason() {
            return Err(EphorError::Command(format!(
                "{} is {reason}; finish those prerequisite tickets before handing it over",
                item.id
            )));
        }
        let ask = entry.workflow.clone().ok_or_else(|| {
            EphorError::Command(format!("action '{}' lays down no workflow", entry.id))
        })?;
        let offered = self.workflows(&item.project)?;
        let workflow = offered.find(&ask.name).cloned().ok_or_else(|| {
            EphorError::Command(match &offered.refusal {
                Some(why) => format!("'{}' lays down '{}', and {why}", entry.id, ask.name),
                None => format!(
                    "'{}' names the workflow '{}', which this runtime does not offer{}",
                    entry.id,
                    ask.name,
                    match offered.workflows.is_empty() {
                        true => String::new(),
                        false => format!(
                            " (it offers: {})",
                            offered
                                .workflows
                                .iter()
                                .map(|workflow| workflow.id.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    }
                ),
            })
        })?;
        crate::work::workflow::validate_file_values(&workflow, file_values)
            .map_err(EphorError::Command)?;
        let site = self.site_for(
            item,
            &entry.id,
            entry.requires_checkout,
            entry.branch.as_deref(),
            entry.root.as_deref(),
        )?;
        // Where the plan goes: named after the matter and the entry, and
        // named apart from what an earlier run of the same entry left, since
        // two runs of one workflow about one item are two records and not a
        // correction of the first (§FS-005-dispatch.19).
        let plan_id = free_plan_id(&site.dir, &plan::laid_plan_id(&item.id, &entry.id));
        let output = site.dir.join(&plan_id);
        // What ephor knows reaches a workflow as files: the paths are fixed
        // here so an answer may name them, and the files themselves are
        // written before the workflow is (§FS-005-dispatch.19).
        let carried = carried(&site.dir, &plan_id);
        let mut values = site.values.clone();
        // Workflow answers receive their distinct request output too
        // (§FS-005-dispatch.13).
        values.insert(
            std::borrow::Cow::Borrowed("reply"),
            runtime::results::request_reply_path(&site.dir, &plan_id, "workflow")
                .to_string_lossy()
                .into_owned(),
        );
        values.insert(
            std::borrow::Cow::Borrowed("dossier"),
            for_shell(&carried.join(DOSSIER)),
        );
        values.insert(
            std::borrow::Cow::Borrowed("item"),
            for_shell(&carried.join(ITEM)),
        );
        // Who does the work, before anything is answered: a refusal leaves
        // nothing behind (§FS-006-project-interface.9).
        let choice = self.hand(&item.project, &entry.id, picked, None, &site.dir);
        if let runtime::roster::Choice::Refused(why) = choice {
            return Err(EphorError::Command(why));
        }
        if let Some(note) = choice.note() {
            self.note_once(note);
        }
        let said = choice.said().map(str::to_string);
        let pool = choice.pool().map(str::to_string);
        let hand = choice.pin().0;
        let named = self.named_hands(item, entry, &ask, &workflow, typed, file_values, &site.dir);
        // What this work needs, and whether this site can have all of it at
        // once. Asked *after* the veto has chosen among whatever alternates
        // each target named, and over what it left standing
        // (§FS-005-dispatch.29, §FS-005-dispatch.33).
        let pools = Self::required_pools(pool.as_deref(), &named);
        let held = headroom::Evidence::read(&self.global, &self.ledger, Utc::now()).held(&pools);
        let answered = crate::work::workflow::answer_with_values(
            &workflow,
            &ask,
            typed,
            file_values,
            &values,
            hand.as_deref(),
            &|name: &str| {
                named
                    .get(name)
                    .map(|named| named.target.clone())
                    .unwrap_or_else(|| Err(format!("'{name}' is not a hand this runtime knows")))
            },
        );
        Ok(Laying {
            entry: entry.id.clone(),
            workflow,
            answered,
            plan_id,
            output,
            preflight_runtime: values_file_supplied,
            site,
            said,
            pool,
            pools,
            held,
        })
    }

    /// Why this entry's work cannot be had here at all right now, where it
    /// cannot (§FS-005-dispatch.33). The requirement a laying would derive,
    /// asked without resolving the rest of the laying, so the menu row is
    /// gated on the clause the door itself would refuse with
    /// (§AR-005-capabilities.2).
    ///
    /// None for anything it cannot resolve: a row that guessed would be a row
    /// the dispatch disagrees with, and a workflow this runtime does not offer
    /// already has a refusal of its own.
    pub fn held_for_entry(
        &mut self,
        item: &Item,
        entry: &ActionConfig,
        root: &std::path::Path,
    ) -> Option<headroom::Held> {
        let ask = entry.workflow.clone()?;
        let workflow = self
            .workflows(&item.project)
            .ok()?
            .find(&ask.name)
            .cloned()?;
        let pool = self
            .hand(&item.project, &entry.id, None, None, root)
            .pool()
            .map(str::to_string);
        let named = self.named_hands(
            item,
            entry,
            &ask,
            &workflow,
            &BTreeMap::new(),
            &serde_json::Map::new(),
            root,
        );
        let pools = Self::required_pools(pool.as_deref(), &named);
        headroom::Evidence::read(&self.global, &self.ledger, Utc::now()).held(&pools)
    }

    /// Every hand the entry or the reader named for this workflow, resolved
    /// through the seven steps before any of them is used
    /// (§DA-006-hands-fill-a-workflows-targets). Resolved up front because
    /// one refusal is the whole entry's refusal, and because answering the
    /// inputs must not need the roster again halfway through.
    fn named_hands(
        &mut self,
        item: &Item,
        entry: &ActionConfig,
        ask: &crate::feed::config::WorkflowAsk,
        workflow: &runtime::workflow::Workflow,
        typed: &BTreeMap<String, String>,
        file_values: &serde_json::Map<String, Value>,
        root: &std::path::Path,
    ) -> BTreeMap<String, NamedHand> {
        let is_hand = |name: &str| {
            workflow
                .input(name)
                .map(|input| input.hand)
                .unwrap_or(false)
                || ask.hands.iter().any(|listed| listed == name)
        };
        let mut names: Vec<String> = Vec::new();
        for (input, value) in &ask.inputs {
            if is_hand(input) && !typed.contains_key(input) && !file_values.contains_key(input) {
                collect_names(value, &mut names);
            }
        }
        for (input, value) in file_values {
            if is_hand(input) && !typed.contains_key(input) {
                collect_names(value, &mut names);
            }
        }
        for (input, word) in typed {
            if is_hand(input) {
                // Read as the input's own type, exactly as the answering will
                // read it: a line naming several hands is several names here
                // too, or the list would be looked up as one hand called
                // `["a","b"]` (§DA-006-hands-fill-a-workflows-targets).
                let kind = workflow
                    .input(input)
                    .map(|input| input.kind)
                    .unwrap_or(runtime::workflow::Kind::Text);
                collect_names(&crate::work::workflow::coerce(word, kind), &mut names);
            }
        }
        names.sort();
        names.dedup();
        names
            .into_iter()
            // Empty means nobody chose, so there is neither a hand pin to
            // parse nor a roster choice to make (§FS-005-dispatch.19).
            .filter(|name| !name.trim().is_empty())
            .map(|name| {
                // One name per input here, never a list: an execution
                // target is one line and a workflow's input has no place to
                // put an alternate (§DA-006-hands-fill-a-workflows-targets).
                let (target, pool) = match recipe::HandPin::parse(&name) {
                    Err(why) => (Err(why), None),
                    Ok(pin) => {
                        let pin = HandList::one(pin);
                        let choice = self.hand(&item.project, &entry.id, None, Some(&pin), root);
                        // The pool this target's work would be bought
                        // against, kept beside the answer because what the
                        // work needs is derived from the hands ephor itself
                        // resolved and never declared (§FS-005-dispatch.33).
                        let pool = choice.pool().map(str::to_string);
                        let target = match &choice {
                            // A required target nothing here reaches already
                            // refuses and already writes nothing; what
                            // §FS-005-dispatch.33 adds is the sentence, which
                            // names the target, says nothing here reaches it,
                            // and names no instant — there is no window to
                            // reopen.
                            runtime::roster::Choice::Refused(why) => Err(format!(
                                "{why}; nothing here reaches '{name}', so work that needs it \
                                 cannot be done here at all — answer that target with a hand \
                                 this site has"
                            )),
                            runtime::roster::Choice::Unasked { .. } => Ok(None),
                            chosen => match chosen.pin().0 {
                                Some(target) => Ok(Some(target)),
                                // A hand with no model of its own rides a run
                                // as flags and has no selector to write into
                                // an input (§FS-005-dispatch.14).
                                None => Err(format!(
                                    "hand '{name}' names an agent with no model of its own, and \
                                     an input naming who does the work needs the full spelling"
                                )),
                            },
                        };
                        (target, pool)
                    }
                };
                (name, NamedHand { target, pool })
            })
            .collect()
    }

    /// Every pool one laying's work would be bought against, at once
    /// (§FS-005-dispatch.33): the hands answering the inputs that name who
    /// does the work, together with the hand the entry's own pin chose.
    ///
    /// Derived, never declared — no configuration key carries it, so
    /// answering a target with a hand on another pool changes what the work
    /// needs along with it (§DA-010-work-is-admitted-whole). Sorted and
    /// distinct, so one requirement reads one way wherever it is printed.
    fn required_pools(
        entry_pool: Option<&str>,
        named: &BTreeMap<String, NamedHand>,
    ) -> Vec<String> {
        named
            .values()
            .filter_map(|hand| hand.pool.clone())
            .chain(entry_pool.map(str::to_string))
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect()
    }

    /// Lay a workflow's plan down beside the item's (§FS-005-dispatch.19).
    /// Writes files and nothing else: what runs the plan is the reader, from
    /// the board where every other operation is run
    /// ([§FS-005-dispatch.7](crate)).
    pub fn lay(&mut self, item: &Item, laying: &Laying, dry_run: bool) -> Result<Laid> {
        if let Some(why) = laying.refusal() {
            return Err(EphorError::Command(why));
        }
        // A directory that is there without being a workspace is the maker's
        // refusal, said before anything is staged and on a dry run as on the
        // real one: [`Dispatcher::mint`] below would say it anyway, and a
        // refusal arriving after the preflight is a refusal that left files
        // behind (§FS-006-project-interface.8).
        if let Some(why) = self.half_made(item, &laying.site) {
            return Err(EphorError::Command(why));
        }
        if laying.preflight_runtime && (!dry_run || laying.site.mint.is_some()) {
            // Values-file inputs are validated against the binding before a
            // real destination is created or a missing-workspace dry run
            // promises one. Staged carried files keep both paths atomic
            // (§FS-005-dispatch.19).
            let destination = carried(&laying.site.dir, &laying.plan_id);
            let staged = StagedWorkflow::new(
                &laying.answered.values,
                &destination,
                &format!("# {}\n\n{}", item.title, laying.site.dossier),
                &identifiers(&laying.site.metadata),
            )?;
            let runtime_at = match laying.site.checkout.workspace.is_dir() {
                true => &laying.site.checkout.workspace,
                false => &laying.site.runtime_root,
            };
            runtime::workflow::lay(
                &self.global,
                runtime_at,
                &laying.workflow,
                &staged.values,
                &laying.output,
                true,
            )
            .map_err(|err| staged.restore_destination_error(err, &destination))?;
        }
        // A run asked what it would do makes nothing at all. Where the
        // workspace itself is not there yet, that has to include the work root
        // and the files the runtime would be shown: they have nowhere to go
        // until the workspace exists, and a dry run that made the directory
        // tree to report on it would be the thing this refuses to do
        // (§FS-005-dispatch.25).
        if dry_run {
            if let Some(target) = &laying.site.mint {
                return Ok(Laid {
                    outcome: Outcome::Laid {
                        plan: laying.output.clone(),
                        plan_id: laying.plan_id.clone(),
                        workflow: laying.workflow.id.clone(),
                        entry: laying.entry.clone(),
                    },
                    report: format!(
                        "would check out {} at {} first, and lay {} down inside it",
                        laying.site.checkout.branch.as_deref().unwrap_or("?"),
                        target.display(),
                        laying.workflow.id,
                    ),
                });
            }
        }
        let states = self.states_yaml(&item.project)?;
        let values_json = serde_json::to_string_pretty(&laying.answered.values)
            .unwrap_or_else(|_| "{}".to_string());
        if dry_run {
            // The runtime validates the resolved inputs even when it is only
            // reporting what it would render. Its values and carried files
            // are staged in a private temporary directory so asking that
            // question cannot make any part of the destination
            // (§FS-005-dispatch.19).
            let destination = carried(&laying.site.dir, &laying.plan_id);
            let staged = StagedWorkflow::new(
                &laying.answered.values,
                &destination,
                &format!("# {}\n\n{}", item.title, laying.site.dossier),
                &identifiers(&laying.site.metadata),
            )?;
            let report = runtime::workflow::lay(
                &self.global,
                &laying.site.checkout.workspace,
                &laying.workflow,
                &staged.values,
                &laying.output,
                true,
            )
            .map(|report| staged.restore_destination_paths(&report, &destination))
            .map_err(|err| staged.restore_destination_error(err, &destination))?;
            return Ok(Laid {
                outcome: Outcome::Laid {
                    plan: laying.output.clone(),
                    plan_id: laying.plan_id.clone(),
                    workflow: laying.workflow.id.clone(),
                    entry: laying.entry.clone(),
                },
                report,
            });
        }
        // Everything above could still refuse; nothing above has written
        // anything. The workspace goes in here, and the work root is the first
        // thing inside it (§FS-005-dispatch.25) — and a root named before the
        // digest is carried over first, because this is the moment ephor is
        // entitled to write in it (§FS-005-dispatch.3.1).
        self.carry_over_before_writing(Some(&item.id))?;
        self.begin_handoff();
        let workflow_reply_path =
            runtime::results::request_reply_path(&laying.site.dir, &laying.plan_id, "workflow");
        let workflow_reply_binding = self.capture_reply(item, workflow_reply_path.clone())?;
        self.journal_work_root(&laying.site.dir)?;
        self.mint(item, &laying.site)?;
        let root = WorkRoot::ensure(&laying.site.dir, &states)?;
        let carried = carried(&root.dir, &laying.plan_id);
        self.journal.remember(&carried)?;
        self.journal.remember(&laying.output)?;
        std::fs::create_dir_all(&carried).map_err(|err| {
            EphorError::Command(format!("Cannot make {}: {err}", carried.display()))
        })?;
        let put = |name: &str, content: &str| -> Result<PathBuf> {
            let path = carried.join(name);
            std::fs::write(&path, content).map_err(|err| {
                EphorError::Command(format!("Cannot write {}: {err}", path.display()))
            })?;
            Ok(path)
        };
        // The matter's own name leads it: a ticket carries the title in the
        // plan's heading, and a workflow's plan is not ephor's to write — so
        // without this the one thing every reader looks for first would be
        // the one thing missing (§FS-005-dispatch.2).
        // A workflow's plan is the runtime's to write, so the choice is
        // recorded in the one thing ephor does write beside it — the dossier
        // it carries there (§FS-005-dispatch.29, §REQ-001-boundary.1).
        let recorded = match &laying.said {
            Some(said) => format!("\n\n{}", chose(said)),
            None => String::new(),
        };
        put(
            DOSSIER,
            &format!(
                "# {}\n\n{}{recorded}",
                item.title,
                laying.site.dossier.trim_end()
            ),
        )?;
        put(ITEM, &identifiers(&laying.site.metadata))?;
        let values = put(VALUES, &values_json)?;
        let report = runtime::workflow::lay(
            &self.global,
            &laying.site.checkout.workspace,
            &laying.workflow,
            &values,
            &laying.output,
            false,
        )?;
        // Where the plan actually landed is the binding's answer, not a path
        // ephor composed (§AR-007-runtime.1).
        let plan = runtime::workflow::laid(&laying.output)
            .map(|found| found.path)
            .unwrap_or_else(|| laying.output.clone());
        let entry_id = laying.entry.clone();
        let ledger_entry = self.ledger.entries.entry(item.id.clone()).or_insert(Entry {
            project: item.project.clone(),
            title: item.title.clone(),
            url: item.url.clone(),
            root: root.dir.clone(),
            checkout: laying.site.checkout.workspace.clone(),
            branch: laying.site.checkout.branch.clone(),
            // The item's own plan, whether or not it has one yet: a workflow
            // lays down a plan beside it and never replaces it
            // (§FS-005-dispatch.3).
            plan_id: plan::plan_id(&item.id),
            plan: plan::plan_path_in(&root.dir, &plan::plan_id(&item.id)),
            dispatches: Vec::new(),
            pool: laying.pool.clone(),
        });
        ledger_entry.retain_dispatch_placements();
        ledger_entry.title = item.title.clone();
        ledger_entry.url = item.url.clone();
        ledger_entry.root = root.dir.clone();
        ledger_entry.checkout = laying.site.checkout.workspace.clone();
        ledger_entry.branch = laying.site.checkout.branch.clone();
        ledger_entry.dispatches.push(Dispatch {
            reply_binding: workflow_reply_binding,
            reply_path: Some(workflow_reply_path),
            ticket: String::new(),
            recipe: entry_id.clone(),
            at: Utc::now(),
            plan: Some(laying.plan_id.clone()),
            root: Some(root.dir.clone()),
            checkout: Some(laying.site.checkout.workspace.clone()),
            branch: laying.site.checkout.branch.clone(),
            // What this plan needs to reach its end, written with it: the
            // unattended sweep decides about roots on disk and never sees the
            // entry that laid them (§FS-005-dispatch.33).
            pools: laying.pools.clone(),
            snapshot: Snapshot::of(item),
        });
        Ok(Laid {
            outcome: Outcome::Laid {
                plan,
                plan_id: laying.plan_id.clone(),
                workflow: laying.workflow.id.clone(),
                entry: entry_id,
            },
            report,
        })
    }

    /// Ask an item for something no recipe covers, in the reader's own words
    /// (§FS-005-dispatch.10). An ordinary ticket in every other respect — the
    /// same dossier, the same plan, the same order — and refused for nothing
    /// but being unrunnable: what is asked for is asked for.
    pub fn ask(
        &mut self,
        item: &Item,
        words: &str,
        state: Option<&str>,
        dry_run: bool,
    ) -> Result<Outcome> {
        let words = words.trim();
        if words.is_empty() {
            return Err(EphorError::Command("Nothing was asked for.".to_string()));
        }
        let recipe = Recipe {
            id: "ask".to_string(),
            icon: "✎".to_string(),
            // The first line names the ticket; the whole of it is the brief.
            description: summarize(words),
            state: state.unwrap_or(WORKING_STATE).to_string(),
            when: Default::default(),
            // Asked for on the spot, about whatever is on screen: a branch
            // that is not here is a fact for the dossier to state, not a
            // reason to refuse the reader.
            needs_checkout: false,
            // And so it mints nothing: what is asked for on the spot is asked
            // about the matter as it stands (§FS-005-dispatch.25).
            branch: None,
            // An ad-hoc ask has no per-recipe placement and keeps the
            // configured project/organization/site answer
            // (§FS-005-dispatch.6.1).
            root: None,
            // Typed on the spot by somebody who is right there: the reader
            // starts it, as they always did (§FS-005-dispatch.24), and there
            // is nothing to sweep for — the matter is the one in front of them
            // (§FS-005-dispatch.32).
            autorun: false,
            dispatch: None,
            brief: Some(words.to_string()),
            // Typed on the spot, so the words are right here: there is no file
            // to go and read, and nothing to record about which version of one
            // this was (§FS-005-dispatch.34).
            brief_file: None,
            based_in: None,
            // What was asked for is what is written down: ephor does not make
            // a move of its own in front of somebody's own words.
            opens_with: None,
            // An ask pins nobody of its own, so `hands` answers for it under
            // the id 'ask' like any other action (§FS-006-project-interface.9).
            hand: None,
            target: None,
            model: None,
        };
        self.dispatch(item, &recipe, None, dry_run)
    }

    /// Reopen an item's work when the item has moved under it
    /// (§FS-005-dispatch.5). Work whose item is unchanged is left alone.
    pub fn sync(&mut self, item: &Item, dry_run: bool) -> Result<Outcome> {
        self.synced(item, dry_run, false)
    }

    /// [`Dispatcher::sync`] as a sweep — `work sync`, whoever typed it: the
    /// same reading, except that where it would reopen work about a private
    /// matter it writes nothing and says so (§FS-018-private-sources.3). The
    /// key on the matter's own row is the person's and keeps
    /// [`Dispatcher::sync`].
    pub fn sync_swept(&mut self, item: &Item, dry_run: bool) -> Result<Outcome> {
        self.synced(item, dry_run, true)
    }

    /// Why no sweep may write work about this matter, where the site lists
    /// the source that reported it as private: the `private` hold, naming the
    /// source and no tickets, since the row is about a matter no ticket was
    /// written on (§FS-018-private-sources.3, §FS-005-dispatch.24.2). Never
    /// asked by a person's named move.
    pub fn swept_past(&self, item: &Item) -> Option<Hold> {
        private::listed(&self.global, &item.source).map(|source| Hold::Private {
            source: source.to_string(),
            tickets: Vec::new(),
        })
    }

    fn synced(&mut self, item: &Item, dry_run: bool, sweep: bool) -> Result<Outcome> {
        let Some(entry) = self.ledger.entries.get(&item.id) else {
            return Ok(Outcome::Current);
        };
        let changes = entry.changes_since(item);
        if changes.is_empty() {
            return Ok(Outcome::Current);
        }
        // Under the recipe that fits the item as it is now, preferring the one
        // last asked for while it still fits. What moved may have moved the
        // item out of its old category: a pull request whose gate went green
        // and whose author asked a question is not a red gate any more, and
        // reopening it as one would hand the work a ticket about a problem
        // that is no longer there.
        let last = entry
            .last()
            .map(|dispatch| dispatch.recipe.clone())
            .unwrap_or_default();
        let offers = self.offers(item);
        let Some(recipe) = offers
            .iter()
            .find(|candidate| candidate.id == last)
            .or_else(|| offers.first())
            .cloned()
        else {
            return Ok(Outcome::Dormant { changes });
        };
        // Asked only where there is something to reopen, so a private
        // matter that did not move is as quiet as any other.
        if let Some(hold) = self.swept_past(item).filter(|_| sweep) {
            return Ok(Outcome::PassedOver {
                recipe: recipe.id,
                hold,
            });
        }
        self.dispatch(item, &recipe, None, dry_run)
    }

    /// Take one of an item's tickets back (§FS-005-dispatch.16): the runtime's
    /// own move into the abandonment state, asked in its own words, refused
    /// beforehand on what ephor can see for itself. The ledger is untouched —
    /// it records what was asked, and this was asked.
    pub fn cancel(&self, item: &str, ticket: &str, why: &str, dry_run: bool) -> Result<Cancelled> {
        let entry = self.ledger.entries.get(item).ok_or_else(|| {
            EphorError::Command(format!(
                "{item} has no work to cancel — nothing was dispatched for it"
            ))
        })?;
        let dispatch = entry
            .dispatches
            .iter()
            .rev()
            .find(|dispatch| !dispatch.is_workflow() && dispatch.ticket == ticket);
        let root = dispatch
            .and_then(|dispatch| dispatch.root.as_ref())
            .unwrap_or(&entry.root);
        let plan = match dispatch.and_then(|dispatch| dispatch.root.as_ref()) {
            Some(_) => plan::plan_path_in(root, &entry.plan_id),
            None => entry.plan.clone(),
        };
        cancel_ticket(
            &self.global,
            root,
            &entry.plan_id,
            &plan,
            ticket,
            why,
            dry_run,
        )
    }

    /// The reply a run drafted about this matter and did not send, where one
    /// was drafted (§FS-005-dispatch.13). Attached to the matter here rather
    /// than stored on it: it is what the runtime left on disk, and reading it
    /// every time is what keeps ephor from reporting on itself
    /// (§FS-005-dispatch.4).
    pub fn proposal(&self, item: &Item) -> Option<runtime::results::Proposal> {
        let entry = self.ledger.entries.get(&item.id)?;
        Self::proposal_from(entry)
    }

    /// Re-read committed request provenance under the reply row lock, so a
    /// hand-off committed after session loading supersedes the old proposal
    /// (§FS-005-dispatch.4, §FS-005-dispatch.13).
    pub fn latest_proposal(item: &Item) -> Result<Option<runtime::results::Proposal>> {
        Ok(ledger::load()?
            .entries
            .get(&item.id)
            .and_then(Self::proposal_from))
    }

    /// Latest request wins even when its output is absent or withdrawn;
    /// legacy requests are readable but unbound (§FS-005-dispatch.13).
    fn proposal_from(entry: &Entry) -> Option<runtime::results::Proposal> {
        let latest = entry.dispatches.last()?;
        if let Some(path) = &latest.reply_path {
            return runtime::results::proposal_at(path.clone(), latest.reply_binding.clone());
        }
        recipe_roots(entry)
            .into_iter()
            .rev()
            .find_map(|root| runtime::results::proposal(&root, &entry.plan_id))
    }

    /// Record that this matter's proposed reply was posted, so it is offered
    /// once (§FS-005-dispatch.13).
    pub fn proposal_posted(&self, item: &Item) -> Result<()> {
        if let Some(proposal) = self.proposal(item) {
            return runtime::results::mark_path_posted(&proposal.path);
        }
        Ok(())
    }

    /// Everything one item's work root has to say about what is going there,
    /// read once (§FS-005-dispatch.15.1, §AR-005-capabilities.1).
    ///
    /// Found by looking, never remembered from the keypress: the ledger says
    /// which tickets this entry opened and which plans it laid, the plans say
    /// what state each is in, the machine says which of those states are over
    /// and which are questions for a person, and the lock says whether a run
    /// holds the root at all (§FS-005-dispatch.15).
    ///
    /// Read here rather than in [`WorkAt::going`] because a menu asks about
    /// every row of one subject, and every row of one subject shares this root:
    /// six recipe entries used to mean six states-document parses, six plan
    /// parses, a dozen lock probes and six descriptor reads for one keypress.
    /// [`WorkAt::going`] then answers each row off what is already in hand
    /// (§FS-005-dispatch.21).
    pub fn work_at(&self, item: &Item) -> Option<WorkAt<'_>> {
        let entry = self.ledger.entries.get(&item.id)?;
        let mut root_paths: Vec<PathBuf> = Vec::new();
        for dispatch in &entry.dispatches {
            let root = dispatch.root.as_ref().unwrap_or(&entry.root);
            if !root_paths
                .iter()
                .any(|seen| canonical(seen) == canonical(root))
            {
                root_paths.push(root.clone());
            }
        }
        if root_paths.is_empty() {
            root_paths.push(entry.root.clone());
        }
        let plans = recorded_recipe_plans(entry);
        let roots = root_paths
            .into_iter()
            .map(|root| {
                let live = runtime::watch::live(&self.global, &root);
                let plan = plans
                    .iter()
                    .find(|plan| canonical(&plan.root) == canonical(&root))
                    .and_then(|recorded| {
                        Plan::read(&recorded.path)
                            .ok()
                            .flatten()
                            .map(|plan| (recorded.path.clone(), plan))
                    });
                WorkAtRoot {
                    machine: WorkRoot::open(&root).ok().flatten(),
                    plan,
                    witness: live.then(|| runtime::watch::witness(&self.global, &root)),
                    lock_born: live.then(|| runtime::watch::lock_born(&root)).flatten(),
                    identity: live
                        .then(|| runtime::watch::identity(&self.global, &root))
                        .flatten(),
                    live,
                    root,
                }
            })
            .collect();
        Some(WorkAt { entry, roots })
    }

    /// What an item's work is doing, read from the plan.
    pub fn status(&self, item: &Item) -> Option<WorkStatus> {
        let entry = self.ledger.entries.get(&item.id)?;
        Some(self.status_of(entry, Some(item)))
    }

    /// The same reading, with the per-root probes shared across a run of
    /// items (§FS-005-dispatch.15.1): a caller building every matter's rows
    /// keeps one [`RootLook`] across them, so a root two matters share is
    /// probed once rather than twice.
    pub fn status_seen(&self, item: &Item, look: &mut RootLook) -> Option<WorkStatus> {
        let entry = self.ledger.entries.get(&item.id)?;
        Some(status_of_entry_seen(&self.global, entry, Some(item), look))
    }

    pub fn status_of(&self, entry: &Entry, item: Option<&Item>) -> WorkStatus {
        status_of_entry(&self.global, entry, item)
    }

    /// Every work root that should have a run and has none
    /// (§FS-005-dispatch.24).
    ///
    /// The whole of the sweep, and it reads the world rather than a memory of
    /// what was dispatched: the roots are the ones every other reading walks
    /// (§FS-005-dispatch.15), a ticket counts by what it is rather than by
    /// who appended it, and whether a run is already there is the runtime's
    /// own lock. So this is idempotent — running it twice in a second starts
    /// one run — and asks nothing about what it did last time, apart from the
    /// one thing it must remember: a root whose start failed rests before it
    /// is tried again.
    pub fn due(&mut self, now: DateTime<Utc>) -> Result<Vec<Due>> {
        let roots = self.work_roots();
        self.due_in(&roots, now, Reach::Sweep)
    }

    /// The plans a run asked for by name reaches: the matter's own and every
    /// one a workflow laid beside it, for the one matter a reader named or —
    /// with none — for every matter the record knows in the projects they
    /// asked about (§FS-005-dispatch.30).
    ///
    /// The sweep's own reading, narrowed. It is deliberately not
    /// [`Dispatcher::start_due`] narrowed: that function is the sweep's *act*,
    /// and the ceilings, the spend refusal, the back-off and the site-wide
    /// reservation live inside it. The key inherits none of them, and reusing
    /// the reading rather than the act is what makes that true by
    /// construction rather than by anyone remembering to exclude them
    /// (§FS-005-dispatch.30, §FS-005-dispatch.24).
    pub fn runnable_of(
        &mut self,
        item: Option<&str>,
        projects: &[String],
        now: DateTime<Utc>,
    ) -> Result<Vec<Due>> {
        let roots = self.work_roots();
        self.due_over(&roots, now, projects, Reach::Key(item))
    }

    /// Due roots from one discovered-root snapshot. `start_due` also derives
    /// its live counts from this same snapshot, so discovery cannot disagree
    /// with capacity accounting halfway through a sweep
    /// (§FS-005-dispatch.24).
    fn due_in(
        &mut self,
        roots: &[runtime::watch::RootPlans],
        now: DateTime<Utc>,
        reach: Reach<'_>,
    ) -> Result<Vec<Due>> {
        // What asked to run itself is the sweep's question alone: the key is
        // blind to `autorun`, so resolving it for the key would cost a summons
        // per project to answer nothing — and would make a start by name
        // depend on the runtime answering `templates`
        // (§FS-005-dispatch.30).
        if let Reach::Key(_) = reach {
            return self.reading(roots, &BTreeMap::new(), &BTreeMap::new(), now, reach);
        }
        let projects: BTreeSet<String> = roots
            .iter()
            .flat_map(|group| group.plans.iter().map(|plan| plan.project.clone()))
            .collect();
        // Which recipes asked to run themselves, per project. Resolved once
        // per project: the tables behind it are the same for every root of
        // one of them.
        let autoruns: BTreeMap<String, BTreeSet<String>> = projects
            .iter()
            .map(|project| {
                let asked = self
                    .recipes(project)
                    .into_iter()
                    .filter(|recipe| recipe.autorun)
                    .map(|recipe| recipe.id)
                    .collect();
                (project.clone(), asked)
            })
            .collect();
        // And which workflow entries did (§FS-005-dispatch.28). Only for a
        // project whose record holds a laying: one of the three homes is
        // beside the workflow itself, so resolving this asks the runtime what
        // it offers — a summons per project, and a sweep on a site that has
        // never laid a workflow has no laid plan for an entry to answer for.
        let laying: BTreeSet<&str> = self
            .ledger
            .entries
            .values()
            .filter(|entry| entry.dispatches.iter().any(Dispatch::is_workflow))
            .map(|entry| entry.project.as_str())
            .collect();
        let asking: Vec<String> = projects
            .iter()
            .filter(|project| laying.contains(project.as_str()))
            .cloned()
            .collect();
        let workflow_autoruns: BTreeMap<String, BTreeSet<String>> = asking
            .into_iter()
            .map(|project| {
                self.autorun_workflows(&project)
                    .map(|asked| (project, asked))
            })
            .collect::<Result<_>>()?;
        self.reading(roots, &autoruns, &workflow_autoruns, now, reach)
    }

    /// The reading itself, once the autorun sets are settled: shared so that
    /// the sweep and the key rank what they found the same way
    /// (§AR-009-surfaces.1).
    fn reading(
        &mut self,
        roots: &[runtime::watch::RootPlans],
        autoruns: &BTreeMap<String, BTreeSet<String>>,
        workflow_autoruns: &BTreeMap<String, BTreeSet<String>>,
        now: DateTime<Utc>,
        reach: Reach<'_>,
    ) -> Result<Vec<Due>> {
        let mut due = due_among(
            &self.global,
            roots,
            autoruns,
            workflow_autoruns,
            &self.ledger,
            now,
            reach,
        );
        if let Some(path) = self.global.ranking.as_deref() {
            let reading = ranking::read(&crate::paths::resolve_path(path));
            due = rank_due(due, &reading.order);
        }
        Ok(due)
    }
}

/// Who is asking for this reading (§FS-005-dispatch.30).
///
/// The sweep and the key ask one question — which plans hold a task a run
/// would advance — and differ only in the guards that are there because the
/// sweep has nobody present: what asked to autorun, the failed-start back-off,
/// and the silent drop of a root its own run holds. Threading this rather than
/// writing a second reading is what keeps the two surfaces from drifting into
/// two answers about one matter's work.
#[derive(Debug, Clone, Copy)]
pub enum Reach<'a> {
    /// The sweep behind autorun, with nobody present (§FS-005-dispatch.24).
    Sweep,
    /// The key. `Some(id)` is the one matter a reader named; `None` is every
    /// matter the record knows in the projects they asked about
    /// (§FS-005-dispatch.30).
    Key(Option<&'a str>),
}

/// The sweep's own reading, with the roots and the recipes already gathered
/// (§FS-005-dispatch.24). A free function so a test can ask the question the
/// way every surface does, without a registry behind it — the shape
/// [`status_of_entry`] and [`enumerate_roots`] already take.
pub fn due_among(
    global: &WorkConfig,
    roots: &[runtime::watch::RootPlans],
    autoruns: &BTreeMap<String, BTreeSet<String>>,
    workflow_autoruns: &BTreeMap<String, BTreeSet<String>>,
    ledger: &Ledger,
    now: DateTime<Utc>,
    // Which reader this reading is for (§FS-005-dispatch.30). Three guards
    // below read it, and they are the three that are there because the sweep
    // has nobody present.
    reach: Reach<'_>,
) -> Vec<Due> {
    // A reader who typed a matter's name is present and is deciding, which is
    // the whole of the difference between the two readers
    // (§FS-005-dispatch.30).
    let key = matches!(reach, Reach::Key(_));
    // Whose a matter is draws the line elsewhere: a run that names no matter
    // is a sweep for a private one, whoever typed it, so only a run naming
    // the matter may start its work (§FS-018-private-sources.3).
    let named = matches!(reach, Reach::Key(Some(_)));
    // What ephor dispatched, so a ticket it wrote is judged by the recipe it
    // was written from rather than by the shape of its id.
    let dispatched: BTreeMap<(PathBuf, String), String> = ledger
        .entries
        .values()
        .flat_map(|entry| {
            entry.dispatches.iter().map(move |dispatch| {
                (
                    (
                        canonical(dispatch.root.as_ref().unwrap_or(&entry.root)),
                        dispatch.ticket.clone(),
                    ),
                    dispatch.recipe.clone(),
                )
            })
        })
        .collect();
    // And which entry laid each plan a workflow wrote (§FS-005-dispatch.28).
    // The record is the only thing that knows: the plan is the runtime's and
    // says nothing about who asked for it
    // (§FS-005-dispatch.4). Keyed by the plan itself, because the ledger
    // names the directory the runtime was pointed at and what landed inside
    // it is the runtime's answer (§AR-007-runtime.1).
    let laid_by: BTreeMap<PathBuf, String> = ledger
        .entries
        .values()
        .flat_map(|entry| {
            entry
                .dispatches
                .iter()
                .filter(|dispatch| dispatch.is_workflow())
                .filter_map(move |dispatch| {
                    let plan_id = dispatch.plan.as_deref()?;
                    let root = dispatch.root.as_ref().unwrap_or(&entry.root);
                    let found = runtime::workflow::laid(&root.join(plan_id))?;
                    Some((canonical(&found.path), dispatch.recipe.clone()))
                })
        })
        .collect();
    // Which trees are being worked, taken over the whole snapshot before
    // anything here is judged due (§FS-005-dispatch.24). The run that holds a
    // tree usually holds no due ticket of its own, so this cannot be folded
    // into the loop below.
    let busy = live_checkouts(global, roots, ledger);
    let nothing: BTreeSet<String> = BTreeSet::new();
    let mut due = Vec::new();
    for group in roots {
        // A root whose own run is live is left alone here and said nothing
        // about: the runtime schedules one run per root, that run reaches
        // every ticket written beneath it, and a sweep reporting on it would
        // repeat the ordinary case for as long as the work takes
        // (§FS-005-dispatch.24). A tree *another* root's run holds is a
        // different matter and is answered below, where the root has been
        // judged due and there is something to pass over.
        //
        // Not for the key: a reader who asked for a run by name is owed the
        // refusal that names the run in the way, with `--force` to lift it,
        // rather than being told the matter holds nothing. So the row comes
        // back and the surface refuses on it (§FS-005-dispatch.30).
        let held_by = holding(&busy, &checkout_of(ledger, &group.root));
        if !key && held_by.is_some_and(|held_by| held_by == &group.root) {
            continue;
        }
        // A root that could not be started is passed over for a while,
        // longer each time — a runner that refuses must not turn every
        // sweep into a spawn. A rule about a decision nobody is watching, so
        // a reader who asks for the root now gets it now
        // (§FS-005-dispatch.30).
        if !key
            && ledger
                .starts
                .get(&root_key(&group.root))
                .is_some_and(|start| start.resting(now))
        {
            continue;
        }
        // Finality and gating are the machine's words. With none to say
        // them, nothing here can be judged runnable, and the honest move
        // is to start nothing rather than to guess (§FS-005-dispatch.15).
        //
        // The sweep drops such a root in silence. The key may not: a reader
        // who named this matter would otherwise be told its work holds
        // nothing, which is this point's own fault one layer down. So the row
        // comes back carrying the reason and the surface refuses on it
        // (§FS-005-dispatch.30).
        let Some(machine) = WorkRoot::open(&group.root).ok().flatten() else {
            if names_a_plan_here(group, reach) {
                due.push(refused_root(ledger, group, no_machine_here(&group.root)));
            }
            continue;
        };
        let mut plans: Vec<String> = Vec::new();
        let mut tickets: Vec<String> = Vec::new();
        let mut items: Vec<String> = Vec::new();
        let mut projects: BTreeSet<String> = BTreeSet::new();
        // What a gate holds instead, kept apart: a root left with nothing but
        // this waits on a person, and its row names the gates rather than the
        // work they hold (§FS-005-dispatch.24.3.2).
        let mut waiting = Waiting::default();
        // And what no sweep may start, because it is about a matter a source
        // the site lists as private reported (§FS-018-private-sources.3).
        let mut kept = Kept::default();
        for plan_ref in &group.plans {
            // Which entry laid this plan down, where a workflow did — and so
            // which set of "asked to run itself" answers for what is inside
            // it (§FS-005-dispatch.28). A plan the record knows nothing
            // about asked for nothing, and is nobody's to start.
            let laid = laid_by.get(&canonical(&plan_ref.path)).map(String::as_str);
            // For a store of its own, the entry is the only way that can be
            // said: its tasks are the runtime's own, and the `<recipe>-<n>`
            // shape below is a fact about the tickets ephor wrote into the
            // root's own plan, so no spelling in there names a recipe
            // (§FS-005-dispatch.28).
            if laid.is_none() && runtime::plan::own_store(&plan_ref.path).is_some() {
                continue;
            }
            // What the key reaches is what the record says is a matter's
            // work, and nothing else: [`enumerate_roots`] writes the matter
            // onto every plan of a ledger entry — the matter's own and each
            // one a workflow laid — and onto none it merely found in a root.
            // So a plan the record can name no matter for is nobody's to
            // start by name, and where a reader named one it is that matter's
            // plans alone (§FS-005-dispatch.30, §FS-005-dispatch.28).
            if let Reach::Key(named) = reach {
                let Some(about) = plan_ref.item.as_deref() else {
                    continue;
                };
                if named.is_some_and(|named| named != about) {
                    continue;
                }
            }
            let asked = match laid {
                Some(_) => workflow_autoruns.get(&plan_ref.project),
                None => autoruns.get(&plan_ref.project),
            }
            .unwrap_or(&nothing);
            // `autorun` is the condition under which work starts with nobody
            // present, so its silence means the key and the key is blind to
            // it (§FS-005-dispatch.30, §FS-005-dispatch.24).
            if !key && asked.is_empty() {
                continue;
            }
            // A plan that is a store of its own runs under the machine it
            // declares there, not the root's: a task's state means whatever
            // the machine in force for its own store says it means
            // (§FS-006-project-interface.7) — which is a plan a workflow laid
            // down, and the same question the board asks of the same plan
            // (§AR-009-surfaces.1). Declaring none, it is the root's the
            // runtime resolves it against, as it is for the plans the root
            // holds directly. With one that will not read, nothing in that
            // plan can be judged runnable, and the honest move is to start
            // nothing rather than to fall back on a machine that answers for
            // other work (§FS-005-dispatch.15).
            let own = match runtime::plan::own_machine(&plan_ref.path) {
                Ok(store) => store,
                Err(_) => continue,
            };
            let judge = own.as_ref().unwrap_or(&machine);
            let Ok(Some(plan)) = Plan::read(&plan_ref.path) else {
                continue;
            };
            let plan_tickets = plan.tickets();
            // This plan's would-be-due tickets, held here until the plan is
            // known to be about no private matter: a run is narrowed to whole
            // plans, so a plan holding one is never handed to a sweep's run,
            // whatever else is in it (§FS-005-dispatch.24).
            let mut ready: Vec<(String, String)> = Vec::new();
            let mut whose: Option<&str> = None;
            // Every open ticket in a gating state here, by the machine in
            // force for this plan and nothing else — not the last run's
            // stream, not its report, not a file's time. Each holds its own
            // top-level tree and no other (§FS-005-dispatch.24.3.1). Never
            // for the key, which is blind to the wait (§FS-005-dispatch.24.3.3).
            let gates: Vec<(&str, &str)> = match key {
                true => Vec::new(),
                false => plan_tickets
                    .iter()
                    .filter_map(|ticket| {
                        let state = ticket.state.as_deref()?;
                        (judge.is_gating(state) && !judge.is_final(state))
                            .then_some((ticket.id.as_str(), state))
                    })
                    .collect(),
            };
            for ticket in plan_tickets.iter() {
                let state = ticket.state.as_deref();
                // Over, waiting on a person, or somebody's to move: none
                // of them is work a run would advance
                // (§FS-005-dispatch.24, §FS-005-dispatch.15).
                if state.map(|state| judge.is_final(state)).unwrap_or(true)
                    || state.map(|state| judge.is_gating(state)).unwrap_or(false)
                    || ticket.assignee.is_some()
                {
                    continue;
                }
                // What asked for this work. For a plan a workflow laid down
                // it is the entry that laid it, and it answers for every
                // task in the plan — the tasks are the workflow's, not
                // ephor's, and no id of theirs names a recipe
                // (§FS-005-dispatch.28). Otherwise the ledger says which
                // recipe ephor wrote a ticket from; for one a hand appended,
                // the id says it, because ids are `<recipe>-<n>` by
                // construction. Either way it is a fact about the ticket
                // (§FS-005-dispatch.24).
                //
                // The key does not ask: it is blind to `autorun`, and the
                // whole of which plans are its own was settled above by the
                // matter the record names (§FS-005-dispatch.30).
                if !key {
                    let asked_for = laid.or_else(|| {
                        dispatched
                            .get(&(canonical(&group.root), ticket.id.clone()))
                            .map(String::as_str)
                            .or_else(|| recipe_of_ticket(&ticket.id))
                    });
                    if !asked_for.is_some_and(|what| asked.contains(what)) {
                        continue;
                    }
                }
                // Would be due, but it is about a private matter, and that is
                // asked first: no sweep could wait for anything that lifts it,
                // and a plain run is one. Whose the ticket is, is what the
                // ticket records (§FS-018-private-sources.3, §FS-005-dispatch.8).
                if !named {
                    if let Some(source) =
                        private::of_ticket(global, plan_ref, laid.is_some(), &plan, &ticket.id)
                    {
                        whose = whose.or(Some(source));
                        ready.push((ticket.id.clone(), state.unwrap_or_default().to_string()));
                        continue;
                    }
                }
                // Would be due, but a gate in its own tree holds it: the run
                // would halt at that gate having done nothing
                // (§FS-005-dispatch.24.3.1).
                let tree = top_level(&ticket.id);
                let holding: Vec<&(&str, &str)> = gates
                    .iter()
                    .filter(|(gated, _)| top_level(gated) == tree)
                    .collect();
                if !holding.is_empty() {
                    waiting.hold(plan_ref, &holding);
                    continue;
                }
                ready.push((ticket.id.clone(), state.unwrap_or_default().to_string()));
            }
            // A plan about a private matter keeps every ticket it would have
            // started; the rest of the root is due without it
            // (§FS-018-private-sources.3, §FS-005-dispatch.24).
            if let Some(source) = whose {
                kept.keep(plan_ref, source, ready);
                continue;
            }
            for (ticket, _) in ready {
                if !plans.contains(&plan_ref.plan_id) {
                    plans.push(plan_ref.plan_id.clone());
                }
                projects.insert(plan_ref.project.clone());
                if let Some(item) = &plan_ref.item {
                    if !items.contains(item) {
                        items.push(item.clone());
                    }
                }
                tickets.push(format!("{}.{ticket}", plan_ref.plan_id));
            }
        }
        if tickets.is_empty() && waiting.tickets.is_empty() && kept.tickets.is_empty() {
            continue;
        }
        // Where the run is made from, and whether it may be made there at
        // all: work about a branch belongs in that branch's working tree,
        // and a tree standing on another branch holds different code
        // (§FS-005-dispatch.3). Dispatch refuses on this and so does a
        // start, because with nobody watching there is no one to notice
        // (§FS-005-dispatch.24).
        let known = ledger.entries.values().find(|entry| {
            entry.dispatches.iter().any(|dispatch| {
                canonical(dispatch.root.as_ref().unwrap_or(&entry.root)) == canonical(&group.root)
            }) || (entry.dispatches.is_empty() && canonical(&entry.root) == canonical(&group.root))
        });
        let checkout = checkout_of(ledger, &group.root);
        // Where a run may not be made here at all, and why. The key is told
        // rather than dropped, for the reason the machine guard above is
        // (§FS-005-dispatch.30).
        let mut refusal = None;
        if let Some(wanted) = branch_of(ledger, &group.root) {
            // Only a branch that can be read and disagrees refuses: an
            // unreadable or detached HEAD is a fact nobody can establish,
            // and refusing on one is worse than the run — the same
            // latitude dispatch takes (§FS-005-dispatch.3).
            if let Some(head) = crate::git::head_branch(&checkout).filter(|head| head != wanted) {
                if !key {
                    continue;
                }
                refusal = Some(checkout_standing_elsewhere(&checkout, &head, wanted));
            }
        }
        // And what the last run here actually did. The start worked, so the
        // record above never saw it, and every sweep for as long as the
        // tickets stay open made another run that did the same
        // (§FS-005-dispatch.24). A root this rests is marked rather than
        // dropped, so the sweep can say so in the row where it used to say
        // *started* — which is the reading that was wrong.
        //
        // The sweep's alone, like the three guards above. The rest binds the
        // sweep and never a run asked for by name, so a root the sweep is
        // resting on is started at once for the reader who typed its matter,
        // and no verdict is owed about a run they are asking after
        // (§FS-005-dispatch.24, §FS-005-dispatch.30).
        //
        // A root waiting on a person is never started, so there is no run to
        // judge, and finding it so drops what was remembered against it and
        // marks the run there as read (§FS-005-dispatch.24.3.3). A mixed root is due for its ready work
        // alone and judged as ever (§FS-005-dispatch.24.3.2).
        //
        // A root left with nothing but private work is held first, and no run
        // was started on it for a verdict to judge (§FS-005-dispatch.24.2).
        let private = match (tickets.is_empty(), kept.source.clone()) {
            (true, Some(source)) => Some(Hold::Private {
                source,
                tickets: kept.tickets.clone(),
            }),
            _ => None,
        };
        let person = (tickets.is_empty() && private.is_none()).then(|| Hold::Person {
            tickets: waiting.tickets.clone(),
        });
        let (verdict, rested) = match (key, &private, &person) {
            (_, Some(_), _) => (None, None),
            (_, None, Some(_)) => (Some(Verdict::Waiting), None),
            (true, None, None) => (None, None),
            (false, None, None) => judge(ledger, &group.root, now),
        };
        // The row of a root held names what holds it, and is attributed to
        // the plans that hold it (§FS-005-dispatch.24.3.2,
        // §FS-005-dispatch.24.2).
        let held = match (&private, &person) {
            (Some(_), _) => Some((kept.plans, kept.tickets, kept.items, kept.projects)),
            (None, Some(_)) => Some((
                waiting.plans,
                waiting.tickets,
                waiting.items,
                waiting.projects,
            )),
            (None, None) => None,
        };
        if let Some((held_plans, held_tickets, held_items, held_projects)) = held {
            plans = held_plans;
            tickets = held_tickets.into_iter().map(|(ticket, _)| ticket).collect();
            items = held_items;
            projects = held_projects;
        }
        due.push(Due {
            project: group
                .plans
                .first()
                .map(|plan| plan.project.clone())
                .unwrap_or_default(),
            // Attribute only plans that contributed a runnable ticket to this
            // due root. Recorded historical placements therefore retain their
            // project, while unreadable, stale, gated, or copied plans do not
            // enter capacity or spend policy (§FS-005-dispatch.15.1,
            // §FS-005-dispatch.30, §FS-015-spend-ceiling.6).
            projects: projects.into_iter().collect(),
            root: group.root.clone(),
            checkout,
            plans,
            tickets,
            item: items.first().cloned().or_else(|| {
                known.and_then(|entry| {
                    ledger
                        .entries
                        .iter()
                        .find(|(_, candidate)| std::ptr::eq(*candidate, entry))
                        .map(|(id, _)| id.clone())
                })
            }),
            items,
            // Due, and held back by a run in another root over this same
            // working tree: the row is kept so the sweep can pass it over
            // with the run in the way, rather than dropping it where a reader
            // would see an empty sweep and go looking for a full ceiling
            // (§FS-005-dispatch.24). Nothing is started on it.
            held_by: held_by.cloned(),
            refusal,
            // Read last, and only for a root that is actually due: the
            // witness is one file per root, and a root with nothing to run is
            // not a root a verdict is owed about (§FS-005-dispatch.24).
            private,
            excluded: None,
            person,
            rested,
            verdict,
        });
    }
    due
}

/// What the private sources keep out of one root's due reading, gathered so
/// the root can be passed over naming them where nothing else there is due
/// (§FS-018-private-sources.3, §FS-005-dispatch.24.2).
#[derive(Default)]
struct Kept {
    /// The first listed source a kept ticket is about, as the site lists it.
    source: Option<String>,
    /// Each ticket a sweep would have started, plan-qualified, with the state
    /// it sits in.
    tickets: Vec<(String, String)>,
    plans: Vec<String>,
    items: Vec<String>,
    projects: BTreeSet<String>,
}

impl Kept {
    /// One plan about a private matter `source` reported, with the tickets it
    /// would have made due.
    fn keep(
        &mut self,
        plan_ref: &runtime::watch::PlanRef,
        source: &str,
        ready: Vec<(String, String)>,
    ) {
        self.source.get_or_insert_with(|| source.to_string());
        for (ticket, state) in ready {
            self.tickets
                .push((format!("{}.{ticket}", plan_ref.plan_id), state));
        }
        if !self.plans.contains(&plan_ref.plan_id) {
            self.plans.push(plan_ref.plan_id.clone());
        }
        if let Some(item) = &plan_ref.item {
            if !self.items.contains(item) {
                self.items.push(item.clone());
            }
        }
        self.projects.insert(plan_ref.project.clone());
    }
}

/// What the gates in one root hold out of the due reading, gathered so the
/// root can be passed over naming them where nothing else there is due
/// (§FS-005-dispatch.24.3.2).
#[derive(Default)]
struct Waiting {
    /// Each gated ticket that holds a would-be-due ticket, plan-qualified,
    /// with the gating state it sits in; each named once.
    tickets: Vec<(String, String)>,
    plans: Vec<String>,
    items: Vec<String>,
    projects: BTreeSet<String>,
}

impl Waiting {
    /// One would-be-due ticket in `plan_ref`, held by `gates` in its tree.
    fn hold(&mut self, plan_ref: &runtime::watch::PlanRef, gates: &[&(&str, &str)]) {
        for (gated, state) in gates {
            let qualified = format!("{}.{gated}", plan_ref.plan_id);
            if !self.tickets.iter().any(|(ticket, _)| ticket == &qualified) {
                self.tickets.push((qualified, state.to_string()));
            }
        }
        if !self.plans.contains(&plan_ref.plan_id) {
            self.plans.push(plan_ref.plan_id.clone());
        }
        if let Some(item) = &plan_ref.item {
            if !self.items.contains(item) {
                self.items.push(item.clone());
            }
        }
        self.projects.insert(plan_ref.project.clone());
    }
}

/// The top-level ticket a ticket's tree hangs from: its id up to the first
/// `.`, so `fix-gate-1.triage` is in `fix-gate-1`'s tree and `fix-gate-10`
/// is a tree of its own (§FS-005-dispatch.24.3.1).
fn top_level(id: &str) -> &str {
    id.split('.').next().unwrap_or(id)
}

/// Whether this reading's reader has a plan of their own in this root — the
/// question asked of a root nothing can be read out of, where the plans
/// cannot be walked for an answer (§FS-005-dispatch.30).
///
/// A plan's matter is what [`enumerate_roots`] wrote onto it from the ledger,
/// so it is readable without the machine, without the plan file, and without
/// the record being asked twice. False for the sweep, which drops such a root
/// in silence and has nobody to say anything to.
fn names_a_plan_here(group: &runtime::watch::RootPlans, reach: Reach<'_>) -> bool {
    let Reach::Key(named) = reach else {
        return false;
    };
    group.plans.iter().any(|plan_ref| {
        plan_ref
            .item
            .as_deref()
            .is_some_and(|about| named.is_none_or(|named| named == about))
    })
}

/// A row for a root the key may start nothing in, carrying why
/// (§FS-005-dispatch.30). No plans and no tickets: there is nothing here to
/// point a run at, and the row exists so the refusal names the root.
fn refused_root(ledger: &Ledger, group: &runtime::watch::RootPlans, says: String) -> Due {
    let item = group.plans.iter().find_map(|plan| plan.item.clone());
    Due {
        project: group
            .plans
            .first()
            .map(|plan| plan.project.clone())
            .unwrap_or_default(),
        projects: group
            .plans
            .iter()
            .map(|plan| plan.project.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        root: group.root.clone(),
        checkout: checkout_of(ledger, &group.root),
        plans: Vec::new(),
        tickets: Vec::new(),
        items: item.iter().cloned().collect(),
        item,
        held_by: None,
        refusal: Some(says),
        // A root the key may start nothing in is not a root the sweep is
        // resting or the reader excluded: this row exists to carry the
        // refusal and nothing else (§FS-005-dispatch.24,
        // §FS-005-dispatch.30).
        private: None,
        excluded: None,
        person: None,
        rested: None,
        verdict: None,
    }
}

/// What this sweep read of the last run on one root, and whether that leaves
/// the root alone (§FS-005-dispatch.24).
///
/// The witness is the finished run's own record of itself and nothing else
/// (§FS-005-dispatch.15.2): a root the sweep is considering has no live run,
/// so the stream there is the last run's and is over. Where there is none,
/// where its layout this reader cannot read, or where it never said which run
/// it was — without which one run could be counted twice — no verdict is
/// taken at all and the sweep behaves exactly as it did before this rule
/// (§AR-007-runtime.3).
fn judge(
    ledger: &Ledger,
    root: &std::path::Path,
    now: DateTime<Utc>,
) -> (Option<Verdict>, Option<Hold>) {
    let Some(last) = runtime::events::progress(root) else {
        return (None, None);
    };
    let Some(run) = last.id else {
        return (None, None);
    };
    let kept = ledger.advances.get(&root_key(root));
    match last.advanced {
        // Dropped whole, at once: a root that moved deserves its next run
        // now rather than at the end of an interval it no longer owes.
        runtime::events::Advance::Advanced => (Some(Verdict::Advanced), None),
        // Unrecognized may never mean stuck: no root rested, no miss
        // counted, and whatever was remembered left exactly as it was.
        runtime::events::Advance::Unknown => (None, None),
        runtime::events::Advance::Nothing => {
            let judged = match kept {
                // The same run is never judged twice, however often the
                // sweep runs: the verdict stands as it was taken, and the
                // rest is still dated from then.
                Some(kept) if kept.run == run => kept.clone(),
                Some(kept) => Judged {
                    run,
                    misses: kept.misses.saturating_add(1),
                    at: now,
                },
                // The last run before the root waited on a person was read
                // then, and halted at the gate: it is known, never a miss
                // (§FS-005-dispatch.24.3.3).
                None if ledger.waited.contains(&root_key(root)) => Judged {
                    run,
                    misses: 0,
                    at: now,
                },
                None => Judged {
                    run,
                    misses: 1,
                    at: now,
                },
            };
            let why = rests(&judged, now);
            (Some(Verdict::Nothing(judged)), why)
        }
    }
}

/// Why a root whose runs advance nothing gets no run this sweep, where it
/// gets none (§FS-005-dispatch.24).
///
/// Louder than what it replaces, deliberately: the complaint this rule
/// answers is that a row saying `started` every time hid a stalled root, and
/// a rest that merely went quiet would hide it again. So the reason names the
/// run it judged, so the reader can go and read it, and says when the root is
/// tried again — or that it is now theirs. The two are two kinds of hold,
/// because they promise different things to whoever reads them
/// (§FS-005-dispatch.24.2).
fn rests(judged: &Judged, now: DateTime<Utc>) -> Option<Hold> {
    if judged.held() {
        return Some(Hold::Stopped {
            run: judged.run.clone(),
            count: judged.misses,
        });
    }
    judged.resting(now).then(|| Hold::Rested {
        run: judged.run.clone(),
        count: judged.misses,
        until: judged.ready_at(),
        left: in_a_while(judged.ready_at(), now),
    })
}

/// How long is left of a rest, for the reader of one row
/// (§FS-005-dispatch.24).
///
/// The feed's own interval is what this is, with the last minute said in
/// words: that renderer answers *now* under a minute, which reads as *tried
/// again in now* in a sentence that is about a moment still to come.
fn in_a_while(ready_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    match (ready_at - now).num_minutes() < 1 {
        true => "under a minute".to_string(),
        false => crate::feed::render::age(ready_at, now),
    }
}

/// What a sweep read of the last run on one root — what an acting sweep
/// writes into its own record, and a report held at the gate does not
/// (§FS-005-dispatch.24, §FS-011-command-line.10).
///
/// Carried on the row rather than written where it is read, because reading
/// is [`due_among`]'s and writing is the act's, and a dry run that wrote one
/// would be a dry run that was not dry.
#[derive(Debug, Clone)]
pub enum Verdict {
    /// The run there advanced something: whatever was remembered is dropped.
    Advanced,
    /// It advanced nothing: this is the record to keep.
    Nothing(Judged),
    /// No run is owed here: the root waits on a person. Whatever was
    /// remembered is dropped, a rest or a stop alike, and only a mark that the
    /// root waited is kept in its place (§FS-005-dispatch.24.3.3).
    Waiting,
}

/// The work roots the reader told one sweep to leave alone
/// (§FS-005-dispatch.24), each with the value they named it by.
///
/// No judgement of ephor's own is in here: it is for the driver that has
/// worked out for itself which root is stuck. Which is why the sweep names
/// the instruction back — a flag that changed what happened and said nothing
/// is indistinguishable from one that did not bind at all.
#[derive(Debug, Default, Clone)]
pub struct Excluded(Vec<(PathBuf, String)>);

impl Excluded {
    /// Each root a value named, with the value itself. Resolving the value is
    /// the command line's (§FS-011-command-line.9); what reaches the sweep is
    /// a root that exists.
    pub fn of(named: Vec<(PathBuf, String)>) -> Excluded {
        Excluded(named)
    }

    /// Mark every root the reader excluded, rather than dropping it: a row
    /// that is not there is a sweep that quietly did less
    /// (§FS-005-dispatch.24).
    pub fn mark(&self, due: Vec<Due>) -> Vec<Due> {
        if self.0.is_empty() {
            return due;
        }
        due.into_iter()
            .map(|mut root| {
                root.excluded = self
                    .0
                    .iter()
                    .find(|(path, _)| canonical(path) == canonical(&root.root))
                    // The value as the reader gave it (§FS-005-dispatch.24.2).
                    .map(|(_, named)| Hold::Excluded {
                        except: named.clone(),
                    });
                root
            })
            .collect()
    }
}

/// Ranked roots first, then every root the file did not distinguish in its
/// existing deterministic order. A root with no matter id is necessarily in
/// the latter group (§FS-005-dispatch.24, §FS-005-dispatch.26).
fn rank_due(due: Vec<Due>, ranked_ids: &[String]) -> Vec<Due> {
    let ranks: BTreeMap<&str, usize> = ranked_ids
        .iter()
        .enumerate()
        .rev()
        .map(|(rank, id)| (id.as_str(), rank))
        .collect();
    let mut due = due;
    due.sort_by(|left, right| {
        let rank = |root: &Due| {
            root.items
                .iter()
                .filter_map(|item| ranks.get(item.as_str()).copied())
                .min()
        };
        rank(left)
            .unwrap_or(usize::MAX)
            .cmp(&rank(right).unwrap_or(usize::MAX))
            .then_with(|| left.root.cmp(&right.root))
    });
    due
}

/// Serialize autorun capacity decisions across every ephor process using this
/// site's state directory. The operating system releases the lock if a sweep
/// exits, so no reservation can outlive the process that made it.
fn autorun_lock() -> Result<fs::File> {
    let state = crate::paths::state_dir();
    fs::create_dir_all(&state).map_err(|err| {
        EphorError::Command(format!(
            "Cannot create the autorun state directory {}: {err}",
            state.display()
        ))
    })?;
    let path = state.join("work.autorun.lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|err| {
            EphorError::Command(format!(
                "Cannot open the autorun capacity lock {}: {err}",
                path.display()
            ))
        })?;
    lock.lock().map_err(|err| {
        EphorError::Command(format!(
            "Cannot take the autorun capacity lock {}: {err}",
            path.display()
        ))
    })?;
    Ok(lock)
}

impl Dispatcher {
    /// Which of this root's laid plans need several pools at once that this
    /// site cannot have together, by plan id, with the entry that laid each
    /// (§FS-005-dispatch.33).
    ///
    /// Read from ephor's own record of the laying rather than from the entry:
    /// a sweep decides about plan roots found on disk and never sees what laid
    /// them (§FS-005-dispatch.4). A record written before ephor knew to write
    /// the requirement carries none and starts as it always did, which is what
    /// growing an interface by addition means
    /// (§FS-006-project-interface.11).
    fn held_plans(
        &self,
        root: &Due,
        evidence: &headroom::Evidence,
    ) -> BTreeMap<String, (String, headroom::Held)> {
        let mut held = BTreeMap::new();
        for entry in self.ledger.entries.values() {
            for dispatch in &entry.dispatches {
                let Some(plan) = dispatch.plan.as_deref() else {
                    continue;
                };
                if !root.plans.iter().any(|named| named == plan) {
                    continue;
                }
                if canonical(dispatch.root.as_ref().unwrap_or(&entry.root)) != canonical(&root.root)
                {
                    continue;
                }
                if let Some(why) = evidence.held(&dispatch.pools) {
                    held.insert(plan.to_string(), (dispatch.recipe.clone(), why));
                }
            }
        }
        held
    }

    /// What a reader who named this work should be told about the pools it
    /// needs, where one of them cannot be had (§FS-005-dispatch.33).
    ///
    /// A warning and never a hold: the plan is in front of them and already
    /// laid, so holding it would leave no way to run such a plan at all, and
    /// the key reaches the whole of the matter's work
    /// (§FS-005-dispatch.30). Only the sweep nobody typed is held.
    pub fn held_warnings(&self, due: &[Due], now: DateTime<Utc>) -> Vec<String> {
        let evidence = headroom::Evidence::read(&self.global, &self.ledger, now);
        due.iter()
            .flat_map(|root| self.held_plans(root, &evidence).into_values())
            .map(|(entry, held)| {
                format!(
                    "the plan '{entry}' laid {} — it is being started all the same",
                    held.clause
                )
            })
            .collect()
    }

    /// Start a run on every root the sweep says is due, and say what each
    /// came to (§FS-005-dispatch.24).
    ///
    /// The one implementation of autorun's *act*, so the timer, the command,
    /// and the dispatch that just wrote a ticket all start runs the same way
    /// and cannot drift into three of them (§AR-009-surfaces.1). Runs go
    /// beneath the screen and only beneath it: a sweep has nobody at a
    /// terminal by definition, so where the binding has no detached shape
    /// this starts nothing and says so rather than seizing the terminal of
    /// whatever invoked it (§FS-005-dispatch.24, §FS-005-dispatch.20).
    pub fn start_due(
        &mut self,
        now: DateTime<Utc>,
        projects: &[String],
        runner_args: &[String],
        max_concurrent: Option<usize>,
        excluded: &Excluded,
        budget: spend::Budget,
    ) -> Result<Sweep> {
        // The runtime is a rung like any other capacity, and with nothing
        // bound there is no run to start (§AR-005-capabilities.2).
        if runtime::refusal(&self.global).is_some() {
            return Ok(Sweep::default());
        }
        let detaches = runtime::can_detach(&self.global);
        // Everything after this point is one site-wide reservation: reload
        // the record another sweep may just have changed, take one fresh root
        // and liveness snapshot, decide capacity, and keep the lock until the
        // launches and their failed-start back-off are committed.
        let _autorun = autorun_lock()?;
        self.ledger = ledger::load()?;
        // The record just reloaded may still be named before the digest — this
        // sweep's own process may never have read it, and a sweep that starts
        // runs is entitled to write in the roots it starts them in
        // (§FS-005-dispatch.3.1). A root it cannot carry over is said and
        // passed over: a sweep is about no one matter, so nothing here is a
        // reason to leave the rest of the site unswept.
        self.carry_over_before_writing(None)?;
        // What the budgets have spent, read once for the whole sweep: the
        // answer is about the window rather than about any one root
        // (§FS-015-spend-ceiling.1).
        let budgets = self.budgets(now);
        let roots = self.work_roots();
        let ceilings = Ceilings {
            // `--max-concurrent N` replaces the configured aggregate ceiling
            // on roots in flight and that one alone; the working ceiling has
            // no flag (§FS-005-dispatch.24).
            site: Limits {
                concurrent: max_concurrent.or(self.global.max_concurrent),
                active: self.global.max_active,
            },
            site_as_configured: self.global.max_concurrent,
            organizations: self
                .organizations
                .iter()
                .filter_map(|(organization, work)| {
                    work.max_concurrent
                        .map(|limit| (organization.clone(), limit))
                })
                .collect(),
            projects: self
                .projects
                .iter()
                .filter_map(|(project, work)| {
                    let limits = Limits {
                        concurrent: work.max_concurrent,
                        active: work.max_active,
                    };
                    (limits.concurrent.is_some() || limits.active.is_some())
                        .then(|| (project.clone(), limits))
                })
                .collect(),
            membership: self.organization_of_each_project(),
        };
        // Said where the ceilings are read, so a pair written the wrong way
        // round — or a ceiling over an organization holding no project — is
        // seen at the sweep it costs something rather than only by whoever
        // runs a check over the file (§FS-005-dispatch.24).
        for unbound in self.ceilings_over_nobody() {
            self.note_once(&unbound);
        }
        for inversion in ceilings.inversions() {
            self.note_once(&inversion);
        }
        let live = LiveRuns::read(&self.global, &roots, &ceilings.membership);
        let mut capacity = Capacity::new(ceilings, live).against(budgets, budget);
        // Budgets that are full where a person asked for this sweep, said once
        // rather than once per root (§FS-015-spend-ceiling.6).
        let mut warned: Vec<String> = Vec::new();
        // The trees this sweep's own launches have taken, checkout to the root
        // the run was started from. The due list was read from a snapshot
        // older than every launch below, so two due roots over one working
        // tree are both in it; without this they would both start
        // (§FS-005-dispatch.24). A tree a run held *before* the sweep is not
        // in here — [`due_among`] wrote that on the root itself.
        let mut taken: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
        let due = excluded.mark(self.due_over(&roots, now, projects, Reach::Sweep)?);
        // What this sweep read of the last run on each root, kept before
        // anything is started. A verdict is the acting sweep's and only its:
        // reading one stream twice gives one answer, so nothing is lost by a
        // report that keeps none (§FS-005-dispatch.24,
        // §FS-011-command-line.10).
        for root in &due {
            let key = root_key(&root.root);
            match &root.verdict {
                Some(Verdict::Advanced) => {
                    self.ledger.advances.remove(&key);
                    self.ledger.waited.remove(&key);
                }
                Some(Verdict::Waiting) => {
                    self.ledger.advances.remove(&key);
                    self.ledger.waited.insert(key);
                }
                Some(Verdict::Nothing(judged)) => {
                    self.ledger.waited.remove(&key);
                    self.ledger.advances.insert(key, judged.clone());
                }
                None => {}
            }
        }
        // What the pools have been reported to be, read once for the whole
        // sweep (§FS-005-dispatch.29).
        let evidence = headroom::Evidence::read(&self.global, &self.ledger, now);
        let runs = due
            .into_iter()
            .map(|mut root| {
                // The reader's own instruction, then a root waiting on a
                // person, then the rest ephor decided on: all successful
                // non-launch outcomes, all before capacity is spent, and all
                // said in the row where this used to say *started*
                // (§FS-005-dispatch.24, §FS-005-dispatch.24.3.2).
                if let Some(hold) = root.hold().cloned() {
                    return Launched::passed_over(&root, hold);
                }
                // A tree another root's run held before this sweep began, and
                // a tree a launch in this same sweep has just taken: one
                // condition, one sentence, one kind of row. Passed over with
                // the run that has it, never raced — a successful non-launch
                // outcome, and a reader told only that nothing started would
                // go looking for a ceiling that is not full
                // (§FS-005-dispatch.24).
                if let Some(held_by) = root
                    .held_by
                    .as_ref()
                    .or_else(|| holding(&taken, &root.checkout))
                {
                    // The same run the sentence names, kept as data
                    // (§FS-005-dispatch.24.2).
                    let hold = held_in_this_checkout(&self.global, held_by);
                    return Launched::passed_over(&root, hold);
                }
                // A plan whose work needs several pools at once that this
                // site cannot have together is passed over *before* capacity
                // is spent, and the plan stays exactly where it is: this row
                // reads *not started, and still yours*, where the admission's
                // reads *held, and still anybody's*
                // (§FS-005-dispatch.33, §FS-005-dispatch.24).
                let held = self.held_plans(&root, &evidence);
                if !held.is_empty() {
                    let runnable: Vec<String> = root
                        .plans
                        .iter()
                        .filter(|plan| !held.contains_key(*plan))
                        .cloned()
                        .collect();
                    match runnable.is_empty() {
                        true => {
                            let (plan, (entry, why)) = held
                                .into_iter()
                                .next()
                                .expect("a held plan, since the map is not empty");
                            // The plan directory, every pool in the order it
                            // named them, and the first spent one
                            // (§FS-005-dispatch.24.2).
                            return Launched::passed_over(
                                &root,
                                Hold::Pools {
                                    plan,
                                    entry,
                                    pools: why.required,
                                    pool: why.pool,
                                    until: why.until,
                                    clause: why.clause,
                                },
                            );
                        }
                        // Some other plan in this root is runnable, so the run
                        // is narrowed to it rather than the root passed over:
                        // what is held is the held plan's work, not its
                        // neighbour's (§FS-005-dispatch.24).
                        false => {
                            root.tickets.retain(|ticket| {
                                runnable
                                    .iter()
                                    .any(|plan| ticket.starts_with(&format!("{plan}.")))
                            });
                            root.plans = runnable;
                        }
                    }
                }
                if let Some(said) = capacity
                    .warning(&root.projects)
                    .map(str::to_string)
                    .filter(|said| !warned.contains(said))
                {
                    warned.push(said);
                }
                if let Some(hold) = capacity.hold(&root.projects) {
                    return Launched::passed_over(&root, hold);
                }
                if !detaches {
                    return Launched::refused(
                        &root,
                        format!(
                            "{} cannot start a run detached here, and a run nobody asked for \
                             must not take a terminal",
                            runtime::runner(&self.global)
                        ),
                    );
                }
                let hand = self
                    .ledger
                    .entries
                    .values()
                    .find(|entry| {
                        entry.dispatches.iter().any(|dispatch| {
                            canonical(dispatch.root.as_ref().unwrap_or(&entry.root))
                                == canonical(&root.root)
                        }) || (entry.dispatches.is_empty()
                            && canonical(&entry.root) == canonical(&root.root))
                    })
                    .cloned()
                    .and_then(|entry| {
                        let status = self.status_of(&entry, None);
                        self.run_hand(&entry, &status)
                    });
                match runtime::start_detached(
                    &self.global,
                    &root.root,
                    &root.checkout,
                    &root.plans,
                    hand.as_ref(),
                    runner_args,
                ) {
                    Ok(started) => {
                        self.start_worked(&root.root);
                        // The descriptor can already say the run is over, and
                        // a run that died just after publishing it has already
                        // released the lock. Neither occupies a slot needed by
                        // the next ranked root (§FS-005-dispatch.24).
                        let remains_live =
                            !started.finished && runtime::watch::live(&self.global, &root.root);
                        if remains_live {
                            take_checkout(&mut taken, &root.checkout, &root.root);
                        }
                        capacity.started(&root.projects, remains_live);
                        Launched {
                            id: started.id,
                            finished: started.finished,
                            failed: None,
                            ..Launched::of(&root)
                        }
                    }
                    Err(err) => {
                        self.start_failed(&root.root, &err.to_string(), now);
                        Launched::refused(&root, err.to_string())
                    }
                }
            })
            .collect();
        ledger::store(&self.ledger)?;
        Ok(Sweep {
            runs,
            capacity: capacity.standing(),
            warned,
        })
    }

    /// The roots a sweep would start a run on, narrowed to the projects it was
    /// asked about (§FS-005-dispatch.24).
    ///
    /// Split out of [`Dispatcher::start_due`] and shared with it, because a
    /// `work run --due` held at the gate has to report the sweep it is
    /// reporting on rather than a second opinion about it
    /// (§FS-011-command-line.10).
    fn due_over(
        &mut self,
        roots: &[runtime::watch::RootPlans],
        now: DateTime<Utc>,
        projects: &[String],
        reach: Reach<'_>,
    ) -> Result<Vec<Due>> {
        Ok(self
            .due_in(roots, now, reach)?
            .into_iter()
            .filter(|root| {
                projects.is_empty()
                    || root
                        .projects
                        .iter()
                        .any(|project| projects.contains(project))
            })
            .collect())
    }

    /// The same reading for a caller that will only say what it found: which
    /// roots are due now, with nothing started, nothing locked and nothing
    /// written (§FS-011-command-line.10).
    pub fn due_now(&mut self, now: DateTime<Utc>, projects: &[String]) -> Result<Vec<Due>> {
        let roots = self.work_roots();
        self.due_over(&roots, now, projects, Reach::Sweep)
    }

    /// Remember that starting a run on this root did not work, so the next
    /// sweep passes it over for a while (§FS-005-dispatch.24). Ephor's record
    /// of ephor's own act; the work's state is untouched and stays the plan's
    /// (§FS-005-dispatch.4).
    pub fn start_failed(&mut self, root: &std::path::Path, says: &str, now: DateTime<Utc>) {
        let key = root_key(root);
        let failures = self
            .ledger
            .starts
            .get(&key)
            .map(|start| start.failures.saturating_add(1))
            .unwrap_or(1);
        self.ledger.starts.insert(
            key,
            ledger::Start {
                at: now,
                failures,
                says: says.to_string(),
            },
        );
        self.pool_refused(root, says, now);
    }

    /// A failed start, read as what a provider said about its own window — but
    /// only where the words carry an instant ephor can read
    /// (§FS-005-dispatch.29).
    ///
    /// A refusal names when it lifts, and that instant is the whole of what
    /// makes one start evidence about a *pool* rather than about one root.
    /// Where the words carry none, nothing about a pool is claimed and the
    /// failure stays exactly what the line above made it — this root's own
    /// doubling back-off (§FS-005-dispatch.24) — because a failure ephor
    /// cannot date is not a window it may guess at.
    ///
    /// `now` is what tells a reset instant apart from a log line: the words are
    /// the run's whole merged output, and only an instant still ahead of now is
    /// a window that can lift.
    fn pool_refused(&mut self, root: &std::path::Path, says: &str, now: DateTime<Utc>) {
        let Some(until) = headroom::instant_in(says, now) else {
            return;
        };
        let Some(pool) = self.pool_of_root(root) else {
            return;
        };
        let record = self.ledger.pools.entry(pool).or_default();
        record.refused_until = Some(until);
        record.says = Some(says.to_string());
    }

    /// Which pool a run started on this root spends from: the one the last
    /// dispatch onto a matter living here chose. None where nothing was
    /// chosen, or where the root holds work ephor never dispatched — in which
    /// case ephor knows of no pool to claim anything about.
    fn pool_of_root(&self, root: &std::path::Path) -> Option<String> {
        self.ledger
            .entries
            .values()
            .find(|entry| {
                entry.dispatches.iter().any(|dispatch| {
                    canonical(dispatch.root.as_ref().unwrap_or(&entry.root)) == canonical(root)
                }) || (entry.dispatches.is_empty() && canonical(&entry.root) == canonical(root))
            })
            .and_then(|entry| entry.pool.clone())
    }

    /// Forget a root's failed starts: a run began there, so whatever was
    /// wrong is not wrong now. Nothing is remembered about a start that
    /// worked — the run leaves a lock, and the lock is what every later sweep
    /// reads (§FS-005-dispatch.15).
    pub fn start_worked(&mut self, root: &std::path::Path) {
        self.ledger.starts.remove(&root_key(root));
        // A start that worked is an observed success on the pool, which clears
        // whatever it last refused (§FS-005-dispatch.29). The count beside it
        // is shown and never read into the rule: counting one's own spawns is
        // deriving a quota under another name.
        if let Some(pool) = self.pool_of_root(root) {
            let record = self.ledger.pools.entry(pool).or_default();
            record.refused_until = None;
            record.says = None;
            record.spawns = record.spawns.saturating_add(1);
        }
    }

    /// Every entry whose recorded plan name is not the stem its own id renders
    /// now (§FS-005-dispatch.3.1). The question is answered by looking, so
    /// nothing is written down about whether the carry-over has run.
    fn plans_behind(&self) -> Vec<Behind> {
        self.ledger
            .entries
            .iter()
            .filter_map(|(id, entry)| {
                let now = plan::plan_id(id);
                (entry.plan_id != now).then(|| Behind {
                    id: id.clone(),
                    was: entry.plan_id.clone(),
                    now,
                    root: entry.root.clone(),
                })
            })
            .collect()
    }

    /// The carry-overs this pass will not make, each said in full and each
    /// naming the matters it holds back (§FS-005-dispatch.3).
    ///
    /// All three shapes are two records of work that only a person can
    /// separate, so all three stop at saying so. None of them reaches past the
    /// matters it names: a collided root is not a reason to leave an unrelated
    /// project behind.
    fn refused_carry_overs(&self, behind: &[Behind]) -> Vec<Refused> {
        let mut refused: Vec<Refused> = Vec::new();
        // Two records of one plan file — the state the collision this digest
        // fixes actually leaves on disk, where an older ephor wrote both
        // matters' tickets into one file and recorded both entries at it.
        // Giving the file to one of them would leave the other's record naming
        // a file that is not there, which is the one outcome a carry-over must
        // not produce (§FS-005-dispatch.3.1).
        let mut sharing: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
        for entry in behind {
            sharing
                .entry(plan::plan_path_in(&entry.root, &entry.was))
                .or_default()
                .push(entry.id.clone());
        }
        for (file, matters) in sharing {
            if matters.len() < 2 || !file.is_file() {
                continue;
            }
            refused.push(Refused {
                says: format!(
                    "{} is recorded as the plan of {} at once. One plan file holding two \
                     matters is what an ephor that named plans without the digest wrote, \
                     and which of its tickets belongs to which matter is yours to say — so \
                     nothing about them has been moved. Split it into one plan per matter, \
                     and the next dispatch carries each of them over.",
                    file.display(),
                    matters.join(" and ")
                ),
                matters,
            });
        }
        let held: BTreeSet<String> = refused
            .iter()
            .flat_map(|one| one.matters.iter().cloned())
            .collect();
        // One matter holding a plan at the digested name and at a pre-digest
        // name at once, which only a mixed pair of binaries can produce
        // (§FS-005-dispatch.3).
        for entry in behind {
            if held.contains(&entry.id) {
                continue;
            }
            let (old, new) = (
                plan::plan_path_in(&entry.root, &entry.was),
                plan::plan_path_in(&entry.root, &entry.now),
            );
            if !(old.is_file() && new.is_file()) {
                continue;
            }
            refused.push(Refused {
                matters: vec![entry.id.clone()],
                says: format!(
                    "{} has a plan at two names: {} and {}. One of them was written by \
                     an ephor that named plans without the digest, and which of two records \
                     of the same work to go on with is yours to say — so nothing here has \
                     been moved. Keep one of the two files and remove the other.",
                    entry.id,
                    old.display(),
                    new.display()
                ),
            });
        }
        let held: BTreeSet<String> = refused
            .iter()
            .flat_map(|one| one.matters.iter().cloned())
            .collect();
        // A carry-over is a rename, and a rename onto a name that already
        // holds a file destroys what is there without a word. Whatever is
        // already at the new name is another record of this same matter's
        // work, so which of the two to keep is the reader's to say and nothing
        // here is moved (§FS-005-dispatch.3.1).
        for entry in behind {
            if held.contains(&entry.id) {
                continue;
            }
            // Where a plan stands at the new name, a writer may be holding the
            // sidecar beside it, and replacing that is the one way this could
            // let two writers think they hold one plan. Where no plan stands
            // there, nothing can be writing one (§FS-005-dispatch.3.1).
            let a_plan_stands_there = plan::plan_path_in(&entry.root, &entry.now).is_file();
            let occupied: Vec<String> =
                runtime::carried_over_paths(&entry.root, &entry.was, &entry.now)
                    .into_iter()
                    // What is held back is a record of work, and a writer's
                    // sidecar is not one: it holds nothing and nothing ever
                    // removes it, so a stale one at the new name would
                    // otherwise refuse this matter for good, under a message
                    // asking which of two records of the work to keep.
                    .filter(|(_, to)| a_plan_stands_there || runtime::holds_work(to))
                    .filter(|(_, to)| to.exists())
                    .map(|(from, to)| format!("{} onto {}", from.display(), to.display()))
                    .collect();
            if occupied.is_empty() {
                continue;
            }
            refused.push(Refused {
                matters: vec![entry.id.clone()],
                says: format!(
                    "{} cannot be carried over from {} to {}: {} would be written over, \
                     and a carry-over never writes over a file that is already there — so \
                     nothing here has been moved. One of each pair was left by an ephor \
                     that named plans without the digest, and which of two records of the \
                     same work to keep is yours to say. Keep one of each pair and remove \
                     the other, and the next dispatch carries the rest over.",
                    entry.id,
                    entry.was,
                    entry.now,
                    occupied.join(", ")
                ),
            });
        }
        refused
    }

    /// One entry's plan, everything the stem names, the references inside the
    /// plans that name it and the recorded name, moved together and committed
    /// on their own (§FS-005-dispatch.3.1).
    ///
    /// Together includes the bytes: a plan's own result block, and an ordering
    /// or a consumed export in a plan beside it, are records of where a file
    /// is, so moving the file and leaving the record is the one outcome a
    /// rename must not produce. The rewrite is made inside this same hand-off,
    /// which is why a reference that cannot be written stops this entry exactly
    /// as a file that cannot be moved does.
    ///
    /// Its own hand-off, so wherever an error escapes the record and the disk
    /// agree: what this entry had already moved is put back and the ledger is
    /// left saying what it said (§FS-005-dispatch.4). Committing per entry is
    /// what keeps one root's trouble from undoing another's move.
    fn carry_one_over(&mut self, behind: &Behind) -> Result<Option<String>> {
        let moves = runtime::carried_over_paths(&behind.root, &behind.was, &behind.now);
        self.begin_handoff();
        for (from, to) in &moves {
            if let Err(why) = self
                .journal
                .remember(from)
                .and_then(|()| self.journal.remember(to))
            {
                return Err(self.unwind(why));
            }
        }
        for (from, to) in &moves {
            if let Err(err) = std::fs::rename(from, to) {
                let why = EphorError::Command(format!(
                    "Cannot carry {} over to {}: {err}",
                    from.display(),
                    to.display()
                ));
                return Err(self.unwind(why));
            }
        }
        // Read after the moves, so the plan that has just been renamed is read
        // where it is now and its own record is rewritten with the rest.
        let rewrites = match runtime::references_rewritten(&behind.root, &behind.was, &behind.now) {
            Ok(rewrites) => rewrites,
            Err(why) => return Err(self.unwind(why)),
        };
        for (path, text) in &rewrites.plans {
            if let Err(why) = self.journal.remember(path) {
                return Err(self.unwind(why));
            }
            if let Err(err) = std::fs::write(path, text) {
                let why = EphorError::Command(format!(
                    "Cannot rewrite what {} says about {}: {err}",
                    path.display(),
                    behind.was
                ));
                return Err(self.unwind(why));
            }
        }
        let entry = self
            .ledger
            .entries
            .get_mut(&behind.id)
            .expect("the entry was read from this ledger");
        entry.plan_id = behind.now.clone();
        entry.plan = plan::plan_path_in(&behind.root, &behind.now);
        self.save()?;
        // A source this carry-over could not read is said on its own: it held
        // the carry-over back from nothing, so it is neither a refusal nor part
        // of what moved (§FS-005-dispatch.3.1).
        for said in rewrites.passed_over {
            self.note_once(&said);
        }
        // What is reported is what a reader has lost a path to, so an entry
        // whose plan was never written has its recorded name corrected and is
        // not reported: there was no file, and so nothing that reads as gone.
        // A sidecar is not such a file either — nobody holds a path to one. A
        // reference rewritten in a plan beside it is: the path a reader had is
        // in those bytes, so an entry that moved only a sidecar and rewrote a
        // plan still says so.
        Ok((moves.iter().any(|(from, _)| runtime::reads_as_gone(from))
            || !rewrites.plans.is_empty())
        .then(|| {
            format!(
                "carried the plan of {} over from {} to {} in {}",
                behind.id,
                behind.was,
                behind.now,
                behind.root.display()
            )
        }))
    }

    /// Put back what this hand-off had already moved, restore the ledger it
    /// began from, and say what went wrong (§FS-005-dispatch.4).
    fn unwind(&mut self, why: EphorError) -> EphorError {
        let journal = std::mem::take(&mut self.journal);
        let (ledger, cleanup) = journal.rollback();
        self.ledger = ledger;
        match cleanup {
            None => why,
            Some((path, failure)) => EphorError::Command(format!(
                "{why}; rollback could not restore {}: {failure}",
                path.display()
            )),
        }
    }

    /// One pass: carry over every root that can be, and collect what stood in
    /// the way rather than stopping on the first of it (§FS-005-dispatch.3.1).
    fn carry_plans_over(&mut self) -> CarriedOver {
        let behind = self.plans_behind();
        let mut pass = CarriedOver::default();
        if behind.is_empty() {
            return pass;
        }
        pass.refused = self.refused_carry_overs(&behind);
        let held: BTreeSet<String> = pass
            .refused
            .iter()
            .flat_map(|one| one.matters.iter().cloned())
            .collect();
        for entry in &behind {
            if held.contains(&entry.id) {
                continue;
            }
            // A run holding this root waits: moving a plan out from under it
            // is the one way this could lose work, and the next verb entitled
            // to write carries it over instead (§FS-005-dispatch.3.1).
            if runtime::watch::live(&self.global, &entry.root) {
                continue;
            }
            match self.carry_one_over(entry) {
                Ok(said) => pass.moved.extend(said),
                // A root that cannot be carried over is reported and stops
                // itself; the rest of the pass goes on, because one
                // unreachable directory is no reason to leave every other
                // project behind.
                Err(why) => pass.refused.push(Refused {
                    matters: vec![entry.id.clone()],
                    says: why.to_string(),
                }),
            }
        }
        pass
    }

    /// Carry every plan named before the digest over to the name its matter's
    /// id renders now, once, and say what moved (§FS-005-dispatch.3.1).
    ///
    /// Run from the verbs entitled to write in the roots they name — the
    /// dispatch, the lay and the sweep — rather than wherever the ledger is
    /// read: a reading command answers from the recorded name and is
    /// self-consistent before anything moves, so carrying over beneath it
    /// would write where `--act` and `--dry-run` promised nothing would be
    /// (§FS-011-command-line.10, §FS-005-dispatch.26).
    ///
    /// Decided per entry and out of the entry itself — a recorded name that is
    /// not the stem of its own id — so it is idempotent and needs nothing
    /// written down about whether it has run.
    ///
    /// What could not be carried over is the error, after everything that
    /// could has been: a refusal is about the matters it names and never about
    /// the rest of the ledger.
    pub fn carry_over_plan_names(&mut self) -> Result<Vec<String>> {
        let pass = self.carry_plans_over();
        if pass.refused.is_empty() {
            return Ok(pass.moved);
        }
        Err(EphorError::Command(pass.stopped()))
    }

    /// Carry over before this verb recomputes a stem in a root it is entitled
    /// to write in, and say what moved (§FS-005-dispatch.3.1, §REQ-002-parity).
    ///
    /// A root that could not be carried over stops the matter it is about and
    /// nothing else: the dispatch of that matter would otherwise open a second
    /// plan at the digested name and orphan the one already there, while a
    /// collided root in another project is somebody else's to split and is
    /// said rather than obeyed (§FS-005-dispatch.3).
    fn carry_over_before_writing(&mut self, about: Option<&str>) -> Result<()> {
        let pass = self.carry_plans_over();
        for note in pass.moved {
            self.note_once(&note);
        }
        for refused in pass.refused {
            if about.is_some_and(|id| refused.matters.iter().any(|matter| matter == id)) {
                return Err(EphorError::Command(refused.says));
            }
            self.note_once(&refused.says);
        }
        Ok(())
    }

    /// What a carry-over would refuse about this matter, asked without moving
    /// anything — so a dry run refuses exactly where the dispatch it promises
    /// would (§FS-005-dispatch.26, §FS-005-dispatch.3).
    fn carry_over_refusal(&self, item: &str) -> Option<String> {
        let behind = self.plans_behind();
        self.refused_carry_overs(&behind)
            .into_iter()
            .find(|refused| refused.matters.iter().any(|matter| matter == item))
            .map(|refused| refused.says)
    }

    /// Where this matter's plan still stands, for a run that reports rather
    /// than writes: the recorded stem and the file it names, where the root
    /// has not been carried over yet (§FS-005-dispatch.3.1).
    ///
    /// A dry run recomputes the stem and moves nothing, so the plan the next
    /// real dispatch appends to is the one the record names
    /// ([`Dispatcher::save`] keeps the two together, §FS-005-dispatch.4). A
    /// dry run that read only the digested name would promise a first ticket
    /// in a fresh plan where an append is what is actually due — and a dry run
    /// that lies is no better than one that writes (§FS-005-dispatch.26).
    fn recorded_plan_behind(&self, item: &str, stem: &str) -> Option<(String, PathBuf)> {
        let entry = self.ledger.entries.get(item)?;
        (entry.plan_id != stem && entry.plan.is_file())
            .then(|| (entry.plan_id.clone(), entry.plan.clone()))
    }

    /// Commit the ledger and only then release the work-root pre-images. A
    /// failed atomic store restores the entire unsaved batch and the loaded
    /// in-memory ledger (§FS-005-dispatch.4).
    pub fn save(&mut self) -> Result<()> {
        match ledger::store(&self.ledger) {
            Ok(()) => {
                self.journal = Journal::default();
                Ok(())
            }
            Err(save_error) => {
                if self.journal.ledger.is_none() {
                    return Err(save_error);
                }
                let journal = std::mem::take(&mut self.journal);
                let (ledger, cleanup) = journal.rollback();
                self.ledger = ledger;
                match cleanup {
                    None => Err(save_error),
                    Some((path, cleanup)) => Err(EphorError::Command(format!(
                        "{save_error}; rollback could not restore {}: {cleanup}",
                        path.display()
                    ))),
                }
            }
        }
    }

    fn begin_handoff(&mut self) {
        self.journal.begin(&self.ledger);
    }

    /// Capture under the row lock and retain it in the hand-off journal through
    /// commit or rollback (§FS-005-dispatch.4, §FS-005-dispatch.13).
    fn capture_reply(
        &mut self,
        item: &Item,
        path: PathBuf,
    ) -> Result<Option<crate::replies::Binding>> {
        if crate::replies::binding::threads(item).is_empty() {
            return Ok(None);
        }
        if !self.journal.reply_locks.contains_key(&item.id) {
            self.journal.reply_locks.insert(
                item.id.clone(),
                crate::replies::Store::site(&item.id, true)?,
            );
        }
        let record = self.journal.reply_locks[&item.id]
            .read()?
            .unwrap_or_else(|| crate::replies::Record::new(item));
        let config = &self.reply_config;
        let sources = crate::feed::providers::Sources {
            project: item.project.clone(),
            own: config
                .projects
                .get(&item.project)
                .map(|project| project.providers.clone())
                .unwrap_or_default(),
            site: config.sources.clone(),
        };
        Ok(crate::replies::Binding::capture(
            item,
            &sources,
            &config.defaults,
            &record,
            path,
        ))
    }

    /// Remember exactly the bootstrap paths `WorkRoot::ensure` may mutate.
    /// A root absent before the batch is remembered as one created tree;
    /// an existing root's unrelated contents are deliberately not captured
    /// or restored (§FS-005-dispatch.4).
    fn journal_work_root(&mut self, root: &std::path::Path) -> Result<()> {
        if !root.exists() {
            return self.journal.remember(root);
        }
        for name in [
            runtime::plan::MANIFEST,
            runtime::plan::STATES,
            runtime::plan::IGNORE,
        ] {
            self.journal.remember(&root.join(name))?;
        }
        Ok(())
    }
}

/// One work root a sweep should start a run on — or, where `held_by` says a
/// run in another root holds its working tree, would start one on but for
/// that run (§FS-005-dispatch.24).
#[derive(Debug, Clone)]
pub struct Due {
    pub project: String,
    /// Every project with a plan on this root. Usually one; all of them count
    /// when a deliberately shared root is live.
    projects: Vec<String>,
    pub root: PathBuf,
    /// The checkout the run is made from.
    pub checkout: PathBuf,
    /// The plans holding what made this root due — what the run is narrowed
    /// to, so a runtime project the reader keeps in the same root for their
    /// own work is not swept up by ephor's.
    pub plans: Vec<String>,
    /// The tickets themselves, plan-qualified: what the line saying a run
    /// started names as the reason.
    pub tickets: Vec<String>,
    /// Every matter whose due tickets contribute to this root. Ranking uses
    /// the best configured position among them; plans with no due ticket do
    /// not get to rank the root.
    items: Vec<String>,
    /// The matter this root's work is about, where the ledger knows one —
    /// where a failure's news lands (§FS-005-dispatch.24).
    pub item: Option<String>,
    /// The other root whose live run holds this root's working tree, where
    /// one does (§FS-005-dispatch.24). Nothing may be started here while it
    /// is set: the row exists so the sweep can say so by name.
    pub held_by: Option<PathBuf>,
    /// Why nothing may be started in this root at all, where the key's
    /// reading returned it only so that the reader who named the matter is
    /// told no (§FS-005-dispatch.30). Set on a root the sweep drops in
    /// silence and the key may not: one whose machine will not read, and one
    /// whose checkout stands on another branch. The row carries no plans a
    /// run could be pointed at — it exists so the refusal names the root
    /// rather than the matter coming back as finished. Never set for the
    /// sweep, which drops both.
    pub refusal: Option<String>,
    /// Why no sweep starts this root, where every ticket that would have made
    /// it due is about a matter a source the site lists as private reported
    /// (§FS-018-private-sources.3). Always [`Hold::Private`], and asked first:
    /// nothing a sweep could wait for lifts it (§FS-005-dispatch.24.2). Set for
    /// a plain run, which names no matter and so is a sweep here; never for a
    /// run that names the matter, which is the person's own move.
    pub private: Option<Hold>,
    /// Why the reader's own `--except` leaves this root out, where it does
    /// (§FS-005-dispatch.24). Set after the reading, because an exclusion is
    /// the reader's instruction rather than anything ephor worked out. Always
    /// [`Hold::Excluded`] (§FS-005-dispatch.24.2).
    pub excluded: Option<Hold>,
    /// Why this root waits on a person, where it does: every ticket that
    /// would have made it due is held by a gate in its own tree
    /// (§FS-005-dispatch.24.3.2). Always [`Hold::Person`], and asked after
    /// the reader's `--except` and before the no-advance rest
    /// (§FS-005-dispatch.24).
    pub person: Option<Hold>,
    /// Why the last run here having advanced nothing leaves this root alone —
    /// the rest, or the end of resting (§FS-005-dispatch.24): [`Hold::Rested`]
    /// or [`Hold::Stopped`] (§FS-005-dispatch.24.2).
    pub rested: Option<Hold>,
    /// What this sweep read of the last run here, where it could read one. A
    /// sweep that acts keeps it; a report held at the gate keeps none
    /// (§FS-011-command-line.10).
    pub verdict: Option<Verdict>,
}

impl Due {
    /// The one reason this root gets no run, where something says so: a
    /// private matter first, because no sweep can lift it, then the reader's
    /// own instruction, because naming it back is what tells them the flag
    /// took effect, and then the rest ephor decided on. One row, one reason,
    /// first match (§FS-005-dispatch.24, §FS-005-dispatch.24.2).
    pub fn passed_over(&self) -> Option<String> {
        self.hold().map(Hold::says)
    }

    /// The same first match as data, for the row a program reads
    /// (§FS-005-dispatch.24.2).
    pub fn hold(&self) -> Option<&Hold> {
        self.private
            .as_ref()
            .or(self.excluded.as_ref())
            .or(self.person.as_ref())
            .or(self.rested.as_ref())
    }
}

/// What starting one due root came to (§FS-005-dispatch.24).
#[derive(Debug, Clone)]
pub struct Launched {
    pub project: String,
    pub root: PathBuf,
    /// The matter the work is about, where the ledger knows one — where the
    /// news of a failure lands (§FS-005-dispatch.24).
    pub item: Option<String>,
    /// The tickets that made this root due, plan-qualified.
    pub tickets: Vec<String>,
    /// What the run calls itself, where it named itself.
    pub id: Option<String>,
    /// The run was over before the launcher returned — nothing was left to
    /// do. Reported as over rather than as started, so nobody is sent to a
    /// board with nothing on it (§FS-005-dispatch.20).
    pub finished: bool,
    /// Why no run was started, where none was.
    pub failed: Option<String>,
    /// What held an otherwise eligible root, where something did. This is a
    /// successful, non-launch outcome, not a failure; its sentence is the
    /// row's reason and its data the row's `hold` (§FS-005-dispatch.24.2).
    pub passed_over: Option<Hold>,
}

impl Launched {
    fn of(due: &Due) -> Launched {
        Launched {
            project: due.project.clone(),
            root: due.root.clone(),
            item: due.item.clone(),
            tickets: due.tickets.clone(),
            id: None,
            finished: false,
            failed: None,
            passed_over: None,
        }
    }

    fn refused(due: &Due, why: String) -> Launched {
        Launched {
            failed: Some(why),
            ..Launched::of(due)
        }
    }

    fn passed_over(due: &Due, hold: Hold) -> Launched {
        Launched {
            passed_over: Some(hold),
            ..Launched::of(due)
        }
    }

    /// The one line this is worth: what started, or what stopped it. The
    /// same sentence wherever it is said, so a command and a screen never
    /// phrase one situation two ways (§AR-009-surfaces.1).
    pub fn says(&self) -> String {
        match (&self.failed, &self.passed_over, &self.id, self.finished) {
            (Some(why), ..) => format!("⚠ no run started on {}: {why}", self.root.display()),
            (None, Some(hold), ..) => {
                format!("↷ {} passed over: {}", self.root.display(), hold.says())
            }
            (None, None, Some(id), false) => format!("▶ run {id} started"),
            (None, None, Some(id), true) => format!("✓ run {id} finished already"),
            (None, None, None, false) => "▶ run started".to_string(),
            (None, None, None, true) => "✓ the run finished already".to_string(),
        }
    }

    /// What a reading calls this (§REQ-002-parity.3).
    pub fn outcome(&self) -> &'static str {
        match (&self.failed, &self.passed_over, self.finished) {
            (Some(_), ..) => "failed",
            (None, Some(_), _) => "passed-over",
            (None, None, true) => "done",
            (None, None, false) => "started",
        }
    }

    pub fn reason(&self) -> Option<String> {
        self.passed_over.as_ref().map(Hold::says)
    }

    /// What held this root, as data (§FS-005-dispatch.24.2).
    pub fn hold(&self) -> Option<&Hold> {
        self.passed_over.as_ref()
    }
}

/// Live roots in one scope — the site, or one project — split by what they
/// are doing (§FS-005-dispatch.24). `live` is every root in flight and
/// answers for `max_concurrent`; `active` is the ones being worked and
/// answers for `max_active`. The difference is the roots parked on a
/// person's answer, which is the count a reader is owed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Counts {
    live: usize,
    active: usize,
}

impl Counts {
    fn took(&mut self, active: bool) {
        self.live += 1;
        self.active += usize::from(active);
    }

    /// Live but not working: the roots waiting on a person.
    fn parked(self) -> usize {
        self.live - self.active
    }
}

/// Live roots at the beginning of a sweep, counted once from the same root
/// snapshot that supplies due candidates (§FS-005-dispatch.24).
#[derive(Debug, Default)]
struct LiveRuns {
    global: Counts,
    /// An organization bounds roots in flight only, so its slots are a plain
    /// count rather than a split one (§FS-005-dispatch.24).
    organizations: BTreeMap<String, usize>,
    projects: BTreeMap<String, Counts>,
}

impl LiveRuns {
    fn read(
        global: &WorkConfig,
        roots: &[runtime::watch::RootPlans],
        membership: &BTreeMap<String, String>,
    ) -> LiveRuns {
        let mut live = LiveRuns::default();
        for root in roots {
            if !runtime::watch::live(global, &root.root) {
                continue;
            }
            // What the root is doing is asked only of a root that is live:
            // the reading costs the plans and the machine, and an idle root
            // has no slot to be spending (§FS-005-dispatch.24).
            let active = !runtime::watch::parked(global, root);
            live.global.took(active);
            let projects: BTreeSet<&str> = root
                .plans
                .iter()
                .map(|plan| plan.project.as_str())
                .collect();
            // One live root is one slot in each organization it reaches, even
            // where two of that organization's projects hold plans on it: the
            // slot is the run, and there is one of those (§FS-005-dispatch.24).
            let organizations: BTreeSet<&str> = projects
                .iter()
                .filter_map(|project| membership.get(*project).map(String::as_str))
                .collect();
            for project in projects {
                live.projects
                    .entry(project.to_string())
                    .or_default()
                    .took(active);
            }
            for organization in organizations {
                *live
                    .organizations
                    .entry(organization.to_string())
                    .or_default() += 1;
            }
        }
        live
    }
}

/// The two ceilings one scope — the site, or one project — may declare
/// (§FS-005-dispatch.24). Missing stays missing: omission is unlimited, while
/// `Some(0)` is deliberately no capacity.
#[derive(Debug, Default, Clone, Copy)]
struct Limits {
    /// `max_concurrent`, over every live root.
    concurrent: Option<usize>,
    /// `max_active`, over the live roots that are working.
    active: Option<usize>,
}

impl Limits {
    /// Which of the two refuses a start here, in the words that name the key
    /// it refused on (§FS-005-dispatch.24). Roots in flight is asked first,
    /// so a scope that never named the second key answers exactly as it did
    /// before there was one — same wording, same count. The count a hold
    /// names is the one its key is over (§FS-005-dispatch.24.2).
    fn full(&self, live: Counts, scope: &spend::Scope) -> Option<Hold> {
        if let Some(limit) = self.concurrent.filter(|limit| live.live >= *limit) {
            return Some(Hold::Concurrency {
                scope: scope.clone(),
                key: Flight::Concurrent,
                limit,
                count: live.live,
            });
        }
        if let Some(limit) = self.active.filter(|limit| live.active >= *limit) {
            return Some(Hold::Concurrency {
                scope: scope.clone(),
                key: Flight::Active {
                    parked: live.parked(),
                },
                limit,
                count: live.active,
            });
        }
        None
    }
}

/// The nested ceilings one sweep reads, and who is inside which organization
/// (§FS-005-dispatch.24). Missing ceilings stay missing: omission is
/// unlimited, while `Some(0)` is deliberately no capacity.
#[derive(Debug, Default)]
struct Ceilings {
    /// The site ceilings in force for this sweep: the configured pair, with
    /// the number `--max-concurrent` put in the flight one's place for this
    /// invocation.
    site: Limits,
    /// The site flight ceiling as configuration writes it. A pair written the
    /// wrong way round is a fact about the configuration, so it is measured
    /// against this rather than against a flag that narrowed one sweep on
    /// purpose.
    site_as_configured: Option<usize>,
    /// An organization bounds roots in flight and nothing else, so it holds
    /// one number rather than a pair (§FS-005-dispatch.24).
    organizations: BTreeMap<String, usize>,
    projects: BTreeMap<String, Limits>,
    /// Which organization each project belongs to, as the registry declares
    /// it. A project absent here belongs to none and is under no organization
    /// ceiling (§FS-005-dispatch.24).
    membership: BTreeMap<String, String>,
}

impl Ceilings {
    /// The organization a project is in, where the registry named one.
    fn organization_of(&self, project: &str) -> Option<&str> {
        self.membership.get(project).map(String::as_str)
    }

    /// Every pair written the wrong way round — a project ceiling above the
    /// organization's or the site's — said by name and left alone. Nothing
    /// here rewrites a number; it says which project, which ceiling it is
    /// above, and both of them (§FS-005-dispatch.24). A ceiling of `0` above
    /// is a pause the reader wrote on purpose rather than a budget anything
    /// can be above, so no pair is read out of it. Roots in flight is the
    /// nesting this reads: it is the ceiling an organization declares.
    fn inversions(&self) -> Vec<String> {
        let mut said = Vec::new();
        for (project, limits) in &self.projects {
            let Some(inner) = limits.concurrent else {
                continue;
            };
            let above = self
                .organization_of(project)
                .and_then(|organization| {
                    self.organizations
                        .get(organization)
                        .map(|outer| (format!("organizations.{organization}.work"), *outer))
                })
                .into_iter()
                .chain(
                    self.site_as_configured
                        .map(|site| ("global work".to_string(), site)),
                );
            for (named, outer) in above {
                if outer > 0 && inner > outer {
                    said.push(format!(
                        "projects.{project}.work.max_concurrent {inner} is above \
                         {named}.max_concurrent {outer}: the project number stands, and the \
                         ceiling above it still bounds its total"
                    ));
                }
            }
        }
        said
    }
}

/// Remaining slots at each scope, as one sweep spends them.
#[derive(Debug)]
struct Capacity {
    ceilings: Ceilings,
    /// The site's budgets and what they have spent — a second dimension asked
    /// at each scope beside that scope's own two (§FS-015-spend-ceiling.5).
    budgets: spend::Budgets,
    /// Whether a full budget refuses this sweep or is only said to it
    /// (§FS-015-spend-ceiling.6).
    budget: spend::Budget,
    global_live: Counts,
    organization_live: BTreeMap<String, usize>,
    project_live: BTreeMap<String, Counts>,
}

impl Capacity {
    fn new(ceilings: Ceilings, live: LiveRuns) -> Capacity {
        Capacity {
            ceilings,
            budgets: spend::Budgets::default(),
            budget: spend::Budget::Binds,
            global_live: live.global,
            organization_live: live.organizations,
            project_live: live.projects,
        }
    }

    /// The budgets this sweep reads, and whether a full one refuses it or is
    /// only said to whoever asked (§FS-015-spend-ceiling.6). Apart from
    /// [`Capacity::new`] because a site that wrote none is a site with none,
    /// and every existing question about capacity is asked exactly as it was.
    fn against(mut self, budgets: spend::Budgets, budget: spend::Budget) -> Capacity {
        self.budgets = budgets;
        self.budget = budget;
        self
    }

    /// The organizations a root's projects put it in, each once and in the
    /// order the root names its projects.
    fn organizations_of(&self, projects: &[String]) -> Vec<&str> {
        let mut found: Vec<&str> = Vec::new();
        for project in projects {
            if let Some(organization) = self.ceilings.organization_of(project) {
                if !found.contains(&organization) {
                    found.push(organization);
                }
            }
        }
        found
    }

    /// Why this root may not start, asked outermost first so the reason names
    /// the widest ceiling that was actually full (§FS-005-dispatch.24) — and
    /// within one scope in a fixed order: roots in flight, working roots,
    /// money, then tokens (§FS-015-spend-ceiling.5).
    ///
    /// A budget answers here only where this sweep is bound by one: where a
    /// person asked for it, [`Capacity::warning`] says what would have refused
    /// and nothing refuses (§FS-015-spend-ceiling.6).
    #[cfg(test)]
    fn refusal(&self, projects: &[String]) -> Option<String> {
        self.hold(projects).as_ref().map(Hold::says)
    }

    /// The same walk as [`Capacity::refusal`], answering the hold itself: the
    /// order here is the order the row's `hold` promises
    /// (§FS-005-dispatch.24.2).
    fn hold(&self, projects: &[String]) -> Option<Hold> {
        if let Some(why) = self
            .ceilings
            .site
            .full(self.global_live, &spend::Scope::Site)
        {
            return Some(why);
        }
        if let Some(why) = self.spent(&spend::Scope::Site) {
            return Some(why);
        }
        for organization in self.organizations_of(projects) {
            let count = self
                .organization_live
                .get(organization)
                .copied()
                .unwrap_or(0);
            if let Some(limit) = self
                .ceilings
                .organizations
                .get(organization)
                .filter(|limit| count >= **limit)
            {
                return Some(Hold::Concurrency {
                    scope: spend::Scope::Organization(organization.to_string()),
                    key: Flight::Concurrent,
                    limit: *limit,
                    count,
                });
            }
            if let Some(why) = self.spent(&spend::Scope::Organization(organization.to_string())) {
                return Some(why);
            }
        }
        for project in projects {
            if let Some(limits) = self.ceilings.projects.get(project) {
                let live = self.project_live.get(project).copied().unwrap_or_default();
                if let Some(why) = limits.full(live, &spend::Scope::Project(project.clone())) {
                    return Some(why);
                }
            }
            if let Some(why) = self.spent(&spend::Scope::Project(project.clone())) {
                return Some(why);
            }
        }
        None
    }

    /// One scope's budget, where it refuses this sweep. A sweep a person asked
    /// for is refused by no budget at all, so this answers nothing there and
    /// the walk carries on to the concurrency ceilings further in
    /// (§FS-015-spend-ceiling.6).
    fn spent(&self, scope: &spend::Scope) -> Option<Hold> {
        match self.budget {
            spend::Budget::Binds => self.budgets.at(scope).map(|full| full.hold(scope)),
            spend::Budget::Warns => None,
        }
    }

    /// What a person who asked for this sweep is told about a budget that is
    /// full: the outermost ceiling that would have refused, which is the one
    /// they would have been sent to (§FS-015-spend-ceiling.6).
    fn warning(&self, projects: &[String]) -> Option<&str> {
        match self.budget {
            spend::Budget::Warns => self.budgets.over(projects).map(|full| full.says.as_str()),
            spend::Budget::Binds => None,
        }
    }

    fn started(&mut self, projects: &[String], remains_live: bool) {
        if !remains_live {
            return;
        }
        // A run that has just begun is working: it takes a slot under both
        // ceilings, and only parking on a person's answer gives the second
        // one back (§FS-005-dispatch.24).
        self.global_live.took(true);
        let organizations: Vec<String> = self
            .organizations_of(projects)
            .into_iter()
            .map(str::to_string)
            .collect();
        for organization in organizations {
            *self.organization_live.entry(organization).or_default() += 1;
        }
        for project in projects {
            self.project_live
                .entry(project.clone())
                .or_default()
                .took(true);
        }
    }

    /// How capacity stands, for the reading the sweep prints
    /// (§FS-005-dispatch.24). The aggregate scope, because that is the one
    /// every root is counted in.
    fn standing(&self) -> Standing {
        Standing {
            live: self.global_live.live,
            active: self.global_live.active,
            parked: self.global_live.parked(),
            max_concurrent: self.ceilings.site.concurrent,
            max_active: self.ceilings.site.active,
        }
    }
}

/// What the sweep saw of the site's capacity when it was done
/// (§FS-005-dispatch.24): the live roots split into the ones being worked and
/// the ones parked on a person, beside the two ceilings that were in force.
/// The counts include what this sweep started, so it reads as the world the
/// sweep left behind rather than the one it found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Standing {
    pub live: usize,
    pub active: usize,
    pub parked: usize,
    pub max_concurrent: Option<usize>,
    pub max_active: Option<usize>,
}

impl Standing {
    /// The one line a reader gets: what is live, and how much of it is a
    /// person's turn rather than work being done.
    pub fn says(&self) -> String {
        let ceiling = |key: &str, limit: Option<usize>| match limit {
            Some(limit) => format!("work.{key} {limit}"),
            None => format!("work.{key} unlimited"),
        };
        format!(
            "capacity: {} live root(s) — {} active, {} parked ({}, {})",
            self.live,
            self.active,
            self.parked,
            ceiling("max_concurrent", self.max_concurrent),
            ceiling("max_active", self.max_active),
        )
    }
}

/// What one sweep came to (§FS-005-dispatch.24): what happened at every due
/// root, and how capacity stood when it was over. Two answers because the
/// second is a reading in its own right — a ceiling reported full must not be
/// the only thing said about the roots filling it.
#[derive(Debug, Default)]
pub struct Sweep {
    pub runs: Vec<Launched>,
    pub capacity: Standing,
    /// The budgets that were full and were not allowed to refuse, because a
    /// person asked for this sweep (§FS-015-spend-ceiling.6). Said once each,
    /// on the error stream where a warning belongs.
    pub warned: Vec<String>,
}

/// How a work root is named in the ledger's record of starts. The path as
/// written, so the key travels with the same spelling the entry carries.
fn root_key(root: &std::path::Path) -> String {
    root.to_string_lossy().into_owned()
}

/// One path, resolved to the file it names — the spelling a caller happened
/// to use is not the identity of a directory or a plan. A path that cannot
/// be resolved is its own answer: something that is not there yet is still
/// only equal to itself.
fn canonical(path: &std::path::Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The checkout a work root belongs to, where nothing in the ledger says: the
/// directory holding it, which is what a work root's own template renders
/// under.
fn root_checkout(root: &std::path::Path) -> PathBuf {
    root.parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.to_path_buf())
}

/// The working tree one root's work runs in: what the ledger recorded for
/// that root, and the directory holding it where nothing recorded anything
/// (§FS-005-dispatch.3). One definition, because the sweep and the manual key
/// both guard on it and two spellings of "which tree is this" would be two
/// guards (§AR-009-surfaces.1).
fn checkout_of(ledger: &Ledger, root: &std::path::Path) -> PathBuf {
    let identity = canonical(root);
    ledger
        .entries
        .values()
        .flat_map(|entry| {
            entry
                .dispatches
                .iter()
                .rev()
                .map(move |dispatch| (entry, dispatch))
        })
        .find(|(entry, dispatch)| {
            canonical(dispatch.root.as_ref().unwrap_or(&entry.root)) == identity
        })
        .map(|(entry, dispatch)| {
            dispatch
                .checkout
                .clone()
                .unwrap_or_else(|| entry.checkout())
        })
        .or_else(|| {
            ledger
                .entries
                .values()
                .find(|entry| entry.dispatches.is_empty() && canonical(&entry.root) == identity)
                .map(Entry::checkout)
        })
        .unwrap_or_else(|| root_checkout(root))
}

/// The branch recorded for work in one root, with the old entry-level field
/// as the legacy answer (§FS-005-dispatch.4, §FS-005-dispatch.30).
fn branch_of<'a>(ledger: &'a Ledger, root: &std::path::Path) -> Option<&'a str> {
    let root = canonical(root);
    ledger
        .entries
        .values()
        .flat_map(|entry| {
            entry
                .dispatches
                .iter()
                .rev()
                .map(move |dispatch| (entry, dispatch))
        })
        .find(|(entry, dispatch)| canonical(dispatch.root.as_ref().unwrap_or(&entry.root)) == root)
        .map(|(entry, dispatch)| dispatch.branch(entry))
        .unwrap_or_else(|| {
            ledger
                .entries
                .values()
                .find(|entry| entry.dispatches.is_empty() && canonical(&entry.root) == root)
                .and_then(|entry| entry.branch.as_deref())
        })
}

/// Every working tree a live run holds right now, each mapped to the work
/// root that run was started from (§FS-005-dispatch.24).
///
/// Read over the whole snapshot rather than over the roots something is
/// waiting to happen on: the root a run holds is usually not one of those —
/// it is working through what it already has — so a guard that only looked
/// among candidates would never see the run it is meant to keep out of the
/// way of.
///
/// Trees as the file system resolves them, because the guard is about one
/// directory and two roots may reach it by two spellings — a relative path, a
/// symbolic link — which must not come out as two answers.
pub fn live_checkouts(
    global: &WorkConfig,
    roots: &[runtime::watch::RootPlans],
    ledger: &Ledger,
) -> BTreeMap<PathBuf, PathBuf> {
    roots
        .iter()
        .filter(|group| runtime::watch::live(global, &group.root))
        .map(|group| {
            (
                canonical(&checkout_of(ledger, &group.root)),
                group.root.clone(),
            )
        })
        .collect()
}

/// The root a live run holds this working tree from, where one does
/// (§FS-005-dispatch.24). The lookup belongs beside [`live_checkouts`] rather
/// than at each call site: a caller that forgot to resolve the tree the same
/// way would have a guard that passes on a spelling.
pub fn holding<'a>(
    busy: &'a BTreeMap<PathBuf, PathBuf>,
    checkout: &std::path::Path,
) -> Option<&'a PathBuf> {
    busy.get(&canonical(checkout))
}

/// Note that a launch has taken this working tree, so the rest of the same
/// command sees it as busy (§FS-005-dispatch.24). Keyed exactly as [`holding`]
/// will look for it, beside it for the same reason the lookup is: a writer
/// that resolved the tree its own way would fill a map the reader misses.
pub fn take_checkout(
    taken: &mut BTreeMap<PathBuf, PathBuf>,
    checkout: &std::path::Path,
    root: &std::path::Path,
) {
    taken.insert(canonical(checkout), root.to_path_buf());
}

/// The one sentence said where a run asked for by name names a matter the
/// record has no work about at all (§FS-005-dispatch.30).
///
/// It quotes the id and names the two verbs that would give it work, because
/// the fault this ends was that a real matter and a typo came back as the
/// same bytes. An input that names nothing is refused by name and exits `2`,
/// like every other refusal of something a reader typed
/// (§FS-011-command-line.9).
pub fn no_work_recorded(item: &str) -> String {
    format!(
        "No work is recorded about '{item}'. Hand some over with 'ephor work dispatch \
         --item {item}', or lay a workflow with 'ephor work lay <entry> --item {item}'."
    )
}

/// The one sentence said where the record knows the matter and nothing in its
/// work is a run's to advance (§FS-005-dispatch.30).
///
/// Not a refusal: the command was understood and the answer is that the work
/// is over, claimed, or waiting on a person. So it names the matter and says
/// which of those it is, in the terms §FS-005-dispatch.24 already uses. Said
/// in one place because the command line and the screen's run key are one
/// ability and must answer alike (§REQ-002-parity.1).
pub fn nothing_to_run(item: Option<&str>) -> String {
    match item {
        Some(item) => format!(
            "Nothing to run for {item}: its work holds no task that is open, unclaimed \
             and not parked."
        ),
        None => "Nothing to run: no work holds a task that is open, unclaimed and not parked."
            .to_string(),
    }
}

/// What the screen's run key hands the runtime for one matter, or the one
/// sentence it shows instead (§FS-005-dispatch.30).
///
/// The key and `ephor work run` are one ability (§REQ-002-parity.1), so the
/// decision made from the reading is made once rather than at each surface:
/// the plans the record named, a refusal where a root the reading returned may
/// start nothing, and the matter's own no-work sentence where it returned
/// nothing at all. The screen runs one matter over one root, so a refusal
/// anywhere in this reading is a refusal of this key.
pub fn plans_to_run(due: &[Due], item: &str) -> std::result::Result<Vec<String>, String> {
    named_run_plans(due, Some(item))
}

/// What one reader-started named run may do, after root validity and live-run
/// safety have been considered in their one shared order (§FS-005-dispatch.30).
///
/// Root validity belongs to the work reading, so it answers before the live
/// lock is even probed and `--force` can never lift it. A root that passed that
/// reading reaches ordinary start safety; only there is a live run a refusal,
/// and only that refusal is lifted by `--force`. Both the command line and the
/// work screen call this decision, leaving each surface only to render its
/// answer (§REQ-002-parity.1).
pub fn named_run_decision<F>(
    due: &[Due],
    item: Option<&str>,
    force: bool,
    live_refusal: F,
) -> std::result::Result<Vec<String>, String>
where
    F: FnOnce() -> Option<String>,
{
    let plans = named_run_plans(due, item)?;
    if !force {
        if let Some(says) = live_refusal() {
            return Err(says);
        }
    }
    Ok(plans)
}

fn named_run_plans(due: &[Due], item: Option<&str>) -> std::result::Result<Vec<String>, String> {
    if let Some(refusal) = due.iter().find_map(|due| due.refusal.clone()) {
        return Err(refusal);
    }
    let plans: Vec<String> = due.iter().flat_map(|due| due.plans.clone()).collect();
    match plans.is_empty() {
        true => Err(nothing_to_run(item)),
        false => Ok(plans),
    }
}

/// The one sentence said where a run asked for by name lands on a root whose
/// machine will not read (§FS-005-dispatch.30, §FS-005-dispatch.15).
///
/// Finality and gating are the machine's words and there are none, so nothing
/// in the root can be called runnable. That is a refusal about the root and
/// not a report that the matter is over: it names the root, and it names the
/// move that ends it. It covers both ways the machine can be absent — no
/// `states.yaml` there at all, and one that will not parse — because neither
/// leaves anything to judge by.
pub fn no_machine_here(root: &std::path::Path) -> String {
    format!(
        "{}: this work root has no state machine that will read, so nothing in it can be \
         judged runnable. Install one with 'ephor work states > {}'.",
        root.display(),
        root.join("states.yaml").display()
    )
}

/// The one sentence said where a run asked for by name lands on a root whose
/// checkout is standing on another branch (§FS-005-dispatch.30,
/// §FS-005-dispatch.3).
///
/// Work about a branch belongs in that branch's working tree, and a tree
/// standing on another one holds different code. Dispatch refuses there
/// naming the branch the root is actually on, and so does this: the sweep can
/// drop such a root in silence because nobody is watching it, and a reader who
/// asked for this matter by name is.
pub fn checkout_standing_elsewhere(checkout: &std::path::Path, head: &str, wanted: &str) -> String {
    format!(
        "{}: this work belongs on {wanted} and the checkout is standing on {head}, so a run \
         here would edit different code. Put the branch there:\n  git -C {} switch {wanted}",
        checkout.display(),
        checkout.display()
    )
}

/// The one sentence said wherever a start is held back because a live run
/// already holds the tree (§FS-005-dispatch.24): the run in the way by the
/// name it published, and by the root holding it where it published none, so
/// the reader is sent to the run rather than to a guess (§AR-009-surfaces.1).
pub fn live_in_this_checkout(global: &WorkConfig, held_by: &std::path::Path) -> String {
    held_in_this_checkout(global, held_by).says()
}

/// The same hold as data: the root holding the tree, and the run by the id it
/// published where it published one (§FS-005-dispatch.24.2).
fn held_in_this_checkout(global: &WorkConfig, held_by: &std::path::Path) -> Hold {
    Hold::Tree {
        root: held_by.to_path_buf(),
        run: runtime::watch::identity(global, held_by).and_then(|run| run.id),
    }
}

/// The recipe a ticket id was written from. Ids are `<recipe>-<n>`
/// ([`plan::Plan::next_ticket_id`]), so the recipe is readable off a ticket
/// nobody recorded a dispatch for (§FS-005-dispatch.24).
fn recipe_of_ticket(id: &str) -> Option<&str> {
    let (recipe, number) = id.rsplit_once('-')?;
    (!recipe.is_empty() && !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
        .then_some(recipe)
}

/// The next ticket id across every committed root of one matter
/// (§FS-005-dispatch.3). The plan is included for compatibility with work a
/// person or older ephor wrote without a matching dispatch record.
fn next_ticket_id(entry: Option<&Entry>, plan: Option<&Plan>, recipe: &str) -> String {
    let number = |id: &str| {
        id.strip_prefix(recipe)?
            .strip_prefix('-')?
            .parse::<u32>()
            .ok()
    };
    let recorded = entry
        .into_iter()
        .flat_map(|entry| entry.dispatches.iter())
        .filter_map(|dispatch| number(&dispatch.ticket));
    let on_plan = plan
        .into_iter()
        .flat_map(Plan::tickets)
        .filter_map(|ticket| number(&ticket.id));
    format!(
        "{recipe}-{}",
        recorded.chain(on_plan).max().unwrap_or(0) + 1
    )
}

/// Every root holding this matter's recipe plan, in dispatch order and once
/// each. Legacy dispatches use the entry-level placement
/// (§FS-005-dispatch.4, §FS-005-dispatch.15.1).
fn recipe_roots(entry: &Entry) -> Vec<PathBuf> {
    recorded_recipe_plans(entry)
        .into_iter()
        .map(|plan| plan.root)
        .collect()
}

/// The normalized recipe-plan placements all consumers use. A dispatch from
/// before provenance existed falls back to the entry's singular fields; new
/// records retain the exact root, checkout and branch used
/// (§FS-005-dispatch.4, §FS-005-dispatch.15.1).
pub fn recorded_recipe_plans(entry: &Entry) -> Vec<RecordedPlan> {
    let mut plans = recipe_placements(entry);
    if plans.is_empty() {
        plans.extend(legacy_placement(entry));
    }
    plans
}

/// The recipe placements alone, before the legacy fallback below is
/// considered. Split out so that the widened reading can apply that fallback
/// over *its* list rather than over this one: a matter whose only dispatch is
/// a workflow has no recipe placement at all, so a fallback applied here
/// would fire beside the plan that workflow laid rather than instead of it,
/// and the row would name a task out of a file ephor never wrote
/// (§FS-005-dispatch.19, §FS-005-dispatch.35).
fn recipe_placements(entry: &Entry) -> Vec<RecordedPlan> {
    let mut plans: Vec<RecordedPlan> = Vec::new();
    for dispatch in entry
        .dispatches
        .iter()
        .filter(|dispatch| !dispatch.is_workflow())
    {
        let root = dispatch.root.clone().unwrap_or_else(|| entry.root.clone());
        let path = match dispatch.root.is_some() {
            true => plan::plan_path_in(&root, &entry.plan_id),
            false => entry.plan.clone(),
        };
        let checkout = dispatch
            .checkout
            .clone()
            .unwrap_or_else(|| entry.checkout());
        let branch = dispatch.branch(entry).map(str::to_string);
        if let Some(existing) = plans
            .iter_mut()
            .find(|have| canonical(&have.root) == canonical(&root))
        {
            existing.checkout = checkout;
            existing.branch = branch;
            existing.path = path;
            continue;
        }
        plans.push(RecordedPlan {
            root,
            checkout,
            branch,
            plan_id: entry.plan_id.clone(),
            path,
            laid: None,
        });
    }
    plans
}

/// The entry's own singular placement, for a record whose dispatches name no
/// plan this reading can place — a ledger written before provenance existed
/// (§FS-005-dispatch.4). A fallback and never an addition: it is read only
/// where nothing else was, so nothing that reads today stops reading and
/// nothing that reads a plan gains a second one beside it.
fn legacy_placement(entry: &Entry) -> Option<RecordedPlan> {
    entry.plan.is_file().then(|| RecordedPlan {
        root: entry.root.clone(),
        checkout: entry.checkout(),
        branch: entry.branch.clone(),
        plan_id: entry.plan_id.clone(),
        path: entry.plan.clone(),
        laid: None,
    })
}

/// The plans this matter's workflows laid beside its own, as the record names
/// them and the disk answers (§FS-005-dispatch.35): the *dispatch's* own root
/// joined to the name it recorded, resolved to the plan that is actually
/// there — whose id is the laid plan's, never the entry's.
///
/// The same resolution [`WorkAt::going`] and the due sweep already make, in
/// one place so that what a run reaches and what a reading counts cannot come
/// apart (§FS-005-dispatch.30). A name nobody can read is not dropped
/// silently: it is counted, because an entry whose plan has gone is a
/// different fact from an entry whose work is finished.
pub fn recorded_workflow_plans(entry: &Entry) -> RecordedWork {
    let mut work = RecordedWork::default();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    for dispatch in entry
        .dispatches
        .iter()
        .filter(|dispatch| dispatch.is_workflow())
    {
        let Some(name) = dispatch.plan.as_deref() else {
            continue;
        };
        let root = dispatch.root.clone().unwrap_or_else(|| entry.root.clone());
        let output = root.join(name);
        // One plan laid twice under one name is one plan: a repeat lays the
        // same workspace again and the record keeps both asks
        // (§FS-005-dispatch.19), but there is one body of work on disk.
        if !seen.insert(canonical(&output)) {
            continue;
        }
        let Some(found) = runtime::workflow::laid(&output) else {
            work.unread += 1;
            continue;
        };
        work.plans.push(RecordedPlan {
            root,
            checkout: dispatch
                .checkout
                .clone()
                .unwrap_or_else(|| entry.checkout()),
            branch: dispatch.branch(entry).map(str::to_string),
            plan_id: found.plan_id,
            path: found.path,
            laid: Some(name.to_string()),
        });
    }
    work
}

/// The whole of one matter's work, which is what every surface reads it from
/// (§FS-005-dispatch.35, §FS-005-dispatch.30): the recipe placements and the
/// laid workflow plans, in the record's own order.
///
/// Deliberately not the same function as [`recorded_recipe_plans`] and
/// deliberately not built on its deduplication. That reading answers the
/// drafted-reply contract (§FS-005-dispatch.13) and the board's one-plan-per-
/// root slot, and it collapses placements by root — which is right for the
/// plan ephor wrote, one per root, and wrong the moment a workflow lays a
/// second plan in a root the matter's own already stands in.
pub fn recorded_plans(entry: &Entry) -> RecordedWork {
    let mut work = recorded_workflow_plans(entry);
    let mut plans = recipe_placements(entry);
    // The legacy fallback fires only where this reading placed nothing at
    // all, and this reading is the widened one: an entry whose work is a
    // plan a workflow laid has no recipe placement, and reading its own plan
    // path beside that laid plan would put a file ephor never wrote into the
    // matter's work (§FS-005-dispatch.19, §FS-005-dispatch.35). A name
    // nobody could read counts as placed for this question: the record did
    // name a plan, so this is not the provenance-less ledger the fallback is
    // for, and a stray file at the entry's own path would otherwise answer
    // for the laid plan that is gone.
    if plans.is_empty() && work.plans.is_empty() && work.unread == 0 {
        plans.extend(legacy_placement(entry));
    }
    plans.extend(work.plans);
    work.plans = plans;
    work
}

/// An entry's work as it stands, read from the plan (§FS-005-dispatch.4). A
/// free function so a test can read a plan back the way every surface does.
pub fn status_of_entry(global: &WorkConfig, entry: &Entry, item: Option<&Item>) -> WorkStatus {
    status_of_entry_seen(global, entry, item, &mut RootLook::default())
}

/// The reads one work root answers the same way for every ticket in it —
/// whether a run is live on it, what that run has in hand, and how long it has
/// been silent — taken once and reused (§FS-005-dispatch.15.1).
///
/// A caller reading one matter can let this be built and dropped around the
/// call; a caller reading every matter on the feed keeps one across them, so a
/// root shared by two matters is probed once rather than twice.
#[derive(Default)]
pub struct RootLook {
    seen: BTreeMap<PathBuf, RootRun>,
}

/// One root's run, as the probe found it.
struct RootRun {
    live: bool,
    /// What the run says it has in hand. None where nothing is live: a slot
    /// nobody released under a free lock is a dead run's leavings, which is
    /// the board's business and not a running mark (§FS-005-dispatch.15).
    witness: Option<runtime::watch::Witness>,
    quiet: Option<u64>,
}

impl RootLook {
    fn of(&mut self, global: &WorkConfig, root: &std::path::Path) -> &RootRun {
        self.seen.entry(root.to_path_buf()).or_insert_with(|| {
            // Liveness and the quiet clock in one probe, the same one the
            // board makes (§FS-005-dispatch.15).
            let pulse = runtime::watch::pulse(global, root);
            RootRun {
                witness: pulse.live.then(|| runtime::watch::witness(global, root)),
                quiet: pulse.quiet,
                live: pulse.live,
            }
        })
    }
}

/// An entry's work as it stands, with the per-root reads hoisted out
/// (§FS-005-dispatch.15.1).
pub fn status_of_entry_seen(
    global: &WorkConfig,
    entry: &Entry,
    item: Option<&Item>,
    look: &mut RootLook,
) -> WorkStatus {
    let recipes: BTreeMap<&str, &str> = entry
        .dispatches
        .iter()
        .map(|dispatch| (dispatch.ticket.as_str(), dispatch.recipe.as_str()))
        .collect();
    // When each ticket was asked for (§FS-005-dispatch.18). The ledger is the
    // only thing that knows: the plan tracks what the work reached, never
    // when it was handed over ([§4]). Dispatched twice under one id, the
    // later ask stands — it is the one the reader last pressed.
    let asked: BTreeMap<&str, DateTime<Utc>> = entry
        .dispatches
        .iter()
        .map(|dispatch| (dispatch.ticket.as_str(), dispatch.at))
        .collect();
    // Every plan the record says is this matter's, the ones ephor wrote and
    // the ones its workflows laid (§FS-005-dispatch.35). A count taken over
    // the recipe plans alone reports no open tickets while a laid plan on
    // disk says otherwise, which is the watch reporting on itself
    // (§FS-005-dispatch.4).
    let RecordedWork {
        plans,
        unread: unread_workflows,
    } = recorded_plans(entry);
    let mut tickets: Vec<TicketStatus> = Vec::new();
    // The plan each ticket was read out of, kept beside them so a report with
    // room for one line can name it (§FS-005-dispatch.35).
    let mut held_in: Vec<PathBuf> = Vec::new();
    let mut missing = false;
    let mut quiet = None;
    for recorded in &plans {
        // The machine that answers for this plan's tasks, never assumed to be
        // the root's (§FS-005-dispatch.28): a laid workspace carries its own,
        // and the root's would misjudge which of its states are final. A
        // store declaring a machine that will not read judges nothing.
        let own = plan::own_machine(&recorded.path);
        let machine = match own {
            Ok(Some(store)) => Some(store),
            Ok(None) => WorkRoot::open(&recorded.root).ok().flatten(),
            Err(_) => None,
        };
        // Where the runtime writes what a ticket left behind: beside the plan
        // where the plan is a store of its own, and in the root otherwise.
        let artifacts = plan::own_store(&recorded.path).unwrap_or(&recorded.root);
        let plan = Plan::read(&recorded.path).ok().flatten();
        missing |= plan.is_none();
        // What a run on this root is doing, read once for every ticket asked
        // about there (§FS-005-dispatch.15.1).
        let run = look.of(global, &recorded.root);
        let live = run.live;
        quiet = quiet.or(run.quiet);
        let lock_born = live
            .then(|| runtime::watch::lock_born(&recorded.root))
            .flatten();
        let holds = |ticket: &plan::PlanTicket| {
            run.witness.as_ref().is_some_and(|witness| {
                witness.holds(
                    &recorded.root,
                    lock_born,
                    &recorded.plan_id,
                    &ticket.id,
                    ticket.state.as_deref(),
                )
            })
        };
        for ticket in plan.into_iter().flat_map(|plan| plan.tickets()) {
            let running = holds(&ticket);
            tickets.push(TicketStatus {
                running,
                queued: live && !running,
                asked: asked.get(ticket.id.as_str()).copied(),
                recipe: recipes
                    .get(ticket.id.as_str())
                    .map(|recipe| recipe.to_string())
                    .unwrap_or_else(|| ticket.id.clone()),
                finished: ticket
                    .state
                    .as_deref()
                    .is_some_and(|state| machine.as_ref().is_some_and(|root| root.is_final(state))),
                cancelled: ticket.cancelled()
                    && machine
                        .as_ref()
                        .is_some_and(|root| root.cancel_state().is_some()),
                waiting: ticket.state.as_deref().is_some_and(|state| {
                    machine.as_ref().is_some_and(|root| root.is_gating(state))
                }),
                verdict: runtime::results::verdict(artifacts, &recorded.plan_id, &ticket.id)
                    .or_else(|| {
                        ticket
                            .cancelled()
                            .then(|| {
                                runtime::results::result(artifacts, &recorded.plan_id, &ticket.id)
                            })
                            .flatten()
                    }),
                assignee: ticket.assignee,
                pinned: ticket.pinned,
                id: ticket.id,
                title: ticket.title,
                state: ticket.state,
            });
            held_in.push(recorded.path.clone());
        }
    }
    // What the matter is still at, ranked as the badge ranks it: a task
    // waiting on a person stands ahead of anything else (§FS-005-dispatch.9),
    // and otherwise the last one nothing has finished.
    let open_at = tickets
        .iter()
        .position(|ticket| ticket.waiting)
        .or_else(|| tickets.iter().rposition(|ticket| !ticket.finished))
        .map(|at| OpenWork {
            plan: held_in[at].clone(),
            ticket: tickets[at].id.clone(),
            state: tickets[at].state.clone().unwrap_or_else(|| "?".to_string()),
        });
    let advance = tickets.iter().find(|ticket| ticket.waiting).map(|ticket| {
        runtime::advance_command(global, &ticket.id, ticket.state.as_deref().unwrap_or("?"))
    });
    WorkStatus {
        workflows: entry
            .dispatches
            .iter()
            .filter(|dispatch| dispatch.is_workflow())
            .count(),
        project: entry.project.clone(),
        root: entry.root.clone(),
        plan_id: entry.plan_id.clone(),
        checkout: entry.checkout(),
        plan: entry.plan.clone(),
        plans,
        // A plan is missing where the record named one and nobody can read
        // it. That is the recipe plan ephor wrote and lost, and — since
        // §FS-005-dispatch.35 — a laid plan the record points at too: ephor
        // did not write that one, but it did say where it is, and a file
        // nobody can read is not evidence the work is over. An entry whose
        // dispatches named no plan at all still has none to be missing, and
        // is not alarmed about a file nobody promised (§FS-005-dispatch.19).
        missing: missing || unread_workflows > 0,
        unread_workflows,
        tickets,
        open_at,
        advance,
        changes: item
            .map(|item| entry.changes_since(item))
            .unwrap_or_default(),
        quiet,
    }
}

/// Cancel one ticket of one plan (§FS-005-dispatch.16): refuse on what the
/// artifacts already say — no runner bound, no such ticket, a ticket already
/// over, a machine with no abandonment state, a ticket a live run holds — and
/// otherwise ask the runtime for the move in its own words, and report what
/// that leaves waiting. A free function so the command line and the interface
/// make one and the same set of refusals in one and the same order
/// (§DA-005-cancel-is-the-runtimes-move). `why` empty is the reason left
/// unsaid, recorded as exactly that.
pub fn cancel_ticket(
    config: &WorkConfig,
    root: &std::path::Path,
    plan_id: &str,
    plan_path: &std::path::Path,
    ticket: &str,
    why: &str,
    dry_run: bool,
) -> Result<Cancelled> {
    // The move is the runtime's; with nobody to make it the plan is left as
    // it is, and the refusal is the workable rung's sentence
    // (§AR-005-capabilities.2).
    if let Some(refusal) = runtime::refusal(config) {
        return Err(EphorError::Command(refusal));
    }
    let plan = Plan::read(plan_path)?.ok_or_else(|| {
        EphorError::Command(format!(
            "the plan {} is gone — nothing there to cancel",
            plan_path.display()
        ))
    })?;
    let found = plan.ticket(ticket).ok_or_else(|| {
        let known: Vec<String> = plan.tickets().into_iter().map(|t| t.id).collect();
        EphorError::Command(format!(
            "{} holds no ticket '{ticket}' (it has: {})",
            plan_path.display(),
            match known.is_empty() {
                true => "none".to_string(),
                false => known.join(", "),
            }
        ))
    })?;
    let from = found.state.clone().ok_or_else(|| {
        EphorError::Command(format!(
            "{ticket} declares no state, so there is nothing to move it from"
        ))
    })?;
    // A machine that cannot say what final means cannot say what cancelled
    // means either; and one that declares no abandonment state has nowhere to
    // put the ticket (§FS-005-dispatch.6).
    let machine = WorkRoot::open(root)?.ok_or_else(|| {
        EphorError::Command(format!(
            "{} declares no state machine, so no state there is one to cancel into",
            root.display()
        ))
    })?;
    if found.cancelled() && machine.cancel_state().is_some() {
        return Err(EphorError::Command(format!(
            "{ticket} is already cancelled"
        )));
    }
    if machine.is_final(&from) {
        return Err(EphorError::Command(format!(
            "{ticket} is already over — it sits in '{from}', which is final"
        )));
    }
    if machine.cancel_state().is_none() {
        return Err(EphorError::Command(format!(
            "the machine '{}' in {} declares no final '{}' state to cancel into (it has: {}) — \
             add one and a transition into it from anywhere (`ephor work states` prints ephor's, \
             which has both), or move the ticket by hand",
            machine.machine,
            root.display(),
            plan::CANCELLED,
            machine.state_names().join(", ")
        )));
    }
    // A ticket a live run holds is that run's to finish (§FS-005-dispatch.16):
    // moving it out from under the agent is interfering with the run, which
    // is not this key's to do (§FS-005-dispatch.15).
    if runtime::watch::held_by_live_run(config, root, plan_id, ticket, &from) {
        return Err(EphorError::Command(format!(
            "{ticket} is held by a live run in '{from}' — the run is its to finish; wait for it, \
             or stop the run where it is running, then cancel"
        )));
    }
    let left_waiting: Vec<String> = plan
        .tickets()
        .into_iter()
        .filter(|other| {
            other.id != ticket
                && other.prior.iter().any(|prior| prior == ticket)
                && !other
                    .state
                    .as_deref()
                    .is_some_and(|state| machine.is_final(state))
        })
        .map(|other| other.id)
        .collect();
    let why = match why.trim().is_empty() {
        true => CANCELLED_UNSAID,
        false => why.trim(),
    };
    if !dry_run {
        runtime::cancel(config, root, plan_path, ticket, &from, why)?;
    }
    Ok(Cancelled {
        ticket: ticket.to_string(),
        from,
        plan: plan_path.to_path_buf(),
        left_waiting,
    })
}

/// Where an item's work goes for a project: its own template, its
/// organization's, or the site's — the innermost one written answers, and the
/// scopes above it are not consulted (§FS-005-dispatch.6.1). A free function
/// because `ephor checkout` resolves the same place
/// (§FS-006-project-interface.7) and two answers to "where does work live"
/// would eventually disagree.
pub fn root_template(
    global: &WorkConfig,
    organization: Option<&OrganizationWorkConfig>,
    project: Option<&ProjectWorkConfig>,
) -> String {
    project
        .and_then(|work| work.root.clone())
        .or_else(|| organization.and_then(|work| work.root.clone()))
        .unwrap_or_else(|| global.root.clone())
}

/// The board's universe (§FS-005-dispatch.15): one group per execution root —
/// the ledger's plans first, since they carry the matter behind them, and
/// every plan enumeration finds after, item-less where nothing dispatched it.
/// Enumeration resolves the work-root template at each configured place: the
/// project's own checkout and each branch workspace on disk, because a
/// branch-addressable project keeps a work root per branch workspace and each
/// one is its own execution root. A template naming a placeholder only an
/// item can fill is skipped rather than guessed — work written through it is
/// the ledger's to know. Reading only, and bounded: one directory listing per
/// candidate work root, no plan opened, no repository entered, no runner
/// asked (§FS-005-dispatch.15.1, §AR-007-runtime.3).
pub fn enumerate_roots(
    global: &WorkConfig,
    organizations: &BTreeMap<String, OrganizationWorkConfig>,
    projects: &BTreeMap<String, ProjectWorkConfig>,
    placements: &[Placement],
    ledger: &Ledger,
) -> Vec<runtime::watch::RootPlans> {
    use runtime::watch::{PlanRef, RootPlans};
    // One row per execution root means one per *directory*, not one per
    // spelling: a workspace template that symlinks back to the checkout
    // renders the same root under two names, and the runtime's lock is on
    // the directory, so two groups here would be one operation shown twice.
    let canon = canonical;
    let mut groups: BTreeMap<PathBuf, RootPlans> = BTreeMap::new();
    for (item_id, entry) in &ledger.entries {
        // Every dispatch is a durable placement seed. Older records carry no
        // placement and therefore fall back to the item-level fields they
        // were written with (§FS-005-dispatch.4,
        // §FS-005-dispatch.15.1).
        for dispatch in &entry.dispatches {
            let recorded_root = dispatch.root.as_ref().unwrap_or(&entry.root);
            let root = canon(recorded_root);
            let group = groups.entry(root.clone()).or_insert_with(|| RootPlans {
                root,
                plans: Vec::new(),
            });
            let found = match dispatch.plan.as_deref() {
                Some(plan_id) => runtime::workflow::laid(&recorded_root.join(plan_id))
                    .map(|found| (found.plan_id, found.path)),
                None => {
                    let path = match dispatch.root.is_some() {
                        true => plan::plan_path_in(recorded_root, &entry.plan_id),
                        false => entry.plan.clone(),
                    };
                    Some((entry.plan_id.clone(), path))
                }
            };
            let Some((plan_id, path)) = found else {
                continue;
            };
            let path_key = canon(&path);
            if group.plans.iter().any(|plan| canon(&plan.path) == path_key) {
                continue;
            }
            group.plans.push(PlanRef {
                project: entry.project.clone(),
                plan_id,
                path,
                item: Some(item_id.clone()),
                title: entry.title.clone(),
            });
        }
        // Keep the current item-level placement as a seed as well.  A ledger
        // append can move an entry to a root whose dispatch history predates
        // that placement (and older records only have these fields); retaining
        // it keeps the named matter reachable without widening discovery.
        if !entry.dispatches.is_empty() && entry.plan.is_file() {
            let root = canon(&entry.root);
            let group = groups.entry(root.clone()).or_insert_with(|| RootPlans {
                root,
                plans: Vec::new(),
            });
            let path_key = canon(&entry.plan);
            if !group.plans.iter().any(|plan| canon(&plan.path) == path_key) {
                group.plans.push(PlanRef {
                    project: entry.project.clone(),
                    plan_id: entry.plan_id.clone(),
                    path: entry.plan.clone(),
                    item: Some(item_id.clone()),
                    title: entry.title.clone(),
                });
            }
        }
        // A defensive compatibility path for a ledger entry with no dispatch
        // history at all: if its old item-level plan exists, retain the exact
        // bounded reading the previous format provided (§FS-005-dispatch.4).
        if entry.dispatches.is_empty() && entry.plan.is_file() {
            let root = canon(&entry.root);
            groups
                .entry(root.clone())
                .or_insert_with(|| RootPlans {
                    root,
                    plans: Vec::new(),
                })
                .plans
                .push(PlanRef {
                    project: entry.project.clone(),
                    plan_id: entry.plan_id.clone(),
                    path: entry.plan.clone(),
                    item: Some(item_id.clone()),
                    title: entry.title.clone(),
                });
        }
    }
    for placement in placements {
        let organization = placement.organization.as_ref();
        let template = root_template(
            global,
            organization.and_then(|org| organizations.get(&org.id)),
            projects.get(&placement.project),
        );
        // And the person's own root, where the site declares one: work about a
        // private matter is written there whatever the project's ladder says,
        // and the board finds it by looking as it finds any other
        // (§FS-018-private-sources.2, §FS-005-dispatch.15).
        let private = global
            .private
            .as_ref()
            .and_then(|private| private.root.clone());
        // A template that ignores the workspace renders every place to one
        // root; listing it once is enough.
        let mut listed: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
        for template in std::iter::once(template).chain(private) {
            let mut places = vec![placement.root.clone()];
            for branch in &placement.branches {
                places.extend(placement.workspace_for(&branch.branch));
            }
            // An organization's root is a place of its own: a work root reaching
            // above the project sits inside no checkout, so every place the
            // project offers can be absent while the root the template names is
            // there (§FS-005-dispatch.6.1). Seeded only where the template
            // actually reaches for it, so a site naming no organization
            // placeholder enumerates exactly the roots it enumerated before
            // (§FS-005-dispatch.15.1).
            if dossier::named(&template)
                .iter()
                .any(|name| dossier::ORGANIZATION_PLACEHOLDERS.contains(&name.as_str()))
            {
                places.extend(organization.and_then(|org| org.root.clone()));
            }
            places.sort();
            places.dedup();
            for place in places {
                if !place.is_dir() {
                    continue;
                }
                let mut values = dossier::fixed([
                    ("workspace", place.to_string_lossy().into_owned()),
                    ("root", placement.root.to_string_lossy().into_owned()),
                    ("project", placement.project.clone()),
                ]);
                // Only what the registry answers goes in. An organization
                // placeholder nothing answers is left standing and skipped by the
                // guard below, exactly as a field only an item can fill is: a
                // template dispatch would have refused wrote nothing to find
                // (§FS-005-dispatch.15.1).
                if let Some(organization) = organization {
                    values.insert(std::borrow::Cow::Borrowed("org"), organization.id.clone());
                    if let Some(root) = &organization.root {
                        values.insert(
                            std::borrow::Cow::Borrowed("org_root"),
                            root.to_string_lossy().into_owned(),
                        );
                    }
                }
                let rendered = dossier::render(&template, &values);
                if rendered.contains('{') {
                    continue;
                }
                let root = canon(&crate::paths::resolve_path(&rendered));
                if !listed.insert(root.clone()) {
                    continue;
                }
                for found in plan::plans_in(&root) {
                    let group = groups.entry(root.clone()).or_insert_with(|| RootPlans {
                        root: root.clone(),
                        plans: Vec::new(),
                    });
                    // The ledger may spell the same plan through a different
                    // alias; a plan is the file, not the spelling.
                    if group
                        .plans
                        .iter()
                        .any(|plan| canon(&plan.path) == found.path)
                    {
                        continue;
                    }
                    group.plans.push(PlanRef {
                        project: placement.project.clone(),
                        plan_id: found.plan_id,
                        path: found.path,
                        item: None,
                        title: String::new(),
                    });
                }
            }
        }
    }
    groups.into_values().collect()
}

/// The states YAML installed into a work root that has none: the project's
/// own, the global one, or the machine ephor ships.
pub fn states_yaml(global: &WorkConfig, project: Option<&ProjectWorkConfig>) -> Result<String> {
    let configured = project
        .and_then(|work| work.states.clone())
        .or_else(|| global.states.clone());
    match configured {
        Some(path) => {
            let path = crate::paths::resolve_path(&path);
            std::fs::read_to_string(&path).map_err(|err| {
                EphorError::Command(format!(
                    "Cannot read the configured state machine {}: {err}",
                    path.display()
                ))
            })
        }
        None => Ok(plan::SHIPPED_STATES.to_string()),
    }
}

/// A work root ephor made or found, and what it could not do on the way
/// (§FS-006-project-interface.7).
pub struct Store {
    pub dir: PathBuf,
    /// Whether this call is what put it there. False where it was already on
    /// disk, which is what lets a checkout asked for twice say what it did
    /// rather than say the same thing twice (§FS-004-quick-actions.7.1).
    pub made: bool,
    /// What the runtime said when it could not make its own project there.
    /// None where it did — and never an error, because the workspace around
    /// this one directory is whole either way (§FS-004-quick-actions.7).
    pub note: Option<String>,
}

/// Make the work root for a workspace, so the first dispatch into that branch
/// has somewhere to land and what is under way is visible from the moment the
/// tree exists (§FS-006-project-interface.7).
///
/// The runtime makes its own project and ephor says where: the directory is
/// the work root resolved here, and what a project in it consists of is the
/// runner's answer rather than a copy of that answer kept in ephor. Ephor's
/// own state machine goes in beside what the runner wrote, and the store's
/// self-ignore is added whatever the runner's project says about version
/// control — that is what keeps this from being an artifact required of the
/// project (§REQ-001-boundary.3): what it holds is ephor's own planning state
/// that happens to live in a checkout.
pub fn ensure_store(
    global: &WorkConfig,
    organization: Option<&OrganizationWorkConfig>,
    project: Option<&ProjectWorkConfig>,
    project_id: &str,
    placed_in: Option<&crate::branches::Organization>,
    workspace: &std::path::Path,
    root: &std::path::Path,
) -> Result<Store> {
    let dir = work_root_in(
        global,
        organization,
        project,
        project_id,
        placed_in,
        workspace,
        root,
    )?;
    ensure_store_at(global, project, &dir)
}

/// Initialize a work root whose template has already been selected by a
/// recipe or workflow entry. The caller has rendered it after branch
/// placement, so no wider project-level template may replace it here.
pub fn ensure_store_at(
    global: &WorkConfig,
    project: Option<&ProjectWorkConfig>,
    dir: &std::path::Path,
) -> Result<Store> {
    let states = states_yaml(global, project)?;
    let made = !dir.is_dir();
    // The directory first: the runner is asked to make a place that is there,
    // and what ephor installs afterwards reads what the runner left rather than
    // racing it.
    plan::create_dir(&dir)?;
    let note = match runtime::init(global, &dir) {
        runtime::Initialized::Project => None,
        runtime::Initialized::Refused(why) => Some(why),
    };
    WorkRoot::ensure(&dir, &states)?;
    Ok(Store {
        dir: dir.to_path_buf(),
        made,
        note,
    })
}

/// Where work resolves to, from the template, with nothing created
/// (§FS-005-dispatch.6.1).
///
/// The resolution half of [`ensure_store`], which is this plus making the
/// directory and installing the machine. A caller that only reads what is
/// already there asks this instead, so a sweep held at the gate creates
/// nothing on its way to reporting (§FS-011-command-line.10).
pub fn work_root_in(
    global: &WorkConfig,
    organization: Option<&OrganizationWorkConfig>,
    project: Option<&ProjectWorkConfig>,
    project_id: &str,
    placed_in: Option<&crate::branches::Organization>,
    workspace: &std::path::Path,
    root: &std::path::Path,
) -> Result<PathBuf> {
    let template = root_template(global, organization, project);
    // The same refusal the dispatch makes, in the same words: a work root
    // reaching above the project has no answer here either, and a checkout
    // must not make a directory called `{org_root}` for a plan to land in
    // (§FS-005-dispatch.6.1).
    if let Some(why) = dossier::organization_gap(&template, project_id, placed_in) {
        return Err(EphorError::Command(why));
    }
    let mut values = dossier::fixed([
        ("workspace", workspace.to_string_lossy().into_owned()),
        ("root", root.to_string_lossy().into_owned()),
        ("project", project_id.to_string()),
    ]);
    if let Some(placed_in) = placed_in {
        values.insert(std::borrow::Cow::Borrowed("org"), placed_in.id.clone());
        if let Some(root) = &placed_in.root {
            values.insert(
                std::borrow::Cow::Borrowed("org_root"),
                root.to_string_lossy().into_owned(),
            );
        }
    }
    Ok(crate::paths::resolve_path(&dossier::render(
        &template, &values,
    )))
}

/// The state a hand-written ask starts in when the reader names none: the
/// shipped machine's working state, which is also the shipped recipes'.
const WORKING_STATE: &str = "fix";

/// A ticket title out of what was asked: its first line, cut at a word so it
/// reads in a list beside the item's own title. The whole of what was asked is
/// in the ticket body regardless; this is only the label.
fn summarize(words: &str) -> String {
    const LABEL: usize = 56;
    let first = words.lines().next().unwrap_or("").trim();
    if first.chars().count() <= LABEL {
        return first.to_string();
    }
    let kept: String = first.chars().take(LABEL).collect();
    let cut = kept.rfind(' ').unwrap_or(kept.len());
    format!("{}…", kept[..cut].trim_end())
}

fn clamp(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    text.chars().take(limit - 1).collect::<String>() + "…"
}

/// What a recipe's deterministic opening move reached
/// (§FS-005-dispatch.12).
enum Opening {
    /// The recipe declares none, or there was nothing here for it to do.
    None,
    /// It finished: there is nothing left to hand over, and so nothing to
    /// render either — a clean move is told by [`Outcome::describe`] and its
    /// report reaches no reader (§FS-005-dispatch.12).
    Finished,
    /// It stopped, and this is the situation the ticket is about — as a
    /// paragraph of the plan body it becomes, not as a document of its own
    /// (§FS-005-dispatch.3).
    Stopped(String),
}

/// What one workflow entry would write, with nothing written yet
/// (§FS-005-dispatch.19).
pub struct Laying {
    /// The entry that names the workflow — the menu's own id, and the key a
    /// hands table answers by (§FS-006-project-interface.9).
    pub entry: String,
    pub workflow: runtime::workflow::Workflow,
    /// Every input, answered, and where each answer came from.
    pub answered: crate::work::workflow::Answered,
    /// The plan this would lay down, by the id the runtime will know it by.
    pub plan_id: String,
    /// Where it goes, under the item's own work root.
    pub output: PathBuf,
    /// Values files require a runtime validation pass before destination
    /// creation, so a rejected input cannot leave a partial workspace.
    preflight_runtime: bool,
    site: Site,
    /// What choosing among the entry's hands had to say
    /// (§FS-005-dispatch.29).
    pub said: Option<String>,
    /// The pool the chosen hand's work is bought against.
    pub pool: Option<String>,
    /// Every pool this laying's work is bought against, at once — derived
    /// from the hands ephor resolved for it and recorded with the laying,
    /// because the sweep that starts the plan never sees the entry that laid
    /// it (§FS-005-dispatch.19, §FS-005-dispatch.33).
    pub pools: Vec<String>,
    /// Why this site cannot have all of them at once, where it cannot: the
    /// one clause every surface wears its own verb in front of
    /// (§FS-005-dispatch.33).
    pub held: Option<headroom::Held>,
}

impl Laying {
    /// The work root the plan lands in.
    pub fn root(&self) -> &std::path::Path {
        &self.site.dir
    }

    /// Whether this can be laid down as it stands.
    pub fn ready(&self) -> bool {
        self.refusal().is_none()
    }

    /// Why it cannot, where it cannot (§FS-005-dispatch.19). Asked before
    /// anything is written — and asked on its own by a sweep's dry run,
    /// which writes nothing at all and must still refuse everything the real
    /// laying would (§FS-005-dispatch.28).
    pub fn refusal(&self) -> Option<String> {
        if !self.answered.refusals.is_empty() {
            return Some(self.answered.refusals.join("; "));
        }
        // Held is a refusal with a reason rather than an error: nothing is
        // written, the matter stays in the feed unclaimed, and the sweep goes
        // on to the next one (§FS-005-dispatch.33).
        if let Some(held) = &self.held {
            return Some(format!(
                "'{}' {}. Nothing was laid, and the matter is still anybody's.",
                self.entry, held.clause
            ));
        }
        if !self.answered.missing.is_empty() {
            return Some(format!(
                "'{}' cannot be laid down: nothing answers {}. Answer them with --set \
                 <input>=<value> or --values <file>, or say them in the entry.",
                self.entry,
                self.answered.missing.join(", ")
            ));
        }
        None
    }
}

/// One execution-target input's name, resolved once before any of it is used
/// (§DA-006-hands-fill-a-workflows-targets): the target the answering writes,
/// and the pool that hand's work would be bought against
/// (§FS-005-dispatch.33). The pool travels beside the answer because what a
/// piece of work needs is derived from the hands ephor resolved for it and
/// nothing declares it.
#[derive(Debug, Clone)]
struct NamedHand {
    target: std::result::Result<Option<String>, String>,
    pool: Option<String>,
}

/// A workflow's plan, written (§FS-005-dispatch.19).
pub struct Laid {
    pub outcome: Outcome,
    /// What the binding said as it wrote — its own account of what it made,
    /// which is what a reader is shown before and after.
    pub report: String,
}

/// The files ephor writes for a workflow to read, under ephor's own hidden
/// corner in the work root ([`plan::CARRIED`]): a dotted name is not a plan, so
/// enumerating the root steps over it (§FS-005-dispatch.15). Writing it is also
/// what names the plan the runtime renders here as one ephor caused to exist,
/// so the feed does not offer it back as the project's own work
/// (§FS-006-project-interface.7).
fn carried(root: &std::path::Path, plan_id: &str) -> PathBuf {
    root.join(plan::CARRIED).join(plan_id)
}

const DOSSIER: &str = "dossier.md";
const ITEM: &str = "item.json";
const VALUES: &str = "values.json";

/// The files a runtime needs to validate a dry run. They live in a fresh
/// temporary directory, not beside the plan, and the directory is removed
/// when the runtime has answered — including when it refused.
struct StagedWorkflow {
    dir: PathBuf,
    dossier: PathBuf,
    item: PathBuf,
    values: PathBuf,
}

impl StagedWorkflow {
    fn new(
        values: &serde_json::Map<String, Value>,
        destination: &std::path::Path,
        dossier: &str,
        item: &str,
    ) -> Result<StagedWorkflow> {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let base = std::env::temp_dir();
        let pid = std::process::id();
        for _ in 0..64 {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = base.join(format!("ephor-workflow-values-{pid}-{serial}"));
            match fs::create_dir(&dir) {
                Ok(()) => {
                    let staged = StagedWorkflow {
                        dossier: dir.join(DOSSIER),
                        item: dir.join(ITEM),
                        values: dir.join(VALUES),
                        dir,
                    };
                    let destination_dossier = for_shell(&destination.join(DOSSIER));
                    let destination_item = for_shell(&destination.join(ITEM));
                    let staged_dossier = for_shell(&staged.dossier);
                    let staged_item = for_shell(&staged.item);
                    let mut runtime_values = values.clone();
                    for value in runtime_values.values_mut() {
                        replace_path(value, &destination_dossier, &staged_dossier);
                        replace_path(value, &destination_item, &staged_item);
                    }
                    let values = serde_json::to_string_pretty(&runtime_values)
                        .unwrap_or_else(|_| "{}".to_string());
                    for (path, content) in [
                        (&staged.dossier, dossier),
                        (&staged.item, item),
                        (&staged.values, values.as_str()),
                    ] {
                        fs::write(path, content).map_err(|err| {
                            EphorError::Command(format!(
                                "Cannot write temporary workflow file {}: {err}",
                                path.display()
                            ))
                        })?;
                    }
                    return Ok(staged);
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(EphorError::Command(format!(
                        "Cannot make a temporary place for workflow values in {}: {err}",
                        base.display()
                    )))
                }
            }
        }
        Err(EphorError::Command(format!(
            "Cannot make a temporary place for workflow values in {}",
            base.display()
        )))
    }

    /// Runtime validation reads temporary files, but its account is public.
    /// Restore only the three exact paths ephor substituted before returning
    /// that account, so reports and refusals keep the destination semantics.
    fn restore_destination_paths(&self, text: &str, destination: &std::path::Path) -> String {
        [
            (&self.values, destination.join(VALUES)),
            (&self.dossier, destination.join(DOSSIER)),
            (&self.item, destination.join(ITEM)),
        ]
        .into_iter()
        .fold(text.to_string(), |text, (staged, destination)| {
            text.replace(&for_shell(staged), &for_shell(&destination))
        })
    }

    fn restore_destination_error(
        &self,
        error: EphorError,
        destination: &std::path::Path,
    ) -> EphorError {
        match error {
            EphorError::Registry(message) => {
                EphorError::Registry(self.restore_destination_paths(&message, destination))
            }
            EphorError::Command(message) => {
                EphorError::Command(self.restore_destination_paths(&message, destination))
            }
        }
    }
}

impl Drop for StagedWorkflow {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Replace only the values sent to a dry-run runtime. Public answers retain
/// the eventual destination paths, while every supported carried-file input
/// the runtime validates points at an existing staged equivalent.
fn replace_path(value: &mut Value, destination: &str, staged: &str) {
    match value {
        Value::String(text) if text.contains(destination) => {
            *text = text.replace(destination, staged);
        }
        Value::Array(items) => {
            for item in items {
                replace_path(item, destination, staged);
            }
        }
        Value::Object(fields) => {
            for field in fields.values_mut() {
                replace_path(field, destination, staged);
            }
        }
        _ => {}
    }
}

/// The item as data, as a workflow's own programs can read it — the same
/// names a shell action gets in its environment (§FS-005-dispatch.8).
fn identifiers(metadata: &[(&'static str, String)]) -> String {
    let fields: serde_json::Map<String, Value> = metadata
        .iter()
        .map(|(key, value)| ((*key).to_string(), Value::String(value.clone())))
        .collect();
    serde_json::to_string_pretty(&fields).unwrap_or_else(|_| "{}".to_string())
}

/// A plan id nothing has taken in this work root: the name the entry earns,
/// then the same name counted, so a second run of one workflow about one item
/// is a second record rather than a correction of the first
/// (§FS-005-dispatch.19).
fn free_plan_id(root: &std::path::Path, base: &str) -> String {
    if !root.join(base).exists() {
        return base.to_string();
    }
    (2..)
        .map(|nth| format!("{base}-{nth}"))
        .find(|id| !root.join(id).exists())
        .unwrap_or_else(|| base.to_string())
}

/// Every hand named in one written answer, at whatever depth it was written.
fn collect_names(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(name) => out.push(name.clone()),
        Value::Array(items) => items.iter().for_each(|item| collect_names(item, out)),
        _ => {}
    }
}

/// Where one item's work goes, and what the ticket will say.
struct Site {
    /// The work root this dispatch writes the plan into: resolved through the
    /// matter's own placement, so it is never inside the project's main
    /// checkout for a matter that merely matched the main branch
    /// (§FS-005-dispatch.25).
    dir: PathBuf,
    dossier: String,
    /// The item as data, for the state machine's programs
    /// (§FS-005-dispatch.8).
    metadata: Vec<(&'static str, String)>,
    values: BTreeMap<std::borrow::Cow<'static, str>, String>,
    /// An existing project root for preflight when a branch workspace is
    /// still to be minted. The real render uses `checkout.workspace`.
    runtime_root: PathBuf,
    /// Where the work runs, and what the ticket is told about where it is:
    /// where the matter's code lives right now, the main branch included
    /// (§FS-005-dispatch.25).
    #[allow(dead_code)] // kept for callers that report where work landed
    checkout: crate::branches::Checkout,
    /// The branch workspace this dispatch has to make before it writes
    /// anything, where a `branch` template named one that is not on disk
    /// (§FS-005-dispatch.25). None for everything else, including a workspace
    /// the template named that is already there.
    mint: Option<PathBuf>,
    /// The branch workspace a `branch` template named, on disk or not — `mint`
    /// is this same path narrowed to the case that has to be made.
    ///
    /// Held beside it because *there* and *made* are not the same fact: the
    /// directory a bound checkout command left behind without making the
    /// workspace reads as checked out, so nothing is minted and the maker never
    /// gets to refuse (§FS-006-project-interface.8). This is what lets the
    /// dispatch ask the one question `mint` cannot.
    named: Option<PathBuf>,
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
