//! `R` on the feed screen: refresh one source, or one capability of it
//! (§FS-011-command-line.9.2).
//!
//! The picker has two levels. The first lists the sources the projects in view
//! are answered from — each one's own bindings, then the site's. The second
//! offers the chosen source's whole answer, and, where the command line would
//! take `--only` for it, each capability it declared. What it starts is the
//! request `ephor refresh --source NAME [--only CAP]` makes, run beneath the
//! screen like `r` (§FS-001-forge-interface.7, §REQ-002-parity.1); a choice the
//! command line refuses is refused here with the same reason.

use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

use crate::feed::cache::ProjectFeed;
use crate::feed::config::StatusConfig;
use crate::feed::refresh::Narrowed;
use crate::forge::FETCHES;

use super::highlight_style;

/// One source the reader can ask alone, and what it can be asked one
/// capability at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    name: String,
    /// The capabilities `--only` would take for it, in fetch order: none for
    /// a native provider, a task store, or a source any project in view
    /// reaches through the site's binding.
    capabilities: Vec<&'static str>,
}

pub(crate) enum PickerOutcome {
    Stay,
    Close,
    Refresh(Narrowed),
}

pub(crate) struct RefreshPicker {
    sources: Vec<Source>,
    selected: usize,
    /// The second level, open over the selected source: 0 is its whole
    /// answer, then each capability.
    answer: Option<usize>,
}

impl RefreshPicker {
    /// The sources the projects `over` are answered from, with the
    /// capabilities each one's cache says it declared. A source no refresh has
    /// recorded a declaration for offers every fetch capability, and the
    /// refresh refuses the one it does not declare, before any data call.
    /// None where nothing in view binds a source.
    pub fn over(config: &StatusConfig, feeds: &[ProjectFeed], over: &[String]) -> Option<Self> {
        let site = crate::feed::refresh::site_source_names(config);
        let mut names: Vec<String> = Vec::new();
        for project in over {
            let Some(project_config) = config.projects.get(project) else {
                continue;
            };
            for name in crate::feed::age::bound(project_config) {
                if !names.iter().any(|known| known == name) {
                    names.push(name.to_string());
                }
            }
        }
        for name in &site {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        let sources: Vec<Source> = names
            .into_iter()
            .map(|name| {
                let capabilities = capabilities(config, feeds, over, &site, &name);
                Source { name, capabilities }
            })
            .collect();
        (!sources.is_empty()).then_some(RefreshPicker {
            sources,
            selected: 0,
            answer: None,
        })
    }

    pub fn handle_key(&mut self, code: KeyCode) -> PickerOutcome {
        let rows = match self.answer {
            None => self.sources.len(),
            Some(_) => 1 + self.sources[self.selected].capabilities.len(),
        };
        let cursor = match &mut self.answer {
            None => &mut self.selected,
            Some(answer) => answer,
        };
        match code {
            KeyCode::Esc | KeyCode::Char('q') => PickerOutcome::Close,
            KeyCode::Char('j') | KeyCode::Down => {
                *cursor = (*cursor + 1).min(rows.saturating_sub(1));
                PickerOutcome::Stay
            }
            KeyCode::Char('k') | KeyCode::Up => {
                *cursor = cursor.saturating_sub(1);
                PickerOutcome::Stay
            }
            KeyCode::Char('h') | KeyCode::Left if self.answer.is_some() => {
                self.answer = None;
                PickerOutcome::Stay
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => match self.answer {
                None => {
                    self.answer = Some(0);
                    PickerOutcome::Stay
                }
                Some(answer) => {
                    let source = &self.sources[self.selected];
                    PickerOutcome::Refresh(Narrowed {
                        source: source.name.clone(),
                        only: answer
                            .checked_sub(1)
                            .and_then(|index| source.capabilities.get(index).copied()),
                    })
                }
            },
            _ => PickerOutcome::Stay,
        }
    }

    pub fn footer(&self) -> String {
        match self.answer {
            None => " j/k move  ·  enter choose a source  ·  esc close".to_string(),
            Some(_) => " j/k move  ·  enter refresh  ·  h back  ·  esc close".to_string(),
        }
    }

    /// Centered overlay over the given area.
    pub fn draw(&self, frame: &mut ratatui::Frame, area: Rect) {
        let (title, rows, selected): (String, Vec<String>, usize) = match self.answer {
            None => (
                " Refresh one source ".to_string(),
                self.sources
                    .iter()
                    .map(|source| source.name.clone())
                    .collect(),
                self.selected,
            ),
            Some(answer) => {
                let source = &self.sources[self.selected];
                let mut rows = vec![format!("{}  whole answer", source.name)];
                rows.extend(
                    source
                        .capabilities
                        .iter()
                        .map(|capability| format!("{}  {capability} only", source.name)),
                );
                (format!(" Refresh {} ", source.name), rows, answer)
            }
        };
        let width = area.width.saturating_sub(4).clamp(20, 48);
        let height = (rows.len() as u16 + 2).min(area.height);
        let rect = Rect {
            x: area.x + (area.width.saturating_sub(width)) / 2,
            y: area.y + (area.height.saturating_sub(height)) / 2,
            width,
            height,
        };
        frame.render_widget(Clear, rect);
        let items: Vec<ListItem> = rows
            .into_iter()
            .map(|row| ListItem::new(Line::from(Span::raw(format!(" {row}")))))
            .collect();
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .highlight_style(highlight_style());
        let mut state = ListState::default();
        state.select(Some(selected));
        frame.render_stateful_widget(list, rect, &mut state);
    }
}

/// What `--only` would take for `name` over the projects in view: nothing
/// where the command line refuses it outright — a native provider, a task
/// store, or a site binding any project in view is reached through — and
/// otherwise the capabilities the cache recorded the source declaring, or every
/// fetch capability where no refresh has recorded one.
fn capabilities(
    config: &StatusConfig,
    feeds: &[ProjectFeed],
    over: &[String],
    site: &[String],
    name: &str,
) -> Vec<&'static str> {
    if crate::feed::providers::built_in(name) || crate::seams::tasks::Kind::parse(name).is_some() {
        return Vec::new();
    }
    let reached_through_site = site.iter().any(|source| source == name)
        && over.iter().any(|project| {
            config.projects.get(project).is_some_and(|project_config| {
                !crate::feed::age::bound(project_config).contains(&name)
            })
        });
    if reached_through_site {
        return Vec::new();
    }
    let declared: Vec<&'static str> = FETCHES
        .into_iter()
        .filter(|fetch| {
            feeds
                .iter()
                .filter(|feed| over.contains(&feed.project))
                .filter_map(|feed| feed.providers.get(name))
                .flat_map(|slot| slot.required_histories())
                .any(|history| history.capability.as_deref() == Some(*fetch))
        })
        .collect();
    match declared.is_empty() {
        true => FETCHES.to_vec(),
        false => declared,
    }
}

#[cfg(test)]
#[path = "refresh_picker_tests.rs"]
mod tests;
