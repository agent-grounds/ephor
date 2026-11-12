//! A retired placement, read the one way every surface reads it
//! (§FS-005-dispatch.4, §FS-005-dispatch.36.5).
//!
//! A plan that was retired is not in its root because ephor put it somewhere
//! and wrote down where, which is a different fact from a plan nobody can
//! find. The work list, the work view, the work screen, `forget`, `cancel` and
//! dispatch all ask here, so none of them can say a different thing about it.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::ledger::{Dispatch, Entry};
use super::runtime::exclusion;
use super::runtime::plan::{self, Plan, WorkRoot};

/// One placement a retirement took or found already gone, resolved against
/// the disk (§FS-005-dispatch.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retired {
    /// Where the plan was.
    pub from: PathBuf,
    /// Where it went in the archive; `None` where it was already gone.
    pub to: Option<PathBuf>,
    /// The retirement's archive directory.
    pub archive: PathBuf,
    /// The record written beside it.
    pub record: PathBuf,
    pub at: DateTime<Utc>,
    /// The archive, or this plan's copy in it, is gone: from then on the rules
    /// for a missing plan apply (§FS-005-dispatch.4).
    pub archive_missing: bool,
    /// How many of the archived plan's tasks are not final, or `None` where
    /// the archived plan cannot be read — which is never read as finished
    /// (§FS-005-dispatch.35.1).
    pub open: Option<usize>,
}

impl Retired {
    /// Where the work is now: the plan's own copy, or the archive.
    pub fn kept_at(&self) -> &Path {
        self.to.as_deref().unwrap_or(&self.archive)
    }

    /// The sentence every surface says about it (§FS-005-dispatch.4).
    pub fn said(&self) -> String {
        match self.archive_missing {
            true => format!("retired; archive missing at {}", self.kept_at().display()),
            false => format!("retired → {}", self.archive.display()),
        }
    }
}

/// Whether a dispatch's placement was retired (§FS-005-dispatch.36.5): it is
/// then not open work, not handed-over work, and not a plan to read in place.
pub fn is_retired(dispatch: &Dispatch) -> bool {
    dispatch.retired_at.is_some()
}

/// The dispatches whose placement still stands, in the record's order: every
/// one no retirement took (§FS-005-dispatch.4). A reader asking where this
/// matter's work is — to read it, run it, count it or attribute a root to it —
/// asks over these, so none of them finds a placement in the archive in a root.
pub fn standing(entry: &Entry) -> impl Iterator<Item = &Dispatch> + '_ {
    entry
        .dispatches
        .iter()
        .filter(|dispatch| !is_retired(dispatch))
}

/// Whether the entry's own singular placement, its `root` and `plan`, may
/// still be read as one (§FS-005-dispatch.4). Those fields are what a record
/// written before provenance falls back to and what a run seeds beside the
/// dispatches. Once any of the matter's placements was retired they name what
/// went to the archive, and every placement made since names its own root, so
/// a file found at the entry's own path is not this matter's open work.
pub fn entry_placement_stands(entry: &Entry) -> bool {
    !entry.dispatches.iter().any(is_retired)
}

/// Every placement this matter's retirements took, oldest first, resolved
/// against the disk (§FS-005-dispatch.4).
pub fn placements(entry: &Entry) -> Vec<Retired> {
    let mut out = Vec::new();
    for retirement in &entry.retirements {
        for plan in &retirement.plans {
            let archive_missing = match &plan.to {
                Some(to) => fs_missing(to),
                None => fs_missing(&retirement.archive),
            };
            let open = match (&plan.to, archive_missing) {
                (Some(to), false) => open_in(to),
                (None, false) => Some(0),
                _ => None,
            };
            out.push(Retired {
                from: plan.from.clone(),
                to: plan.to.clone(),
                archive: retirement.archive.clone(),
                record: retirement.record.clone(),
                at: retirement.at,
                archive_missing,
                open,
            });
        }
    }
    out
}

/// The latest retirement that took `path`, where one did — what a refusal to
/// cancel a ticket in it names (§FS-005-dispatch.16).
pub fn of_path(entry: &Entry, path: &Path) -> Option<Retired> {
    let wanted = canonical(path);
    placements(entry)
        .into_iter()
        .rev()
        .find(|retired| canonical(&retired.from) == wanted)
}

/// The paragraph a new plan's first ticket carries after a reopen
/// (§FS-005-dispatch.5): earlier work on this item was retired, when, where it
/// is kept unchanged, the record, and the archived plans, and nothing here
/// waits on them. `None` where nothing was ever retired.
pub fn paragraph(entry: &Entry) -> Option<String> {
    let retirement = entry.retirements.last()?;
    let plans: Vec<String> = retirement
        .plans
        .iter()
        .filter_map(|plan| plan.to.as_ref())
        .map(|to| format!("`{}`", to.display()))
        .collect();
    let mut said = format!(
        "Earlier work on this item was retired on {} and is kept unchanged in `{}` \
         (record: `{}`).",
        retirement.at.format("%Y-%m-%d %H:%M UTC"),
        retirement.archive.display(),
        retirement.record.display()
    );
    if !plans.is_empty() {
        said.push_str(&format!(" The archived plans: {}.", plans.join(", ")));
    }
    said.push_str(" Nothing here waits on them.");
    Some(said)
}

/// Whether nothing is at `path` — not even a dangling link.
fn fs_missing(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_err()
}

/// How many of an archived plan's tasks are not final, judged by its own
/// machine or the copy of its root's machine carried beside it
/// (§FS-005-dispatch.36.4). `None` where it cannot be read or judged.
pub fn open_in(to: &Path) -> Option<usize> {
    let file = exclusion::plan_file(to);
    let unit_parent = to.parent()?;
    let machine = match plan::own_machine(&file) {
        Ok(Some(own)) => own,
        Ok(None) => WorkRoot::open(unit_parent).ok().flatten()?,
        Err(_) => return None,
    };
    let read = Plan::read(&file).ok().flatten()?;
    Some(
        read.tickets()
            .iter()
            .filter(|ticket| {
                !ticket
                    .state
                    .as_deref()
                    .is_some_and(|state| machine.is_final(state))
            })
            .count(),
    )
}

/// The canonical path, or, for a path that is gone, its canonical parent
/// joined to its name — a retired plan's own path no longer exists.
fn canonical(path: &Path) -> PathBuf {
    if let Ok(found) = std::fs::canonicalize(path) {
        return found;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => canonical(parent).join(name),
        _ => path.to_path_buf(),
    }
}
