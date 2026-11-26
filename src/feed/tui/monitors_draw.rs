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

use crate::api::monitor_views::{
    self as views, clip, when, MonitorRow, FOOTNOTE, NO_MONITORS, TITLE,
};
use crate::monitors::model::Health;
use crate::monitors::page::Rank;
use crate::monitors::strip::{Cell, Way};

use super::highlight_style;
use super::monitors::MonitorsScreen;

/// The strip's width on the page: its 24 cells (§FS-021-monitors.18.5).
const STRIP: usize = crate::monitors::strip::CELLS;

/// The narrowest a title or a condition is cut to before a column goes.
const NARROWEST: usize = 12;

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
fn health_style(health: Option<Health>) -> Style {
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
fn way_style(way: Way) -> Style {
    match way {
        Way::Better => Style::default().fg(Color::Green),
        Way::Worse => Style::default().fg(Color::Yellow),
        Way::Plain => Style::default(),
    }
}

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn pad(text: &str, width: usize) -> String {
    let text = clip(text, width);
    let short = width.saturating_sub(text.chars().count());
    format!("{text}{}", " ".repeat(short))
}

/// What fits across the page: each column's width, and whether the strip
/// and the condition are drawn. A narrow screen gives up the condition
/// first, then cuts the titles, then the strip, and clips the rest at its
/// edge; the detail view still says everything (§FS-021-monitors.19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Columns {
    pub title: usize,
    value: usize,
    every: usize,
    pub condition: Option<usize>,
    pub strip: bool,
    change: usize,
    health: usize,
}

impl Columns {
    pub(super) fn fit(page: &views::Monitors, width: usize) -> Columns {
        let widest = |say: &dyn Fn(&MonitorRow) -> usize, heading: &str| {
            page.rows()
                .map(say)
                .chain([heading.chars().count()])
                .max()
                .unwrap_or(0)
        };
        let title = page
            .projects
            .iter()
            .map(|group| group.project.chars().count())
            .chain(page.rows().map(|row| row.title.chars().count()))
            .max()
            .unwrap_or(0)
            .min(TITLE);
        let mut columns = Columns {
            title,
            value: widest(&|row| row.shows.value.chars().count(), "value"),
            every: widest(&|row| row.shows.every.chars().count(), "every"),
            condition: None,
            strip: true,
            change: widest(&|row| row.shows.change.says.chars().count(), "change"),
            health: widest(&|row| row.shows.health.text().chars().count(), "health"),
        };
        let matter = widest(&|row| row.shows.matter.chars().count(), "matter");
        let condition = widest(
            &|row| row.condition_noted_says(when).chars().count(),
            "condition",
        );
        // The glyph and its space, and two spaces after each column but the matter.
        let fixed = |columns: &Columns| {
            2 + columns.title
                + 2
                + columns.value
                + 2
                + columns.every
                + 2
                + columns.change
                + 2
                + columns.health
                + 2
                + matter
        };
        let room = width.saturating_sub(fixed(&columns) + STRIP + 2);
        if room >= NARROWEST + 2 {
            columns.condition = Some(condition.min(room - 2));
            return columns;
        }
        let short = fixed(&columns) + STRIP + 2;
        if short > width {
            columns.title = columns
                .title
                .saturating_sub(short - width)
                .max(NARROWEST.min(columns.title));
        }
        if fixed(&columns) + STRIP + 2 > width {
            columns.strip = false;
            columns.title = title;
            let over = fixed(&columns).saturating_sub(width);
            columns.title = title.saturating_sub(over).max(NARROWEST.min(title));
        }
        columns
    }
}

impl MonitorsScreen {
    pub fn draw(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        match self.detail().is_some() {
            true => self.draw_detail(frame, area),
            false => self.draw_page(frame, area),
        }
    }

    fn draw_page(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        self.viewport = area.height;
        let (lines, at) = self.page_lines(usize::from(area.width));
        let height = usize::from(area.height.max(1));
        let mut scroll = usize::from(self.scroll);
        if let Some(at) = at {
            if at < scroll {
                scroll = at;
            } else if at >= scroll + height {
                scroll = at + 1 - height;
            }
        }
        scroll = scroll.min(lines.len().saturating_sub(height));
        self.scroll = u16::try_from(scroll).unwrap_or(u16::MAX);
        frame.render_widget(Paragraph::new(lines).scroll((self.scroll, 0)), area);
    }

    /// The page's lines, and which of them the cursor is on
    /// (§FS-021-monitors.18.3, §FS-021-monitors.18.4).
    pub(super) fn page_lines(&self, width: usize) -> (Vec<Line<'static>>, Option<usize>) {
        let page = self.page();
        let mut lines = Vec::new();
        // The title bar carries the header; where it cannot fit, what `w`
        // changes is said again here so the window is never cut off.
        let header = page.header(self.now());
        if header.chars().count() + 1 > width {
            lines.push(Line::styled(
                format!(
                    " window {} · {}",
                    page.window.label(),
                    views::refreshed(page.refreshed_at, self.now())
                ),
                dim(),
            ));
        }
        if page.projects.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::raw(format!(" {NO_MONITORS}")));
        }
        let columns = Columns::fit(page, width);
        let mut at = None;
        let mut index = 0;
        for group in &page.projects {
            lines.push(Line::raw(""));
            lines.push(heading(
                &group.project,
                &columns,
                page,
                !group.rows.is_empty(),
            ));
            for row in &group.rows {
                let mut line = row_line(row, &columns);
                if index == self.selected() {
                    line = line.style(highlight_style());
                    at = Some(lines.len());
                }
                lines.push(line);
                index += 1;
            }
            for note in &group.notes {
                lines.push(Line::styled(
                    format!("  note: {note}"),
                    Style::default().fg(Color::Yellow),
                ));
            }
        }
        for project in &page.without {
            lines.push(Line::raw(""));
            lines.push(Line::styled(format!("  {project} has no monitors"), dim()));
        }
        if !page.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::styled(format!("  {FOOTNOTE}"), dim()));
        }
        (lines, at)
    }
}

/// A project's heading: its name, then the columns' names, dim
/// (§FS-021-monitors.18.3).
fn heading(project: &str, columns: &Columns, page: &views::Monitors, named: bool) -> Line<'static> {
    let name = Span::styled(
        format!("  {}", pad(project, columns.title)),
        Style::default().add_modifier(Modifier::BOLD),
    );
    if !named {
        return Line::from(name);
    }
    let strip = format!("last {}", page.window.label());
    let mut names = vec![pad("value", columns.value), pad("every", columns.every)];
    if let Some(width) = columns.condition {
        names.push(pad("condition", width));
    }
    if columns.strip {
        names.push(pad(&strip, STRIP));
    }
    names.push(pad("change", columns.change));
    names.push(pad("health", columns.health));
    names.push("matter".to_string());
    Line::from(vec![
        name,
        Span::styled(format!("  {}", names.join("  ")), dim()),
    ])
}

/// One monitor's row (§FS-021-monitors.18.4): the glyph and the health in
/// the rank's colour, the strip cell by cell in each cell's health, and the
/// change by `better`.
fn row_line(row: &MonitorRow, columns: &Columns) -> Line<'static> {
    let shows = &row.shows;
    let rank = rank_style(shows.rank);
    let mut spans = vec![
        Span::styled(shows.glyph.to_string(), rank),
        Span::raw(format!(" {}  ", pad(&row.title, columns.title))),
        Span::raw(format!("{}  ", pad(&shows.value, columns.value))),
        Span::styled(format!("{}  ", pad(&shows.every, columns.every)), dim()),
    ];
    if let Some(width) = columns.condition {
        spans.push(Span::styled(
            format!("{}  ", pad(&row.condition_noted_says(when), width)),
            dim(),
        ));
    }
    if columns.strip {
        spans.extend(
            shows
                .strip
                .iter()
                .map(|cell| Span::styled(cell.glyph().to_string(), health_style(cell.health))),
        );
        spans.push(Span::raw("  "));
    }
    spans.push(Span::styled(
        format!("{}  ", pad(&shows.change.says, columns.change)),
        way_style(shows.change.way),
    ));
    spans.push(Span::styled(
        format!("{}  ", pad(&shows.health.text(), columns.health)),
        rank,
    ));
    spans.push(Span::raw(shows.matter.to_string()));
    Line::from(spans)
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
        let top = head_lines(&detail);
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
                    format!(
                        "  ── {} {} {}",
                        line.of,
                        line.bound,
                        detail.row.say(line.at)
                    ),
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

/// What heads the detail view: the health as the health column says it,
/// coloured by its rank, then the monitor's facts and its matter line
/// (§FS-021-monitors.19). The matter line comes here, beside what `p` acts
/// on, rather than under every reading of the window.
fn head_lines(detail: &views::MonitorDetail) -> Vec<Line<'static>> {
    let mut lines = vec![Line::styled(
        detail.health_says(when),
        rank_style(detail.row.shows.rank).add_modifier(Modifier::BOLD),
    )];
    lines.extend(detail.facts_say(when).into_iter().map(Line::raw));
    lines.push(Line::raw(format!("matter: {}", detail.matter_says())));
    lines.push(Line::raw(""));
    lines
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
