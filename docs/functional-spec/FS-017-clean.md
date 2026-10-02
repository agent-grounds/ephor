# FS-017-clean: an idle checkout gives back what its builds took, through the project's own verb

Every branch checkout an agent works in keeps the build output it made, and
nobody comes back for it: a machine with forty checkouts fills its disk with
forty build trees, and the person finds out when nothing can be written. Ephor
already knows every project's checkouts and which of them a live run holds, so
giving the space back is a routine move that can leave the person's hands
([§GOAL-004-handover](../goals.md#goal-004-handover-routine-moves-leave-the-persons-hands)). What ephor does not know is how a project un-builds, any
more than how it builds: that is the project's knowledge, so cleaning is a verb
the project declares rather than a rule ephor carries
([§FS-006-project-interface.5](FS-006-project-interface.md#5-checks-are-verbs-and-every-script-is-self-contained), [§REQ-001-boundary.3](../requirements/REQ-001-boundary.md#3-requirements-on-a-project-are-capabilities-never-artifacts)).

## 1. Cleaning is a verb the project declares

The `clean` verb is bound exactly as a check verb is: probed as `./clean.sh` at
the checkout's root, declared under a path of the project's choosing by a
manifest's `clean` key beside `checks`, and overridden by the site
configuration's `projects.<id>.clean` — site configuration over manifest over
probe ([§FS-006-project-interface.1](FS-006-project-interface.md#1-the-three-homes)). A branch checkout carries its own
manifest and its own script, so the binding is resolved in each checkout rather
than once per project.

It runs as a summons ([§FS-006-project-interface.3](FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)) in the checkout, told what a
summons about a branch is told — the project, its root, the checkout as the
workspace, the branch — and its exit code is read the usual way: `0` done, `75`
parked, anything else failed. It is self-contained, and what it removes is its
own business: ephor neither inspects the tree for build output nor removes
anything itself.

A project that binds no verb gets no guessed fallback. Its checkouts are passed
over, saying that no clean verb is declared, and the *cleanable* rung it does
not hold says the same ([§FS-006-project-interface.10](FS-006-project-interface.md#10-capability-rung-by-rung)).

## 2. The sweep covers every checkout nobody is holding

`ephor clean` asks the verb of the branch checkouts on disk of every project the
scope selectors name ([§FS-011-command-line.9](FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)), and of every project in the
registry where none is given. The checkouts are the ones the rebase sweep
enumerates ([§FS-004-quick-actions.6.1](FS-004-quick-actions.md#61-the-same-replay-over-every-checkout-nobody-is-holding)), and two are never touched:

1. **The project's main-branch checkout.** Tools are installed from it and the
   directory belongs to `ephor update`, so it is not cleaned, and the report
   says so once per project. A project that names no main branch cannot have
   that checkout told from a branch's, so none of it is cleaned and the project
   is reported as not reached.
2. **A checkout a live run holds.** Removing a build under an agent mid-build
   loses work, so the tree is read the way the rebase sweep and the due sweep
   of `work run` read it ([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)), and the checkout is passed over
   with the run that holds it named.

A checkout passed over is always a row with its reason, never a silence.

Cleaning writes into trees, so the verb is summoned only under `--act`, at every
width, for the reason the rebase sweep gives: one project is still many
checkouts ([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)).

## 3. What a run says

Without `--act` the report lists what would be summoned where — the command,
and the checkout it would run in. It cannot know what a verb would give back
before the verb runs, so it names no number of bytes.

Under `--act` each checkout is measured before its verb runs and again after,
and the row says what was reclaimed, in human-readable bytes. A measurement
counts the space the tree occupies on disk: a symbolic link is not followed and
a file linked twice is counted once. The report ends with the total. A verb that
fails is reported on its row and the sweep goes on to the next checkout; the run
exits `1` where any verb failed or any project was not reached, and `0`
otherwise — cleaned, parked and passed over are all good ends. `--json` carries
the same facts as fields ([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).
