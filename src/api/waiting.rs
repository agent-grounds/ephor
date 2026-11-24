//! The first screen's reading (§FS-019-waiting): what waits on the reader,
//! what nothing has been done about, and what is running — one reading the
//! screen draws and `ephor waiting` prints (§FS-019-waiting.6).
//!
//! Gates, person polls and live runs come from the board's own walk of the
//! work roots ([`Session::board_over`]), never a second scanner
//! (§FS-005-dispatch.15). New and held matters come from the feed the overview
//! draws, measured against the ledger (§FS-005-dispatch.4). Each new matter
//! carries the rhei it would be assigned, read off its own menu
//! (§FS-019-waiting.7.1).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::{DateTime, Utc};

use crate::feed::cache;
use crate::feed::model::{Item, ItemRole};
use crate::work::runtime::watch::{Doing, RootPlans};

use super::read::BoardReading;
use super::views;
use super::Session;

/// The names a poll gives whoever its work is being done for — on this
/// machine, the reader (§FS-019-waiting.1.1).
const READER: [&str; 2] = ["you", "owner"];

impl Session {
    /// The reading, with the work roots enumerated on the spot — what a
    /// command does (§FS-019-waiting.6).
    pub fn waiting(&mut self) -> views::Waiting {
        let groups = self.work_roots();
        self.waiting_over(&groups)
    }

    /// The reading over an enumeration the caller already holds — what the
    /// interface does between ticks, exactly as the board does
    /// (§FS-005-dispatch.15.1). Mutable because each new row's proposal is
    /// its menu's, and a menu asks the runtime what it offers
    /// (§FS-019-waiting.7.1).
    pub fn waiting_over(&mut self, groups: &[RootPlans]) -> views::Waiting {
        let board = self.board_over(groups);
        let now = Utc::now();
        let items = self.items();
        let overview = views::Overview {
            matters: items.len(),
            unread: items
                .iter()
                .filter(|item| cache::is_unread(&self.seen, item))
                .count(),
            // The newest fetch any feed in the reading records
            // (§FS-019-waiting.3).
            refreshed_at: self.feeds.iter().filter_map(|feed| feed.fetched_at).max(),
        };
        // Processed: a rhei is assigned, or a person said none is needed and
        // the matter has not moved since (§FS-019-waiting.7).
        let processed = |item: &Item| self.has_work(&item.id) || self.dismissed(item);
        let holds = self.holding_labels();
        let review_id = |item: Option<&str>, plan_id: &str, ticket: &str| {
            item.filter(|id| self.item(id).is_some())
                .map(str::to_string)
                .or_else(|| {
                    let own = crate::work::runtime::own_task_id(plan_id, ticket);
                    self.item(&own).map(|_| own)
                })
        };
        let mut reading = reading(board, &review_id, &self.work_config);
        // What only the runtime knows about each gate — its brief's title, how
        // long it has waited, what it holds — asked once per work root, and
        // only of a runtime that prepares briefs (§FS-019-waiting.1.1.1).
        if crate::work::runtime::gate::briefs(&self.work_config) {
            let config = &self.work_config;
            listed(&mut reading.on_you, &|root| {
                crate::work::runtime::gate::list(config, root).ok()
            });
        }
        // The entry and branch the ledger records for each live run's matter
        // (§FS-019-waiting.1.3), and what the reader decided today
        // (§FS-019-waiting.2): ephor's own records, read and never written.
        if let Some(dispatcher) = &self.dispatcher {
            for row in &mut reading.running {
                let Some(entry) = row
                    .item
                    .as_ref()
                    .and_then(|id| dispatcher.ledger.entries.get(id))
                else {
                    continue;
                };
                let last = crate::work::retired::standing(entry).last();
                row.entry = last
                    .map(|dispatch| dispatch.recipe.clone())
                    .filter(|recipe| !recipe.is_empty());
                row.branch = last
                    .and_then(|dispatch| dispatch.branch.clone())
                    .or_else(|| entry.branch.clone());
            }
            reading.decided = decided(&dispatcher.ledger, chrono::Local::now());
        }
        // The matters the feed carries, and not the saved reply rows the
        // overview keeps beside them: a drafted reply is not a matter nothing
        // has been done about (§FS-019-waiting.1.2).
        let shown: Vec<(Item, bool)> = self
            .feeds
            .iter()
            .flat_map(|feed| feed.items())
            .filter(|item| self.shows(item, now))
            .map(|item| {
                let unread = cache::is_unread(&self.seen, &item);
                (item, unread)
            })
            .collect();
        let (mut new, held) = unprocessed(shown.iter().cloned(), &processed, &holds);
        let live: BTreeSet<String> = reading
            .running
            .iter()
            .filter_map(|row| row.item.clone())
            .collect();
        let on_you: BTreeSet<String> = reading
            .on_you
            .iter()
            .filter_map(|row| row.item.clone())
            .collect();
        reading.queued = self.queued_rows(&live, &on_you);
        let by_id: BTreeMap<&str, &Item> = shown
            .iter()
            .map(|(item, _)| (item.id.as_str(), item))
            .collect();
        for row in &mut new {
            if let Some(item) = by_id.get(row.id.as_str()) {
                row.proposal = self.proposal_of(item);
            }
        }
        reading.new = new;
        reading.held = held;
        reading.adopt = self.adopt();
        reading.overview = overview;
        reading
    }

    /// Every label some selector of the site excludes by name: what holds a
    /// matter out of the machine's hands (§FS-019-waiting.1.2). Read from the
    /// site's actions and recipes and from each project's, so ephor names no
    /// label of its own.
    pub fn holding_labels(&self) -> BTreeSet<String> {
        let selectors = self
            .actions
            .iter()
            .chain(self.project_actions.values().flatten())
            .map(|action| &action.when)
            .chain(self.work_config.recipes.iter().map(|recipe| &recipe.when))
            .chain(
                self.config
                    .projects
                    .values()
                    .flat_map(|project| project.work.recipes.iter())
                    .map(|recipe| &recipe.when),
            );
        selectors
            .flat_map(|selector| selector.labels.iter())
            .filter_map(|label| label.strip_prefix('!'))
            .map(str::to_string)
            .collect()
    }
}

/// The sentences both surfaces say about the reading, so the screen and the
/// command cannot word one fact two ways (§FS-019-waiting.3).
impl views::Waiting {
    /// `ephor — waiting   3 on you · 5 new · 4 running · refreshed 2m ago`.
    pub fn header(&self) -> String {
        format!(
            "ephor — waiting   {} on you · {} new · {} running · {}",
            self.on_you.len(),
            self.new.len(),
            self.running.len(),
            self.refreshed(Utc::now())
        )
    }

    /// `overview: 143 matters · 12 changed since you looked`.
    pub fn overview_line(&self) -> String {
        let noun = if self.overview.matters == 1 {
            "matter"
        } else {
            "matters"
        };
        format!(
            "overview: {} {noun} · {} changed since you looked",
            self.overview.matters, self.overview.unread
        )
    }

    /// Whether neither of the first two sections has a row — the screen
    /// then opens with [`QUIET`] (§FS-019-waiting.2).
    pub fn quiet(&self) -> bool {
        self.on_you.is_empty() && self.new.is_empty()
    }

    /// The whole answer a quiet screen gives: [`QUIET`], and what it is the
    /// answer to (§FS-019-waiting.2).
    pub fn quiet_answer(&self) -> String {
        let which = match self.held.is_empty() {
            true => "every matter",
            false => "every matter not held",
        };
        format!("{QUIET} No gate is open and {which} has work laid or a recorded decision.")
    }

    /// `decided today: 5 laid · 1 needs no work` (§FS-019-waiting.2).
    pub fn decided_line(&self) -> String {
        format!(
            "decided today: {} laid · {} needs no work",
            self.decided.laid, self.decided.dismissed
        )
    }

    /// `refreshed 2m ago`, or `never refreshed` (§FS-019-waiting.3).
    pub fn refreshed(&self, now: DateTime<Utc>) -> String {
        match self.overview.refreshed_at {
            Some(at) => format!("refreshed {}", crate::feed::render::ago(now, at)),
            None => "never refreshed".to_string(),
        }
    }
}

/// What the reader decided since local midnight, counted from the processing
/// marks and the dismissals (§FS-019-waiting.2).
pub(crate) fn decided(
    ledger: &crate::work::ledger::Ledger,
    now: chrono::DateTime<chrono::Local>,
) -> views::Decided {
    let today = now.date_naive();
    let on_today = |at: &DateTime<Utc>| at.with_timezone(&chrono::Local).date_naive() == today;
    views::Decided {
        laid: ledger
            .processed
            .values()
            .filter(|mark| on_today(&mark.at))
            .count(),
        dismissed: ledger
            .dismissed
            .values()
            .filter(|dismissal| on_today(&dismissal.at))
            .count(),
    }
}

impl views::Waiting {
    /// `queued 2 · ephor#119, fissile#78 · waiting for a slot (max_active 4)`:
    /// the one line under the live runs naming what processing laid that has
    /// not started, with the admission's own reason (§FS-019-waiting.7.6).
    pub fn queued_line(&self) -> Option<String> {
        let last = self.queued.last()?;
        let ids: Vec<&str> = self.queued.iter().map(|row| short_id(&row.id)).collect();
        let why = last
            .says
            .clone()
            .unwrap_or_else(|| super::process::NO_START_NOW.to_string());
        Some(format!(
            "queued {} · {} · {why}",
            self.queued.len(),
            ids.join(", ")
        ))
    }
}

impl views::Proposal {
    /// What a new row says about the rhei it would be assigned
    /// (§FS-019-waiting.7.1, §FS-019-waiting.1.2): the entry, the hand and
    /// the branch where it names them, how many others would take it, and why
    /// it cannot be processed as it stands, where it cannot.
    pub fn says(&self) -> String {
        if self.fallback && self.gate == "blocked" {
            return format!("→ {} · not available", self.entry);
        }
        if let Some(refusal) = &self.refusal {
            return format!("→ {} · {}", self.entry, one_line(refusal));
        }
        let mut says = format!("→ {}", self.entry);
        for named in [&self.hand, &self.branch].into_iter().flatten() {
            says.push_str(&format!("  {}", one_line(named)));
        }
        if self.gate == "needs-checkout" {
            says.push_str(" · makes its checkout");
        }
        if self.others > 0 {
            says.push_str(&format!(" · {} more", self.others));
        }
        says
    }
}

/// A sentence held to one line.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A matter's id as a person says it: `ephor#119` for
/// `github-issues:agent-grounds/ephor#119`, and the id after its source
/// otherwise.
pub(crate) fn short_id(id: &str) -> &str {
    let rest = id.rsplit(':').next().unwrap_or(id);
    match rest.contains('#') {
        true => rest.rsplit('/').next().unwrap_or(rest),
        false => rest,
    }
}

/// What an empty Waiting screen opens with (§FS-019-waiting.2).
pub const QUIET: &str = "Nothing is waiting on you.";

impl views::OnYou {
    /// What the row says beside its title: the state a gate is parked in, and
    /// whom a poll waits on (§FS-019-waiting.1.1), with how long a gate has
    /// waited and what it holds where the runtime listed them
    /// (§FS-019-waiting.1.1.1).
    pub fn says(&self) -> String {
        self.says_at(Utc::now())
    }

    /// [`Self::says`], counted from `now`.
    pub fn says_at(&self, now: DateTime<Utc>) -> String {
        if let Some(person) = &self.waiting_on {
            return format!("waiting on {person} · not yours to move here");
        }
        let mut says = self.state.clone();
        match self.since {
            // Unknown is said as unknown, never as no time at all.
            Some(None) => says.push_str(" · waited unknown"),
            Some(Some(at)) => match crate::feed::render::age(now, at).as_str() {
                "now" => says.push_str(" · waited under a minute"),
                age => says.push_str(&format!(" · waited {age}")),
            },
            None => {}
        }
        if let Some(holds) = self.holds {
            says.push_str(" · ");
            says.push_str(&crate::work::runtime::gate::say_holds(holds));
        }
        says
    }
}

/// The runtime's listing laid over the enumeration's gate rows: each root that
/// holds a gate row is listed once, and a row the listing names in the same
/// state takes its title, start and holds. The listing adds no row and takes
/// none away; a row it does not name, or names in another state, or whose root
/// could not be listed, stays as it was. Then the rows whose start is known
/// lead, oldest first, and every other row keeps its place behind them
/// (§FS-019-waiting.1.1.1).
pub(crate) fn listed(
    rows: &mut [views::OnYou],
    list: &dyn Fn(&Path) -> Option<Vec<crate::work::runtime::gate::Listed>>,
) {
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    for row in rows.iter().filter(|row| row.kind == "gate") {
        if !roots.contains(&row.root) {
            roots.push(row.root.clone());
        }
    }
    for root in roots {
        let Some(gates) = list(&root) else {
            continue;
        };
        for row in rows
            .iter_mut()
            .filter(|row| row.kind == "gate" && row.root == root)
        {
            let Some(gate) = gates
                .iter()
                .find(|gate| gate.ticket == row.ticket && gate.state == row.state)
            else {
                continue;
            };
            if !gate.title.trim().is_empty() {
                row.title = gate.title.clone();
            }
            // A start the runtime wrote and this reader cannot read is as
            // unknown as one it did not write.
            row.since = Some(
                gate.since
                    .as_deref()
                    .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                    .map(|at| at.with_timezone(&Utc)),
            );
            row.holds = Some(gate.holds);
        }
    }
    // A stable sort, so the rows with no known start keep the enumeration's
    // order among themselves, behind the ones that have one.
    rows.sort_by_key(|row| match row.since {
        Some(Some(at)) => (false, Some(at)),
        _ => (true, None),
    });
}

/// Which feed id a gate is reviewed by, asked with the ticket's matter, its
/// plan and its id; None where the feed carries none.
pub(crate) type ReviewedBy<'a> = &'a dyn Fn(Option<&str>, &str, &str) -> Option<String>;

/// The board's half of the reading: the gates and the polls on the reader,
/// and every live run (§FS-019-waiting.1.1, §FS-019-waiting.1.3). `review`
/// names the feed id a gate is reviewed by, where the feed carries one.
pub(crate) fn reading(
    board: BoardReading,
    review: ReviewedBy<'_>,
    config: &crate::work::recipe::WorkConfig,
) -> views::Waiting {
    let BoardReading {
        running,
        asks,
        refusal,
        matters,
    } = board;
    let matter = |id: Option<&str>| {
        id.and_then(|id| matters.get(id))
            .map(|item| item.title.clone())
    };
    // Gates first within a root and polls after them, then the whole list held
    // to the enumeration's order: project, root, plan order
    // (§FS-019-waiting.1.1).
    let mut on_you: Vec<views::OnYou> = Vec::new();
    for row in &running {
        for ticket in &row.op.tickets {
            if !matches!(ticket.doing, Doing::Waiting) {
                continue;
            }
            on_you.push(views::OnYou {
                kind: "gate",
                ticket: format!("{}.{}", ticket.plan_id, ticket.ticket),
                title: named(&ticket.ticket_title, &ticket.title),
                state: ticket.state.clone().unwrap_or_else(|| "?".to_string()),
                project: row.op.project.clone(),
                root: row.op.root.clone(),
                plan: ticket.plan.clone(),
                item: ticket.item.clone(),
                matter: matter(ticket.item.as_deref()),
                waiting_on: None,
                movable: true,
                review: review(ticket.item.as_deref(), &ticket.plan_id, &ticket.ticket),
                since: None,
                holds: None,
            });
        }
    }
    for asked in &asks {
        let ask = &asked.ask;
        if !names_reader(&ask.waiting_on, asked.item.as_ref()) {
            continue;
        }
        on_you.push(views::OnYou {
            kind: "poll",
            ticket: format!("{}.{}", ask.plan_id, ask.ticket),
            title: named(&ask.ticket_title, ""),
            state: ask.state.clone(),
            project: ask.project.clone(),
            root: ask.root.clone(),
            plan: ask.plan.clone(),
            item: ask.item.clone(),
            matter: matter(ask.item.as_deref()),
            waiting_on: Some(ask.waiting_on.clone()),
            movable: false,
            review: None,
            since: None,
            holds: None,
        });
    }
    let order: Vec<&Path> = running
        .iter()
        .map(|row| row.op.root.as_path())
        .chain(asks.iter().map(|asked| asked.ask.root.as_path()))
        .collect();
    let seen = |root: &Path| order.iter().position(|known| *known == root);
    on_you.sort_by(|a, b| {
        a.project
            .cmp(&b.project)
            .then_with(|| seen(&a.root).cmp(&seen(&b.root)))
    });

    let running = running
        .iter()
        .filter(|row| row.op.live)
        .map(|row| views::RunningRoot {
            root: row.op.root.clone(),
            project: row.op.project.clone(),
            item: row.op.item().map(str::to_string),
            matter: matter(row.op.item()),
            plan: row.plan.clone(),
            entry: None,
            branch: None,
            tickets: row
                .op
                .tickets
                .iter()
                .filter(|ticket| matches!(ticket.doing, Doing::Running))
                .map(|ticket| views::RunningTicket {
                    ticket: format!("{}.{}", ticket.plan_id, ticket.ticket),
                    title: named(&ticket.ticket_title, &ticket.title),
                    state: ticket.state.clone(),
                })
                .collect(),
            run: row.op.run_id().map(str::to_string),
            // The start the run published beside its lock, and nothing else:
            // a run that published none shows no age (§FS-019-waiting.1.3).
            started_at: row
                .op
                .identity
                .as_ref()
                .and_then(|run| run.started_at.as_deref())
                .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                .map(|at| at.with_timezone(&Utc)),
            attach: row
                .op
                .run_id()
                .map(|id| crate::work::runtime::attach_command(config, id)),
        })
        .collect();

    views::Waiting {
        refusal,
        on_you,
        new: Vec::new(),
        held: Vec::new(),
        running,
        queued: Vec::new(),
        adopt: None,
        decided: views::Decided::default(),
        overview: views::Overview::default(),
    }
}

/// The ticket's own heading, and the plan's title only where the ticket has
/// none (§FS-019-waiting.1.1).
fn named(ticket: &str, plan: &str) -> String {
    match ticket.trim().is_empty() {
        true => plan.to_string(),
        false => ticket.to_string(),
    }
}

/// Whether the person a poll declares is the reader: whoever the work is done
/// for, or the reader's own role on the matter (§FS-019-waiting.1.1).
pub(crate) fn names_reader(person: &str, item: Option<&Item>) -> bool {
    let person = person.trim().to_ascii_lowercase();
    if READER.contains(&person.as_str()) {
        return true;
    }
    match item.and_then(|item| item.role) {
        Some(ItemRole::Author) => person == "author",
        Some(ItemRole::Reviewer) => person == "reviewer",
        None => false,
    }
}

/// The feed's half: every matter the feed shows that nothing has been done
/// about, newest first, and the held ones apart (§FS-019-waiting.1.2). `items`
/// are what the feed shows, each with whether it is unread; `processed` asks
/// the ledger, for work and for a standing dismissal (§FS-019-waiting.7.4).
/// Each new row's proposal is left for the caller, which reads the menus.
pub(crate) fn unprocessed(
    items: impl IntoIterator<Item = (Item, bool)>,
    processed: &dyn Fn(&Item) -> bool,
    holds: &BTreeSet<String>,
) -> (Vec<views::NewMatter>, Vec<views::HeldMatter>) {
    let mut new = Vec::new();
    let mut held = Vec::new();
    for (item, unread) in items {
        if item.is_finished() || processed(&item) || crate::work::runtime::is_own_task(&item) {
            continue;
        }
        match super::process::holding_label(&item, holds) {
            Some(label) => held.push(views::HeldMatter {
                id: item.id,
                project: item.project,
                kind: item.kind,
                title: item.title,
                label,
            }),
            None => new.push(views::NewMatter {
                id: item.id,
                project: item.project,
                kind: item.kind,
                title: item.title,
                url: item.url,
                updated_at: item.updated_at,
                unread,
                proposal: views::Proposal::default(),
            }),
        }
    }
    new.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    (new, held)
}

#[cfg(test)]
#[path = "waiting_tests.rs"]
mod tests;
