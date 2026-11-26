//! The Monitors page and one monitor's detail view, as data
//! (§FS-021-monitors.18, §FS-021-monitors.19): what `ephor monitors --json`
//! and `ephor monitors show --json` print (§FS-021-monitors.21.5), and what
//! the screen and the command's table draw. Plain data, and the few sentences
//! both surfaces say the same way; nothing here reads or decides.

use std::path::PathBuf;

use chrono::{DateTime, FixedOffset, Utc};
use serde::Serialize;
use serde_json::Value as Json;

use crate::monitors::condition::Judge;
use crate::monitors::episode::Sort;
use crate::monitors::model::{Better, Condition, Health, Home, Kind, Point, Source, Value};
use crate::monitors::page::{Column, Rank};
use crate::monitors::strip::{Cell, Change, Span};
use crate::monitors::window::Window;

/// What the page says with no monitor to show (§FS-021-monitors.18.2).
pub const NO_MONITORS: &str = "no monitors — ephor guide monitors says how to declare one";

/// `ephor monitors` (§FS-021-monitors.21.1): the header's counts, then each
/// project and its monitors, worst first (§FS-021-monitors.18.3).
#[derive(Debug, Clone, Serialize)]
pub struct Monitors {
    pub window: Window,
    pub counts: PageCounts,
    /// The newest fetch the cache of the projects read records, as Waiting's
    /// header says it (§FS-019-waiting.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refreshed_at: Option<DateTime<Utc>>,
    pub projects: Vec<MonitorProject>,
    /// Projects named with `--project` that have no monitor
    /// (§FS-021-monitors.21.1).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub without: Vec<String>,
}

impl Monitors {
    /// Whether there is no row to show; a project whose manifest could not
    /// be read may still have its notes to say (§FS-021-monitors.17.3).
    pub fn is_empty(&self) -> bool {
        self.rows().next().is_none()
    }

    /// Every row, in the page's order: what `[` and `]` step through
    /// (§FS-021-monitors.19).
    pub fn rows(&self) -> impl Iterator<Item = &MonitorRow> {
        self.projects.iter().flat_map(|project| project.rows.iter())
    }

    /// Whether `--check` exits `4`: a status printed down or unknown,
    /// overdue included, a KPI never and a monitor the header leaves out
    /// never (§FS-021-monitors.21.2).
    pub fn fails_check(&self) -> bool {
        self.rows().any(|row| {
            row.shows.status && matches!(row.shows.counts, Some(Rank::Down | Rank::Unknown))
        })
    }

    /// The header (§FS-021-monitors.18.2): `ephor — monitors   1 down · 2
    /// unknown · … · 1 overdue   window 7d · refreshed 4m ago`.
    pub fn header(&self, now: DateTime<Utc>) -> String {
        format!(
            "ephor — monitors   {}   window {} · {}",
            self.counts.say(),
            self.window.label(),
            refreshed(self.refreshed_at, now)
        )
    }
}

/// `refreshed 4m ago`, or `never refreshed`, as Waiting's header says it
/// (§FS-019-waiting.3).
pub fn refreshed(at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    match at {
        Some(at) => format!("refreshed {}", crate::feed::render::ago(now, at)),
        None => "never refreshed".to_string(),
    }
}

/// The header's counts (§FS-021-monitors.18.2): each monitor once by the
/// rank its row shows, and `overdue` a second time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct PageCounts {
    pub down: usize,
    pub unknown: usize,
    pub failing: usize,
    pub warn: usize,
    pub ok: usize,
    pub kpis: usize,
    pub overdue: usize,
}

impl PageCounts {
    pub fn add(&mut self, shows: &Shows) {
        match shows.counts {
            Some(Rank::Down) => self.down += 1,
            Some(Rank::Unknown) => self.unknown += 1,
            Some(Rank::Failing) => self.failing += 1,
            Some(Rank::Warn) => self.warn += 1,
            Some(Rank::Ok) => self.ok += 1,
            Some(Rank::Kpi) => self.kpis += 1,
            None => {}
        }
        if shows.overdue {
            self.overdue += 1;
        }
    }

    /// `1 down · 2 unknown · 1 failing · 1 warn · 11 ok · 4 KPIs · 1 overdue`,
    /// overdue only when it is not zero.
    pub fn say(&self) -> String {
        let mut said = format!(
            "{} down · {} unknown · {} failing · {} warn · {} ok · {} KPIs",
            self.down, self.unknown, self.failing, self.warn, self.ok, self.kpis
        );
        if self.overdue > 0 {
            said.push_str(&format!(" · {} overdue", self.overdue));
        }
        said
    }
}

/// One project's monitors on the page.
#[derive(Debug, Clone, Serialize)]
pub struct MonitorProject {
    pub project: String,
    pub rows: Vec<MonitorRow>,
    /// What the declarations or the store could not be read for.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// One monitor as the page shows it (§FS-021-monitors.18.4,
/// §FS-021-monitors.21.5).
#[derive(Debug, Clone, Serialize)]
pub struct MonitorRow {
    pub project: String,
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    /// The last reading's value, as the sample gave it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// The whole that value is a fraction of (§FS-021-monitors.2.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub of: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub better: Option<Better>,
    /// `ok`, `warn`, `down` or `unknown` (§FS-021-monitors.1.3); absent for a
    /// KPI whose value holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<Health>,
    /// When that health began.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<DateTime<Utc>>,
    pub shows: Shows,
    pub cadence: CadenceView,
    pub freshness: FreshnessView,
    pub condition: ConditionView,
    /// The open matter, its drop's before its unknown one, or the finished
    /// one still reported (§FS-021-monitors.18.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matter: Option<MatterView>,
    /// The last sample's `detail` (§FS-021-monitors.8.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The last sample's `url`, what `o` opens (§FS-021-monitors.18.10).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Keys its declarations do not know, noted beside the condition
    /// (§FS-021-monitors.4.6): `unknown field heathy`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// The last reading's own `at`, where it lay after its read, which
    /// counts as measured when it was read (§FS-021-monitors.7.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skew: Option<DateTime<FixedOffset>>,
}

impl MonitorRow {
    /// The command's health column (§FS-021-monitors.21.1): the health as
    /// one word, with what the page's health column says beside it where
    /// that is more (§FS-021-monitors.18.7): `ok`, `failing 1 of 2`,
    /// `unknown (overdue 4h)`, `ok (parked 30m)`, `down (recovering 1 of 2)`.
    pub fn health_says(&self) -> String {
        let (word, column) = (&self.shows.word, &self.shows.health);
        if column.state == *word {
            return word.clone();
        }
        if column.state.starts_with(&format!("{word} ")) {
            return column.state.clone();
        }
        format!("{word} ({})", column.text())
    }

    /// A number in the row's terms, its unit and its whole, as its value is
    /// said: a point's value, `17.4 MiB`, `12/15`.
    pub fn say(&self, number: f64) -> String {
        crate::monitors::words::value(&Value::Number(number), self.unit.as_deref(), self.of)
    }

    /// What the row notes beside its condition: each key its declarations
    /// do not know, and a sample measured after its read
    /// (§FS-021-monitors.4.6, §FS-021-monitors.7.1), each time said by `when`.
    pub fn notes_say(&self, when: impl Fn(DateTime<Utc>) -> String) -> Vec<String> {
        let mut said = self.notes.clone();
        if let Some(at) = self.skew {
            said.push(format!(
                "its sample says it was measured {}, after it was read",
                when(at.with_timezone(&Utc))
            ));
        }
        said
    }

    /// The condition column: the condition in words with the sample's
    /// `detail`, or the sentence a monitor ephor does not read carries
    /// (§FS-021-monitors.18.4, §FS-021-monitors.4.6).
    pub fn condition_says(&self) -> String {
        if let Some(sentence) = &self.shows.health.sentence {
            return sentence.clone();
        }
        match &self.detail {
            Some(detail) => format!("{} · {detail}", self.shows.condition),
            None => self.shows.condition.clone(),
        }
    }

    /// `every 1d at 06:00 · last read … · next slot … · grace 6h`
    /// (§FS-021-monitors.19), each time said by `when`.
    pub fn cadence_says(&self, when: impl Fn(DateTime<Utc>) -> String) -> String {
        let mut said = vec![self.cadence.says.clone()];
        if let Some(at) = self.freshness.last_read {
            let found = match self.freshness.last_found {
                Some("unreadable") => " (failed)",
                Some("parked") => " (parked)",
                Some("stale") => " (stale)",
                _ => "",
            };
            said.push(format!("last read {}{found}", when(at)));
        }
        if let Some(at) = self.freshness.next_slot {
            said.push(format!("next slot {}", when(at)));
        }
        said.push(format!("grace {}", self.cadence.grace));
        said.join(" · ")
    }
}

/// What the row shows (§FS-021-monitors.18.3, §FS-021-monitors.18.4).
#[derive(Debug, Clone, Serialize)]
pub struct Shows {
    pub rank: Rank,
    pub glyph: &'static str,
    /// The value in words: `79.2%`, `12/15`, `yes`, `—` where none is read
    /// or the last no longer holds (§FS-021-monitors.7.4).
    pub value: String,
    /// The cadence column: `10m`, `daily`, `weekly` (§FS-021-monitors.18.4).
    pub every: String,
    /// The condition in words (§FS-021-monitors.3.4).
    pub condition: String,
    /// The health column (§FS-021-monitors.18.7).
    pub health: Column,
    /// The health as one word: `down`, `unknown`, `failing`, `warn`, `ok`,
    /// `KPI`, or `not run`, `not read` or `not read yet` for a monitor ephor
    /// does not read or has not yet read.
    pub word: String,
    /// How long that health has held, `2d 4h`, or in points for a series
    /// drawn by `x` labels; none for a KPI and before the first reading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held: Option<String>,
    /// The header count the row falls under, none where it is not yet a
    /// failure (§FS-021-monitors.18.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counts: Option<Rank>,
    pub overdue: bool,
    /// Something judges it, so `--check` reads it (§FS-021-monitors.21.2).
    pub status: bool,
    /// What the strip and the change cover (§FS-021-monitors.18.5).
    pub span: Span,
    pub strip: Vec<Cell>,
    pub change: Change,
    /// `New`, `running`, `Recent`, `dismissed` or `—` (§FS-021-monitors.18.4).
    pub matter: &'static str,
}

/// The cadence (§FS-021-monitors.6): `every`, `at`, and the home that said
/// them.
#[derive(Debug, Clone, Serialize)]
pub struct CadenceView {
    pub every: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// `site`, `manifest`, or `default` where no home states it, which for a
    /// monitor read from the answer that brings it is that answer's
    /// (§FS-021-monitors.6.8).
    pub from: String,
    /// `every 1d at 06:00`.
    pub says: String,
    /// How long past a slot its value still holds (§FS-021-monitors.7.1).
    pub grace: String,
}

/// Where its value stands (§FS-021-monitors.7, §FS-021-monitors.21.5).
#[derive(Debug, Clone, Default, Serialize)]
pub struct FreshnessView {
    /// `fresh`, `late`, `unknown`, `not-read-yet`, `not-run` or `not-read`.
    pub state: &'static str,
    /// When it was last read, whatever the read found
    /// (§FS-021-monitors.9.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_read: Option<DateTime<Utc>>,
    /// What that read found: `reading`, `unreadable`, `parked` or `stale`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_found: Option<&'static str>,
    /// The schedule's next slot (§FS-021-monitors.6.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_slot: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holds_until: Option<DateTime<Utc>>,
    /// `unreadable`, `parked`, `stale` or `overdue`, for a late or unknown
    /// monitor (§FS-021-monitors.7.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<&'static str>,
    /// Why a read failed, for an unreadable one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// When the cause began to apply: the slot that passed without a reading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<DateTime<Utc>>,
    /// When a stale value was measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured: Option<DateTime<Utc>>,
    /// The unknown matter's key, where one is open (§FS-021-monitors.11.2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matter: Option<String>,
}

impl FreshnessView {
    /// A late or unknown monitor's freshness in words: `unknown · stale ·
    /// measured … · holds until …`; none for one whose value holds
    /// (§FS-021-monitors.7.2).
    pub fn says(&self, when: impl Fn(DateTime<Utc>) -> String) -> Option<String> {
        let cause = self.cause?;
        let mut said = vec![self.state.to_string(), cause.to_string()];
        if let Some(why) = &self.why {
            said.push(why.clone());
        }
        if let Some(at) = self.since {
            said.push(format!("since {}", when(at)));
        }
        if let Some(at) = self.measured {
            said.push(format!("measured {}", when(at)));
        }
        if let Some(at) = self.holds_until {
            said.push(format!("held until {}", when(at)));
        }
        if let Some(matter) = &self.matter {
            said.push(matter.clone());
        }
        Some(said.join(" · "))
    }
}

/// The condition, and the home each part came from (§FS-021-monitors.3.6,
/// §FS-021-monitors.21.5).
#[derive(Debug, Clone, Serialize)]
pub struct ConditionView {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthy: Option<Condition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn: Option<Condition>,
    /// What judges it: `site` or `manifest` for a stated `healthy`, `sample`
    /// for one its samples bring, `ok` for the sample's own `ok`, `default`
    /// for the kind's, and `kpi` where nothing does.
    pub from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn_from: Option<Home>,
    /// The condition in words.
    pub says: String,
    /// What the site's `healthy` and `warn` override: the manifest's, as it
    /// states them, `{"healthy": {"min": 75}, "from": "manifest"}`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Json>,
    /// Each part in words, with the home it came from and what it overrode
    /// (§FS-021-monitors.19): `healthy ≥ 80%` from the site over the
    /// manifest's `healthy ≥ 75%`.
    pub parts: Vec<ConditionPart>,
}

/// One part of a condition, `healthy` or `warn`, in words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConditionPart {
    /// `healthy` or `warn`.
    pub part: &'static str,
    /// `healthy ≥ 80%`, `warn below 90%`, `KPI`.
    pub says: String,
    /// What it came from, as `from` says it for `healthy` and `warn_from`
    /// for `warn`.
    pub from: String,
    /// The manifest's, where the site's overrides it, in words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub over: Option<String>,
}

impl ConditionPart {
    /// `healthy ≥ 80% (the site's, over the manifest's healthy ≥ 75%)`.
    pub fn say(&self) -> String {
        let from = match self.from.as_str() {
            "site" => "the site's",
            "manifest" => "the manifest's",
            "sample" => "its samples'",
            "default" => "the kind's default",
            _ => return self.says.clone(),
        };
        match &self.over {
            Some(over) => format!("{} ({from}, over the manifest's {over})", self.says),
            None => format!("{} ({from})", self.says),
        }
    }
}

impl ConditionView {
    /// The condition and where each part came from, as the detail view
    /// says it (§FS-021-monitors.19).
    pub fn parts_say(&self) -> String {
        match self.parts.is_empty() {
            true => self.says.clone(),
            false => self
                .parts
                .iter()
                .map(ConditionPart::say)
                .collect::<Vec<_>>()
                .join(" · "),
        }
    }

    pub fn judge_from(judge: &Judge, healthy_home: Option<Home>) -> String {
        match (judge, healthy_home) {
            (Judge::Kpi, _) => "kpi".to_string(),
            (Judge::SampleOk, _) => "ok".to_string(),
            (Judge::Condition(_), Some(home)) => home_name(home).to_string(),
            (Judge::Condition(_), None) => "default".to_string(),
        }
    }
}

/// A home's name as the machine forms spell it.
pub fn home_name(home: Home) -> &'static str {
    match home {
        Home::Site => "site",
        Home::Manifest => "manifest",
        Home::Sample => "sample",
    }
}

/// A monitor's matter, as the matter column and `p` take it
/// (§FS-021-monitors.18.3, §FS-021-monitors.18.4).
#[derive(Debug, Clone, Serialize)]
pub struct MatterView {
    /// Its key, which is its feed id (§FS-021-monitors.11.3).
    pub key: String,
    pub sort: Sort,
    /// `New`, `running`, `dismissed`, or `Recent` for a finished one.
    pub state: &'static str,
    /// The work it would be handed, for a New one (§FS-021-monitors.16).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<super::views::Proposal>,
    /// Why the monitor's own `action` is not that proposal
    /// (§FS-021-monitors.16.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passed_over: Option<String>,
}

/// `ephor monitors show` (§FS-021-monitors.19, §FS-021-monitors.21.3): the
/// row, and everything the detail view adds to it.
#[derive(Debug, Clone, Serialize)]
pub struct MonitorDetail {
    #[serde(flatten)]
    pub row: MonitorRow,
    pub window: Window,
    /// False for a series drawn by `x` labels, which has no time to set a
    /// window on (§FS-021-monitors.19).
    pub windowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refreshed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Where it is read from and where that runs; absent for a monitor read
    /// from the answer that brings it (§FS-021-monitors.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<FromView>,
    pub declared: DeclaredView,
    /// The hold; absent for a KPI (§FS-021-monitors.10.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<HoldView>,
    /// The span's readings, oldest first, for the chart.
    pub chart: Vec<ChartPoint>,
    /// Today's `healthy` and `warn` bounds, drawn as threshold lines.
    pub thresholds: Vec<Threshold>,
    /// The points the repository supplied: a series', or the history a
    /// number hands over (§FS-021-monitors.2.2, §FS-021-monitors.2.4).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Point>,
    /// The episodes inside the window.
    pub episodes: Vec<EpisodeView>,
    /// Every reading in the window, newest first.
    pub readings: Vec<ReadingView>,
    /// Why there is no open matter, where there is none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_matter: Option<String>,
}

impl MonitorDetail {
    /// `ephor — monitors › <project> › <title>`.
    pub fn title(&self) -> String {
        format!(
            "ephor — monitors › {} › {}",
            self.row.project, self.row.title
        )
    }

    /// The matter line (§FS-021-monitors.19): the open matter, its state
    /// and its proposal, or why there is none.
    pub fn matter_says(&self) -> String {
        let Some(matter) = &self.row.matter else {
            return self.no_matter.clone().unwrap_or_else(|| "none".to_string());
        };
        let mut said = format!("{} · {}", matter.key, matter.state);
        if let Some(proposal) = &matter.proposal {
            said.push_str(&format!(" · proposes {} {}", proposal.kind, proposal.entry));
        }
        if let Some(why) = &matter.passed_over {
            said.push_str(&format!(" · {why}"));
        }
        said
    }

    /// `window 7d`, `window 7d · over the last 24 readings` where those
    /// reach further back (§FS-021-monitors.18.5), or `window n/a: series`.
    pub fn window_says(&self) -> String {
        match (self.windowed, self.row.shows.span.by_readings) {
            (false, _) => "window n/a: series".to_string(),
            (true, true) => format!("window {} · over the last 24 readings", self.window.label()),
            (true, false) => format!("window {}", self.window.label()),
        }
    }

    /// The first line under the title (§FS-021-monitors.19): the health as
    /// the health column says it, then since when the health has held,
    /// `✖ down 2h · down since 2026-11-25 10:00`.
    pub fn health_says(&self, when: impl Fn(DateTime<Utc>) -> String) -> String {
        let row = &self.row;
        let mut said = format!("{} {}", row.shows.glyph, row.shows.health.text());
        if let (Some(health), Some(since)) = (row.health, row.since) {
            said.push_str(&format!(" · {} since {}", health.name(), when(since)));
        }
        said
    }
}

/// Where a monitor is read from (§FS-021-monitors.5).
#[derive(Debug, Clone, Serialize)]
pub struct FromView {
    pub source: Source,
    /// Where its command runs or its file is read
    /// (§FS-021-monitors.5.3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runs_in: Option<PathBuf>,
    /// The home that said it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home: Option<Home>,
}

/// Where a monitor is declared (§FS-021-monitors.4.4).
#[derive(Debug, Clone, Serialize)]
pub struct DeclaredView {
    /// Every home that says something of it.
    pub homes: Vec<Home>,
    /// The fields the site names where the manifest also does.
    pub site_overrides: Vec<String>,
}

impl DeclaredView {
    /// `site, manifest · the site overrides healthy, warn`.
    pub fn says(&self) -> String {
        let homes: Vec<&str> = self.homes.iter().map(|home| home_name(*home)).collect();
        let mut said = match homes.is_empty() {
            true => "by the answer that brings it".to_string(),
            false => homes.join(", "),
        };
        if !self.site_overrides.is_empty() {
            said.push_str(&format!(
                " · the site overrides {}",
                self.site_overrides.join(", ")
            ));
        }
        said
    }
}

/// The hold (§FS-021-monitors.10.1).
#[derive(Debug, Clone, Serialize)]
pub struct HoldView {
    pub drop_after: u32,
    pub recover_after: u32,
}

impl HoldView {
    /// `drop after 1 failing reading · recover after 2 passing readings`.
    pub fn says(&self) -> String {
        let readings = |count: u32, how: &str| match count {
            1 => format!("1 {how} reading"),
            count => format!("{count} {how} readings"),
        };
        format!(
            "drop after {} · recover after {}",
            readings(self.drop_after, "failing"),
            readings(self.recover_after, "passing")
        )
    }
}

/// One point of the chart.
#[derive(Debug, Clone, Serialize)]
pub struct ChartPoint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    /// On the chart's scale: a number, a level's place, a boolean's 1 or 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// Judged under today's condition (§FS-021-monitors.18.9).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<Health>,
}

/// A threshold line: one bound of today's `healthy` or `warn`.
#[derive(Debug, Clone, Serialize)]
pub struct Threshold {
    pub at: f64,
    /// `healthy` or `warn`.
    pub of: &'static str,
    /// `min` or `max`.
    pub bound: &'static str,
}

/// One episode inside the window (§FS-021-monitors.11).
#[derive(Debug, Clone, Serialize)]
pub struct EpisodeView {
    pub sort: Sort,
    pub start: DateTime<Utc>,
    /// Absent while it is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<DateTime<Utc>>,
    /// Its matter's key.
    pub matter: String,
    /// How it ended, as its finished matter says it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended: Option<String>,
}

/// One of the last readings (§FS-021-monitors.19).
#[derive(Debug, Clone, Serialize)]
pub struct ReadingView {
    pub t: DateTime<Utc>,
    /// The value in words, `—` for a read that failed.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<Health>,
    /// `read out of cadence`, `the read failed`, `12 readings thinned`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
