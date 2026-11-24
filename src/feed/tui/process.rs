//! The processing screen (§FS-019-waiting.7.2): one new matter, the rhei it
//! would be assigned and why that one, and the account a dry run of the same
//! move gives — or, after `A`, every unambiguous row's account and the rows
//! left for the reader (§FS-019-waiting.7.5).
//!
//! Presentation only. The proposal, the choices `a` lists and the accounts
//! are the session's ([`crate::api::Session::process`]), read by the shell
//! when the screen opens; every key here either moves a cursor or hands the
//! shell a move `ephor work process` or `ephor work dismiss` makes.

use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::api::views;
use crate::feed::model::Item;

use super::Action;

pub(crate) use crate::api::process::Choice;

/// Where one matter stands among the new rows, and the one after it: what
/// the title counts and what `n` opens (§FS-019-waiting.7.2).
#[derive(Default)]
pub(crate) struct Place {
    /// 1-based, and how many new rows there are, where the matter is one.
    pub at: Option<(usize, usize)>,
    pub next: Option<Box<Item>>,
}

/// What the screen is over.
pub(crate) enum Over {
    /// One matter, the entry proposed or chosen, and a dry run's account.
    One {
        item: Box<Item>,
        place: Place,
        proposal: views::Proposal,
        account: views::ProcessRow,
        /// A workflow with inputs nobody answered: `Enter` opens them rather
        /// than laying it with a guess (§FS-019-waiting.7.2).
        wants: bool,
        /// What `a` lists: every entry that would hand the matter over, then
        /// every workflow no entry names.
        choices: Vec<Choice>,
    },
    /// `A`'s preview: what processing every unambiguous row would lay, and
    /// what it leaves.
    All(views::Processing),
}

pub(crate) struct ProcessScreen {
    pub over: Over,
    /// `a`'s list, open over the screen, and the row under the cursor.
    choosing: Option<usize>,
    scroll: u16,
}

impl ProcessScreen {
    pub fn one(
        item: Item,
        place: Place,
        proposal: views::Proposal,
        account: views::ProcessRow,
        wants: bool,
        choices: Vec<Choice>,
    ) -> Self {
        ProcessScreen {
            over: Over::One {
                item: Box::new(item),
                place,
                proposal,
                account,
                wants,
                choices,
            },
            choosing: None,
            scroll: 0,
        }
    }

    pub fn all(view: views::Processing) -> Self {
        ProcessScreen {
            over: Over::All(view),
            choosing: None,
            scroll: 0,
        }
    }

    /// Open `a`'s list at once — what `a` on the Waiting row does
    /// (§FS-019-waiting.7.3).
    pub fn choosing(mut self) -> Self {
        if let Over::One { choices, .. } = &self.over {
            if !choices.is_empty() {
                self.choosing = Some(0);
            }
        }
        self
    }

    pub fn title(&self) -> String {
        match &self.over {
            // The matter, and where it stands among the new rows
            // (§FS-019-waiting.7.2).
            Over::One { item, place, .. } => {
                let short = crate::api::waiting::short_id(&item.id);
                match place.at {
                    Some((at, of)) => format!(" ephor — new matter — {short} · {at} of {of} new"),
                    None => format!(" ephor — new matter — {short}"),
                }
            }
            Over::All(view) => format!(
                " ephor — process {} new {}",
                view.processed.len(),
                match view.processed.len() {
                    1 => "matter",
                    _ => "matters",
                }
            ),
        }
    }

    pub fn footer(&self) -> String {
        if self.choosing.is_some() {
            return " j/k move  ·  enter processes with it  ·  esc back".to_string();
        }
        match &self.over {
            Over::One { wants, place, .. } => {
                let enter = match wants {
                    true => "enter answers its inputs",
                    false => "enter lay and start",
                };
                let next = match place.next {
                    Some(_) => "  ·  n next",
                    None => "",
                };
                format!(
                    " {enter}  ·  a other workflow  ·  x needs no work{next}  ·  l open  ·  esc waiting"
                )
            }
            Over::All(view) if view.processed.is_empty() => " esc back".to_string(),
            Over::All(_) => " enter processes them  ·  esc back".to_string(),
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> Action {
        if let (Some(selected), Over::One { item, choices, .. }) = (self.choosing, &self.over) {
            return match code {
                KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => {
                    self.choosing = None;
                    Action::None
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.choosing = Some((selected + 1).min(choices.len().saturating_sub(1)));
                    Action::None
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.choosing = Some(selected.saturating_sub(1));
                    Action::None
                }
                KeyCode::Enter => match choices.get(selected) {
                    Some(choice) => Action::ProcessMatter {
                        item: (**item).clone(),
                        entry: Some(choice.key.clone()),
                        choosing: false,
                    },
                    None => Action::None,
                },
                KeyCode::Char('q') => Action::Quit,
                _ => Action::None,
            };
        }
        match code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => Action::Back,
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll = self.scroll.saturating_add(1);
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
                Action::None
            }
            KeyCode::Enter => match &self.over {
                Over::One {
                    item,
                    proposal,
                    account,
                    wants,
                    ..
                } => {
                    // Nothing is laid that the dry run refused, except for
                    // the inputs the reader is about to answer.
                    if !account.ok && !wants {
                        return Action::SetMessage(account.says.clone());
                    }
                    Action::ConfirmProcess {
                        item: (**item).clone(),
                        entry: proposal.entry.clone(),
                        wants: *wants,
                    }
                }
                Over::All(view) if view.processed.is_empty() => {
                    Action::SetMessage("No new matter has an unambiguous proposal".to_string())
                }
                Over::All(_) => Action::ConfirmProcessAll,
            },
            KeyCode::Char('a') => match &self.over {
                Over::One { choices, .. } if choices.is_empty() => {
                    Action::SetMessage("No other entry hands this matter over".to_string())
                }
                Over::One { .. } => {
                    self.choosing = Some(0);
                    Action::None
                }
                Over::All(_) => Action::None,
            },
            KeyCode::Char('x') => match &self.over {
                Over::One { item, .. } => Action::DismissMatter((**item).clone()),
                Over::All(_) => Action::None,
            },
            KeyCode::Char('l') => match &self.over {
                Over::One { item, .. } => Action::OpenThread {
                    item: (**item).clone(),
                    or_url: true,
                },
                Over::All(_) => Action::None,
            },
            // The next new row's screen, this one left undecided
            // (§FS-019-waiting.7.2).
            KeyCode::Char('n') => match &self.over {
                Over::One {
                    place:
                        Place {
                            next: Some(next), ..
                        },
                    ..
                } => Action::ProcessMatter {
                    item: (**next).clone(),
                    entry: None,
                    choosing: false,
                },
                Over::One { .. } => Action::SetMessage(LAST.to_string()),
                Over::All(_) => Action::None,
            },
            _ => Action::None,
        }
    }

    fn layout(&self) -> Vec<Line<'static>> {
        let dim = Style::default().fg(Color::DarkGray);
        let heading = dim.add_modifier(Modifier::BOLD);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line<'static>> = Vec::new();
        match &self.over {
            Over::One {
                item,
                proposal,
                account,
                wants,
                choices,
                ..
            } => {
                // The matter, who holds it, and that no rhei has it yet
                // (§FS-019-waiting.7.2).
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", item.kind.label()), dim),
                    Span::styled(item.title.clone(), bold),
                    Span::styled(format!("   {} · {}", item.project, item.id), dim),
                ]));
                let mut moved = format!(
                    "moved {} ago",
                    crate::feed::render::age(chrono::Utc::now(), item.updated_at)
                );
                if !account.labels.is_empty() {
                    moved.push_str(&format!(" · labels {}", account.labels.join(", ")));
                }
                lines.push(Line::from(Span::styled(format!("  {moved}"), dim)));
                match &account.assignees {
                    Some(held) if held.is_empty() => {
                        lines.push(field("held by", "nobody · no assignee on the forge"))
                    }
                    Some(held) => lines.push(field("held by", &held.join(", "))),
                    None => {}
                }
                lines.push(field("work", "none laid · no plan in the ledger"));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled("  PROPOSED".to_string(), heading)));
                lines.push(Line::from(vec![
                    Span::raw(format!("    {} ", proposal.icon)),
                    Span::styled(proposal.entry.clone(), bold),
                    Span::styled(format!("   {}", proposal.kind), dim),
                    Span::raw(format!("   {}", proposal.description)),
                ]));
                lines.push(Line::from(Span::styled(
                    format!("    {}", why(proposal)),
                    dim,
                )));
                if let Some(hand) = &proposal.hand {
                    lines.push(field("hand", hand));
                }
                if let Some(plan) = &account.plan {
                    lines.push(field(
                        "plan",
                        &format!("{} · written on confirm", plan.display()),
                    ));
                }
                // One line for the checkout: its branch, where it is, and
                // whether it is there or will be created.
                if proposal.branch.is_some() || proposal.workspace.is_some() {
                    let there = match proposal.gate {
                        "needs-checkout" => "will be created",
                        _ => "exists",
                    };
                    let mut checkout = proposal.branch.clone().unwrap_or_default();
                    if let Some(workspace) = &proposal.workspace {
                        if !checkout.is_empty() {
                            checkout.push_str(" at ");
                        }
                        checkout.push_str(&workspace.display().to_string());
                    }
                    lines.push(field("checkout", &format!("{checkout} · {there}")));
                }
                if let Some(root) = &proposal.root {
                    lines.push(field("work root", &root.display().to_string()));
                }
                if let Some(refusal) = &proposal.refusal {
                    lines.push(Line::from(Span::styled(
                        format!("    {refusal}"),
                        Style::default().fg(Color::Yellow),
                    )));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "  WHAT IT WOULD DO".to_string(),
                    heading,
                )));
                let tone = match (account.ok, wants) {
                    (true, _) => Style::default(),
                    (false, true) => Style::default().fg(Color::Yellow),
                    (false, false) => Style::default().fg(Color::Red),
                };
                for line in account.says.lines() {
                    lines.push(Line::from(Span::styled(format!("    {line}"), tone)));
                }
                for ticket in &account.tickets {
                    lines.push(field("ticket", ticket));
                }
                if !account.answers.is_empty() {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled("  INPUTS".to_string(), heading)));
                }
                for answer in &account.answers {
                    let shown = match answer.shown.is_empty() {
                        true => format!("({})", answer.from),
                        false => format!("{}  ({})", answer.shown, answer.from),
                    };
                    lines.push(field(&answer.input, &shown));
                }
                // What confirming does, in one sentence (§FS-019-waiting.7.2).
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!(
                        "  Confirming lays the plan, records it against {} in the ledger, and \
                         asks for its start. Ephor posts nothing to the forge.",
                        crate::api::waiting::short_id(&item.id)
                    ),
                    dim,
                )));
                if let Some(selected) = self.choosing {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  CHOOSE ANOTHER".to_string(),
                        heading,
                    )));
                    for (index, choice) in choices.iter().enumerate() {
                        let cursor = if index == selected { "▸" } else { " " };
                        let style = match index == selected {
                            true => bold,
                            false => Style::default(),
                        };
                        let mut spans = vec![
                            Span::styled(format!("  {cursor} "), dim),
                            Span::styled(choice.key.clone(), style),
                            Span::styled(format!("   {}", choice.kind), dim),
                            Span::raw(format!("   {}", choice.description)),
                        ];
                        if let Some(refusal) = &choice.refusal {
                            spans.push(Span::styled(format!("   {refusal}"), dim));
                        }
                        lines.push(Line::from(spans));
                    }
                }
            }
            Over::All(view) => {
                lines.push(Line::from(Span::styled(
                    format!("  WOULD PROCESS · {}", view.processed.len()),
                    heading,
                )));
                if view.processed.is_empty() {
                    lines.push(Line::from(Span::styled("    nothing".to_string(), dim)));
                }
                for row in &view.processed {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(row.title.clone(), bold),
                        Span::styled(format!("   {}", row.item), dim),
                    ]));
                    lines.push(Line::from(Span::styled(
                        format!("      → {} · {}", row.entry, first_line(&row.says)),
                        dim,
                    )));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    format!("  LEFT FOR YOU · {}", view.left.len()),
                    heading,
                )));
                if view.left.is_empty() {
                    lines.push(Line::from(Span::styled("    nothing".to_string(), dim)));
                }
                for row in &view.left {
                    lines.push(Line::from(vec![
                        Span::raw(format!("    {}", row.title)),
                        Span::styled(format!("   {}", row.item), dim),
                    ]));
                    lines.push(Line::from(Span::styled(
                        format!("      {}", first_line(&row.says)),
                        dim,
                    )));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "  Nothing is written until enter. The admission decides how many start; \
                     the rest are laid and queued."
                        .to_string(),
                    dim,
                )));
            }
        }
        lines
    }

    pub fn draw(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let lines = self.layout();
        let max_scroll = lines.len().saturating_sub(area.height as usize);
        self.scroll = self.scroll.min(max_scroll as u16);
        frame.render_widget(Paragraph::new(lines).scroll((self.scroll, 0)), area);
    }

    /// The screen as text, for tests.
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.layout()
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Why this is the one proposed (§FS-019-waiting.7.2).
fn why(proposal: &views::Proposal) -> String {
    if proposal.fallback {
        return "no entry hands this matter over, so the runtime's triage is proposed".to_string();
    }
    match proposal.others {
        0 => "the only entry that hands this matter over".to_string(),
        1 => "the first entry in menu order · 1 other would take it · a to choose".to_string(),
        others => {
            format!("the first entry in menu order · {others} others would take it · a to choose")
        }
    }
}

/// What `n` says on the last new row (§FS-019-waiting.7.2).
pub(crate) const LAST: &str = "This is the last new matter";

fn field(name: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("    {name:<10} "),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(value.to_string()),
    ])
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_string()
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
