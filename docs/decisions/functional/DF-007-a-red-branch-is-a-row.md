# DF-007-a-red-branch-is-a-row: a watched branch's gate is a row of its own, keyed by the branch and judged workflow by workflow

**Status:** Accepted
**Date:** 2026-10-17

A `github-ci` source can now watch named branches as well as open pull
requests. A source that names `branches` asks, for each repository and each
branch, for the gate of the branch itself, and the feed holds one `ci` row per
branch while that gate is red. The row is gone on the first refresh that finds
the branch green. The capability is **Branch gate**
([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities)). An extension forge declares it as
`branch_gate` and answers `branch-gates`
([§FS-001-forge-interface.2](../../functional-spec/FS-001-forge-interface.md#2-two-transports-one-interface)), and the decision that a red branch needs
the reader sits above both
([§FS-001-forge-interface.3](../../functional-spec/FS-001-forge-interface.md#3-policy-lives-above-the-interface-never-in-an-implementation)), so a branch on an extension forge goes red
and resolves the same way.

This record was written because a red `main` went unseen for five hours. A
flaky test failed the required job of a push run on a watched repository's
default branch. `ephor feed --kind ci` said *No feed items*, and the red stayed
until someone ran `gh run list --branch main` by hand. That hand sweep is the
one [§GOAL-003-nothing-lost](../../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep) wants retired: an empty CI section has to
mean that no watched gate is red, not only that no open pull request's gate is.
What follows is what was decided, what was weighed and refused, and what was
named and left for later, so that a reader who finds one of these absent meets
a reason rather than an oversight
([§REQ-002-parity.2](../../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).

## 1. Decided

**`branches`, not `default_branch`.** The key is a list of branch names,
`"branches": ["main"]`, watched on every repository the source names. A
`default_branch: true` would cost a second request per repository per refresh
to learn which branch that is, and it would hide in the configuration which
branch is actually being watched. A site that renames its default branch
changes one line, and a release branch is watched the same way.

**The verdict is per workflow.** The branch is red while any workflow's newest
verdict is red. Judging it by the single newest run instead reads one push that
ran a red `ci.yml` and a green `docs.yml` as green whenever the docs finished
last. The workflow is the forge's stable id for it, never its name. *Newest* is
by run number or creation time, never by the last update, so an old red run
rerun until it goes green does not clear a newer red one. A run in progress, or
one that ended without a verdict, is passed over and the verdict before it
stands. Only runs the branch itself made count: the head is in the watched
repository, and no pull request's event started the run. A fork's pull request
from its own `main` is listed under `main`, and so is a run it triggers, and
neither may turn the watched `main` red.

**The row is the branch, not the run.** Its key is the repository and the
branch, `github-ci:acme/widget@main`, so a second red run updates the row
rather than adding one, and a green run resolves it
([§FS-007-matters.1](../../functional-spec/FS-007-matters.md#1-a-matter-is-a-subject-with-a-stated-identity)). The report marks its subject as a branch
(`watched_branch` in its `raw`) instead of leaving it to be inferred from
`branch`, which a pull request's report carries too, so a branch named
`release/2` is never read as pull request 2 and is never merged with a
`hotfix/2`.

**Scheduled runs count.** A run the schedule started on the branch is a run on
the branch. The weekly *Auto bump* on ephor, grund and fissile ends red while
their CI is green, so those `main` rows go red on the first refresh after this
lands. That is the watch working, so nothing is filtered out. The row's title
names the red workflow, so it says *Auto bump* and does not read as CI.

**The keys are the source's scripts, and the commands serve extensions.** A
branch row is offered the three red-gate entries every red gate gets: *see the
CI failures*, *restart what failed* and *restart the whole gate*
([§FS-004-quick-actions.4](../../functional-spec/FS-004-quick-actions.md#4-failing-ci-answers-what-failed-and-why), [§FS-004-quick-actions.9](../../functional-spec/FS-004-quick-actions.md#9-a-gate-is-offered-the-restart-in-two-shapes)). On `github-ci` they are
the source's own quick-action scripts, with the red runs written into them from
the row, and each is reached without the screen as `ephor actions run`
([§REQ-002-parity.1](../../requirements/REQ-002-parity.md#1-an-ability-is-a-key-that-reveals-a-fact-or-changes-the-world)). `github-ci` answers neither `ephor failures` nor
`ephor restart` today, not even for a pull request's row, and it gains neither
here. The `--branch` subject of those two commands, with their `--json`, is
served by extension forges that declare `branch_gate`
([§FS-011-command-line.6](../../functional-spec/FS-011-command-line.md#6-the-gate-spelled-out)).

**The cost is one listing per branch.** `github-ci` reads a branch with one
`gh api` listing of its newest hundred runs, plus one jobs listing per red run,
on the hourly REST meter ([§FS-001-forge-interface.8.4](../../functional-spec/FS-001-forge-interface.md#84-a-watched-branch-is-one-listing-and-a-red-one-a-listing-more)). `gh run list` and
`gh run view` spend two and three requests on the same answer. A source without
`branches` asks exactly what it asked before.

## 2. Rejected

**`default_branch: true`**, for the two reasons above: a request per refresh
to learn a name the site already knows, and a watch that does not say what it
watches.

**The latest completed run alone**, which the ticket first asked for. It is one
row of the listing instead of one per workflow, and it hides a red behind any
green that finished later.

**A key per run.** Every red run would become a new row, and "resolves on the
first green" could never hold, because no green run names the red one.

**Leaving scheduled runs out.** It would make the *Auto bump* rows go away,
and with them a red nightly that is exactly what a hand sweep would catch.

**`github-notifications`' `ci_activity`.** It is off by default and arrives
only for runs the reader started, so a merge-queue push or another person's push
never reaches it. It has no gate shape and never resolves on green.

**A provider of its own for branch runs.** It would repeat `github-ci`'s
authentication, host and quick-action wiring, and it would still need the same
capability.

## 3. Deferred

These are named and not done; none is needed for a red branch to reach the
feed.

- `fix-gate` on a branch row. That recipe selects pull requests and CI items
  the reader authored, and a branch has no author, so it does not pick the row
  up. Whether a red `main` should be handed to an agent is its own decision.
- `ephor failures` and `ephor restart` answered by `github-ci` itself, for pull
  requests and branches alike.
- Watching a branch by pattern, such as `release/*`.
