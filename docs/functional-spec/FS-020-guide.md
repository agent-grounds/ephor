# FS-020-guide: how work gets into the shapes ephor reads is asked of the ephor that will read it

Whoever writes a workflow, and an agent about to stop at a gate, has to know
what ephor will show and what it expects back: what a gate's brief holds and
where its answer goes ([§FS-005-dispatch.9.3](FS-005-dispatch.md#93-a-prepared-gate-is-answered-in-its-own-form)), and what makes a matter new and
which rhei it is offered ([§FS-019-waiting.7](FS-019-waiting.md#7-a-new-matter-is-processed-where-it-is-shown)). A document in a repository
answers for the version it was written against. The binary on the machine is
the version that will read the work, so the explanation is a command,
`ephor guide`, and what it says is always the installed version's. A workflow
written to it is one whose gates a person answers in their form rather than in
prose, which is what lets a routine move leave the person's hands
([§GOAL-004-handover](../goals.md#goal-004-handover-routine-moves-leave-the-persons-hands)).

## 1. `ephor guide` lists the topics

`ephor guide` with no topic lists each topic by its name, with one line saying
what it explains, and says how to ask for one. A topic it does not know is
refused, and the refusal names the topics there are. Today there are two:
`gate` ([§FS-020-guide.2](FS-020-guide.md#2-gate-how-a-workflow-prepares-the-question-its-gate-asks)) and `matter` ([§FS-020-guide.3](FS-020-guide.md#3-matter-what-makes-a-matter-new-and-how-a-workflow-becomes-its-proposal)).

The text is written for an agent to read: short, plain, no decoration, and a
topic's explanation fits one screen of 80 columns and 48 lines. A topic that
ends with a worked example puts it after the explanation, and the example is
as long as a complete one has to be: it is the part a reader copies rather
than reads, and one cut down to fit a screen would not run. It reads
the cache, the disk and the bound runtime, as every reading does; it fetches
nothing from a forge and writes nothing.

## 2. `gate`: how a workflow prepares the question its gate asks

`ephor guide gate` explains, in one screen, the brief of
[§FS-005-dispatch.9.3](FS-005-dispatch.md#93-a-prepared-gate-is-answered-in-its-own-form) in the bound runtime's own plan language:

- declaring the brief on the gating state;
- naming the one file to show;
- asking the questions, either written by the state before the gate as a
  request file or declared once in the machine;
- the four kinds of question: a yes or no, one choice, several choices, and
  text, each required unless it says otherwise;
- how routes turn the answers into the destination, which is the runtime's to
  decide ([§DA-012-the-runtime-routes-a-gate-answer](../decisions/architectural/DA-012-the-runtime-routes-a-gate-answer.md#da-012-the-runtime-routes-a-gate-answer-the-runtime-turns-a-gates-answers-into-its-move-and-ephor-only-sends-them));
- where the answer is recorded, every path relative to the ticket's
  execution root as the runtime defines it, which for a plan in a directory
  of its own is that directory and not the work root above it; and
- how the state the answer moves the ticket into reads it.

It ends with one complete worked example, a machine and the request its
pre-gate state writes, and the commands that check it: the runtime's own
validation, its own reading of the brief and its own dry run of an answer,
and `ephor work review --json` and `ephor work gate answer --dry-run`, which
are what ephor makes of the same gate. The example is held to the runtime it
is written for: a guide whose example that runtime refuses is a defect.

The plan language is the runtime's, so this text is the runtime binding's to
say, and nothing above the binding names the runtime ([§REQ-001-boundary.5](../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)).
The commands in it are spelled with the runner the site binds, so the line an
agent copies is the line that runs here. The prose is the same whether or not
the bound runtime can brief a gate, because it describes the format the
binding reads; whether the runtime here can is said beside it
([§FS-020-guide.4](FS-020-guide.md#4---json-the-topics-the-text-and-the-runtimes-own-schemas)).

## 3. `matter`: what makes a matter new, and how a workflow becomes its proposal

`ephor guide matter` explains the other half, in ephor's own words, with no
runtime named:

- what makes a matter **new** and what makes it **held** ([§FS-019-waiting.1.2](FS-019-waiting.md#12-new-every-matter-nothing-has-been-done-about));
- what **processed** means: work the ledger records about the matter, or a
  recorded decision that it needs none, lifted when the matter moves
  ([§FS-019-waiting.7](FS-019-waiting.md#7-a-new-matter-is-processed-where-it-is-shown), [§FS-019-waiting.7.4](FS-019-waiting.md#74-x-records-that-a-matter-needs-no-rhei));
- how an entry becomes the **proposal** for a kind of matter: an action entry
  naming a workflow, or a recipe, whose selector in the site configuration
  accepts the matter, the first such entry in menu order, and the runtime's
  `triage` workflow where none does ([§FS-019-waiting.7.1](FS-019-waiting.md#71-every-new-row-carries-a-proposal)); and
- the commands that show and make each move: `ephor actions --item`,
  `ephor waiting`, `ephor work process --item … --dry-run` and
  `ephor work dismiss`.

## 4. `--json`: the topics, the text, and the runtime's own schemas

`ephor guide --json` prints the published `guide` view
([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)): the topics, and for a named topic its name and its text.
For `gate`, the view also carries the request and answer schemas, asked of
the bound runtime when the command runs. Ephor keeps no copy of either, for
the reason the roster is asked of the binding rather than kept
([§DA-004-roster-is-asked-not-configured](../decisions/architectural/DA-004-roster-is-asked-not-configured.md#da-004-roster-is-asked-not-configured-the-roster-is-asked-of-the-binding-never-kept-by-ephor)): the schema that holds is the one
the runtime installed here will validate against.

With no runtime bound, or one that prepares no gate briefs, there are no
schemas to print. The view then carries the text and the sentence saying why,
and the text form prints that sentence after the prose. Nothing is guessed in
their place, and the command still succeeds: the prose is still the answer to
the question asked.

## 5. A command, not a screen

The guide has no key. Nothing on a screen waits on it, and what a screen
needs to say about a gate it already says from the brief itself. A screen
that shows it later shows this reading, so the two cannot word the guide two
ways ([§REQ-002-parity.2](../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).
