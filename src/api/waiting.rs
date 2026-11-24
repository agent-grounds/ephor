//! The first screen's reading (§FS-019-waiting): what waits on the reader,
//! what nothing has been done about, and what is running — one reading the
//! screen draws and `ephor waiting` prints (§FS-019-waiting.6).
//!
//! Gates, person polls and live runs come from the board's own walk of the
//! work roots ([`Session::board_over`]), never a second scanner
//! (§FS-005-dispatch.15). New and held matters come from the feed the overview
//! draws, measured against the ledger (§FS-005-dispatch.4).

use std::collections::BTreeSet;
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
    /// (§FS-005-dispatch.15.1).
    pub fn waiting_over(&self, groups: &[RootPlans]) -> views::Waiting {
        let board = self.board_over(groups);
        let now = Utc::now();
        let items = self.items();
        let overview = views::Overview {
            matters: items.len(),
            unread: items
                .iter()
                .filter(|item| cache::is_unread(&self.seen, item))
                .count(),
        };
        let processed = |id: &str| {
            self.dispatcher
                .as_ref()
                .is_some_and(|dispatcher| dispatcher.ledger.entries.contains_key(id))
        };
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
        // The matters the feed carries, and not the saved reply rows the
        // overview keeps beside them: a drafted reply is not a matter nothing
        // has been done about (§FS-019-waiting.1.2).
        let (new, held) = unprocessed(
            self.feeds
                .iter()
                .flat_map(|feed| feed.items())
                .filter(|item| self.shows(item, now))
                .map(|item| {
                    let unread = cache::is_unread(&self.seen, &item);
                    (item, unread)
                }),
            &processed,
            &holds,
        );
        reading.new = new;
        reading.held = held;
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
    /// `ephor — waiting   3 on you · 5 new · 4 running`.
    pub fn header(&self) -> String {
        format!(
            "ephor — waiting   {} on you · {} new · {} running",
            self.on_you.len(),
            self.new.len(),
            self.running.len()
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
}

/// What an empty Waiting screen opens with (§FS-019-waiting.2).
pub const QUIET: &str = "Nothing is waiting on you.";

impl views::OnYou {
    /// What the row says beside its title: the state a gate is parked in, and
    /// whom a poll waits on (§FS-019-waiting.1.1).
    pub fn says(&self) -> String {
        match &self.waiting_on {
            Some(person) => format!("waiting on {person} · not yours to move here"),
            None => self.state.clone(),
        }
    }
}

/// The board's half of the reading: the gates and the polls on the reader,
/// and every live run (§FS-019-waiting.1.1, §FS-019-waiting.1.3). `review`
/// names the feed id a gate is reviewed by, where the feed carries one.
pub(crate) fn reading(
    board: BoardReading,
    review: &dyn Fn(Option<&str>, &str, &str) -> Option<String>,
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
/// the ledger.
pub(crate) fn unprocessed(
    items: impl IntoIterator<Item = (Item, bool)>,
    processed: &dyn Fn(&str) -> bool,
    holds: &BTreeSet<String>,
) -> (Vec<views::NewMatter>, Vec<views::HeldMatter>) {
    let mut new = Vec::new();
    let mut held = Vec::new();
    for (item, unread) in items {
        if item.is_finished() || processed(&item.id) || crate::work::runtime::is_own_task(&item) {
            continue;
        }
        let label = item
            .raw
            .get("labels")
            .and_then(|labels| labels.as_array())
            .into_iter()
            .flatten()
            .filter_map(|label| label.as_str())
            .find(|label| holds.contains(*label))
            .map(str::to_string);
        match label {
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
            }),
        }
    }
    new.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    (new, held)
}

#[cfg(test)]
#[path = "waiting_tests.rs"]
mod tests;
