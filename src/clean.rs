//! `ephor clean`: each project's own clean verb, asked of every branch
//! checkout on disk that no live run holds (§FS-017-clean).
//!
//! Nothing here knows how a project un-builds. The verb is bound and resolved
//! by the code that binds the check verbs ([`crate::seams::checks::bind`]),
//! summoned the way every summons is, and the checkouts are the ones the
//! rebase sweep walks ([`Placement::branch_checkouts`]), passed over by the
//! same live-run reading ([`crate::sweep::live_run_holds`]). What this module
//! adds is the before and after: measuring each checkout around its verb, and
//! saying what came back.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value};

use crate::branches::{BranchInfo, Placement};
use crate::cli::CleanArgs;
use crate::error::Result;
use crate::manifest::Manifest;
use crate::scope::{Act, Gate, Projects, Scope};
use crate::seams::checks::{self, Bound, Verb};
use crate::seams::summons::{Mode, Outcome as Ended, Site};

/// The verb, as a refusal and a held gate name it.
const VERB: &str = "clean";

/// What the report calls the scope where no selector narrowed it
/// (§FS-017-clean.2).
const EVERY: &str = "every project in the registry";

/// A checkout's size on disk before its verb ran and after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Measured {
    before: u64,
    after: u64,
}

impl Measured {
    /// What the verb gave back. A verb that grew the tree gave back nothing,
    /// rather than a negative number nobody can act on.
    fn reclaimed(&self) -> u64 {
        self.before.saturating_sub(self.after)
    }
}

/// What became of one checkout (§FS-017-clean.3).
enum Outcome {
    /// The verb exited `0`.
    Cleaned(Measured),
    /// The verb exited `75`: not now, ask again later.
    Parked(Measured),
    /// The verb exited anything else, or could not be summoned at all. The
    /// measurement is there where the verb ran: a verb that failed half way
    /// may still have given something back.
    Failed(String, Option<Measured>),
    /// A live run holds the checkout, or no clean verb is declared there.
    PassedOver(String),
    /// The gate held, so nothing was summoned and nothing measured: a verb's
    /// yield is not known before it runs (§FS-017-clean.3).
    Would,
}

impl Outcome {
    fn name(&self) -> &'static str {
        match self {
            Outcome::Cleaned(_) => "cleaned",
            Outcome::Parked(_) => "parked",
            Outcome::Failed(..) => "failed",
            Outcome::PassedOver(_) => "passed-over",
            Outcome::Would => "would-clean",
        }
    }

    fn measured(&self) -> Option<Measured> {
        match self {
            Outcome::Cleaned(measured) | Outcome::Parked(measured) => Some(*measured),
            Outcome::Failed(_, measured) => *measured,
            Outcome::PassedOver(_) | Outcome::Would => None,
        }
    }
}

/// One row of the sweep.
struct Row {
    project: String,
    branch: String,
    checkout: PathBuf,
    /// The bound command, and where it runs, wherever one is bound.
    bound: Option<(String, PathBuf)>,
    outcome: Outcome,
}

/// Whether a project's checkouts were reached, and why not where they were
/// not.
struct Reached {
    project: String,
    refusal: Option<String>,
}

/// The sweep (§FS-017-clean.2). The scope is selected here, out of the
/// registry this loads once: every registry project where no selector was
/// given.
pub fn clean(args: &CleanArgs, scope: &Scope, act: Act) -> Result<ExitCode> {
    let config = crate::feed::config::load_config()?;
    let registry = crate::feed::commands::load_registry_doc()?;
    let projects = if scope.selects_projects() {
        scope.against(&registry)?
    } else {
        Projects::every()
    };
    let in_scope: Vec<String> = crate::registry::select_projects(&registry, &[], &[], None)?
        .into_iter()
        .map(|project| crate::registry::id_of(project).to_string())
        .filter(|project| projects.holds(project))
        .collect();
    let said = if projects.narrowed() {
        projects.said().to_string()
    } else {
        EVERY.to_string()
    };
    // Cleaning writes into trees, so it acts only under `--act`, at every
    // width (§FS-011-command-line.10).
    let gate = act.over_a_sweep(VERB);

    // Which trees a live run holds, read over the whole site the way the
    // rebase sweep and `work run --due` read it (§FS-005-dispatch.24). A guard
    // that cannot be read stops the sweep rather than passing.
    let busy = crate::work::Dispatcher::load(&config)?.live_checkouts();

    let mut rows: Vec<Row> = Vec::new();
    let mut reached: Vec<Reached> = Vec::new();
    for project in &in_scope {
        let Some(placement) = Placement::load(&registry, project) else {
            reached.push(Reached {
                project: project.clone(),
                refusal: Some("no root in the registry, so it has no checkouts".to_string()),
            });
            continue;
        };
        // The main checkout is `ephor update`'s; a project that names no main
        // branch cannot have it told from a branch's (§FS-017-clean.2).
        if placement.main_branch.is_none() {
            reached.push(Reached {
                project: project.clone(),
                refusal: Some(
                    "it names no main branch, so its main checkout cannot be told from a \
                     branch's"
                        .to_string(),
                ),
            });
            continue;
        }
        reached.push(Reached {
            project: project.clone(),
            refusal: None,
        });
        let site = config
            .projects
            .get(project)
            .and_then(|configured| configured.clean.as_deref());
        for (branch, checkout) in placement.branch_checkouts() {
            rows.push(one(
                &placement, project, branch, checkout, site, &busy, &gate,
            ));
        }
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&view(&said, &rows, &reached, &gate))
                .unwrap_or_else(|_| "null".to_string())
        );
    } else {
        print!("{}", say(&said, &rows, &reached, &gate));
    }
    Ok(exit_code(&rows, &reached))
}

/// One checkout: passed over, listed, or cleaned and measured
/// (§FS-017-clean.2, §FS-017-clean.3).
fn one(
    placement: &Placement,
    project: &str,
    branch: &BranchInfo,
    checkout: PathBuf,
    site: Option<&str>,
    busy: &BTreeMap<PathBuf, PathBuf>,
    gate: &Gate,
) -> Row {
    let mut row = Row {
        project: project.to_string(),
        branch: branch.branch.clone(),
        checkout: checkout.clone(),
        bound: None,
        outcome: Outcome::Would,
    };
    // Removing a build under an agent mid-build loses work (§FS-017-clean.2).
    if let Some(held) = crate::sweep::live_run_holds(busy, &checkout) {
        row.outcome = Outcome::PassedOver(held);
        return row;
    }
    // Each checkout carries its own manifest and script, so the binding is
    // resolved here rather than once per project (§FS-017-clean.1).
    let manifest = match Manifest::read(&checkout, placement.trust) {
        Ok(manifest) => manifest,
        Err(err) => {
            row.outcome = Outcome::Failed(format!("its manifest could not be read: {err}"), None);
            return row;
        }
    };
    let Some(bound) = checks::bind(Verb::Clean, &checkout, manifest.as_ref(), site) else {
        row.outcome = Outcome::PassedOver(crate::capabilities::no_clean_verb(&checkout));
        return row;
    };
    let runs_in = match runs_in(&bound, &checkout) {
        Ok(place) => place,
        Err(err) => {
            row.outcome = Outcome::Failed(err.to_string(), None);
            return row;
        }
    };
    row.bound = Some((bound.command.clone(), runs_in));
    if gate.holds() {
        return row;
    }

    // Told what a summons about a branch is told (§FS-017-clean.1).
    let dossier = crate::seams::dossier::of_branch(
        project,
        &placement.root,
        &checkout,
        placement.organization.as_ref(),
        Some(branch),
        Some(&placement.forest(&checkout)),
    );
    let before = occupied(&checkout);
    // Aside: the verb's own output goes to the error stream, so the report on
    // standard output is never interleaved with a build log.
    let answer = checks::run(&bound, &checkout, dossier, None, Mode::Aside);
    let measured = Measured {
        before,
        after: occupied(&checkout),
    };
    row.outcome = match answer {
        Ok(answer) => match answer.outcome {
            Ended::Done => Outcome::Cleaned(measured),
            Ended::Parked => Outcome::Parked(measured),
            Ended::Failed => Outcome::Failed(answer.refusal(VERB), Some(measured)),
        },
        Err(err) => Outcome::Failed(err.to_string(), Some(measured)),
    };
    row
}

/// Where a bound verb runs in this checkout: the checkout, or the repository
/// of it the binding names.
fn runs_in(bound: &Bound, checkout: &Path) -> Result<PathBuf> {
    let summons = bound.summons(Vec::new())?;
    Ok(Site::root(checkout).place_of(&summons.place))
}

/// The space a tree occupies on disk (§FS-017-clean.3): a symbolic link is
/// not followed, a file linked twice is counted once, and what cannot be read
/// is left out rather than failing the sweep.
pub fn occupied(root: &Path) -> u64 {
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    let mut total = 0;
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            // A directory entry's own metadata, never its target's.
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                pending.push(entry.path());
            }
            total += on_disk(&meta, &mut seen);
        }
    }
    total
}

#[cfg(unix)]
fn on_disk(meta: &std::fs::Metadata, seen: &mut HashSet<(u64, u64)>) -> u64 {
    use std::os::unix::fs::MetadataExt;
    if !meta.is_dir() && meta.nlink() > 1 && !seen.insert((meta.dev(), meta.ino())) {
        return 0;
    }
    meta.blocks() * 512
}

#[cfg(not(unix))]
fn on_disk(meta: &std::fs::Metadata, _seen: &mut HashSet<(u64, u64)>) -> u64 {
    if meta.is_file() {
        meta.len()
    } else {
        0
    }
}

/// Bytes as a reader says them, in powers of 1024 (§FS-017-clean.3).
pub fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["KiB", "MiB", "GiB", "TiB", "PiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// Exit `1` where any verb failed or any project was not reached; cleaned,
/// parked and passed over are good ends (§FS-017-clean.3).
fn exit_code(rows: &[Row], reached: &[Reached]) -> ExitCode {
    let failed = rows
        .iter()
        .any(|row| matches!(row.outcome, Outcome::Failed(..)));
    if failed || reached.iter().any(|project| project.refusal.is_some()) {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn counted(rows: &[Row]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for row in rows {
        *counts.entry(row.outcome.name()).or_insert(0) += 1;
    }
    counts
}

/// What every verb that ran gave back. None on a run held at the gate, which
/// ran nothing and so knows no number (§FS-017-clean.3).
fn reclaimed(rows: &[Row], gate: &Gate) -> Option<u64> {
    if gate.holds() {
        return None;
    }
    Some(
        rows.iter()
            .filter_map(|row| row.outcome.measured())
            .map(|measured| measured.reclaimed())
            .sum(),
    )
}

/// One line for the sweep, counted, and the total where verbs ran.
fn summary(rows: &[Row], reached: &[Reached], gate: &Gate) -> String {
    let mut parts: Vec<String> = counted(rows)
        .iter()
        .map(|(name, count)| format!("{count} {name}"))
        .collect();
    let unreached = reached
        .iter()
        .filter(|project| project.refusal.is_some())
        .count();
    if unreached > 0 {
        parts.push(format!("{unreached} project(s) not reached"));
    }
    if parts.is_empty() {
        return "no branch checkout to clean".to_string();
    }
    if let Some(total) = reclaimed(rows, gate) {
        parts.push(format!("{} reclaimed", human(total)));
    }
    parts.join(", ")
}

/// One checkout's line: the branch, what became of it, and where.
fn says(row: &Row) -> String {
    let what = match &row.outcome {
        Outcome::Cleaned(measured) => format!(
            "cleaned, {} reclaimed ({} to {})",
            human(measured.reclaimed()),
            human(measured.before),
            human(measured.after)
        ),
        Outcome::Parked(measured) => format!(
            "parked: the verb asked to be asked later, {} reclaimed",
            human(measured.reclaimed())
        ),
        Outcome::Failed(why, Some(measured)) => {
            format!("failed: {why}, {} reclaimed", human(measured.reclaimed()))
        }
        Outcome::Failed(why, None) => format!("failed: {why}"),
        Outcome::PassedOver(why) => format!("passed over: {why}"),
        Outcome::Would => match &row.bound {
            // The checkout closes the line, so the place is named only where
            // the binding runs somewhere else in it.
            Some((command, cwd)) if cwd == &row.checkout => format!("would run `{command}`"),
            Some((command, cwd)) => format!("would run `{command}` in {}", cwd.display()),
            None => "would be cleaned".to_string(),
        },
    };
    format!("{} — {what} ({})", row.branch, row.checkout.display())
}

const SAYS_INDENT: &str = "  ";
const NESTED_INDENT: &str = "    ";

/// The sweep as prose: the summary first, then each project and its
/// checkouts, the main checkout said once per project (§FS-017-clean.2).
fn say(said: &str, rows: &[Row], reached: &[Reached], gate: &Gate) -> String {
    let mut out = format!("clean over {said} — {}.\n", summary(rows, reached, gate));
    for project in reached {
        if let Some(why) = &project.refusal {
            out.push_str(&format!(
                "{SAYS_INDENT}{}: not reached. No checkout of this project was cleaned: {why}.\n",
                project.project
            ));
            continue;
        }
        out.push_str(&format!(
            "{SAYS_INDENT}{}: the main branch's checkout is `ephor update`'s, and is not \
             cleaned.\n",
            project.project
        ));
        let mine: Vec<&Row> = rows
            .iter()
            .filter(|row| row.project == project.project)
            .collect();
        if mine.is_empty() {
            out.push_str(&format!(
                "{NESTED_INDENT}No branch checkout is on disk here.\n"
            ));
            continue;
        }
        for row in mine {
            out.push_str(&format!("{NESTED_INDENT}{}\n", says(row)));
        }
    }
    if let Some(held) = gate.says() {
        out.push_str(&format!("{held}\n"));
    }
    out
}

/// The same facts for a program (§REQ-002-parity.3).
fn view(said: &str, rows: &[Row], reached: &[Reached], gate: &Gate) -> Value {
    let counts = counted(rows);
    let count = |name: &str| counts.get(name).copied().unwrap_or(0);
    let mut view = json!({
        "scope": said,
        "summary": summary(rows, reached, gate),
        "cleaned": count("cleaned"),
        "parked": count("parked"),
        "failed": count("failed"),
        "passed_over": count("passed-over"),
        "would_clean": count("would-clean"),
        "projects": reached.iter().map(|project| {
            let mut row = json!({
                "project": project.project,
                "reached": project.refusal.is_none(),
            });
            if let (Some(row), Some(why)) = (row.as_object_mut(), &project.refusal) {
                row.insert("says".to_string(), json!(why));
            }
            row
        }).collect::<Vec<_>>(),
        "checkouts": rows.iter().map(checkout_view).collect::<Vec<_>>(),
    });
    let object = view.as_object_mut().expect("the reading is an object");
    if let Some(total) = reclaimed(rows, gate) {
        object.insert("reclaimed_bytes".to_string(), json!(total));
        object.insert("reclaimed".to_string(), json!(human(total)));
    }
    if let Some(held) = gate.says() {
        object.insert("gated".to_string(), json!(true));
        object.insert("says".to_string(), json!(held));
    }
    view
}

fn checkout_view(row: &Row) -> Value {
    let mut checkout = json!({
        "project": row.project,
        "branch": row.branch,
        "checkout": row.checkout,
        "outcome": row.outcome.name(),
        "says": says(row),
    });
    let object = checkout.as_object_mut().expect("a row is an object");
    if let Some((command, cwd)) = &row.bound {
        object.insert("command".to_string(), json!(command));
        object.insert("cwd".to_string(), json!(cwd));
    }
    if let Some(measured) = row.outcome.measured() {
        object.insert("before_bytes".to_string(), json!(measured.before));
        object.insert("after_bytes".to_string(), json!(measured.after));
        object.insert("reclaimed_bytes".to_string(), json!(measured.reclaimed()));
        object.insert("reclaimed".to_string(), json!(human(measured.reclaimed())));
    }
    checkout
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file linked twice occupies its blocks once, and a symbolic link is
    /// not followed into what it points at (§FS-017-clean.3).
    #[cfg(unix)]
    #[test]
    fn a_tree_is_measured_on_disk_once_per_file() {
        let tmp = tempfile::tempdir().unwrap();
        let tree = tmp.path().join("tree");
        std::fs::create_dir_all(tree.join("target")).unwrap();
        std::fs::write(tree.join("target/blob"), vec![7u8; 256 * 1024]).unwrap();
        let alone = occupied(&tree);
        assert!(alone >= 256 * 1024, "{alone}");

        std::fs::hard_link(tree.join("target/blob"), tree.join("again")).unwrap();
        assert_eq!(occupied(&tree), alone);

        let elsewhere = tmp.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("big"), vec![1u8; 1024 * 1024]).unwrap();
        std::os::unix::fs::symlink(&elsewhere, tree.join("link")).unwrap();
        assert!(occupied(&tree) < alone + 1024 * 1024);
    }

    #[test]
    fn bytes_are_said_in_powers_of_1024() {
        assert_eq!(human(0), "0 B");
        assert_eq!(human(1023), "1023 B");
        assert_eq!(human(1024), "1.0 KiB");
        assert_eq!(human(1536 * 1024), "1.5 MiB");
        assert_eq!(human(3 * 1024 * 1024 * 1024), "3.0 GiB");
    }

    #[test]
    fn a_verb_that_grew_the_tree_gave_back_nothing() {
        let measured = Measured {
            before: 10,
            after: 20,
        };
        assert_eq!(measured.reclaimed(), 0);
    }
}
