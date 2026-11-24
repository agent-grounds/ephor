//! How a prepared gate is drawn: the file to read first, the form beneath it,
//! and the runtime's own account of what an answer will do
//! (§FS-005-dispatch.9.3).

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use serde_json::Value;

use crate::work::runtime::gate::{say_holds, Question, Verdict};

use super::answers::fit;
use super::brief::{BriefScreen, Focus, Stop};
use super::highlight_style;

/// The width the confirmation gives a question's prompt.
const PROMPT: usize = 30;

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn heading(focused: bool) -> Style {
    match focused {
        true => Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        false => Style::default().add_modifier(Modifier::BOLD),
    }
}

impl BriefScreen {
    pub fn title(&self) -> String {
        match &self.preview {
            Some((path, _)) => format!(" ephor — local file — {path}"),
            None => format!(
                " ephor — gate — {}   {} · waiting {}",
                self.brief.title,
                self.state,
                self.waited()
            ),
        }
    }

    pub fn footer(&self) -> String {
        if self.typing.is_some() {
            " type  ·  enter keeps it  ·  esc drops it  ·  ^w word back  ·  ^u clear".to_string()
        } else if self.preview.is_some() {
            " j/k scroll  PgUp/PgDn page  esc back to files · reading changes nothing".to_string()
        } else if self.files.is_some() {
            " j k select  enter read  esc back to the brief · reading changes nothing".to_string()
        } else if self.confirm.is_some() {
            " enter submit   esc back to the form · submitting starts no run · the same call as: \
             ephor work gate answer --answers -"
                .to_string()
        } else {
            format!(
                " tab file / form  j k move  space choose  e editor  f files  s submit  esc {} \
                 · reading changes nothing",
                self.back.word()
            )
        }
    }

    pub fn draw(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        if let Some((_, text)) = &self.preview {
            self.viewport = usize::from(area.height);
            frame.render_widget(
                Paragraph::new(text.as_str())
                    .wrap(Wrap { trim: false })
                    .scroll((self.preview_scroll, 0)),
                area,
            );
            return;
        }
        if let Some(selected) = self.files {
            self.draw_files(frame, area, selected);
            return;
        }
        if let Some(verdict) = &self.confirm {
            let lines = self.confirmation(verdict);
            frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
            return;
        }
        let mut facts = format!(
            "matter {} · ticket {} · visit {}",
            self.item.title, self.ticket, self.brief.visit
        );
        if let Some(holds) = self.holds {
            facts.push_str(&format!(" · {}", say_holds(holds)));
        }
        let mut top = vec![Line::from(Span::styled(facts, dim()))];
        if let Some(summary) = &self.brief.summary {
            top.push(Line::from(""));
            top.extend(summary.lines().map(|line| Line::from(line.to_string())));
        }
        for problem in &self.brief.problems {
            top.push(Line::from(Span::styled(
                format!("! {problem}"),
                Style::default().fg(Color::Yellow),
            )));
        }
        top.push(Line::from(""));
        let width = usize::from(area.width).max(1);
        let top_height = top
            .iter()
            .map(|line| line.width().max(1).div_ceil(width))
            .sum::<usize>();
        let (form, cursor_line) = self.form_lines(usize::from(area.width));
        let room = usize::from(area.height).saturating_sub(top_height + 1);
        // The file keeps a few lines whatever the form asks; past that, the
        // form scrolls to keep the cursor in sight.
        let form_height = form.len().min(room.saturating_sub(4)).max(1);
        let [top_area, file_area, others_area, form_area] = Layout::vertical([
            Constraint::Length(top_height as u16),
            Constraint::Min(2),
            Constraint::Length(1),
            Constraint::Length(form_height as u16),
        ])
        .areas(area);
        frame.render_widget(Paragraph::new(top).wrap(Wrap { trim: false }), top_area);
        self.draw_file(frame, file_area);
        frame.render_widget(Paragraph::new(self.others_line(width)), others_area);
        let offset = cursor_line.saturating_sub(form_height.saturating_sub(1));
        frame.render_widget(Paragraph::new(form).scroll((offset as u16, 0)), form_area);
    }

    fn draw_file(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let [head, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
        self.viewport = usize::from(body.height);
        let name = self
            .brief
            .display
            .as_ref()
            .map(|file| file.path.clone())
            .unwrap_or_else(|| "no file to read first".to_string());
        let text = match &self.display {
            Ok(text) => text.as_str(),
            Err(problem) => problem.as_str(),
        };
        let total = text.lines().count().max(1);
        let at = format!("line {} of {total}", (self.scroll + 1).min(total));
        let gap = usize::from(head.width).saturating_sub(name.chars().count() + at.chars().count());
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(name, heading(self.focus == Focus::File)),
                Span::raw(" ".repeat(gap.max(1))),
                Span::styled(at, dim()),
            ])),
            head,
        );
        let style = match &self.display {
            Ok(_) => Style::default(),
            Err(_) => Style::default().fg(Color::Yellow),
        };
        // Headings stand out; the rest is the file as written.
        let lines: Vec<Line> = text
            .lines()
            .skip(self.scroll)
            .take(usize::from(body.height))
            .map(|line| match line.starts_with('#') {
                true => Line::from(Span::styled(
                    line.to_string(),
                    style.add_modifier(Modifier::BOLD),
                )),
                false => Line::from(Span::styled(line.to_string(), style)),
            })
            .collect();
        frame.render_widget(Paragraph::new(lines), body);
    }

    /// `f other files (N) · name · name …`, cut to the width.
    fn others_line(&self, width: usize) -> Line<'static> {
        let listed = self.listed();
        let names: Vec<String> = listed
            .iter()
            .map(|(_, file)| {
                file.path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file.name.clone())
            })
            .collect();
        let mut line = format!("f other files ({})", listed.len());
        for name in names {
            line.push_str(" · ");
            line.push_str(&name);
        }
        Line::from(Span::styled(fit(&line, width), dim()))
    }

    /// The form's lines, and the line the cursor stands on.
    pub(super) fn form_lines(&self, width: usize) -> (Vec<Line<'static>>, usize) {
        let (answered, of) = self.count();
        let mut lines = vec![Line::from(vec![
            Span::styled("Your answer", heading(self.focus == Focus::Form)),
            Span::styled(format!("   {answered} of {of} answered"), dim()),
        ])];
        if let Some(says) = &self.says {
            lines.push(Line::from(Span::styled(
                format!("refused: {says}"),
                Style::default().fg(Color::Red),
            )));
        }
        let stops = self.stops();
        let here = stops.get(self.cursor).copied();
        let mut cursor_line = 0;
        for (index, question) in self.brief.questions.iter().enumerate() {
            let marker = match (self.answered(question), question.required) {
                (true, _) => "✓",
                (false, true) => "?",
                (false, false) => "·",
            };
            let mut head = vec![Span::raw(format!("{marker} {}", question.prompt))];
            if !self.answered(question) {
                head.push(Span::styled(
                    match question.required {
                        true => "  required",
                        false => "  optional",
                    },
                    dim(),
                ));
            }
            lines.push(Line::from(head));
            if let Some(help) = &question.help {
                lines.push(Line::from(Span::styled(format!("    {help}"), dim())));
            }
            for (stop, text) in self.choices(index, question, width) {
                if Some(stop) == here {
                    cursor_line = lines.len();
                }
                let style = match (Some(stop) == here, self.focus == Focus::Form) {
                    (true, true) => highlight_style(),
                    _ => Style::default(),
                };
                lines.push(Line::from(Span::styled(text, style)));
            }
            for refusal in self
                .refused
                .iter()
                .filter(|refusal| refusal.question.as_deref() == Some(question.id.as_str()))
            {
                lines.push(Line::from(Span::styled(
                    format!("    {}: {}", refusal.kind, refusal.message),
                    Style::default().fg(Color::Red),
                )));
            }
        }
        (lines, cursor_line)
    }

    /// The lines a question's answer is chosen on, each with its stop.
    fn choices(&self, index: usize, question: &Question, width: usize) -> Vec<(Stop, String)> {
        let at = |option| Stop {
            question: index,
            option,
        };
        let answer = self.answers.get(&question.id);
        match question.kind.as_str() {
            "choice" | "multi" if !question.options.is_empty() => question
                .options
                .iter()
                .enumerate()
                .map(|(option, choice)| {
                    let taken = match answer {
                        Some(Value::String(value)) => *value == choice.value,
                        Some(Value::Array(values)) => values
                            .iter()
                            .any(|value| value.as_str() == Some(choice.value.as_str())),
                        _ => false,
                    };
                    let mark = match (question.kind.as_str(), taken) {
                        ("choice", true) => "(•)",
                        ("choice", false) => "( )",
                        (_, true) => "[x]",
                        (_, false) => "[ ]",
                    };
                    let described = choice
                        .description
                        .as_ref()
                        .map(|description| format!(" — {description}"))
                        .unwrap_or_default();
                    (
                        at(Some(option)),
                        fit(&format!("    {mark} {}{described}", choice.shown()), width),
                    )
                })
                .collect(),
            "boolean" => {
                let (yes, no) = match answer {
                    Some(Value::Bool(true)) => ("[x]", "[ ]"),
                    Some(Value::Bool(false)) => ("[ ]", "[x]"),
                    _ => ("[ ]", "[ ]"),
                };
                vec![(at(None), format!("    {yes} yes   {no} no"))]
            }
            "text" => {
                let shown = match (&self.typing, here_is(self, index)) {
                    (Some(line), true) => format!("{line}▏"),
                    _ => match answer {
                        Some(Value::String(text)) if !text.trim().is_empty() => text.clone(),
                        _ => "type here, or e for the editor".to_string(),
                    },
                };
                vec![(at(None), fit(&format!("    {shown}"), width))]
            }
            kind => vec![(
                at(None),
                format!("    a '{kind}' question — answer it with `ephor work gate answer`"),
            )],
        }
    }

    fn draw_files(&self, frame: &mut ratatui::Frame, area: Rect, selected: usize) {
        let mut rows = Vec::new();
        let mut was_attached = None;
        for (attached, file) in self.listed() {
            let section = match attached {
                true => "attachments",
                false => "other files",
            };
            let problem = file
                .problem
                .as_ref()
                .map(|problem| format!(" · {problem}"))
                .unwrap_or_default();
            let mut lines = Vec::new();
            if was_attached != Some(attached) {
                lines.push(Line::from(Span::styled(section, heading(false))));
                was_attached = Some(attached);
            }
            lines.push(Line::from(format!("📄 {}{problem}", file.name)));
            lines.push(Line::from(Span::styled(
                format!("  {}", file.path.display()),
                dim(),
            )));
            rows.push(ListItem::new(lines));
        }
        let list = List::new(rows)
            .highlight_symbol("▸ ")
            .highlight_style(Style::default().bg(Color::DarkGray));
        let mut state = ListState::default().with_selected(Some(selected));
        frame.render_stateful_widget(list, area, &mut state);
    }

    /// Screen 4: the answers as they will be sent, and what the runtime says
    /// it will do with them (§FS-005-dispatch.9.3).
    pub(super) fn confirmation(&self, verdict: &Verdict) -> Vec<Line<'static>> {
        let mut lines = vec![
            Line::from(Span::styled("Submit this answer?", heading(false))),
            Line::from(""),
        ];
        for question in &self.brief.questions {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<PROMPT$}", fit(&question.prompt, PROMPT)),
                    dim(),
                ),
                Span::raw(format!("  {}", self.said(question))),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("What {} says it will do (dry run)", self.runner),
            heading(false),
        )));
        let unknown = || "unknown".to_string();
        let route = match &verdict.route {
            Some(route) => when(route.get("when").unwrap_or(&Value::Null)),
            None => "the destination you chose".to_string(),
        };
        for (what, says) in [
            (
                "move",
                format!(
                    "{}  {} → {}",
                    self.ticket,
                    self.state,
                    verdict.to.clone().unwrap_or_else(unknown)
                ),
            ),
            ("by route", route),
            (
                "answer recorded at",
                verdict.answer_path.clone().unwrap_or_else(unknown),
            ),
        ]
        .into_iter()
        .chain(self.holds.map(|holds| ("releases", releases(holds))))
        {
            lines.push(Line::from(vec![
                Span::styled(format!("  {what:<20}"), dim()),
                Span::raw(says),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "Ephor sends the answers and {runner} performs the move. If the ticket changed \
                 state since you opened it, {runner} refuses and nothing is written.",
                runner = self.runner
            ),
            dim(),
        )));
        lines
    }

    /// One question's answer as the reader chose it: the option's label, yes
    /// or no, the words typed, or a dash where it is unanswered.
    fn said(&self, question: &Question) -> String {
        let label = |value: &str| {
            question
                .options
                .iter()
                .find(|choice| choice.value == value)
                .map(|choice| choice.shown().to_string())
                .unwrap_or_else(|| value.to_string())
        };
        match self.answers.get(&question.id) {
            Some(Value::Bool(true)) => "yes".to_string(),
            Some(Value::Bool(false)) => "no".to_string(),
            Some(Value::String(value)) if question.kind == "text" => fit(value, 60),
            Some(Value::String(value)) => label(value),
            Some(Value::Array(values)) => values
                .iter()
                .filter_map(Value::as_str)
                .map(label)
                .collect::<Vec<_>>()
                .join(", "),
            _ => "—".to_string(),
        }
    }
}

/// What leaving the gate lets go of, on the confirmation
/// (§FS-005-dispatch.9.3).
fn releases(holds: u64) -> String {
    match holds {
        0 => "no held task".to_string(),
        1 => "1 held task; the next run picks it up".to_string(),
        held => format!("{held} held tasks; the next run picks them up"),
    }
}

/// Whether the cursor stands on question `index`.
fn here_is(screen: &BriefScreen, index: usize) -> bool {
    screen
        .stops()
        .get(screen.cursor)
        .is_some_and(|stop| stop.question == index)
}

/// A route's condition as the runtime wrote it, one `name = value` per
/// answer it names; shown, never evaluated (§DA-012-the-runtime-routes-a-gate-answer).
fn when(condition: &Value) -> String {
    match condition {
        Value::Object(fields) if fields.is_empty() => "(no condition)".to_string(),
        Value::Object(fields) => fields
            .iter()
            .map(|(name, value)| match value {
                Value::String(text) => format!("{name} = {text}"),
                other => format!("{name} = {other}"),
            })
            .collect::<Vec<_>>()
            .join(", "),
        Value::Null => "(no condition)".to_string(),
        other => other.to_string(),
    }
}
