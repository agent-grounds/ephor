# DF-008-ephor-judges-a-monitors-health: ephor decides a monitor's health from the declared condition, site over manifest, and from the sample's own ok only where none is declared

**Status:** Accepted
**Date:** 2026-11-25

A project may now declare monitors: quantities ephor reads at their own
cadence, keeps as history, and draws on a page, where a monitor with a
condition that stops meeting it becomes a matter ([§FS-021-monitors](../../functional-spec/FS-021-monitors.md#fs-021-monitors-a-quantity-a-project-declares-is-read-on-its-own-cadence-and-kept-and-its-drop-is-a-matter)). Until
now a source decided that a matter exists: a custom-status script reports a
`matters[]` row, and ephor keeps no value, no history and no notion of
healthy ([§FS-006-project-interface.4](../../functional-spec/FS-006-project-interface.md#4-the-answer-envelope)). A monitor turns that around. The
source reports a value, and something has to decide whether that value is
healthy.

This record says who decides, and what follows from the answer: how a drop
is keyed, how a monitor's failures reach the refresh, and which work is
proposed for a drop. It is written so that a reader who finds one of the
refused alternatives absent meets a reason rather than an oversight
([§REQ-002-parity.2](../../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).

## 1. Decided

**Ephor judges, from the declared condition.** A monitor's health is
decided by ephor, from the first of these that exists: the site's
condition, the manifest's, the condition a self-described sample brought,
the sample's own `ok`, and the kind's default ([§FS-021-monitors.3.6](../../functional-spec/FS-021-monitors.md#36-ephor-decides-health-from-the-declared-condition)). This is
the one resolution order every seam keeps, site configuration over manifest
over what the project's side says on its own, with the shipped default last
([§REQ-001-boundary.2](../../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)). The person has the last word on what healthy means for
their watch, a repository can still ship sensible bounds, and a script that
knows only its own verdict can still say it where nothing is declared.
`"healthy": null` in any home ends the search, so the person can turn any
monitor into a KPI whatever its samples say. A monitor nothing judges is a
KPI: it is read, kept and drawn, and never a matter ([§FS-021-monitors.1.2](../../functional-spec/FS-021-monitors.md#12-a-kpi-is-a-monitor-nothing-judges)).

**A drop is an episode, and the episode is the key.** Because ephor judges,
ephor also knows when a monitor dropped, and it keys the matter by that
moment: `monitor:<project>/<id>@<episode>`, with the episode named by its
UTC time and no `:` in it ([§FS-021-monitors.11.3](../../functional-spec/FS-021-monitors.md#113-the-keys)). Ephor lays work once per
matter ([§FS-005-dispatch.4](../../functional-spec/FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), so without the episode the second drop after a
recovery would find its work already laid and nothing would be offered. An
unknown monitor is keyed apart, `<id>.unknown@<episode>`, because mending a
read is different work from mending the number.

**A monitor's failure is the monitor's, never the source's.** The monitor
machinery answers its slot exactly once on every plain refresh, and an
unreadable monitor is that monitor's state inside a good answer
([§FS-021-monitors.17.2](../../functional-spec/FS-021-monitors.md#172-one-monitors-failure-is-that-monitors)). Only a failure of the machinery itself fails the
slot ([§FS-021-monitors.17.3](../../functional-spec/FS-021-monitors.md#173-the-slot-fails-only-when-the-machinery-does)). Ephor deciding health is what makes this
possible: a broken read is a fact ephor judges, `unknown`, not an error the
source raises.

**The monitor's own workflow is proposed first.** A monitor's `workflow`, or
an `action` naming an entry that hands work over, the site's over the
manifest's, heads its drop matter's menu, ahead even of what ephor itself
recognized and of the project's offers and the person's actions, so it is the
proposal ([§FS-021-monitors.16.1](../../functional-spec/FS-021-monitors.md#161-the-monitors-own-workflow-comes-first)). Without one, the matter is proposed what
any matter is: the first entry that hands it over, and otherwise the bound
runtime's `triage` workflow ([§FS-019-waiting.7.1](../../functional-spec/FS-019-waiting.md#71-every-new-row-carries-a-proposal)). It is only a proposal: a
monitor never dispatches work by itself.

## 2. Rejected

**The declared condition only.** A script could then never say "I am down"
about a number nothing declared a bound for, and a self-described monitor
could never be a status. The sample's `ok` costs nothing to keep where
nothing is declared, and anything declared overrides it.

**The sample decides.** Taking the sample's `ok` over the declaration would
let the script that measures a value also be the one that excuses it, and
would leave the person no way to hold a repository to a stricter bound than
it chose for itself.

**The kind's default before the sample's `ok`.** A boolean or a level always
has a default, so a script's own verdict on one would never be heard. The
default is the shipped one, and a shipped default sits below what the
project says.

**The source decides that a matter exists, as custom-status does.** That
keeps no value and no history, so there is nothing to draw, no hold against a
flapping number, and no way to change a bound without changing the script.

**One matter per monitor, reopened on each drop.** The second drop would find
its work laid and done, and the ledger would read it as handled.

**One unreadable monitor fails the slot.** A failed slot keeps every monitor
of the project at its last good answer, the ones that just read fine
included, and a refresh's exit code would turn red for one script's mistake.

**The proposal as a generated offer and a generated action.** Written into
the project's offers and the site's actions, the manifest's workflow would
come before the site's in menu order, which inverts the precedence this
record decides. Teaching the proposal to read a field of the matter instead
would change the proposal for every matter, and the chooser with it.

## 3. Deferred

- A matter for `warn`. The band is shown in amber and never laid.
- Monitors ephor builds itself from the feeds it already has, such as a main
  branch's gate.
- A monitor's workflow proposed for its unknown matter as well as its drop.
