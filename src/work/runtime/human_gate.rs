//! Local review and ordinary owner transitions (§FS-005-dispatch.9.1).

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{EphorError, Result};
use crate::feed::model::{Item, ItemKind};
use crate::seams::summons::{self, quote, Mode, Site, Summons};
use crate::work::recipe::WorkConfig;

use super::plan::{self, Plan};

const FILE_LIMIT: usize = 256 * 1024;

/// What a review with no live gate says on both surfaces, still true where
/// finished files are listed under it (§FS-005-dispatch.9.2).
pub const NO_GATE: &str = "No work is waiting at a human gate for this matter.";

#[derive(Debug, Clone, Default, Serialize)]
pub struct Review {
    pub gates: Vec<Gate>,
    pub files: Vec<Artifact>,
    /// The files of finished blocked tickets, held back until every plan's
    /// gates have listed theirs, then appended by [`Review::settled`]
    /// (§FS-005-dispatch.9.2).
    #[serde(skip)]
    pub(crate) finished: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Gate {
    pub ticket: String,
    pub title: String,
    pub state: String,
    pub root: PathBuf,
    pub plan: PathBuf,
    pub machine: Option<PathBuf>,
    pub destinations: Vec<Destination>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Destination {
    pub state: String,
    pub condition: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Artifact {
    pub name: String,
    pub path: PathBuf,
    pub problem: Option<String>,
}

#[derive(Deserialize)]
struct Machine {
    states: Vec<State>,
    transitions: Vec<Edge>,
}

#[derive(Deserialize)]
struct State {
    name: String,
    description: Option<String>,
    #[serde(default)]
    gating: bool,
    #[serde(default, rename = "final")]
    is_final: bool,
    #[serde(default)]
    inputs: Vec<DeclaredArtifact>,
    #[serde(default)]
    outputs: Vec<DeclaredArtifact>,
}

#[derive(Deserialize)]
struct DeclaredArtifact {
    name: String,
    path: String,
    #[serde(default)]
    optional: bool,
}

#[derive(Deserialize)]
struct Edge {
    from: String,
    to: String,
    condition: Option<String>,
    exit_code: Option<serde_json::Value>,
}

fn refused(message: impl Into<String>) -> EphorError {
    EphorError::Command(message.into())
}

fn machine_flag(machine: Option<&Path>) -> String {
    machine
        .map(|path| format!(" --state-machine {}", quote(&path.to_string_lossy())))
        .unwrap_or_default()
}

fn captured(config: &WorkConfig, root: &Path, command: String) -> Result<String> {
    if let Some(reason) = super::refusal(config) {
        return Err(refused(reason));
    }
    let answer = summons::run(
        &Summons::new("work.review", command),
        &Site::root(root),
        Mode::Captured(Duration::from_secs(30)),
    )?;
    let output = answer.output.as_deref().unwrap_or("");
    if !answer.is_done() {
        return Err(refused(format!(
            "{}: {} · {}",
            super::runner(config),
            answer.refusal("work.review"),
            output.trim()
        )));
    }
    Ok(output.to_string())
}

/// The source task's own plan, never newly dispatched work (§FS-005-dispatch.9.1).
pub fn source_plan(item: &Item) -> Result<Option<(PathBuf, String, String)>> {
    if item.source != super::RUNNER || item.kind != ItemKind::Task {
        return Ok(None);
    }
    let path = PathBuf::from(
        item.raw
            .get("plan")
            .and_then(|value| value.as_str())
            .ok_or_else(|| refused("the task has no local plan path"))?,
    );
    let qualified = item
        .id
        .strip_prefix(&format!("{}:", super::RUNNER))
        .ok_or_else(|| refused("the task has no source-qualified identity"))?;
    let plan =
        Plan::read(&path)?.ok_or_else(|| refused(format!("{} is missing", path.display())))?;
    let mut tasks = plan.tickets();
    tasks.sort_by_key(|task| std::cmp::Reverse(task.id.len()));
    let task = tasks
        .into_iter()
        .find(|task| qualified.ends_with(&format!(".{}", task.id)))
        .ok_or_else(|| refused(format!("{} is no longer in {}", item.id, path.display())))?;
    let plan_id = qualified
        .strip_suffix(&format!(".{}", task.id))
        .expect("matched task suffix");
    Ok(Some((path, plan_id.to_string(), task.id)))
}

/// Read the effective machine through the runtime, keeping its imports and
/// inherited transition rules authoritative (§FS-005-dispatch.9.1).
pub fn review_plan(
    config: &WorkConfig,
    root: &Path,
    path: &Path,
    plan_id: &str,
    only_ticket: Option<&str>,
) -> Result<Review> {
    let plan =
        Plan::read(path)?.ok_or_else(|| refused(format!("{} is missing", path.display())))?;
    let execution_root = plan::own_store(path).unwrap_or(root);
    let machine = [execution_root.join(plan::STATES), root.join(plan::STATES)]
        .into_iter()
        .find(|path| path.is_file());
    let root = execution_root;
    let output = captured(
        config,
        root,
        format!(
            "{} states {}{} --json 2>&1",
            super::runner(config),
            quote(&path.to_string_lossy()),
            machine_flag(machine.as_deref())
        ),
    )?;
    let machine_data: Machine = serde_json::from_str(&output)
        .map_err(|err| refused(format!("cannot read the runtime's state machine: {err}")))?;
    let mut review = Review::default();
    for (task, task_path) in plan.tickets_with_paths() {
        if only_ticket.is_some_and(|wanted| wanted != task.id) {
            continue;
        }
        let current = task.state.as_ref().and_then(|current| {
            machine_data
                .states
                .iter()
                .find(|state| state.name == *current)
        });
        // Gates come from current state alone (§FS-005-dispatch.9.1); a final
        // ticket that wrote `blocked` is history beside them, never a gate
        // (§FS-005-dispatch.9.2).
        let Some(state) = current.filter(|state| state.gating) else {
            if current.is_some_and(|state| state.is_final) && !task.cancelled() {
                review
                    .finished
                    .extend(super::results::finished_blocked(root, plan_id, &task.id));
            }
            continue;
        };
        let ticket = format!("{plan_id}.{}", task.id);
        let mut destinations = Vec::new();
        for edge in &machine_data.transitions {
            if edge.from != state.name && edge.from != "*" {
                continue;
            }
            destinations.push(Destination {
                state: edge.to.clone(),
                description: machine_data
                    .states
                    .iter()
                    .find(|target| target.name == edge.to)
                    .and_then(|target| target.description.clone()),
                condition: edge.condition.clone().or_else(|| {
                    edge.exit_code
                        .as_ref()
                        .map(|code| format!("exit code {code}"))
                }),
            });
        }
        review.gates.push(Gate {
            ticket: ticket.clone(),
            title: task.title,
            state: state.name.clone(),
            root: root.to_path_buf(),
            plan: path.to_path_buf(),
            machine: machine.clone(),
            destinations,
        });
        review.add_file(format!("{ticket} · plan"), path.to_path_buf(), None);
        review.add_file(format!("{ticket} · task"), task_path.to_path_buf(), None);
        if let Some(machine) = &machine {
            review.add_file(format!("{ticket} · state machine"), machine.clone(), None);
        }
        review.add_links(task_path, &ticket);
        for (kind, artifacts) in [("input", &state.inputs), ("output", &state.outputs)] {
            for artifact in artifacts {
                let expanded = artifact
                    .path
                    .replace("{task_id}", &ticket)
                    .replace("{state}", &state.name);
                let problem = (expanded.contains('{') || expanded.contains('}'))
                    .then(|| "unresolved artifact template".to_string());
                review.add_file(
                    format!(
                        "{ticket} · {kind} {}{}",
                        artifact.name,
                        if artifact.optional { " (optional)" } else { "" }
                    ),
                    root.join(expanded),
                    problem,
                );
            }
        }
        let result = root.join("runtime/results").join(format!("{ticket}.md"));
        if result.exists() {
            review.add_links(&result, &ticket);
            review.add_file(format!("{ticket} · result"), result, None);
        }
    }
    Ok(review)
}

impl Review {
    pub fn extend(&mut self, review: Review) {
        self.gates.extend(review.gates);
        for file in review.files {
            self.add_file(file.name, file.path, file.problem);
        }
        self.finished.extend(review.finished);
    }

    /// The review as it is read: every live gate's materials first, then each
    /// finished blocked ticket's verdict and result in plan order, a path
    /// already listed not listed twice (§FS-005-dispatch.9.2).
    pub fn settled(mut self) -> Review {
        for file in std::mem::take(&mut self.finished) {
            self.add_file(file.name, file.path, file.problem);
        }
        self
    }

    fn add_file(&mut self, name: String, path: PathBuf, problem: Option<String>) {
        if self.files.iter().any(|file| file.path == path) {
            return;
        }
        let problem = problem.or_else(|| unreadable(&path));
        self.files.push(Artifact {
            name,
            path,
            problem,
        });
    }

    fn add_links(&mut self, source: &Path, ticket: &str) {
        let root = source.parent().unwrap_or(Path::new("."));
        let artifact = Artifact {
            name: String::new(),
            path: source.to_path_buf(),
            problem: None,
        };
        let Ok(text) = read_file(&artifact) else {
            return;
        };
        for fragment in text.split("](").skip(1).take(128) {
            let Some((destination, _)) = fragment.split_once(')') else {
                continue;
            };
            let destination = destination.trim();
            let destination = if let Some(bracketed) = destination.strip_prefix('<') {
                bracketed
                    .split_once('>')
                    .map(|(path, _)| path)
                    .unwrap_or("")
            } else {
                destination.split_whitespace().next().unwrap_or("")
            };
            let destination = destination.split('#').next().unwrap_or("");
            if destination.is_empty() || destination.contains(':') || destination.starts_with("//")
            {
                continue;
            }
            self.add_file(
                format!("{ticket} · linked file"),
                root.join(destination),
                None,
            );
        }
    }
}

/// Why a review file cannot be opened, where it cannot — the one availability
/// check every review file goes through (§FS-005-dispatch.9.1).
pub fn unreadable(path: &Path) -> Option<String> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => File::open(path).err().map(|err| err.to_string()),
        Ok(_) => Some("not a regular file".to_string()),
        Err(err) => Some(err.to_string()),
    }
}

/// Bounded local viewing neither starts work nor prepares outbound content
/// (§FS-005-dispatch.9.1).
pub fn read_file(artifact: &Artifact) -> Result<String> {
    if let Some(problem) = &artifact.problem {
        return Err(refused(format!("{}: {problem}", artifact.path.display())));
    }
    let mut bytes = Vec::new();
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(&artifact.path)?;
    if !file.metadata()?.is_file() {
        return Err(refused("not a regular file"));
    }
    file.take((FILE_LIMIT + 1) as u64).read_to_end(&mut bytes)?;
    let truncated = bytes.len() > FILE_LIMIT;
    bytes.truncate(FILE_LIMIT);
    if bytes.contains(&0) {
        return Err(refused("not a text file"));
    }
    if truncated {
        if let Err(error) = std::str::from_utf8(&bytes) {
            if error.error_len().is_none() {
                bytes.truncate(error.valid_up_to());
            }
        }
    }
    let mut text = String::from_utf8(bytes).map_err(|_| refused("not a UTF-8 text file"))?;
    text = text
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .collect();
    if truncated {
        text.push_str("\n\n[Preview truncated at 256 KiB]");
    }
    Ok(text)
}

/// A normal runtime transition with its original-state guard and callbacks;
/// neither an override nor a direct edit of the task (§FS-005-dispatch.9.1).
pub fn transition(config: &WorkConfig, gate: &Gate, to: &str, result: &str) -> Result<()> {
    if !gate
        .destinations
        .iter()
        .any(|destination| destination.state == to)
    {
        return Err(refused(format!(
            "{} declares no transition to '{to}'",
            gate.state
        )));
    }
    captured(
        config,
        &gate.root,
        format!(
            "{} transition {} --task {} --from {} --to {} --result {}{} 2>&1",
            super::runner(config),
            quote(&gate.plan.to_string_lossy()),
            quote(&gate.ticket),
            quote(&gate.state),
            quote(to),
            quote(result),
            machine_flag(gate.machine.as_deref())
        ),
    )?;
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "human_gate_tests.rs"]
mod tests;
