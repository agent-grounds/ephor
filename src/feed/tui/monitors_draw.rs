//! Drawing the Monitors page and one monitor's detail view
//! (§FS-021-monitors.18, §FS-021-monitors.19).
//!
//! Presentation only (§AR-009-surfaces.4): every word is the reading's or
//! the model's, said as `ephor monitors` says it, and what is decided here
//! is where it goes, its colour, and what a narrow screen leaves out.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::api::monitor_views::{self as views, clip, when, HEALTH};
use crate::monitors::model::Health;
use crate::monitors::page::Rank;
use crate::monitors::strip::Way;

use super::monitors::MonitorsScreen;
use super::monitors_chart::{threshold_style, Plot};

/// What the cursor's line and the threshold lines' legend are headed by.
const CURSOR: &str = "cursor: ";
const BOUNDS: &str = "bounds:";

/// The colour a rank is drawn in: down red, failing and warn amber, ok
/// green, and unknown and a KPI dim (§FS-021-monitors.3.7).
pub(super) fn rank_style(rank: Rank) -> Style {
    match rank {
        Rank::Down => Style::default().fg(Color::Red),
        Rank::Failing | Rank::Warn => Style::default().fg(Color::Yellow),
        Rank::Ok => Style::default().fg(Color::Green),
        Rank::Unknown | Rank::Kpi => Style::default().fg(Color::DarkGray),
    }
}

/// The colour a health is drawn in, a cell's or a point's, as its rank's.
pub(super) fn health_style(health: Option<Health>) -> Style {
    match health {
        Some(Health::Down) => Style::default().fg(Color::Red),
        Some(Health::Warn) => Style::default().fg(Color::Yellow),
        Some(Health::Ok) => Style::default().fg(Color::Green),
        Some(Health::Unknown) => Style::default().fg(Color::DarkGray),
        None => Style::default(),
    }
}

/// The change, green the better way and amber the worse
/// (§FS-021-monitors.18.6).
pub(super) fn way_style(way: Way) -> Style {
    match way {
        Way::Better => Style::default().fg(Color::Green),
        Way::Worse => Style::default().fg(Color::Yellow),
        Way::Plain => Style::default(),
    }
}

pub(super) fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// `text` wrapped to `width` at its words, each line `indent` spaces in, so
/// a line too long for the screen is said in full rather than cut off.
pub(super) fn wrapped(text: &str, indent: usize, width: usize) -> Vec<String> {
    let room = width.saturating_sub(indent).max(1);
    super::thread::wrap_line(text, room)
        .into_iter()
        .map(|line| format!("{}{line}", " ".repeat(indent)))
        .collect()
}

pub(super) fn pad(text: &str, width: usize) -> String {
    let text = clip(text, width);
    let short = width.saturating_sub(text.chars().count());
    format!("{text}{}", " ".repeat(short))
}

impl MonitorsScreen {
    pub fn draw(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        match self.detail().is_some() {
            true => self.draw_detail(frame, area),
            false => self.draw_page(frame, area),
        }
    }
}

/// Where a band of the detail view's rows lands once the view is scrolled:
/// the row of the area it starts on, how many of its rows show, and how
/// many of them are scrolled off above.
fn band(start: usize, len: usize, scroll: usize, height: usize) -> Option<(u16, u16, u16)> {
    let first = start.max(scroll);
    let end = (start + len).min(scroll + height);
    if first >= end {
        return None;
    }
    let to = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
    Some((to(first - scroll), to(end - first), to(first - start)))
}

/// What the detail view lays out once per reading of the monitor rather
/// than once per frame: the chart's geometry, and the episodes, readings
/// and points as last wrapped, with the width they were wrapped to.
pub(super) struct Drawn {
    pub plot: Plot,
    pub(super) sections: Option<(usize, Vec<Line<'static>>)>,
}

impl Drawn {
    pub(super) fn of(detail: &views::MonitorDetail) -> Drawn {
        Drawn {
            plot: Plot::of(detail),
            sections: None,
        }
    }
}

impl MonitorsScreen {
    /// The detail view (§FS-021-monitors.19): what the monitor is and where
    /// it is read, the chart with the strip of health beneath it and the
    /// cursor's point, then the episodes, the readings and the points. The
    /// whole scrolls as one; the chart keeps its height while it is in
    /// view. Only the rows on screen are drawn.
    fn draw_detail(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let width = usize::from(area.width);
        let height = usize::from(area.height);
        self.detail_viewport = area.height;
        if self.drawn.is_none() {
            self.drawn = self.detail().map(Drawn::of);
        }
        let laid = self
            .drawn
            .as_ref()
            .and_then(|drawn| drawn.sections.as_ref())
            .is_some_and(|(at, _)| *at == width);
        if !laid {
            let lines = self.detail().map(|detail| sections(detail, width));
            if let (Some(drawn), Some(lines)) = (self.drawn.as_mut(), lines) {
                drawn.sections = Some((width, lines));
            }
        }
        let chart = chart_rows(height);
        let (Some(detail), Some(drawn)) = (self.detail(), self.drawn.as_ref()) else {
            return;
        };
        let top = self.head_lines(detail, width);
        let graph = drawn
            .plot
            .graph(Rect::new(0, 0, area.width, to_rows(chart)));
        let tail = self.tail_lines(detail, &drawn.plot, graph, width);
        let sections = drawn.sections.as_ref().map_or(0, |(_, lines)| lines.len());
        let bottom = tail.len() + sections;
        let total = top.len() + chart + bottom;
        let scroll = usize::from(self.detail_scroll).min(total.saturating_sub(height));
        self.detail_scroll = u16::try_from(scroll).unwrap_or(u16::MAX);
        let (Some(detail), Some(drawn)) = (self.detail(), self.drawn.as_ref()) else {
            return;
        };
        let at = |y: u16, rows: u16| Rect {
            x: area.x,
            y: area.y + y,
            width: area.width,
            height: rows,
        };
        let start = top.len();
        if let Some((y, rows, skip)) = band(0, start, scroll, height) {
            frame.render_widget(Paragraph::new(top).scroll((skip, 0)), at(y, rows));
        }
        if let Some((y, rows, _)) = band(start, chart, scroll, height) {
            if rows >= 3 {
                drawn.plot.draw(frame, at(y, rows), detail, self.cursor);
            }
        }
        if let Some((y, rows, skip)) = band(start + chart, bottom, scroll, height) {
            let laid = drawn.sections.as_ref().map_or(&[][..], |(_, lines)| lines);
            let shown: Vec<Line<'static>> = tail
                .iter()
                .chain(laid)
                .skip(usize::from(skip))
                .take(usize::from(rows))
                .cloned()
                .collect();
            frame.render_widget(Paragraph::new(shown), at(y, rows));
        }
    }

    /// Under the chart: the strip of health, its columns under the plot's,
    /// the change, the point the cursor stands on, and the threshold lines'
    /// legend (§FS-021-monitors.19, §FS-021-monitors.18.6).
    fn tail_lines(
        &self,
        detail: &views::MonitorDetail,
        plot: &Plot,
        graph: Rect,
        width: usize,
    ) -> Vec<Line<'static>> {
        let gutter = usize::from(graph.x);
        let mut strip = vec![Span::styled(pad(HEALTH, gutter), dim())];
        strip.extend(plot.strip(detail, usize::from(graph.width)));
        let mut lines = vec![Line::from(strip)];
        lines.extend(
            wrapped(&detail.change_says(), gutter, width)
                .into_iter()
                .map(|line| Line::styled(line, way_style(detail.row.shows.change.way))),
        );
        if let Some(point) = detail.chart.get(self.cursor) {
            let says = detail.cursor_says(point, when);
            let style = health_style(point.health).add_modifier(Modifier::BOLD);
            match CURSOR.len() + says.chars().count() <= width {
                true => lines.push(Line::from(vec![
                    Span::styled(CURSOR, dim()),
                    Span::styled(says, style),
                ])),
                false => lines.extend(
                    wrapped(&format!("{CURSOR}{says}"), 0, width)
                        .into_iter()
                        .map(|line| Line::styled(line, style)),
                ),
            }
        }
        if !detail.thresholds.is_empty() && detail.steps.is_empty() {
            let legend = |line: &views::Threshold| format!("  ── {}", line.says);
            let wide: usize = BOUNDS.len()
                + detail
                    .thresholds
                    .iter()
                    .map(|line| legend(line).chars().count())
                    .sum::<usize>();
            let mut spans = vec![Span::styled(BOUNDS, dim())];
            for line in &detail.thresholds {
                let span = Span::styled(legend(line), threshold_style(line.of));
                match wide <= width {
                    true => spans.push(span),
                    false => {
                        lines.push(Line::from(std::mem::take(&mut spans)));
                        spans.push(span);
                    }
                }
            }
            lines.push(Line::from(spans));
        }
        lines
    }
}

/// The episodes, the readings and the points the repository supplied, each
/// said in full, wrapped to `width` (§FS-021-monitors.19).
fn sections(detail: &views::MonitorDetail, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut section = |name: &str, said: Vec<String>| {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            format!("{name} ({})", said.len()),
            Style::default().add_modifier(Modifier::BOLD),
        ));
        for said in said {
            lines.extend(wrapped(&said, 2, width).into_iter().map(Line::raw));
        }
    };
    let episodes = detail
        .episodes
        .iter()
        .map(|episode| episode.says(when))
        .collect();
    section("episodes", episodes);
    let readings = detail
        .readings
        .iter()
        .map(|reading| reading.says(when))
        .collect();
    section("readings", readings);
    if !detail.points.is_empty() {
        let points = detail
            .points
            .iter()
            .map(|point| detail.point_says(point, when))
            .collect();
        section("points", points);
    }
    lines
}

fn to_rows(rows: usize) -> u16 {
    u16::try_from(rows).unwrap_or(u16::MAX)
}

impl MonitorsScreen {
    /// What heads the detail view: what the title bar could not hold, the
    /// health as the health column says it, coloured by its rank, then the
    /// monitor's facts and its matter line (§FS-021-monitors.19), each
    /// wrapped to `width` so nothing is cut off the edge. The matter line
    /// comes here, beside what `p` acts on, rather than under every reading
    /// of the window.
    fn head_lines(&self, detail: &views::MonitorDetail, width: usize) -> Vec<Line<'static>> {
        let mut lines: Vec<Line<'static>> = self
            .header_lines(width)
            .into_iter()
            .map(|line| Line::styled(line, dim()))
            .collect();
        let health = rank_style(detail.row.shows.rank).add_modifier(Modifier::BOLD);
        lines.extend(
            wrapped(&detail.health_says(when), 0, width)
                .into_iter()
                .map(|line| Line::styled(line, health)),
        );
        let facts = detail
            .facts_say(when)
            .into_iter()
            .chain([format!("matter: {}", detail.matter_says())]);
        for fact in facts {
            lines.extend(wrapped(&fact, 0, width).into_iter().map(Line::raw));
        }
        lines.push(Line::raw(""));
        lines
    }
}

/// The chart's height: about a third of the view, never under 6 rows or
/// over 12.
fn chart_rows(height: usize) -> usize {
    (height / 3).clamp(6, 12)
}
