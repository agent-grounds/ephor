//! A monitor's episodes as the feed's matters (§FS-021-monitors.11,
//! §FS-021-monitors.12): one per open drop and open unknown episode, and one
//! per finished episode the recency window still keeps
//! (§FS-021-monitors.14.3). The slot reports them on every plain refresh,
//! built from the store, so what the feed shows is what the store says.
//!
//! What only the monitor knows rides on the item under one record ephor
//! writes, [`RECORD`], read back only for the monitor slot's own source, so
//! no other source can state a monitor's finality or its movement
//! (§FS-003-feed-categories.2, §FS-006-project-interface.4).

use std::collections::BTreeMap;

use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

use crate::feed::model::{Item, ItemKind, META};
use crate::monitors::episode::{self, Ending, Movement, Sort};
use crate::monitors::freshness::Cause;
use crate::monitors::model::Proposal;
use crate::monitors::overlay::Monitor;
use crate::monitors::state::{Document, Moved, OpenDrop, OpenUnknown, Past, Shown};
use crate::monitors::words;

/// The source a monitor's matter is reported by: the monitor slot's own
/// name, which no configured source may take (§FS-021-monitors.17.4).
pub const SOURCE: &str = "monitor";

/// Where on the item what only the monitor knows is kept.
pub const RECORD: &str = "_ephor_monitor";

/// What a monitor's matter states of itself (§FS-021-monitors.12).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stated {
    /// Its episode ended (§FS-021-monitors.14).
    pub terminal: bool,
    #[serde(default)]
    pub moves: Moves,
    /// When it last turned New again: still down, or still unknown, after
    /// the work laid on it ended (§FS-021-monitors.13.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renewed: Option<DateTime<Utc>>,
    /// The monitor's own `workflow` or `action`, on an open drop
    /// (§FS-021-monitors.16.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposes: Option<Proposal>,
}

/// The whole of what moves a monitor's matter (§FS-021-monitors.13.1), and
/// nothing else: the worst level its drop reached, the condition judging it
/// in words, and a mark for each work laid on it that ended. A value moving
/// inside its band is none of these.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moves {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub condition: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ended: Vec<String>,
    /// What still being down or unknown after its work ended says, in the
    /// episode's own words (§FS-021-monitors.15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub still: Option<String>,
}

impl Moves {
    /// Why the matter moved since `previous`, as the sentences the
    /// resurfaced matter and the stale work name (§FS-021-monitors.13.2):
    /// `level worse: degraded → failed`, `condition changed: healthy ≥ 80% →
    /// healthy ≥ 85%`, `still down after its work ended`.
    pub fn since(&self, previous: &Moves) -> Vec<String> {
        let mut said = Vec::new();
        if let (Some(from), Some(to)) = (&previous.worst, &self.worst) {
            if from != to {
                let (from, to) = (from.clone(), to.clone());
                said.push(Movement::LevelWorse { from, to }.say());
            }
        }
        if !previous.condition.is_empty()
            && !self.condition.is_empty()
            && previous.condition != self.condition
        {
            let (from, to) = (previous.condition.clone(), self.condition.clone());
            said.push(Movement::ConditionChanged { from, to }.say());
        }
        if self.ended.iter().any(|work| !previous.ended.contains(work)) {
            said.push(
                self.still
                    .clone()
                    .unwrap_or_else(|| Movement::StillDown.say()),
            );
        }
        said
    }
}

/// What the monitor slot's matter states of itself. None for every other
/// source, whatever it carries at the same key (§FS-003-feed-categories.2).
pub fn stated(source: &str, raw: &Json) -> Option<Stated> {
    if source != SOURCE {
        return None;
    }
    serde_json::from_value(raw.get(RECORD)?.clone()).ok()
}

/// The matters of one project's monitors at `now`: every open episode, and
/// every finished one, forgotten monitors' included, that ended within the
/// last `recent_days` (§FS-021-monitors.14.3). `monitors` gives each its
/// proposal (§FS-021-monitors.16.1); times are said on `zone`'s clock.
pub fn items<Tz>(
    project: &str,
    document: &Document,
    monitors: &BTreeMap<String, Monitor>,
    now: DateTime<Utc>,
    recent_days: u64,
    zone: &Tz,
) -> Vec<Item>
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    let from = now - TimeDelta::days(i64::try_from(recent_days).unwrap_or(7));
    let mut items = Vec::new();
    for (id, record) in &document.monitors {
        let proposes = monitors
            .get(id)
            .and_then(|monitor| monitor.proposal.clone());
        if let Some(drop) = &record.drop {
            items.push(open_drop(project, id, drop, proposes));
        }
        if let Some(unknown) = &record.unknown {
            items.push(open_unknown(project, id, unknown, zone));
        }
    }
    let pasts = document
        .monitors
        .iter()
        .map(|(id, record)| (id, &record.past))
        .chain(document.forgotten.iter());
    for (id, pasts) in pasts {
        for past in pasts.iter().filter(|past| past.ended >= from) {
            items.push(finished(project, id, past, zone));
        }
    }
    items
}

fn open_drop(project: &str, id: &str, drop: &OpenDrop, proposes: Option<Proposal>) -> Item {
    let shown = &drop.shown;
    let mut title = format!(
        "{} dropped: {} ({})",
        shown.title,
        said(shown),
        shown.condition
    );
    if let Some(detail) = &shown.detail {
        title = format!("{title} · {detail}");
    }
    let stated = Stated {
        terminal: false,
        moves: Moves {
            worst: drop.worst.clone(),
            condition: drop.judged_words.clone(),
            ended: drop.works_ended.clone(),
            still: still(drop.moved.as_ref()),
        },
        renewed: renewed(drop.moved.as_ref()),
        proposes,
    };
    item(
        project,
        (episode::key(project, id, Sort::Drop, drop.named), id),
        (title, "down"),
        last_moved(drop.named, drop.moved.as_ref()),
        shown,
        None,
        stated,
    )
}

fn open_unknown<Tz>(project: &str, id: &str, unknown: &OpenUnknown, zone: &Tz) -> Item
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    let stated = Stated {
        terminal: false,
        moves: Moves {
            ended: unknown.works_ended.clone(),
            still: still(unknown.moved.as_ref()),
            ..Moves::default()
        },
        renewed: renewed(unknown.moved.as_ref()),
        proposes: None,
    };
    item(
        project,
        (episode::key(project, id, Sort::Unknown, unknown.named), id),
        (
            cannot(&unknown.shown.title, &unknown.cause, zone),
            "unknown",
        ),
        last_moved(unknown.named, unknown.moved.as_ref()),
        &unknown.shown,
        Some(&unknown.cause),
        stated,
    )
}

fn finished<Tz>(project: &str, id: &str, past: &Past, zone: &Tz) -> Item
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    let state = match &past.how {
        Ending::Recovered { .. } => "recovered",
        Ending::ReadAgain { .. } => "readable",
        Ending::Unwatched { .. } => "unwatched",
    };
    let title = format!("{} {}", past.shown.title, past.how.say(past.ended, zone));
    item(
        project,
        (episode::key(project, id, past.sort, past.named), id),
        (title, state),
        past.ended,
        &past.shown,
        past.cause.as_ref(),
        Stated {
            terminal: true,
            ..Stated::default()
        },
    )
}

fn item(
    project: &str,
    (id, monitor): (String, &str),
    (title, state): (String, &str),
    updated_at: DateTime<Utc>,
    shown: &Shown,
    cause: Option<&Cause>,
    stated: Stated,
) -> Item {
    let mut raw = Map::new();
    raw.insert(META.to_string(), Json::Object(meta(monitor, shown, cause)));
    raw.insert(
        RECORD.to_string(),
        serde_json::to_value(stated).unwrap_or_default(),
    );
    Item {
        id,
        project: project.to_string(),
        source: SOURCE.to_string(),
        kind: ItemKind::Status,
        role: None,
        title,
        // Its link is the sample's (§FS-021-monitors.12.2).
        url: shown.url.clone(),
        state: Some(state.to_string()),
        needs_response: false,
        updated_at,
        raw: Json::Object(raw),
    }
}

/// When the matter last moved: its latest move, else when its episode
/// opened (§FS-021-monitors.12.2).
fn last_moved(named: DateTime<Utc>, moved: Option<&Moved>) -> DateTime<Utc> {
    moved.map_or(named, |moved| moved.at.max(named))
}

fn renewed(moved: Option<&Moved>) -> Option<DateTime<Utc>> {
    moved
        .filter(|moved| moved.why.renews())
        .map(|moved| moved.at)
}

fn still(moved: Option<&Moved>) -> Option<String> {
    moved
        .filter(|moved| moved.why.renews())
        .map(|moved| moved.why.say())
}

/// The value the matter shows, as the page says it.
fn said(shown: &Shown) -> String {
    shown.value.as_ref().map_or_else(
        || "?".to_string(),
        |value| words::value(value, shown.unit.as_deref(), shown.of),
    )
}

/// An unknown matter's title, naming its cause (§FS-021-monitors.12.1).
fn cannot<Tz>(title: &str, cause: &Cause, zone: &Tz) -> String
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    let day = |at: DateTime<Utc>| at.with_timezone(zone).format("%b %-d").to_string();
    let minute = |at: DateTime<Utc>| at.with_timezone(zone).format("%b %-d %H:%M").to_string();
    match cause {
        Cause::Unreadable { why } => format!("{title} cannot be read: {why}"),
        Cause::Stale { measured: Some(at) } => format!("{title} is stale: measured {}", day(*at)),
        Cause::Stale { measured: None } => {
            format!("{title} is stale: nothing new was measured")
        }
        Cause::Parked { since } => format!(
            "{title} is parked: every read since {} exited 75",
            minute(*since)
        ),
        Cause::Overdue => format!("{title} is overdue"),
    }
}

/// The matter's facts, so what takes it does not find them again
/// (§FS-021-monitors.12.3).
fn meta(id: &str, shown: &Shown, cause: Option<&Cause>) -> Map<String, Json> {
    let mut meta = Map::new();
    let mut put = |key: &str, value: Option<String>| {
        if let Some(value) = value {
            meta.insert(key.to_string(), Json::String(value));
        }
    };
    put("monitor", Some(id.to_string()));
    put(
        "value",
        shown
            .value
            .as_ref()
            .map(|value| words::value(value, None, shown.of)),
    );
    put("unit", shown.unit.clone());
    put(
        "condition",
        Some(shown.condition.clone()).filter(|words| !words.is_empty()),
    );
    put("since", shown.since.map(|at| at.to_rfc3339()));
    put(
        "last_ok_value",
        shown
            .last_ok
            .as_ref()
            .map(|kept| words::value(&kept.v, None, kept.of)),
    );
    put(
        "last_ok_at",
        shown.last_ok.as_ref().map(|kept| kept.t.to_rfc3339()),
    );
    put("detail", shown.detail.clone());
    put("url", shown.url.clone());
    put("cause", cause.map(|cause| cause.name().to_string()));
    meta
}

#[cfg(test)]
#[path = "matters_tests.rs"]
mod tests;
