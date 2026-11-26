//! What a row's strip and change cover, and what they say
//! (§FS-021-monitors.18.5, §FS-021-monitors.18.6): the window, or the last 24
//! readings where those reach further back, drawn as 24 cells and judged
//! under today's condition (§FS-021-monitors.18.9).
//!
//! The cells are data, a value, a health and a height of eight for each, so
//! the page and `ephor monitors` draw the same strip; neither decides what a
//! cell holds.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::monitors::condition::{self, Judge};
use crate::monitors::history::{self, Line};
use crate::monitors::model::{Better, Health, Kind, Point, Value};
use crate::monitors::overlay::Monitor;
use crate::monitors::window::Window;
use crate::monitors::words;

/// A strip is this many cells, and covers at least this many readings
/// (§FS-021-monitors.18.5).
pub const CELLS: usize = 24;

/// The heights a cell is drawn at, lowest first.
pub const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// What the strip and the change cover (§FS-021-monitors.18.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Span {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    /// The last 24 readings reach further back than the window, so they set
    /// where it starts.
    pub by_readings: bool,
}

impl Span {
    pub fn holds(&self, at: DateTime<Utc>) -> bool {
        at >= self.from && at <= self.to
    }
}

/// The window ending `now`, or the monitor's last 24 readings where those
/// reach further back, so a weekly monitor always shows its last 24 weeks
/// (§FS-021-monitors.18.5).
pub fn span(lines: &[Line], window: Window, now: DateTime<Utc>) -> Span {
    let from = now - window.span();
    let readings: Vec<DateTime<Utc>> = lines
        .iter()
        .filter(|line| !matches!(line, Line::Marker { .. }))
        .map(Line::t)
        .collect();
    let earliest = readings
        .len()
        .checked_sub(CELLS.min(readings.len()))
        .and_then(|at| readings.get(at))
        .copied();
    match earliest {
        Some(earliest) if earliest < from => Span {
            from: earliest,
            to: now,
            by_readings: true,
        },
        _ => Span {
            from,
            to: now,
            by_readings: false,
        },
    }
}

/// What one cell of a strip shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mark {
    /// It lies before the monitor's history begins: nothing is drawn.
    Before,
    /// Nothing was read in it: a gap, unknown where a read failed.
    Gap,
    /// A value was read in it.
    Read,
}

/// One cell of a strip (§FS-021-monitors.18.5).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Cell {
    pub mark: Mark,
    /// A number's mean, a level's worst as its place among the levels, a
    /// boolean as 1 or 0; a point's value for a series.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// The worst health in it, judged under today's condition; none for a
    /// KPI's (§FS-021-monitors.18.9).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<Health>,
    /// 0 to 7, the block a read cell is drawn as: a boolean's a constant, a
    /// level's its level, a number's its place between the strip's least
    /// and greatest value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u8>,
}

impl Cell {
    fn empty(mark: Mark) -> Cell {
        Cell {
            mark,
            value: None,
            health: None,
            height: None,
        }
    }

    /// The cell as text: its block, `·` for a gap, and a space before the
    /// history begins.
    pub fn glyph(&self) -> char {
        match (self.mark, self.height) {
            (Mark::Read, Some(height)) => BLOCKS[usize::from(height.min(7))],
            (Mark::Read, None) => BLOCKS[3],
            (Mark::Gap, _) => '·',
            (Mark::Before, _) => ' ',
        }
    }
}

/// A line's value on the strip's scale: a number's (a thinned line's mean),
/// a level's place, the worse of a thinned level's two, and a boolean's 1
/// or 0.
pub fn scaled(line: &Line, levels: Option<&[String]>) -> Option<f64> {
    let place = |value: &Value| match value {
        Value::Number(number) => Some(*number),
        Value::Boolean(yes) => Some(f64::from(u8::from(*yes))),
        Value::Level(level) => levels
            .and_then(|levels| levels.iter().position(|named| named == level))
            .and_then(|at| u32::try_from(at).ok())
            .map(f64::from),
    };
    match line {
        Line::Marker { .. } => None,
        Line::Reading { v, .. } => place(v),
        Line::Thinned {
            min: Value::Number(min),
            mean,
            max: Value::Number(max),
            ..
        } => Some(mean.unwrap_or((min + max) / 2.0)),
        Line::Thinned { min, max, .. } => match (place(min), place(max)) {
            (Some(min), Some(max)) => Some(min.max(max)),
            (one, other) => one.or(other),
        },
    }
}

/// The strip of a monitor read over time (§FS-021-monitors.18.5): 24 cells
/// dividing the span, each holding the lines that fall in it.
pub fn strip(lines: &[Line], span: &Span, monitor: &Monitor, judge: &Judge) -> Vec<Cell> {
    let levels = monitor.levels.as_deref();
    let first = lines.first().map(Line::t);
    let width = (span.to - span.from) / i32::try_from(CELLS).unwrap_or(24);
    let mut cells = Vec::with_capacity(CELLS);
    for at in 0..CELLS {
        let step = i32::try_from(at).unwrap_or(0);
        let low = span.from + width * step;
        let high = span.from + width * (step + 1);
        let last = at + 1 == CELLS;
        let inside: Vec<&Line> = lines
            .iter()
            .filter(|line| line.t() >= low && (line.t() < high || (last && line.t() <= span.to)))
            .collect();
        if first.is_none_or(|first| high <= first) && inside.is_empty() {
            cells.push(Cell::empty(Mark::Before));
            continue;
        }
        let health = history::worst(
            inside
                .iter()
                .map(|line| history::rejudged(line, judge, monitor.warn.as_ref(), levels)),
        );
        let values: Vec<f64> = inside
            .iter()
            .filter_map(|line| scaled(line, levels))
            .collect();
        let value = match monitor.kind {
            _ if values.is_empty() => None,
            Some(Kind::Level) => values.iter().copied().reduce(f64::max),
            Some(Kind::Boolean) => values.last().copied(),
            _ => Some(values.iter().sum::<f64>() / values.len() as f64),
        };
        cells.push(Cell {
            mark: if value.is_some() {
                Mark::Read
            } else {
                Mark::Gap
            },
            value,
            health,
            height: None,
        });
    }
    heighten(&mut cells, monitor);
    cells
}

/// A series' strip: its last 24 points, each judged under today's
/// condition (§FS-021-monitors.18.5).
pub fn series(points: &[Point], monitor: &Monitor, judge: &Judge) -> Vec<Cell> {
    let drawn = &points[points.len().saturating_sub(CELLS)..];
    let mut cells: Vec<Cell> = std::iter::repeat_n(Cell::empty(Mark::Before), CELLS - drawn.len())
        .chain(drawn.iter().map(|point| {
            Cell {
                mark: Mark::Read,
                value: Some(point.v),
                health: condition::band(
                    judge,
                    monitor.warn.as_ref(),
                    &Value::Number(point.v),
                    None,
                    monitor.levels.as_deref(),
                )
                .map(Health::from),
                height: None,
            }
        }))
        .collect();
    heighten(&mut cells, monitor);
    cells
}

/// Gives each read cell its height: a boolean's is constant, a level's its
/// place among the levels, the first the tallest, and a number's its place
/// between the strip's least and greatest value, the middle where they are
/// the same.
fn heighten(cells: &mut [Cell], monitor: &Monitor) {
    let values = cells.iter().filter_map(|cell| cell.value);
    let low = values.clone().fold(f64::INFINITY, f64::min);
    let high = values.fold(f64::NEG_INFINITY, f64::max);
    let levels = monitor.levels.as_ref().map_or(0, Vec::len);
    for cell in cells.iter_mut().filter(|cell| cell.mark == Mark::Read) {
        let Some(value) = cell.value else {
            continue;
        };
        let height = match monitor.kind {
            Some(Kind::Boolean) => 5.0,
            Some(Kind::Level) if levels > 1 => {
                let last = (levels - 1) as f64;
                ((last - value) / last * 6.0).round() + 1.0
            }
            Some(Kind::Level) => 7.0,
            _ if high > low => ((value - low) / (high - low) * 7.0).round(),
            _ => 3.0,
        };
        cell.height = Some(height.clamp(0.0, 7.0) as u8);
    }
}

/// Which way a change went, by the monitor's `better`
/// (§FS-021-monitors.18.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Way {
    /// It went the way `better` names: green.
    Better,
    /// It went the other way: amber.
    Worse,
    /// Nothing says which way is better, or nothing changed.
    Plain,
}

/// A row's change (§FS-021-monitors.18.6).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change {
    /// `no drops`, `3 drops`, `▾ 2 ms`, `▴ 1.2 pt`, `±0` or `—`.
    pub says: String,
    pub way: Way,
}

impl Change {
    fn plain(says: impl Into<String>) -> Change {
        Change {
            says: says.into(),
            way: Way::Plain,
        }
    }
}

/// The change over the span: how often a boolean or a level dropped, and a
/// number's difference between the first and the last value it read
/// (§FS-021-monitors.18.6). A drop is counted as the hold counts one
/// (§FS-021-monitors.10.1): `drop_after` failing readings in a row, ended by
/// `recover_after` passing ones.
pub fn change(
    lines: &[Line],
    span: &Span,
    monitor: &Monitor,
    judge: &Judge,
    recover_after: u32,
) -> Change {
    if matches!(monitor.kind, Some(Kind::Boolean | Kind::Level)) {
        let hold = (monitor.drop_after, recover_after);
        return Change::plain(match drops(lines, span, monitor, judge, hold) {
            0 => "no drops".to_string(),
            1 => "1 drop".to_string(),
            drops => format!("{drops} drops"),
        });
    }
    let values: Vec<f64> = lines
        .iter()
        .filter(|line| span.holds(line.t()))
        .filter_map(|line| scaled(line, None))
        .collect();
    difference(values.first(), values.last(), monitor)
}

/// How many drops turned inside the span, the hold run over the whole
/// history under today's condition so a span that opens already down counts
/// none for it. Only readings count: a marker neither adds to a run nor
/// breaks it (§FS-021-monitors.10.2), and a thinned line counts as one
/// reading of its worst health.
fn drops(
    lines: &[Line],
    span: &Span,
    monitor: &Monitor,
    judge: &Judge,
    (drop_after, recover_after): (u32, u32),
) -> usize {
    let levels = monitor.levels.as_deref();
    let (mut down, mut run, mut drops) = (false, 0, 0);
    for line in lines.iter().filter(|line| line.t() <= span.to) {
        if matches!(line, Line::Marker { .. }) {
            continue;
        }
        let fails =
            history::rejudged(line, judge, monitor.warn.as_ref(), levels) == Some(Health::Down);
        // A run counts readings that go against where it stands.
        run = if fails != down { run + 1 } else { 0 };
        let needed = if down { recover_after } else { drop_after };
        if run >= needed.max(1) {
            down = !down;
            run = 0;
            if down && span.holds(line.t()) {
                drops += 1;
            }
        }
    }
    drops
}

/// A series' change: between the first and the last point its strip draws
/// (§FS-021-monitors.18.6).
pub fn series_change(points: &[Point], monitor: &Monitor) -> Change {
    let drawn = &points[points.len().saturating_sub(CELLS)..];
    difference(
        drawn.first().map(|point| &point.v),
        drawn.last().map(|point| &point.v),
        monitor,
    )
}

/// `▴ 1.2 pt`, `▾ 2 ms`, `±0`, or `—` where there is no value, coloured by
/// `better`.
fn difference(first: Option<&f64>, last: Option<&f64>, monitor: &Monitor) -> Change {
    let (Some(first), Some(last)) = (first, last) else {
        return Change::plain("—");
    };
    let moved = last - first;
    if moved.abs() < 1e-9 {
        return Change::plain("±0");
    }
    let arrow = if moved > 0.0 { "▴" } else { "▾" };
    let size = moved.abs();
    let said = match monitor.unit.as_deref() {
        // A percentage moves in points (§FS-021-monitors.18.6).
        Some("%") if monitor.of.is_none() => format!("{size:.1} pt"),
        unit => words::with_unit(&words::number((size * 100.0).round() / 100.0), unit),
    };
    let way = match monitor.better {
        None => Way::Plain,
        Some(Better::Higher) if moved > 0.0 => Way::Better,
        Some(Better::Lower) if moved < 0.0 => Way::Better,
        Some(_) => Way::Worse,
    };
    Change {
        says: format!("{arrow} {said}"),
        way,
    }
}

#[cfg(test)]
#[path = "strip_tests.rs"]
mod tests;
