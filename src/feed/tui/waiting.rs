//! The Waiting screen: what the interface opens on (§FS-019-waiting).
//!
//! Presentation over one reading, [`crate::api::Session::waiting_over`], which
//! `ephor waiting` prints (§FS-019-waiting.6). The rows are built by the shell
//! off the draw path; a draw only renders what was already decided, and every
//! key here either moves the cursor or hands the shell a move another screen
//! or `ephor work process` already makes (§FS-019-waiting.4,
//! §FS-019-waiting.7).

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::Utc;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::api::views;
use crate::feed::model::Item;

use super::Action;

/// What `Enter` on a gate says where no matter on the feed is about its plan
/// (§FS-019-waiting.4).
pub(crate) const NO_REVIEW: &str =
    "No matter on the feed is about this plan, so it cannot be reviewed by matter — e reads the plan";

/// What a key on a new row says where its matter left the feed since the
/// reading.
pub(crate) const LEFT_THE_FEED: &str = "That matter has left the feed";

/// What `Enter` on a running row says where the run left no id beside its lock
/// (§FS-019-waiting.4).
pub(crate) const NO_RUN_ID: &str =
    "This run left no id beside its lock, so there is nothing to attach to";

/// One selectable row: which section, and where in it. Held matters are not
/// rows — they are offered nothing (§FS-019-waiting.1.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum At {
    OnYou(usize),
    New(usize),
    Running(usize),
}

pub(crate) struct WaitingScreen {
    view: views::Waiting,
    /// The matters the rows lead to, by feed id, resolved by the shell when
    /// the reading was taken: what `Enter` opens is the matter itself, as
    /// the overview opens it.
    items: BTreeMap<String, Item>,
    selected: usize,
    scroll: u16,
    viewport: u16,
    /// Whether a reading has landed. Before one has, the screen says it is
    /// reading rather than drawing empty sections, which would be an answer
    /// (§FS-019-waiting.2).
    read: bool,
}

/// What the screen says before its first reading lands (§FS-019-waiting.2).
pub(crate) const READING: &str = "Reading what waits on you…";

impl WaitingScreen {
    pub fn new(view: views::Waiting, items: BTreeMap<String, Item>) -> Self {
        WaitingScreen {
            view,
            items,
            selected: 0,
            scroll: 0,
            viewport: 0,
            read: true,
        }
    }

    /// The screen before any reading: what the interface draws its first
    /// frame with, so the walk of the work roots is not a blank terminal.
    pub fn pending() -> Self {
        let mut screen = WaitingScreen::new(
            views::Waiting {
                refusal: None,
                on_you: Vec::new(),
                new: Vec::new(),
                held: Vec::new(),
                running: Vec::new(),
                queued: Vec::new(),
                adopt: None,
                decided: views::Decided::default(),
                overview: views::Overview::default(),
            },
            BTreeMap::new(),
        );
        screen.read = false;
        screen
    }

    /// Whether a reading has landed yet.
    pub fn is_read(&self) -> bool {
        self.read
    }

    /// The header counts the three sections (§FS-019-waiting.3).
    pub fn title(&self) -> String {
        match self.read {
            true => format!(" {}", self.view.header()),
            false => " ephor — waiting".to_string(),
        }
    }

    /// The footer opens with the overview line, so a quiet screen is never
    /// mistaken for a quiet feed (§FS-019-waiting.3).
    pub fn footer(&self) -> String {
        if !self.read {
            return " O overview  ; operations  $ burn  q quit".to_string();
        }
        // The setting a repeated confirmation would make automatic leads,
        // and changes nothing (§FS-019-waiting.7.7).
        let adopt = self
            .view
            .adopt
            .as_deref()
            .map(|adopt| format!(" {adopt}  ·"))
            .unwrap_or_default();
        format!(
            "{adopt} {}   O overview  e plan  A process all  ; operations  $ burn  r refresh  q quit",
            self.view.overview_line()
        )
    }

    /// A fresh reading, with the cursor still on the row it was on: a
    /// rebuild fires from the tick and from every refresh landing, and a row
    /// appearing above the cursor would otherwise change what the next
    /// `Enter` acts on (§FS-005-dispatch.15.1).
    pub fn replace(&mut self, view: views::Waiting, items: BTreeMap<String, Item>) {
        let was = self.rows().get(self.selected).map(|at| self.key(*at));
        self.view = view;
        self.items = items;
        self.read = true;
        let rows = self.rows();
        self.selected = was
            .and_then(|key| rows.iter().position(|at| self.key(*at) == key))
            .unwrap_or_else(|| self.selected.min(rows.len().saturating_sub(1)));
        self.follow_selection();
    }

    /// The execution roots of the live runs this screen shows: what the tick
    /// probes to tell a run ending or starting (§FS-005-dispatch.15.1).
    pub fn live_roots(&self) -> Vec<PathBuf> {
        self.view
            .running
            .iter()
            .map(|row| row.root.clone())
            .collect()
    }

    /// Where a matter stands among the new rows, 1-based, and the matter
    /// after it: what the processing screen counts and what its `n` opens
    /// (§FS-019-waiting.7.2).
    pub fn place_of(&self, id: &str) -> super::process::Place {
        let Some(index) = self.view.new.iter().position(|row| row.id == id) else {
            return super::process::Place::default();
        };
        super::process::Place {
            at: Some((index + 1, self.view.new.len())),
            next: self
                .view
                .new
                .iter()
                .skip(index + 1)
                .find_map(|row| self.items.get(&row.id))
                .map(|item| Box::new(item.clone())),
        }
    }

    fn rows(&self) -> Vec<At> {
        (0..self.view.on_you.len())
            .map(At::OnYou)
            .chain((0..self.view.new.len()).map(At::New))
            .chain((0..self.view.running.len()).map(At::Running))
            .collect()
    }

    /// What a row *is*, for keeping the cursor on it across a rebuild.
    fn key(&self, at: At) -> String {
        match at {
            At::OnYou(index) => format!("ticket:{}", self.view.on_you[index].ticket),
            At::New(index) => format!("matter:{}", self.view.new[index].id),
            At::Running(index) => format!("root:{}", self.view.running[index].root.display()),
        }
    }

    fn selected_at(&self) -> Option<At> {
        self.rows().get(self.selected).copied()
    }

    pub fn handle_key(&mut self, code: KeyCode) -> Action {
        let count = self.rows().len();
        match code {
            KeyCode::Char('q') => Action::Quit,
            // The floor: there is nothing under Waiting to go back to
            // (§FS-019-waiting.4).
            KeyCode::Esc => Action::None,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected + 1 < count {
                    self.selected += 1;
                }
                self.follow_selection();
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.follow_selection();
                Action::None
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.selected = 0;
                self.scroll = 0;
                Action::None
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.selected = count.saturating_sub(1);
                self.follow_selection();
                Action::None
            }
            KeyCode::Char('f') | KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(self.viewport.max(1));
                Action::None
            }
            KeyCode::Char('b') | KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(self.viewport.max(1));
                Action::None
            }
            // On a new row `Enter` processes it and `l` opens the matter
            // itself; everywhere else they are the same key
            // (§FS-019-waiting.4, §FS-019-waiting.7.2).
            KeyCode::Enter => match self.selected_at() {
                Some(At::New(index)) => self.process(index, false),
                _ => self.open(),
            },
            KeyCode::Char('l') => self.open(),
            KeyCode::Char('a') => match self.selected_at() {
                Some(At::New(index)) => self.process(index, true),
                _ => Action::None,
            },
            KeyCode::Char('x') => match self.selected_at() {
                Some(At::New(index)) => match self.items.get(&self.view.new[index].id) {
                    Some(item) => Action::DismissMatter(item.clone()),
                    None => Action::SetMessage(LEFT_THE_FEED.to_string()),
                },
                _ => Action::None,
            },
            KeyCode::Char('A') => match self.view.new.is_empty() {
                true => Action::SetMessage("Nothing new to process".to_string()),
                false => Action::PreviewProcessAll,
            },
            KeyCode::Char('e') => self.plan(),
            KeyCode::Char('r') => Action::Refresh,
            _ => Action::None,
        }
    }

    /// `Enter` or `a` on a new row: its processing screen, with `a`'s list
    /// open where `choosing` (§FS-019-waiting.7.2, §FS-019-waiting.7.3).
    fn process(&self, index: usize, choosing: bool) -> Action {
        match self.items.get(&self.view.new[index].id) {
            Some(item) => Action::ProcessMatter {
                item: item.clone(),
                entry: None,
                choosing,
            },
            None => Action::SetMessage(LEFT_THE_FEED.to_string()),
        }
    }

    /// `Enter` (§FS-019-waiting.4): a gate opens its review, the screen `H`
    /// opens from the matter; a poll opens its matter's conversation, or its
    /// plan; a new matter opens as `Enter` on the overview opens it; a running
    /// row attaches.
    fn open(&self) -> Action {
        match self.selected_at() {
            Some(At::OnYou(index)) => {
                let row = &self.view.on_you[index];
                match row.kind {
                    "gate" => match row.review.as_ref().and_then(|id| self.items.get(id)) {
                        Some(item) => Action::ReviewHumanGate(item.clone()),
                        None => Action::SetMessage(NO_REVIEW.to_string()),
                    },
                    _ => match row.item.as_ref().and_then(|id| self.items.get(id)) {
                        Some(item) => Action::OpenThread {
                            item: item.clone(),
                            or_url: true,
                        },
                        None => Action::ReadPlan(row.plan.clone()),
                    },
                }
            }
            Some(At::New(index)) => match self.items.get(&self.view.new[index].id) {
                Some(item) => Action::OpenThread {
                    item: item.clone(),
                    or_url: true,
                },
                None => Action::SetMessage(LEFT_THE_FEED.to_string()),
            },
            Some(At::Running(index)) => {
                let row = &self.view.running[index];
                match &row.run {
                    Some(id) => Action::AttachRun {
                        root: row.root.clone(),
                        id: id.clone(),
                    },
                    None => Action::SetMessage(NO_RUN_ID.to_string()),
                }
            }
            None => Action::None,
        }
    }

    /// `e`: the plan behind a gate, poll or running row (§FS-019-waiting.4).
    fn plan(&self) -> Action {
        match self.selected_at() {
            Some(At::OnYou(index)) => Action::ReadPlan(self.view.on_you[index].plan.clone()),
            Some(At::Running(index)) => match &self.view.running[index].plan {
                Some(plan) => Action::ReadPlan(plan.clone()),
                None => Action::SetMessage("No plan behind this run".to_string()),
            },
            Some(At::New(_)) => {
                Action::SetMessage("Nothing has been done about this matter yet".to_string())
            }
            None => Action::None,
        }
    }

    /// Everything on the screen, phrased, with where each row's first line
    /// sits — so the scroll can follow the cursor without a second layout.
    fn layout(&self) -> (Vec<Line<'static>>, Vec<usize>) {
        let now = Utc::now();
        let dim = Style::default().fg(Color::DarkGray);
        let heading = dim.add_modifier(Modifier::BOLD);
        let view = &self.view;
        let selected = self.selected_at();
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut offsets = Vec::new();
        if !self.read {
            lines.push(Line::from(Span::styled(format!("  {READING}"), dim)));
            return (lines, offsets);
        }
        let cursor = |at: At| if selected == Some(at) { "▸" } else { " " };
        let strong = |at: At| {
            if selected == Some(at) {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            }
        };
        let section = |lines: &mut Vec<Line<'static>>, name: &str, count: usize, keys: &str| {
            lines.push(Line::from(vec![
                Span::styled(format!("  {name} · {count}"), heading),
                Span::styled(format!("   {keys}"), dim),
            ]));
        };
        // The refusal stands in for the two sections the runtime would have
        // filled (§FS-019-waiting.2).
        let refused = view
            .refusal
            .as_deref()
            .filter(|_| view.on_you.is_empty() && view.running.is_empty());

        // The answer, what it is the answer to, and what the reader decided
        // today (§FS-019-waiting.2).
        if view.quiet() {
            lines.push(Line::from(Span::styled(
                format!("  {}", view.quiet_answer()),
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::styled(
                format!("  {}", view.decided_line()),
                dim,
            )));
            lines.push(Line::from(""));
        }

        section(
            &mut lines,
            "WAITING ON YOU",
            view.on_you.len(),
            "enter open",
        );
        match refused {
            Some(refusal) => lines.push(Line::from(Span::styled(format!("    {refusal}"), dim))),
            None if view.on_you.is_empty() => {
                lines.push(Line::from(Span::styled("    nothing".to_string(), dim)))
            }
            None => {}
        }
        for (index, row) in view.on_you.iter().enumerate() {
            let at = At::OnYou(index);
            offsets.push(lines.len());
            let (mark, style) = if row.movable {
                (
                    "✋",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )
            } else {
                ("⏳", Style::default().fg(Color::Yellow))
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", cursor(at)), dim),
                Span::styled(format!("{mark} "), style),
                Span::styled(row.title.clone(), strong(at)),
                Span::styled(format!("   {}", row.project), dim),
                Span::raw(format!("   {}", row.says())),
                Span::styled(format!("   {}", row.ticket), dim),
            ]));
            let about = match (&row.matter, &row.item) {
                (Some(matter), _) => matter.clone(),
                (None, Some(item)) => item.clone(),
                (None, None) => "no matter on the feed".to_string(),
            };
            lines.push(Line::from(Span::styled(format!("       {about}"), dim)));
        }
        lines.push(Line::from(""));

        section(
            &mut lines,
            "NEW",
            view.new.len(),
            "enter process  l open  a another  x needs none  A all",
        );
        if view.new.is_empty() {
            lines.push(Line::from(Span::styled("    nothing".to_string(), dim)));
        }
        for (index, row) in view.new.iter().enumerate() {
            let at = At::New(index);
            offsets.push(lines.len());
            let unread = if row.unread { "●" } else { " " };
            // The age leads, as on the overview's rows, so a long title is
            // what the edge of the screen cuts rather than the age.
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", cursor(at)), dim),
                Span::styled(format!("{unread} "), Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{:<5} ", crate::feed::render::age(now, row.updated_at)),
                    dim,
                ),
                Span::styled(format!("{:<6} ", row.kind.label()), dim),
                Span::styled(format!("{:<10} ", row.project), dim),
                Span::styled(row.title.clone(), strong(at)),
                // The rhei it would be assigned (§FS-019-waiting.7.1).
                Span::styled(
                    format!("   {}", row.proposal.says()),
                    proposal(&row.proposal),
                ),
            ]));
        }
        // Held matters once, apart, offered nothing (§FS-019-waiting.1.2).
        if !view.held.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("    {}", held_line(&view.held)),
                dim,
            )));
        }
        lines.push(Line::from(""));

        section(&mut lines, "RUNNING", view.running.len(), "enter attach");
        match refused {
            Some(refusal) => lines.push(Line::from(Span::styled(format!("    {refusal}"), dim))),
            None if view.running.is_empty() => {
                lines.push(Line::from(Span::styled("    nothing".to_string(), dim)))
            }
            None => {}
        }
        for (index, row) in view.running.iter().enumerate() {
            let at = At::Running(index);
            offsets.push(lines.len());
            let about = row
                .matter
                .clone()
                .or_else(|| row.item.clone())
                .unwrap_or_else(|| super::display_root(&row.root.to_string_lossy()));
            let holding: Vec<String> = row
                .tickets
                .iter()
                .map(|ticket| {
                    format!(
                        "{} [{}]",
                        ticket.ticket,
                        ticket.state.as_deref().unwrap_or("?")
                    )
                })
                .collect();
            // The age the run published, and none where it published none
            // (§FS-019-waiting.1.3).
            let age = row
                .started_at
                .map(|at| crate::feed::render::age(now, at))
                .unwrap_or_default();
            // A run between two turns holds nothing, and says so rather than
            // reading as a run doing nothing (§FS-019-waiting.1.3).
            let holding = match holding.is_empty() {
                true => "between turns".to_string(),
                false => holding.join(", "),
            };
            let work: Vec<&str> = [&row.entry, &row.branch]
                .into_iter()
                .flatten()
                .map(String::as_str)
                .collect();
            let work = match work.is_empty() {
                true => String::new(),
                false => format!("   {}", work.join("  ")),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", cursor(at)), dim),
                Span::styled("▶ ".to_string(), Style::default().fg(Color::Green)),
                Span::styled(format!("{age:<5} "), dim),
                Span::styled(format!("{:<10} ", row.project), dim),
                Span::styled(about, strong(at)),
                Span::styled(work, Style::default().fg(Color::Cyan)),
                Span::styled(format!("   {holding}"), dim),
            ]));
        }
        // What processing laid that waits for a slot (§FS-019-waiting.7.6).
        if let Some(queued) = view.queued_line() {
            lines.push(Line::from(Span::styled(
                format!("    {queued}"),
                Style::default().fg(Color::Yellow),
            )));
        }
        (lines, offsets)
    }

    /// Bring the selected row into the viewport.
    fn follow_selection(&mut self) {
        if self.viewport == 0 {
            return;
        }
        let (_, offsets) = self.layout();
        let Some(top) = offsets.get(self.selected).map(|top| *top as u16) else {
            return;
        };
        // A gate is two lines; every other row one.
        let height = match self.selected_at() {
            Some(At::OnYou(_)) => 2,
            _ => 1,
        };
        if top < self.scroll {
            self.scroll = top;
        } else if top + height > self.scroll.saturating_add(self.viewport) {
            self.scroll = (top + height).saturating_sub(self.viewport);
        }
    }

    pub fn draw(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        self.viewport = area.height;
        let (lines, _) = self.layout();
        let max_scroll = lines.len().saturating_sub(area.height as usize);
        self.scroll = self.scroll.min(max_scroll as u16);
        frame.render_widget(Paragraph::new(lines).scroll((self.scroll, 0)), area);
    }

    /// The screen as text, for tests.
    #[cfg(test)]
    pub fn text(&self) -> String {
        let (lines, _) = self.layout();
        lines
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

/// A proposal ready to process is drawn as quietly as the row's other facts;
/// one that is not says so in colour (§FS-019-waiting.7.1).
fn proposal(proposal: &views::Proposal) -> Style {
    match proposal.unambiguous {
        true => Style::default().fg(Color::Cyan),
        false => Style::default().fg(Color::Yellow),
    }
}

/// The held matters on one line: by title while there are few, and counted by
/// the label that holds them once there are more than a line has room for —
/// `ephor waiting` lists each (§FS-019-waiting.1.2, §FS-019-waiting.6).
fn held_line(held: &[views::HeldMatter]) -> String {
    let count = held.len();
    let noun = if count == 1 {
        "matter is"
    } else {
        "matters are"
    };
    let which = if count <= 3 {
        held.iter()
            .map(|row| format!("{} ({})", row.title, row.label))
            .collect::<Vec<_>>()
            .join(" · ")
    } else {
        let mut by: BTreeMap<&str, usize> = BTreeMap::new();
        for row in held {
            *by.entry(row.label.as_str()).or_default() += 1;
        }
        by.iter()
            .map(|(label, count)| format!("{count} by {label}"))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    format!("{count} {noun} held, and offered nothing: {which}")
}

#[cfg(test)]
#[path = "waiting_tests.rs"]
mod tests;
