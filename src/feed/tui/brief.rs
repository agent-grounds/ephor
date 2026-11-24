//! A prepared gate, answered in its own form (§FS-005-dispatch.9.3).
//!
//! The brief comes from the runtime through the review, and the verdict comes
//! back from it: this screen draws both and holds what the reader has chosen
//! so far. It never says where an answer leads — the runtime's dry run does,
//! on the confirmation (§DA-012-the-runtime-routes-a-gate-answer) — and every
//! move it makes is `ephor work gate answer`, with `--dry-run` and without
//! (§REQ-002-parity.2).

use std::collections::BTreeMap;
use std::path::PathBuf;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use serde_json::Value;

use crate::feed::model::Item;
use crate::work::runtime::gate::{self, Brief, BriefFile, Question, Refusal, Verdict};
use crate::work::runtime::human_gate::{self, Artifact, Review};

use super::answers::edit_line;
use super::human_gate::Return;
use super::Action;

/// Which half of the screen the motion keys move: the file read first, or the
/// form beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Focus {
    File,
    Form,
}

/// One place the form's cursor can stand: an option of a choice or of
/// several, or the one line of a yes-or-no or a text question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Stop {
    pub question: usize,
    pub option: Option<usize>,
}

pub(crate) struct BriefScreen {
    pub item: Item,
    /// Where `esc` goes: back to Waiting, the overview or the work screen it
    /// was opened from, or to the review it was sent from (§FS-019-waiting.4).
    pub back: Return,
    /// Where that review goes in turn, when `back` is the review.
    pub floor: Return,
    pub ticket: String,
    pub(super) state: String,
    pub(super) brief: Brief,
    /// The bound runtime's name, for the line that says what it will do.
    pub(super) runner: String,
    /// The files the review infers for this ticket, demoted to *other files*.
    pub(super) others: Vec<Artifact>,
    /// The display file's text, or why it cannot be read.
    pub(super) display: std::result::Result<String, String>,
    pub(super) scroll: usize,
    pub(super) viewport: usize,
    pub(super) focus: Focus,
    pub(super) cursor: usize,
    pub(super) answers: BTreeMap<String, Value>,
    /// The line being typed into a text question, while it is.
    pub(super) typing: Option<String>,
    /// The file list `f` opens, and which row of it is selected.
    pub(super) files: Option<usize>,
    /// A file read from that list.
    pub(super) preview: Option<(String, String)>,
    pub(super) preview_scroll: u16,
    /// The runtime's dry-run verdict, waiting for `enter`.
    pub(super) confirm: Option<Verdict>,
    /// What the runtime refused, kept on the form until the next try.
    pub(super) refused: Vec<Refusal>,
    /// A refusal that is one sentence rather than the runtime's list.
    pub(super) says: Option<String>,
}

impl BriefScreen {
    /// The screen for `ticket` in `review`, or None where the runtime gave
    /// that gate no brief.
    pub fn open(item: Item, review: &Review, ticket: &str, runner: &str) -> Option<Self> {
        let gate = review.gates.iter().find(|gate| gate.ticket == ticket)?;
        let brief = gate.brief.clone()?;
        let prefix = format!("{ticket} · ");
        let named: Vec<&std::path::Path> = brief
            .display
            .iter()
            .chain(&brief.attachments)
            .map(|file| file.absolute.as_path())
            .collect();
        let others = review
            .files
            .iter()
            .filter(|file| file.name.starts_with(&prefix))
            .filter(|file| !named.contains(&file.path.as_path()))
            .cloned()
            .collect();
        let display = match &brief.display {
            None => Err("The brief names no file to read first.".to_string()),
            Some(file) => human_gate::read_file(&artifact(file)).map_err(|err| err.to_string()),
        };
        Some(Self {
            item,
            back: Return::Work,
            floor: Return::Work,
            ticket: gate.ticket.clone(),
            state: gate.state.clone(),
            brief,
            runner: runner.to_string(),
            others,
            display,
            scroll: 0,
            viewport: 0,
            focus: Focus::Form,
            cursor: 0,
            answers: BTreeMap::new(),
            typing: None,
            files: None,
            preview: None,
            preview_scroll: 0,
            confirm: None,
            refused: Vec::new(),
            says: None,
        })
    }

    /// The same screen, left by `esc` for `back`, and a review reopened from
    /// it for `floor`.
    pub fn returning(mut self, back: Return, floor: Return) -> Self {
        self.back = back;
        self.floor = floor;
        self
    }

    /// Whether every key is the line's: while a text answer is typed, the
    /// keys that open boards over any screen are letters (§FS-005-dispatch.15).
    pub fn is_typing(&self) -> bool {
        self.typing.is_some()
    }

    /// Every place the cursor can stand, in the order the form is drawn.
    pub(super) fn stops(&self) -> Vec<Stop> {
        let mut stops = Vec::new();
        for (index, question) in self.brief.questions.iter().enumerate() {
            match question.kind.as_str() {
                "choice" | "multi" if !question.options.is_empty() => {
                    stops.extend((0..question.options.len()).map(|option| Stop {
                        question: index,
                        option: Some(option),
                    }));
                }
                _ => stops.push(Stop {
                    question: index,
                    option: None,
                }),
            }
        }
        stops
    }

    fn here(&self) -> Option<Stop> {
        self.stops().get(self.cursor).copied()
    }

    /// Whether a question has an answer that would be sent: a text with words
    /// in it, at least one of several, any yes or no, any choice.
    pub(super) fn answered(&self, question: &Question) -> bool {
        match self.answers.get(&question.id) {
            None | Some(Value::Null) => false,
            Some(Value::String(text)) => !text.trim().is_empty(),
            Some(Value::Array(values)) => !values.is_empty(),
            Some(_) => true,
        }
    }

    /// How many questions are answered, of how many.
    pub(super) fn count(&self) -> (usize, usize) {
        let questions = &self.brief.questions;
        let answered = questions
            .iter()
            .filter(|question| self.answered(question))
            .count();
        (answered, questions.len())
    }

    /// The answer document the form holds: the brief's digest copied, and
    /// every answered question by id (§FS-005-dispatch.9.3).
    pub fn document(&self) -> Value {
        gate::document(&self.brief, &self.answers)
    }

    /// The text a question was answered with, for the editor to start from.
    fn text_of(&self, question: &Question) -> String {
        match self.answers.get(&question.id) {
            Some(Value::String(text)) => text.clone(),
            _ => String::new(),
        }
    }

    /// Set a text answer, as typed or as written in the editor; nothing but
    /// whitespace takes the answer back.
    pub fn set_text(&mut self, id: &str, text: &str) {
        let text = text.trim_end();
        match text.trim().is_empty() {
            true => self.answers.remove(id),
            false => self
                .answers
                .insert(id.to_string(), Value::String(text.to_string())),
        };
    }

    /// The runtime's dry run came back: its verdict is the confirmation, or
    /// its refusal stays on the form (§FS-005-dispatch.9.3).
    pub fn dry_ran(&mut self, outcome: crate::error::Result<Verdict>) {
        match outcome {
            Ok(verdict) if verdict.ok => {
                self.refused.clear();
                self.says = None;
                self.confirm = Some(verdict);
            }
            other => self.refuse(other),
        }
    }

    /// An answer the runtime refused, or one ephor refused before asking:
    /// kept on the form, beside the question it names where it names one.
    pub fn refuse(&mut self, outcome: crate::error::Result<Verdict>) {
        self.confirm = None;
        match outcome {
            Ok(verdict) => {
                // Beside its question where it names one the form draws; on
                // the line above the form otherwise, so nothing is said twice
                // and nothing goes unsaid.
                let asked = |refusal: &Refusal| {
                    refusal.question.as_ref().is_some_and(|id| {
                        self.brief
                            .questions
                            .iter()
                            .any(|question| question.id == *id)
                    })
                };
                let general: Vec<String> = verdict
                    .errors
                    .iter()
                    .filter(|refusal| !asked(refusal))
                    .map(|refusal| match &refusal.question {
                        Some(id) => format!("{id}: {}", refusal.message),
                        None => refusal.message.clone(),
                    })
                    .collect();
                self.says = match (general.is_empty(), verdict.errors.is_empty()) {
                    (false, _) => Some(general.join("; ")),
                    (true, true) => verdict.says(),
                    (true, false) => None,
                };
                self.refused = verdict.errors;
            }
            Err(err) => {
                self.refused.clear();
                self.says = Some(err.to_string());
            }
        }
    }

    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Action {
        if let Some(line) = &mut self.typing {
            match code {
                KeyCode::Esc => self.typing = None,
                KeyCode::Enter => {
                    let line = self.typing.take().unwrap_or_default();
                    if let Some(question) = self.here().map(|stop| stop.question) {
                        let id = self.brief.questions[question].id.clone();
                        self.set_text(&id, &line);
                    }
                }
                _ => {
                    edit_line(line, code, modifiers);
                }
            }
            return Action::None;
        }
        if self.preview.is_some() {
            match code {
                KeyCode::Esc => {
                    self.preview = None;
                    self.preview_scroll = 0;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.preview_scroll = self.preview_scroll.saturating_add(1)
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.preview_scroll = self.preview_scroll.saturating_sub(1)
                }
                KeyCode::PageDown => {
                    self.preview_scroll = self.preview_scroll.saturating_add(self.page() as u16)
                }
                KeyCode::PageUp => {
                    self.preview_scroll = self.preview_scroll.saturating_sub(self.page() as u16)
                }
                _ => {}
            }
            return Action::None;
        }
        if let Some(selected) = self.files {
            let count = self.listed().len();
            match code {
                KeyCode::Esc => self.files = None,
                KeyCode::Char('j') | KeyCode::Down => {
                    self.files = Some((selected + 1).min(count.saturating_sub(1)))
                }
                KeyCode::Char('k') | KeyCode::Up => self.files = Some(selected.saturating_sub(1)),
                KeyCode::Enter => return self.read_listed(selected),
                _ => {}
            }
            return Action::None;
        }
        if self.confirm.is_some() {
            return match code {
                // The same call as the dry run, with `--dry-run` left off
                // (§FS-005-dispatch.9.3).
                KeyCode::Enter => self.answer(false),
                KeyCode::Esc => {
                    self.confirm = None;
                    Action::None
                }
                _ => Action::None,
            };
        }
        match code {
            KeyCode::Esc => self.leave(),
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::File => Focus::Form,
                    Focus::Form => Focus::File,
                };
                Action::None
            }
            KeyCode::Char('f') => {
                match self.listed().is_empty() {
                    true => return Action::SetMessage("The brief names no other files.".into()),
                    false => self.files = Some(0),
                }
                Action::None
            }
            // Asks first: the runtime's dry run is the confirmation
            // (§FS-005-dispatch.9.3).
            KeyCode::Char('s') => self.answer(true),
            _ => match self.focus {
                Focus::File => self.file_key(code),
                Focus::Form => self.form_key(code),
            },
        }
    }

    fn page(&self) -> usize {
        self.viewport.max(1)
    }

    fn file_key(&mut self, code: KeyCode) -> Action {
        let last = self
            .display
            .as_ref()
            .map(|text| text.lines().count().saturating_sub(1))
            .unwrap_or(0);
        self.scroll = match code {
            KeyCode::Char('j') | KeyCode::Down => self.scroll + 1,
            KeyCode::Char('k') | KeyCode::Up => self.scroll.saturating_sub(1),
            KeyCode::PageDown => self.scroll + self.page(),
            KeyCode::PageUp => self.scroll.saturating_sub(self.page()),
            KeyCode::Char('g') => 0,
            KeyCode::Char('G') => last,
            _ => self.scroll,
        }
        .min(last);
        Action::None
    }

    fn form_key(&mut self, code: KeyCode) -> Action {
        let stops = self.stops().len();
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.cursor = (self.cursor + 1).min(stops.saturating_sub(1))
            }
            KeyCode::Char('k') | KeyCode::Up => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Char(' ') | KeyCode::Enter => return self.choose(),
            KeyCode::Char('e') => return self.edit(),
            KeyCode::Backspace => {
                if let Some(stop) = self.here() {
                    let id = self.brief.questions[stop.question].id.clone();
                    self.answers.remove(&id);
                }
            }
            _ => {}
        }
        Action::None
    }

    /// `space` on the stop under the cursor: the one option of a choice, an
    /// option of several in or out, yes then no, or a line to type.
    fn choose(&mut self) -> Action {
        let Some(stop) = self.here() else {
            return Action::None;
        };
        let question = self.brief.questions[stop.question].clone();
        let value = |option: usize| Value::String(question.options[option].value.clone());
        match (question.kind.as_str(), stop.option) {
            ("choice", Some(option)) => {
                self.answers.insert(question.id, value(option));
            }
            ("multi", Some(option)) => {
                let chosen: Vec<String> = match self.answers.get(&question.id) {
                    Some(Value::Array(values)) => values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .collect(),
                    _ => Vec::new(),
                };
                let toggled = &question.options[option].value;
                // In the order the brief lists them, whatever order they
                // were taken in.
                let kept: Vec<Value> = question
                    .options
                    .iter()
                    .map(|choice| &choice.value)
                    .filter(|value| (*value == toggled) != chosen.contains(value))
                    .map(|value| Value::String(value.clone()))
                    .collect();
                match kept.is_empty() {
                    true => self.answers.remove(&question.id),
                    false => self.answers.insert(question.id, Value::Array(kept)),
                };
            }
            ("boolean", _) => {
                let next = !matches!(self.answers.get(&question.id), Some(Value::Bool(true)));
                self.answers.insert(question.id, Value::Bool(next));
            }
            ("text", _) => self.typing = Some(self.text_of(&question)),
            (kind, _) => {
                return Action::SetMessage(format!(
                    "This form draws no '{kind}' question; answer it with `ephor work gate answer`."
                ))
            }
        }
        Action::None
    }

    /// `e` on a text question: the reader's editor, on a private scratch file
    /// that starts from what is answered so far.
    fn edit(&self) -> Action {
        match self.here().map(|stop| &self.brief.questions[stop.question]) {
            Some(question) if question.kind == "text" => Action::EditGateText {
                question: question.id.clone(),
                text: self.text_of(question),
            },
            _ => Action::SetMessage("e writes a text answer in your editor.".into()),
        }
    }

    fn answer(&mut self, dry_run: bool) -> Action {
        Action::AnswerGate {
            item: self.item.clone(),
            ticket: self.ticket.clone(),
            from: self.state.clone(),
            document: self.document(),
            dry_run,
        }
    }

    /// Back where the screen was opened from (§FS-019-waiting.4).
    pub fn leave(&self) -> Action {
        match self.back {
            Return::Work => Action::OpenWork(self.item.clone()),
            Return::Review => Action::ReviewHumanGate(self.item.clone()),
            Return::Waiting | Return::Overview => Action::Back,
        }
    }

    /// The files `f` lists: the brief's attachments first, then the files the
    /// review infers, each with the name it is listed under.
    pub(super) fn listed(&self) -> Vec<(bool, Artifact)> {
        self.brief
            .attachments
            .iter()
            .map(|file| (true, artifact(file)))
            .chain(self.others.iter().map(|file| (false, file.clone())))
            .collect()
    }

    fn read_listed(&mut self, selected: usize) -> Action {
        let Some((_, file)) = self.listed().into_iter().nth(selected) else {
            return Action::None;
        };
        match human_gate::read_file(&file) {
            Ok(text) => {
                self.preview = Some((file.path.display().to_string(), text));
                self.preview_scroll = 0;
                Action::None
            }
            Err(err) => Action::SetMessage(err.to_string()),
        }
    }

    /// When the ticket entered the gate, as an age, or *unknown* where the
    /// runtime does not know — never zero (§FS-005-dispatch.9.3).
    pub(super) fn waited(&self) -> String {
        self.brief
            .since
            .as_deref()
            .and_then(|since| chrono::DateTime::parse_from_rfc3339(since).ok())
            .map(|since| {
                crate::feed::render::age(chrono::Utc::now(), since.with_timezone(&chrono::Utc))
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// A brief's file as a review file, so it opens through the same bounded,
/// local viewer (§FS-005-dispatch.9.1). The runtime's own problem stands
/// where it found one.
fn artifact(file: &BriefFile) -> Artifact {
    Artifact {
        name: file.path.clone(),
        path: file.absolute.clone(),
        problem: match file.readable {
            true => None,
            false => Some(
                file.problem
                    .clone()
                    .unwrap_or_else(|| "the runtime could not read it".to_string()),
            ),
        },
    }
}

/// A private place for a text answer while the editor has it: a directory
/// only this account can read, under the system's temporary directory, so a
/// half-written answer never lands beside the plan (§FS-005-dispatch.9.3).
pub(super) fn scratch(question: &str, text: &str) -> std::io::Result<(PathBuf, PathBuf)> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let name: String = question
        .chars()
        .map(|ch| match ch.is_ascii_alphanumeric() {
            true => ch,
            false => '-',
        })
        .collect();
    let dir = std::env::temp_dir().join(format!(
        "ephor-gate-text-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&dir)?;
    let path = dir.join(format!("{name}.md"));
    std::fs::write(&path, text)?;
    Ok((dir, path))
}

impl super::App {
    /// Open a gate where the reader asked for it: its own form where the
    /// runtime prepares one, and the review it always had where it does not —
    /// an older runtime changes nothing (§FS-005-dispatch.9.3,
    /// §FS-019-waiting.4).
    pub(super) fn open_gate(&mut self, item: Item, ticket: &str) {
        if !self.ctx.gate_briefs() {
            return self.open_human_review(item);
        }
        let home = match self.home {
            super::Home::Waiting => Return::Waiting,
            super::Home::Overview => Return::Overview,
        };
        // Where `esc` goes, and where the review goes in turn when it is the
        // review this form was sent from.
        let (back, floor) = match &self.screen {
            super::Screen::HumanGate(open) => (Return::Review, open.back),
            super::Screen::Work(_) => (Return::Work, Return::Work),
            super::Screen::Brief(open) => (open.back, open.floor),
            _ => (home, home),
        };
        let review = match self.ctx.human_review(&item) {
            Ok(review) => review,
            Err(err) => {
                self.message = err.to_string();
                return;
            }
        };
        let runner = std::path::Path::new(crate::work::runtime::runner(&self.work))
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "the runtime".to_string());
        match BriefScreen::open(item.clone(), &review, ticket, &runner) {
            Some(screen) => self.screen = super::Screen::Brief(screen.returning(back, floor)),
            // A gate the runtime could not brief stays reviewable, and says
            // why it has no form.
            None => {
                let why = review
                    .gates
                    .iter()
                    .find(|gate| gate.ticket == ticket)
                    .and_then(|gate| gate.unbriefed.clone());
                if let Some(why) = why {
                    self.message = format!("{ticket} has no brief: {why}");
                }
                let back = match back {
                    Return::Review => floor,
                    other => other,
                };
                self.screen = super::Screen::HumanGate(
                    super::HumanGateScreen::new(item, review).returning(back),
                );
            }
        }
    }

    /// Send the form's answer document to the runtime. A dry run's verdict is
    /// the confirmation; a real answer that moved the ticket goes back where
    /// the form was opened from, and a refusal stays on the form
    /// (§FS-005-dispatch.9.3).
    pub(super) fn answer_gate(
        &mut self,
        item: Item,
        ticket: &str,
        from: &str,
        document: &Value,
        dry_run: bool,
    ) {
        let outcome = self.ctx.answer_gate(&item, ticket, from, document, dry_run);
        let super::Screen::Brief(screen) = &mut self.screen else {
            return;
        };
        if dry_run {
            return screen.dry_ran(outcome);
        }
        match outcome {
            Ok(verdict) if verdict.ok => {
                let leave = screen.leave();
                self.message = format!(
                    "{ticket}: {from} → {}",
                    verdict.to.as_deref().unwrap_or("unknown")
                );
                self.reload_work();
                self.reload_operations();
                self.rebuild_view();
                match leave {
                    Action::OpenWork(item) => self.open_work(item),
                    Action::ReviewHumanGate(item) => self.open_human_review(item),
                    _ => self.back(),
                }
            }
            other => screen.refuse(other),
        }
    }

    /// A text answer written in the reader's editor, on a private scratch
    /// file removed once it is read back (§FS-005-dispatch.9.3).
    pub(super) fn edit_gate_text(
        &mut self,
        terminal: &mut ratatui::DefaultTerminal,
        question: &str,
        text: &str,
    ) -> crate::error::Result<()> {
        let (dir, path) = match scratch(question, text) {
            Ok(made) => made,
            Err(err) => {
                self.message = format!("could not open an editor on the answer: {err}");
                return Ok(());
            }
        };
        let edited = self.edit_file(terminal, &path);
        let written = std::fs::read_to_string(&path);
        let _ = std::fs::remove_dir_all(&dir);
        edited?;
        match (written, &mut self.screen) {
            (Ok(text), super::Screen::Brief(screen)) => screen.set_text(question, &text),
            (Err(err), _) => self.message = format!("could not read the answer back: {err}"),
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "brief_tests.rs"]
mod tests;
