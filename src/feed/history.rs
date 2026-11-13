//! A source's answer kept per **history** (§FS-001-forge-interface.6.1): one
//! history per fetch capability a forge reached out of process declared, or a
//! single one — its whole answer — where a source has nothing finer to ask.
//!
//! The reports are stored once, in the slot's `matters`, in the one order the
//! merge reads them (§AR-008-pipeline.2): by configuration block, then by
//! capability in [`FETCHES`] order. Each history records how many of them are
//! its own from each block, which is what lets a run that asked one capability
//! replace that capability's reports and leave every other one where it was.
//! What a refresh commits is decided here, and nowhere else.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::feed::cache::ProviderSlot;
use crate::forge::FETCHES;
use crate::matter::Matter;

/// One source's answer to one capability (§AR-006-matters.4).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    /// The fetch subcommand this history answers, or None where it is the
    /// source's whole answer.
    #[serde(default)]
    pub capability: Option<String>,
    /// When a refresh last asked it, whether or not the answer was complete.
    /// None is *never asked*: what it would have reported is unknown, not
    /// absent (§FS-001-forge-interface.6.1).
    #[serde(default)]
    pub asked_at: Option<DateTime<Utc>>,
    /// When it last answered completely; its reports are from then.
    #[serde(default)]
    pub answered_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// It failed at its last asking and what it holds is last-good.
    #[serde(default)]
    pub stale: bool,
    /// Its failure was "could not reach the destination".
    #[serde(default, skip_serializing_if = "is_false")]
    pub unreachable: bool,
    /// Incremental fetch cursor, for a source that reads a stream from where
    /// it last stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// How many of the slot's reports are this history's, from each
    /// configuration block of the source in configuration order.
    #[serde(default)]
    pub blocks: Vec<usize>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl History {
    fn of(capability: Option<&str>) -> History {
        History {
            capability: capability.map(String::from),
            ..History::default()
        }
    }

    /// Whether a refresh ever asked it (§FS-011-command-line.12).
    pub fn asked(&self) -> bool {
        self.asked_at.is_some()
    }

    /// Whether its last asking answered completely.
    pub fn answered(&self) -> bool {
        self.asked() && self.error.is_none() && self.answered_at.is_some()
    }

    /// The fixed place of a history among its slot's: the whole answer, then
    /// the capabilities in [`FETCHES`] order (§AR-008-pipeline.2).
    fn rank(&self) -> usize {
        match self.capability.as_deref() {
            None => 0,
            Some(capability) => {
                1 + FETCHES
                    .iter()
                    .position(|fetch| *fetch == capability)
                    .unwrap_or(FETCHES.len())
            }
        }
    }
}

/// Why a source's answer was not complete, as the run reported it.
#[derive(Debug, Clone, Copy)]
pub struct Failure<'a> {
    pub message: &'a str,
    pub unreachable: bool,
}

/// Each history's reports, per configuration block.
pub type Parts = Vec<Vec<Matter>>;

/// The slot's reports split back into each history's, per block. A slot whose
/// counts do not add up to its reports — written by hand rather than by a
/// refresh — is read as its first history holding every report.
fn split(slot: &ProviderSlot) -> Vec<Parts> {
    let mut parts: Vec<Parts> = slot
        .histories
        .iter()
        .map(|history| vec![Vec::new(); history.blocks.len()])
        .collect();
    let held: usize = slot
        .histories
        .iter()
        .map(|history| history.blocks.iter().sum::<usize>())
        .sum();
    if held != slot.matters.len() {
        if let Some(first) = parts.first_mut() {
            *first = vec![slot.matters.clone()];
        }
        return parts;
    }
    let blocks = parts.iter().map(Vec::len).max().unwrap_or(0);
    let mut reports = slot.matters.iter().cloned();
    for block in 0..blocks {
        for (index, history) in slot.histories.iter().enumerate() {
            let count = history.blocks.get(block).copied().unwrap_or(0);
            if count > 0 {
                parts[index][block].extend(reports.by_ref().take(count));
            }
        }
    }
    parts
}

/// Lay histories and their reports into one slot: block by block, and within
/// a block history by history in fixed order, whichever run wrote each
/// (§AR-008-pipeline.2). What the slot as a whole reads as follows from its
/// histories.
fn assemble(mut pairs: Vec<(History, Parts)>) -> ProviderSlot {
    pairs.sort_by_key(|(history, _)| history.rank());
    let blocks = pairs
        .iter()
        .map(|(_, parts)| parts.len())
        .max()
        .unwrap_or(0);
    let histories: Vec<History> = pairs
        .iter()
        .map(|(history, parts)| History {
            blocks: parts.iter().map(Vec::len).collect(),
            ..history.clone()
        })
        .collect();
    let mut columns: Vec<_> = pairs
        .into_iter()
        .map(|(_, parts)| parts.into_iter())
        .collect();
    let mut matters = Vec::new();
    for _ in 0..blocks {
        for column in &mut columns {
            if let Some(part) = column.next() {
                matters.extend(part);
            }
        }
    }
    summarize(histories, matters)
}

/// What a slot reads as from its histories: failed where any history's last
/// asking failed, with every distinct diagnosis; stale where any holds
/// last-good reports; last answered when its oldest history did.
fn summarize(histories: Vec<History>, matters: Vec<Matter>) -> ProviderSlot {
    let mut errors: Vec<&str> = Vec::new();
    for error in histories
        .iter()
        .filter_map(|history| history.error.as_deref())
    {
        if !errors.contains(&error) {
            errors.push(error);
        }
    }
    let fetched_at = match histories
        .iter()
        .all(|history| history.answered_at.is_some())
    {
        true => histories
            .iter()
            .filter_map(|history| history.answered_at)
            .min(),
        false => None,
    };
    ProviderSlot {
        fetched_at,
        ok: errors.is_empty(),
        error: (!errors.is_empty()).then(|| errors.join("; ")),
        stale: histories.iter().any(|history| history.stale),
        unreachable: !errors.is_empty()
            && histories
                .iter()
                .filter(|history| history.error.is_some())
                .all(|history| history.unreachable),
        matters,
        histories,
    }
}

/// A history asked now that did not answer completely: it keeps its last-good
/// reports and time, is marked failed, and is stale where it holds anything.
fn failed(history: History, parts: &Parts, now: DateTime<Utc>, failure: Failure) -> History {
    History {
        asked_at: Some(now),
        error: Some(failure.message.to_string()),
        unreachable: failure.unreachable,
        stale: parts.iter().any(|part| !part.is_empty()),
        ..history
    }
}

/// Commit a source asked whole: by a plain refresh, or by one narrowed to the
/// source (§FS-001-forge-interface.6.1). A complete answer replaces every
/// history, empty ones included, with the histories the source answered now.
/// Where any contribution failed, every history keeps its last-good answer, is
/// marked stale and failed, and is still stamped asked; a source with no
/// history yet gets one per capability `declared` names, holding nothing.
pub fn commit_whole(
    previous: Option<&ProviderSlot>,
    now: DateTime<Utc>,
    answer: Result<Vec<(Option<&str>, Parts)>, Failure>,
    declared: &[Option<&str>],
) -> ProviderSlot {
    let failure = match answer {
        Ok(histories) => {
            return assemble(
                histories
                    .into_iter()
                    .map(|(capability, parts)| {
                        let cursor = previous
                            .and_then(|slot| slot.history(capability))
                            .and_then(|history| history.cursor.clone());
                        let history = History {
                            asked_at: Some(now),
                            answered_at: Some(now),
                            cursor,
                            ..History::of(capability)
                        };
                        (history, parts)
                    })
                    .collect(),
            )
        }
        Err(failure) => failure,
    };
    let kept: Vec<(History, Parts)> = match previous.filter(|slot| !slot.histories.is_empty()) {
        Some(slot) => slot.histories.iter().cloned().zip(split(slot)).collect(),
        None if declared.is_empty() => vec![(History::of(None), Vec::new())],
        None => declared
            .iter()
            .map(|capability| (History::of(*capability), Vec::new()))
            .collect(),
    };
    assemble(
        kept.into_iter()
            .map(|(history, parts)| (failed(history, &parts, now, failure), parts))
            .collect(),
    )
}

/// Commit one capability of a source, asked alone
/// (§FS-001-forge-interface.6.1, §FS-011-command-line.9.2). A complete answer
/// replaces that history, an empty one included; a failed or partial one keeps
/// its last-good answer and marks it. Every other history is left exactly as
/// it was, and a capability `declared` names that the slot has no history for
/// yet is recorded as never asked rather than as an empty answer. A whole
/// answer from a source never probed gives way to the declaration.
pub fn commit_one(
    previous: Option<&ProviderSlot>,
    now: DateTime<Utc>,
    capability: &str,
    declared: &[&str],
    answer: Result<Parts, Failure>,
) -> ProviderSlot {
    let mut pairs: Vec<(History, Parts)> = match previous {
        Some(slot) => slot
            .histories
            .iter()
            .cloned()
            .zip(split(slot))
            .filter(|(history, _)| history.capability.is_some())
            .collect(),
        None => Vec::new(),
    };
    for fetch in declared.iter().chain(std::iter::once(&capability)) {
        if !pairs
            .iter()
            .any(|(history, _)| history.capability.as_deref() == Some(*fetch))
        {
            pairs.push((History::of(Some(fetch)), Vec::new()));
        }
    }
    if let Some((history, parts)) = pairs
        .iter_mut()
        .find(|(history, _)| history.capability.as_deref() == Some(capability))
    {
        match answer {
            Ok(answered) => {
                *history = History {
                    asked_at: Some(now),
                    answered_at: Some(now),
                    cursor: history.cursor.take(),
                    ..History::of(Some(capability))
                };
                *parts = answered;
            }
            Err(failure) => *history = failed(history.clone(), parts, now, failure),
        }
    }
    assemble(pairs)
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
