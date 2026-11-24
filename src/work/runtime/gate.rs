//! A gate's brief and the answer to it, in the bound runner's own words
//! (§AR-007-runtime.1, §FS-005-dispatch.9.3).
//!
//! Two summons, `work.gate.show` and `work.gate.answer`, behind one runtime
//! feature, `work.gate`, asked of the runner once per process
//! (§AR-005-capabilities.4). The runner routes: what goes up from here is its
//! brief and its verdict as data, never a destination this module picked
//! (§DA-012-the-runtime-routes-a-gate-answer). A later gate verb — the listing,
//! the schemas — is one more verb through [`summon`] under the same feature.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{EphorError, Result};
use crate::seams::summons::{self, quote, Answer, Mode, Site, Summons};
use crate::work::recipe::WorkConfig;

use super::human_gate::{Gate, Review};

/// The feature the gate verbs sit behind (§AR-005-capabilities.4).
pub const FEATURE: &str = "work.gate";
/// Reading one gate's brief.
pub const SHOW_VERB: &str = "work.gate.show";
/// Sending one gate's answer, as a dry run or for real.
pub const ANSWER_VERB: &str = "work.gate.answer";
/// The probe: the runner's own schema verb, which exits zero only where the
/// gate verbs exist. The guide asks the same verb for the schemas it prints
/// (§FS-020-guide.4).
pub const SCHEMA_VERB: &str = "work.gate.schema";
/// Listing every gate under one work root (§FS-019-waiting.1.1.1).
pub const LIST_VERB: &str = "work.gate.list";

/// The version of the listing the binding reads.
const LIST_VERSION: u64 = 1;

/// The version of the answer document the binding reads. The binding's
/// grammar, so it is spelled here and nowhere above.
const ANSWER_VERSION: u64 = 1;

/// The ids the runtime gives its own stand-in questions: choosing a
/// destination and giving a reason. A required question of any other id is
/// the workflow's own (§FS-005-dispatch.9.3).
const STAND_INS: [&str; 2] = ["destination", "reason"];

/// How long one gate verb gets. A local process reading a plan.
const TIMEOUT: Duration = Duration::from_secs(30);

/// How long the probe gets: a schema printed by a binary already on PATH.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// One gate's brief, as the runtime gave it (§FS-005-dispatch.9.3). Fields the
/// review already carries on the gate — ticket, state, plan, root — are not
/// repeated; a field the runtime adds later is ignored rather than refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Brief {
    pub title: String,
    /// `request`, `questions` or `synthesized`: where the runtime took the
    /// questions from.
    pub source: String,
    pub visit: u64,
    /// When the ticket entered the gate, as the runtime wrote it; None where
    /// the runtime does not know, which every surface says as *unknown*.
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub display: Option<BriefFile>,
    #[serde(default)]
    pub attachments: Vec<BriefFile>,
    #[serde(default)]
    pub questions: Vec<Question>,
    #[serde(default)]
    pub destinations: Vec<Exit>,
    /// The runtime's routes, kept as it wrote them: shown, never evaluated
    /// (§DA-012-the-runtime-routes-a-gate-answer).
    #[serde(default)]
    pub routes: Vec<Value>,
    /// Opaque. Copied into the answer unchanged, never computed here.
    pub digest: String,
    #[serde(default)]
    pub answer_path: Option<String>,
    #[serde(default)]
    pub problems: Vec<String>,
}

/// A file the brief names, with whether the runtime could read it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BriefFile {
    pub path: String,
    pub absolute: PathBuf,
    #[serde(default)]
    pub readable: bool,
    #[serde(default)]
    pub problem: Option<String>,
}

/// One question: `boolean`, `choice`, `multi` or `text`. Kept as a word, so a
/// kind a newer runtime adds is shown and refused by the runtime rather than
/// by ephor's parser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub kind: String,
    pub prompt: String,
    #[serde(default = "required_by_default")]
    pub required: bool,
    #[serde(default)]
    pub options: Vec<Choice>,
    #[serde(default)]
    pub help: Option<String>,
}

fn required_by_default() -> bool {
    true
}

/// One option of a choice or of several.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    pub value: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl Choice {
    /// What the reader is shown: the label, or the value where there is none.
    pub fn shown(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.value)
    }
}

/// One declared way out of the gate, as the brief lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exit {
    pub state: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default, rename = "final")]
    pub terminal: bool,
}

impl Brief {
    /// Whether the brief asks a required question of the workflow's own,
    /// which the runtime will not let a plain transition skip
    /// (§FS-005-dispatch.9.3).
    pub fn asks_its_own(&self) -> bool {
        self.questions
            .iter()
            .any(|question| question.required && !STAND_INS.contains(&question.id.as_str()))
    }
}

/// What the runtime answered about one answer (§FS-005-dispatch.9.3). Every
/// field a refusal can leave unknown is optional: the runtime fills what it
/// knew when it refused, and nothing here guesses the rest.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub ok: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub route: Option<Value>,
    #[serde(default)]
    pub answer_path: Option<String>,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub errors: Vec<Refusal>,
}

/// One reason the runtime refused, naming the question where it names one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
    pub kind: String,
    #[serde(default)]
    pub question: Option<String>,
    pub message: String,
}

impl Verdict {
    /// The refusal as one line, for a message: each of the runtime's reasons,
    /// in its words.
    pub fn says(&self) -> Option<String> {
        if self.ok {
            return None;
        }
        let reasons: Vec<String> = self
            .errors
            .iter()
            .map(|error| match &error.question {
                Some(question) => format!("{question}: {}", error.message),
                None => error.message.clone(),
            })
            .collect();
        Some(match reasons.is_empty() {
            true => "the runtime refused the answer and gave no reason".to_string(),
            false => reasons.join("; "),
        })
    }
}

/// The answer document the binding reads: its version, the brief's digest
/// copied unchanged, and the answers by question id (§FS-005-dispatch.9.3).
pub fn document(brief: &Brief, answers: &BTreeMap<String, Value>) -> Value {
    serde_json::json!({
        "version": ANSWER_VERSION,
        "request_digest": brief.digest,
        "answers": answers,
    })
}

/// What each bound runner answered about the gate verbs, once per process
/// (§AR-005-capabilities.4).
static BRIEFS: OnceLock<Mutex<BTreeMap<String, bool>>> = OnceLock::new();

/// Whether the bound runner prepares gate briefs and routes their answers
/// (§AR-005-capabilities.4). False with nothing bound, and false for a runner
/// that has no gate verbs or could not be asked: every surface is then what
/// it was before (§AR-007-runtime.3).
pub fn briefs(config: &WorkConfig) -> bool {
    if super::refusal(config).is_some() {
        return false;
    }
    let runner = super::runner(config).to_string();
    let seen = BRIEFS.get_or_init(Default::default);
    if let Ok(known) = seen.lock() {
        if let Some(answer) = known.get(&runner) {
            return *answer;
        }
    }
    let answer = probe(&runner);
    if let Ok(mut known) = seen.lock() {
        known.insert(runner, answer);
    }
    answer
}

/// The probe: the runner's own schema verb, which exits zero and prints its
/// schemas only where the gate verbs exist. Both are asked, so a runner that
/// answers every word with success and says nothing is not taken for one that
/// briefs. Run from the temporary directory, because a probe asks about the
/// binary and never about wherever ephor happened to be started.
fn probe(runner: &str) -> bool {
    let command = format!("{runner} gate schema 2>/dev/null");
    let Ok(answer) = summons::run(
        &Summons::new(SCHEMA_VERB, command),
        &Site::root(std::env::temp_dir()),
        Mode::Captured(PROBE_TIMEOUT),
    ) else {
        return false;
    };
    answer.is_done() && prints_schemas(answer.output.as_deref().unwrap_or(""))
}

/// Whether the schema verb's output is the document of schemas it promises.
fn prints_schemas(output: &str) -> bool {
    serde_json::from_str::<Value>(output)
        .is_ok_and(|document| document.get("schemas").is_some_and(Value::is_object))
}

/// Why the gate verbs are refused, or None where the runner has them: the
/// sentence an answer gets from a runtime that prepares no briefs
/// (§FS-005-dispatch.9.3).
pub fn refusal(config: &WorkConfig) -> Option<String> {
    if let Some(reason) = super::refusal(config) {
        return Some(reason);
    }
    (!briefs(config)).then(|| {
        format!(
            "{} prepares no gate briefs; move the gate with `ephor work transition`",
            super::runner(config)
        )
    })
}

/// One gate verb, run captured from `root`: `<runner> gate <verb> <args>…`,
/// every argument quoted. Standard error stays beside standard output, never
/// in it, because the runner's JSON is on standard output alone.
pub(crate) fn summon(
    config: &WorkConfig,
    verb: &str,
    root: &Path,
    args: &[String],
) -> Result<Answer> {
    let mut command = format!("{} gate", super::runner(config));
    for arg in args {
        command.push(' ');
        command.push_str(&quote(arg));
    }
    summons::run(
        &Summons::new(verb, command),
        &Site::root(root),
        Mode::Captured(TIMEOUT),
    )
}

/// The arguments every per-ticket gate verb takes: the plan, the ticket, and
/// the machine the review read it under.
fn ticket_args(sub: &str, gate: &Gate) -> Vec<String> {
    let mut args = vec![
        sub.to_string(),
        gate.plan.to_string_lossy().into_owned(),
        "--task".to_string(),
        gate.ticket.clone(),
    ];
    if let Some(machine) = &gate.machine {
        args.push("--state-machine".to_string());
        args.push(machine.to_string_lossy().into_owned());
    }
    args
}

/// The runner's own sentence for a summons that did not answer as asked: the
/// first meaningful lines of what it said, standard error first.
fn sentence(config: &WorkConfig, verb: &str, answer: &Answer) -> EphorError {
    let said = [answer.errors.as_deref(), answer.output.as_deref()]
        .into_iter()
        .flatten()
        .map(super::said)
        .find(|said| !said.is_empty())
        .unwrap_or_default();
    EphorError::Command(match said.is_empty() {
        true => format!("{}: {}", super::runner(config), answer.refusal(verb)),
        false => format!("{}: {said}", super::runner(config)),
    })
}

/// One gate's brief (§FS-005-dispatch.9.3).
pub fn show(config: &WorkConfig, gate: &Gate) -> Result<Brief> {
    let mut args = ticket_args("show", gate);
    args.push("--json".to_string());
    let answer = summon(config, SHOW_VERB, &gate.root, &args)?;
    if !answer.is_done() {
        return Err(sentence(config, SHOW_VERB, &answer));
    }
    serde_json::from_str(answer.output.as_deref().unwrap_or("")).map_err(|err| {
        EphorError::Command(format!(
            "cannot read the brief {} gave: {err}",
            super::runner(config)
        ))
    })
}

/// Every gate of a review with its brief, where the runner prepares briefs;
/// a gate it could not brief keeps the runner's sentence instead and stays
/// reviewable as before. With no gate verbs the review is left exactly as it
/// was read (§FS-005-dispatch.9.3).
pub fn brief_review(config: &WorkConfig, review: &mut Review) {
    if review.gates.is_empty() || !briefs(config) {
        return;
    }
    for gate in &mut review.gates {
        match show(config, gate) {
            Ok(brief) => gate.brief = Some(brief),
            Err(err) => gate.unbriefed = Some(err.to_string()),
        }
    }
}

/// One gate as the runner's listing gives it: what the first screen could not
/// know from the plans alone (§FS-019-waiting.1.1.1). Only these fields are
/// read; a field the runner adds later is ignored rather than refused.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Listed {
    /// The project-qualified ticket id, `<plan>.<ticket>`.
    pub ticket: String,
    pub state: String,
    /// The brief's title.
    #[serde(default)]
    pub title: String,
    /// When the ticket entered the gate, as the runner wrote it; None where
    /// the runner does not know.
    #[serde(default)]
    pub since: Option<String>,
    /// How many unfinished tasks cannot move until the ticket leaves.
    pub holds: u64,
}

/// The listing's envelope.
#[derive(Deserialize)]
struct Listing {
    version: u64,
    gates: Vec<Listed>,
}

/// Every gate under one work root, in the runner's words: one summons for the
/// whole root, never one per gate (§FS-019-waiting.1.1.1). The caller asks only
/// where [`briefs`] holds; a listing that cannot be had is an error the caller
/// reads as "this root's rows stay as the enumeration gave them".
pub fn list(config: &WorkConfig, root: &Path) -> Result<Vec<Listed>> {
    let args = vec![
        "list".to_string(),
        root.to_string_lossy().into_owned(),
        "--json".to_string(),
    ];
    let answer = summon(config, LIST_VERB, root, &args)?;
    if !answer.is_done() {
        return Err(sentence(config, LIST_VERB, &answer));
    }
    let unread = |why: String| {
        EphorError::Command(format!(
            "cannot read the gates {} listed: {why}",
            super::runner(config)
        ))
    };
    let listing: Listing = serde_json::from_str(answer.output.as_deref().unwrap_or(""))
        .map_err(|err| unread(err.to_string()))?;
    if listing.version != LIST_VERSION {
        return Err(unread(format!("version {}", listing.version)));
    }
    Ok(listing.gates)
}

/// The runner's own request and answer schemas, as it printed them: the
/// guide carries them and ephor keeps no copy (§FS-020-guide.4). The error is
/// the sentence the guide says in their place — no runner bound, a runner
/// that prepares no briefs, or one that could not be asked.
pub fn schemas(config: &WorkConfig) -> std::result::Result<(Value, Value), String> {
    if let Some(reason) = super::refusal(config) {
        return Err(reason);
    }
    if !briefs(config) {
        return Err(format!(
            "{} prepares no gate briefs, so there are no request or answer schemas to print",
            super::runner(config)
        ));
    }
    let answer = summon(
        config,
        SCHEMA_VERB,
        &std::env::temp_dir(),
        &["schema".to_string()],
    )
    .map_err(|err| err.to_string())?;
    if !answer.is_done() {
        return Err(sentence(config, SCHEMA_VERB, &answer).to_string());
    }
    let document: Value =
        serde_json::from_str(answer.output.as_deref().unwrap_or("")).map_err(|err| {
            format!(
                "cannot read the schemas {} printed: {err}",
                super::runner(config)
            )
        })?;
    let named = |name: &str| {
        document
            .get("schemas")
            .and_then(|schemas| schemas.get(name))
            .cloned()
            .ok_or_else(|| format!("{} printed no {name} schema", super::runner(config)))
    };
    Ok((named("request")?, named("answer")?))
}

/// Send one answer document, unchanged, and read back the runtime's verdict
/// (§FS-005-dispatch.9.3). A refusal is a verdict, not an error: the runner
/// exits non-zero and still prints it. An error here is only a runner that
/// could not be asked or printed no verdict at all.
pub fn answer(
    config: &WorkConfig,
    gate: &Gate,
    document: &Value,
    dry_run: bool,
) -> Result<Verdict> {
    if let Some(reason) = refusal(config) {
        return Err(EphorError::Command(reason));
    }
    let private = Private::write(document)?;
    let mut args = ticket_args("answer", gate);
    args.extend([
        "--from".to_string(),
        gate.state.clone(),
        "--answers".to_string(),
        private.path.to_string_lossy().into_owned(),
        "--json".to_string(),
    ]);
    if dry_run {
        args.push("--dry-run".to_string());
    }
    let answer = summon(config, ANSWER_VERB, &gate.root, &args);
    drop(private);
    let answer = answer?;
    match serde_json::from_str::<Verdict>(answer.output.as_deref().unwrap_or("")) {
        Ok(verdict) => Ok(verdict),
        Err(_) => Err(sentence(config, ANSWER_VERB, &answer)),
    }
}

/// The answers on their way to the runner: a file only this account can
/// read, in a directory of its own under the system's temporary directory,
/// both removed when this is dropped — so the plan's directory holds only
/// what the runner records (§FS-005-dispatch.9.3).
struct Private {
    dir: PathBuf,
    path: PathBuf,
}

impl Private {
    fn write(document: &Value) -> Result<Private> {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let base = std::env::temp_dir();
        let text = serde_json::to_string(document)
            .map_err(|err| EphorError::Command(format!("cannot write the answers: {err}")))?;
        for _ in 0..64 {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = base.join(format!("ephor-gate-{}-{serial}", std::process::id()));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&dir) {
                Ok(()) => {
                    let path = dir.join("answers.json");
                    let private = Private { dir, path };
                    std::fs::write(&private.path, text).map_err(|err| {
                        EphorError::Command(format!(
                            "cannot write the answers to {}: {err}",
                            private.path.display()
                        ))
                    })?;
                    return Ok(private);
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(EphorError::Command(format!(
                        "cannot make a place for the answers in {}: {err}",
                        base.display()
                    )))
                }
            }
        }
        Err(EphorError::Command(format!(
            "cannot make a place for the answers in {}",
            base.display()
        )))
    }
}

impl Drop for Private {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(all(test, unix))]
#[path = "gate_tests.rs"]
mod tests;
