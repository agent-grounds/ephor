//! The lines both surfaces say of the Monitors page and of one monitor's
//! detail view, so the screen `M` opens and `ephor monitors` print the same
//! words (§FS-021-monitors.18, §FS-021-monitors.19, §REQ-002-parity.2).
//! Layout stays each surface's own; what a line says is decided here.

use chrono::{DateTime, Local, Utc};

use super::{ChartPoint, EpisodeView, FromView, MonitorDetail, MonitorRow, ReadingView};
use crate::api::monitors::NEW;
use crate::monitors::episode::Sort;
use crate::monitors::model::{Point, Source};

/// The footnote under the page (§FS-021-monitors.18.5, §FS-021-monitors.21.1).
pub const FOOTNOTE: &str =
    "change: over the window, or over the last 24 readings when those reach further back";

/// The widest a title is shown, `…` standing for the rest, so one long title
/// does not push every row of the page out of line (§FS-021-monitors.21.1).
pub const TITLE: usize = 25;

/// A title as the page shows it: clipped past [`TITLE`] characters.
pub fn clipped(title: &str) -> String {
    clip(title, TITLE)
}

/// `text` in at most `width` characters, its last one `…` where it was cut.
pub fn clip(text: &str, width: usize) -> String {
    match text.chars().count() > width {
        true if width == 0 => String::new(),
        true => text.chars().take(width - 1).chain(['…']).collect(),
        false => text.to_string(),
    }
}

/// A moment as the detail view says it, in local time: `2026-11-25 10:00`.
pub fn when(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

impl FromView {
    /// `command scripts/kpis.sh · in /repo` (§FS-021-monitors.5.3).
    pub fn says(&self) -> String {
        let source = match &self.source {
            Source::Command { command, .. } => format!("command {command}"),
            Source::File { file, .. } => format!("file {file}"),
        };
        let runs = self
            .runs_in
            .as_ref()
            .map(|dir| format!(" · in {}", dir.display()))
            .unwrap_or_default();
        format!("{source}{runs}")
    }
}

impl EpisodeView {
    /// `drop    2026-11-24 10:00 → open  monitor:widget/coverage@…`
    /// (§FS-021-monitors.19).
    pub fn says(&self, when: impl Fn(DateTime<Utc>) -> String) -> String {
        let end = self.end.map_or_else(|| "open".to_string(), &when);
        let sort = match self.sort {
            Sort::Drop => "drop",
            Sort::Unknown => "unknown",
        };
        format!("{sort:<7} {} → {end}  {}", when(self.start), self.matter)
    }
}

impl ReadingView {
    /// `2026-11-25 10:00  79.2%        down    read out of cadence`
    /// (§FS-021-monitors.19).
    pub fn says(&self, when: impl Fn(DateTime<Utc>) -> String) -> String {
        let health = self.health.map_or("—", |health| health.name());
        let note = self
            .note
            .as_ref()
            .map(|note| format!("  {note}"))
            .unwrap_or_default();
        let said = format!("{}  {:<12} {:<8}{note}", when(self.t), self.value, health);
        said.trim_end().to_string()
    }
}

impl MonitorRow {
    /// The condition column as the page and `ephor monitors` both show it:
    /// the condition with the sample's `detail`, then the row's notes
    /// (§FS-021-monitors.18.4, §FS-021-monitors.4.6, §FS-021-monitors.7.1),
    /// `healthy ≤ 50 ms · unknown field heathy`, so a misspelt key is seen on
    /// the row itself.
    pub fn condition_noted_says(&self, when: impl Fn(DateTime<Utc>) -> String) -> String {
        std::iter::once(self.condition_says())
            .chain(self.notes_say(when))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// Why `p` has nothing to process on this row, where it has not
    /// (§FS-021-monitors.18.10): why there is no matter, as the detail view's
    /// matter line says it (§FS-021-monitors.19), or what a matter past New
    /// leaves to do. A dismissed matter comes back when its monitor moves
    /// (§FS-021-monitors.13.2); only a finished one waits for the next drop
    /// (§FS-021-monitors.11.1). None for a New matter, which `p` processes.
    pub fn unprocessed_says(&self) -> Option<String> {
        let Some(matter) = &self.matter else {
            return Some(self.no_matter.clone().unwrap_or_else(|| "none".to_string()));
        };
        match matter.state {
            NEW => None,
            "running" => {
                Some("work is already running on this matter; Waiting shows the run".to_string())
            }
            "dismissed" => {
                Some("the matter is dismissed; it comes back if the monitor moves".to_string())
            }
            state => Some(format!(
                "the matter is {state}; the next drop opens a new one"
            )),
        }
    }
}

impl MonitorDetail {
    /// What the detail view says of the monitor under its health, one
    /// `label: text` line each (§FS-021-monitors.19): its sentence where
    /// ephor does not read it, its value, its condition in words, the
    /// sample's `detail` and the row's notes, its description, where it is
    /// read from and declared, its cadence and freshness, and its hold.
    pub fn facts_say(&self, when: impl Fn(DateTime<Utc>) -> String) -> Vec<String> {
        let row = &self.row;
        let mut said = Vec::new();
        said.extend(row.shows.health.sentence.clone());
        said.push(format!("value: {}", row.shows.value));
        said.push(format!("condition: {}", row.condition.parts_say()));
        said.extend(
            row.detail
                .as_ref()
                .map(|detail| format!("detail: {detail}")),
        );
        said.extend(
            row.notes_say(&when)
                .into_iter()
                .map(|note| format!("note: {note}")),
        );
        said.extend(
            self.description
                .as_ref()
                .map(|about| format!("about: {about}")),
        );
        said.extend(
            self.from
                .as_ref()
                .map(|from| format!("from: {}", from.says())),
        );
        said.push(format!("declared: {}", self.declared.says()));
        said.push(format!("cadence: {}", row.cadence_says(&when)));
        said.extend(
            row.freshness
                .says(&when)
                .map(|said| format!("freshness: {said}")),
        );
        said.extend(
            self.hold
                .as_ref()
                .map(|hold| format!("hold: {}", hold.says())),
        );
        said
    }

    /// One point a repository supplied: its time or its `x` label, then its
    /// value in the row's words (§FS-021-monitors.2.4).
    pub fn point_says(&self, point: &Point, when: impl Fn(DateTime<Utc>) -> String) -> String {
        let at = match (&point.t, &point.x) {
            (Some(t), _) => when(t.with_timezone(&Utc)),
            (None, Some(x)) => x.clone(),
            (None, None) => "—".to_string(),
        };
        format!("{at:<16}  {}", self.row.say(point.v))
    }

    /// The chart's cursor (§FS-021-monitors.19): the point it stands on,
    /// its time or label, its value in the row's words, a boolean's or a
    /// level's by its name, and its health, `KPI` where nothing judges it.
    pub fn cursor_says(
        &self,
        point: &ChartPoint,
        when: impl Fn(DateTime<Utc>) -> String,
    ) -> String {
        let at = match (&point.t, &point.x) {
            (Some(t), _) => when(*t),
            (None, Some(x)) => x.clone(),
            (None, None) => "—".to_string(),
        };
        let value = match point.value {
            None => "—".to_string(),
            Some(place) if !self.steps.is_empty() => self
                .steps
                .get(place.round().max(0.0) as usize)
                .cloned()
                .unwrap_or_else(|| self.row.say(place)),
            Some(value) => self.row.say(value),
        };
        let health = match (point.health, self.row.shows.status) {
            (Some(health), _) => health.name(),
            (None, false) => "KPI",
            (None, true) => "—",
        };
        format!("{at}  {value}  {health}")
    }
}
