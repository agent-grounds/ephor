//! `ephor checkout` — making the workspace that is not there
//! (§FS-004-quick-actions.7).
//!
//! The reader presses a key for it, a state machine runs it as a program, and a
//! dispatch mints the workspace a `branch` template named; all three arrive
//! here, so there is one answer to where a project's branch workspace goes and
//! what it holds (§FS-005-dispatch.12). Everything the git fallback needs is
//! already in the registry — the directory template, the repositories, the main
//! branch — which is why nobody has to configure a command for it.
//!
//! And where a project does bind one, that command is the maker here rather
//! than beside it (§FS-006-project-interface.8): honoured *inside* the
//! operation, every caller of the operation gets it at once, which is the only
//! shape in which the sentence above stays true of the code as well as of this
//! comment.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::branches::Placement;
use crate::cli::CheckoutArgs;
use crate::error::{EphorError, Result};
use crate::feed::cache;
use crate::feed::config::{load_config, CheckoutConfig};
use crate::feed::model::Item;
use crate::git;
use crate::given;
use crate::seams::{dossier, summons};

/// What making one branch workspace came to (§FS-004-quick-actions.7), in the
/// shape every caller of [`make`] needs: what was already there, what git did
/// with the rest, and what the store came to.
pub struct Made {
    /// The directory the workspace belongs at — made now, or found there.
    pub target: PathBuf,
    /// Every declared repository was already on disk, so there was no tree
    /// left to make and the store was all this had left to do
    /// (§FS-004-quick-actions.7.1).
    pub already: bool,
    /// The repositories that were absent from a directory that was there, for
    /// the line a reader is owed about what is being made.
    pub missing: Vec<String>,
    /// What git came to, where git was the maker. None where the project's own
    /// checkout command was asked instead (§FS-006-project-interface.8): there
    /// is no per-repository creation to report, because ephor made none of it.
    pub outcome: Option<git::Creation>,
    /// None where the tree is half-made: a work root inside one would be a
    /// place for plans that cannot be worked (§FS-006-project-interface.7).
    ///
    /// Which makes it the one field that says *whether the workspace was made*,
    /// whichever maker was asked — the store goes in exactly where the tree it
    /// belongs to is whole, so a bound command that returned without making the
    /// workspace leaves this `None` and [`Made::refusal`] answers for it.
    pub store: Option<Store>,
}

/// The repositories a workspace is missing, as a person reads them. A project
/// that keeps one repository declares it as `.`, which names the workspace
/// itself rather than a directory inside it, and a project that declares no
/// forest has no name to give at all — so the reading says what was looked for
/// rather than printing a bare dot at a site author (§AR-004-forest.1).
fn absent(missing: &[String]) -> String {
    if missing.is_empty() {
        return "no repository of this project is in it".to_string();
    }
    let names: Vec<&str> = missing
        .iter()
        .map(|name| match name.as_str() {
            "." => "the repository at its root",
            other => other,
        })
        .collect();
    format!("{} not on disk there", names.join(", "))
}

/// Whether the directory a bound command was asked for is a workspace of this
/// project (§FS-006-project-interface.8).
///
/// One predicate because three sites meet this state — the maker's own
/// already-whole answer, the verification after the command returns, and the
/// dispatch's [`half_made`] — and a directory two of them call whole while the
/// third refuses it is the silence this contract is about, one ask later. So
/// *whole* is the whole of it: every declared repository on disk, **and** at
/// least one repository of the project in there. The second clause is what a
/// project whose declared forest is empty needs: with nothing declared there is
/// nothing to be absent, so `absent` alone answers *whole* for a bare directory
/// (§AR-004-forest.2), and `absent(&[])` is the sentence that says why it is
/// not.
///
/// Only where a command is bound. Where nothing is bound, ephor's git has
/// always filled in whatever the fold found missing and a project that declares
/// no forest answers as it always did, which is the fallback's own contract and
/// not this one (§FS-004-quick-actions.7).
pub fn whole(forest: &crate::forest::Forest) -> bool {
    forest.absent.is_empty() && !forest.repos.is_empty()
}

/// Why a directory that is already there is not a workspace this project's own
/// command may be asked for, where it is not one (§FS-006-project-interface.8).
///
/// Neither maker may finish what it did not start: ephor's git does not fill in
/// a tree the command did not make, because the command owns what a workspace of
/// this project is, and a directory that is already there is never handed back
/// to the command either — a command written to create one would be run over a
/// half-made tree. So what is absent is named and the checkout refuses, and
/// clearing the directory is the one move that gets the site its workspace.
///
/// One sentence in one place because two surfaces meet this state and have to
/// say the same thing about it: the maker, before it summons anything, and the
/// dispatch, which resolves *checked out* from the directory alone and would
/// otherwise promise a ticket behind it (§FS-005-dispatch.25). It asks
/// [`whole`], so it is the same question the other two sites ask rather than a
/// third reading of the same directory.
pub fn half_made(
    project: &str,
    bound: &CheckoutConfig,
    target: &Path,
    forest: &crate::forest::Forest,
) -> Option<String> {
    (!whole(forest)).then(|| {
        format!(
            "{} is there, but it is not a workspace of {project}: {}. {project}'s own checkout \
             command is what makes its workspaces (`{}`), and ephor neither fills in a tree that \
             command did not make nor hands it back a directory it did not make — remove {} and \
             ask for the checkout again.",
            target.display(),
            absent(&forest.absent),
            bound.command,
            target.display(),
        )
    })
}

/// Who made a branch workspace (§FS-006-project-interface.8). The fact a
/// reading owes whoever asked, because the two answers hold different things:
/// the project's own command decides what a workspace of this project *is*, and
/// ephor's git is what answers where nothing is bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Maker {
    /// The command the project bound.
    Command,
    /// ephor's own git operation, which is the fallback and not a second maker.
    Git,
}

impl Maker {
    pub fn name(self) -> &'static str {
        match self {
            Maker::Command => "command",
            Maker::Git => "git",
        }
    }
}

impl Made {
    /// Which maker made this workspace, where one made anything now
    /// (§FS-006-project-interface.8). None rather than a made-up answer where
    /// nothing was made: a workspace that was already whole, and a bound
    /// command that returned without making one (§REQ-002-parity.4).
    pub fn maker(&self) -> Option<Maker> {
        if self.already {
            return None;
        }
        match &self.outcome {
            Some(outcome) => (!outcome.repos.is_empty()).then_some(Maker::Git),
            // The store is what says the tree is whole, so it is what says the
            // command made the workspace it was asked for.
            None => self.store.as_ref().map(|_| Maker::Command),
        }
    }

    /// Why this is not a workspace, where it is not: nothing to make it from, a
    /// repository the checkout refused, or a bound command that returned
    /// without making the one it was asked for. Half a workspace is not one —
    /// whatever is missing, the next thing to run in here would fail on it.
    ///
    /// Returned rather than printed, because the two callers answer for it
    /// differently: the command has already reported every repository and
    /// stops at an exit code, and a dispatch has written nothing yet and
    /// refuses in the checkout's own words (§FS-005-dispatch.25).
    pub fn refusal(&self, source: &Path) -> Option<String> {
        let Some(outcome) = self.outcome.as_ref() else {
            // No git creation, so either the workspace was already whole or the
            // project's own command was the maker. *Verified* is the directory
            // and every repository the project declares, never the exit code,
            // and ephor does not fill in a tree it did not make
            // (§FS-006-project-interface.8).
            if self.already || self.store.is_some() {
                return None;
            }
            return Some(format!(
                "The project's own checkout command returned, but {} is not a workspace: {}.",
                self.target.display(),
                absent(&self.missing)
            ));
        };
        if outcome.repos.is_empty() {
            return Some(format!(
                "No repository under {} to make a workspace from.",
                source.display()
            ));
        }
        (!outcome.refused().is_empty()).then(|| outcome.report())
    }
}

/// What a base was given as, and the input it arrived on — the flag a reader
/// typed or the name a program state set (§FS-011-command-line.9).
///
/// Both, because where the project binds a checkout command the base is that
/// command's to decide and the value is refused: a refusal has to name the
/// spelling the caller actually used, and only the caller knows it. Nothing
/// inside ephor passes a base at all.
pub struct Base<'a> {
    pub value: &'a str,
    pub input: &'a str,
}

/// What one caller asks the maker for (§FS-004-quick-actions.7).
///
/// One struct rather than six positional arguments, because every caller passes
/// every one of them and the next caller would add a seventh: a call site of
/// six anonymous options is how the third caller of this operation came to pass
/// the wrong thing without anybody noticing.
pub struct Ask<'a> {
    pub placement: &'a Placement,
    pub project: &'a str,
    /// The branch this checkout is making — the name a reader typed, the one a
    /// state machine set, or the one a `branch` template minted.
    pub branch: &'a str,
    pub from: Option<Base<'a>>,
    /// The hand-off's selected work root, where a recipe or workflow entry
    /// chose one after branch placement.
    pub selected_root: Option<&'a Path>,
    /// The matter this is about, where there is one. A checkout asked for by
    /// branch alone is not one, and saying so is what keeps a stand-in matter
    /// out of the command's dossier (§FS-006-project-interface.8).
    pub about: Option<&'a Item>,
}

/// Make the branch workspace `ask.branch` belongs in, or find it already made
/// (§FS-004-quick-actions.7).
///
/// The one implementation of that operation, for every caller: the key the
/// reader presses, the command a state machine runs, and the dispatch that
/// makes the workspace a `branch` template named (§FS-005-dispatch.25). Two
/// of them would eventually disagree about what a checked-out workspace is —
/// which repositories it holds, what its branches are grown from, whether it
/// has a store — and the disagreement would be discovered by work landing in
/// a directory that is not one.
///
/// Which is also why the project's own bound command is summoned from in here
/// (§FS-006-project-interface.8) rather than by each caller that knows about
/// it: a binding honoured on some of a seam's paths and not the others is a
/// seam that is not done (§REQ-001-boundary.1), and it fails in the quietest
/// way there is, because a directory is there either way.
///
/// It writes nothing to the registry and pushes nothing: a workspace is found
/// on disk like every other (§FS-008-attribution.2), and publishing a branch
/// is the work's move.
pub fn make(ask: &Ask) -> Result<(Made, PathBuf)> {
    let (placement, project, branch) = (ask.placement, ask.project, ask.branch);
    // Where the workspace goes is settled before the first directory can be
    // created (§FS-004-quick-actions.7.3), and here rather than at the command
    // line alone: this is the implementation every caller shares
    // (§FS-005-dispatch.12), so the name a reader typed, the one a state
    // machine set and the one a `branch` template minted are all held to it.
    // All three hold whichever maker will be asked, so a command is never
    // handed a directory this project would not have put a workspace at.
    if !crate::forest::is_branch_name(branch) {
        return Err(EphorError::Registry(format!(
            "'{branch}' is a placeholder nothing filled, not a branch to check out."
        )));
    }
    if let Some(why) = crate::branches::why_git_refuses(branch) {
        return Err(EphorError::Registry(format!(
            "git will not take '{branch}' as a branch name: {why}."
        )));
    }
    let target = placement.workspace_for(branch).ok_or_else(|| {
        EphorError::Command(format!(
            "{project} does not use a checkout per branch (no branch_root_template), so \
             there is no workspace to make for {branch} — its root is the checkout."
        ))
    })?;
    // Read once and answered three times: the work root says whether this name
    // lands on the place plans go, the same reading puts the store there once
    // the tree is whole (§FS-006-project-interface.7), and the same reading
    // says whether this project bound a maker of its own.
    let work = Work::read(placement, project);
    if let Some(why) =
        crate::branches::why_the_workspace_is_refused(placement, branch, &work.root())
    {
        return Err(EphorError::Registry(why));
    }
    let bound = work.checkout();
    // A value this operation will not act on is refused naming the input it
    // came in on, before anything is made rather than inside somebody else's
    // making (§FS-004-quick-actions.7.4, §FS-011-command-line.9).
    if let (Some(bound), Some(from)) = (&bound, &ask.from) {
        return Err(EphorError::Registry(format!(
            "{} was given '{}', which {project}'s own checkout command decides — it is the \
             maker of this workspace (`{}`), and a base is not among the things it is told.",
            from.input, from.value, bound.command
        )));
    }
    // A directory is not a workspace: the declared repositories are what make
    // it one. This is the operation whose answer says whether the workspace is
    // whole (§AR-004-forest.1), so a directory that is there is asked which of
    // them are — by path, which is what tells presence (§AR-004-forest.3) —
    // and only a whole one stops here. Where a command is bound that is
    // [`whole`], the one predicate the verification and the dispatch ask too,
    // so this end of the function cannot call a directory whole that the other
    // end refuses (§FS-006-project-interface.8). Where nothing is bound, a
    // project that declares no forest has nothing to be missing and answers as
    // it always did.
    // None where there is no directory to judge, which is the ask the maker is
    // for: there is nothing half-made about a workspace that is simply absent.
    let standing = target.is_dir().then(|| placement.forest(&target));
    let mut missing = Vec::new();
    if let Some(forest) = &standing {
        missing = forest.absent.clone();
        let already = match bound.is_some() {
            true => whole(forest),
            false => missing.is_empty(),
        };
        if already {
            // Every repository is here, so there is no tree left to make. The
            // store still may be: a workspace made before ephor made stores at
            // all, or made by the project's own checkout command, holds every
            // repository it should and has nowhere for a plan to land
            // (§FS-004-quick-actions.7.1). Asking again is what repairs it.
            let store = init_store(&work, placement, project, &target, ask.selected_root);
            return Ok((
                Made {
                    target: target.clone(),
                    already: true,
                    missing,
                    outcome: None,
                    store: Some(store),
                },
                target,
            ));
        }
    }

    // The workspace is absent, so this is the ask the project bound its command
    // for (§FS-006-project-interface.8) — unless this very operation is already
    // inside one making it, which is what lets a command wrap `ephor checkout`
    // instead of summoning itself for ever.
    if let Some(bound) = bound.filter(|_| !nested(project, branch)) {
        // Except where a directory is there without being a workspace, which
        // neither maker may finish: ephor's git would fill in a tree the command
        // did not make, and a directory that is already there is never handed
        // back to the command either (§FS-006-project-interface.8).
        if let Some(why) = standing
            .as_ref()
            .and_then(|forest| half_made(project, &bound, &target, forest))
        {
            return Err(EphorError::Command(why));
        }
        return summoned(&bound, ask, &work, target, missing);
    }

    // A working tree is added from a repository, so one has to be on disk —
    // and not this one: a half-made workspace cannot supply the repositories it
    // is itself missing, and taking it as the source would answer *no
    // repository at* per repository instead of saying there is nothing to grow
    // them from. The requirement is the git fallback's alone: a bound command is
    // asked what it makes a workspace from (§FS-004-quick-actions.7.3).
    let source = placement
        .source_checkout()
        .filter(|source| source != &target)
        .ok_or_else(|| {
            EphorError::Command(format!(
                "{project} has no checkout on disk to make {} from — clone the project first.",
                target.display()
            ))
        })?;

    // The shape of the workspace being made is the shape of the one it is made
    // from: the declared forest where the row declares one, the source
    // checkout's own repositories otherwise (§AR-004-forest.1).
    let forest = placement.forest(&source);
    let base = match ask
        .from
        .as_ref()
        .map(|from| from.value.to_string())
        .or_else(|| placement.main_branch.clone())
    {
        Some(base) => base,
        None => forest
            .repos
            .first()
            .and_then(|repo| git::default_base(&repo.path, &repo.remote))
            .ok_or_else(|| {
                EphorError::Command(format!(
                    "Nothing says what to grow {branch} from — pass --from, or give \
                     {project} a main_branch in the registry."
                ))
            })?,
    };

    let outcome = git::create(&source, &target, &forest, branch, &base);
    // The store goes in only where the tree it belongs to is whole: a half-made
    // workspace is refused by the caller, and a work root inside one would be a
    // place for plans that cannot be worked (§FS-006-project-interface.7).
    let store = (outcome.refused().is_empty() && !outcome.repos.is_empty())
        .then(|| init_store(&work, placement, project, &target, ask.selected_root));
    Ok((
        Made {
            target,
            already: false,
            missing,
            outcome: Some(outcome),
            store,
        },
        source,
    ))
}

/// The one name a bound checkout command is told about ephor itself
/// (§FS-006-project-interface.8): what is being made, so a command that wraps
/// `ephor checkout` composes instead of summoning itself for ever.
///
/// The value is one `project:branch` per line, outermost first — a list rather
/// than a single pair because a command may legitimately wrap the checkout of a
/// *different* branch, and what ends the recursion is the same ask arriving
/// twice rather than the second ask arriving at all.
pub const MAKING: &str = "EPHOR_CHECKOUT_MAKING";

fn making(project: &str, branch: &str) -> String {
    format!("{project}:{branch}")
}

/// The marker to hand the command: whatever is already under way, and this.
fn marker(project: &str, branch: &str) -> String {
    let mut under_way: Vec<String> = held().collect();
    under_way.push(making(project, branch));
    under_way.join("\n")
}

/// Whether a bound command making this very workspace is what this operation is
/// running inside. Read from the environment because that is what crosses a
/// process boundary: the wrapper is a shell script and the nested maker is
/// another `ephor` (§REQ-001-boundary.1).
fn nested(project: &str, branch: &str) -> bool {
    let mine = making(project, branch);
    held().any(|under_way| under_way == mine)
}

fn held() -> impl Iterator<Item = String> {
    std::env::var(MAKING)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>()
        .into_iter()
}

/// Ask the command the project bound to make the workspace, and hold it to what
/// it came back having made (§FS-006-project-interface.8).
///
/// Making the repositories is the command's contract and the store is never its
/// to make, so the two makers meet again here: the same fold decides whether
/// the workspace is whole (§AR-004-forest.1), the store goes in exactly where
/// it does on the other path (§FS-004-quick-actions.7.1), and what the caller
/// is handed back is one shape either way.
fn summoned(
    bound: &CheckoutConfig,
    ask: &Ask,
    work: &Work,
    target: PathBuf,
    missing: Vec<String>,
) -> Result<(Made, PathBuf)> {
    let (placement, project) = (ask.placement, ask.project);
    let root = placement.root.clone();
    // The vocabulary is the one every other summons carries
    // (§FS-005-dispatch.8), including its empties: a summons does not start
    // from a cleared environment, so a name a matter would have answered is set
    // and empty where there is no matter rather than inherited from whatever
    // launched ephor (§AR-002-summons.1).
    let standing = placement.forest(&target);
    // The registry branch behind this ask, which is where `EPHOR_TICKET` comes
    // from: the matter's own names are the matter's where there is one, and the
    // ticket key the row carries is one of them (§FS-006-project-interface.8).
    // Read here from the placement the ask already carries rather than taken as
    // a field every caller would have to remember to fill — one caller
    // forgetting it is how the name came to be emptied in the first place
    // (§REQ-001-boundary.1).
    let matched = match ask.about {
        Some(item) => placement.matched(item),
        // A checkout asked for by branch alone has no matter to match, so the
        // row for the branch being made is what answers — the same branch the
        // chain's own row is read from.
        None => placement
            .branches
            .iter()
            .find(|info| info.branch == ask.branch),
    };
    // The organization the registry places the project in comes from the
    // placement the ask already carries, the same read every other summons
    // resolves it through, so a checkout's command cannot be told a different
    // organization than a menu action on the same project (§FS-005-dispatch.8).
    let organization = placement.organization.as_ref();
    let mut carrying = match ask.about {
        Some(item) => {
            dossier::of_item(item, &root, &target, organization, matched, Some(&standing))
        }
        None => dossier::of_branch(
            project,
            &root,
            &target,
            organization,
            matched,
            Some(&standing),
        ),
    };
    // `EPHOR_BRANCH` is the branch this checkout is *making*. The matter's own
    // answer is the wrong one and on the dispatch's path it is empty, which is
    // exactly where the command needs it most: nobody has cut the branch a
    // `branch` template minted (§FS-005-dispatch.25).
    carrying.retain(|(name, _)| name != "EPHOR_BRANCH" && name != MAKING);
    carrying.push(("EPHOR_BRANCH".to_string(), ask.branch.to_string()));
    carrying.push((MAKING.to_string(), marker(project, ask.branch)));

    // Placed at the project's root: the workspace is not there to run in yet,
    // and the root is what lets a command reach the checkouts beside the one it
    // is making by relative path. Run aside, because the surface that asked for
    // this owns its standard output — the command's own output still reaches
    // whoever is watching (§REQ-002-parity.3).
    let summons = summons::Summons::new("checkout", &bound.command)
        .at(summons::Place::Root)
        .carrying(carrying);
    let answer = summons::run(&summons, &summons::Site::root(&root), summons::Mode::Aside)?;
    if !answer.is_done() {
        // A non-zero exit is the checkout not made, and `75` is among them:
        // there is no *parked* here, since a workspace either exists or it does
        // not (§FS-006-project-interface.8).
        return Err(EphorError::Command(match answer.exit_code {
            Some(code) => format!(
                "{project}'s own checkout command exited {code} without making {} — nothing \
                 was made and nothing was done behind it.",
                target.display()
            ),
            None => format!(
                "{project}'s own checkout command was killed without making {}.",
                target.display()
            ),
        }));
    }
    // *Verified* is [`whole`] — the directory, every repository the project
    // declares in it, and a repository of this project in there at all — never
    // the exit code. One the command did not make is named and the checkout is
    // refused rather than completed: ephor's git does not fill in a tree it did
    // not make, because the command owns what a workspace of this project is
    // (§FS-006-project-interface.8).
    let forest = placement.forest(&target);
    let made = target.is_dir() && whole(&forest);
    let store = made.then(|| init_store(work, placement, project, &target, ask.selected_root));
    Ok((
        Made {
            target,
            already: false,
            missing: match made {
                true => missing,
                false => forest.absent,
            },
            outcome: None,
            store,
        },
        root,
    ))
}

pub fn checkout(args: &CheckoutArgs) -> Result<ExitCode> {
    // Every value this command takes is honoured or refused naming the input it
    // came in on, before the first of them is acted on
    // (§FS-011-command-line.9). What the reader passed decides the answer, so
    // one they passed and ephor cannot use may not become one they did not.
    let item = match given::value(&args.item, "ITEM")? {
        Some(id) => Some(find_item(&id)?),
        None => None,
    };
    let project = given::value(&args.project, "PROJECT")?
        .or_else(|| item.as_ref().map(|item| item.project.clone()))
        .ok_or_else(|| {
            EphorError::Command(
                "Nothing says which project this branch belongs to — pass --project or --item."
                    .to_string(),
            )
        })?;

    let registry = crate::feed::commands::load_registry_doc()?;
    let placement = Placement::load(&registry, &project).ok_or_else(|| {
        EphorError::Command(format!(
            "{project}: no root in the registry, so there is nowhere to put a checkout."
        ))
    })?;

    let branch = given::branch(&args.branch, "BRANCH")?
        .or_else(|| item.as_ref().and_then(|item| placement.branch_name(item)))
        .ok_or_else(|| {
            EphorError::Command(
                "Nothing says which branch to check out — pass --branch, or --item for one \
                 the feed knows the branch of."
                    .to_string(),
            )
        })?;

    // Read before the making rather than where each is used: a value this
    // command will not act on is refused instead of the workspace being made
    // and the refusal arriving afterwards (§FS-004-quick-actions.7.3).
    let from = given::branch(&args.from, "FROM")?;
    let from_input = given::input(&args.from, "FROM");
    let report = given::value(&args.report, "REPORT")?;

    // Everything below is reporting: what the workspace came to is the one
    // operation's answer, and this command is the reading of it
    // (§AR-009-surfaces.1).
    let (made, source) = make(&Ask {
        placement: &placement,
        project: &project,
        branch: &branch,
        // The input as well as the value, because a base this project's own
        // checkout command decides is refused naming the spelling the caller
        // used (§FS-004-quick-actions.7.4).
        from: from
            .as_deref()
            .zip(from_input.as_deref())
            .map(|(value, input)| Base { value, input }),
        selected_root: None,
        about: item.as_ref(),
    })?;
    if made.already {
        let behind = distance(&placement, &made.target);
        let summary = match &behind {
            Some(behind) => format!(
                "{} is already checked out, {}",
                made.target.display(),
                behind.says(placement.main_branch.as_deref().unwrap_or("its base"))
            ),
            None => format!("{} is already checked out", made.target.display()),
        };
        if args.json {
            let mut view = serde_json::json!({
                "workspace": made.target,
                "branch": branch,
                "ready": true,
                "summary": summary,
                "repos": [],
                "store": made.store.as_ref().map(Store::view),
            });
            // Absent rather than `null` typed as the fact would have been: the
            // published shape says a distance is an object, and a workspace on
            // which nothing could be measured states none
            // (§FS-004-quick-actions.7.1, §REQ-002-parity.4).
            if let (Some(object), Some(behind)) = (view.as_object_mut(), &behind) {
                object.insert("behind".to_string(), serde_json::json!(behind));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&view).unwrap_or_else(|_| "null".to_string())
            );
        } else {
            println!("{summary}.");
            if let Some(store) = &made.store {
                store.say();
            }
        }
        return Ok(ExitCode::SUCCESS);
    }
    // Where the project's own command was the maker there is no per-repository
    // creation to report, because ephor made none of it — so the reading says
    // who made the workspace and what the store came to, and the command's own
    // output has already been the reader's (§FS-006-project-interface.8).
    let Some(outcome) = made.outcome.as_ref() else {
        if let Some(why) = made.refusal(&source) {
            // The two makers' refusals leave the same trace. `--report` is the
            // file a caller reads the checkout out of rather than the terminal,
            // and the git path writes it before its own refusal is tested
            // below — a bound command's refusal reaching that reader as an
            // absent file would be the one maker whose failure is silent
            // exactly where it is being watched (§REQ-002-parity.3).
            if let Some(path) = report {
                write_report(&path, &format!("{why}\n"))?;
            }
            return Err(EphorError::Command(why));
        }
        let summary = format!(
            "{project}'s own checkout command made {}",
            made.target.display()
        );
        if args.json {
            let mut view = serde_json::json!({
                "workspace": made.target,
                "branch": branch,
                "ready": true,
                "summary": summary,
                "repos": [],
                "store": made.store.as_ref().map(Store::view),
            });
            // Absent rather than `null` typed as the fact would have been
            // (§REQ-002-parity.4) — the same rule the distance follows above.
            if let (Some(object), Some(maker)) = (view.as_object_mut(), made.maker()) {
                object.insert(
                    "maker".to_string(),
                    serde_json::Value::String(maker.name().to_string()),
                );
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&view).unwrap_or_else(|_| "null".to_string())
            );
        } else {
            println!("{summary}.");
            if let Some(store) = &made.store {
                store.say();
            }
        }
        if let Some(path) = report {
            write_report(&path, &format!("{summary}.\n"))?;
        }
        return Ok(ExitCode::SUCCESS);
    };
    if !args.json && !made.missing.is_empty() {
        println!(
            "{} is missing {} — making {}.",
            made.target.display(),
            made.missing.join(", "),
            if made.missing.len() == 1 {
                "it"
            } else {
                "them"
            }
        );
    }
    if args.json {
        let mut view = outcome.view();
        if let Some(object) = view.as_object_mut() {
            object.insert(
                "report".to_string(),
                serde_json::Value::String(outcome.report()),
            );
            // Which maker made it, absent rather than `null` where nothing was
            // made — the same rule the distance above follows
            // (§REQ-002-parity.3, §REQ-002-parity.4).
            if let Some(maker) = made.maker() {
                object.insert(
                    "maker".to_string(),
                    serde_json::Value::String(maker.name().to_string()),
                );
            }
            if let Some(store) = &made.store {
                object.insert("store".to_string(), store.view());
            }
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&view).unwrap_or_else(|_| "null".to_string())
        );
    } else {
        print!("{}", outcome.report());
    }
    if let Some(path) = report {
        write_report(&path, &outcome.report())?;
    }

    if let Some(why) = made.refusal(&source) {
        // Nothing to make it from is this command refusing. Half a workspace is
        // not one either — whatever is missing, the next thing to run in here
        // would fail on it — but every repository has already been reported
        // above, so that one stops at the exit code.
        if outcome.repos.is_empty() {
            return Err(EphorError::Command(why));
        }
        return Ok(ExitCode::from(1));
    }
    if !args.json {
        println!("{}", outcome.summary());
        if let Some(store) = &made.store {
            store.say();
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// How far a workspace that is already there trails the project's main branch
/// (§FS-004-quick-actions.7.1).
///
/// The same fold over the same forest the branch row for this very directory
/// is rendered from (§AR-004-forest.1) — not a second measurement, which is
/// what keeps the two surfaces from ever saying different things about one
/// workspace. Gated exactly as the row is: a project that names no main branch
/// has no distance to state, and a forest on which nothing could be measured
/// says none rather than a made-up zero.
fn distance(placement: &Placement, workspace: &Path) -> Option<crate::api::views::Distance> {
    placement.main_branch.as_deref()?;
    placement
        .forest(workspace)
        .staleness()
        .trail()
        .map(crate::api::views::Distance::from)
}

/// This project's work configuration, read once at the three tiers a work root
/// resolves through — the site's, its organization's, and its own
/// (§FS-005-dispatch.6.1).
///
/// The checkout asks for it twice and must get one answer both times: before
/// anything is made, to know whether the branch name lands on the work root
/// (§FS-004-quick-actions.7.3), and after, to put the store there
/// (§FS-006-project-interface.7). Two readings of one configuration would
/// eventually name two directories, and the second of them would be where the
/// plans went.
struct Work<'a> {
    /// None where no site configuration can be read: the shipped default
    /// answers then, which is `ephor checkout` working on a machine that has a
    /// registry and nothing else.
    config: Option<crate::feed::config::StatusConfig>,
    placement: &'a Placement,
    project: &'a str,
}

impl<'a> Work<'a> {
    fn read(placement: &'a Placement, project: &'a str) -> Work<'a> {
        Work {
            config: load_config().ok(),
            placement,
            project,
        }
    }

    fn global(&self) -> crate::work::recipe::WorkConfig {
        self.config
            .as_ref()
            .map(|config| config.work.clone())
            .unwrap_or_default()
    }

    /// The checkout command this project bound, where it bound one
    /// (§FS-006-project-interface.8). Site configuration and nothing else: the
    /// maker is one of a seam's four parts and the seam's configured binding is
    /// the person's to write (§REQ-001-boundary.1, §REQ-001-boundary.2).
    fn checkout(&self) -> Option<CheckoutConfig> {
        self.config
            .as_ref()?
            .projects
            .get(self.project)?
            .checkout
            .clone()
    }

    fn per_project(&self) -> Option<&crate::work::recipe::ProjectWorkConfig> {
        self.config
            .as_ref()
            .and_then(|config| config.projects.get(self.project))
            .map(|project| &project.work)
    }

    /// The organization tier, read through the membership the registry declares
    /// (§FS-005-dispatch.6.1) — the same resolution dispatch makes, because the
    /// work root is one answer for both.
    fn per_organization(&self) -> Option<&crate::work::recipe::OrganizationWorkConfig> {
        let placed_in = self.placement.organization.as_ref()?;
        self.config
            .as_ref()?
            .organizations
            .get(&placed_in.id)
            .map(|organization| &organization.work)
    }

    /// Where this project's work goes, as the template says it.
    fn root(&self) -> String {
        crate::work::root_template(&self.global(), self.per_organization(), self.per_project())
    }
}

/// A workspace ephor makes gets a task store, so the first dispatch into
/// this branch has somewhere to land and what is under way is visible from
/// the moment the tree exists (§FS-006-project-interface.7). A workspace that
/// was already there is owed it just the same: *already checked out* answers
/// the question about repositories, not the one about work
/// (§FS-004-quick-actions.7.1).
///
/// Reported and never fatal: the workspace is made either way, and a checkout
/// that failed because a convenience did is a checkout that did not need to
/// fail. Where no site configuration can be read, the shipped default answers
/// — this is `ephor checkout` working on a machine that has a registry and
/// nothing else.
fn init_store(
    work: &Work,
    placement: &Placement,
    project: &str,
    workspace: &std::path::Path,
    selected_root: Option<&std::path::Path>,
) -> Store {
    let result = match selected_root {
        Some(root) => crate::work::ensure_store_at(&work.global(), work.per_project(), root),
        None => crate::work::ensure_store(
            &work.global(),
            work.per_organization(),
            work.per_project(),
            project,
            placement.organization.as_ref(),
            workspace,
            &placement.root,
        ),
    };
    match result {
        Ok(store) => Store {
            dir: Some(store.dir),
            made: store.made,
            note: store.note,
        },
        Err(err) => Store {
            dir: None,
            made: false,
            note: Some(format!("no task store was made — {err}")),
        },
    }
}

/// What the store came to, in the shape both answers need: the reading prints
/// it and `--json` carries it, so a runtime is told what a reader is told
/// (§REQ-002-parity.3).
pub struct Store {
    /// None where none could be made; the note says why.
    pub dir: Option<PathBuf>,
    pub made: bool,
    pub note: Option<String>,
}

impl Store {
    /// The line worth printing, where there is one: what was made now, and
    /// whatever could not be. A store that was already there says nothing —
    /// the reader asked for a checkout and it changed nothing.
    pub fn say(&self) {
        if let (true, Some(dir)) = (self.made, &self.dir) {
            println!("  task store at {}", dir.display());
        }
        if let Some(note) = &self.note {
            eprintln!("note: {note}");
        }
    }

    pub fn view(&self) -> serde_json::Value {
        serde_json::json!({
            "dir": self.dir,
            "made": self.made,
            "note": self.note,
        })
    }
}

fn write_report(path: &str, contents: &str) -> Result<()> {
    let path = PathBuf::from(crate::paths::resolve_path(path));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            EphorError::Command(format!("Cannot create {}: {err}", parent.display()))
        })?;
    }
    std::fs::write(&path, contents)
        .map_err(|err| EphorError::Command(format!("Cannot write {}: {err}", path.display())))
}

/// The item by its feed id, out of whatever the last refresh cached.
fn find_item(id: &str) -> Result<Item> {
    let config = load_config()?;
    for project in config.projects.keys() {
        let Some(feed) = cache::load_feed(project)? else {
            continue;
        };
        let found = feed.items().find(|item| item.id == id);
        if let Some(item) = found {
            return Ok(item);
        }
    }
    Err(EphorError::Command(format!(
        "{id} is not in any cached feed — run `ephor refresh` first."
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A checkout that was not made must not read as one that was
    /// (§FS-006-project-interface.8). This is the shape a bound command leaves
    /// when it returns without making the workspace: nothing was already
    /// there, git was never asked, the declared repositories are absent, and
    /// no store went into a directory that is not a workspace. The caller asks
    /// one question about that — [`Made::refusal`] — and it has to answer,
    /// because the caller that asks it is a dispatch deciding whether to write
    /// a plan (§FS-005-dispatch.25).
    #[test]
    fn a_workspace_that_was_not_made_is_a_refusal() {
        let made = Made {
            target: PathBuf::from("/demo/fix/issue-95"),
            already: false,
            missing: vec!["ce".to_string(), "ee".to_string()],
            outcome: None,
            store: None,
        };

        let why = made
            .refusal(Path::new("/demo/main"))
            .expect("a workspace nothing made is a refusal, not a success");
        assert!(
            why.contains("/demo/fix/issue-95"),
            "the refusal names the workspace that was not made: {why}"
        );
        for absent in &made.missing {
            assert!(
                why.contains(absent.as_str()),
                "the refusal names the declared repository that is not there: {why}"
            );
        }
    }
}
