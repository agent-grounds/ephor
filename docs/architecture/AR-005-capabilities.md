# AR-005-capabilities: availability is computed once and consulted everywhere

The ladder of [§FS-006-project-interface.10](../functional-spec/FS-006-project-interface.md#10-capability-rung-by-rung) is realized as a **capability
table**: one `CapabilitySet` per project, resolved during refresh and on
demand, recording for each rung whether it holds and — where it does not —
the one sentence that says why. Everything that offers, gates, or refuses
reads this table; nothing else runs its own `command_exists` or path check.

## 1. Resolution

Each rung names its establishment and is resolved accordingly
([§FS-006-project-interface.1](../functional-spec/FS-006-project-interface.md#1-the-three-homes)): *placed* and *checkable* and *tasks* by
probing the checkout, and *cleanable* by asking the clean verb's seam, which
weighs the site binding over the manifest over the probe; *checkout-able*,
*workable*, and parts of *gated* by looking up bindings; *observable* and
*branch-addressable* from the row and the sources' last answers. Resolution is cheap by construction — stat
calls, config lookups, no spawning — so it can rerun whenever the world
may have moved (a refresh, a checkout that appeared).

## 2. Features declare needs

A feature — a menu entry, a recipe, a dispatch, a shipped CI step — carries
the list of rungs it needs. Offering is filtering on the table; refusing is
rendering the first missing rung's sentence. This gives [§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)'s
degrade rule one implementation: the sentence a person sees on a gated menu
entry, the reason `work dispatch` prints, and the log line a shipped step
emits are the same text from the same place. A pool a piece of work requires is
a need like any other and refuses like one: the clause naming which pools it
needs together and which of them cannot be had is rendered here, so the menu
row, the command line and the log carry one text rather than three
([§FS-005-dispatch.33](../functional-spec/FS-005-dispatch.md#33-work-that-needs-several-pools-at-once-is-admitted-whole)).

## 3. The table is honest about time

A capability is what held when it was resolved. The executor re-checks the
one rung a summons is about to lean on (the place exists, the binding is
still on disk) at invocation, because a table that answers from ten minutes
ago and a directory deleted nine minutes ago must fail as the world, not as
a lie ([§FS-001-forge-interface.6](../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not) has the same stance toward sources).

## 4. A runtime's own features are asked of it, once

Some of what a surface offers depends not on a rung but on what the bound
runtime can do: whether it starts a run beneath the screen
([§FS-005-dispatch.20](../functional-spec/FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)), whether it prepares a gate's brief and routes its answer
([§FS-005-dispatch.9.3](../functional-spec/FS-005-dispatch.md#93-a-prepared-gate-is-answered-in-its-own-form)). These are **runtime features**, answered by the runtime
module alone ([§AR-007-runtime.1](AR-007-runtime.md#1-what-the-module-owns)) and asked of the runner itself — its own help
text, its own schema verb — never of a version number. Each is resolved the
first time a surface needs it and kept for the rest of the process, per bound
runner, so a menu built on every keystroke never forks the runner to rebuild
it. A feature is false wherever the workable rung does not hold, and false for
a runner that could not be asked; the surface then offers exactly what it
offered before the feature existed ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)). The features are
named by the summons verbs they cover: `work.run.detaches` for the detached
run, `work.gate` for `work.gate.show` and `work.gate.answer` and the gate verbs
added after them.
