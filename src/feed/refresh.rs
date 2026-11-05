//! Refresh orchestration: fetch every configured provider for the selected
//! projects in parallel, isolate failures, merge into the cache.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::{registry_error, EphorError, Result};
use crate::feed::cache::{load_feed, store_feed, ProjectFeed, ProviderSlot};
use crate::feed::config::{Defaults, ProjectFeedConfig, StatusConfig};
use crate::feed::model::Item;
use crate::feed::provider::ProviderContext;
use crate::feed::providers::build_provider;
use crate::feed::reachability;
use crate::paths;
use crate::registry::{self, Workspace};

/// One provider that did not deliver, and whether it is the reader's to fix.
pub struct ProviderFailure {
    pub provider: String,
    pub message: String,
    /// The destination could not be reached at all. Kept apart from every
    /// other failure because it asks nothing of the reader but a working
    /// network, and because the items left on screen are last-good data
    /// rather than this source's current state.
    pub unreachable: bool,
}

impl ProviderFailure {
    /// One warning line: which provider, what happened, and whether this is a
    /// network condition or a configuration to go and fix.
    pub fn describe(&self) -> String {
        if self.unreachable {
            format!("{}: unreachable — {}", self.provider, self.message)
        } else {
            format!("{}: {}", self.provider, self.message)
        }
    }
}

pub struct RefreshOutcome {
    pub item_count: usize,
    pub failures: Vec<ProviderFailure>,
    /// What a source answered but could not be carried whole: a key a plan
    /// wrote about one of its tasks that the bound refused
    /// (§FS-005-dispatch.8). Not a failure — the source answered, and its slot
    /// says so — but not silent either, because a drop nobody is told about is
    /// how a selector stops matching for no reason a reader can see.
    pub notes: Vec<String>,
    /// Actual answers and failures, including stores and empty answers
    /// (§FS-001-forge-interface.6.1).
    pub contributions: ContributionTally,
}

/// Refresh exits count attempts, never cached data or idle projects
/// (§FS-001-forge-interface.6.1).
#[derive(Default, Clone, Copy)]
pub struct ContributionTally {
    pub answered: usize,
    pub failed: usize,
}

impl ContributionTally {
    pub fn add(&mut self, other: Self) {
        self.answered += other.answered;
        self.failed += other.failed;
    }

    pub fn total_failure(&self) -> bool {
        self.failed > 0 && self.answered == 0
    }
}

/// Each invocation contributes once to completeness and outcome accounting
/// (§FS-001-forge-interface.6.1).
type Contribution = std::result::Result<Vec<Item>, String>;

/// A stable provider name holds all contributions, including every distinct
/// diagnosis; successes cannot reset failures (§FS-001-forge-interface.6.1).
#[derive(Default)]
struct ProviderAnswer {
    contributions: ContributionTally,
    errors: BTreeSet<String>,
    items: Vec<Item>,
}

impl ProviderAnswer {
    fn record(&mut self, contribution: Contribution) {
        match contribution {
            Ok(items) => {
                self.contributions.answered += 1;
                self.items.extend(items);
            }
            Err(error) => {
                self.contributions.failed += 1;
                self.errors.insert(error);
            }
        }
    }

    fn failure(&self, name: &str) -> Option<ProviderFailure> {
        if self.errors.is_empty() {
            return None;
        }
        Some(ProviderFailure {
            provider: name.to_string(),
            message: self.errors.iter().cloned().collect::<Vec<_>>().join("; "),
            // A mixed network/actionable failure must still ask the reader
            // to fix the actionable part (§FS-001-forge-interface.6.1).
            unreachable: self
                .errors
                .iter()
                .all(|why| reachability::is_unreachable(why)),
        })
    }
}

/// Ask all blocks concurrently, then reduce in configuration order rather
/// than completion order, before either collector persists or attributes
/// anything (§FS-001-forge-interface.6.1).
fn collect_providers(configs: &[Value], ctx: &ProviderContext) -> BTreeMap<String, ProviderAnswer> {
    std::thread::scope(|scope| {
        let jobs: Vec<_> = configs
            .iter()
            .map(|config| scope.spawn(move || fetch_one(config, ctx)))
            .collect();
        let mut answers: BTreeMap<String, ProviderAnswer> = BTreeMap::new();
        for job in jobs {
            let (name, contribution) = job.join().unwrap();
            answers.entry(name).or_default().record(contribution);
        }
        answers
    })
}

/// Only a complete answer replaces a slot. An incomplete answer retains the
/// whole last-good slot, timestamp and cursor (§FS-001-forge-interface.6.1).
fn refreshed_slot(
    previous: Option<&ProviderSlot>,
    now: DateTime<Utc>,
    matters: Vec<crate::matter::Matter>,
    failure: Option<&ProviderFailure>,
) -> ProviderSlot {
    if let Some(failure) = failure {
        ProviderSlot {
            fetched_at: previous.and_then(|slot| slot.fetched_at),
            ok: false,
            error: Some(failure.message.clone()),
            stale: previous.is_some_and(|slot| !slot.matters.is_empty()),
            unreachable: failure.unreachable,
            matters: previous
                .map(|slot| slot.matters.clone())
                .unwrap_or_default(),
            cursor: previous.and_then(|slot| slot.cursor.clone()),
        }
    } else {
        ProviderSlot {
            fetched_at: Some(now),
            ok: true,
            error: None,
            stale: false,
            unreachable: false,
            matters,
            cursor: previous.and_then(|slot| slot.cursor.clone()),
        }
    }
}

impl RefreshOutcome {
    pub fn unreachable_count(&self) -> usize {
        self.failures.iter().filter(|f| f.unreachable).count()
    }
}

/// Build the provider context for one project from the registry.
pub fn build_context(
    registry_doc: &Value,
    project_id: &str,
    defaults: &Defaults,
) -> Result<ProviderContext> {
    let project = registry::array_field(registry_doc, "projects")
        .iter()
        .find(|p| registry::id_of(p) == project_id)
        .ok_or_else(|| {
            registry_error(format!(
                "Feed config references unknown project '{project_id}' (not in the registry)."
            ))
        })?;

    let mut tickets: Vec<String> = Vec::new();
    for workspace in registry::derive_project_workspaces(project)? {
        if !workspace.active {
            continue;
        }
        if let Some(ticket) = workspace_ticket(&workspace) {
            tickets.push(ticket);
        }
    }
    tickets.sort();
    tickets.dedup();

    Ok(ProviderContext {
        project_id: project_id.to_string(),
        project_root: paths::resolve_path(registry::str_field(project, "root").unwrap_or("")),
        // Read here and never written back: membership and the root an
        // organization declares are identity (§REQ-001-boundary.2), and the
        // summons a provider makes is one more occasion they are read on
        // (§FS-014-work-root-scopes.1).
        organization: registry::organization_of(registry_doc, project_id)
            .map(|(id, root)| crate::branches::Organization { id, root }),
        main_branch: registry::str_field(project, "main_branch")
            .unwrap_or("")
            .to_string(),
        tickets,
        github_user: defaults.github_user.clone(),
        timeout: Duration::from_secs(defaults.provider_timeout_seconds),
        secrets_dir: paths::secrets_dir(),
    })
}

/// A provider block's own timeout ceiling, when it sets one.
///
/// `provider_timeout_seconds` is the shared default, sized for a local `gh`
/// call. A forge reached over a VPN, or one that walks a pull request's
/// comment threads, needs longer — and raising the default for its sake would
/// make every genuinely hung provider take that long to give up.
pub fn provider_timeout(config: &Value) -> Option<Duration> {
    config
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .filter(|seconds| *seconds > 0)
        .map(Duration::from_secs)
}

fn workspace_ticket(workspace: &Workspace) -> Option<String> {
    if let Some(ticket) = workspace.vars.get("ticket").and_then(Value::as_str) {
        if !ticket.is_empty() {
            return Some(ticket.to_string());
        }
    }
    let branch = workspace
        .vars
        .get("branch")
        .and_then(Value::as_str)
        .unwrap_or("");
    let extracted = crate::ticket_ids::extract_ticket(branch);
    if extracted.is_empty() {
        None
    } else {
        Some(extracted)
    }
}

/// Every task store this project's refresh reads, each one once.
///
/// The root, and every active branch's workspace: work about a change lives in
/// that change's tree, so a branch-addressable project keeps a store per
/// workspace (§FS-006-project-interface.7). The root is named twice over —
/// `task_stores` answers for it wherever it probes as one, and it stays named
/// here besides because only `find` honours a manifest that declares a store
/// somewhere neither kind is probed — so a store found by both roads is kept
/// once. A store read twice answers twice, and its answer's drops would then be
/// reported twice where §FS-005-dispatch.8 says once.
fn stores_to_read(
    placement: &crate::branches::Placement,
    manifest: Option<&crate::manifest::Manifest>,
) -> Vec<crate::seams::tasks::Store> {
    let mut seen: std::collections::HashSet<std::path::PathBuf> = std::collections::HashSet::new();
    std::iter::once(placement.root.clone())
        .chain(placement.task_stores())
        .flat_map(|root| crate::seams::tasks::find(&root, manifest))
        .filter(|store| seen.insert(store.path.clone()))
        .collect()
}

/// Fetch all providers of one project concurrently and store the merged feed.
/// Repeated blocks and stores form one complete answer per provider
/// (§FS-001-forge-interface.6.1).
///
/// `site_sources` names the sources bound for the site ([`site_source_names`]).
/// They are not asked here, so the slots they placed in this project are kept
/// as they were (§FS-001-forge-interface.9).
pub fn refresh_project(
    registry_doc: &Value,
    project_id: &str,
    project_config: &ProjectFeedConfig,
    site_sources: &[String],
    defaults: &Defaults,
) -> Result<RefreshOutcome> {
    let ctx = build_context(registry_doc, project_id, defaults)?;
    let previous = load_feed(project_id)?.unwrap_or_default();

    // The same reducer handles configured blocks and discovered stores
    // (§FS-001-forge-interface.6.1).
    let mut results = collect_providers(&project_config.providers, &ctx);

    let mut notes: Vec<String> = Vec::new();

    // Checkout sources read the forest itself, where there is one on disk
    // (§AR-008-pipeline.1). A task store is a project-native thing that
    // exists without ephor, so finding one is a capability rather than an
    // obligation (§FS-006-project-interface.7).
    if let Some(placement) = crate::branches::Placement::load(registry_doc, project_id) {
        let manifest = placement.manifest();
        for store in stores_to_read(&placement, manifest.as_ref()) {
            let name = store.kind.name().to_string();
            match crate::seams::tasks::read_reporting(&store, project_id) {
                // A store that answered lands its matters, empty answer
                // included: a store with no open tasks has said so, and its
                // slot saying so is what tells last refresh's tasks to go.
                Ok((items, said)) => {
                    results.entry(name).or_default().record(Ok(items));
                    // Reported once, where this source's own answer for this
                    // read is reported (§FS-005-dispatch.8).
                    notes.extend(
                        said.into_iter()
                            .map(|said| format!("{}: {said}", store.kind.name())),
                    );
                }
                // A store ephor could not read is reported like any other
                // source that did not answer, rather than read as a store with
                // nothing in it (§FS-001-forge-interface.6).
                Err(why) => {
                    results.entry(name).or_default().record(Err(why));
                }
            }
        }
    }

    let now = Utc::now();
    let mut feed = ProjectFeed {
        project: project_id.to_string(),
        fetched_at: Some(now),
        model: crate::feed::cache::MODEL,
        providers: BTreeMap::new(),
    };
    let mut failures = Vec::new();
    let mut contributions = ContributionTally::default();

    for (name, answer) in results {
        contributions.add(answer.contributions);
        let failure = answer.failure(&name);
        let matters = if failure.is_none() {
            let matters: Vec<crate::matter::Matter> = answer
                .items
                .iter()
                .map(crate::matter::Matter::of_item)
                .collect();
            if name == "custom-status" {
                reconcile_custom_status_answer_matters(
                    matters,
                    previous
                        .providers
                        .get(&name)
                        .map(|slot| slot.matters.as_slice()),
                )
            } else {
                matters
            }
        } else {
            Vec::new()
        };
        let slot = refreshed_slot(
            previous.providers.get(&name),
            now,
            matters,
            failure.as_ref(),
        );
        if let Some(failure) = failure {
            failures.push(failure);
        }
        feed.providers.insert(name, slot);
    }

    let item_count = feed.items().count();
    // A site source's slot here is `refresh_shared`'s to rewrite, so this run
    // carries it over unasked: that is the slot a failure there marks stale
    // (§FS-001-forge-interface.6 rule 5, §FS-001-forge-interface.9). A name no
    // longer in `sources`, or one this project binds itself, is not carried.
    for name in site_sources {
        if feed.providers.contains_key(name) {
            continue;
        }
        if let Some(slot) = previous.providers.get(name) {
            feed.providers.insert(name.clone(), slot.clone());
        }
    }
    store_feed(&feed)?;
    Ok(RefreshOutcome {
        item_count,
        failures,
        notes,
        contributions,
    })
}

/// Missing activity belongs to the retained provider-slot/key observation,
/// not to the refresh that happened to see it. Supplied time remains
/// authoritative, and an empty successful result naturally ends the
/// observation because only the immediately previous slot is consulted
/// (§FS-006-project-interface.4, §FS-007-matters.1).
fn reconcile_custom_status_answer_matters(
    mut current: Vec<crate::matter::Matter>,
    previous: Option<&[crate::matter::Matter]>,
) -> Vec<crate::matter::Matter> {
    let previous = previous.unwrap_or_default();
    for matter in &mut current {
        if custom_status_answer_time_supplied(matter) != Some(false) {
            continue;
        }
        if let Some(retained) = previous.iter().find(|retained| {
            retained.key == matter.key && custom_status_answer_time_supplied(retained).is_some()
        }) {
            matter.updated_at = retained.updated_at;
        }
    }
    current
}

fn custom_status_answer_time_supplied(matter: &crate::matter::Matter) -> Option<bool> {
    crate::feed::model::custom_status_answer(&matter.source, &matter.raw)?
        .get("time_supplied")?
        .as_bool()
}

/// One project's refresh, as it lands.
pub struct Landed {
    pub project: String,
    /// Providers of this project that did not deliver, each already qualified
    /// by the project, since the reader's screen holds several
    /// (§FS-001-forge-interface.6).
    pub lost: Vec<String>,
    pub unreachable: usize,
    /// Why the project could not be asked at all — a registry that does not
    /// describe it, a context that would not build. None of its providers ran.
    pub failed: Option<String>,
}

/// A refresh running beneath the interface (§FS-001-forge-interface.7).
///
/// The fetch is the slowest thing ephor does — a source is entitled to a
/// ceiling of its own (§FS-001-forge-interface.2), and minutes is a legitimate
/// answer — while the reader's screen is not the fetch's to hold. So the run
/// lives on a thread of its own and reports one project at a time through a
/// channel the interface drains between key reads.
///
/// Projects are still asked one after another, exactly as a foreground refresh
/// asked them: what moved off the reader's screen is the waiting, not the load
/// on the forge (§GOAL-005-costless).
pub struct BackgroundRefresh {
    landed: Receiver<Landed>,
    /// Held so the thread is reaped when the run ends. Dropping it detaches,
    /// which is what a reader quitting mid-run should do: nothing on their
    /// way out waits for a forge, and every feed is written whole
    /// (§AR-008-pipeline.1) so a half-finished run leaves no torn cache.
    worker: Option<JoinHandle<()>>,
    /// The projects this run asks for, in the order it asks them — so the one
    /// in flight is the one after those that have reported.
    queue: Vec<String>,
    finished: usize,
    lost: Vec<String>,
    unreachable: usize,
    /// The first project that could not be asked at all, and why. Kept apart
    /// from the lost providers: it is a sentence about configuration, and a
    /// name on its own does not say what to go and fix.
    failed: Option<String>,
}

/// The projects a background run will ask, in the configuration's own order.
///
/// `over` is the list the screen was opened over rather than the whole watch
/// list: a screen opened under a scope selector fetches what it shows and
/// nothing else, or the key would reach organizations the reader narrowed
/// away and the run would cost what the scope was meant to save
/// (§FS-011-command-line.9).
fn jobs_over(config: &StatusConfig, over: &[String]) -> Vec<(String, ProjectFeedConfig)> {
    config
        .projects
        .iter()
        .filter(|(project, _)| over.contains(project))
        .map(|(project, project_config)| (project.clone(), project_config.clone()))
        .collect()
}

impl BackgroundRefresh {
    /// Start a run and give the screen straight back. `over` is which projects
    /// to ask: the one the detail view shows, or every project the screen was
    /// opened over.
    ///
    /// The registry is read here, on the caller's thread: it is a local file,
    /// and a registry that will not parse is an answer the keystroke can have
    /// rather than one that arrives later looking like a refresh that lost
    /// everything.
    pub fn start(config: &StatusConfig, over: &[String]) -> Result<Self> {
        let registry_doc = crate::feed::commands::load_registry_doc()?;
        let defaults = config.defaults.clone();
        let site_sources = site_source_names(config);
        let jobs = jobs_over(config, over);
        let queue: Vec<String> = jobs.iter().map(|(project, _)| project.clone()).collect();

        let (sender, landed) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("ephor-refresh".to_string())
            .spawn(move || {
                for (project, project_config) in jobs {
                    let report = match refresh_project(
                        &registry_doc,
                        &project,
                        &project_config,
                        &site_sources,
                        &defaults,
                    ) {
                        Ok(outcome) => Landed {
                            lost: outcome
                                .failures
                                .iter()
                                .map(|failure| format!("{project}/{}", failure.provider))
                                .collect(),
                            unreachable: outcome.unreachable_count(),
                            failed: None,
                            project,
                        },
                        Err(err) => Landed {
                            lost: vec![project.clone()],
                            unreachable: 0,
                            failed: Some(err.to_string()),
                            project,
                        },
                    };
                    // A reader who left took the receiver with them. There is
                    // nobody to report to, and the rest of the run is work
                    // nobody asked for any more.
                    if sender.send(report).is_err() {
                        return;
                    }
                }
            })
            .map_err(|err| EphorError::Command(format!("Could not start the refresh: {err}")))?;

        Ok(BackgroundRefresh {
            landed,
            worker: Some(worker),
            queue,
            finished: 0,
            lost: Vec::new(),
            unreachable: 0,
            failed: None,
        })
    }

    /// Everything that has landed since the last call, folded into the run's
    /// tally on the way past. Never blocks: a project still being asked is
    /// simply not in the answer yet.
    pub fn collect(&mut self) -> Vec<Landed> {
        let mut arrived = Vec::new();
        loop {
            match self.landed.try_recv() {
                Ok(report) => {
                    self.finished += 1;
                    self.unreachable += report.unreachable;
                    self.lost.extend(report.lost.iter().cloned());
                    if let (None, Some(why)) = (&self.failed, &report.failed) {
                        self.failed = Some(format!("{}: {why}", report.project));
                    }
                    arrived.push(report);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                // The thread the run was on is gone with projects left to
                // ask. Nothing more is coming, so the run is over — and what
                // never reported is named as lost rather than left as a
                // progress line that never moves again
                // (§FS-001-forge-interface.6).
                Err(mpsc::TryRecvError::Disconnected) => {
                    let unasked: Vec<String> = self.queue[self.finished..].to_vec();
                    if !unasked.is_empty() {
                        self.lost.extend(unasked);
                        if self.failed.is_none() {
                            self.failed = Some("the refresh stopped early".to_string());
                        }
                        self.finished = self.queue.len();
                    }
                    break;
                }
            }
        }
        arrived
    }

    /// Whether every project the run asked for has reported.
    pub fn done(&self) -> bool {
        self.finished >= self.queue.len()
    }

    /// What the header says while the run is in flight: the project being
    /// asked now, and how far through the run it is
    /// (§FS-001-forge-interface.7).
    pub fn progress(&self) -> String {
        match self.queue.get(self.finished) {
            Some(project) => format!(
                "Refreshing {project} ({}/{})…",
                self.finished + 1,
                self.queue.len()
            ),
            None => "Refreshing…".to_string(),
        }
    }

    /// The one line a finished run leaves behind.
    pub fn summary(&self) -> String {
        summarize(&self.lost, self.unreachable, self.failed.as_deref())
    }

    /// Reap the worker once the run is done. It has already sent everything it
    /// will send, so this only collects the thread.
    pub fn finish(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// What a finished run tells the reader it lost (§FS-001-forge-interface.6).
///
/// Named, not counted. "6 provider warnings" is the same sentence whether a
/// forge has been uninstalled for months or a laptop is off the VPN for a
/// minute, and in both cases the sections those providers fill just look
/// empty.
pub fn summarize(lost: &[String], unreachable: usize, failed: Option<&str>) -> String {
    if lost.is_empty() {
        return "Refreshed".to_string();
    }
    // Enough names to act on, then a count for the rest.
    const NAMED: usize = 3;
    let mut shown = lost
        .iter()
        .take(NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if lost.len() > NAMED {
        shown = format!("{shown} +{} more", lost.len() - NAMED);
    }
    let network = if unreachable > 0 {
        format!(" ({unreachable} unreachable — check the network/VPN)")
    } else {
        String::new()
    };
    // A project that could not be asked at all is a configuration to go and
    // fix, and its name on its own does not say what.
    let why = match failed {
        Some(failed) => format!(" — {failed}"),
        None => String::new(),
    };
    format!(
        "Refreshed — NO DATA from {}: {shown}{network}{why}",
        lost.len()
    )
}

/// The reserved feed a matter lands in when nothing claimed it, or when two
/// projects claimed it equally (§FS-008-attribution.4). The bucket is part of
/// the store rather than a log line: a conversation ephor could not place is
/// still a conversation somebody is waiting on.
pub const UNATTRIBUTED: &str = "unattributed";

/// Fetch the sources shared across the site, once, and let the engine place
/// what they report (§AR-008-pipeline.1, §AR-008-pipeline.2).
///
/// This is the half of the pipeline no source owns: a notification stream asks
/// nothing about any one project and answers about all of them, so it is
/// fetched once here and its findings are placed against every project's
/// identity (§AR-003-attribution). What nothing claims is kept, visibly.
/// Repeated blocks combine before placement; incomplete answers preserve
/// whole-slot last-good data (§FS-001-forge-interface.6.1).
pub fn refresh_shared(registry_doc: &Value, config: &StatusConfig) -> Result<RefreshOutcome> {
    if config.sources.is_empty() {
        return Ok(RefreshOutcome {
            item_count: 0,
            failures: Vec::new(),
            notes: Vec::new(),
            contributions: ContributionTally::default(),
        });
    }

    // Every configured project's identity, compiled from its row. A project
    // the registry does not describe cannot claim anything, and is skipped
    // rather than guessed at.
    let identities: Vec<crate::attribution::Identity> = config
        .projects
        .keys()
        .filter_map(|project| crate::branches::Placement::load(registry_doc, project))
        .map(|placement: crate::branches::Placement| placement.identity())
        .collect();

    let ctx = site_context(&config.defaults);

    let results = collect_providers(&config.sources, &ctx);
    let now = Utc::now();
    let mut failures = Vec::new();
    let mut contributions = ContributionTally::default();
    let mut item_count = 0usize;
    // Where each source's findings ended up: project id (or the bucket) to the
    // matters placed there.
    let mut placed: BTreeMap<String, BTreeMap<String, Vec<crate::matter::Matter>>> =
        BTreeMap::new();

    for (name, answer) in results {
        contributions.add(answer.contributions);
        if let Some(failure) = answer.failure(&name) {
            failures.push(failure);
            continue;
        }
        item_count += answer.items.len();
        for item in answer.items {
            let mut matter = crate::matter::Matter::of_item(&item);
            let evidence = crate::matter::evidence_of(&item);
            matter.placement = match crate::attribution::place(&evidence, &identities) {
                // How firmly it was claimed travels with it: what may be done
                // with a placement depends on how it was reached
                // (§FS-008-attribution.3).
                crate::attribution::Placed::On { project, how } => {
                    crate::matter::Placement::claimed(project, how)
                }
                crate::attribution::Placed::Ambiguous { candidates } => {
                    crate::matter::Placement::Unattributed { candidates }
                }
                crate::attribution::Placed::Nothing => crate::matter::Placement::Unattributed {
                    candidates: Vec::new(),
                },
            };
            let home = matter
                .placement
                .project()
                .unwrap_or(UNATTRIBUTED)
                .to_string();
            placed
                .entry(home)
                .or_default()
                .entry(name.clone())
                .or_default()
                .push(matter);
        }
    }

    // Every project a shared source could have spoken about gets its slot
    // rewritten, including to empty: a source that used to place something
    // here and no longer does has to be able to say so.
    let homes: Vec<String> = config
        .projects
        .keys()
        .cloned()
        .chain(std::iter::once(UNATTRIBUTED.to_string()))
        .collect();
    let names = site_source_names(config);
    for home in homes {
        let mut feed = load_feed(&home)?.unwrap_or(ProjectFeed {
            project: home.clone(),
            ..ProjectFeed::default()
        });
        let mine = placed.remove(&home).unwrap_or_default();
        let mut touched = false;
        for name in &names {
            let matters = mine.get(name).cloned().unwrap_or_default();
            let failed = failures.iter().find(|failure| &failure.provider == name);
            if failed.is_some() {
                // Last-good data waits out a failure here exactly as it does
                // for a per-project source, and says why, and which kind of
                // not, beside it (§FS-001-forge-interface.6). A home that
                // never held this source's rows is not given a slot for its
                // failure: the run already says it, once.
                if !feed.providers.contains_key(name) {
                    continue;
                }
            }
            let slot = refreshed_slot(feed.providers.get(name), now, matters, failed);
            feed.providers.insert(name.clone(), slot);
            touched = true;
        }
        if touched {
            feed.fetched_at = Some(now);
            feed.model = crate::feed::cache::MODEL;
            store_feed(&feed)?;
        }
    }

    Ok(RefreshOutcome {
        item_count,
        failures,
        // Shared sources are forge readers; no store answers here.
        notes: Vec::new(),
        contributions,
    })
}

/// The names of the sources bound for the site, each the slot it writes in
/// every home it places a matter in (§FS-001-forge-interface.9). Repeated
/// contributions still write that slot only once (§FS-001-forge-interface.6.1).
pub fn site_source_names(config: &StatusConfig) -> Vec<String> {
    config
        .sources
        .iter()
        .filter_map(|source: &Value| source.get("provider").and_then(Value::as_str))
        .map(String::from)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The one context a source bound for the site is asked in: no project,
/// because that is the point — on a fetch, and on a move that goes back to it
/// (§FS-001-forge-interface.9).
pub fn site_context(defaults: &crate::feed::config::Defaults) -> ProviderContext {
    ProviderContext {
        project_id: String::new(),
        project_root: paths::state_dir(),
        // No project, so no organization to place it in: both names reach a
        // shared source's summons defined and empty, beside the empty
        // `EPHOR_PROJECT` that says the same thing (§FS-005-dispatch.6.1).
        organization: None,
        main_branch: String::new(),
        tickets: Vec::new(),
        github_user: defaults.github_user.clone(),
        timeout: Duration::from_secs(defaults.provider_timeout_seconds),
        secrets_dir: paths::secrets_dir(),
    }
}

/// Build one source and ask it, reporting a failure as data rather than
/// letting it end the refresh. Project and site blocks share this invocation
/// boundary and preserve each result (§FS-001-forge-interface.6.1).
fn fetch_one(config: &Value, ctx: &ProviderContext) -> (String, Contribution) {
    let raised;
    let ctx = match provider_timeout(config) {
        Some(timeout) => {
            raised = ProviderContext {
                timeout,
                ..ctx.clone()
            };
            &raised
        }
        None => ctx,
    };
    match build_provider(config) {
        Ok(provider) => {
            let name = provider.name().to_string();
            if !provider.available(ctx) {
                let reason = provider
                    .unavailable_reason()
                    .unwrap_or_else(|| "unavailable (missing tool or secret)".to_string());
                return (name, Err(reason));
            }
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| provider.fetch(ctx))) {
                Ok(Ok(items)) => (name, Ok(items)),
                Ok(Err(err)) => (name, Err(err.0)),
                Err(_) => (name, Err("provider panicked".to_string())),
            }
        }
        Err(err) => (
            config
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            Err(err.0),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A provider's context carries the organization the registry places its
    /// project in, so the summons a provider makes can be told it
    /// (§FS-005-dispatch.8).
    ///
    /// Membership and the root are absent independently, which is why all
    /// three answers are asserted here rather than one: an organization that
    /// declares no `root` still names itself, and a project no row places in
    /// one has no organization at all rather than an empty one. The empty
    /// strings a summons ends up carrying are [`crate::seams::dossier`]'s to
    /// make from this (§FS-005-dispatch.6.1).
    #[test]
    fn a_providers_context_is_told_which_organization_the_project_is_placed_in() {
        let registry = serde_json::json!({
            "organizations": [
                { "id": "foundation", "root": "/shared" },
                { "id": "personal" }
            ],
            "projects": [
                { "id": "demo", "root": "/r/demo", "organization": "foundation" },
                { "id": "mill", "root": "/r/mill", "organization": "personal" },
                { "id": "outland", "root": "/r/outland" }
            ]
        });
        let defaults: Defaults = serde_json::from_value(serde_json::json!({})).unwrap();
        let organization = |project: &str| {
            build_context(&registry, project, &defaults)
                .expect("the registry describes the project")
                .organization
        };

        assert_eq!(
            organization("demo"),
            Some(crate::branches::Organization {
                id: "foundation".to_string(),
                root: Some(paths::resolve_path("/shared")),
            })
        );
        assert_eq!(
            organization("mill"),
            Some(crate::branches::Organization {
                id: "personal".to_string(),
                root: None,
            })
        );
        assert_eq!(organization("outland"), None);
    }

    fn answer_matter(key: &str, at: &str, supplied: bool) -> crate::matter::Matter {
        crate::matter::Matter::of_item(&crate::feed::model::Item {
            id: key.to_string(),
            project: "demo".to_string(),
            source: "custom-status".to_string(),
            kind: crate::feed::model::ItemKind::Status,
            role: None,
            title: key.to_string(),
            url: None,
            state: Some("waiting".to_string()),
            needs_response: false,
            updated_at: at.parse().unwrap(),
            raw: crate::feed::model::custom_status_raw(
                serde_json::json!({}),
                Some(serde_json::json!({ "time_supplied": supplied, "terminal": null })),
            ),
        })
    }

    /// One store is read once, so what its answer dropped is reported once
    /// (§FS-005-dispatch.8). The root reaches the list by two roads — named
    /// outright, and answered for by `task_stores` — and a store read twice
    /// answers twice, which doubled every `note:` line on the error stream and
    /// in `refresh --json`.
    #[test]
    fn a_store_the_root_and_the_workspaces_both_name_is_read_once() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("demo");
        std::fs::create_dir_all(root.join("panta")).unwrap();
        let registry = serde_json::json!({
            "projects": [{ "id": "demo", "root": root.to_string_lossy() }]
        });
        let placement =
            crate::branches::Placement::load(&registry, "demo").expect("the row names a root");
        // Both roads answer for it: the guard is the list, not the roads.
        assert!(placement.task_stores().contains(&root));

        let stores = stores_to_read(&placement, None);
        let paths: Vec<_> = stores.iter().map(|store| store.path.clone()).collect();
        assert_eq!(paths, vec![root.join("panta")], "one store, read twice");
    }

    /// The root stays named in its own right, because `task_stores` probes the
    /// two store kinds while only `find` reads a manifest that declares one
    /// somewhere else (§FS-006-project-interface.2).
    #[test]
    fn a_manifest_declared_store_at_the_root_is_still_read() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("demo");
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();
        let registry = serde_json::json!({
            "projects": [{ "id": "demo", "root": root.to_string_lossy() }]
        });
        let placement =
            crate::branches::Placement::load(&registry, "demo").expect("the row names a root");
        // Nothing probes as a store here, so the other road answers for nothing.
        assert!(placement.task_stores().is_empty());

        let manifest = crate::manifest::Manifest {
            tasks: vec![crate::manifest::TaskStore {
                kind: "rhei".to_string(),
                path: "elsewhere".to_string(),
            }],
            ..Default::default()
        };
        let paths: Vec<_> = stores_to_read(&placement, Some(&manifest))
            .iter()
            .map(|store| store.path.clone())
            .collect();
        assert_eq!(paths, vec![root.join("elsewhere")]);
    }

    /// The previous-slot boundary retains omitted activity only for one
    /// continuous same-key observation, while every newly supplied time wins
    /// (§FS-006-project-interface.4, §FS-007-matters.1).
    #[test]
    fn custom_status_answer_reconciliation_tracks_one_retained_observation() {
        let first = answer_matter("poll:fixed", "2026-09-01T00:00:00Z", false);
        let omitted = answer_matter("poll:fixed", "2026-09-02T00:00:00Z", false);
        let retained = reconcile_custom_status_answer_matters(
            vec![omitted],
            Some(std::slice::from_ref(&first)),
        );
        assert_eq!(retained[0].updated_at, first.updated_at);

        let supplied = answer_matter("poll:fixed", "2026-09-03T00:00:00Z", true);
        let later_omission = answer_matter("poll:fixed", "2026-09-04T00:00:00Z", false);
        let retained = reconcile_custom_status_answer_matters(
            vec![later_omission],
            Some(std::slice::from_ref(&supplied)),
        );
        assert_eq!(retained[0].updated_at, supplied.updated_at);

        let disappeared =
            reconcile_custom_status_answer_matters(Vec::new(), Some(std::slice::from_ref(&first)));
        let reappeared = answer_matter("poll:fixed", "2026-09-05T00:00:00Z", false);
        let reappeared =
            reconcile_custom_status_answer_matters(vec![reappeared], Some(disappeared.as_slice()));
        assert_eq!(
            reappeared[0].updated_at,
            "2026-09-05T00:00:00Z"
                .parse::<chrono::DateTime<chrono::Utc>>()
                .unwrap()
        );

        let authoritative = answer_matter("poll:fixed", "2026-09-06T00:00:00Z", true);
        let authoritative = reconcile_custom_status_answer_matters(
            vec![authoritative],
            Some(std::slice::from_ref(&first)),
        );
        assert_eq!(
            authoritative[0].updated_at,
            "2026-09-06T00:00:00Z"
                .parse::<chrono::DateTime<chrono::Utc>>()
                .unwrap()
        );
    }

    /// §FS-011-command-line.9: a run started from the screen asks the
    /// projects it was handed — the ones that screen was opened over — so a
    /// screen narrowed to one organization does not reach the others when
    /// the reader presses the refresh key.
    #[test]
    fn a_background_run_asks_only_the_projects_it_was_given() {
        let config: StatusConfig = serde_json::from_value(serde_json::json!({
            "projects": {
                "ephor": { "providers": [] },
                "graal": { "providers": [] },
                "rhei": { "providers": [] }
            }
        }))
        .expect("a watch list");
        let asked = |over: &[&str]| -> Vec<String> {
            let over: Vec<String> = over.iter().map(|id| id.to_string()).collect();
            jobs_over(&config, &over)
                .into_iter()
                .map(|(project, _)| project)
                .collect()
        };

        assert_eq!(asked(&["ephor", "rhei"]), ["ephor", "rhei"]);
        // Detail's single project, and the whole watch list where the screen
        // was opened over all of it.
        assert_eq!(asked(&["graal"]), ["graal"]);
        assert_eq!(
            asked(&["ephor", "graal", "rhei"]),
            ["ephor", "graal", "rhei"]
        );
    }

    #[test]
    fn a_clean_run_says_only_that_it_refreshed() {
        assert_eq!(summarize(&[], 0, None), "Refreshed");
    }

    #[test]
    fn what_was_lost_is_named_and_the_rest_is_counted() {
        let lost: Vec<String> = ["a/one", "b/two", "c/three", "d/four", "e/five"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        assert_eq!(
            summarize(&lost, 0, None),
            "Refreshed — NO DATA from 5: a/one, b/two, c/three +2 more"
        );
    }

    /// A network condition asks nothing of the reader but a working network,
    /// so it is said in those words (§FS-001-forge-interface.6).
    #[test]
    fn unreachable_is_its_own_sentence() {
        let lost = vec!["graal/gdev".to_string()];
        assert!(summarize(&lost, 1, None).ends_with("(1 unreachable — check the network/VPN)"));
    }

    /// A project that could not be asked at all carries its reason: the name
    /// alone does not say what to go and fix.
    #[test]
    fn a_project_that_could_not_be_asked_says_why() {
        let lost = vec!["widget".to_string()];
        assert_eq!(
            summarize(&lost, 0, Some("widget: not in the registry")),
            "Refreshed — NO DATA from 1: widget — widget: not in the registry"
        );
    }
}
