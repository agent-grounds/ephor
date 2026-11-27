//! What opening the session is reading now, for a surface that waits on it
//! (§FS-019-waiting.5.1, §AR-009-surfaces.2).
//!
//! Opening the session reads the registry, every cached feed, where every
//! checked-out branch stands, what each project can do and the work handed
//! over. The interface draws while that runs, so the session says each step
//! as it begins, through a hook the caller hands it. A command waits on
//! nothing it could be told, so it opens the session through
//! [`super::Session::open_over`], which hands the steps to [`silent`]: the
//! same reading, in the same order, said to nobody.

/// One step of opening the session, as it begins. What is read and in which
/// order is the session's own; a step only names it (§FS-019-waiting.5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opening {
    /// The registry, what the reader has seen, and the work ledger.
    Registry,
    /// The cached feed of every project the session watches.
    Feeds { projects: usize },
    /// Where each checked-out branch stands (§AR-004-forest.2): `done` of
    /// `checkouts` measured so far. Said once before the first measurement
    /// and once after each, `done` rising by one each time.
    Branches { checkouts: usize, done: usize },
    /// What each project can do, rung by rung (§AR-005-capabilities.1): the
    /// forest and the manifest of its checkout, and the verbs on disk.
    Capabilities { projects: usize },
    /// What the work handed over about the feed's matters is doing
    /// (§FS-005-dispatch.4).
    Work,
    /// The jobs ephor started beneath an earlier screen (§FS-005-dispatch.17).
    Jobs,
    /// The work roots beneath the watch (§FS-005-dispatch.15).
    Roots,
}

/// Where the steps are said. `Sync`, because the branch measurements run on
/// worker threads and each says its own completion.
pub type Progress<'a> = &'a (dyn Fn(Opening) + Sync);

/// Says nothing: what a command opens the session with.
pub fn silent(_: Opening) {}
