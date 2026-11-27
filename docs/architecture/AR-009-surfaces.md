# AR-009-surfaces: one API beneath both surfaces, and one schema per answer

[§REQ-002-parity](../requirements/REQ-002-parity.md#req-002-parity-every-ability-is-reachable-without-the-screen-and-every-answer-has-a-machine-form) says the command line and the interface offer the same
abilities. This page says how that is true by construction rather than by
diligence: there is exactly one implementation of every ability, it lives
below both surfaces, and what it returns is a declared type that serializes
to a published schema.

A surface — the CLI, the TUI, the status widget — is presentation over this
API and nothing else ([§AR-001-layers.1](AR-001-layers.md#1-the-layers)). It may choose what to show, how to
lay it out, and which keys reach which call. It may not compute an answer of
its own.

## 1. The API is readings and moves

`src/api/` is the whole surface-facing API. It has two kinds of entry point
and no third:

- **A reading** takes what it is asked about and returns a view — a plain
  serializable value describing a fact. It changes nothing, and calling it
  twice is the same as calling it once. `actions`, `branches`, `operations`,
  `conversation` and `work_of` are readings; so are `work_entries`,
  `work_offers` and `running`, which answer the same questions in the terms a
  screen draws in rather than the terms a command prints — and which return
  their refusal rather than an empty list, because an absence with no reason on
  it reads as an oversight ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)); and so are the two that reach a
  source rather than the cache — the gate behind `ephor failures` and the
  roster behind `ephor capabilities` — which live with their adapters and
  answer the same way.
- **A move** takes a request and returns an outcome — a plain serializable
  value describing what changed. Every move is total about its refusals: it
  returns the sentence rather than printing one, because the same refusal
  has to reach a greyed menu row and a JSON field ([§AR-005-capabilities.2](AR-005-capabilities.md#2-features-declare-needs)).

The two surfaces are then thin by force. `ephor actions run` and the menu's
Enter call the same move with the same request and differ only in what they
do with the outcome; the TUI renders it into the status line and the CLI
prints it as prose or as JSON ([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)). That holds for every move
an entry can be — running one here, starting one beneath the screen, opening
a ticket, laying a workflow down — and for the readings behind them: what may
be done here, what work is on the table, and what is running are each derived
once and rendered twice.

Where a screen has to *ask* for something the command line takes as an
argument, the asking is the surface's and the move is still the API's: the
workflow key prompts for one missing scalar and opens an editor for anything
larger ([§FS-005-dispatch.19](../functional-spec/FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)), then hands what it collected to the same call
`--values` and `--set` feed. A screen that asked and then laid the plan down itself would
be a second implementation of one move, which is how two surfaces come to
disagree about what a workflow was answered with.

## 2. The session is built once and shared

Opening the screen reuses the session's initial feed, standing and work load;
it does not repeat that load before drawing. Screen-specific initialization
still enumerates work groups, builds the view, records the initial work
baseline and discovers job news. Later explicit reloads and completed refreshes
continue to reload the session and rebuild all dependent screen state. The
initial reading remains complete: concurrent standing measurements
([§AR-004-forest.2](AR-004-forest.md#2-probes-not-declarations)) finish before it is presented,
serving the first glance ([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now)).

That load runs behind the screen rather than in front of it
([§FS-019-waiting.5.1](../functional-spec/FS-019-waiting.md#51-the-interface-is-on-the-screen-before-it-reads-anything)). The interface enters the terminal, then opens the
session and initializes the screen state on the main thread, where it always
ran, while a thread of its own draws a frame naming the step under way and
reads only the keys that quit; what the load read goes to the event loop
whole once it lands. The load keeps its thread because a load moved off it was
measurably slower: the allocator grows a second thread's heap a page at a
time, and the first reading allocates enough for that to show. The session
says its steps through a hook its opener passes in: a command opens the same
session through the opening that hands every step to a silent hook, so
neither its signature nor its reading changes, and the steps are said where
the reading already happens rather than read back off it. The standing
workers say each measurement as it finishes, counted under one lock, so the
count reaches the screen in the order it rises.

A quit shows the cursor and restores the terminal at once, then ends the
process with success, as the event loop's own `ctrl-c` does; while the load
still ran in front of the terminal, `ctrl-c` there was an interrupt, and the
process died of it. The load cannot be stopped where it is, so the process
ends under it, except in the one place the load writes: the sweep of jobs
long over ([§FS-005-dispatch.17](../functional-spec/FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)) removes each one's directory, and a quit that comes
during the sweep waits for it to finish, so no directory is left without the
record a later sweep would find it by. A failure in the load comes back to the caller unchanged
([§FS-019-waiting.5.1](../functional-spec/FS-019-waiting.md#51-the-interface-is-on-the-screen-before-it-reads-anything)), and wins over one the frame met, which says only that
the terminal stopped answering; a frame that fails gives the terminal back
straight away, so `ctrl-c` is an interrupt again for the rest of the load. A
panic on any thread stops the frame, after the draw under way and before the
terminal is given back to print the panic, so no frame lands on the message.

The interactive screen also observes known feed-cache files locally, without
reloading the session or fetching from a forge. Its freshness baseline is the
displayed feed's reading, not a later metadata probe. A newer valid cache makes
the screen identify its older snapshot and offer `r`; normal feed loads and
per-project absorption retire that indication only for snapshots that caught
up ([§FS-001-forge-interface.7.1](../functional-spec/FS-001-forge-interface.md#71-a-displayed-snapshot-older-than-the-cache-says-so)).

Both surfaces open the same `Session`: the feeds, what is seen, where each
project is placed ([§AR-004-forest.3](AR-004-forest.md#3-workspace-resolution)), what each project can do
([§AR-005-capabilities](AR-005-capabilities.md#ar-005-capabilities-availability-is-computed-once-and-consulted-everywhere)), the branch standings, the configured entries, the
provider blocks — and the one dispatcher that writes the work ledger. It is
the state the interface used to hold privately, moved below the screen so
that a command answers from exactly the data a key would have answered from.

*One* of each, and that is the load-bearing word. A surface holding a second
dispatcher on top of the session's own saves its dispatch through that one
and leaves the session reading a ledger the dispatch has already moved past —
two answers to "what work is there", from one process, a keystroke apart. The
same reasoning makes the registry a single read per run: several things want
it in one invocation, and four reads are four chances to answer from four
different registries.

A command that needs one fact still opens the session — it is a read of
cache and configuration, not a fetch — because a cheaper path that skipped
the placement would answer a different question than the screen does, and
two answers to "can this project be checked out" is the failure
[§AR-005-capabilities](AR-005-capabilities.md#ar-005-capabilities-availability-is-computed-once-and-consulted-everywhere) exists to prevent.

## 3. Every machine form has a published shape

What `--json` prints is not an internal struct that happens to be
`Serialize`. Each shape has an entry in a JSON schema shipped with the binary
and printable by name ([§FS-006-project-interface.11](../functional-spec/FS-006-project-interface.md#11-the-interface-is-versioned)), and the schemas are the
stability surface that [§REQ-002-parity.4](../requirements/REQ-002-parity.md#4-the-machine-form-is-a-contract-not-a-dump) promises: what a release may change
is answerable by diffing them.

The schemas live in `assets/` beside the manifest, answer, registry and forge
schemas, and `ephor schema <name>` prints any of them verbatim;
`ephor schema views` is the one document holding every reading and every
outcome. A shape that gained a field gains it in the schema in the same
change.

What holds that is a walk of the **command tree**, not of a list somebody
keeps: a test resolves every `--json` clap knows about and fails on one that
names no shape, and two more hold every named shape to the published document
and back. A command that gains a machine form without a schema therefore
fails the build — which is the direction the drift comes from, since it is a
command that grows a `--json`, never a schema that loses an entry.

Those three only ever ask whether a *name* is on both lists, and a schema
describing something else entirely passes all of them — which is what happened:
one shape was published as an object while its command has always printed an
array, and half a dozen fields were declared as strings where the code prints
`null`. So the shapes are also held to what the commands actually print:
`tests/e2e/cases/E2E-012-command-line-parity.rs` runs every one of them and
validates its output against the published subschema, and its table of what it
swept is checked against the shape list in both directions, so a shape nobody
validates fails rather than passing quietly. A declared shape nobody validates
against is a declaration; [§REQ-002-parity.4](../requirements/REQ-002-parity.md#4-the-machine-form-is-a-contract-not-a-dump) promises a contract.

One shape is not a command's: under `--json`, a command that is *refused*
prints an `outcome` with `ok` false, whatever it would have printed had it
worked. The refusal is a reading too — a program that reads only standard
output has to learn that the thing did not happen, and prose on the error
stream beside an empty standard output reads to it exactly like a move that
worked and had nothing to say.

A few shapes are declared **by reference**. `ephor feed`, `ephor status` and
`ephor list` print documents another published schema already describes: a
source's own matters (`ephor schema forge`) and the registry's own rows
(`ephor schema registry`). Their entries say so and stop there, because a
second description of one document is a second thing to keep true, and the
two would drift the moment a binding gained a field.

## 4. Where the code sits

```
src/api/mod.rs           the API's own surface: what a caller reaches for
src/api/session.rs       the Session — feeds, placements, capabilities, work
src/api/views.rs         the view and outcome types — serializable, no IO
src/api/offers.rs        what may be done here: entries, their gates, their hands
src/api/conversation.rs  a matter's messages, walked once for both surfaces
src/api/read.rs          the readings: actions, branches, operations, work
src/api/act.rs           the moves: run an entry, hand work over, react, tick, reply
src/api/schema.rs        which command prints which shape, and the schema each publishes
src/api/parity.rs        the ability list of §REQ-002-parity.5
```

`views.rs` is data and pure functions, which is where [§AR-001-layers.1](AR-001-layers.md#1-the-layers) puts
it. The rest is engine: it reads the cache, resolves bindings, and calls the
one executor ([§AR-002-summons](AR-002-summons.md#ar-002-summons-one-executor-runs-everything-ephor-asks-of-the-world)) for anything that reaches the world.

`src/cli.rs` with `src/commands.rs`, and `src/feed/tui/`, are the surfaces.
Neither declares a view type, and neither spawns a process for an ability: a
surface that ran its own `sh -c` would be a second executor with its own
environment, which is how the menu and the command line come to disagree about
where an entry runs ([§AR-002-summons.1](AR-002-summons.md#1-resolving-the-place)).

## 5. The seam between them is checked

The ability list of [§REQ-002-parity.5](../requirements/REQ-002-parity.md#5-the-parity-list-is-checked-not-remembered) lives in `src/api/parity.rs`, and it is
held from both ends. Two tests beside it resolve every command it names, and
every flag it names, against the actual command tree — so renaming a command
out from under the law fails rather than leaving the list pointing at nothing.
`scripts/check_parity.py` holds the other end, and holds it three ways: every
key the interface binds is an ability on that list or an exemption on its
presentation list; every ability's command actually takes `--json`, asked of
the built binary rather than of anybody's memory; and a binding the script
cannot read — a key decided by a constant, a variant it has no name for — is
reported rather than skipped. A check that quietly ignores what it does not
understand grows blind spots, and a blind spot in a parity gate is a key with
nothing behind it shipping green. It runs in `just check` and in CI, beside
`scripts/check_boundary.py`, and for the same reason — parity observed as a
convention is parity that has already drifted.

### 5.1 Cargo chooses the executable the parity gate inspects

Before it asks the command tree for help, `scripts/check_parity.py` resolves
Cargo's effective target directory by running `cargo metadata --no-deps
--format-version 1 --locked` from the repository root. The absolute
`target_directory` in that answer is authoritative: Cargo therefore owns the
precedence and relative-path semantics of `CARGO_TARGET_DIR`, configuration,
and the ordinary checkout-local `target/`, and an executable in some other
target directory cannot mask the configured one.

Within that directory the debug executable is first, then the release
executable. If neither exists, the check runs one `cargo build --quiet
--locked` from the same root and environment, and proceeds only when the
debug executable then exists. It launches the selected path directly. This
keeps selection and the standalone build on the same Cargo configuration and
makes the executable itself, rather than source or memory, the evidence for
the parity law ([§REQ-002-parity.5](../requirements/REQ-002-parity.md#5-the-parity-list-is-checked-not-remembered)).

Failure to resolve the target, build or find the executable, or ask the
selected executable for help is an operational failure, distinct from a
parity finding. It exits 2 with a `parity: cannot resolve/build/launch ephor`
diagnostic on stderr, the Cargo or operating-system reason, every candidate
path known at that point, and a command that rebuilds in the resolved target
directory from the repository root. It never exposes a Python traceback.
Once an executable launches, missing machine forms — including a nonzero
help response — remain parity findings and exit 1; a clean check remains 0.

What is not yet mechanical is the direction of the dependency: that no surface
reaches past `crate::api` to a provider, a binding, or the executor. It is the
rule this page states, and it is **not yet true of the tree**. It holds for
every *ability*: the interface reaches no provider and posts nothing itself —
`react`, `tick` and `reply` were the last three that did, and a screen that
posted its own reply had a second copy of the half that retires the draft. What
it does not yet hold for is the interface's own housekeeping: `src/feed/tui/`
still calls `crate::seams::summons` to hand its terminal to a command, and
`crate::seams::jobs` to sweep, list and tail the jobs a row is drawn from. Those
are the same one executor and the same one job store the API uses, so there is
no second implementation of anything — but they are reached around the API
rather than through it, and until they are not, this paragraph says so rather
than claiming a rule the tree does not keep. The check is the boundary script's
to grow, alongside the layer list it already holds ([§AR-001-layers.2](AR-001-layers.md#2-where-literals-live)).
