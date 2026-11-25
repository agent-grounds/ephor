//! Reading a monitor's `from` (§FS-021-monitors.5): a command run once per
//! read through the one summons executor every seam uses, or a file read and
//! never run. Each distinct `from` is read once however many monitors name it
//! (§FS-021-monitors.5.1), and the runs of one refresh go in parallel.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

/// The most of a file a read takes (§FS-021-monitors.5.2): an answer is a
/// few samples, and a file that keeps growing must not hold the refresh.
pub const FILE_LIMIT_BYTES: u64 = 16 * 1024 * 1024;

/// What one read brought: the answer's samples, or why there are none, a
/// file's mark where the `from` is a file (§FS-021-monitors.9.2), and the
/// moment the answer was taken, which a sample is measured at where it says
/// no other (§FS-021-monitors.7.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Got {
    pub source: Source,
    pub outcome: Outcome,
    pub mark: Option<FileMark>,
    pub read_at: DateTime<Utc>,
}

/// The clock a refresh reads by: the moment it began, and how long has
/// passed since, so an answer is timed when it is taken rather than when the
/// refresh began (§FS-021-monitors.7.1), and a test that fixes the moment
/// still times its reads after it.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    began: DateTime<Utc>,
    instant: Instant,
}

impl Clock {
    pub fn starting(began: DateTime<Utc>) -> Clock {
        Clock {
            began,
            instant: Instant::now(),
        }
    }

    pub fn now(&self) -> DateTime<Utc> {
        chrono::TimeDelta::from_std(self.instant.elapsed())
            .ok()
            .and_then(|elapsed| self.began.checked_add_signed(elapsed))
            .unwrap_or(self.began)
    }
}

/// Reads every job at once, one thread each, and gives each answer back in
/// the jobs' order. A project with no place on disk runs nothing: each of
/// its reads is unreadable with why.
pub fn read_all(jobs: &[Job], place: Option<&Where>, clock: &Clock) -> Vec<Got> {
    std::thread::scope(|scope| {
        let running: Vec<_> = jobs
            .iter()
            .map(|job| scope.spawn(move || read(job, place, clock)))
            .collect();
        running
            .into_iter()
            .zip(jobs)
            .map(|(running, job)| {
                running.join().unwrap_or_else(|_| Got {
                    source: job.source.clone(),
                    outcome: Outcome::Failed("the read panicked".to_string()),
                    mark: None,
                    read_at: clock.now(),
                })
            })
            .collect()
    })
}

fn read(job: &Job, place: Option<&Where>, clock: &Clock) -> Got {
    let Some(place) = place else {
        return Got {
            source: job.source.clone(),
            outcome: Outcome::Failed(
                "the project has no checkout on disk to read its monitors in".to_string(),
            ),
            mark: None,
            read_at: clock.now(),
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
        read_at: clock.now(),
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
    // §FS-021-monitors.5.5: a monitor answers by its exit code and its answer, never its output.
    let timeout = Mode::Drained(Duration::from_secs(job.timeout));
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
/// missing, or not an answer, is unreadable, in the words a matter's title
/// says it (§FS-021-monitors.12.1). Only a regular file is read, and no more
/// of it than [`FILE_LIMIT_BYTES`]: a pipe or a device would hold the
/// refresh, and every project's after it, for one monitor's mistake
/// (§FS-021-monitors.17.2). Its mark says whether the next read finds it
/// unchanged (§FS-021-monitors.9.2).
fn file_read(file: &str, cwd: Option<&str>, at: &Where) -> Result<(Outcome, FileMark), String> {
    let site = Site::root(at.checkout);
    let dir = site
        .resolve(&place_of(cwd)?)
        .map_err(|err| err.to_string())?;
    let path: PathBuf = dir.join(file);
    let cannot = |err: std::io::Error| format!("cannot read {file}: {err}");
    let meta = std::fs::metadata(&path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => format!("{file} is missing"),
        _ => cannot(err),
    })?;
    if !meta.is_file() {
        return Err(format!("{file} is not a regular file"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|opened| opened.take(FILE_LIMIT_BYTES + 1).read_to_end(&mut bytes))
        .map_err(cannot)?;
    if bytes.len() as u64 > FILE_LIMIT_BYTES {
        return Err(format!(
            "{file} is larger than {FILE_LIMIT_BYTES} bytes, more than an answer is"
        ));
    }
    let modified = meta.modified().map_err(cannot)?;
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
