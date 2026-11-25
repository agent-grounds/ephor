//! What a monitor and a sample are, in the shapes the manifest and answer
//! schemas publish (§FS-021-monitors.4.1, §FS-021-monitors.8.1).
//!
//! These are the values once read. Reading them from JSON, with a sentence for
//! each way an entry can be wrong, is [`crate::monitors::shape`]; serializing
//! them writes the published shape back, so a declaration or a sample kept in
//! the store reads as what the project wrote.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize, Serializer};

/// What a monitor's value is (§FS-021-monitors.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Boolean,
    Number,
    Level,
    Series,
}

impl Kind {
    /// The word the schema and every sentence spell it with.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Boolean => "boolean",
            Kind::Number => "number",
            Kind::Level => "level",
            Kind::Series => "series",
        }
    }

    /// Reads the schema's word for a kind.
    pub fn parse(word: &str) -> Option<Kind> {
        match word {
            "boolean" => Some(Kind::Boolean),
            "number" => Some(Kind::Number),
            "level" => Some(Kind::Level),
            "series" => Some(Kind::Series),
            _ => None,
        }
    }

    /// Whether a condition on this kind is written in numeric bounds
    /// (§FS-021-monitors.3.1): a number's value and a series' last point.
    pub fn is_numeric(self) -> bool {
        matches!(self, Kind::Number | Kind::Series)
    }
}

/// Which direction of a number is better (§FS-021-monitors.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Better {
    Higher,
    Lower,
}

/// One value: a boolean, a number, or a level's name (§FS-021-monitors.2).
/// A series' value is its last point's, so it is a number here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Boolean(bool),
    Number(f64),
    Level(String),
}

impl Value {
    /// The kind a value states by itself where nothing else does
    /// (§FS-021-monitors.4.3): a string is a level only where levels are
    /// known, which the caller decides.
    pub fn kind(&self) -> Kind {
        match self {
            Value::Boolean(_) => Kind::Boolean,
            Value::Number(_) => Kind::Number,
            Value::Level(_) => Kind::Level,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(number) => Some(*number),
            _ => None,
        }
    }
}

/// A condition on the value, in the manifest's language
/// (§FS-021-monitors.3.1). A value meets it when it meets every key it holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// `in`, which Rust keeps for itself.
    #[serde(rename = "in", default, skip_serializing_if = "Option::is_none")]
    pub within: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_most: Option<String>,
    /// Keys the language does not know. The schema lets them through, so they
    /// cost the monitor whose condition holds them, never the configuration
    /// that carries it (§FS-021-monitors.4.6).
    #[serde(skip)]
    pub unknown: Vec<String>,
}

/// A field a home may name, set aside with `null`, or leave unsaid
/// (§FS-021-monitors.4.4). Only presence decides whose a field is: a site's
/// `"warn": null` means no band over whatever the manifest said.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Stated<T> {
    #[default]
    Unsaid,
    Null,
    Said(T),
}

impl<T> Stated<T> {
    pub fn is_unsaid(&self) -> bool {
        matches!(self, Stated::Unsaid)
    }

    /// What it says, where it says anything.
    pub fn said(&self) -> Option<&T> {
        match self {
            Stated::Said(value) => Some(value),
            _ => None,
        }
    }

    /// `None` where unsaid; `Some(None)` where set aside with `null`.
    pub fn stated(&self) -> Option<Option<&T>> {
        match self {
            Stated::Unsaid => None,
            Stated::Null => Some(None),
            Stated::Said(value) => Some(Some(value)),
        }
    }
}

impl<T: Serialize> Serialize for Stated<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Stated::Said(value) => value.serialize(serializer),
            Stated::Unsaid | Stated::Null => serializer.serialize_none(),
        }
    }
}

/// Where a monitor's value comes from (§FS-021-monitors.5). Two are the same
/// when they name the same command or file and the same `cwd`
/// (§FS-021-monitors.5.1), which is this type's equality.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Source {
    /// A command, run as a summons once per read (§FS-021-monitors.5.1).
    Command {
        command: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
    /// A file in the answer shape, read and never run (§FS-021-monitors.5.2).
    File {
        file: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
    },
}

impl Source {
    pub fn is_command(&self) -> bool {
        matches!(self, Source::Command { .. })
    }
}

/// The work proposed for a monitor's drop, one choice of two
/// (§FS-021-monitors.16.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Proposal {
    /// One of the runtime's workflows.
    Workflow(String),
    /// The id of a menu entry that hands work over.
    Action(String),
}

/// Which home a field of a monitor came from (§FS-021-monitors.4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Home {
    Site,
    Manifest,
    Sample,
}

impl Home {
    /// The home as a sentence names it.
    pub fn phrase(self) -> &'static str {
        match self {
            Home::Site => "the site configuration",
            Home::Manifest => "the manifest",
            Home::Sample => "its samples",
        }
    }
}

/// One home's declaration of one monitor, as read (§FS-021-monitors.4.1).
/// Only `id` is required; every other field is the home's to name or not.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Declaration {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub better: Option<Better>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub of: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Stated::is_unsaid")]
    pub healthy: Stated<Condition>,
    #[serde(skip_serializing_if = "Stated::is_unsaid")]
    pub warn: Stated<Condition>,
    #[serde(skip_serializing_if = "Stated::is_unsaid")]
    pub from: Stated<Source>,
    /// As written, such as `1d`; read by [`crate::monitors::cadence`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub every: Option<String>,
    /// As written, such as `Mon 06:00`; read by [`crate::monitors::cadence`].
    #[serde(skip_serializing_if = "Stated::is_unsaid")]
    pub at: Stated<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_after: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recover_after: Option<u32>,
    #[serde(skip_serializing_if = "Stated::is_unsaid")]
    pub workflow: Stated<String>,
    #[serde(skip_serializing_if = "Stated::is_unsaid")]
    pub action: Stated<String>,
    /// Keys the declaration does not know, noted on its row and read without
    /// (§FS-021-monitors.4.6): `unknown field heathy`.
    #[serde(skip)]
    pub notes: Vec<String>,
}

/// What a sample says of its monitor (§FS-021-monitors.4.3): the fields that
/// describe one, which a declaration overrides field by field
/// (§FS-021-monitors.4.4).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Description {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub better: Option<Better>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Stated::is_unsaid")]
    pub healthy: Stated<Condition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn: Option<Condition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One point a repository supplied: `{t, v}` or `{x, v}`
/// (§FS-021-monitors.2.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t: Option<DateTime<FixedOffset>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<String>,
    pub v: f64,
}

impl Point {
    /// Whether two points are the same measurement: the same `t` or `x` and
    /// the same `v` (§FS-021-monitors.2.4).
    pub fn same_measurement(&self, other: &Point) -> bool {
        self.t == other.t && self.x == other.x && self.v == other.v
    }
}

/// One observation of a monitor, as an answer carries it
/// (§FS-021-monitors.8.1). Its measurement is `value`, `of`, `at`, `ok`,
/// `detail`, `url` and `points`; the rest describes the monitor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "serde_json::Value")]
pub struct Sample {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub of: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<DateTime<FixedOffset>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<Point>>,
    #[serde(flatten)]
    pub description: Description,
}

impl TryFrom<serde_json::Value> for Sample {
    type Error = String;

    /// A sample kept whole in the store reads back through the same checks an
    /// answer's does (§FS-021-monitors.9.1).
    fn try_from(value: serde_json::Value) -> Result<Sample, String> {
        crate::monitors::shape::read_sample(&value).map_err(|fault| fault.sentence)
    }
}

/// A reading's own health, before the hold (§FS-021-monitors.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Band {
    Ok,
    Warn,
    Down,
}

impl Band {
    /// Whether the reading passes: `ok` and `warn` both meet `healthy`
    /// (§FS-021-monitors.10.1).
    pub fn passes(self) -> bool {
        !matches!(self, Band::Down)
    }

    pub fn name(self) -> &'static str {
        match self {
            Band::Ok => "ok",
            Band::Warn => "warn",
            Band::Down => "down",
        }
    }
}

/// A status's health, one of four (§FS-021-monitors.1.3). A KPI has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Ok,
    Warn,
    Down,
    Unknown,
}

impl Health {
    /// How bad a health is, worst first, in the page's order
    /// (§FS-021-monitors.18.3): down before unknown before warn before ok.
    pub fn badness(self) -> u8 {
        match self {
            Health::Down => 3,
            Health::Unknown => 2,
            Health::Warn => 1,
            Health::Ok => 0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Health::Ok => "ok",
            Health::Warn => "warn",
            Health::Down => "down",
            Health::Unknown => "unknown",
        }
    }
}

impl From<Band> for Health {
    fn from(band: Band) -> Health {
        match band {
            Band::Ok => Health::Ok,
            Band::Warn => Health::Warn,
            Band::Down => Health::Down,
        }
    }
}
