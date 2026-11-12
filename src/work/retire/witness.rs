//! Whose a plan is, by what says so (§FS-005-dispatch.36.2): the `id` its
//! tickets record, the lay record ephor wrote beside it, and every ledger
//! dispatch whose plan it is. The rule is judged apart from the reading, so
//! every combination is a unit test.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::candidates::canonical;
use crate::work::ledger::Ledger;
use crate::work::retired;
use crate::work::runtime::plan::{self, Plan};

/// What one witness is (§FS-005-dispatch.36.5): the record names each.
pub const TICKET: &str = "ticket";
pub const LAY_RECORD: &str = "lay record";
pub const LEDGER: &str = "ledger";

/// Every matter something says a plan is, and what says it.
pub type Witnesses = BTreeMap<String, BTreeSet<&'static str>>;

/// The rule's answer for one open plan (§FS-005-dispatch.36.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Taken: witnessed as this matter's alone, or admitted by the caller.
    Take { admitted: bool },
    /// Nothing says whose it is, and the caller did not admit it.
    Unwitnessed,
    /// It is somebody else's: the matters that say so.
    Foreign(Vec<String>),
    /// Its witnesses name this matter and others: the others.
    Mixed(Vec<String>),
}

/// The witnesses must be a set that is not empty and names `item` and no
/// other (§FS-005-dispatch.36.2). Admission vouches for a plan nobody claimed;
/// it never overrules one somebody else did.
pub fn judge(item: &str, witnesses: &Witnesses, admitted: bool) -> Verdict {
    let others: Vec<String> = witnesses
        .keys()
        .filter(|matter| matter.as_str() != item)
        .cloned()
        .collect();
    match (witnesses.contains_key(item), others.is_empty()) {
        (true, true) => Verdict::Take { admitted: false },
        (true, false) => Verdict::Mixed(others),
        (false, false) => Verdict::Foreign(others),
        (false, true) if admitted => Verdict::Take { admitted: true },
        (false, true) => Verdict::Unwitnessed,
    }
}

/// Every witness of one plan (§FS-005-dispatch.36.2). `unit` is the path that
/// would move — the file, or the workspace directory. A lay record is read
/// beside it and beside each dispatch's recorded root.
pub fn witnesses(read: &Plan, plan_id: &str, unit: &Path, ledger: &Ledger) -> Witnesses {
    let mut found = Witnesses::new();
    for ticket in read.tickets() {
        if let Some(matter) = read.matter_of(plan_id, &ticket.id) {
            found.entry(matter).or_default().insert(TICKET);
        }
    }
    let wanted = canonical(unit);
    for record in lay_records(unit) {
        if let Some(matter) = said(&record, "id") {
            found.entry(matter).or_default().insert(LAY_RECORD);
        }
    }
    for (matter, entry) in &ledger.entries {
        let named = placements(entry).into_iter().any(|(_, path)| {
            let path = canonical(&path);
            path == wanted || wanted.starts_with(&path) && path != PathBuf::new()
        });
        if named {
            found.entry(matter.clone()).or_default().insert(LEDGER);
        }
        // The lay record beside the dispatch's recorded root, where the plan
        // was put somewhere other than the root it names.
        for dispatch in entry.dispatches.iter().filter(|d| !retired::is_retired(d)) {
            let (Some(name), Some(root)) = (dispatch.plan.as_deref(), dispatch.root.as_ref())
            else {
                continue;
            };
            if canonical(&root.join(name)) != wanted {
                continue;
            }
            let record = root.join(plan::CARRIED).join(name).join("item.json");
            if let Some(matter) = said(&record, "id") {
                found.entry(matter).or_default().insert(LAY_RECORD);
            }
        }
    }
    found
}

/// Every placement one ledger entry's live dispatches name, as `(dispatch
/// index, path)`: a workflow's laid directory, or the recipe plan's file —
/// the entry's own plan path where the dispatch recorded no root
/// (§FS-005-dispatch.36.2).
pub fn placements(entry: &crate::work::ledger::Entry) -> Vec<(usize, PathBuf)> {
    entry
        .dispatches
        .iter()
        .enumerate()
        .filter(|(_, dispatch)| !retired::is_retired(dispatch))
        .map(|(at, dispatch)| {
            let root = dispatch.root.clone().unwrap_or_else(|| entry.root.clone());
            let path = match (&dispatch.plan, &dispatch.root) {
                (Some(name), _) => root.join(name),
                (None, Some(_)) => plan::plan_path_in(&root, &entry.plan_id),
                (None, None) => entry.plan.clone(),
            };
            (at, path)
        })
        .collect()
}

/// The lay record ephor wrote beside a plan: under the work root's carried
/// corner, named after the plan's directory or its id.
pub fn lay_records(unit: &Path) -> Vec<PathBuf> {
    let (Some(root), Some(name)) = (unit.parent(), unit.file_name()) else {
        return Vec::new();
    };
    let mut names = vec![name.to_string_lossy().into_owned()];
    if let Some(stem) = name
        .to_str()
        .and_then(|name| name.strip_suffix(plan::PLAN_SUFFIX))
    {
        names.push(stem.to_string());
    }
    names
        .into_iter()
        .map(|name| root.join(plan::CARRIED).join(name).join("item.json"))
        .filter(|record| record.is_file())
        .collect()
}

/// The directory of a plan's lay record, where it has one.
pub fn lay_record_dir(unit: &Path) -> Option<PathBuf> {
    lay_records(unit)
        .into_iter()
        .next()
        .and_then(|record| record.parent().map(Path::to_path_buf))
}

/// One string a lay record says, where it says a non-empty one.
pub fn said(record: &Path, key: &str) -> Option<String> {
    let text = std::fs::read_to_string(record).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .get(key)?
        .as_str()
        .filter(|said| !said.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(matters: &[&str]) -> Witnesses {
        matters
            .iter()
            .map(|matter| (matter.to_string(), BTreeSet::from([TICKET])))
            .collect()
    }

    /// §FS-005-dispatch.36.2 over every combination, with and without
    /// `--admit`: this matter alone is taken; nothing is taken only when
    /// admitted; another matter, or this one and another, refuses either way.
    #[test]
    fn the_witness_rule_over_every_combination() {
        let item = "github-issues:acme/widget#301";
        let other = "github-issues:acme/widget#302";
        for admitted in [false, true] {
            assert_eq!(
                judge(item, &set(&[item]), admitted),
                Verdict::Take { admitted: false },
                "a plan witnessed as this matter's needs no admission"
            );
            assert_eq!(
                judge(item, &set(&[other]), admitted),
                Verdict::Foreign(vec![other.to_string()])
            );
            assert_eq!(
                judge(item, &set(&[item, other]), admitted),
                Verdict::Mixed(vec![other.to_string()])
            );
        }
        assert_eq!(judge(item, &set(&[]), false), Verdict::Unwitnessed);
        assert_eq!(
            judge(item, &set(&[]), true),
            Verdict::Take { admitted: true }
        );
    }
}
