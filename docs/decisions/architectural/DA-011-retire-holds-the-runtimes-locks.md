# DA-011-retire-holds-the-runtimes-locks: retiring holds the runtime's own locks, rather than asking the runtime

**Status:** Accepted
**Date:** 2026-11-12

Retiring a finished matter's work moves its plans out of the work roots and into
an archive ([§FS-005-dispatch.36](../../functional-spec/FS-005-dispatch.md#36-a-finished-matters-work-can-be-retired-and-the-record-says-where-it-went)). Something has to keep a run from
having those files open while they move, and something has to say the move
happened. Three hands could do it: the runtime, asked for a transition as
cancelling asks it; ephor, after probing that nothing is running; or ephor,
holding the runtime's own exclusion while it moves. This record fixes the third.

## 1. The decision

Ephor takes, and holds for the length of the move, **every lock the runtime
itself keeps for a file the move takes**: each affected root's run lock in the
runtime's canonical order, then the writer lock of every file that leaves
([§FS-005-dispatch.36.3](../../functional-spec/FS-005-dispatch.md#363-it-holds-the-runtimes-own-exclusion-and-asks-the-runtime-to-move-nothing)). They are taken non-blocking, so a
lock already held is a refusal and never a wait, and they are released only once
the ledger has committed or the move has rolled back. A missing lock file is
created as the runtime creates it; none is written into, broken, or moved.

Knowing which files are locks, and in what order the runtime takes them, is
the runtime's grammar, so it lives in the runtime module beside the probe the
board already reads ([§AR-007-runtime.1](../../architecture/AR-007-runtime.md#1-what-the-module-owns)), and nothing above it spells a
lock path ([§REQ-001-boundary.5](../../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)).

## 2. The rejected alternatives

**A transition, as cancelling is
([§DA-005-cancel-is-the-runtimes-move](DA-005-cancel-is-the-runtimes-move.md#da-005-cancel-is-the-runtimes-move-cancelling-a-ticket-asks-the-runtime-and-never-rewrites-the-state-line)).** Cancelling is the runtime's move
because it changes a ticket's state, and the runtime's verb is what keeps that
change honest. Retiring changes no state, and the transition is the wrong tool
for it twice over: it would rewrite every open ticket as abandoned, which is
not what happened to work that was overtaken by a merge, and it would fire the
machine's hooks — the ones that comment, label or close — against an issue the
forge already closed. The archive is supposed to be the plan as it was.

**A probe, then the move.** The board already asks whether a root is live with a
try-lock it releases at once. Asking that and then moving leaves a window
between the answer and the first rename, and a run started in it opens files
that are about to leave. A guard that is true only at the moment it was asked
is not a guard on the move.

**Asking the runtime to hold its locks for ephor.** It would be the cleanest
seam, and the runtime has no such verb. Waiting for one would leave the matter
this exists for unsolved; when one exists, the runtime module can ask it
instead and nothing above the module changes.

## 3. The cost

For the seconds ephor holds a root's locks, a run started there waits, or fails
fast and is retried by the next autorun turn, as it would against any other
holder. A retired directory workspace leaves its lock files and the directories
holding them behind, because a lock file that moves is a lock two processes can
each believe they hold; neither tool reads what stays as a plan. And ephor now
follows the runtime's locking rule rather than only probing one lock: a change
in which files the runtime locks is a change this module has to follow, which
is why the rule is written as *every lock kept for a file the move takes* and
not as a list of names.
