//! A project's monitor declarations, read from their two homes: the site's
//! configuration and the manifest in the project's own checkout
//! (§FS-021-monitors.4.1, §FS-021-monitors.4.2).
//!
//! Each declaration is checked on its own (§FS-021-monitors.4.6): one that
//! breaks a rule is shown and not read, and every refresh notes it, while the
//! others are read as if it were not there.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value as Json;

use crate::branches::Placement;
use crate::manifest::{Manifest, Trust};
use crate::monitors::model::Home;
use crate::monitors::overlay::{self, Homes, Monitor};
use crate::monitors::shape::{self, Declared, Refused};
use crate::monitors::state::{Document, Record};

/// What both homes declare for one project.
#[derive(Debug, Clone, Default)]
pub struct Declarations {
    pub site: Declared,
    pub manifest: Declared,
    /// Why the manifest could not be read, where it could not
    /// (§FS-021-monitors.17.3).
    pub manifest_error: Option<String>,
    /// The registry row trusts only what the manifest says about itself, so
    /// none of its commands is run (§FS-021-monitors.4.5).
    pub descriptions_only: bool,
}

/// Where a project's manifest is read and its monitors are run: its default
/// branch's checkout, or the forest root where none is on disk
/// (§FS-021-monitors.4.1, §FS-021-monitors.5.3).
pub fn checkout(placement: &Placement) -> PathBuf {
    placement
        .project_checkout()
        .unwrap_or_else(|| placement.root.clone())
}

/// Reads both homes. A manifest that cannot be read is kept as its error,
/// for the caller to decide whether that fails the slot
/// (§FS-021-monitors.17.3).
pub fn read(placement: Option<&Placement>, site: &[Json]) -> Declarations {
    let mut declarations = Declarations {
        site: shape::read_declarations(site, Home::Site),
        ..Declarations::default()
    };
    let Some(placement) = placement else {
        return declarations;
    };
    declarations.descriptions_only = placement.trust == Trust::Descriptions;
    match Manifest::read(&checkout(placement), placement.trust) {
        Ok(Some(manifest)) => {
            declarations.manifest = shape::read_declarations(&manifest.monitors, Home::Manifest);
        }
        Ok(None) => {}
        Err(err) => declarations.manifest_error = Some(err.to_string()),
    }
    declarations
}

impl Declarations {
    /// Every id either home declares, a refused declaration's included: a
    /// declared monitor is shown even where it is not read
    /// (§FS-021-monitors.4.6).
    pub fn ids(&self) -> BTreeSet<String> {
        [&self.site, &self.manifest]
            .into_iter()
            .flat_map(|home| {
                home.declarations
                    .iter()
                    .map(|declaration| declaration.id.clone())
                    .chain(home.refused.iter().filter_map(|refused| refused.id.clone()))
            })
            .collect()
    }

    /// Whether either home declares a monitor at all (§FS-021-monitors.1.4).
    pub fn any(&self) -> bool {
        self.site_declares()
            || !self.manifest.declarations.is_empty()
            || !self.manifest.refused.is_empty()
    }

    /// Whether the site declares one, which is what lets a manifest that
    /// cannot be read fail the slot (§FS-021-monitors.17.3).
    pub fn site_declares(&self) -> bool {
        !self.site.declarations.is_empty() || !self.site.refused.is_empty()
    }

    /// The homes one monitor resolves from, with the description its last
    /// accepted sample carried (§FS-021-monitors.4.4).
    pub fn homes<'a>(&'a self, id: &str, record: Option<&'a Record>) -> Homes<'a> {
        Homes {
            site: self.site.get(id),
            manifest: self.manifest.get(id),
            sample: record
                .and_then(|record| record.last_sample.as_ref())
                .map(|sample| &sample.description),
            first_kind: record.and_then(|record| record.kind),
            descriptions_only: self.descriptions_only,
        }
    }

    /// One monitor, resolved from its homes, with the faults of any of its
    /// declarations a home refused: such a monitor is not read until the
    /// declaration is mended or goes (§FS-021-monitors.4.6).
    pub fn resolve(&self, id: &str, record: Option<&Record>) -> Monitor {
        let mut monitor = overlay::resolve(id, &self.homes(id, record));
        self.refuse(&mut monitor);
        monitor
    }

    /// Lays a refused declaration's faults on a monitor resolved elsewhere.
    pub fn refuse(&self, monitor: &mut Monitor) {
        for fault in self
            .site
            .faults(&monitor.id)
            .into_iter()
            .chain(self.manifest.faults(&monitor.id))
        {
            if !monitor.faults.contains(&fault) {
                monitor.faults.push(fault);
            }
        }
    }

    /// What every refresh notes about the declarations
    /// (§FS-021-monitors.4.6): each one refused, with why, and each unknown
    /// field one carries.
    pub fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        for (home, declared) in [("site", &self.site), ("manifest", &self.manifest)] {
            for refused in &declared.refused {
                notes.push(format!(
                    "{home} monitor {} is not read",
                    say_refused(refused)
                ));
                notes.extend(
                    refused
                        .notes
                        .iter()
                        .map(|note| format!("{home} monitor {}: {note}", name(refused))),
                );
            }
            for declaration in &declared.declarations {
                notes.extend(
                    declaration
                        .notes
                        .iter()
                        .map(|note| format!("{home} monitor {}: {note}", declaration.id)),
                );
            }
        }
        notes
    }

    /// What every refresh notes about the monitors the declarations resolve
    /// to (§FS-021-monitors.4.6): a rule the homes break together, such as a
    /// `warn` laxer than its `healthy` (§FS-021-monitors.3.5), leaves the
    /// monitor shown and not read, and says why as a refused declaration
    /// does. A refused declaration is noted by [`Declarations::notes`].
    pub fn faulted(&self, document: &Document) -> Vec<String> {
        self.ids()
            .into_iter()
            .filter_map(|id| {
                let faults =
                    overlay::resolve(&id, &self.homes(&id, document.monitors.get(&id))).faults;
                (!faults.is_empty())
                    .then(|| format!("monitor {id} is not read: {}", faults.join("; ")))
            })
            .collect()
    }
}

fn name(refused: &Refused) -> String {
    refused
        .id
        .clone()
        .unwrap_or_else(|| format!("monitors[{}]", refused.index))
}

/// One refused declaration, by its id or its place, with why
/// (§FS-021-monitors.4.6).
pub fn say_refused(refused: &Refused) -> String {
    format!("{}: {}", name(refused), refused.faults.join("; "))
}

#[cfg(test)]
#[path = "declared_tests.rs"]
mod tests;
