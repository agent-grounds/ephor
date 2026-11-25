//! Reading a monitor's `from` (§FS-021-monitors.5): a command run once per
//! read through the one summons executor every seam uses, or a file read and
//! never run. Each distinct `from` is read once however many monitors name it
//! (§FS-021-monitors.5.1), and the runs of one refresh go in parallel.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value as Json;

use crate::branches::Organization;
use crate::forest::Forest;
use crate::monitors::model::Source;
use crate::monitors::route::Outcome;
use crate::monitors::state::FileMark;
use crate::seams::summons::{self, Mode, Place, Site, Summons};

/// The variable that names the monitors a run reads, so a script that
/// answers for many may compute only those (§FS-021-monitors.5.4).
pub const MONITORS_VAR: &str = "EPHOR_MONITORS";

/// Where a project's monitors are read: the project, its root, the checkout
/// a `from` runs in, and what a dossier says of them (§FS-021-monitors.5.3).
#[derive(Debug, Clone, Copy)]
pub struct Where<'a> {
    pub project: &'a str,
    pub root: &'a Path,
    pub checkout: &'a Path,
    pub organization: Option<&'a Organization>,
    pub forest: Option<&'a Forest>,
}

/// One read to make: a `from`, the monitors it reads, and how long it may
/// take, the longest timeout among them (§FS-021-monitors.5.5).
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub source: Source,
    pub reads: BTreeSet<String>,
    pub timeout: u64,
}

/// What one read brought: the answer's samples, or why there are none, and
/// a file's mark where the `from` is a file (§FS-021-monitors.9.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Got {
    pub source: Source,
    pub outcome: Outcome,
    pub mark: Option<FileMark>,
}

/// Reads every job at once, one thread each, and gives each answer back in
/// the jobs' order. A project with no place on disk runs nothing: each of
/// its reads is unreadable with why.
pub fn read_all(jobs: &[Job], place: Option<&Where>) -> Vec<Got> {
    std::thread::scope(|scope| {
        let running: Vec<_> = jobs
            .iter()
            .map(|job| scope.spawn(move || read(job, place)))
            .collect();
        running
            .into_iter()
            .zip(jobs)
            .map(|(running, job)| {
                running.join().unwrap_or_else(|_| Got {
                    source: job.source.clone(),
                    outcome: Outcome::Failed("the read panicked".to_string()),
                    mark: None,
                })
            })
            .collect()
    })
}

fn read(job: &Job, place: Option<&Where>) -> Got {
    let Some(place) = place else {
        return Got {
            source: job.source.clone(),
            outcome: Outcome::Failed(
                "the project has no checkout on disk to read its monitors in".to_string(),
            ),
            mark: None,
        };
    };
    let (outcome, mark) = match &job.source {
        Source::Command { command, cwd } => (run(command, cwd.as_deref(), job, place), None),
        Source::File { file, cwd } => match file_read(file, cwd.as_deref(), place) {
            Ok((outcome, mark)) => (outcome, Some(mark)),
            Err(why) => (Outcome::Failed(why), None),
        },
    };
    Got {
        source: job.source.clone(),
        outcome,
        mark,
    }
}

/// The place a `from` names by its `cwd`: the checkout's root, or one of
/// its repositories (§FS-021-monitors.5.3).
fn place_of(cwd: Option<&str>) -> Result<Place, String> {
    match cwd {
        None => Ok(Place::Root),
        Some(cwd) => Place::parse(cwd).map_err(|err| err.to_string()),
    }
}

/// Runs a command as a summons (§FS-021-monitors.5.4, §FS-021-monitors.5.5):
/// the project's dossier with every item variable blank, the monitors it
/// reads, and its timeout. `0` answers, `75` parks, anything else and a
/// timeout leave every monitor it reads unreadable. An answer that does not
/// read refuses that answer only.
fn run(command: &str, cwd: Option<&str>, job: &Job, at: &Where) -> Outcome {
    let place = match place_of(cwd) {
        Ok(place) => place,
        Err(why) => return Outcome::Failed(why),
    };
    let mut dossier = crate::seams::dossier::of_branch(
        at.project,
        at.root,
        at.checkout,
        at.organization,
        None,
        at.forest,
    );
    dossier.push((
        MONITORS_VAR.to_string(),
        job.reads.iter().cloned().collect::<Vec<_>>().join(","),
    ));
    let summons = Summons::new("monitor", command).at(place).carrying(dossier);
    let carried: RefCell<Option<Vec<Json>>> = RefCell::new(None);
    // The samples ride beside the validated envelope rather than in it, so
    // the public answer types stay as every other consumer reads them.
    let reader = |text: &str, verb: &str, place: &Path| {
        let normalized = crate::seams::answer::parse(text, verb, place)?;
        *carried.borrow_mut() = Some(samples(text));
        Ok(normalized)
    };
    let timeout = Mode::Captured(Duration::from_secs(job.timeout));
    match summons::run_with_answer_reader(&summons, &Site::root(at.checkout), timeout, &reader) {
        Err(err) => Outcome::Failed(err.to_string()),
        Ok(answer) => match answer.outcome {
            summons::Outcome::Done => Outcome::Answered(carried.take().unwrap_or_default()),
            summons::Outcome::Parked => Outcome::Parked,
            summons::Outcome::Failed => Outcome::Failed(match answer.exit_code {
                Some(code) => format!("{command} exited {code}"),
                None => format!("{command} was killed"),
            }),
        },
    }
}

/// The `monitors` of an answer the shared reader already validated.
pub fn samples(text: &str) -> Vec<Json> {
    serde_json::from_str::<Json>(text)
        .ok()
        .and_then(|input| input.get("monitors").and_then(Json::as_array).cloned())
        .unwrap_or_default()
}

/// Reads a file in the answer shape (§FS-021-monitors.5.2): a file that is
/// not there, or not an answer, is unreadable. Its mark says whether the
/// next read finds it unchanged (§FS-021-monitors.9.2).
fn file_read(file: &str, cwd: Option<&str>, at: &Where) -> Result<(Outcome, FileMark), String> {
    let site = Site::root(at.checkout);
    let dir = site
        .resolve(&place_of(cwd)?)
        .map_err(|err| err.to_string())?;
    let path: PathBuf = dir.join(file);
    let bytes = std::fs::read(&path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => format!("{file} is not there"),
        _ => format!("cannot read {file}: {err}"),
    })?;
    let modified = std::fs::metadata(&path)
        .and_then(|meta| meta.modified())
        .map_err(|err| format!("cannot read {file}: {err}"))?;
    let mark = FileMark {
        mtime: DateTime::<Utc>::from(modified),
        digest: digest(&bytes),
    };
    let text = String::from_utf8_lossy(&bytes);
    let outcome = match crate::seams::answer::parse(&text, "monitor", &dir) {
        Ok(_) => Outcome::Answered(samples(&text)),
        Err(err) => Outcome::Failed(format!("{file} is not an answer: {err}")),
    };
    Ok((outcome, mark))
}

/// A file's content, digested so an unchanged file is known without keeping
/// it: FNV-1a, 64 bits.
pub fn digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
#[path = "reader_tests.rs"]
mod tests;
