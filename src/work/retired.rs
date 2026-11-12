//! A retired placement, read the one way every surface reads it
//! (§FS-005-dispatch.4, §FS-005-dispatch.36.5).
//!
//! A plan that was retired is not in its root because ephor put it somewhere
//! and wrote down where, which is a different fact from a plan nobody can
//! find. The work list, the work view, the work screen, `forget`, `cancel` and
//! dispatch all ask here, so none of them can say a different thing about it.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::ledger::{Dispatch, Entry, Retirement};
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

/// One of `items` per archive, in the order first seen: the one saying its
/// archive is gone where any placement in that archive does, and otherwise the
/// first (§FS-005-dispatch.4). A retirement that took two placements went to
/// one archive, so every surface saying where it went says it once.
pub fn one_per_archive<'a, T>(
    items: &'a [T],
    archive: impl Fn(&T) -> &Path,
    missing: impl Fn(&T) -> bool,
) -> Vec<&'a T> {
    let mut out: Vec<&T> = Vec::new();
    for item in items {
        if out.iter().any(|seen| archive(seen) == archive(item)) {
            continue;
        }
        let gone = items
            .iter()
            .filter(|other| archive(other) == archive(item))
            .find(|other| missing(other));
        out.push(gone.unwrap_or(item));
    }
    out
}

/// The plan names this matter's retired dispatches recorded in `root`. The
/// record keeps them as where the work was committed, so new work laid in that
/// root takes none of them (§FS-005-dispatch.5).
pub fn names_in(entry: &Entry, root: &Path) -> std::collections::BTreeSet<String> {
    let wanted = canonical(root);
    entry
        .dispatches
        .iter()
        .filter(|dispatch| is_retired(dispatch))
        .filter(|dispatch| canonical(dispatch.root.as_ref().unwrap_or(&entry.root)) == wanted)
        .filter_map(|dispatch| dispatch.plan.clone())
        .collect()
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

/// The retirement that took a dispatch's placement: the one stamped with the
/// same instant as the dispatch's `retired_at`, which is what ties the two
/// (§FS-005-dispatch.36.5), and the latest only where no stamp matches. What a
/// refusal to cancel a ticket in it names (§FS-005-dispatch.16) — a path
/// cannot say it, because a later retirement may have taken the same path
/// again. `None` where the dispatch was not retired.
pub fn of_dispatch<'a>(entry: &'a Entry, dispatch: &Dispatch) -> Option<&'a Retirement> {
    let at = dispatch.retired_at?;
    entry
        .retirements
        .iter()
        .find(|retirement| retirement.at == at)
        .or_else(|| entry.retirements.last())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn retirement(at: DateTime<Utc>, archive: &str) -> Retirement {
        Retirement {
            at,
            record: PathBuf::from(archive).join("retirement.json"),
            archive: PathBuf::from(archive),
            plans: Vec::new(),
        }
    }

    /// A dispatch names the retirement that took it, by the instant both carry,
    /// not the latest one — after a reopen a second retirement may take the
    /// same path again — and a dispatch nothing took names none
    /// (§FS-005-dispatch.16, §FS-005-dispatch.36.5).
    #[test]
    fn a_dispatch_names_the_retirement_stamped_with_its_own_instant() {
        let first = Utc::now() - chrono::Duration::hours(1);
        let second = Utc::now();
        let mut entry: Entry = serde_json::from_value(serde_json::json!({
            "project": "widget", "title": "t", "root": "/w/panta", "plan_id": "p",
            "plan": "/w/panta/p.rhei.md",
            "dispatches": [
                { "ticket": "answer-1", "recipe": "answer", "at": first,
                  "snapshot": { "updated_at": first } },
                { "ticket": "answer-2", "recipe": "answer", "at": second,
                  "snapshot": { "updated_at": second } },
                { "ticket": "answer-3", "recipe": "answer", "at": second,
                  "snapshot": { "updated_at": second } }
            ]
        }))
        .unwrap();
        entry.retirements = vec![retirement(first, "/a/1"), retirement(second, "/a/2")];
        entry.dispatches[0].retired_at = Some(first);
        entry.dispatches[1].retired_at = Some(second);

        let named = |index: usize| of_dispatch(&entry, &entry.dispatches[index]);
        assert_eq!(
            named(0).map(|r| r.archive.clone()),
            Some(PathBuf::from("/a/1"))
        );
        assert_eq!(
            named(1).map(|r| r.archive.clone()),
            Some(PathBuf::from("/a/2"))
        );
        assert_eq!(named(2), None, "a dispatch nothing took names nothing");

        // A stamp no retirement carries falls back to the latest.
        entry.dispatches[2].retired_at = Some(first - chrono::Duration::hours(1));
        assert_eq!(
            of_dispatch(&entry, &entry.dispatches[2]).map(|r| r.archive.clone()),
            Some(PathBuf::from("/a/2"))
        );
    }
}
