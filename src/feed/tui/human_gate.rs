//! Review local materials before choosing a declared move (§FS-005-dispatch.9.1).

use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Constraint, Layout, Rect};
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
        } else if self.review.gates.is_empty() {
            // Nothing to decide; the finished files are only read
            // (§FS-005-dispatch.9.2).
            match self.review.files.is_empty() {
                true => " H reload  esc work".to_string(),
                false => " No human gate · j/k files  enter read  H reload  esc work".to_string(),
            }
        } else if self.transitions() == 0 {
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
        // No live gate is said, and any finished blocked files are listed
        // under it, opened through the same viewer (§FS-005-dispatch.9.2).
        let area = match self.review.gates.is_empty() {
            true => {
                let [said, rest] =
                    Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(area);
                frame.render_widget(Paragraph::new(human_gate::NO_GATE), said);
                rest
            }
            false => area,
        };
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
                finished: Vec::new(),
            },
        )
    }

    /// A review with no live gate and one finished blocked file, as
    /// §FS-005-dispatch.9.2 lists it.
    fn history(path: std::path::PathBuf) -> HumanGateScreen {
        let mut screen = screen(path.clone());
        screen.review = Review {
            gates: Vec::new(),
            files: vec![Artifact {
                name: "review.1 · finished blocked · verdict".to_string(),
                path,
                problem: None,
            }],
            finished: Vec::new(),
        };
        screen
    }

    fn drawn(screen: &mut HumanGateScreen) -> String {
        let backend = ratatui::backend::TestBackend::new(80, 12);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| screen.draw(frame, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        buffer
            .content
            .chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// With no gate the review still says so and lists the finished files
    /// under it (§FS-005-dispatch.9.2).
    #[test]
    fn a_review_without_gates_draws_its_finished_files() {
        let temp = tempfile::tempdir().unwrap();
        let mut screen = history(temp.path().join("verdict.md"));
        let drawn = drawn(&mut screen);
        assert!(drawn.contains(human_gate::NO_GATE), "{drawn}");
        assert!(drawn.contains("finished blocked · verdict"), "{drawn}");
        assert!(screen.footer().contains("enter read"));
    }

    /// A finished file opens in the bounded viewer with no gate, and going
    /// back moves nothing (§FS-005-dispatch.9.2).
    #[test]
    fn a_finished_file_opens_without_a_gate_and_returns_without_a_move() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("verdict.md");
        std::fs::write(&path, "VERDICT: blocked\n\nThe VPN was down.\n").unwrap();
        let mut screen = history(path);
        assert!(matches!(
            screen.handle_key(KeyCode::Char('j')),
            Action::None
        ));
        assert!(matches!(screen.handle_key(KeyCode::Enter), Action::None));
        assert!(screen
            .preview
            .as_ref()
            .unwrap()
            .1
            .contains("The VPN was down."));
        assert!(matches!(screen.handle_key(KeyCode::Esc), Action::None));
        assert!(screen.preview.is_none());
        assert!(matches!(
            screen.handle_key(KeyCode::Esc),
            Action::OpenWork(_)
        ));
    }

    /// Beside a finished file a live gate keeps its moves first, and the
    /// finished file after them is only read (§FS-005-dispatch.9.1,
    /// §FS-005-dispatch.9.2).
    #[test]
    fn a_mixed_review_keeps_transition_selection_for_its_live_gate() {
        let temp = tempfile::tempdir().unwrap();
        let verdict = temp.path().join("verdict.md");
        std::fs::write(&verdict, "VERDICT: blocked\n").unwrap();
        let mut screen = screen(temp.path().join("review.md"));
        screen.review.files.push(Artifact {
            name: "review.0 · finished blocked · verdict".to_string(),
            path: verdict,
            problem: None,
        });
        assert!(matches!(
            screen.handle_key(KeyCode::Enter),
            Action::TransitionHumanGate { to, .. } if to == "release"
        ));
        for _ in 0..3 {
            screen.handle_key(KeyCode::Char('j'));
        }
        assert!(matches!(screen.handle_key(KeyCode::Enter), Action::None));
        assert!(screen
            .preview
            .as_ref()
            .unwrap()
            .1
            .contains("VERDICT: blocked"));
        let drawn = {
            screen.preview = None;
            drawn(&mut screen)
        };
        assert!(!drawn.contains(human_gate::NO_GATE), "{drawn}");
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
