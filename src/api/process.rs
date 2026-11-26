//! Processing a new matter where the Waiting screen shows it
//! (§FS-019-waiting.7): the rhei proposed for each, the move that lays it,
//! records it and asks for its start, a person's record that a matter needs
//! none, and the sentence naming the setting a repeated confirmation would
//! make automatic.
//!
//! Nothing here is a second implementation of anything. The proposal is the
//! menu's first entry that hands work over ([`Session::menu`]), the laying is
//! [`Session::hand_over`]'s, and the start is the sweep's own admission
//! narrowed to the matter ([`crate::work::Dispatcher::start_for`]), so the
//! screen, `ephor work process` and `ephor actions run` cannot come apart
//! (§AR-009-surfaces.1, §REQ-002-parity.2).

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;

use crate::feed::model::{Item, ItemKind};
use crate::work::ledger::{Dismissal, Processed, Snapshot};

use crate::feed::config::Minted;

use super::offers::{Gate, MenuEntry};
use super::read::Subject;
use super::{views, Session};

/// The workflow a matter no entry hands over is offered
/// (§FS-019-waiting.7.1). Ephor ships none by this name: which one stands
/// behind it is the runtime's.
pub const TRIAGE: &str = "triage";

/// How many confirmations of one entry, for one kind of matter, before the
/// footer names the setting that would make it automatic
/// (§FS-019-waiting.7.7).
pub const ADOPT_AFTER: usize = 3;

/// What a queued row says where the admission left no sentence of its own:
/// nothing was due to start on the matter's root just now, which is a live
/// run already holding it or a root resting after a failed start
/// (§FS-005-dispatch.24.5).
pub const NO_START_NOW: &str = "no run could start for it now, and the next sweep starts it";

/// One entry `a` offers: the name `ephor work process --entry` takes, and
/// what the menu says about it (§FS-019-waiting.7.3).
#[derive(Debug, Clone)]
pub struct Choice {
    pub key: String,
    pub kind: &'static str,
    pub description: String,
    /// Why it cannot hand the matter over as it stands, where it cannot.
    pub refusal: Option<String>,
}

/// A workflow entry's inputs, accounted for before anything is laid.
#[derive(Default)]
pub(crate) struct Inputs {
    pub answered: Vec<views::Answered>,
    /// Required, and nobody answered them.
    pub missing: Vec<String>,
    /// What could not stand.
    pub refusals: Vec<String>,
}

/// The entry a matter would be processed with, and how it was chosen.
pub(crate) struct Chosen {
    pub entry: MenuEntry,
    /// How many other entries would also hand the matter over.
    pub others: usize,
    /// The runtime's `triage`, because no entry hands the matter over.
    pub fallback: bool,
}

impl Session {
    /// The label that holds a matter out of the machine's hands, where one
    /// does (§FS-019-waiting.1.2).
    pub(crate) fn held_by(&self, item: &Item) -> Option<String> {
        holding_label(item, &self.holding_labels())
    }

    /// The ledger, read again from disk before a move that saves it
    /// (§FS-005-dispatch.4). Nothing to read without a registry.
    pub(crate) fn reload_ledger(&mut self) -> Result<(), String> {
        match self.dispatcher.as_mut() {
            Some(dispatcher) => dispatcher.reload_ledger().map_err(|err| err.to_string()),
            None => Ok(()),
        }
    }

    /// Whether the ledger records work about this matter — the first of the
    /// two ways a matter is processed (§FS-019-waiting.7). A monitor's matter
    /// still down or unknown after its work ended is New again until work is
    /// laid on it anew (§FS-019-waiting.1.2, §FS-021-monitors.13.2).
    pub(crate) fn has_work(&self, item: &Item) -> bool {
        self.dispatcher.as_ref().is_some_and(|dispatcher| {
            dispatcher
                .ledger
                .entries
                .get(&item.id)
                .is_some_and(|entry| {
                    crate::monitors::matters::still_processed(
                        &item.source,
                        &item.raw,
                        entry.dispatches.iter().map(|laid| laid.at),
                    )
                })
        })
    }

    /// Whether a person's dismissal of this matter still stands: it does
    /// until the matter moves (§FS-019-waiting.7.4, §FS-005-dispatch.5).
    pub(crate) fn dismissed(&self, item: &Item) -> bool {
        self.dispatcher.as_ref().is_some_and(|dispatcher| {
            dispatcher
                .ledger
                .dismissed
                .get(&item.id)
                .is_some_and(|dismissal| dismissal.stands(item))
        })
    }

    /// Every entry of the matter's menu that hands work over, in menu order
    /// (§FS-019-waiting.7.1, §FS-019-waiting.7.3). The source's own entries
    /// are left out of the reading: none of them hands work over, and asking
    /// for them launches the source once per row (§FS-019-waiting.6).
    pub(crate) fn handing(&mut self, item: &Item) -> Result<Vec<MenuEntry>, String> {
        Ok(self
            .menu_with(&Subject::Item(item), false)?
            .into_iter()
            .filter(|entry| matches!(entry.kind(), "agent" | "workflow"))
            .collect())
    }

    /// Every workflow the runtime offers about this matter that no entry of
    /// its menu names, as entries: the rest of what `a` chooses among
    /// (§FS-019-waiting.7.3).
    pub(crate) fn other_workflows(&mut self, item: &Item, handing: &[MenuEntry]) -> Vec<MenuEntry> {
        let Some(dispatcher) = self.dispatcher.as_mut() else {
            return Vec::new();
        };
        let Ok(offered) = dispatcher.workflows(&item.project) else {
            return Vec::new();
        };
        let named: BTreeSet<&str> = handing
            .iter()
            .filter_map(|entry| entry.action.workflow.as_ref())
            .map(|ask| ask.name.as_str())
            .collect();
        offered
            .workflows
            .iter()
            .filter(|workflow| !named.contains(workflow.id.as_str()))
            .map(|workflow| bare(crate::work::workflow::Beside::default().action(workflow)))
            .collect()
    }

    /// What `a` lists (§FS-019-waiting.7.3): every entry of the matter's
    /// menu that hands work over, in menu order, then every workflow the
    /// runtime offers there that no entry names.
    pub fn process_choices(&mut self, item: &Item) -> Vec<Choice> {
        let handing = self.handing(item).unwrap_or_default();
        let others = self.other_workflows(item, &handing);
        handing
            .iter()
            .chain(others.iter())
            .map(|entry| Choice {
                key: entry.key(),
                kind: entry.kind(),
                description: entry.action.description.clone(),
                refusal: match &entry.running {
                    Some(_) => Some("already going".to_string()),
                    None => entry.gate.refusal().map(str::to_string),
                },
            })
            .collect()
    }

    /// The entry processing uses: the proposal where `named` is None, and
    /// otherwise the menu entry by the name `ephor actions` prints, or the
    /// runtime's workflow by its own id (§FS-019-waiting.7.1,
    /// §FS-019-waiting.7.3).
    pub(crate) fn choose(&mut self, item: &Item, named: Option<&str>) -> Result<Chosen, String> {
        let handing = self.handing(item)?;
        let eligible =
            |entry: &MenuEntry| !matches!(entry.gate, Gate::Blocked(_)) && entry.running.is_none();
        let count = handing.iter().filter(|entry| eligible(entry)).count();
        match named {
            None => match handing.iter().find(|entry| eligible(entry)) {
                Some(first) => Ok(Chosen {
                    entry: first.clone(),
                    others: count.saturating_sub(1),
                    fallback: false,
                }),
                None => self
                    .runtime_workflow(item, TRIAGE)
                    .map(|entry| Chosen {
                        entry,
                        others: 0,
                        fallback: true,
                    })
                    .map_err(|_| no_triage(&item.project)),
            },
            Some(name) => {
                if let Some(entry) = handing.iter().find(|entry| entry.key() == name) {
                    return Ok(Chosen {
                        entry: entry.clone(),
                        others: count.saturating_sub(usize::from(eligible(entry))),
                        fallback: false,
                    });
                }
                self.runtime_workflow(item, name).map(|entry| Chosen {
                    entry,
                    others: count,
                    fallback: name == TRIAGE && count == 0,
                })
            }
        }
    }

    /// One of the runtime's workflows, as a bare entry: what laying it by its
    /// own id lays, exactly as `ephor work lay <workflow>` does where no entry
    /// names it (§FS-005-dispatch.19).
    fn runtime_workflow(&mut self, item: &Item, id: &str) -> Result<MenuEntry, String> {
        let Some(dispatcher) = self.dispatcher.as_mut() else {
            return Err("Work needs the registry, which could not be read".to_string());
        };
        let offered = dispatcher
            .workflows(&item.project)
            .map_err(|err| err.to_string())?;
        if let Some(refusal) = offered.refusal {
            return Err(refusal);
        }
        let workflow = offered.find(id).ok_or_else(|| {
            format!(
                "No entry or workflow called '{id}' hands {} over. `ephor actions --item {}` \
                 and `ephor work workflows` list what there is.",
                item.id, item.id
            )
        })?;
        Ok(bare(
            crate::work::workflow::Beside::default().action(workflow),
        ))
    }

    /// The proposal a new row carries (§FS-019-waiting.7.1): every field the
    /// menu's, and a refusal where nothing can be proposed.
    pub(crate) fn proposal_of(&mut self, item: &Item) -> views::Proposal {
        match self.choose(item, None) {
            Ok(chosen) => self.proposal_from(item, &chosen),
            Err(refusal) => views::Proposal {
                entry: TRIAGE.to_string(),
                kind: "workflow",
                icon: String::new(),
                description: "the runtime's triage workflow".to_string(),
                gate: "blocked",
                hand: None,
                branch: None,
                workspace: None,
                root: None,
                others: 0,
                fallback: true,
                unambiguous: false,
                refusal: Some(refusal),
            },
        }
    }

    /// The proposal for an entry already chosen — the processing screen shows
    /// one the reader picked with `a` in the same shape (§FS-019-waiting.7.3).
    pub(crate) fn proposal_from(&mut self, item: &Item, chosen: &Chosen) -> views::Proposal {
        let entry = &chosen.entry;
        let (branch, template) = match &entry.action.agent {
            Some(recipe) => (recipe.branch.as_deref(), recipe.root.as_deref()),
            None => (entry.action.branch.as_deref(), entry.action.root.as_deref()),
        };
        let root = self.work_root_for(item, branch, template);
        let mut refusal = entry.gate.refusal().map(str::to_string);
        // A workflow whose inputs are not all answered is not laid with a
        // guess, so it is not unambiguous (§FS-019-waiting.7.5).
        if refusal.is_none() {
            let inputs = self.inputs_of(item, entry, &[]);
            if !inputs.refusals.is_empty() {
                refusal = Some(inputs.refusals.join("; "));
            } else if !inputs.missing.is_empty() {
                refusal = Some(format!("it still wants {}", inputs.missing.join(", ")));
            }
        }
        // A recipe's hand is the menu's; a workflow's is the one its laying
        // would resolve at the same root (§FS-019-waiting.7.1,
        // §FS-005-dispatch.14).
        let hand = match &entry.action.hand {
            Some(hand) => Some(hand.says.clone()),
            None if entry.action.workflow.is_some() => root
                .as_deref()
                .and_then(|root| self.workflow_hand(item, entry, root)),
            None => None,
        };
        views::Proposal {
            entry: entry.key(),
            kind: entry.kind(),
            icon: entry.action.icon.clone(),
            description: entry.action.description.clone(),
            gate: entry.gate.name(),
            hand,
            branch: match &entry.action.minted {
                Some(Minted::Named { branch, .. }) => Some(branch.clone()),
                _ => None,
            },
            workspace: match &entry.action.minted {
                Some(Minted::Named { workspace, .. }) => Some(workspace.clone()),
                _ => None,
            },
            root,
            others: chosen.others,
            fallback: chosen.fallback,
            unambiguous: refusal.is_none() && entry.running.is_none(),
            refusal,
        }
    }

    /// Who a workflow entry's work would go to, resolved as its laying
    /// resolves it, against the root it would be laid in
    /// (§FS-005-dispatch.14, §FS-005-dispatch.19).
    fn workflow_hand(
        &mut self,
        item: &Item,
        entry: &MenuEntry,
        root: &std::path::Path,
    ) -> Option<String> {
        let unbound = crate::work::runtime::refusal(&self.work_config);
        let dispatcher = self.dispatcher.as_mut()?;
        let choice = dispatcher.hand(
            &item.project,
            &entry.action.id,
            entry.picked.as_ref(),
            None,
            root,
        );
        Some(super::session::who_gets_it(&choice, unbound.as_deref()).says)
    }

    /// Process one matter (§FS-019-waiting.7.2): lay the chosen entry down
    /// through the move `ephor actions run` makes, record it, and ask the
    /// sweep's admission for its start. `named` is None for the proposal.
    /// A dry run is the account the processing screen shows, and writes
    /// nothing.
    pub fn process(
        &mut self,
        item: &Item,
        named: Option<&str>,
        answers: &[String],
        dry_run: bool,
    ) -> views::ProcessRow {
        let mut row = views::ProcessRow {
            item: item.id.clone(),
            title: item.title.clone(),
            entry: named.unwrap_or_default().to_string(),
            kind: "",
            ok: false,
            says: String::new(),
            plan: None,
            tickets: Vec::new(),
            start: None,
            // Who holds the matter and how it is labelled, as the feed says
            // (§FS-019-waiting.7.2).
            assignees: crate::work::recipe::reported(item, "assignees"),
            labels: crate::work::recipe::reported(item, "labels").unwrap_or_default(),
            answers: Vec::new(),
        };
        // Asked of the ledger as it is now, and saved over it under the lock a
        // sweep holds, so neither writes over the other (§FS-019-waiting.7.2).
        let mut lock = None;
        if !dry_run {
            match crate::work::ledger_lock() {
                Ok(held) => lock = Some(held),
                Err(err) => {
                    row.says = err.to_string();
                    return row;
                }
            }
            if let Err(err) = self.reload_ledger() {
                row.says = err;
                return row;
            }
        }
        if let Some(label) = self.held_by(item) {
            row.says = format!(
                "{} is held by {label}, and a held matter is offered nothing",
                item.id
            );
            return row;
        }
        if self.has_work(item) {
            row.says = format!(
                "{} already has work: `ephor work offers --item {}` says what",
                item.id, item.id
            );
            return row;
        }
        let chosen = match self.choose(item, named) {
            Ok(chosen) => chosen,
            Err(refusal) => {
                row.says = refusal;
                return row;
            }
        };
        let entry = chosen.entry;
        row.entry = entry.key();
        row.kind = entry.kind();
        row.answers = self.inputs_of(item, &entry, answers).answered;
        let outcome = self.hand_over_with(item, &entry, answers, dry_run, false);
        row.ok = outcome.ok;
        row.says = outcome.says;
        row.plan = outcome.plan;
        row.tickets = outcome.tickets;
        // Only work laid is a rhei assigned: a move that wrote nothing
        // records nothing (§FS-019-waiting.7).
        if !row.ok || dry_run || (row.plan.is_none() && row.tickets.is_empty()) {
            return row;
        }
        // The record first, so the start below reads it: the sweep reloads
        // the ledger from disk (§FS-005-dispatch.24.5).
        if let Err(err) = self.record_processed(item, &entry) {
            row.says = format!("{} · {err}", row.says);
            return row;
        }
        // The start takes the lock again as a sweep, so it is let go here.
        drop(lock);
        let (start, says) = self.start_processed(item);
        row.start = Some(start);
        row.says = format!("{} · {says}", row.says);
        self.reload_work();
        row
    }

    /// What answered each input of a workflow entry, what nobody did, and
    /// what could not stand, as `ephor work lay` accounts for them
    /// (§FS-005-dispatch.19). Empty on a recipe; a laying that fails is left
    /// for the move itself to refuse.
    pub(crate) fn inputs_of(
        &mut self,
        item: &Item,
        entry: &MenuEntry,
        answers: &[String],
    ) -> Inputs {
        let mut inputs = Inputs::default();
        if entry.action.workflow.is_none() {
            return inputs;
        }
        let Ok(typed) = super::act::parse_answers(answers) else {
            return inputs;
        };
        let Some(dispatcher) = self.dispatcher.as_mut() else {
            return inputs;
        };
        if let Ok(laying) = dispatcher.laying(item, &entry.action, &typed, entry.picked.as_ref()) {
            inputs.answered = laying
                .answered
                .answers
                .iter()
                .map(|answer| views::Answered {
                    input: answer.input.clone(),
                    shown: answer.shown.clone(),
                    from: answer.from.label().to_string(),
                })
                .collect();
            inputs.missing = laying.answered.missing.clone();
            inputs.refusals = laying.answered.refusals.clone();
        }
        inputs
    }

    /// Write the processing mark, queued, and lift any dismissal: the matter
    /// has a rhei now (§FS-005-dispatch.4).
    fn record_processed(&mut self, item: &Item, entry: &MenuEntry) -> Result<(), String> {
        let Some(dispatcher) = self.dispatcher.as_mut() else {
            return Err("Work needs the registry, which could not be read".to_string());
        };
        dispatcher.ledger.dismissed.remove(&item.id);
        // Which run the root last had, so the first run there spends the mark
        // whoever starts it (§FS-005-dispatch.24.5).
        let root = crate::work::work_root_of(&dispatcher.ledger, &item.id);
        let run = root.as_deref().and_then(crate::work::last_run);
        dispatcher.ledger.processed.insert(
            item.id.clone(),
            Processed {
                entry: entry.key(),
                kind: entry.kind().to_string(),
                matter: item.kind.label().to_string(),
                project: item.project.clone(),
                title: item.title.clone(),
                at: Utc::now(),
                queued: true,
                says: None,
                root,
                run,
            },
        );
        dispatcher.save().map_err(|err| err.to_string())
    }

    /// Ask the sweep's admission for the start, narrowed to this matter, with
    /// the budget binding as it binds a sweep (§FS-005-dispatch.24.5,
    /// §FS-015-spend-ceiling.6). What it does not start stays queued, with
    /// the admission's own sentence for why.
    fn start_processed(&mut self, item: &Item) -> (&'static str, String) {
        let Some(dispatcher) = self.dispatcher.as_mut() else {
            return ("queued", format!("queued: {NO_START_NOW}"));
        };
        let id = item.id.clone();
        let (start, says, reason) = match dispatcher.start_for(
            Utc::now(),
            std::slice::from_ref(&id),
            crate::work::spend::Budget::Binds,
        ) {
            Ok(sweep) => {
                let ours: Vec<&crate::work::Launched> = sweep
                    .runs
                    .iter()
                    .filter(|run| run.items.contains(&id))
                    .collect();
                match ours
                    .iter()
                    .find(|run| matches!(run.outcome(), "started" | "done"))
                {
                    Some(run) => ("started", run.says(), None),
                    None => {
                        let why = ours
                            .first()
                            .and_then(|run| run.reason().or_else(|| run.failed.clone()))
                            .unwrap_or_else(|| NO_START_NOW.to_string());
                        ("queued", format!("queued: {why}"), Some(why))
                    }
                }
            }
            Err(err) => ("queued", format!("queued: {err}"), Some(err.to_string())),
        };
        if let Some(reason) = reason {
            if let Some(mark) = dispatcher.ledger.processed.get_mut(&id) {
                mark.says = Some(reason);
            }
            if let Err(err) = dispatcher.save() {
                return (start, format!("{says} · {err}"));
            }
        }
        (start, says)
    }

    /// Process every new row whose proposal is unambiguous, in the screen's
    /// order (§FS-019-waiting.7.5). A row whose dry run is refused — an input
    /// nobody answered, most often — is left, with the refusal. A dry run of
    /// the whole is the preview.
    /// `projects` narrows which projects' rows are taken; empty takes every
    /// one (§FS-011-command-line.9).
    pub fn process_all(&mut self, projects: &[String], dry_run: bool) -> views::Processing {
        let reading = self.waiting();
        let mut processed = Vec::new();
        let mut left = Vec::new();
        for row in reading.new {
            if !projects.is_empty() && !projects.contains(&row.project) {
                continue;
            }
            if !row.proposal.unambiguous {
                left.push(views::LeftMatter {
                    item: row.id,
                    title: row.title,
                    says: row
                        .proposal
                        .refusal
                        .unwrap_or_else(|| "its proposal is not ready".to_string()),
                });
                continue;
            }
            let Some(item) = self.item(&row.id) else {
                left.push(views::LeftMatter {
                    item: row.id,
                    title: row.title,
                    says: "it has left the feed".to_string(),
                });
                continue;
            };
            let entry = row.proposal.entry.clone();
            let preview = self.process(&item, Some(&entry), &[], true);
            if !preview.ok {
                left.push(views::LeftMatter {
                    item: row.id,
                    title: row.title,
                    says: preview.says,
                });
                continue;
            }
            processed.push(match dry_run {
                true => preview,
                false => self.process(&item, Some(&entry), &[], false),
            });
        }
        views::Processing {
            dry_run,
            processed,
            left,
            gated: false,
            says: None,
        }
    }

    /// Record that a matter needs no rhei (§FS-019-waiting.7.4): a ledger
    /// record with the reason and the matter's fingerprint, lifted by the
    /// matter moving and by nothing written (§FS-005-dispatch.5).
    pub fn dismiss(&mut self, item: &Item, reason: &str) -> views::Outcome {
        let reason = reason.trim();
        if reason.is_empty() {
            return views::Outcome::refused(
                "A dismissal needs a reason: say why this matter needs no work",
            );
        }
        // The ledger as it is now, read and saved under the sweep's lock as
        // processing's record is (§FS-019-waiting.7.2).
        let _lock = match crate::work::ledger_lock() {
            Ok(held) => held,
            Err(err) => return views::Outcome::refused(err.to_string()),
        };
        if let Err(err) = self.reload_ledger() {
            return views::Outcome::refused(err);
        }
        if let Some(label) = self.held_by(item) {
            return views::Outcome::refused(format!(
                "{} is held by {label}, and a held matter is offered nothing",
                item.id
            ));
        }
        if self.has_work(item) {
            return views::Outcome::refused(format!(
                "{} already has work, so there is nothing to dismiss: \
                 `ephor work offers --item {}` says what",
                item.id, item.id
            ));
        }
        let Some(dispatcher) = self.dispatcher.as_mut() else {
            return views::Outcome::refused("Work needs the registry, which could not be read");
        };
        dispatcher.ledger.dismissed.insert(
            item.id.clone(),
            Dismissal {
                project: item.project.clone(),
                title: item.title.clone(),
                reason: reason.to_string(),
                at: Utc::now(),
                snapshot: Snapshot::of(item),
            },
        );
        match dispatcher.save() {
            Ok(()) => views::Outcome::ok(format!(
                "✓ {} needs no work: {reason} — until it moves",
                item.id
            )),
            Err(err) => views::Outcome::refused(err.to_string()),
        }
    }

    /// The work processing laid and asked to start that has not started
    /// (§FS-019-waiting.7.6): a queued mark whose matter still has work open
    /// and no live run. `live` and `on_you` are the matters the reading
    /// already shows running or waiting on the reader.
    pub(crate) fn queued_rows(
        &self,
        live: &BTreeSet<String>,
        on_you: &BTreeSet<String>,
    ) -> Vec<views::QueuedMatter> {
        let Some(dispatcher) = self.dispatcher.as_ref() else {
            return Vec::new();
        };
        let mut rows: Vec<views::QueuedMatter> = dispatcher
            .ledger
            .processed
            .iter()
            .filter(|(id, mark)| {
                crate::work::still_queued(mark)
                    && !live.contains(*id)
                    && !on_you.contains(*id)
                    && self.working(id)
            })
            .map(|(id, mark)| views::QueuedMatter {
                id: id.clone(),
                project: mark.project.clone(),
                title: mark.title.clone(),
                at: mark.at,
                says: mark.says.clone(),
            })
            .collect();
        rows.sort_by(|a, b| a.at.cmp(&b.at));
        rows
    }

    /// The sentence naming the setting a repeated confirmation would make
    /// automatic (§FS-019-waiting.7.7). Counted from the processing marks,
    /// by entry and kind of matter; the most confirmed one that still lacks a
    /// setting is named.
    pub(crate) fn adopt(&mut self) -> Option<String> {
        let marks: Vec<Processed> = self
            .dispatcher
            .as_ref()?
            .ledger
            .processed
            .values()
            .cloned()
            .collect();
        let mut counts: BTreeMap<(String, String, String), (usize, String)> = BTreeMap::new();
        for mark in &marks {
            let counted = counts
                .entry((mark.entry.clone(), mark.kind.clone(), mark.matter.clone()))
                .or_insert((0, mark.project.clone()));
            counted.0 += 1;
        }
        let mut ranked: Vec<_> = counts
            .into_iter()
            .filter(|(_, (count, _))| *count >= ADOPT_AFTER)
            .collect();
        ranked.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
        for ((entry, kind, matter), (count, project)) in ranked {
            let missing = self.missing_settings(&project, &entry, &kind);
            if missing.is_empty() {
                continue;
            }
            return Some(format!(
                "{entry} confirmed for {count} {} · {} on {entry} would do it without you",
                plural(&matter),
                missing.join(" and ")
            ));
        }
        None
    }

    /// What an entry still needs to hand its work over with nobody present:
    /// `autorun` and `dispatch` on a recipe (§FS-005-dispatch.32), `autorun`
    /// on a workflow entry (§FS-005-dispatch.28). An entry that cannot be
    /// found is named with every setting it would need.
    fn missing_settings(&mut self, project: &str, entry: &str, kind: &str) -> Vec<&'static str> {
        let Some(dispatcher) = self.dispatcher.as_mut() else {
            return Vec::new();
        };
        if kind == "agent" {
            let recipe = dispatcher
                .recipes(project)
                .into_iter()
                .find(|recipe| recipe.id == entry);
            let (autorun, dispatch) = recipe
                .map(|recipe| (recipe.autorun, recipe.dispatch.is_some()))
                .unwrap_or((false, false));
            let mut missing = Vec::new();
            if !autorun {
                missing.push("\"autorun\": true");
            }
            if !dispatch {
                missing.push("\"dispatch\"");
            }
            return missing;
        }
        let beside = dispatcher.workflow_entries(project).unwrap_or_default();
        let autorun = self
            .actions
            .iter()
            .chain(self.project_actions.get(project).into_iter().flatten())
            .chain(beside.iter().map(|(_, action)| action))
            .find(|action| action.id == entry && action.workflow.is_some())
            .and_then(|action| action.workflow.as_ref())
            .is_some_and(|ask| ask.autorun);
        match autorun {
            true => Vec::new(),
            false => vec!["\"autorun\": true"],
        }
    }
}

/// What processing every unambiguous row came to, in one line: how many were
/// laid, started and queued, and how many were left (§FS-019-waiting.7.5).
pub fn summary(view: &views::Processing) -> String {
    let laid = view
        .processed
        .iter()
        .filter(|row| row.start.is_some())
        .count();
    let started = view
        .processed
        .iter()
        .filter(|row| row.start == Some("started"))
        .count();
    let refused = view.processed.iter().filter(|row| !row.ok).count();
    let mut says = format!(
        "processed {laid}: {started} started, {} queued",
        laid - started
    );
    if refused > 0 {
        says.push_str(&format!(" · {refused} refused"));
    }
    if !view.left.is_empty() {
        says.push_str(&format!(" · {} left for you", view.left.len()));
    }
    says
}

/// A menu entry for an action no menu assembled: ready, picked by nobody,
/// with nothing going.
fn bare(action: crate::feed::config::ActionConfig) -> MenuEntry {
    MenuEntry {
        action,
        is_checkout: false,
        is_freehand: false,
        is_workflows: false,
        picked: None,
        roster: Vec::new(),
        gate: Gate::Ready,
        running: None,
    }
}

/// What a matter no entry hands over is told where the runtime offers no
/// `triage` either (§FS-019-waiting.7.1).
pub fn no_triage(project: &str) -> String {
    format!(
        "no workflow accepts it, and the runtime offers no `{TRIAGE}` workflow for {project} \
         — so `{TRIAGE}` is not available"
    )
}

/// The label of the matter's own that holds it, where one does
/// (§FS-019-waiting.1.2).
pub(crate) fn holding_label(item: &Item, holds: &BTreeSet<String>) -> Option<String> {
    item.raw
        .get("labels")
        .and_then(|labels| labels.as_array())
        .into_iter()
        .flatten()
        .filter_map(|label| label.as_str())
        .find(|label| holds.contains(*label))
        .map(str::to_string)
}

/// A kind of matter, counted (§FS-019-waiting.7.7).
fn plural(matter: &str) -> String {
    let kind = [
        ItemKind::Issue,
        ItemKind::Pr,
        ItemKind::Ci,
        ItemKind::Task,
        ItemKind::Message,
        ItemKind::Status,
    ]
    .into_iter()
    .find(|kind| kind.label() == matter);
    match kind {
        Some(ItemKind::Issue) => "issues".to_string(),
        Some(ItemKind::Pr) => "pull requests".to_string(),
        Some(ItemKind::Ci) => "CI runs".to_string(),
        Some(ItemKind::Task) => "tasks".to_string(),
        Some(ItemKind::Message) => "messages".to_string(),
        Some(ItemKind::Status) => "status matters".to_string(),
        None => format!("{matter} matters"),
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
