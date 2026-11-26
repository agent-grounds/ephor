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
use crate::monitors::model::{Home, Stated};
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
    /// The manifest's `monitors` as parsed, which the store keeps to stand
    /// for it while it cannot be (§FS-021-monitors.17.3).
    pub parsed: Option<Vec<Json>>,
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
            declarations.parsed = Some(manifest.monitors);
        }
        Ok(None) => {}
        Err(err) => declarations.manifest_error = Some(err.to_string()),
    }
    declarations
}

impl Declarations {
    /// Lays the manifest's declarations as the store last kept them over a
    /// manifest that cannot be parsed, so its monitors keep their titles,
    /// conditions and health while its error fails the slot
    /// (§FS-021-monitors.17.3). Whether it did.
    pub fn recall(&mut self, document: &Document) -> bool {
        let Some(kept) = document
            .manifest
            .as_ref()
            .filter(|_| self.manifest_error.is_some())
        else {
            return false;
        };
        self.manifest = shape::read_declarations(kept, Home::Manifest);
        true
    }

    /// Every id either home declares, a refused declaration's included: a
    /// declared monitor is shown even where it is not read, and one refused
    /// with no id to name it by its place, `monitors[0]`
    /// (§FS-021-monitors.4.6).
    pub fn ids(&self) -> BTreeSet<String> {
        [&self.site, &self.manifest]
            .into_iter()
            .flat_map(|home| {
                home.declarations
                    .iter()
                    .map(|declaration| declaration.id.clone())
                    .chain(home.refused.iter().map(name))
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

    /// Lays a refused declaration's faults and notes on a monitor resolved
    /// elsewhere, and whether its homes say something judges it, the site's
    /// word over the manifest's, so a status in error still counts as
    /// unknown (§FS-021-monitors.4.6, §FS-021-monitors.21.2).
    pub fn refuse(&self, monitor: &mut Monitor) {
        let id = monitor.id.clone();
        let refused = || {
            [&self.site, &self.manifest]
                .into_iter()
                .flat_map(|home| home.refused.iter())
                .filter(|refused| name(refused) == id)
        };
        if refused().next().is_none() {
            return;
        }
        for fault in refused().flat_map(|refused| refused.faults.iter()) {
            if !monitor.faults.contains(fault) {
                monitor.faults.push(fault.clone());
            }
        }
        for note in refused().flat_map(|refused| refused.notes.iter()) {
            if !monitor.notes.contains(note) {
                monitor.notes.push(note.clone());
            }
        }
        monitor.declared_judged = [&self.site, &self.manifest].into_iter().find_map(|home| {
            let said = home
                .get(&id)
                .and_then(|declaration| match declaration.healthy {
                    Stated::Said(_) => Some(true),
                    Stated::Null => Some(false),
                    Stated::Unsaid => None,
                });
            said.or_else(|| {
                home.refused
                    .iter()
                    .filter(|refused| name(refused) == id)
                    .find_map(|refused| refused.judged)
            })
        });
    }

    /// What every refresh notes about the declarations
    /// (§FS-021-monitors.4.6): each one refused, with why, in the words a
    /// rule the homes break together is noted in, and each unknown field one
    /// carries, each note once however many declarations say it.
    pub fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        for (home, declared) in [("site", &self.site), ("manifest", &self.manifest)] {
            for refused in &declared.refused {
                notes.push(format!(
                    "{home} monitor {} is not read: {}",
                    name(refused),
                    refused.faults.join("; ")
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
        let mut said = BTreeSet::new();
        notes.retain(|note| said.insert(note.clone()));
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
