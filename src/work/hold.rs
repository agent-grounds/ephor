//! What stopped a due root from being started, as data
//! (§FS-005-dispatch.24.2).
//!
//! Every hold the sweep finds is decided here as a [`Hold`] and only then said:
//! the sentence a reader sees as `reason` is rendered from the same value the
//! `hold` object is, so the two can never be two accounts of one pass-over
//! (§AR-009-surfaces.1). Where another module owns a hold's wording — the
//! budget's sentence in [`super::spend`], the pools' clause in
//! [`super::headroom`] — the variant carries that wording beside the numbers
//! rather than reconstructing it, so not one character of a reason moves.

use std::path::PathBuf;

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Map, Value};

use super::spend::Scope;

/// The first hold that stopped one root, in the order the sweep asks them
/// (§FS-005-dispatch.24.2).
#[derive(Debug, Clone, PartialEq)]
pub enum Hold {
    /// The reader's `--except` named the root; `except` is the value as given.
    Excluded { except: String },
    /// Every ticket that would have made the root due sits in a top-level
    /// tree an open ticket in a gating state holds (§FS-005-dispatch.24.3.2).
    /// `tickets` is each gated ticket, plan-qualified, with the gating state
    /// it waits in.
    Person { tickets: Vec<(String, String)> },
    /// The last run advanced nothing and the root rests until `until`. `left`
    /// is how long that was when the sweep read it, in the words the reason
    /// uses — the sweep's clock, not the renderer's.
    Rested {
        run: String,
        count: u32,
        until: DateTime<Utc>,
        left: String,
    },
    /// Enough runs in a row advanced nothing that the sweep starts no more.
    Stopped { run: String, count: u32 },
    /// A live run started from `root` holds the checkout; `run` is the id it
    /// published, where it published one.
    Tree { root: PathBuf, run: Option<String> },
    /// A plan needs pools that cannot be had together
    /// (§FS-005-dispatch.33). `plan` is the plan directory, `entry` the
    /// entry the sentence names, `clause` the clause [`super::headroom`]
    /// rendered.
    Pools {
        plan: String,
        entry: String,
        pools: Vec<String>,
        pool: String,
        until: Option<DateTime<Utc>>,
        clause: String,
    },
    /// A ceiling on roots in flight or on working roots is full.
    Concurrency {
        scope: Scope,
        key: Flight,
        limit: usize,
        count: usize,
    },
    /// A ceiling on what unattended work may spend is full
    /// (§FS-015-spend-ceiling.9). `says` is the sentence the budget decided.
    Budget {
        scope: Scope,
        limit: Amount,
        total: Option<Amount>,
        until: Option<String>,
        says: String,
    },
}

/// Which of a scope's two concurrency keys is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flight {
    /// `max_concurrent`, over every live root.
    Concurrent,
    /// `max_active`, over the working ones; `parked` is the live roots
    /// waiting on a person, which the sentence names beside the count.
    Active { parked: usize },
}

/// A budget figure in the denomination its key is written in: dollars under
/// `max_spend`, tokens under `max_tokens`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Amount {
    Dollars(f64),
    Tokens(u64),
}

impl Amount {
    fn key(&self) -> &'static str {
        match self {
            Amount::Dollars(_) => "max_spend",
            Amount::Tokens(_) => "max_tokens",
        }
    }

    fn number(&self) -> Value {
        match self {
            Amount::Dollars(dollars) => json!(dollars),
            Amount::Tokens(tokens) => json!(tokens),
        }
    }
}

impl Hold {
    /// The kind a program selects on (§FS-005-dispatch.24.2).
    pub fn kind(&self) -> &'static str {
        match self {
            Hold::Excluded { .. } => "excluded",
            Hold::Person { .. } => "person",
            Hold::Rested { .. } => "rested",
            Hold::Stopped { .. } => "stopped",
            Hold::Tree { .. } => "tree",
            Hold::Pools { .. } => "pools",
            Hold::Concurrency { .. } => "concurrency",
            Hold::Budget { .. } => "budget",
        }
    }

    /// The sentence a reader is told, word for word what each hold said
    /// before it was data (§FS-005-dispatch.24, §FS-005-dispatch.24.2).
    pub fn says(&self) -> String {
        match self {
            Hold::Excluded { except } => {
                format!("--except {except} — left out of this sweep at your asking")
            }
            Hold::Person { tickets } => {
                let gates: Vec<String> = tickets
                    .iter()
                    .map(|(ticket, state)| format!("{ticket} is in '{state}'"))
                    .collect();
                format!(
                    "it waits on a person — {}; it is started again once that ticket moves",
                    gates.join(", ")
                )
            }
            Hold::Rested { run, left, .. } => {
                format!("the last run here ({run}) advanced nothing — this root is tried again in {left}")
            }
            Hold::Stopped { run, count } => format!(
                "{count} runs in a row here advanced nothing, the last of them {run} — nothing \
                 more will be started on this root until a run advances there or you start one \
                 by hand"
            ),
            Hold::Tree { root, run } => {
                let named = run.clone().unwrap_or_else(|| root.display().to_string());
                format!("a run is live in this checkout: {named}")
            }
            Hold::Pools { entry, clause, .. } => {
                format!("the plan '{entry}' laid {clause}. The plan stays where it is.")
            }
            Hold::Concurrency {
                scope,
                key: Flight::Concurrent,
                limit,
                count,
            } => format!(
                "{}.max_concurrent {limit} is full ({count} live run(s))",
                scope.key()
            ),
            Hold::Concurrency {
                scope,
                key: Flight::Active { parked },
                limit,
                count,
            } => format!(
                "{}.max_active {limit} is full ({count} active run(s), {parked} parked)",
                scope.key()
            ),
            Hold::Budget { says, .. } => says.clone(),
        }
    }

    /// The `hold` object a passed-over row carries: `kind` and the numbers
    /// the hold rests on, with every absent member left out rather than null
    /// (§FS-005-dispatch.24.2).
    pub fn data(&self) -> Value {
        let mut object = Map::new();
        object.insert("kind".into(), json!(self.kind()));
        match self {
            Hold::Excluded { except } => {
                object.insert("except".into(), json!(except));
            }
            Hold::Person { tickets } => {
                let tickets: Vec<Value> = tickets
                    .iter()
                    .map(|(ticket, state)| json!({ "ticket": ticket, "state": state }))
                    .collect();
                object.insert("tickets".into(), Value::Array(tickets));
            }
            Hold::Rested {
                run, count, until, ..
            } => {
                object.insert("run".into(), json!(run));
                object.insert("count".into(), json!(count));
                object.insert(
                    "until".into(),
                    json!(until.to_rfc3339_opts(SecondsFormat::Secs, true)),
                );
            }
            Hold::Stopped { run, count } => {
                object.insert("run".into(), json!(run));
                object.insert("count".into(), json!(count));
            }
            Hold::Tree { root, run } => {
                object.insert("root".into(), json!(root));
                if let Some(run) = run {
                    object.insert("run".into(), json!(run));
                }
            }
            Hold::Pools {
                plan,
                pools,
                pool,
                until,
                ..
            } => {
                object.insert("plan".into(), json!(plan));
                object.insert("pools".into(), json!(pools));
                object.insert("pool".into(), json!(pool));
                if let Some(until) = until {
                    object.insert("until".into(), json!(super::headroom::instant(until)));
                }
            }
            Hold::Concurrency {
                scope,
                key,
                limit,
                count,
            } => {
                scoped(&mut object, scope);
                let key = match key {
                    Flight::Concurrent => "max_concurrent",
                    Flight::Active { .. } => "max_active",
                };
                object.insert("key".into(), json!(key));
                object.insert("limit".into(), json!(limit));
                object.insert("count".into(), json!(count));
            }
            Hold::Budget {
                scope,
                limit,
                total,
                until,
                ..
            } => {
                scoped(&mut object, scope);
                object.insert("key".into(), json!(limit.key()));
                object.insert("limit".into(), limit.number());
                if let Some(total) = total {
                    object.insert("total".into(), total.number());
                }
                if let Some(until) = until {
                    object.insert("until".into(), json!(until));
                }
            }
        }
        Value::Object(object)
    }
}

/// `scope` as `site`, `organization` or `project`, and `id` beside the two
/// that name one (§FS-005-dispatch.24.2).
fn scoped(object: &mut Map<String, Value>, scope: &Scope) {
    let (name, id) = match scope {
        Scope::Site => ("site", None),
        Scope::Organization(id) => ("organization", Some(id)),
        Scope::Project(id) => ("project", Some(id)),
    };
    object.insert("scope".into(), json!(name));
    if let Some(id) = id {
        object.insert("id".into(), json!(id));
    }
}
