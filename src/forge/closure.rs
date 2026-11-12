//! §FS-001-forge-interface.1, Closure: whether one issue or pull request is
//! over, and what is still open about it, asked by number at the moment it
//! matters rather than read from a refresh.
//!
//! The answer is facts only. Whether those facts allow a retirement is
//! §FS-005-dispatch.36.1's, decided above the interface
//! (§FS-001-forge-interface.3). Every list here carries whether it is
//! complete, because a list the forge cut short read as everything is the one
//! answer this capability must never give.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What the item asked about turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Subject {
    Issue,
    PullRequest,
}

/// The state of an issue or a pull request, in the forge's words lowercased:
/// an issue is `open` or `closed`, a pull request may also be `merged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Open,
    Closed,
    Merged,
}

impl State {
    pub fn word(self) -> &'static str {
        match self {
            State::Open => "open",
            State::Closed => "closed",
            State::Merged => "merged",
        }
    }
}

/// One pull request the answer names: one linked as closing the issue, or one
/// still open from a head branch that was asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pull {
    pub number: u64,
    pub state: State,
    /// When it merged, in the forge's own spelling.
    pub merged_at: Option<String>,
    /// Its head branch.
    pub branch: String,
    /// The owner of the repository the head was pushed to, so a fork's head
    /// is told apart from the base's. `None` where the forge no longer knows
    /// it (a deleted fork).
    pub owner: Option<String>,
}

/// A list and whether it is all of it (§FS-001-forge-interface.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    pub pulls: Vec<Pull>,
    pub complete: bool,
}

impl Listing {
    pub fn whole(pulls: Vec<Pull>) -> Listing {
        Listing {
            pulls,
            complete: true,
        }
    }
}

/// The forge's answer about one item (§FS-001-forge-interface.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Closure {
    pub subject: Subject,
    pub state: State,
    /// When it closed, in the forge's own spelling.
    pub closed_at: Option<String>,
    /// For a pull request: its own head, the branch it came from.
    pub head: Option<Pull>,
    /// For an issue: every pull request the forge links as closing it, open,
    /// merged or closed. Empty and complete for a pull request.
    pub closing: Listing,
    /// For each head branch asked about, and each closing pull request's head,
    /// the pull requests still open from a branch of that name, whoever owns
    /// the repository it was pushed to.
    pub open: BTreeMap<String, Listing>,
}

/// Why there is no answer. Each is its own sentence at the surface
/// (§FS-005-dispatch.36.1), so they are kept apart here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosureError {
    /// The source's implementation does not declare the capability.
    Undeclared,
    /// The item is neither an issue nor a pull request.
    Neither,
    /// The lookup failed, or answered with errors: the forge's own words.
    Failed(String),
}
