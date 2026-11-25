//! One declaration or one sample, read from its JSON with a sentence for each
//! way it can be wrong (§FS-021-monitors.4.6, §FS-021-monitors.8.2).
//!
//! The manifest and answer schemas accept any entry in `monitors` and publish
//! the strict shape of one entry beside it (§FS-021-monitors.8.4). This holds
//! each entry to that shape on its own, so one entry that does not fit costs
//! its monitor and nothing else, and says what is wrong in words the page can
//! show rather than in a schema path.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, FixedOffset};
use serde_json::{Map, Value as Json};

use crate::monitors::model::{
    Better, Condition, Declaration, Description, Home, Kind, Point, Sample, Source, Stated, Value,
};

/// The keys a declaration knows (§FS-021-monitors.4.1).
const DECLARATION_KEYS: &[&str] = &[
    "id",
    "title",
    "description",
    "kind",
    "unit",
    "better",
    "of",
    "levels",
    "healthy",
    "warn",
    "from",
    "every",
    "at",
    "timeout",
    "drop_after",
    "recover_after",
    "workflow",
    "action",
];

/// The keys a condition knows (§FS-021-monitors.3.1).
const CONDITION_KEYS: &[&str] = &["is", "min", "max", "in", "at_most"];

/// A declaration the shape refuses: its id where one could be read, and every
/// sentence that says what is wrong (§FS-021-monitors.4.6).
#[derive(Debug, Clone, PartialEq)]
pub struct Refused {
    /// Where it stands in its home's `monitors`, for one with no id to name.
    pub index: usize,
    pub id: Option<String>,
    pub faults: Vec<String>,
}

/// One home's declarations, each checked on its own (§FS-021-monitors.4.6).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Declared {
    /// The declarations that fit the shape, by id, in the order written.
    pub declarations: Vec<Declaration>,
    /// The ones that did not, and every id declared more than once: such an
    /// id is not read until one of its declarations goes.
    pub refused: Vec<Refused>,
}

impl Declared {
    pub fn get(&self, id: &str) -> Option<&Declaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.id == id)
    }

    /// The faults that keep `id` from being read, where this home has any.
    pub fn faults(&self, id: &str) -> Vec<String> {
        self.refused
            .iter()
            .filter(|refused| refused.id.as_deref() == Some(id))
            .flat_map(|refused| refused.faults.iter().cloned())
            .collect()
    }
}

/// Whether a text is a monitor id: lowercase letters, digits and `-`,
/// starting with a letter or a digit (§FS-021-monitors.1.1).
pub fn is_id(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Reads one home's `monitors`, entry by entry, and refuses every id it
/// declares twice (§FS-021-monitors.4.6).
pub fn read_declarations(entries: &[Json], home: Home) -> Declared {
    let mut read = Vec::new();
    let mut refused = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        match read_declaration(entry) {
            Ok(declaration) => read.push((index, declaration)),
            Err(mut fault) => {
                fault.index = index;
                refused.push(fault);
            }
        }
    }
    let mut seen = BTreeMap::<String, usize>::new();
    for (_, declaration) in &read {
        *seen.entry(declaration.id.clone()).or_default() += 1;
    }
    for refusal in &refused {
        if let Some(id) = &refusal.id {
            *seen.entry(id.clone()).or_default() += 1;
        }
    }
    let twice: BTreeSet<String> = seen
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(id, _)| id)
        .collect();
    let mut declarations = Vec::new();
    for (index, declaration) in read {
        if twice.contains(&declaration.id) {
            refused.push(Refused {
                index,
                id: Some(declaration.id.clone()),
                faults: vec![format!(
                    "{} is declared more than once in {}",
                    declaration.id,
                    home.phrase()
                )],
            });
        } else {
            declarations.push(declaration);
        }
    }
    refused.sort_by_key(|refusal| refusal.index);
    Declared {
        declarations,
        refused,
    }
}

/// Reads one declaration and holds it to the published shape
/// (§FS-021-monitors.4.1, §FS-021-monitors.4.6). A key it does not know is
/// noted and read without, so a misspelt `healthy` is seen.
pub fn read_declaration(entry: &Json) -> Result<Declaration, Refused> {
    let Some(map) = entry.as_object() else {
        return Err(Refused {
            index: 0,
            id: None,
            faults: vec![format!(
                "a monitor is declared as an object, not {}",
                json_type(entry)
            )],
        });
    };
    let mut fields = Fields::new(map);
    let id = match map.get("id") {
        Some(Json::String(id)) if is_id(id) => Some(id.clone()),
        Some(other) => {
            fields.fault(format!(
                "id {other} is not a monitor id: lowercase letters, digits and '-', \
                 starting with a letter or a digit"
            ));
            None
        }
        None => {
            fields.fault("a monitor's declaration needs an id".to_string());
            None
        }
    };
    let declaration = Declaration {
        id: id.clone().unwrap_or_default(),
        title: fields.title("title"),
        description: fields.string("description"),
        kind: fields.kind("kind"),
        unit: fields.string("unit"),
        better: fields.better("better"),
        of: fields.number("of"),
        levels: fields.levels("levels"),
        healthy: fields.nullable("healthy", Fields::condition),
        warn: fields.nullable("warn", Fields::condition),
        from: fields.nullable("from", Fields::source),
        every: fields.every("every"),
        at: fields.nullable("at", Fields::at),
        timeout: fields.whole("timeout", "a whole number of seconds"),
        drop_after: fields.whole("drop_after", "a whole number").map(clamp),
        recover_after: fields.whole("recover_after", "a whole number").map(clamp),
        workflow: fields.nullable("workflow", Fields::name),
        action: fields.nullable("action", Fields::name),
        notes: Vec::new(),
    };
    if map.contains_key("workflow") && map.contains_key("action") {
        fields.fault("workflow and action are one choice; name one of them".to_string());
    }
    let mut notes: Vec<String> = map
        .keys()
        .filter(|key| !DECLARATION_KEYS.contains(&key.as_str()))
        .map(|key| format!("unknown field {key}"))
        .collect();
    notes.append(&mut fields.notes);
    if fields.faults.is_empty() {
        Ok(Declaration {
            notes,
            ..declaration
        })
    } else {
        Err(Refused {
            index: 0,
            id,
            faults: fields.faults,
        })
    }
}

fn clamp(whole: u64) -> u32 {
    u32::try_from(whole).unwrap_or(u32::MAX)
}

/// Why a sample was read for no monitor, or why it is an unreadable read of
/// the one it names (§FS-021-monitors.8.2).
#[derive(Debug, Clone, PartialEq)]
pub struct SampleFault {
    /// The monitor it is an unreadable read of; none where the sample names
    /// no monitor, and is noted on the refresh instead.
    pub id: Option<String>,
    pub sentence: String,
}

/// Reads one sample and holds it to the published shape
/// (§FS-021-monitors.8.1, §FS-021-monitors.8.2). Whether its value fits its
/// monitor is [`crate::monitors::sample`]'s, because only the monitor knows
/// its kind.
pub fn read_sample(entry: &Json) -> Result<Sample, SampleFault> {
    let id = sample_id(entry)?;
    let map = entry.as_object().expect("sample_id read an object");
    let mut fields = Fields::new(map);
    let value = match map.get("value") {
        None => None,
        Some(Json::Bool(value)) => Some(Value::Boolean(*value)),
        Some(Json::Number(number)) => number.as_f64().map(Value::Number),
        Some(Json::String(level)) => Some(Value::Level(level.clone())),
        Some(other) => {
            fields.fault(format!(
                "the sample's value is {}, not true, false, a number or a level",
                json_type(other)
            ));
            None
        }
    };
    let sample = Sample {
        id: id.clone(),
        value,
        of: fields.number("of"),
        at: fields.time("at"),
        ok: fields.boolean("ok"),
        detail: fields.string("detail"),
        url: fields.string("url"),
        points: fields.points("points"),
        description: Description {
            title: fields.title("title"),
            kind: fields.kind("kind"),
            unit: fields.string("unit"),
            better: fields.better("better"),
            levels: fields.levels("levels"),
            healthy: fields.nullable("healthy", Fields::condition),
            warn: fields.condition_of("warn"),
            description: fields.string("description"),
        },
    };
    match fields.faults.into_iter().next() {
        None => Ok(sample),
        Some(sentence) => Err(SampleFault {
            id: Some(id),
            sentence,
        }),
    }
}

/// The id a sample is for, or why it is for none (§FS-021-monitors.8.2).
pub fn sample_id(entry: &Json) -> Result<String, SampleFault> {
    let none = |sentence: String| SampleFault { id: None, sentence };
    let Some(map) = entry.as_object() else {
        return Err(none(format!(
            "a sample is an object, not {}, so it is read for no monitor",
            json_type(entry)
        )));
    };
    match map.get("id") {
        Some(Json::String(id)) if is_id(id) => Ok(id.clone()),
        Some(other) => Err(none(format!(
            "a sample's id {other} is not a monitor id, so it is read for no monitor"
        ))),
        None => Err(none(
            "a sample with no id is read for no monitor".to_string(),
        )),
    }
}

fn json_type(value: &Json) -> &'static str {
    match value {
        Json::Null => "null",
        Json::Bool(_) => "true or false",
        Json::Number(_) => "a number",
        Json::String(_) => "a string",
        Json::Array(_) => "a list",
        Json::Object(_) => "an object",
    }
}

/// One entry's fields, read one at a time, each fault said once.
struct Fields<'a> {
    map: &'a Map<String, Json>,
    faults: Vec<String>,
    notes: Vec<String>,
}

impl<'a> Fields<'a> {
    fn new(map: &'a Map<String, Json>) -> Self {
        Fields {
            map,
            faults: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn fault(&mut self, sentence: String) {
        self.faults.push(sentence);
    }

    fn string(&mut self, key: &str) -> Option<String> {
        match self.map.get(key)? {
            Json::String(text) => Some(text.clone()),
            other => {
                self.fault(format!("{key} is {}, not a string", json_type(other)));
                None
            }
        }
    }

    fn title(&mut self, key: &str) -> Option<String> {
        let title = self.string(key)?;
        if title.is_empty() {
            self.fault(format!("{key} is empty"));
            return None;
        }
        Some(title)
    }

    fn boolean(&mut self, key: &str) -> Option<bool> {
        match self.map.get(key)? {
            Json::Bool(value) => Some(*value),
            other => {
                self.fault(format!("{key} is {}, not true or false", json_type(other)));
                None
            }
        }
    }

    fn number(&mut self, key: &str) -> Option<f64> {
        match self.map.get(key)? {
            Json::Number(number) => number.as_f64(),
            other => {
                self.fault(format!("{key} is {}, not a number", json_type(other)));
                None
            }
        }
    }

    fn whole(&mut self, key: &str, what: &str) -> Option<u64> {
        let value = self.map.get(key)?;
        let whole = value.as_u64().or_else(|| {
            value
                .as_f64()
                .filter(|number| number.fract() == 0.0 && *number >= 0.0)
                .map(|number| number as u64)
        });
        match whole {
            Some(whole) if whole >= 1 => Some(whole),
            _ => {
                self.fault(format!("{key} is {value}, not {what} of at least 1"));
                None
            }
        }
    }

    fn kind(&mut self, key: &str) -> Option<Kind> {
        let value = self.map.get(key)?;
        match value.as_str().and_then(Kind::parse) {
            Some(kind) => Some(kind),
            None => {
                self.fault(format!(
                    "{key} {value} is not boolean, number, level or series"
                ));
                None
            }
        }
    }

    fn better(&mut self, key: &str) -> Option<Better> {
        let value = self.map.get(key)?;
        match value.as_str() {
            Some("higher") => Some(Better::Higher),
            Some("lower") => Some(Better::Lower),
            _ => {
                self.fault(format!("{key} {value} is not higher or lower"));
                None
            }
        }
    }

    fn levels(&mut self, key: &str) -> Option<Vec<String>> {
        let value = self.map.get(key)?;
        let levels: Option<Vec<String>> = value.as_array().and_then(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .filter(|name| !name.is_empty())
                        .map(String::from)
                })
                .collect()
        });
        match levels {
            Some(levels)
                if !levels.is_empty()
                    && levels.iter().collect::<BTreeSet<_>>().len() == levels.len() =>
            {
                Some(levels)
            }
            _ => {
                self.fault(format!(
                    "{key} must be a list of distinct level names, best first, such as \
                     [\"clean\", \"degraded\", \"failed\"]"
                ));
                None
            }
        }
    }

    /// A field a home may set aside with `null` (§FS-021-monitors.4.4).
    fn nullable<T>(
        &mut self,
        key: &str,
        read: impl FnOnce(&mut Self, &str, &Json) -> Option<T>,
    ) -> Stated<T> {
        match self.map.get(key) {
            None => Stated::Unsaid,
            Some(Json::Null) => Stated::Null,
            Some(value) => match read(self, key, value) {
                Some(read) => Stated::Said(read),
                None => Stated::Unsaid,
            },
        }
    }

    fn condition_of(&mut self, key: &str) -> Option<Condition> {
        let value = self.map.get(key)?;
        self.condition(key, value)
    }

    /// A condition in the manifest's language (§FS-021-monitors.3.1). An
    /// empty one is the shape's to refuse; a key the language does not know
    /// is kept on the condition for the monitor that uses it to refuse.
    fn condition(&mut self, key: &str, value: &Json) -> Option<Condition> {
        let Some(map) = value.as_object() else {
            self.fault(format!(
                "{key} is {}, not a condition such as {{\"min\": 80}}",
                json_type(value)
            ));
            return None;
        };
        if map.is_empty() {
            self.fault(format!(
                "{key} is empty: a condition holds is, min, max, in or at_most"
            ));
            return None;
        }
        let before = self.faults.len();
        let mut condition = Condition {
            unknown: map
                .keys()
                .filter(|name| !CONDITION_KEYS.contains(&name.as_str()))
                .cloned()
                .collect(),
            ..Condition::default()
        };
        for (name, value) in map {
            match (name.as_str(), value) {
                ("is", Json::Bool(is)) => condition.is = Some(*is),
                ("min", Json::Number(min)) => condition.min = min.as_f64(),
                ("max", Json::Number(max)) => condition.max = max.as_f64(),
                ("at_most", Json::String(level)) => condition.at_most = Some(level.clone()),
                ("in", Json::Array(items)) if !items.is_empty() => {
                    match items
                        .iter()
                        .map(|item| item.as_str().map(String::from))
                        .collect()
                    {
                        Some(levels) => condition.within = Some(levels),
                        None => self.fault(format!("{key}'s in must list level names")),
                    }
                }
                ("is", _) => self.fault(format!("{key}'s is must be true or false")),
                ("min" | "max", _) => self.fault(format!("{key}'s {name} must be a number")),
                ("at_most", _) => self.fault(format!("{key}'s at_most must be a level name")),
                ("in", _) => self.fault(format!("{key}'s in must list level names")),
                _ => {}
            }
        }
        (self.faults.len() == before).then_some(condition)
    }

    /// `{"command": …}` or `{"file": …}`, with an optional `cwd`
    /// (§FS-021-monitors.5, §FS-021-monitors.5.3).
    fn source(&mut self, key: &str, value: &Json) -> Option<Source> {
        let Some(map) = value.as_object() else {
            self.fault(format!(
                "{key} is {}, not {{\"command\": …}} or {{\"file\": …}}",
                json_type(value)
            ));
            return None;
        };
        let text = |name: &str| {
            map.get(name)
                .map(|value| value.as_str().filter(|s| !s.is_empty()))
        };
        let cwd = match map.get("cwd") {
            None => None,
            Some(Json::String(cwd)) if is_cwd(cwd) => Some(cwd.clone()),
            Some(other) => {
                self.fault(format!(
                    "{key}'s cwd {other} is not 'root' or 'repo:<name>'"
                ));
                return None;
            }
        };
        for name in map.keys() {
            if !["command", "file", "cwd"].contains(&name.as_str()) {
                self.notes.push(format!("unknown field {key}.{name}"));
            }
        }
        match (text("command"), text("file")) {
            (Some(_), Some(_)) => {
                self.fault(format!("{key} names a command or a file, not both"));
                None
            }
            (Some(Some(command)), None) => Some(Source::Command {
                command: command.to_string(),
                cwd,
            }),
            (None, Some(Some(file))) => Some(Source::File {
                file: file.to_string(),
                cwd,
            }),
            (Some(None), None) => {
                self.fault(format!("{key}'s command must be a non-empty string"));
                None
            }
            (None, Some(None)) => {
                self.fault(format!("{key}'s file must be a non-empty string"));
                None
            }
            (None, None) => {
                self.fault(format!("{key} must name a command or a file"));
                None
            }
        }
    }

    /// `every`, held to the shape a duration has: a whole number from 1 and a
    /// unit, `m`, `h`, `d` or `w` (§FS-021-monitors.6.1). What it is as a
    /// length is [`crate::monitors::cadence`]'s.
    fn every(&mut self, key: &str) -> Option<String> {
        let value = self.map.get(key)?;
        match value.as_str().filter(|text| is_duration(text)) {
            Some(text) => Some(text.to_string()),
            None => {
                self.fault(format!(
                    "{key} {value} is not a duration: a whole number and m, h, d or w, \
                     such as 10m, 4h, 1d or 1w"
                ));
                None
            }
        }
    }

    /// `HH:MM`, or a day and a time such as `Mon 06:00` (§FS-021-monitors.6.1).
    fn at(&mut self, key: &str, value: &Json) -> Option<String> {
        match value
            .as_str()
            .filter(|text| crate::monitors::cadence::At::parse(text).is_some())
        {
            Some(text) => Some(text.to_string()),
            None => {
                self.fault(format!(
                    "{key} {value} is not a time of day: HH:MM, or a day and a time \
                     such as Mon 06:00"
                ));
                None
            }
        }
    }

    fn name(&mut self, key: &str, value: &Json) -> Option<String> {
        match value.as_str().filter(|text| !text.is_empty()) {
            Some(text) => Some(text.to_string()),
            None => {
                self.fault(format!("{key} {value} is not a name, nor null"));
                None
            }
        }
    }

    /// A time with its offset, which the schema only annotates and code must
    /// read (§FS-021-monitors.8.1).
    fn time(&mut self, key: &str) -> Option<DateTime<FixedOffset>> {
        let value = self.map.get(key)?;
        match value
            .as_str()
            .and_then(|text| DateTime::parse_from_rfc3339(text).ok())
        {
            Some(time) => Some(time),
            None => {
                self.fault(format!(
                    "the sample's {key} {value} is not a time with its offset, such as \
                     2026-11-25T05:40:00Z"
                ));
                None
            }
        }
    }

    /// A series' points or a number's history: every point `{t, v}` or every
    /// point `{x, v}`, and at least one (§FS-021-monitors.2.4).
    fn points(&mut self, key: &str) -> Option<Vec<Point>> {
        let value = self.map.get(key)?;
        let Some(items) = value.as_array().filter(|items| !items.is_empty()) else {
            self.fault(format!("the sample's {key} must list at least one point"));
            return None;
        };
        let mut points = Vec::new();
        for item in items {
            let point = item.as_object().and_then(|point| {
                let v = point.get("v")?.as_f64()?;
                match (point.get("t"), point.get("x")) {
                    (Some(t), None) => Some(Point {
                        t: Some(DateTime::parse_from_rfc3339(t.as_str()?).ok()?),
                        x: None,
                        v,
                    }),
                    (None, Some(x)) => Some(Point {
                        t: None,
                        x: Some(x.as_str()?.to_string()),
                        v,
                    }),
                    _ => None,
                }
            });
            match point {
                Some(point) => points.push(point),
                None => {
                    self.fault(format!(
                        "the sample's point {item} is not {{t, v}} or {{x, v}}, with t a time \
                         and v a number"
                    ));
                    return None;
                }
            }
        }
        let timed = points.iter().filter(|point| point.t.is_some()).count();
        if timed != 0 && timed != points.len() {
            self.fault("the sample's points mix t and x; one sample uses one".to_string());
            return None;
        }
        Some(points)
    }
}

/// The shape `every` has: `^[1-9][0-9]*[mhdw]$` (§FS-021-monitors.6.1).
fn is_duration(text: &str) -> bool {
    let Some(count) = text.strip_suffix(['m', 'h', 'd', 'w']) else {
        return false;
    };
    count.starts_with(|c: char| ('1'..='9').contains(&c))
        && count.chars().all(|c| c.is_ascii_digit())
}

/// `root`, or `repo:<name>` (§FS-021-monitors.5.3).
fn is_cwd(text: &str) -> bool {
    text == "root"
        || text.strip_prefix("repo:").is_some_and(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        })
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
