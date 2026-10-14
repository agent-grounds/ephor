//! Review local materials before choosing a declared move (§FS-005-dispatch.9.1).

use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};

use crate::feed::model::Item;
use crate::work::runtime::human_gate::{self, Review};

use super::Action;

pub(crate) struct HumanGateScreen {
    pub item: Item,
    review: Review,
    selected: usize,
    preview: Option<(String, String)>,
    scroll: u16,
    viewport: u16,
}

impl HumanGateScreen {
    pub fn new(item: Item, review: Review) -> Self {
        Self {
            item,
            review,
            selected: 0,
            preview: None,
            scroll: 0,
            viewport: 0,
        }
    }

    fn transitions(&self) -> usize {
        self.review
            .gates
            .iter()
            .map(|gate| gate.destinations.len())
            .sum()
    }

    fn count(&self) -> usize {
        self.transitions() + self.review.files.len()
    }

    pub fn title(&self) -> String {
        match &self.preview {
            Some((path, _)) => format!(" ephor — local file — {path}"),
            None => format!(" ephor — human review — {}", self.item.title),
        }
    }

    pub fn footer(&self) -> String {
        if self.preview.is_some() {
            " j/k scroll  PgUp/PgDn page  esc back to files · reading changes nothing".to_string()
        } else if !self.review.gates.is_empty() && self.transitions() == 0 {
            " No declared transitions · j/k files  enter read  H reload  esc work".to_string()
        } else {
            " j/k select  f files  enter review choice/read file  H reload  esc work".to_string()
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> Action {
        if self.preview.is_some() {
            match code {
                KeyCode::Esc => {
                    self.preview = None;
                    self.scroll = 0;
                }
                KeyCode::Char('j') | KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
                KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
                KeyCode::PageDown => self.scroll = self.scroll.saturating_add(self.viewport.max(1)),
                KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(self.viewport.max(1)),
                _ => {}
            }
            return Action::None;
        }
        match code {
            KeyCode::Esc => Action::OpenWork(self.item.clone()),
            KeyCode::Char('H') => Action::ReviewHumanGate(self.item.clone()),
            KeyCode::Char('j') | KeyCode::Down => {
                self.selected = (self.selected + 1).min(self.count().saturating_sub(1));
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Char('f') => {
                if !self.review.files.is_empty() {
                    self.selected = self.transitions();
                }
                Action::None
            }
            KeyCode::Enter => self.choose(),
            _ => Action::None,
        }
    }

    fn choose(&mut self) -> Action {
        let mut index = self.selected;
        for gate in &self.review.gates {
            if let Some(destination) = gate.destinations.get(index) {
                return Action::TransitionHumanGate {
                    item: self.item.clone(),
                    ticket: gate.ticket.clone(),
                    from: gate.state.clone(),
                    to: destination.state.clone(),
                };
            }
            index = index.saturating_sub(gate.destinations.len());
        }
        if let Some(file) = self.review.files.get(index) {
            match human_gate::read_file(file) {
                Ok(contents) => {
                    self.preview = Some((file.path.display().to_string(), contents));
                    self.scroll = 0;
                }
                Err(err) => return Action::SetMessage(err.to_string()),
            }
        }
        Action::None
    }

    pub fn draw(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        self.viewport = area.height;
        if let Some((_, contents)) = &self.preview {
            let paragraph = Paragraph::new(contents.as_str())
                .wrap(Wrap { trim: false })
                .scroll((self.scroll, 0));
            frame.render_widget(paragraph, area);
            return;
        }
        if self.review.gates.is_empty() {
            frame.render_widget(
                Paragraph::new("No task is currently waiting at a human gate."),
                area,
            );
            return;
        }
        let mut rows = Vec::new();
        for gate in &self.review.gates {
            for destination in &gate.destinations {
                let condition = destination
                    .condition
                    .as_ref()
                    .map(|condition| format!(" · {condition}"))
                    .unwrap_or_default();
                rows.push(ListItem::new(vec![
                    Line::from(format!("{} · {}", gate.ticket, gate.title)),
                    Line::from(vec![Span::styled(
                        format!("  {} → {}{condition}", gate.state, destination.state),
                        Style::default().fg(Color::Yellow),
                    )]),
                    Line::from(destination.description.clone().unwrap_or_default()),
                ]));
            }
        }
        for file in &self.review.files {
            let availability = file
                .problem
                .as_ref()
                .map(|problem| format!(" · {problem}"))
                .unwrap_or_default();
            rows.push(ListItem::new(vec![
                Line::from(format!("📄 {}{availability}", file.name)),
                Line::from(Span::styled(
                    format!("  {}", file.path.display()),
                    Style::default().fg(Color::DarkGray),
                )),
            ]));
        }
        let list = List::new(rows)
            .highlight_symbol("▸ ")
            .highlight_style(Style::default().bg(Color::DarkGray));
        let mut state = ListState::default().with_selected(Some(self.selected));
        frame.render_stateful_widget(list, area, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::runtime::human_gate::{Artifact, Destination, Gate};

    fn screen(path: std::path::PathBuf) -> HumanGateScreen {
        let item = Item {
            id: "tasks:review.1".to_string(),
            project: "demo".to_string(),
            source: "tasks".to_string(),
            kind: crate::feed::model::ItemKind::Task,
            role: None,
            title: "Review proposal".to_string(),
            url: None,
            state: Some("owner-choice".to_string()),
            needs_response: true,
            updated_at: chrono::Utc::now(),
            raw: serde_json::json!({}),
        };
        let gate = Gate {
            ticket: "review.1".to_string(),
            title: "Review proposal".to_string(),
            state: "owner-choice".to_string(),
            root: path.parent().unwrap().to_path_buf(),
            plan: path.clone(),
            machine: None,
            destinations: vec![
                Destination {
                    state: "release".to_string(),
                    condition: None,
                    description: None,
                },
                Destination {
                    state: "revise".to_string(),
                    condition: None,
                    description: None,
                },
            ],
        };
        HumanGateScreen::new(
            item,
            Review {
                gates: vec![gate],
                files: vec![Artifact {
                    name: "proposal".to_string(),
                    path,
                    problem: None,
                }],
            },
        )
    }

    #[test]
    fn human_gate_enter_selects_a_declared_move_for_confirmation() {
        let temp = tempfile::tempdir().unwrap();
        let mut screen = screen(temp.path().join("review.md"));
        screen.handle_key(KeyCode::Char('j'));
        match screen.handle_key(KeyCode::Enter) {
            Action::TransitionHumanGate {
                ticket, from, to, ..
            } => {
                assert_eq!(ticket, "review.1");
                assert_eq!(from, "owner-choice");
                assert_eq!(to, "revise");
            }
            _ => panic!("a selected edge asks the shell to confirm it"),
        }
        assert!(matches!(
            screen.handle_key(KeyCode::Esc),
            Action::OpenWork(_)
        ));
    }

    #[test]
    fn human_gate_files_open_in_place_and_return_without_a_move() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("review.md");
        std::fs::write(&path, "Local rationale\nMore evidence\n").unwrap();
        let mut screen = screen(path.clone());
        assert!(matches!(
            screen.handle_key(KeyCode::Char('f')),
            Action::None
        ));
        assert!(matches!(screen.handle_key(KeyCode::Enter), Action::None));
        assert!(screen
            .preview
            .as_ref()
            .unwrap()
            .1
            .contains("Local rationale"));
        screen.handle_key(KeyCode::Char('j'));
        assert_eq!(screen.scroll, 1);
        assert!(matches!(screen.handle_key(KeyCode::Esc), Action::None));
        assert!(screen.preview.is_none());
        assert_eq!(screen.selected, 2);
        std::fs::remove_file(path).unwrap();
        assert!(matches!(
            screen.handle_key(KeyCode::Enter),
            Action::SetMessage(_)
        ));
        assert!(screen.preview.is_none());
    }
}
