//! Drawing the Monitors page (§FS-021-monitors.18): each project's heading
//! and rows, worst first, under what the title bar could not hold.
//!
//! Presentation only (§AR-009-surfaces.4): every word is the model's, said
//! as `ephor monitors` says it. What is decided here is where it goes and
//! its colour. A screen too narrow for a row on one line gives it a second
//! rather than leaving a column out, so the page says on any width what the
//! command prints (§FS-021-monitors.18.4, §REQ-002-parity.2).

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::api::monitor_views::{self as views, when, MonitorRow, FOOTNOTE, NO_MONITORS, TITLE};
use crate::monitors::window::Window;

use super::highlight_style;
use super::monitors::MonitorsScreen;
use super::monitors_draw::{dim, health_style, pad, rank_style, way_style, wrapped};

/// The strip's width on the page: its 24 cells (§FS-021-monitors.18.5).
const STRIP: usize = crate::monitors::strip::CELLS;

/// How far a row's second line stands in: under its title.
const UNDER: usize = 4;

/// The narrowest the condition is given beside the strip on a row's second
/// line before it goes under them instead.
const NARROWEST: usize = 20;

/// How a row is laid across the page (§FS-021-monitors.18.4). Every row
/// of the page takes the same shape, so the columns stay in line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shape {
    /// Every column on one line.
    One,
    /// The condition under the rest of the row.
    Under,
    /// The glyph, title, value, cadence, health and matter on the first
    /// line; the strip, the change and the condition on the second.
    Split,
}

/// What fits across the page: each column's width and the row's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Columns {
    pub title: usize,
    value: usize,
    every: usize,
    condition: usize,
    change: usize,
    health: usize,
    matter: usize,
    pub shape: Shape,
}

impl Columns {
    pub(super) fn fit(page: &views::Monitors, width: usize) -> Columns {
        let widest = |say: &dyn Fn(&MonitorRow) -> usize, heading: usize| {
            page.rows().map(say).chain([heading]).max().unwrap_or(0)
        };
        let count = |text: &str| text.chars().count();
        let title = page
            .projects
            .iter()
            .map(|group| count(&group.project))
            .chain(page.rows().map(|row| count(&row.title)))
            .max()
            .unwrap_or(0)
            .min(TITLE);
        let strip = page
            .projects
            .iter()
            .map(|group| count(&group.strip_heading(page.window)))
            .max()
            .unwrap_or(0);
        let mut columns = Columns {
            title,
            value: widest(&|row| count(&row.shows.value), 5),
            every: widest(&|row| count(&row.shows.every), 5),
            condition: widest(&|row| count(&row.condition_noted_says(when)), 9),
            change: widest(&|row| count(&row.shows.change.says), 6),
            health: widest(&|row| count(&row.shows.health.text()), 6),
            matter: widest(&|row| count(row.shows.matter), 6),
            shape: Shape::One,
        };
        debug_assert!(strip <= STRIP, "the strip's heading fits over its cells");
        // The glyph and its space, then two spaces after each column but the last.
        let first = 2 + columns.title + 2 + columns.value + 2 + columns.every;
        let last = 2 + columns.health + 2 + columns.matter;
        let strip_change = 2 + STRIP + 2 + columns.change;
        columns.shape = match () {
            _ if first + 2 + columns.condition + strip_change + last <= width => Shape::One,
            _ if first + strip_change + last <= width => Shape::Under,
            _ => Shape::Split,
        };
        columns
    }
}

impl MonitorsScreen {
    pub(super) fn draw_page(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        self.viewport = area.height;
        let (lines, at) = self.page_lines(usize::from(area.width));
        let height = usize::from(area.height.max(1));
        let mut scroll = usize::from(self.scroll);
        // Both of the cursor's row's lines stay on screen.
        if let Some((from, to)) = at {
            if from < scroll {
                scroll = from;
            } else if to >= scroll + height {
                scroll = (to + 1).saturating_sub(height).min(from);
            }
        }
        scroll = scroll.min(lines.len().saturating_sub(height));
        self.scroll = u16::try_from(scroll).unwrap_or(u16::MAX);
        frame.render_widget(Paragraph::new(lines).scroll((self.scroll, 0)), area);
    }

    /// The page's lines, and the first and last of them the cursor's row
    /// takes (§FS-021-monitors.18.3, §FS-021-monitors.18.4).
    pub(super) fn page_lines(&self, width: usize) -> (Vec<Line<'static>>, Option<(usize, usize)>) {
        let page = self.page();
        let mut lines: Vec<Line<'static>> = self
            .header_lines(width)
            .into_iter()
            .map(|line| Line::styled(line, dim()))
            .collect();
        let say = |lines: &mut Vec<Line<'static>>, text: &str, indent: usize, style: Style| {
            lines.extend(
                wrapped(text, indent, width)
                    .into_iter()
                    .map(|line| Line::styled(line, style)),
            );
        };
        if page.projects.is_empty() {
            lines.push(Line::raw(""));
            say(&mut lines, NO_MONITORS, 1, Style::default());
        }
        let columns = Columns::fit(page, width);
        let mut at = None;
        let mut index = 0;
        for group in &page.projects {
            lines.push(Line::raw(""));
            lines.extend(heading(group, &columns, page.window));
            for row in &group.rows {
                let mut drawn = row_lines(row, &columns, width);
                if index == self.selected() {
                    drawn = drawn
                        .into_iter()
                        .map(|line| line.style(highlight_style()))
                        .collect();
                    at = Some((lines.len(), lines.len() + drawn.len() - 1));
                }
                lines.extend(drawn);
                index += 1;
            }
            for note in group.notes_say() {
                say(&mut lines, &note, 2, Style::default().fg(Color::Yellow));
            }
        }
        for without in page.without_say() {
            lines.push(Line::raw(""));
            say(&mut lines, &without, 2, dim());
        }
        if !page.is_empty() {
            lines.push(Line::raw(""));
            say(&mut lines, FOOTNOTE, 2, dim());
        }
        (lines, at)
    }
}

/// A project's heading: its name, then the columns' names, dim, laid as its
/// rows are (§FS-021-monitors.18.3).
fn heading(group: &views::MonitorProject, columns: &Columns, window: Window) -> Vec<Line<'static>> {
    let name = Span::styled(
        format!("  {}", pad(&group.project, columns.title)),
        Style::default().add_modifier(Modifier::BOLD),
    );
    if group.rows.is_empty() {
        return vec![Line::from(name)];
    }
    let strip = group.strip_heading(window);
    let named = |names: Vec<String>| names.join("  ");
    let value = [pad("value", columns.value), pad("every", columns.every)];
    let tail = [pad("health", columns.health), "matter".to_string()];
    let middle = [pad(&strip, STRIP), pad("change", columns.change)];
    let first = |names: Vec<String>| {
        Line::from(vec![
            name.clone(),
            Span::styled(format!("  {}", named(names)), dim()),
        ])
    };
    let under =
        |names: Vec<String>| Line::styled(format!("{}{}", " ".repeat(UNDER), named(names)), dim());
    match columns.shape {
        Shape::One => {
            let mut names = value.to_vec();
            names.push(pad("condition", columns.condition));
            names.extend(middle);
            names.extend(tail);
            vec![first(names)]
        }
        Shape::Under => {
            let names = value.into_iter().chain(middle).chain(tail).collect();
            vec![first(names), under(vec!["condition".to_string()])]
        }
        Shape::Split => {
            let names = value.into_iter().chain(tail).collect();
            let below = middle
                .into_iter()
                .chain(["condition".to_string()])
                .collect();
            vec![first(names), under(below)]
        }
    }
}

/// One monitor's row (§FS-021-monitors.18.4): the glyph and the health in
/// the rank's colour, the strip cell by cell in each cell's health, the
/// change by `better`, and the condition with its notes in full, wrapped
/// where it is longer than the room left for it.
fn row_lines(row: &MonitorRow, columns: &Columns, width: usize) -> Vec<Line<'static>> {
    let shows = &row.shows;
    let rank = rank_style(shows.rank);
    let mut first = vec![
        Span::styled(shows.glyph.to_string(), rank),
        Span::raw(format!(" {}  ", pad(&row.title, columns.title))),
        Span::raw(format!("{}  ", pad(&shows.value, columns.value))),
        Span::styled(format!("{}  ", pad(&shows.every, columns.every)), dim()),
    ];
    let mut strip: Vec<Span<'static>> = shows
        .strip
        .iter()
        .map(|cell| Span::styled(cell.glyph().to_string(), health_style(cell.health)))
        .collect();
    strip.push(Span::raw("  "));
    strip.push(Span::styled(
        format!("{}  ", pad(&shows.change.says, columns.change)),
        way_style(shows.change.way),
    ));
    let last = [
        Span::styled(
            format!("{}  ", pad(&shows.health.text(), columns.health)),
            rank,
        ),
        Span::raw(shows.matter.to_string()),
    ];
    let condition = row.condition_noted_says(when);
    let said = |text: &str, indent: usize| -> Vec<Line<'static>> {
        wrapped(text, indent, width)
            .into_iter()
            .map(|line| Line::styled(line, dim()))
            .collect()
    };
    match columns.shape {
        Shape::One => {
            first.push(Span::styled(
                format!("{}  ", pad(&condition, columns.condition)),
                dim(),
            ));
            first.extend(strip);
            first.extend(last);
            vec![Line::from(first)]
        }
        Shape::Under => {
            first.extend(strip);
            first.extend(last);
            let mut lines = vec![Line::from(first)];
            lines.extend(said(&condition, UNDER));
            lines
        }
        Shape::Split => {
            first.extend(last);
            let mut lines = vec![Line::from(first)];
            let mut second = vec![Span::raw(" ".repeat(UNDER))];
            second.extend(strip);
            let beside = UNDER + STRIP + 2 + columns.change + 2;
            if width >= beside + NARROWEST {
                let mut wrapped = said(&condition, beside).into_iter();
                if let Some(head) = wrapped.next() {
                    second.extend(
                        head.spans
                            .into_iter()
                            .map(|span| Span::styled(span.content.trim_start().to_string(), dim())),
                    );
                }
                lines.push(Line::from(second));
                lines.extend(wrapped);
            } else {
                lines.push(Line::from(second));
                lines.extend(said(&condition, UNDER));
            }
            lines
        }
    }
}
