//! The detail view's chart and the strip of health under it
//! (§FS-021-monitors.19).
//!
//! Presentation only (§AR-009-surfaces.4): the values, their healths and
//! the bounds are the model's. What is decided here is where each lies,
//! what the axes say and the colour each stretch of line is drawn in.
//!
//! The axes are a ratatui `Chart` with no data; the readings, the bounds and
//! the cursor's rule are painted on one `Canvas` over its plot, so a frame
//! costs one pass over the readings however many there are. `Plot` is laid
//! once per reading of the monitor, not once per frame.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Line as Stroke};
use ratatui::widgets::{Axis, Chart};

use crate::api::monitor_views::{self as views, when, HEALTH};
use crate::monitors::history::worst;
use crate::monitors::model::Health;
use crate::monitors::strip::{Cell, Mark};

use super::monitors_draw::{dim, health_style};

/// The narrowest the y labels' gutter is: as wide as what heads the strip
/// of health, so the strip's cells start under the plot's first column.
const GUTTER: usize = HEALTH.len();

/// A stretch of the chart's line: the points it passes through, and the
/// health it is drawn in.
pub(super) type Trace = (Vec<(f64, f64)>, Option<Health>);

/// A threshold line's colour: today's `healthy` bound red, where crossing
/// it is a drop, and `warn` amber (§FS-021-monitors.3.7).
pub(super) fn threshold_style(of: &str) -> Style {
    match of {
        "healthy" => Style::default().fg(Color::Red),
        _ => Style::default().fg(Color::Yellow),
    }
}

/// The chart's geometry, from the detail's points: where each lies on x
/// and y, and what the axes say. A boolean's or a level's places are its
/// steps, the healthy or the best on top.
pub(super) struct Plot {
    /// Each chart point's place on x, a failed read's included.
    pub places: Vec<f64>,
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
        let places: Vec<f64> = detail
            .chart
            .iter()
            .enumerate()
            .map(|(at, point)| match (timed, point.t) {
                (true, Some(t)) => (t - span.from).num_seconds() as f64,
                _ => at as f64,
            })
            .collect();
        let points = detail
            .chart
            .iter()
            .zip(&places)
            .map(|(point, x)| point.value.map(|value| (*x, value)))
            .collect::<Vec<_>>();
        let x = match timed {
            true => places.iter().fold(
                [0.0, (span.to - span.from).num_seconds() as f64],
                |x, here| [x[0].min(*here), x[1].max(*here)],
            ),
            false => [0.0, detail.chart.len().saturating_sub(1).max(1) as f64],
        };
        let x_labels = match timed {
            true => {
                let at = |offset: f64| when(span.from + chrono::TimeDelta::seconds(offset as i64));
                vec![at(x[0]), at((x[0] + x[1]) / 2.0), at(x[1])]
            }
            false => untimed_labels(detail),
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
                let marks = axis(values);
                let say = |v: &f64| detail.row.say(*v);
                ([marks[0], marks[2]], marks.iter().map(say).collect())
            }
        };
        Plot {
            places,
            points,
            x,
            y,
            x_labels,
            y_labels,
            stepped,
        }
    }

    /// The line between each two readings in turn, as the points it passes
    /// through and the health it is drawn in: a straight stretch in the
    /// worse of its two ends' healths; a boolean or a level holds its value
    /// in that reading's health until the next reading, then steps in the
    /// worse of the two (§FS-021-monitors.19).
    pub(super) fn segments(&self, detail: &views::MonitorDetail) -> Vec<Trace> {
        let mut segments = Vec::new();
        for (at, pair) in self.points.windows(2).enumerate() {
            let (Some(from), Some(to)) = (pair[0], pair[1]) else {
                continue;
            };
            let (was, now) = (detail.chart[at].health, detail.chart[at + 1].health);
            let worse = worst([was, now]);
            match self.stepped {
                true => {
                    let corner = (to.0, from.1);
                    segments.push((vec![from, corner], was));
                    segments.push((vec![corner, to], worse));
                }
                false => segments.push((vec![from, to], worse)),
            }
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

    /// Where the chart's plot lies in `area`, as ratatui's `Chart` lays it:
    /// the x labels on the bottom row and the x axis above them; the y
    /// labels' gutter, at most a third of the width, then the y axis.
    pub(super) fn graph(&self, area: Rect) -> Rect {
        let width =
            |label: &String| u16::try_from(Line::raw(label.as_str()).width()).unwrap_or(u16::MAX);
        let mut y = area.bottom().saturating_sub(1);
        let mut x = area.left();
        if y > area.top() {
            y -= 1;
        }
        let left = self
            .y_labels
            .iter()
            .map(width)
            .max()
            .unwrap_or(0)
            .max(
                self.x_labels
                    .first()
                    .map_or(0, |label| width(label).saturating_sub(1)),
            )
            .max(GUTTER as u16);
        x += left.min(area.width / 3);
        if y > area.top() {
            y -= 1;
        }
        if x + 1 < area.right() {
            x += 1;
        }
        Rect::new(
            x,
            area.top(),
            area.right().saturating_sub(x),
            y.saturating_sub(area.top()) + 1,
        )
    }

    /// The cell a point lands on in `graph`, as the canvas maps it.
    fn cell(&self, graph: Rect, (x, y): (f64, f64)) -> Option<(u16, u16)> {
        let (wide, tall) = (self.x[1] - self.x[0], self.y[1] - self.y[0]);
        let inside = (self.x[0]..=self.x[1]).contains(&x) && (self.y[0]..=self.y[1]).contains(&y);
        if !inside || wide <= 0.0 || tall <= 0.0 || graph.is_empty() {
            return None;
        }
        let dots = |cells: u16, of: f64| (f64::from(cells) * of - 1.0).max(0.0);
        let column = ((x - self.x[0]) * dots(graph.width, 2.0) / wide) as u16 / 2;
        let row = ((self.y[1] - y) * dots(graph.height, 4.0) / tall) as u16 / 4;
        Some((
            graph.x + column.min(graph.width - 1),
            graph.y + row.min(graph.height - 1),
        ))
    }

    /// The strip of health under the chart, one cell per column of the plot
    /// over the same span (§FS-021-monitors.19): the worst health read in
    /// the column, or the health of the reading in force through it; `·`
    /// where only failed reads fall or the reading in force failed, and
    /// blank before the first reading. Each cell is drawn by its health, so
    /// it reads without colour.
    pub(super) fn strip(
        &self,
        detail: &views::MonitorDetail,
        columns: usize,
    ) -> Vec<Span<'static>> {
        let glyph = |health: Option<Health>| {
            let cell = Cell {
                mark: Mark::Read,
                value: None,
                health,
                height: None,
            };
            Span::styled(cell.health_glyph().to_string(), health_style(health))
        };
        let wide = self.x[1] - self.x[0];
        let mut next = 0;
        let mut in_force = None;
        let mut cells = Vec::with_capacity(columns);
        for column in 0..columns {
            let end = self.x[0] + wide * (column + 1) as f64 / columns as f64;
            let last = column + 1 == columns;
            let (mut any, mut read) = (false, Vec::new());
            while next < self.places.len() && (last || self.places[next] <= end) {
                any = true;
                in_force = Some(next);
                if self.points[next].is_some() {
                    read.push(detail.chart[next].health);
                }
                next += 1;
            }
            cells.push(match in_force {
                _ if !read.is_empty() => glyph(worst(read)),
                None => Span::raw(" "),
                Some(at) if any || self.points[at].is_none() => Span::styled("·", dim()),
                Some(at) => glyph(detail.chart[at].health),
            });
        }
        cells
    }

    /// The chart (§FS-021-monitors.19): labelled axes, today's `healthy`
    /// and `warn` as threshold lines, the readings coloured by their
    /// health, and the cursor as a dim rule through the point it is on.
    ///
    /// A boolean or a level is drawn stepped, holding each reading until the
    /// next: a line drawn straight between `no` and `yes` would show values
    /// nobody read, while a step shows when it flipped and how long it held,
    /// and its y labels name the states, so it reads without colour.
    pub(super) fn draw(
        &self,
        frame: &mut ratatui::Frame,
        area: Rect,
        detail: &views::MonitorDetail,
        cursor: usize,
    ) {
        let axes = Chart::new(Vec::new())
            .legend_position(None)
            .x_axis(
                Axis::default()
                    .style(dim())
                    .bounds(self.x)
                    .labels(self.x_labels.iter().map(|label| Span::raw(label.clone()))),
            )
            .y_axis(
                Axis::default().style(dim()).bounds(self.y).labels(
                    self.y_labels
                        .iter()
                        .map(|label| Span::raw(format!("{label:GUTTER$}"))),
                ),
            );
        frame.render_widget(axes, area);
        let graph = self.graph(area);
        if graph.is_empty() {
            return;
        }
        let colour = |style: Style| style.fg.unwrap_or(Color::Reset);
        let at = self.points.get(cursor).copied().flatten();
        let segments = self.segments(detail);
        let canvas = Canvas::default()
            .x_bounds(self.x)
            .y_bounds(self.y)
            .marker(Marker::Braille)
            .paint(|ctx| {
                if let Some((x, _)) = at {
                    ctx.draw(&Stroke::new(x, self.y[0], x, self.y[1], Color::DarkGray));
                }
                ctx.layer();
                if !self.stepped {
                    for line in &detail.thresholds {
                        let style = colour(threshold_style(line.of));
                        ctx.draw(&Stroke::new(self.x[0], line.at, self.x[1], line.at, style));
                    }
                }
                ctx.layer();
                for (through, health) in &segments {
                    for pair in through.windows(2) {
                        let (from, to) = (pair[0], pair[1]);
                        let style = colour(health_style(*health));
                        ctx.draw(&Stroke::new(from.0, from.1, to.0, to.1, style));
                    }
                }
            });
        frame.render_widget(canvas, graph);
        let buffer = frame.buffer_mut();
        for (at, point) in self.alone() {
            if let Some(cell) = self.cell(graph, point) {
                buffer[cell]
                    .set_symbol("•")
                    .set_style(health_style(detail.chart[at].health));
            }
        }
        if let Some(cell) = at.and_then(|point| self.cell(graph, point)) {
            let mark = Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD);
            buffer[cell].set_symbol("█").set_style(mark);
        }
    }
}

/// A series drawn by `x` labels: its first and last labels, and the middle
/// one only where a point stands at the middle of the axis, which is where
/// ratatui draws it.
fn untimed_labels(detail: &views::MonitorDetail) -> Vec<String> {
    let label = |at: usize| {
        detail
            .chart
            .get(at)
            .and_then(|point| point.x.clone())
            .unwrap_or_else(|| at.to_string())
    };
    match detail.chart.len() {
        0 | 1 => vec![label(0), String::new()],
        points if (points - 1) % 2 == 0 => {
            vec![label(0), label((points - 1) / 2), label(points - 1)]
        }
        points => vec![label(0), label(points - 1)],
    }
}

/// A number axis over `values`: a tenth of their spread beyond each end,
/// never below zero where nothing read is, and rounded out to a step of
/// 1, 2, 2.5 or 5 times a power of ten, so its three labels are round
/// numbers: the low end, one step up, and the high end two steps up.
pub(super) fn axis(values: impl Iterator<Item = f64>) -> [f64; 3] {
    let (lo, hi) = values
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
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
    let (floor, top) = (
        (lo - pad).max(if lo >= 0.0 { 0.0 } else { f64::MIN }),
        hi + pad,
    );
    let mut step = nice((top - floor) / 2.0);
    let mut from = (floor / step).floor() * step;
    for _ in 0..8 {
        if from + 2.0 * step >= top {
            break;
        }
        step = nice(step * 1.01);
        from = (floor / step).floor() * step;
    }
    let round = |v: f64| match decimals(step) {
        Some(places) => {
            let scale = 10f64.powi(places);
            (v * scale).round() / scale
        }
        None => v,
    };
    [round(from), round(from + step), round(from + 2.0 * step)]
}

/// The least of 1, 2, 2.5, 5 and 10 times a power of ten that is not under
/// `raw`.
fn nice(raw: f64) -> f64 {
    if !(raw.is_finite() && raw > 0.0) {
        return 1.0;
    }
    let power = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|times| times * power)
        .find(|step| *step >= raw)
        .unwrap_or(10.0 * power)
}

/// How many decimals `step` needs to be said exactly, where it has a few.
fn decimals(step: f64) -> Option<i32> {
    (0..=12).find(|places| {
        let scaled = step * 10f64.powi(*places);
        (scaled - scaled.round()).abs() < 1e-6 * scaled.abs().max(1.0)
    })
}
