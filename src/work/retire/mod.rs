//! `ephor work retire` (§FS-005-dispatch.36): put a finished matter's leftover
//! work away, unchanged, in an archive beside ephor's own data, under the
//! runtime's own exclusion, and write down what went where and why it was
//! allowed.
//!
//! The order is the spec's. The forge is asked whether the matter is over and
//! what is still open from its branches (§FS-005-dispatch.36.1); the plans the
//! record and the live checkout witness as the matter's are read
//! (§FS-005-dispatch.36.2); the runtime's locks are taken and everything is
//! read again under them (§FS-005-dispatch.36.3); the archive's placement and
//! what git tracks are checked (§FS-005-dispatch.36.4); then the record, the
//! moves, the ledger and the record again (§FS-005-dispatch.36.5). Every guard
//! that refuses does so before the first write, in one sentence, with the
//! guard's name (§FS-005-dispatch.36.6).

mod archive;
mod candidates;
mod witness;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::api::views::{
    RetireBranch, RetireDiscarded, RetireEligibility, RetireMerged, RetireOpen, RetirePlan,
    RetireRefusal, RetireState, RetireTask, WorkRetire,
};
use crate::branches::Placement;
use crate::cli::WorkRetireArgs;
use crate::error::{EphorError, Result};
use crate::feed::config::StatusConfig;
use crate::feed::render::Style;
use crate::forge::closure::{Closure, ClosureError, Pull, State, Subject};
use crate::work::ledger::{self, Entry, Ledger, RetiredPlan, Retirement};
use crate::work::runtime;
use crate::work::runtime::exclusion::{self, Held, Unit};
use crate::work::runtime::plan::{self, Plan, WorkRoot};
use crate::work::{recorded_plans, retired, Dispatcher};

use archive::Move;
use candidates::{canonical, Came, Recorded, Table};
use witness::{Verdict, Witnesses};

/// How long the forge gets to answer whether the matter is over: a question
/// asked once, at the moment it matters (§FS-005-dispatch.36.1).
const FORGE_TIMEOUT: Duration = Duration::from_secs(60);

/// The surface a retirement from this command is recorded as coming from
/// (§FS-005-dispatch.36.5).
const SURFACE: &str = "cli";

/// Why a retirement stopped.
enum Stop {
    /// A guard refused before anything moved (§FS-005-dispatch.36.6).
    Refused(RetireRefusal),
    /// Something failed once the record was written; every move that could be
    /// put back was (§FS-005-dispatch.36.5).
    Failed(String),
}

fn refused(guard: &'static str, says: impl Into<String>) -> Stop {
    refused_naming(guard, says, Vec::new())
}

fn refused_naming(guard: &'static str, says: impl Into<String>, paths: Vec<PathBuf>) -> Stop {
    Stop::Refused(RetireRefusal {
        guard,
        says: says.into(),
        paths,
    })
}

/// A root the runtime holds, in the exclusion's own sentence
/// (§FS-005-dispatch.36.3).
fn live(refusal: exclusion::Refused) -> Stop {
    refused_naming("live-root", refusal.says, vec![refusal.root])
}

/// `ephor work retire` (§FS-005-dispatch.36.6).
pub fn command(config: &StatusConfig, args: &WorkRetireArgs) -> Result<ExitCode> {
    // §FS-005-dispatch.36.2: the admission names the matter it vouches for.
    if !args.admit.is_empty() && args.item.is_none() {
        return Err(EphorError::Command(
            "--admit vouches for a plan as one matter's, so it is taken only beside --item, \
             which names that matter"
                .to_string(),
        ));
    }
    let mut dispatcher = Dispatcher::load(config)?;
    let item = match &args.item {
        Some(item) => item.clone(),
        None => resolve(&mut dispatcher, args)?,
    };
    let Some(entry) = dispatcher.ledger.entries.get(&item).cloned() else {
        return Err(EphorError::Command(format!(
            "the ledger records no work about {item}, so nothing of it can be retired"
        )));
    };
    let mut report = WorkRetire {
        item: item.clone(),
        project: entry.project.clone(),
        dry_run: args.dry_run,
        ..WorkRetire::default()
    };
    let code = match run(config, &mut dispatcher, args, &entry, &mut report) {
        Ok(()) => 0,
        Err(Stop::Refused(refusal)) => {
            report.retired = false;
            report.says = refusal.says.clone();
            report.refused = Some(refusal);
            1
        }
        Err(Stop::Failed(says)) => {
            report.retired = false;
            report.says = says;
            1
        }
    };
    print(&report, args.json);
    Ok(ExitCode::from(code))
}

/// The matter a branch, or the checkout the caller stands in, identifies
/// (§FS-005-dispatch.36.6): exactly one, or a refusal naming them.
fn resolve(dispatcher: &mut Dispatcher, args: &WorkRetireArgs) -> Result<String> {
    let (project, checkout, branch) = match (&args.project, &args.branch) {
        (Some(project), Some(branch)) => {
            let checkout = dispatcher
                .placement(project)
                .and_then(|placement| placement.workspace_for(branch))
                .ok_or_else(|| {
                    EphorError::Command(format!(
                        "the registry places no checkout of {project} for {branch}; pass --item"
                    ))
                })?;
            (project.clone(), checkout, Some(branch.clone()))
        }
        _ => {
            let here = std::env::current_dir().map_err(|err| {
                EphorError::Command(format!("Cannot read the working directory: {err}"))
            })?;
            let project = project_holding(&dispatcher.registry_doc, &here).ok_or_else(|| {
                EphorError::Command(format!(
                    "{} is in no registered project; pass --item",
                    here.display()
                ))
            })?;
            let checkout = checkout_of(&here);
            let branch = crate::git::head_branch(&checkout);
            (project, checkout, branch)
        }
    };
    let matters = matters_at(&dispatcher.ledger, &project, branch.as_deref(), &checkout);
    let mut matters = matters.into_iter();
    match (matters.next(), matters.next()) {
        (Some(one), None) => Ok(one),
        (None, _) => Err(EphorError::Command(format!(
            "no matter's work is recorded on {}; pass --item",
            checkout.display()
        ))),
        (Some(first), Some(second)) => {
            let mut all = vec![first, second];
            all.extend(matters);
            Err(EphorError::Command(format!(
                "{} carries the work of {}; pass --item to say which",
                checkout.display(),
                all.join(" and ")
            )))
        }
    }
}

/// The registered project whose directory holds `here`, the nearest one.
fn project_holding(registry_doc: &Value, here: &Path) -> Option<String> {
    let here = canonical(here);
    crate::registry::array_field(registry_doc, "projects")
        .iter()
        .map(crate::registry::id_of)
        .filter(|id| !id.is_empty())
        .filter_map(|id| {
            let placement = Placement::load(registry_doc, id)?;
            let root = canonical(&placement.root);
            here.starts_with(&root).then(|| (root, id.to_string()))
        })
        .max_by_key(|(root, _)| root.components().count())
        .map(|(_, id)| id)
}

/// The checkout `here` stands in: the nearest directory holding a repository,
/// or `here` itself.
fn checkout_of(here: &Path) -> PathBuf {
    let here = canonical(here);
    here.ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
        .unwrap_or(here)
}

/// Every matter one checkout carries (§FS-005-dispatch.36.6,
/// §FS-004-quick-actions.10): the entries that record its branch, and the
/// matters the open plans in its work roots are witnessed as.
pub fn matters_at(
    ledger: &Ledger,
    project: &str,
    branch: Option<&str>,
    checkout: &Path,
) -> BTreeSet<String> {
    let checkout = canonical(checkout);
    let mut found = BTreeSet::new();
    for (id, entry) in ledger.entries.iter().filter(|(_, e)| e.project == project) {
        let records = branch.is_some_and(|branch| {
            entry.branch.as_deref() == Some(branch)
                || entry
                    .dispatches
                    .iter()
                    .any(|dispatch| dispatch.branch(entry) == Some(branch))
        });
        if records {
            found.insert(id.clone());
        }
    }
    let roots: BTreeSet<PathBuf> = ledger
        .entries
        .values()
        .filter(|entry| entry.project == project)
        .flat_map(|entry| work_roots(entry, &checkout))
        .collect();
    for root in roots {
        for listed in plan::plans_in(&root) {
            let unit = located(Unit::of(&listed.path).path());
            let file = exclusion::plan_file(&unit);
            let Ok(Some(read)) = Plan::read(&file) else {
                continue;
            };
            if open_tasks(&read, &file).is_ok_and(|open| !open.is_empty()) {
                found.extend(witness::witnesses(&read, &listed.plan_id, &unit, ledger).into_keys());
            }
        }
    }
    found
}

/// Whether the ledger records work about `item` that no retirement has taken
/// (§FS-004-quick-actions.10).
pub fn has_unretired_work(ledger: &Ledger, item: &str) -> bool {
    ledger.entries.get(item).is_some_and(|entry| {
        entry
            .dispatches
            .iter()
            .any(|dispatch| !retired::is_retired(dispatch))
    })
}

/// The work roots inside a checkout one entry records — its own and its
/// dispatches' — and the checkout's own runtime project, where it has one
/// (§FS-005-dispatch.36.2).
fn work_roots(entry: &Entry, checkout: &Path) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = std::iter::once(entry.root.clone())
        .chain(entry.dispatches.iter().filter_map(|d| d.root.clone()))
        .map(|root| canonical(&root))
        .filter(|root| root.starts_with(checkout) && root.is_dir())
        .collect();
    let own = checkout.join(plan::PROJECT_DIR);
    if plan::is_project(&own) {
        roots.push(canonical(&own));
    }
    roots.sort();
    roots.dedup();
    roots
}

/// The whole of one retirement, filling `report` as it goes.
fn run(
    config: &StatusConfig,
    dispatcher: &mut Dispatcher,
    args: &WorkRetireArgs,
    entry: &Entry,
    report: &mut WorkRetire,
) -> std::result::Result<(), Stop> {
    let item = report.item.clone();
    // §FS-005-dispatch.36.3: with no runner bound, whether a run is live
    // cannot be asked.
    if let Some(why) = runtime::refusal(&dispatcher.global) {
        return Err(refused("runtime", why));
    }
    // §FS-005-dispatch.36.5: a retirement left half-done comes first.
    let matter = archive::matter_dir(&entry.project, &item);
    recover(&matter, entry, args.dry_run, report)?;

    // §FS-005-dispatch.36.1: the candidates, and the forge's word on them.
    let placement = dispatcher.placement(&entry.project).cloned();
    let worktrees = placement
        .as_ref()
        .map(candidates::worktrees)
        .unwrap_or_default();
    let main = placement.as_ref().and_then(|p| p.main_branch.clone());
    let recorded = recorded_branches(entry);
    let caller: Vec<String> = args.pr_branch.clone();
    let asked = candidates::table(&recorded, &[], &caller, &worktrees, main.as_deref()).names();
    let closure = ask_forge(config, &entry.project, &item, &asked)?;
    let closing: Vec<String> = closure
        .closing
        .pulls
        .iter()
        .chain(closure.head.iter())
        .map(|pull| pull.branch.clone())
        .collect();
    let table = candidates::table(&recorded, &closing, &caller, &worktrees, main.as_deref());
    report.eligibility = eligibility(&closure, &table);
    over(&item, &closure)?;
    usable(&item, &table)?;
    nothing_open(&item, &closure, &table)?;

    // §FS-005-dispatch.36.2: what is taken.
    let checkout = table.worktree.as_ref().map(|tree| tree.path.clone());
    let admitted = admitted(&args.admit);
    let reader = Reader {
        entry,
        checkout: checkout.as_deref(),
        ledger: &dispatcher.ledger,
        item: &item,
        admitted: &admitted,
    };
    let mut reading = reader.read()?;
    let archive_dir = archive::stamped(&matter, Utc::now());
    let walked = walked(dispatcher, &worktrees);
    let mut layout = layout(&reading, &archive_dir, &entry.project)?;
    checked(&reading, &layout, &archive_dir, &walked)?;

    // §FS-005-dispatch.36.3: the runtime's own exclusion, then everything
    // again under it.
    let mut held: Vec<Held> = Vec::new();
    if !reading.units().is_empty() {
        let mut roots = exclusion::roots(&reading.units());
        if let Some(found) = exclusion::marker(&roots) {
            return Err(live(found));
        }
        let mut locked: BTreeSet<PathBuf> = BTreeSet::new();
        lock(
            &reading.units(),
            &roots,
            &mut locked,
            &mut held,
            args.dry_run,
        )?;
        if let Some(found) = exclusion::listed_live(&dispatcher.global, &roots) {
            return Err(live(found));
        }
        let mut settled = false;
        for _ in 0..2 {
            reading = reader.read()?;
            let now = exclusion::roots(&reading.units());
            if now == roots {
                settled = true;
                break;
            }
            roots = now;
            lock(
                &reading.units(),
                &roots,
                &mut locked,
                &mut held,
                args.dry_run,
            )?;
        }
        if !settled {
            return Err(refused_naming(
                "live-root",
                "the set of roots to lock changed twice while it was being locked; retire again",
                roots,
            ));
        }
        if let Some(found) = exclusion::marker(&roots) {
            return Err(live(found));
        }
        layout = self::layout(&reading, &archive_dir, &entry.project)?;
        checked(&reading, &layout, &archive_dir, &walked)?;
    }

    report.plans = reading.report(&layout, args.dry_run);
    report.left = layout.left.clone();
    let taken = reading.taken().count();
    let absent = reading.absent().count();
    if taken == 0 && absent == 0 {
        report.says = format!("nothing of {item} is left to retire");
        return Ok(());
    }
    report.archive = Some(archive_dir.clone());
    if args.dry_run {
        report.says = format!(
            "would move {} to {}",
            counted(taken, "plan"),
            archive_dir.display()
        );
        return Ok(());
    }
    let record_path = commit(entry, args, report, &reading, &layout, &archive_dir)?;
    drop(held);
    report.record = Some(record_path);
    report.retired = true;
    report.says = match absent {
        0 => format!(
            "moved {} to {}",
            counted(taken, "plan"),
            archive_dir.display()
        ),
        _ => format!(
            "moved {} to {}, and recorded {} already gone",
            counted(taken, "plan"),
            archive_dir.display(),
            counted(absent, "placement")
        ),
    };
    Ok(())
}

fn counted(n: usize, what: &str) -> String {
    match n {
        1 => format!("1 {what}"),
        _ => format!("{n} {what}s"),
    }
}

/// Take the locks of `units` not already held (§FS-005-dispatch.36.3). A
/// lock is one open handle: taking it twice from here would refuse against
/// the first.
fn lock(
    units: &[Unit],
    roots: &[PathBuf],
    locked: &mut BTreeSet<PathBuf>,
    held: &mut Vec<Held>,
    probe: bool,
) -> std::result::Result<(), Stop> {
    let fresh: Vec<PathBuf> = exclusion::lock_set(units)
        .into_iter()
        .filter(|lock| !locked.contains(lock))
        .collect();
    held.push(exclusion::take(&fresh, roots, probe).map_err(live)?);
    locked.extend(fresh);
    Ok(())
}

/// The paths `--admit` names, as every other path here is compared.
fn admitted(paths: &[PathBuf]) -> Vec<PathBuf> {
    let here = std::env::current_dir().unwrap_or_default();
    paths
        .iter()
        .map(|path| match path.is_absolute() {
            true => located(path),
            false => located(&here.join(path)),
        })
        .collect()
}

/// A path with its parent made canonical and its own name kept, so a plan
/// that is itself a symlink is still seen as one (§FS-005-dispatch.36.4).
fn located(path: &Path) -> PathBuf {
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
            canonical(parent).join(name)
        }
        _ => canonical(path),
    }
}

// --- recovery -------------------------------------------------------------

/// Finish or undo every retirement of this matter still `pending`
/// (§FS-005-dispatch.36.5). A dry run only says one is there.
fn recover(
    matter: &Path,
    entry: &Entry,
    dry_run: bool,
    report: &mut WorkRetire,
) -> std::result::Result<(), Stop> {
    for (path, record) in archive::pending(matter) {
        if dry_run {
            report.recovered.push(format!(
                "{} is pending; a retirement that is not a dry run finishes or undoes it first",
                path.display()
            ));
            continue;
        }
        let roots = archive::roots_of(&record);
        let _held = exclusion::take(&exclusion::run_locks(&roots), &roots, false).map_err(live)?;
        let wanted = canonical(&path);
        let saved = entry
            .retirements
            .iter()
            .any(|retirement| canonical(&retirement.record) == wanted);
        if saved {
            archive::mark(&path, archive::COMMITTED).map_err(|err| {
                Stop::Failed(format!(
                    "{} could not be marked committed: {err}",
                    path.display()
                ))
            })?;
            report.recovered.push(format!(
                "{}: the ledger already had it, so it is marked committed",
                path.display()
            ));
            continue;
        }
        let not_restored = archive::restore(&archive::moves_of(&record));
        if !not_restored.is_empty() {
            let named: Vec<String> = not_restored
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            return Err(refused_naming(
                "recovery",
                format!(
                    "the pending retirement {} could not be fully undone: {} is occupied, so its \
                     copy stays in the archive; nothing was replaced",
                    path.display(),
                    named.join(", ")
                ),
                not_restored,
            ));
        }
        archive::mark(&path, archive::ROLLED_BACK).map_err(|err| {
            Stop::Failed(format!(
                "{} could not be marked rolled-back: {err}",
                path.display()
            ))
        })?;
        report.recovered.push(format!(
            "{}: every move was put back, so it is marked rolled-back",
            path.display()
        ));
    }
    Ok(())
}

// --- the forge --------------------------------------------------------------

/// Every branch the record names (§FS-005-dispatch.36.1): the ledger's, with
/// the checkout each was recorded with and what that is now; the plans' own;
/// and the lay records'.
fn recorded_branches(entry: &Entry) -> Vec<Recorded> {
    let at = |checkout: &Path| (canonical(checkout), candidates::shape(checkout));
    let mut out = Vec::new();
    if let Some(name) = entry.branch.as_deref() {
        out.push(Recorded {
            name: name.to_string(),
            came: Came::Ledger,
            checkout: Some(at(&entry.checkout())),
        });
    }
    for dispatch in &entry.dispatches {
        if let Some(name) = dispatch.branch(entry) {
            let checkout = dispatch
                .checkout
                .clone()
                .unwrap_or_else(|| entry.checkout());
            out.push(Recorded {
                name: name.to_string(),
                came: Came::Ledger,
                checkout: Some(at(&checkout)),
            });
        }
    }
    let work = recorded_plans(entry);
    for found in &work.plans {
        if let Ok(Some(read)) = Plan::read(&found.path) {
            for ticket in read.tickets() {
                if let Some(name) = read.branch_of(&found.plan_id, &ticket.id) {
                    out.push(Recorded {
                        name,
                        came: Came::Plan,
                        checkout: None,
                    });
                }
            }
        }
    }
    let units = work
        .plans
        .iter()
        .map(|found| Unit::of(&found.path).path().to_path_buf())
        .chain(work.unread.iter().cloned());
    for unit in units {
        for record in witness::lay_records(&unit) {
            if let Some(name) = witness::said(&record, "branch") {
                out.push(Recorded {
                    name,
                    came: Came::LayRecord,
                    checkout: None,
                });
            }
        }
    }
    out
}

/// The forge's answer, asked now (§FS-005-dispatch.36.1,
/// §FS-001-forge-interface.1), each failure in its own sentence.
fn ask_forge(
    config: &StatusConfig,
    project: &str,
    item: &str,
    heads: &[String],
) -> std::result::Result<Closure, Stop> {
    let source = item.split_once(':').map_or(item, |(source, _)| source);
    let sources = crate::feed::providers::Sources {
        project: project.to_string(),
        own: config
            .projects
            .get(project)
            .map(|project| project.providers.clone())
            .unwrap_or_default(),
        site: config.sources.clone(),
    };
    let block = sources.find(source).map(|(block, _)| block.clone());
    crate::feed::providers::closure(item, block.as_ref(), heads, FORGE_TIMEOUT).map_err(|err| {
        match err {
            ClosureError::Undeclared => refused(
                "forge",
                format!("the forge for {source} cannot say whether {item} is over"),
            ),
            ClosureError::Neither => refused(
                "forge",
                format!("retire needs an issue or a pull request, and {item} is neither"),
            ),
            ClosureError::Failed(words) => refused(
                "forge",
                format!("the forge did not say whether {item} is closed: {words}"),
            ),
        }
    })
}

/// The eligibility as it was read (§FS-005-dispatch.36.5).
fn eligibility(closure: &Closure, table: &Table) -> RetireEligibility {
    let merged_of = |pull: &Pull, via: &'static str| RetireMerged {
        pr: pull.number,
        head: pull.branch.clone(),
        merged_at: pull.merged_at.clone(),
        via,
    };
    let mut merged: Vec<RetireMerged> = closure
        .closing
        .pulls
        .iter()
        .filter(|pull| pull.state == State::Merged)
        .map(|pull| merged_of(pull, "closes the issue"))
        .collect();
    if let Some(pull) = closure
        .head
        .as_ref()
        .filter(|pull| pull.state == State::Merged)
    {
        merged.push(merged_of(pull, "is the matter"));
    }
    RetireEligibility {
        issue: Some(RetireState {
            state: closure.state.word(),
            closed_at: closure.closed_at.clone(),
        }),
        merged,
        branches: table
            .branches
            .iter()
            .map(|(name, came)| RetireBranch {
                name: name.clone(),
                from: came.iter().map(|came| came.word()).collect(),
            })
            .collect(),
        discarded: table
            .discarded
            .iter()
            .map(|(name, why)| RetireDiscarded {
                name: name.clone(),
                why: why.clone(),
            })
            .collect(),
        open: still_open(closure, table)
            .into_iter()
            .map(|pull| RetireOpen {
                pr: pull.number,
                owner: pull.owner.clone(),
                branch: pull.branch.clone(),
            })
            .collect(),
    }
}

/// Every pull request still open from a candidate, or linked to the issue,
/// each once (§FS-005-dispatch.36.1).
fn still_open<'a>(closure: &'a Closure, table: &Table) -> Vec<&'a Pull> {
    let names = table.names();
    let mut open: Vec<&Pull> = Vec::new();
    let listed = closure
        .open
        .iter()
        .filter(|(branch, _)| names.contains(branch))
        .flat_map(|(_, listing)| listing.pulls.iter());
    let linked = closure.closing.pulls.iter();
    for pull in listed
        .chain(linked)
        .filter(|pull| pull.state == State::Open)
    {
        if !open.iter().any(|have| have.number == pull.number) {
            open.push(pull);
        }
    }
    open
}

/// The positive condition (§FS-005-dispatch.36.1): the issue is closed or a
/// pull request that closes it merged; a pull request merged or closed.
fn over(item: &str, closure: &Closure) -> std::result::Result<(), Stop> {
    let (over, says) = match closure.subject {
        Subject::Issue => (
            closure.state != State::Open
                || closure
                    .closing
                    .pulls
                    .iter()
                    .any(|pull| pull.state == State::Merged),
            format!(
                "{item} is not over: the issue is open, and no pull request that closes it has \
                 merged"
            ),
        ),
        Subject::PullRequest => (
            closure.state != State::Open,
            format!("{item} is not over: the pull request is still open"),
        ),
    };
    match over {
        true => Ok(()),
        false => Err(refused("eligibility", says)),
    }
}

/// A usable candidate, and a worktree whose branch something records
/// (§FS-005-dispatch.36.1).
fn usable(item: &str, table: &Table) -> std::result::Result<(), Stop> {
    if let Some(tree) = &table.conflict {
        return Err(refused_naming(
            "branch",
            format!(
                "the checkout {} is now on {}, which nothing records for this matter",
                tree.path.display(),
                tree.branch.as_deref().unwrap_or("no branch")
            ),
            vec![tree.path.clone()],
        ));
    }
    if !table.branches.is_empty() {
        return Ok(());
    }
    let says = match table.discarded.as_slice() {
        [] => format!(
            "no branch of {item} can be checked: the record names none, and the forge links no \
             pull request; name one with --pr-branch"
        ),
        discarded => {
            let named: Vec<String> = discarded
                .iter()
                .map(|(name, _)| format!("'{name}'"))
                .collect();
            let why: BTreeSet<&str> = discarded
                .iter()
                .map(|(_, why)| candidates::kind_of(why))
                .collect();
            format!(
                "no branch of {item} can be checked: the record names only {}, {}, and the \
                 forge links no pull request; name one with --pr-branch",
                named.join(", "),
                why.into_iter().collect::<Vec<_>>().join(" or ")
            )
        }
    };
    Err(refused("branch", says))
}

/// The negative condition (§FS-005-dispatch.36.1): every list whole, and
/// nothing open.
fn nothing_open(item: &str, closure: &Closure, table: &Table) -> std::result::Result<(), Stop> {
    if !closure.closing.complete {
        return Err(refused(
            "forge",
            format!(
                "the forge's list of pull requests that close {item} was cut short; nothing was \
                 moved"
            ),
        ));
    }
    for name in table.names() {
        match closure.open.get(&name) {
            Some(listing) if !listing.complete => {
                return Err(refused(
                    "forge",
                    format!(
                        "the forge's list of open pull requests from {name} was cut short; \
                         nothing was moved"
                    ),
                ))
            }
            Some(_) => {}
            None => {
                return Err(refused(
                    "forge",
                    format!("the forge did not say what is open from {name}; nothing was moved"),
                ))
            }
        }
    }
    let open = still_open(closure, table);
    if open.is_empty() {
        return Ok(());
    }
    let said: Vec<String> = open
        .iter()
        .map(|pull| {
            let from = match &pull.owner {
                Some(owner) => format!("{owner}:{}", pull.branch),
                None => pull.branch.clone(),
            };
            format!("pull request #{} from {from} is still open", pull.number)
        })
        .collect();
    Err(refused("open-pull-request", said.join("; ")))
}

// --- the plans ----------------------------------------------------------------

/// What became of one plan (§FS-005-dispatch.36.2).
enum Fate {
    Take {
        open: Vec<RetireTask>,
        witnesses: Witnesses,
        admitted: bool,
    },
    Final,
    Absent,
}

/// One plan found, by the path that would move.
struct Found {
    unit: PathBuf,
    plan_id: String,
    fate: Fate,
}

/// Every plan found, in the order found: the record's, then the live
/// checkout's.
struct Reading {
    plans: Vec<Found>,
}

impl Reading {
    fn taken(&self) -> impl Iterator<Item = &Found> {
        self.plans
            .iter()
            .filter(|found| matches!(found.fate, Fate::Take { .. }))
    }

    fn absent(&self) -> impl Iterator<Item = &Found> {
        self.plans
            .iter()
            .filter(|found| matches!(found.fate, Fate::Absent))
    }

    /// The units the move takes, as the runtime sees them.
    fn units(&self) -> Vec<Unit> {
        self.taken()
            .map(|found| match found.unit.is_dir() {
                true => Unit::Workspace(found.unit.clone()),
                false => Unit::File(found.unit.clone()),
            })
            .collect()
    }

    /// The `plans` of the published shape (§FS-005-dispatch.36.6).
    fn report(&self, layout: &Layout, dry_run: bool) -> Vec<RetirePlan> {
        self.plans
            .iter()
            .map(|found| {
                let (outcome, open, admitted) = match &found.fate {
                    Fate::Take { open, admitted, .. } => (
                        if dry_run { "would-move" } else { "moved" },
                        open.clone(),
                        *admitted,
                    ),
                    Fate::Final => ("left-final", Vec::new(), false),
                    Fate::Absent => ("absent", Vec::new(), false),
                };
                RetirePlan {
                    plan: found.unit.clone(),
                    outcome,
                    to: layout.to.get(&found.unit).cloned(),
                    open,
                    admitted_by_caller: admitted,
                }
            })
            .collect()
    }
}

/// What a reading needs, kept so it can be made again under the locks
/// (§FS-005-dispatch.36.2).
struct Reader<'a> {
    entry: &'a Entry,
    checkout: Option<&'a Path>,
    ledger: &'a Ledger,
    item: &'a str,
    admitted: &'a [PathBuf],
}

impl Reader<'_> {
    /// Every plan the record names, then every plan in the live checkout's
    /// work roots, each judged (§FS-005-dispatch.36.2).
    fn read(&self) -> std::result::Result<Reading, Stop> {
        let work = recorded_plans(self.entry);
        let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
        let mut plans = Vec::new();
        for found in &work.plans {
            let unit = located(Unit::of(&found.path).path());
            if seen.insert(unit.clone()) {
                plans.push(self.judge(unit, &found.plan_id)?);
            }
        }
        for gone in &work.unread {
            let unit = located(gone);
            if !seen.insert(unit.clone()) {
                continue;
            }
            if fs::symlink_metadata(&unit).is_err() {
                plans.push(Found {
                    unit,
                    plan_id: String::new(),
                    fate: Fate::Absent,
                });
                continue;
            }
            // There, and the record's reading could not open it.
            let file = exclusion::plan_file(&unit);
            let why = match Plan::read(&file) {
                Err(err) => err.to_string(),
                _ => "it does not read as a plan".to_string(),
            };
            return Err(unreadable(&file, &why));
        }
        if let Some(checkout) = self.checkout {
            for root in work_roots(self.entry, checkout) {
                for listed in plan::plans_in(&root) {
                    let unit = located(Unit::of(&listed.path).path());
                    if !seen.insert(unit.clone()) {
                        continue;
                    }
                    // A final plan found only in the checkout is not this
                    // matter's to list.
                    let found = self.judge(unit, &listed.plan_id)?;
                    if !matches!(found.fate, Fate::Final) {
                        plans.push(found);
                    }
                }
            }
        }
        Ok(Reading { plans })
    }

    /// One plan, by the rule (§FS-005-dispatch.36.2).
    fn judge(&self, unit: PathBuf, plan_id: &str) -> std::result::Result<Found, Stop> {
        if fs::symlink_metadata(&unit).is_err() {
            return Ok(Found {
                unit,
                plan_id: plan_id.to_string(),
                fate: Fate::Absent,
            });
        }
        let file = exclusion::plan_file(&unit);
        let read = match Plan::read(&file) {
            Ok(Some(read)) => read,
            Ok(None) => return Err(unreadable(&file, "it is not a plan file")),
            Err(err) => return Err(unreadable(&file, &err.to_string())),
        };
        let open = open_tasks(&read, &file).map_err(|why| unreadable(&file, &why))?;
        if open.is_empty() {
            return Ok(Found {
                unit,
                plan_id: plan_id.to_string(),
                fate: Fate::Final,
            });
        }
        let witnesses = witness::witnesses(&read, plan_id, &unit, self.ledger);
        let is_admitted = self
            .admitted
            .iter()
            .any(|path| *path == unit || *path == file);
        match witness::judge(self.item, &witnesses, is_admitted) {
            Verdict::Take { admitted } => Ok(Found {
                unit,
                plan_id: plan_id.to_string(),
                fate: Fate::Take {
                    open,
                    witnesses,
                    admitted,
                },
            }),
            Verdict::Unwitnessed => Err(refused_naming(
                "witness",
                format!(
                    "{} is open and nothing says whose it is; if it is {}'s, name it with \
                     --admit {}",
                    unit.display(),
                    self.item,
                    unit.display()
                ),
                vec![unit],
            )),
            Verdict::Foreign(others) => Err(refused_naming(
                "witness",
                format!(
                    "{} is {}'s, not {}'s; nothing was moved",
                    unit.display(),
                    others.join(" and "),
                    self.item
                ),
                vec![unit],
            )),
            Verdict::Mixed(others) => Err(refused_naming(
                "witness",
                format!(
                    "{} is witnessed as {}'s and as {}'s; nothing was moved",
                    unit.display(),
                    self.item,
                    others.join(" and ")
                ),
                vec![unit],
            )),
        }
    }
}

fn unreadable(file: &Path, why: &str) -> Stop {
    refused_naming(
        "unreadable",
        format!(
            "{} cannot be read, so it is not known to be finished: {why}",
            file.display()
        ),
        vec![file.to_path_buf()],
    )
}

/// The tasks of a plan that are not final, judged by the machine the runtime
/// itself would judge them by (§FS-005-dispatch.28).
fn open_tasks(read: &Plan, file: &Path) -> std::result::Result<Vec<RetireTask>, String> {
    let machine = match plan::own_machine(file).map_err(|err| err.to_string())? {
        Some(own) => own,
        None => {
            let root = file.parent().unwrap_or(Path::new("."));
            WorkRoot::in_force(root).map_err(|err| err.to_string())?
        }
    };
    Ok(read
        .tickets()
        .into_iter()
        .filter(|ticket| {
            !ticket
                .state
                .as_deref()
                .is_some_and(|state| machine.is_final(state))
        })
        .map(|ticket| RetireTask {
            task: ticket.id,
            state: ticket.state.unwrap_or_default(),
        })
        .collect())
}

// --- the archive --------------------------------------------------------------

/// Where everything goes (§FS-005-dispatch.36.4).
#[derive(Default)]
struct Layout {
    moves: Vec<Move>,
    /// The machine copies, one per root something came from.
    machines: Vec<Move>,
    /// Each taken unit's place in the archive.
    to: BTreeMap<PathBuf, PathBuf>,
    /// The roots something came from, in the order they were read.
    roots: Vec<PathBuf>,
    /// What a moved single plan leaves in its shared root
    /// (§FS-005-dispatch.36.5).
    left: Vec<PathBuf>,
}

/// Lay every move out (§FS-005-dispatch.36.4): one directory per root, a
/// single plan whole, a workspace's content without its coordination state,
/// and each plan's lay record beside it.
fn layout(
    reading: &Reading,
    archive_dir: &Path,
    project: &str,
) -> std::result::Result<Layout, Stop> {
    let mut layout = Layout::default();
    for found in reading.taken() {
        let root = found
            .unit
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let number = match layout.roots.iter().position(|have| *have == root) {
            Some(at) => at + 1,
            None => {
                layout.roots.push(root.clone());
                layout.roots.len()
            }
        };
        let root_name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dir = archive_dir.join(format!("{number}-{project}-{root_name}"));
        let Some(name) = found.unit.file_name() else {
            continue;
        };
        let to = dir.join(name);
        if found.unit.is_dir() {
            let leaving = exclusion::leaving(&found.unit).map_err(|err| {
                unreadable(&found.unit, &format!("its content cannot be listed: {err}"))
            })?;
            for path in leaving {
                let Ok(within) = path.strip_prefix(&found.unit) else {
                    continue;
                };
                layout.moves.push(Move {
                    to: to.join(within),
                    from: path,
                });
            }
        } else {
            layout.moves.push(Move {
                from: found.unit.clone(),
                to: to.clone(),
            });
            layout
                .left
                .extend(exclusion::left_behind(&found.unit, &found.plan_id));
        }
        if let Some(record) = witness::lay_record_dir(&found.unit) {
            if let Some(record_name) = record.file_name() {
                layout.moves.push(Move {
                    to: dir.join(plan::CARRIED).join(record_name),
                    from: record,
                });
            }
        }
        layout.to.insert(found.unit.clone(), to);
        let machine = root.join(plan::STATES);
        if machine.is_file() && !layout.machines.iter().any(|copy| copy.from == machine) {
            layout.machines.push(Move {
                from: machine,
                to: dir.join(plan::STATES),
            });
        }
    }
    Ok(layout)
}

/// Every place a reader walks (§FS-005-dispatch.36.4): every registered
/// project's directory and its task stores, every root the ledger records,
/// and every worktree git lists for the project.
fn walked(dispatcher: &Dispatcher, worktrees: &[candidates::Worktree]) -> Vec<PathBuf> {
    let mut walked: Vec<PathBuf> = Vec::new();
    for id in crate::registry::array_field(&dispatcher.registry_doc, "projects")
        .iter()
        .map(crate::registry::id_of)
        .filter(|id| !id.is_empty())
    {
        if let Some(placement) = Placement::load(&dispatcher.registry_doc, id) {
            walked.push(canonical(&placement.root));
            walked.extend(placement.task_stores().iter().map(|store| canonical(store)));
        }
    }
    for entry in dispatcher.ledger.entries.values() {
        walked.push(canonical(&entry.root));
        walked.extend(
            entry
                .dispatches
                .iter()
                .filter_map(|d| d.root.as_deref())
                .map(canonical),
        );
    }
    walked.extend(worktrees.iter().map(|tree| tree.path.clone()));
    walked.sort();
    walked.dedup();
    walked
}

/// The archive's placement and what git tracks (§FS-005-dispatch.36.4).
fn checked(
    reading: &Reading,
    layout: &Layout,
    archive_dir: &Path,
    walked: &[PathBuf],
) -> std::result::Result<(), Stop> {
    if reading.taken().next().is_none() {
        return Ok(());
    }
    let archive = archive::resolved(archive_dir);
    let units: Vec<PathBuf> = reading.taken().map(|found| found.unit.clone()).collect();
    let mut places = walked.to_vec();
    places.extend(units.iter().cloned());
    if let Some(place) = archive::inside(&archive, &places) {
        return Err(refused_naming(
            "archive",
            format!(
                "the archive {} would be inside {}, which is read for work; put ephor's data \
                 directory somewhere else",
                archive.display(),
                place.display()
            ),
            vec![archive.clone(), place.clone()],
        ));
    }
    for unit in &units {
        if fs::symlink_metadata(unit).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(refused_naming(
                "archive",
                format!(
                    "{} is a symlink; a plan that is a link is not moved",
                    unit.display()
                ),
                vec![unit.clone()],
            ));
        }
        let (here, there) = (archive::device(unit), archive::device(&archive));
        if here.is_some() && there.is_some() && here != there {
            return Err(refused_naming(
                "archive",
                format!(
                    "{} and the archive {} are on different filesystems, and a move is a rename",
                    unit.display(),
                    archive.display()
                ),
                vec![unit.clone(), archive.clone()],
            ));
        }
    }
    tracked(layout)
}

/// Nothing git tracks moves (§FS-005-dispatch.36.4): asked in the directory
/// each moving path sits in.
fn tracked(layout: &Layout) -> std::result::Result<(), Stop> {
    let mut by_dir: BTreeMap<PathBuf, Vec<PathBuf>> = BTreeMap::new();
    for step in &layout.moves {
        if let Some(dir) = step.from.parent() {
            by_dir
                .entry(dir.to_path_buf())
                .or_default()
                .push(step.from.clone());
        }
    }
    for (dir, paths) in by_dir {
        if !dir.is_dir() {
            continue;
        }
        let mut command = crate::git::command();
        crate::git::without_repository_variables(&mut command);
        let output = command
            .arg("-C")
            .arg(&dir)
            .args(["ls-files", "-z", "--"])
            .args(&paths)
            .output();
        let output = match output {
            Ok(output) => output,
            Err(err) => {
                return Err(refused_naming(
                    "tracked",
                    format!(
                        "whether git tracks what would move from {} cannot be said: {err}",
                        dir.display()
                    ),
                    paths,
                ))
            }
        };
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("not a git repository") {
                continue;
            }
            return Err(refused_naming(
                "tracked",
                format!(
                    "whether git tracks what would move from {} cannot be said: {}",
                    dir.display(),
                    stderr.trim()
                ),
                paths,
            ));
        }
        let listed: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter(|name| !name.is_empty())
            .map(|name| dir.join(name))
            .collect();
        if !listed.is_empty() {
            let named: Vec<String> = listed.iter().map(|p| p.display().to_string()).collect();
            return Err(refused_naming(
                "tracked",
                format!(
                    "git tracks {}, and moving it would leave a change in that checkout",
                    named.join(", ")
                ),
                listed,
            ));
        }
    }
    Ok(())
}

// --- the commit -----------------------------------------------------------------

/// The record, the moves, the ledger, the record again
/// (§FS-005-dispatch.36.5). Returns the record's path.
fn commit(
    entry: &Entry,
    args: &WorkRetireArgs,
    report: &WorkRetire,
    reading: &Reading,
    layout: &Layout,
    archive_dir: &Path,
) -> std::result::Result<PathBuf, Stop> {
    let at = Utc::now();
    let record_path = archive_dir.join(archive::RECORD);
    let record = record(entry, args, report, reading, layout, at);
    archive::write(&record_path, &record).map_err(|err| {
        Stop::Failed(format!(
            "the record {} could not be written, so nothing was moved: {err}",
            record_path.display()
        ))
    })?;
    let undo = |failed: String, not_restored: Vec<PathBuf>| -> Stop {
        let _ = archive::mark(&record_path, archive::ROLLED_BACK);
        Stop::Failed(with_unrestored(failed, &not_restored))
    };
    for copy in &layout.machines {
        let copied = copy
            .to
            .parent()
            .map(fs::create_dir_all)
            .unwrap_or(Ok(()))
            .and_then(|_| fs::copy(&copy.from, &copy.to));
        if let Err(err) = copied {
            return Err(undo(
                format!(
                    "the machine {} could not be copied: {err}",
                    copy.from.display()
                ),
                Vec::new(),
            ));
        }
    }
    if let Err((failed, not_restored)) = archive::perform(&layout.moves) {
        return Err(undo(failed, not_restored));
    }
    if let Err(err) = save(&report.item, &record_path, archive_dir, at, reading, layout) {
        let not_restored = archive::restore(&layout.moves);
        return Err(undo(
            format!("the ledger could not be saved, so every move was put back: {err}"),
            not_restored,
        ));
    }
    if let Err(err) = archive::mark(&record_path, archive::COMMITTED) {
        eprintln!(
            "note: {} still says pending ({err}); the next retire of this matter marks it \
             committed",
            record_path.display()
        );
    }
    Ok(record_path)
}

fn with_unrestored(failed: String, not_restored: &[PathBuf]) -> String {
    if not_restored.is_empty() {
        return failed;
    }
    let named: Vec<String> = not_restored
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    format!(
        "{failed}; not restored, because something is there now: {}",
        named.join(", ")
    )
}

/// The ledger's half (§FS-005-dispatch.36.5): the retirement on the entry,
/// and `retired_at` on every dispatch whose placement was taken. Read fresh,
/// so nothing written since this command started is lost.
fn save(
    item: &str,
    record: &Path,
    archive_dir: &Path,
    at: DateTime<Utc>,
    reading: &Reading,
    layout: &Layout,
) -> Result<()> {
    let mut ledger = ledger::load()?;
    let Some(entry) = ledger.entries.get_mut(item) else {
        return Err(EphorError::Command(format!(
            "the ledger no longer records {item}"
        )));
    };
    let taken: BTreeSet<PathBuf> = reading
        .taken()
        .chain(reading.absent())
        .map(|found| found.unit.clone())
        .collect();
    let retiring: Vec<usize> = witness::placements(entry)
        .into_iter()
        .filter(|(_, path)| taken.contains(&located(path)) || taken.contains(&canonical(path)))
        .map(|(at, _)| at)
        .collect();
    for index in retiring {
        entry.dispatches[index].retired_at = Some(at);
    }
    let plans = reading
        .taken()
        .chain(reading.absent())
        .map(|found| RetiredPlan {
            from: found.unit.clone(),
            to: layout.to.get(&found.unit).cloned(),
            absent: matches!(found.fate, Fate::Absent),
        })
        .collect();
    entry.retirements.push(Retirement {
        at,
        record: record.to_path_buf(),
        archive: archive_dir.to_path_buf(),
        plans,
    });
    ledger::store(&ledger)
}

/// What the record says (§FS-005-dispatch.36.5).
fn record(
    entry: &Entry,
    args: &WorkRetireArgs,
    report: &WorkRetire,
    reading: &Reading,
    layout: &Layout,
    at: DateTime<Utc>,
) -> Value {
    let plans: Vec<Value> = reading
        .plans
        .iter()
        .map(|found| {
            let (outcome, open, witnesses, admitted) = match &found.fate {
                Fate::Take {
                    open,
                    witnesses,
                    admitted,
                } => ("moved", open.clone(), witnesses.clone(), *admitted),
                Fate::Final => ("left-final", Vec::new(), Witnesses::new(), false),
                Fate::Absent => ("absent", Vec::new(), Witnesses::new(), false),
            };
            json!({
                "from": found.unit,
                "plan_id": found.plan_id,
                "outcome": outcome,
                "to": layout.to.get(&found.unit),
                "witnesses": witnesses,
                "open": open,
                "admitted_by_caller": admitted,
                "admitted": admitted.then(|| json!({
                    "item": report.item,
                    "project": entry.project,
                    "path": found.unit,
                    "plan_id": found.plan_id,
                })),
            })
        })
        .collect();
    json!({
        "schema": archive::SCHEMA,
        "state": archive::PENDING,
        "matter": { "id": report.item, "project": entry.project, "title": entry.title },
        "retired": {
            "at": at,
            "surface": SURFACE,
            "host": archive::host(),
            "by": archive::user(),
            "why": args.why,
        },
        "eligibility": report.eligibility,
        "guards": guards(layout),
        "roots": layout.roots,
        "plans": plans,
        "machines": layout.machines,
        "moves": layout.moves,
        "left": layout.left,
    })
}

/// Each guard and what it found, per root (§FS-005-dispatch.36.5): every one
/// of them let the move through, or there would be no record.
fn guards(layout: &Layout) -> Vec<Value> {
    layout
        .roots
        .iter()
        .map(|root| {
            json!({
                "root": root,
                "live-root": "every lock taken, no marker, no live run listed",
                "tracked": "nothing that moves is tracked",
                "archive": "outside every root, on the same filesystem",
            })
        })
        .collect()
}

// --- the answer ---------------------------------------------------------------

fn print(report: &WorkRetire, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(report).unwrap_or_else(|_| "null".to_string())
        );
        return;
    }
    let style = Style::detect();
    for recovered in &report.recovered {
        println!("{}", style.dim(&format!("recovered {recovered}")));
    }
    println!("{} — {}", report.item, report.says);
    if let Some(refusal) = &report.refused {
        println!(
            "  {}",
            style.dim(&format!("refused by the {} guard", refusal.guard))
        );
        return;
    }
    for found in &report.plans {
        let line = match (found.outcome, &found.to) {
            ("moved", Some(to)) => format!("moved {} → {}", found.plan.display(), to.display()),
            ("would-move", Some(to)) => {
                format!("would move {} → {}", found.plan.display(), to.display())
            }
            ("left-final", _) => format!("left final {}", found.plan.display()),
            ("absent", _) => format!("already gone {}", found.plan.display()),
            (outcome, _) => format!("{outcome} {}", found.plan.display()),
        };
        println!("  {line}");
        for task in &found.open {
            println!(
                "    {}",
                style.dim(&format!("{} · {}", task.task, task.state))
            );
        }
    }
    for left in &report.left {
        println!("  {}", style.dim(&format!("left {}", left.display())));
    }
    if let Some(record) = &report.record {
        println!("  {}", style.dim(&format!("record {}", record.display())));
    }
}
