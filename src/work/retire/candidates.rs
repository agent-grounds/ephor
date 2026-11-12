//! The branches a retirement checks for open pull requests, gathered in the
//! order §FS-005-dispatch.36.1 gives: the matter's live worktree, every branch
//! the record names minus placeholders plus the heads of the pull requests
//! that close the issue, and every branch the caller names.
//!
//! The table is worked out from facts already read — the record, the forge's
//! closing heads, the worktrees git lists — so it can be judged without a
//! disk or a forge, and every case is a unit test.

use std::path::{Path, PathBuf};

use crate::branches::Placement;

/// Where a candidate came from, in the order the record says them
/// (§FS-005-dispatch.36.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Came {
    Worktree,
    Ledger,
    Plan,
    LayRecord,
    ClosingPr,
    Caller,
}

impl Came {
    pub fn word(self) -> &'static str {
        match self {
            Came::Worktree => "worktree",
            Came::Ledger => "ledger",
            Came::Plan => "plan",
            Came::LayRecord => "lay record",
            Came::ClosingPr => "closing pr",
            Came::Caller => "caller",
        }
    }
}

/// What the recorded checkout of a ledger dispatch is on disk — read by the
/// caller, so the table itself never touches one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Nothing there any more: a checkout that was removed, whose branch is
    /// still a real candidate.
    Gone,
    /// A working tree.
    Checkout,
    /// A directory that is not a working tree. Whether it holds checkouts of
    /// other branches is the table's question, answered from the worktrees.
    Directory,
}

/// One branch the record names.
#[derive(Debug, Clone)]
pub struct Recorded {
    pub name: String,
    pub came: Came,
    /// For a ledger dispatch: the checkout it recorded, and what that is now.
    pub checkout: Option<(PathBuf, Shape)>,
}

/// A working tree git lists for the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
}

/// The candidates, as §FS-005-dispatch.36.1 gathers them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    /// Each candidate once, with every place it came from.
    pub branches: Vec<(String, Vec<Came>)>,
    /// Each placeholder once, with why it is not a branch to check.
    pub discarded: Vec<(String, String)>,
    /// The matter's live worktree, where one is.
    pub worktree: Option<Worktree>,
    /// A recorded checkout now standing on a branch nothing records.
    pub conflict: Option<Worktree>,
}

impl Table {
    pub fn names(&self) -> Vec<String> {
        self.branches.iter().map(|(name, _)| name.clone()).collect()
    }

    fn add(&mut self, name: &str, came: Came) {
        match self.branches.iter_mut().find(|(have, _)| have == name) {
            Some((_, from)) => {
                if !from.contains(&came) {
                    from.push(came);
                    from.sort();
                }
            }
            None => self.branches.push((name.to_string(), vec![came])),
        }
    }
}

/// The two kinds of placeholder, in the words a refusal names them by
/// (§FS-005-dispatch.36.1).
pub const DEFAULT_BRANCH: &str = "the project's default branch";
pub const DIRECTORY: &str = "a directory of checkouts";

/// Which kind of placeholder a discarded branch's reason says it is.
pub fn kind_of(why: &str) -> &'static str {
    match why == DEFAULT_BRANCH {
        true => DEFAULT_BRANCH,
        false => DIRECTORY,
    }
}

/// Why a recorded branch is a placeholder rather than a branch, or `None`
/// (§FS-005-dispatch.36.1): the project's default branch, or a branch whose
/// recorded checkout is a directory of checkouts — one that holds the
/// working trees of other branches, or an emptied one that holds none.
fn placeholder(recorded: &Recorded, main: Option<&str>, worktrees: &[Worktree]) -> Option<String> {
    if main == Some(recorded.name.as_str()) {
        return Some(DEFAULT_BRANCH.to_string());
    }
    let (checkout, Shape::Directory) = recorded.checkout.as_ref()? else {
        return None;
    };
    let under: Vec<&Worktree> = worktrees
        .iter()
        .filter(|tree| tree.path != *checkout && tree.path.starts_with(checkout))
        .collect();
    let of_others = under
        .iter()
        .any(|tree| tree.branch.as_deref() != Some(recorded.name.as_str()));
    (under.is_empty() || of_others).then(|| {
        format!(
            "its recorded checkout {} is {DIRECTORY}, not a checkout",
            checkout.display()
        )
    })
}

/// The candidate table (§FS-005-dispatch.36.1). `closing` is every head
/// branch the forge links as closing the matter; `caller` is `--pr-branch`.
pub fn table(
    recorded: &[Recorded],
    closing: &[String],
    caller: &[String],
    worktrees: &[Worktree],
    main: Option<&str>,
) -> Table {
    let mut table = Table::default();
    for record in recorded {
        match placeholder(record, main, worktrees) {
            Some(why) => {
                if !table.discarded.iter().any(|(name, _)| *name == record.name) {
                    table.discarded.push((record.name.clone(), why));
                }
            }
            None => table.add(&record.name, record.came),
        }
    }
    // A branch recorded once as a placeholder and once for real is real.
    let real = table.names();
    table.discarded.retain(|(name, _)| !real.contains(name));
    for head in closing.iter().filter(|head| !head.is_empty()) {
        table.add(head, Came::ClosingPr);
    }
    let named = table.names();
    let mine = |tree: &&Worktree| tree.branch.as_deref() != main || main.is_none();
    // First the worktree at a checkout the record names, which is the
    // matter's whatever branch it stands on — and conflicts where that branch
    // is one nothing records. Then one on a branch the record names, or on a
    // closing pull request's head.
    let at_recorded = worktrees.iter().filter(mine).find(|tree| {
        recorded.iter().any(|record| {
            record
                .checkout
                .as_ref()
                .is_some_and(|(path, _)| *path == tree.path)
        })
    });
    let found = match at_recorded {
        Some(tree) => match tree.branch.as_ref().filter(|branch| named.contains(branch)) {
            Some(_) => Some(tree.clone()),
            None => {
                table.conflict = Some(tree.clone());
                None
            }
        },
        None => worktrees
            .iter()
            .filter(mine)
            .find(|tree| {
                tree.branch
                    .as_ref()
                    .is_some_and(|branch| named.contains(branch))
            })
            .cloned(),
    };
    if let Some(tree) = found {
        if let Some(branch) = &tree.branch {
            table.add(branch, Came::Worktree);
        }
        table.worktree = Some(tree);
    }
    for branch in caller.iter().filter(|branch| !branch.is_empty()) {
        table.add(branch, Came::Caller);
    }
    table
}

/// Every working tree git lists for the project, canonical, from each
/// repository of the checkout a new working tree would be grown from
/// (§FS-005-dispatch.36.1). Empty where git has nothing to say.
pub fn worktrees(placement: &Placement) -> Vec<Worktree> {
    let Some(source) = placement.source_checkout() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for repo in placement.forest(&source).repos {
        let mut command = crate::git::command();
        crate::git::without_repository_variables(&mut command);
        let Ok(output) = command
            .arg("-C")
            .arg(&repo.path)
            .args(["worktree", "list", "--porcelain"])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        found.extend(parse(&String::from_utf8_lossy(&output.stdout)));
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found.dedup();
    found
}

fn parse(listing: &str) -> Vec<Worktree> {
    let mut out = Vec::new();
    let mut current: Option<Worktree> = None;
    for line in listing.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            out.extend(current.take());
            current = Some(Worktree {
                path: canonical(Path::new(path)),
                branch: None,
            });
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            if let Some(tree) = current.as_mut() {
                tree.branch = Some(branch.to_string());
            }
        }
    }
    out.extend(current);
    out
}

/// What a recorded checkout is now (§FS-005-dispatch.36.1).
pub fn shape(path: &Path) -> Shape {
    if !path.is_dir() {
        return Shape::Gone;
    }
    match path.join(".git").exists() {
        true => Shape::Checkout,
        false => Shape::Directory,
    }
}

pub fn canonical(path: &Path) -> PathBuf {
    if let Ok(found) = std::fs::canonicalize(path) {
        return found;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if parent != path => canonical(parent).join(name),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger(name: &str, checkout: &str, shape: Shape) -> Recorded {
        Recorded {
            name: name.to_string(),
            came: Came::Ledger,
            checkout: Some((PathBuf::from(checkout), shape)),
        }
    }

    fn tree(path: &str, branch: &str) -> Worktree {
        Worktree {
            path: PathBuf::from(path),
            branch: Some(branch.to_string()),
        }
    }

    fn from(table: &Table, name: &str) -> Vec<&'static str> {
        table
            .branches
            .iter()
            .find(|(have, _)| have == name)
            .map(|(_, came)| came.iter().map(|came| came.word()).collect())
            .unwrap_or_default()
    }

    /// §FS-005-dispatch.36.1, case by case: the default branch and a
    /// directory of checkouts are discarded with a reason, a removed checkout
    /// keeps its branch, the closing heads and the caller's branches join,
    /// the worktree is found by its recorded path or its branch, and a
    /// recorded checkout on an unrecorded branch conflicts.
    #[test]
    fn the_branch_candidate_table() {
        let main = Some("main");
        let trees = vec![
            tree("/p/main", "main"),
            tree("/p/fix/issue-249", "fix/issue-249"),
        ];

        // grund#249: `fix` at the directory `/p/fix`, the worktree below it.
        let placeholder = table(
            &[ledger("fix", "/p/fix", Shape::Directory)],
            &["fix/issue-249".to_string()],
            &[],
            &trees,
            main,
        );
        assert_eq!(placeholder.discarded.len(), 1);
        assert_eq!(placeholder.discarded[0].0, "fix");
        assert!(placeholder.discarded[0]
            .1
            .contains("directory of checkouts"));
        assert_eq!(
            from(&placeholder, "fix/issue-249"),
            vec!["worktree", "closing pr"]
        );
        assert_eq!(placeholder.worktree, Some(trees[1].clone()));

        // The same, its worktree removed: the emptied directory is still one.
        let emptied = table(
            &[ledger("fix", "/p/fix", Shape::Directory)],
            &[],
            &[],
            &trees[..1],
            main,
        );
        assert!(emptied.branches.is_empty());
        assert_eq!(emptied.discarded[0].0, "fix");

        // The caller supplies one, and it is only the caller's.
        let supplied = table(
            &[ledger("fix", "/p/fix", Shape::Directory)],
            &[],
            &["fix/issue-249".to_string()],
            &trees[..1],
            main,
        );
        assert_eq!(from(&supplied, "fix/issue-249"), vec!["caller"]);
        assert!(supplied.worktree.is_none(), "--pr-branch makes no worktree");

        // The default branch is never a candidate, and its checkout is never
        // the matter's.
        let trunk = table(
            &[ledger("main", "/p/main", Shape::Checkout)],
            &[],
            &[],
            &trees,
            main,
        );
        assert!(trunk.branches.is_empty());
        assert_eq!(trunk.discarded[0].1, "the project's default branch");
        assert!(trunk.worktree.is_none() && trunk.conflict.is_none());

        // grund#301: the checkout was removed; its branch is still checked.
        let gone = table(
            &[ledger("fix/issue-301", "/p/fix/issue-301", Shape::Gone)],
            &["fix/issue-301".to_string()],
            &[],
            &trees,
            main,
        );
        assert_eq!(from(&gone, "fix/issue-301"), vec!["ledger", "closing pr"]);
        assert!(gone.discarded.is_empty());
        assert!(gone.worktree.is_none());

        // A plan and a lay record name branches too, merged with the ledger's.
        let named = table(
            &[
                ledger("fix/issue-249", "/p/fix/issue-249", Shape::Checkout),
                Recorded {
                    name: "fix/issue-249".to_string(),
                    came: Came::LayRecord,
                    checkout: None,
                },
                Recorded {
                    name: "fix/other".to_string(),
                    came: Came::Plan,
                    checkout: None,
                },
            ],
            &[],
            &[],
            &trees,
            main,
        );
        assert_eq!(
            from(&named, "fix/issue-249"),
            vec!["worktree", "ledger", "lay record"]
        );
        assert_eq!(from(&named, "fix/other"), vec!["plan"]);

        // The recorded checkout now stands on a branch nothing records.
        let moved = table(
            &[ledger("fix/a", "/p/fix/issue-249", Shape::Checkout)],
            &[],
            &[],
            &trees,
            main,
        );
        assert_eq!(moved.conflict, Some(trees[1].clone()));
        assert!(moved.worktree.is_none());
    }

    #[test]
    fn a_porcelain_listing_reads_paths_and_branches() {
        let listing = "worktree /nowhere/main\nHEAD 1\nbranch refs/heads/main\n\n\
                       worktree /nowhere/fix/a\nHEAD 2\ndetached\n";
        let trees = parse(listing);
        assert_eq!(trees.len(), 2);
        assert_eq!(trees[0].branch.as_deref(), Some("main"));
        assert_eq!(trees[1].branch, None);
    }
}
