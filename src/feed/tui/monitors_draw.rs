//! Drawing the Monitors page and one monitor's detail view
//! (§FS-021-monitors.18, §FS-021-monitors.19).
//!
//! Presentation only (§AR-009-surfaces.4): every word is the reading's or
//! the model's, said as `ephor monitors` says it, and what is decided here
//! is where it goes, its colour, and what a narrow screen leaves out.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Paragraph};

use crate::api::monitor_views::{self as views, clip, when};
use crate::monitors::model::Health;
use crate::monitors::page::Rank;
use crate::monitors::strip::{Cell, Way};

use super::monitors::MonitorsScreen;

/// The strip's width on the page: its 24 cells (§FS-021-monitors.18.5).
const STRIP: usize = crate::monitors::strip::CELLS;

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

impl MonitorsScreen {
    /// The detail view (§FS-021-monitors.19): what the monitor is and where
    /// it is read, the chart with the cursor's point and the strip of health
    /// beneath it, then the episodes, the readings and the points. The whole
    /// scrolls as one; the chart keeps its height while it is in view.
    fn draw_detail(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let Some(detail) = self.detail().cloned() else {
            return;
        };
        let width = usize::from(area.width);
        let height = usize::from(area.height);
        self.detail_viewport = area.height;
        let top = self.head_lines(&detail, width);
        let chart = chart_rows(height);
        let bottom = self.tail_lines(&detail, width);
        let total = top.len() + chart + bottom.len();
        let scroll = usize::from(self.detail_scroll).min(total.saturating_sub(height));
        self.detail_scroll = u16::try_from(scroll).unwrap_or(u16::MAX);
        let at = |y: u16, rows: u16| Rect {
            x: area.x,
            y: area.y + y,
            width: area.width,
            height: rows,
        };
        if let Some((y, rows, skip)) = band(0, top.len(), scroll, height) {
            frame.render_widget(Paragraph::new(top).scroll((skip, 0)), at(y, rows));
        }
        let start = total - bottom.len() - chart;
        if let Some((y, rows, _)) = band(start, chart, scroll, height) {
            if rows >= 3 {
                self.draw_chart(frame, at(y, rows), &detail);
            }
        }
        if let Some((y, rows, skip)) = band(start + chart, bottom.len(), scroll, height) {
            frame.render_widget(Paragraph::new(bottom).scroll((skip, 0)), at(y, rows));
        }
    }

    /// Under the chart: the point the cursor stands on, the strip of health,
    /// the threshold lines' legend, then the episodes, the readings and the
    /// points the repository supplied (§FS-021-monitors.19).
    fn tail_lines(&self, detail: &views::MonitorDetail, width: usize) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        if let Some(point) = detail.chart.get(self.cursor) {
            lines.push(Line::from(vec![
                Span::styled("cursor: ", dim()),
                Span::styled(
                    detail.cursor_says(point, when),
                    health_style(point.health).add_modifier(Modifier::BOLD),
                ),
            ]));
        }
        lines.push(health_strip(detail, width));
        if !detail.thresholds.is_empty() && detail.steps.is_empty() {
            let mut spans = vec![Span::styled("bounds:", dim())];
            for line in &detail.thresholds {
                spans.push(Span::styled(
                    format!("  ── {}", line.says),
                    threshold_style(line.of),
                ));
            }
            lines.push(Line::from(spans));
        }
        let section = |lines: &mut Vec<Line<'static>>, name: &str, said: Vec<String>| {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                format!("{name} ({})", said.len()),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            lines.extend(said.into_iter().map(|said| Line::raw(format!("  {said}"))));
        };
        let episodes = detail
            .episodes
            .iter()
            .map(|episode| episode.says(when))
            .collect();
        section(&mut lines, "episodes", episodes);
        let readings = detail
            .readings
            .iter()
            .map(|reading| reading.says(when))
            .collect();
        section(&mut lines, "readings", readings);
        if !detail.points.is_empty() {
            let points = detail
                .points
                .iter()
                .map(|point| detail.point_says(point, when))
                .collect();
            section(&mut lines, "points", points);
        }
        lines
    }
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

/// `health:` and the row's 24 cells drawn by health, each stretched to share
/// the width, then the change (§FS-021-monitors.19, §FS-021-monitors.18.6).
fn health_strip(detail: &views::MonitorDetail, width: usize) -> Line<'static> {
    let shows = &detail.row.shows;
    let change = format!("  change {}", shows.change.says);
    let label = "health: ";
    let room = width.saturating_sub(label.len() + change.chars().count());
    let each = (room / STRIP.max(1)).max(1);
    let mut spans = vec![Span::styled(label, dim())];
    spans.extend(shows.strip.iter().map(|cell: &Cell| {
        Span::styled(
            cell.health_glyph().to_string().repeat(each),
            health_style(cell.health),
        )
    }));
    spans.push(Span::styled(change, way_style(shows.change.way)));
    Line::from(spans)
}

/// A threshold line's colour: today's `healthy` bound red, where crossing
/// it is a drop, and `warn` amber (§FS-021-monitors.3.7).
fn threshold_style(of: &str) -> Style {
    match of {
        "healthy" => Style::default().fg(Color::Red),
        _ => Style::default().fg(Color::Yellow),
    }
}

/// A stretch of the chart's line: the points it passes through, and the
/// health it is drawn in.
pub(super) type Trace = (Vec<(f64, f64)>, Option<Health>);

/// The worse of two healths, for the stretch of line between two readings.
fn worse(a: Option<Health>, b: Option<Health>) -> Option<Health> {
    let rank = |health: Option<Health>| match health {
        Some(Health::Down) => 0,
        Some(Health::Unknown) => 1,
        Some(Health::Warn) => 2,
        Some(Health::Ok) => 3,
        None => 4,
    };
    if rank(a) <= rank(b) {
        a
    } else {
        b
    }
}

/// The chart's geometry, from the detail's points: where each lies on x
/// and y, and what the axes say. Presentation only: the values are the
/// model's, and a boolean's or a level's places are its steps.
pub(super) struct Plot {
    /// Each chart point's (x, y), none where it holds no value.
    pub points: Vec<Option<(f64, f64)>>,
    pub x: [f64; 2],
    pub y: [f64; 2],
    pub x_labels: Vec<String>,
    pub y_labels: Vec<String>,
    /// A boolean or a level: drawn as steps, its y labels its places.
    pub stepped: bool,
}

impl Plot {
    pub(super) fn of(detail: &views::MonitorDetail) -> Plot {
        let span = &detail.row.shows.span;
        let timed = detail.windowed && detail.chart.iter().all(|point| point.t.is_some());
        let place = |at: usize, point: &views::ChartPoint| match (timed, point.t) {
            (true, Some(t)) => (t - span.from).num_seconds() as f64,
            _ => at as f64,
        };
        let points: Vec<Option<(f64, f64)>> = detail
            .chart
            .iter()
            .enumerate()
            .map(|(at, point)| point.value.map(|value| (place(at, point), value)))
            .collect();
        let x = match timed {
            true => {
                let mut x = [0.0, (span.to - span.from).num_seconds() as f64];
                for (at, point) in detail.chart.iter().enumerate() {
                    let here = place(at, point);
                    x = [x[0].min(here), x[1].max(here)];
                }
                x
            }
            false => [0.0, detail.chart.len().saturating_sub(1).max(1) as f64],
        };
        let x_labels = match timed {
            true => {
                let at = |offset: f64| when(span.from + chrono::TimeDelta::seconds(offset as i64));
                vec![at(x[0]), at((x[0] + x[1]) / 2.0), at(x[1])]
            }
            false => {
                let label = |at: usize| {
                    detail
                        .chart
                        .get(at)
                        .and_then(|point| point.x.clone())
                        .unwrap_or_else(|| at.to_string())
                };
                let last = detail.chart.len().saturating_sub(1);
                vec![label(0), label(last / 2), label(last)]
            }
        };
        let stepped = detail.steps.len() >= 2;
        let (y, y_labels) = match stepped {
            true => ([0.0, (detail.steps.len() - 1) as f64], detail.steps.clone()),
            false => {
                let values = points
                    .iter()
                    .flatten()
                    .map(|(_, y)| *y)
                    .chain(detail.thresholds.iter().map(|line| line.at));
                let (lo, hi) = values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v), hi.max(v))
                });
                let (lo, hi) = match lo.is_finite() {
                    true => (lo, hi),
                    false => (0.0, 1.0),
                };
                let pad = match hi - lo {
                    spread if spread > 0.0 => spread * 0.1,
                    _ => lo.abs().max(1.0) * 0.1,
                };
                let (lo, hi) = (lo - pad, hi + pad);
                let say = |v: f64| detail.row.say(v);
                ([lo, hi], vec![say(lo), say((lo + hi) / 2.0), say(hi)])
            }
        };
        Plot {
            points,
            x,
            y,
            x_labels,
            y_labels,
            stepped,
        }
    }

    /// The line between each two readings in turn, as the points it passes
    /// through and the health it is drawn in; a boolean or a level holds its
    /// value until the next reading, then steps (§FS-021-monitors.19).
    pub(super) fn segments(&self, detail: &views::MonitorDetail) -> Vec<Trace> {
        let mut segments = Vec::new();
        for (at, pair) in self.points.windows(2).enumerate() {
            let (Some(from), Some(to)) = (pair[0], pair[1]) else {
                continue;
            };
            let health = worse(detail.chart[at].health, detail.chart[at + 1].health);
            let through = match self.stepped {
                true => vec![from, (to.0, from.1), to],
                false => vec![from, to],
            };
            segments.push((through, health));
        }
        segments
    }

    /// The points with no reading beside them, which no line reaches.
    pub(super) fn alone(&self) -> Vec<(usize, (f64, f64))> {
        let has = |at: Option<usize>| {
            at.and_then(|at| self.points.get(at))
                .is_some_and(Option::is_some)
        };
        self.points
            .iter()
            .enumerate()
            .filter_map(|(at, point)| point.map(|point| (at, point)))
            .filter(|(at, _)| !has(at.checked_sub(1)) && !has(Some(at + 1)))
            .collect()
    }
}

impl MonitorsScreen {
    /// The chart (§FS-021-monitors.19): labelled axes, today's `healthy`
    /// and `warn` as threshold lines, the readings coloured by their
    /// health, and the cursor as a dim rule through the point it is on.
    ///
    /// A boolean or a level is drawn stepped, holding each reading until the
    /// next: a line drawn straight between `no` and `yes` would show values
    /// nobody read, while a step shows when it flipped and how long it held,
    /// and its y labels name the states, so it reads without colour.
    fn draw_chart(&self, frame: &mut ratatui::Frame, area: Rect, detail: &views::MonitorDetail) {
        let plot = Plot::of(detail);
        let segments = plot.segments(detail);
        let alone: Vec<Trace> = plot
            .alone()
            .into_iter()
            .map(|(at, point)| (vec![point], detail.chart[at].health))
            .collect();
        let thresholds: Vec<(Vec<(f64, f64)>, &str)> = match plot.stepped {
            true => Vec::new(),
            false => detail
                .thresholds
                .iter()
                .map(|line| (vec![(plot.x[0], line.at), (plot.x[1], line.at)], line.of))
                .collect(),
        };
        let cursor = plot.points.get(self.cursor).copied().flatten();
        let rule: Vec<(f64, f64)> = cursor
            .map(|(x, _)| vec![(x, plot.y[0]), (x, plot.y[1])])
            .unwrap_or_default();
        let mark: Vec<(f64, f64)> = cursor.into_iter().collect();
        let mut datasets = vec![Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(dim())
            .data(&rule)];
        for (data, of) in &thresholds {
            datasets.push(
                Dataset::default()
                    .marker(Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(threshold_style(of))
                    .data(data),
            );
        }
        for (data, health) in &segments {
            datasets.push(
                Dataset::default()
                    .marker(Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(health_style(*health))
                    .data(data),
            );
        }
        for (data, health) in &alone {
            datasets.push(
                Dataset::default()
                    .marker(Marker::Dot)
                    .graph_type(GraphType::Scatter)
                    .style(health_style(*health))
                    .data(data),
            );
        }
        datasets.push(
            Dataset::default()
                .marker(Marker::Block)
                .graph_type(GraphType::Scatter)
                .style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .data(&mark),
        );
        let chart = Chart::new(datasets)
            .legend_position(None)
            .x_axis(
                Axis::default()
                    .style(dim())
                    .bounds(plot.x)
                    .labels(plot.x_labels.iter().map(|label| Span::raw(label.clone()))),
            )
            .y_axis(
                Axis::default()
                    .style(dim())
                    .bounds(plot.y)
                    .labels(plot.y_labels.iter().map(|label| Span::raw(label.clone()))),
            );
        frame.render_widget(chart, area);
    }
}
