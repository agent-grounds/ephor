//! A refresh narrowed below its projects, to one source or to one capability
//! of one source (§FS-011-command-line.9.2).
//!
//! Everything here is decided **before any data call**: which binding each
//! selected project asks, and every refusal — the flags on their own, a source
//! nothing binds, a source with nothing finer to ask, a site binding, and a
//! capability no block declares. The last needs each block's `capabilities`
//! probe, which is not a data call, and the probes are kept so the fetch that
//! follows asks each block once.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::error::{registry_error, Result};
use crate::feed::config::{ProjectFeedConfig, StatusConfig};
use crate::forge::{Capabilities, FETCHES};

/// Where one selected project's narrowed refresh is answered from
/// (§FS-001-forge-interface.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    /// The project binds the source itself.
    Own,
    /// It does not, and the site does: asked once, placed everywhere.
    Site,
    /// Neither does: the project is skipped, and named.
    Unbound,
}

/// What a narrowed run asks, resolved for every selected project.
#[derive(Debug, Default)]
pub struct Narrowing {
    pub source: String,
    /// The one capability, under `--only`.
    pub only: Option<&'static str>,
    /// Each selected project, in the order selected, and where it is asked.
    pub projects: Vec<(String, Binding)>,
    /// Under `--only`, each project's own blocks of the source, probed: the
    /// declaration, or why it could not be read.
    pub probes: BTreeMap<String, Vec<std::result::Result<Capabilities, String>>>,
}

impl Narrowing {
    /// Whether the site's binding of the source is asked.
    pub fn asks_the_site(&self) -> bool {
        self.projects
            .iter()
            .any(|(_, binding)| *binding == Binding::Site)
    }

    /// The selected projects neither binding reaches.
    pub fn skipped(&self) -> Vec<&str> {
        self.projects
            .iter()
            .filter(|(_, binding)| *binding == Binding::Unbound)
            .map(|(project, _)| project.as_str())
            .collect()
    }

    pub fn binding(&self, project: &str) -> Binding {
        self.projects
            .iter()
            .find(|(selected, _)| selected == project)
            .map(|(_, binding)| *binding)
            .unwrap_or(Binding::Unbound)
    }
}

/// The capability `--only` names, refused unless it is one of the five fetch
/// subcommands (§FS-011-command-line.9.2 refusals 1 and 2).
pub fn only(source: Option<&str>, only: Option<&str>) -> Result<Option<&'static str>> {
    let Some(only) = only else {
        return Ok(None);
    };
    if source.is_none() {
        return Err(registry_error(format!(
            "`--only {only}` narrows one source and needs `--source NAME` to say which."
        )));
    }
    FETCHES
        .into_iter()
        .find(|fetch| *fetch == only)
        .map(Some)
        .ok_or_else(|| {
            registry_error(format!(
                "`--only {only}` names no fetch capability; `--only` takes one of {}.",
                FETCHES.join(", ")
            ))
        })
}

/// Resolve `--source` (and `--only`) for every selected project, refusing
/// before any data call (§FS-011-command-line.9.2).
pub fn resolve(
    config: &StatusConfig,
    selected: &[(&String, &ProjectFeedConfig)],
    source: &str,
    only: Option<&'static str>,
) -> Result<Narrowing> {
    // Refusal 3, by name: ephor's own providers and a task store have one
    // history, their whole answer.
    if let Some(only) = only {
        let store = crate::seams::tasks::Kind::parse(source).is_some();
        if store || crate::feed::providers::built_in(source) {
            return Err(registry_error(format!(
                "`--only {only}` asks one capability of a forge reached out of process; \
                 '{source}' is {}, whose answer is one history with nothing finer to ask. \
                 `--source {source}` alone refreshes it.",
                if store {
                    "a task store"
                } else {
                    "a native provider"
                }
            )));
        }
    }
    let projects: Vec<(String, Binding)> = selected
        .iter()
        .map(|(project, project_config)| {
            let sources = crate::feed::providers::Sources {
                project: (*project).clone(),
                own: project_config.providers.clone(),
                site: config.sources.clone(),
            };
            let binding = match sources.find(source) {
                Some((_, "")) => Binding::Site,
                Some(_) => Binding::Own,
                None => Binding::Unbound,
            };
            ((*project).clone(), binding)
        })
        .collect();
    // Refusal 6.
    if projects
        .iter()
        .all(|(_, binding)| *binding == Binding::Unbound)
    {
        let task_store = match crate::seams::tasks::Kind::parse(source) {
            Some(_) => " A task store is bound by no configuration; a plain refresh reads it.",
            None => "",
        };
        return Err(registry_error(format!(
            "No selected project and not the site binds a source named '{source}'.{task_store}"
        )));
    }
    let mut narrowing = Narrowing {
        source: source.to_string(),
        only,
        projects,
        probes: BTreeMap::new(),
    };
    let Some(only) = only else {
        return Ok(narrowing);
    };
    // Refusal 4: a site source is placed on the combined evidence of every
    // capability's reports, so one capability alone could misplace a subject.
    if let Some((project, _)) = narrowing
        .projects
        .iter()
        .find(|(_, binding)| *binding == Binding::Site)
    {
        return Err(registry_error(format!(
            "'{source}' reaches '{project}' through the site's binding, which is placed on the \
             evidence of its whole answer; `--only {only}` is refused there. \
             `--source {source}` alone refreshes it whole."
        )));
    }
    // Refusal 5 needs the declarations: every own block probed, at once.
    let owned: Vec<(&String, Vec<&Value>)> = selected
        .iter()
        .filter(|(project, _)| narrowing.binding(project) == Binding::Own)
        .map(|(project, project_config)| {
            let blocks = project_config
                .providers
                .iter()
                .filter(|block| block.get("provider").and_then(Value::as_str) == Some(source))
                .collect();
            (*project, blocks)
        })
        .collect();
    narrowing.probes = std::thread::scope(|scope| {
        let jobs: Vec<_> = owned
            .iter()
            .map(|(project, blocks)| {
                let probes: Vec<_> = blocks
                    .iter()
                    .map(|block| scope.spawn(move || probe(block)))
                    .collect();
                ((*project).clone(), probes)
            })
            .collect();
        jobs.into_iter()
            .map(|(project, probes)| {
                let probed = probes
                    .into_iter()
                    .map(|job| {
                        job.join()
                            .unwrap_or_else(|_| Err("the probe panicked".to_string()))
                    })
                    .collect();
                (project, probed)
            })
            .collect()
    });
    let mut probes = narrowing.probes.values().flatten();
    let read: Vec<&Capabilities> = narrowing
        .probes
        .values()
        .flatten()
        .filter_map(|probe| probe.as_ref().ok())
        .collect();
    // A probe that failed is a fetch failure, not a refusal: whether that
    // block declares the capability is not known.
    if probes.all(|probe| probe.is_ok()) && !read.iter().any(|caps| caps.fetches().contains(&only))
    {
        return Err(registry_error(format!(
            "'{source}' declares no {only}: none of its blocks answers `{only}` \
             (it declares {}).",
            match read.first().map(|caps| caps.fetches()) {
                Some(fetches) if !fetches.is_empty() => fetches.join(", "),
                _ => "no fetch capability".to_string(),
            }
        )));
    }
    Ok(narrowing)
}

/// One block's `capabilities` probe, the way a fetch would make it: a
/// declaration of nothing is the failure a fetch reads it as.
fn probe(block: &Value) -> std::result::Result<Capabilities, String> {
    let provider =
        crate::feed::providers::forge::ForgeProvider::external(block).map_err(|err| err.0)?;
    match provider.probe().map_err(|err| err.0)? {
        nothing if nothing == Capabilities::default() => Err(format!(
            "{source} declared no capabilities — is it answering `capabilities` with JSON?",
            source = block.get("provider").and_then(Value::as_str).unwrap_or("")
        )),
        declared => Ok(declared),
    }
}
