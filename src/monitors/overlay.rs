//! A monitor resolved from its homes, field by field (§FS-021-monitors.4.4),
//! and the rules that hold only on the whole (§FS-021-monitors.3.5,
//! §FS-021-monitors.6.1).
//!
//! The site's where it names a field, the manifest's otherwise, and the last
//! accepted sample's where neither does: the one resolution order every seam
//! keeps (§REQ-001-boundary.2). `healthy`, `warn` and `from` are each taken
//! whole from the home that names them, and `workflow` and `action` are one
//! choice. Health is judged in the order of §FS-021-monitors.3.6, which is
//! §DF-008-ephor-judges-a-monitors-health.

use std::collections::BTreeMap;

use crate::monitors::cadence::{Cadence, DEFAULT_TIMEOUT_SECONDS};
use crate::monitors::condition::{self, Judge};
use crate::monitors::model::{
    Better, Condition, Declaration, Description, Home, Kind, Proposal, Source, Stated,
};
use crate::monitors::words::{self, Terms};

/// What a monitor's homes say of it (§FS-021-monitors.4).
#[derive(Debug, Clone, Copy, Default)]
pub struct Homes<'a> {
    pub site: Option<&'a Declaration>,
    pub manifest: Option<&'a Declaration>,
    /// What the last accepted sample says of its monitor
    /// (§FS-021-monitors.4.3), or the sample being checked.
    pub sample: Option<&'a Description>,
    /// The kind the monitor's first accepted sample gave it, where nothing
    /// else states one (§FS-021-monitors.4.4).
    pub first_kind: Option<Kind>,
}

/// The `healthy` the first three homes resolve to (§FS-021-monitors.3.6).
#[derive(Debug, Clone, PartialEq)]
pub enum Healthy {
    /// No home names it: the sample's `ok`, then the kind's default, judge.
    Unsaid,
    /// `"healthy": null` in that home: a KPI whatever its samples say.
    Null(Home),
    Said(Condition, Home),
}

/// One monitor, resolved (§FS-021-monitors.4.4).
#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    pub id: String,
    /// Defaults to the id (§FS-021-monitors.4.1).
    pub title: String,
    pub description: Option<String>,
    /// None until a home, its levels, its condition or its first sample says
    /// (§FS-021-monitors.4.4).
    pub kind: Option<Kind>,
    pub unit: Option<String>,
    pub better: Option<Better>,
    /// The declared whole; a sample's own `of` is that reading's
    /// (§FS-021-monitors.2.2).
    pub of: Option<f64>,
    pub levels: Option<Vec<String>>,
    pub healthy: Healthy,
    /// Set aside where `healthy` is null: a KPI has no band
    /// (§FS-021-monitors.3.5).
    pub warn: Option<Condition>,
    /// Absent: read from the answer that brings its samples
    /// (§FS-021-monitors.5.6).
    pub from: Option<Source>,
    /// The effective cadence (§FS-021-monitors.6.2), or the default where
    /// what the homes state cannot pair (that is one of its `faults`).
    pub cadence: Cadence,
    /// Whether any home states `every` or `at`; one that states neither and
    /// names no `from` takes its cadence from the answer that brings it
    /// (§FS-021-monitors.6.8).
    pub cadence_stated: bool,
    /// Seconds a command reading it may run (§FS-021-monitors.5.5).
    pub timeout: u64,
    /// Failing readings in a row that make it down (§FS-021-monitors.10.1).
    pub drop_after: u32,
    /// Passing readings in a row that end a drop, where a home says; see
    /// [`Monitor::recover_after`].
    pub recover_after_stated: Option<u32>,
    /// The work proposed for its drop (§FS-021-monitors.16.1).
    pub proposal: Option<Proposal>,
    /// The home each field a home names came from, for the detail view's
    /// "naming each field the site overrides" (§FS-021-monitors.19).
    pub origins: BTreeMap<&'static str, Home>,
    /// Keys a declaration did not know (§FS-021-monitors.4.6).
    pub notes: Vec<String>,
    /// Every rule it breaks as its homes resolve it (§FS-021-monitors.3.5,
    /// §FS-021-monitors.4.6). A monitor with any is not read.
    pub faults: Vec<String>,
}

impl Monitor {
    /// What judges a reading, the first of these that exists
    /// (§FS-021-monitors.3.6): the site's `healthy`, the manifest's, the one
    /// its samples bring, the sample's own `ok`, and the kind's default. A
    /// `null` in any of the first three stops the search there.
    pub fn judge(&self, sample_ok: Option<bool>) -> Judge {
        match &self.healthy {
            Healthy::Said(condition, _) => Judge::Condition(condition.clone()),
            Healthy::Null(_) => Judge::Kpi,
            Healthy::Unsaid if sample_ok.is_some() => Judge::SampleOk,
            Healthy::Unsaid => match condition::kind_default(self.kind, self.levels.as_deref()) {
                Some(default) => Judge::Condition(default),
                None => Judge::Kpi,
            },
        }
    }

    /// How many passing readings end a drop when it is read at `cadence`:
    /// what a home says, or two under an hour and one at an hour or longer
    /// (§FS-021-monitors.10.1).
    pub fn recover_after(&self, cadence: &Cadence) -> u32 {
        self.recover_after_stated
            .unwrap_or_else(|| cadence.recover_after())
    }

    /// The terms its values are said in, with the whole of the reading at hand.
    pub fn terms(&self, of: Option<f64>) -> Terms<'_> {
        Terms {
            kind: self.kind,
            unit: self.unit.as_deref(),
            of: of.or(self.of),
            levels: self.levels.as_deref(),
        }
    }

    /// Its condition in words, as the page, the command and a matter's title
    /// say it (§FS-021-monitors.3.4).
    pub fn words(&self, sample_ok: Option<bool>, of: Option<f64>) -> String {
        words::condition(&self.judge(sample_ok), self.warn.as_ref(), &self.terms(of))
    }

    /// Whether it is read: a monitor whose declaration breaks a rule is shown
    /// and not read (§FS-021-monitors.4.6).
    pub fn is_readable(&self) -> bool {
        self.faults.is_empty()
    }
}

/// Resolves one monitor from its homes (§FS-021-monitors.4.4), and judges the
/// rules a single home cannot: a condition against its kind
/// (§FS-021-monitors.3.1), a `warn` against its `healthy`
/// (§FS-021-monitors.3.5), and `at` against `every` (§FS-021-monitors.6.1).
pub fn resolve(id: &str, homes: &Homes) -> Monitor {
    let mut origins = BTreeMap::new();
    let site = homes.site;
    let manifest = homes.manifest;
    let sample = homes.sample;

    macro_rules! field {
        ($name:literal, $site:expr, $manifest:expr, $sample:expr) => {{
            let picked = [
                (Home::Site, $site),
                (Home::Manifest, $manifest),
                (Home::Sample, $sample),
            ]
            .into_iter()
            .find_map(|(home, value)| value.map(|value| (home, value)));
            picked.map(|(home, value)| {
                origins.insert($name, home);
                value
            })
        }};
    }

    let title = field!(
        "title",
        site.and_then(|d| d.title.clone()),
        manifest.and_then(|d| d.title.clone()),
        sample.and_then(|d| d.title.clone())
    );
    let description = field!(
        "description",
        site.and_then(|d| d.description.clone()),
        manifest.and_then(|d| d.description.clone()),
        sample.and_then(|d| d.description.clone())
    );
    let stated_kind = field!(
        "kind",
        site.and_then(|d| d.kind),
        manifest.and_then(|d| d.kind),
        sample.and_then(|d| d.kind)
    );
    let unit = field!(
        "unit",
        site.and_then(|d| d.unit.clone()),
        manifest.and_then(|d| d.unit.clone()),
        sample.and_then(|d| d.unit.clone())
    );
    let better = field!(
        "better",
        site.and_then(|d| d.better),
        manifest.and_then(|d| d.better),
        sample.and_then(|d| d.better)
    );
    let levels = field!(
        "levels",
        site.and_then(|d| d.levels.clone()),
        manifest.and_then(|d| d.levels.clone()),
        sample.and_then(|d| d.levels.clone())
    );
    let of = field!(
        "of",
        site.and_then(|d| d.of),
        manifest.and_then(|d| d.of),
        None
    );

    let healthy = match stated(
        site.map(|d| &d.healthy),
        manifest.map(|d| &d.healthy),
        sample.map(|d| &d.healthy),
    ) {
        Some((home, None)) => Healthy::Null(home),
        Some((home, Some(condition))) => Healthy::Said(condition.clone(), home),
        None => Healthy::Unsaid,
    };
    if let Healthy::Null(home) | Healthy::Said(_, home) = &healthy {
        origins.insert("healthy", *home);
    }
    let sample_warn = sample.and_then(|d| d.warn.clone()).map(Stated::Said);
    let warn_stated = stated(
        site.map(|d| &d.warn),
        manifest.map(|d| &d.warn),
        sample_warn.as_ref(),
    );
    if let Some((home, _)) = &warn_stated {
        origins.insert("warn", *home);
    }
    let warn = match (&healthy, warn_stated) {
        (Healthy::Null(_), _) => None,
        (_, Some((_, condition))) => condition.cloned(),
        (_, None) => None,
    };
    let from = stated(site.map(|d| &d.from), manifest.map(|d| &d.from), None);
    if let Some((home, _)) = &from {
        origins.insert("from", *home);
    }
    let from = from.and_then(|(_, source)| source.cloned());

    let every = field!(
        "every",
        site.and_then(|d| d.every.clone()),
        manifest.and_then(|d| d.every.clone()),
        None
    );
    let at = stated(site.map(|d| &d.at), manifest.map(|d| &d.at), None);
    if let Some((home, _)) = &at {
        origins.insert("at", *home);
    }
    let at = at.and_then(|(_, at)| at.cloned());
    let timeout = field!(
        "timeout",
        site.and_then(|d| d.timeout),
        manifest.and_then(|d| d.timeout),
        None
    );
    let drop_after = field!(
        "drop_after",
        site.and_then(|d| d.drop_after),
        manifest.and_then(|d| d.drop_after),
        None
    );
    let recover_after = field!(
        "recover_after",
        site.and_then(|d| d.recover_after),
        manifest.and_then(|d| d.recover_after),
        None
    );
    let proposal = [(Home::Site, site), (Home::Manifest, manifest)]
        .into_iter()
        .find_map(|(home, declaration)| {
            let declaration = declaration?;
            let chosen = !declaration.workflow.is_unsaid() || !declaration.action.is_unsaid();
            chosen.then(|| (home, choice(declaration)))
        });
    if let Some((home, _)) = &proposal {
        origins.insert("workflow", *home);
    }
    let proposal = proposal.and_then(|(_, proposal)| proposal);

    let mut faults = Vec::new();
    let condition_kind = match &healthy {
        Healthy::Said(condition, _) => condition::implied_kind(condition),
        _ => None,
    }
    .or_else(|| warn.as_ref().and_then(condition::implied_kind));
    let kind = stated_kind
        .or_else(|| levels.as_ref().map(|_| Kind::Level))
        .or(condition_kind)
        .or(homes.first_kind);

    let cadence = match Cadence::resolve(every.as_deref(), at.as_deref()) {
        Ok(cadence) => cadence,
        Err(fault) => {
            faults.push(fault);
            Cadence::default_every()
        }
    };

    if let Healthy::Said(condition, _) = &healthy {
        faults.extend(condition::faults(
            "healthy",
            condition,
            kind,
            levels.as_deref(),
        ));
    }
    if let Some(warn) = &warn {
        faults.extend(condition::faults("warn", warn, kind, levels.as_deref()));
        let narrowed = match &healthy {
            Healthy::Said(condition, _) => Some(condition.clone()),
            _ => condition::kind_default(kind, levels.as_deref()),
        };
        let has_default = matches!(kind, Some(Kind::Boolean | Kind::Level));
        match narrowed {
            Some(healthy) => faults.extend(condition::laxer(&healthy, warn)),
            None if !has_default => faults.push(
                "warn has no healthy to narrow: declare a healthy, or drop the warn".to_string(),
            ),
            None => {}
        }
    }

    let mut notes = Vec::new();
    for declaration in [site, manifest].into_iter().flatten() {
        notes.extend(declaration.notes.iter().cloned());
    }

    Monitor {
        id: id.to_string(),
        title: title.unwrap_or_else(|| id.to_string()),
        description,
        kind,
        unit,
        better,
        of,
        levels,
        healthy,
        warn,
        from,
        cadence,
        cadence_stated: every.is_some() || at.is_some(),
        timeout: timeout.unwrap_or(DEFAULT_TIMEOUT_SECONDS),
        drop_after: drop_after.unwrap_or(1),
        recover_after_stated: recover_after,
        proposal,
        origins,
        notes,
        faults,
    }
}

/// The first home that states a nullable field, with what it states
/// (§FS-021-monitors.4.4).
fn stated<'a, T>(
    site: Option<&'a Stated<T>>,
    manifest: Option<&'a Stated<T>>,
    sample: Option<&'a Stated<T>>,
) -> Option<(Home, Option<&'a T>)> {
    [
        (Home::Site, site),
        (Home::Manifest, manifest),
        (Home::Sample, sample),
    ]
    .into_iter()
    .find_map(|(home, field)| field?.stated().map(|value| (home, value)))
}

/// What one home chose for the proposal: its workflow, its action, or
/// neither where it set both aside (§FS-021-monitors.16.1).
fn choice(declaration: &Declaration) -> Option<Proposal> {
    match (&declaration.workflow, &declaration.action) {
        (Stated::Said(workflow), _) => Some(Proposal::Workflow(workflow.clone())),
        (_, Stated::Said(action)) => Some(Proposal::Action(action.clone())),
        _ => None,
    }
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
