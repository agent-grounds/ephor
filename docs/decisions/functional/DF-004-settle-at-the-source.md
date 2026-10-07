# DF-004-settle-at-the-source: a conversation is put away at its source by a capability of its own, and only by the reader's move

**Status:** Accepted
**Date:** 2026-10-08

A conversation reported under **Messages by reason** can now be settled at
the network it came from: a source declares **Settle**
([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities)), and the reader asks for it with `ephor settle` or
the key `s` ([§FS-011-command-line.4](../../functional-spec/FS-011-command-line.md#4-a-conversation-and-the-moves-inside-it)). What settling means there is the
source's: a mail thread archived, a chat marked done. Ephor knows only that a
conversation has a place it can be put away from, and nothing about how a
network spells it, the same way it knows a task has a state and a way to
transition it ([§FS-004-quick-actions.5](../../functional-spec/FS-004-quick-actions.md#5-a-task-is-ticked-where-it-is-read)).

This record was written because a matter finished in ephor stayed open where it
came from. The reader answered a mail from the feed and marked the row done,
and then opened the mail client to archive the thread, where the next
unrelated message was waiting. That is the sweep
[§GOAL-003-nothing-lost](../../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep) wants retired, run afterwards instead of before,
and the last step of the move [§GOAL-001-fewest-moves](../../goals.md#goal-001-fewest-moves-the-most-frequent-response-is-the-cheapest-one) lists as "mark it
done". What follows is what was decided, what was weighed and refused, and what
was named and left for later, so that a reader who finds one of these absent
meets a reason rather than an oversight
([§REQ-002-parity.2](../../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).

## 1. Decided

**One row joins the capability set, and no row changes.** A source that
declares `settle` is sent the conversation's own `id`, exactly as it reported
it, in the request's existing `target`; the request gains no field. It answers
`{}` when the network accepted, or fails with a diagnosis. A repeat is
success, so a settle that timed out is run again and nothing like reply
reconciliation is needed. The source reports whether the network accepted,
and ephor never reads a settled state back as its own done or answered
([§FS-001-forge-interface.3](../../functional-spec/FS-001-forge-interface.md#3-policy-lives-above-the-interface-never-in-an-implementation)).

**The id, not a descriptor.** Settling acts on the conversation, which is one
row however many threads it has, and a conversation nobody can answer from
ephor may still be one the reader wants put away. `restart` already works this
way: an identity the source reported, and a declared capability.

**The row carries one duty ephor cannot check.** A later message in a settled
conversation must still reach Messages by reason, so that the matter comes
back and says why ([§FS-007-matters.5](../../functional-spec/FS-007-matters.md#5-an-event-moves-state-and-resurfacing-names-its-reason)). A source whose venue keeps a settled
conversation out of the view it reads reads that view too, or does not declare
the capability. A conversation the source stops listing until it moves again
is what archiving means; a message that lands nowhere is a silent drop.

**It is the reader's move and nothing else's.** Reading, the local mark done,
`mark-read` and `reply` keep exactly their meaning and never settle; dispatch,
sync, autorun and recipes never call it. A runtime may run `ephor settle`
deliberately, as a person would ([§GOAL-004-handover](../../goals.md#goal-004-handover-routine-moves-leave-the-persons-hands)).

**Accepted, the row is done; settled is not answered.** Once the source
accepts, the row gets the done mark the local move writes, and comes back when
the conversation moves. Whether a thread awaits the reader is still read off
its messages ([§FS-003-feed-categories.4](../../functional-spec/FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it)).

**Offered where it would work, refused by name elsewhere.** The key appears
only on a conversation whose source declared the capability
([§FS-004-quick-actions.2](../../functional-spec/FS-004-quick-actions.md#2-offered-only-where-it-would-work)), and every other row is refused with a sentence
that names the source, before anything is sent
([§REQ-001-boundary.1](../../requirements/REQ-001-boundary.md#1-the-anatomy)). A name the capability set does not have, such as
`archive`, still offers nothing. The move goes back to the source that
reported the conversation, wherever it is bound and wherever the conversation
was placed ([§FS-001-forge-interface.9](../../functional-spec/FS-001-forge-interface.md#9-a-source-is-bound-to-one-project-or-to-the-site-and-every-move-goes-back-to-the-source)).

**The word is "settle".** It is no network's spelling, and it is neither
"done" nor "read", which stay the local move
([§REQ-001-boundary.5](../../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)). The key, the command, `--json` and the parity entry
land together ([§REQ-002-parity](../../requirements/REQ-002-parity.md#req-002-parity-every-ability-is-reachable-without-the-screen-and-every-answer-has-a-machine-form)).

## 2. Rejected

**The local mark done also settling.** It is the cheapest form, and it turns a
reversible local move into an irreversible remote one under the same key: a
reader who only meant to clear the row archives the mail.

**Settling as a side effect of `reply`.** Many replies do not finish a
conversation, and the reader is the one who knows when it is finished. It would
also change the reply request.

**Gateways archiving on their own**, for example when ephor's reply arrives.
Ephor never learns of it, so the dry run, the refusal by name and the JSON form
are lost, and it is the automatic behaviour this record rules out.

**Folding it into `resolve-task`.** A task belongs to one message and has a
state the reader reads beside that message; a conversation's place in an inbox
is not a task on any message.

**A per-conversation settle descriptor.** It would be a second identity for
what the `id` already names.

**A confirmation prompt on `s`.** Ticking and posting write without one, and
settling can be undone at the source.

## 3. Deferred

These are named and not done; none is needed for a conversation to be settled.

- Settling a notice, a pull request or an issue at its source.
- A bulk settle over several rows.
- A built-in source that declares the capability.
