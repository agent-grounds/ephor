# FS-005-dispatch: what ephor watches, it can hand to an agent runtime

A watch that only watches hands its reader a list. Nearly every row on that
list has an obvious next move — the gate is red so the failures need reading
and fixing, a reviewer asked a question so it needs answering, an issue was
filed so it needs doing — and every one of those moves is the same shape: read
a change in a checkout, do something small, say what was done. That is work an
agent can be asked to do, and asking it is the boring half of the day.

ephor does not do that work. It **dispatches** it: it turns an item into a
ticket in an agent runtime, hands over what it already knows, and then keeps
the ledger — which items have work under way, what that work reached, and
whether the item has moved since. Watching and working are one loop, ephor is
the half that remembers, and the routine moves leave the reader's hands
([§GOAL-004-handover](../goals.md#goal-004-handover-routine-moves-leave-the-persons-hands)).

The runtime is a binding with [rhei](https://github.com/agent-grounds/rhei) as the
shipped default ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy), decided with its tradeoff recorded in
[§DA-001-runtime-bound-default](../decisions/architectural/DA-001-runtime-bound-default.md#da-001-runtime-bound-default-the-runtime-is-a-bound-default-not-a-named-coupling)). What ephor writes is a plan file in a
documented plain-text language, and that language — together with the runner
command configured to execute it and the verdict read back from its results —
is the entire coupling: a contract in files, never a linked process. Choosing
a runtime remains a property of how a person works, which is why one ships
wired and ready; requiring it would be something else. Nothing in ephor's
core names the default runner, and with no runner installed every part of
dispatch except the running still holds — tickets are written, read, and
reopened, staying readable, diffable, and hand-editable on disk — while
running refuses with the configured runner named.

## 1. A recipe decides which items deserve work, and what to ask for

A **recipe** is a named piece of configuration with a selector and a brief: the
selector says which items it applies to — kind, role, whether the gate is red,
whether a response is owed and why, which source reported it — and the brief is what
the ticket asks for, in the reader's own words. Those words may be written
inline or kept in a file the recipe names, which is the same brief arriving by
another door ([§FS-005-dispatch.34](FS-005-dispatch.md#34-a-brief-may-be-kept-in-the-file-that-owns-it)).

Recipes are how the same watch serves different projects: what to do about a
red gate in one repository is not what to do about it in another, and neither
is ephor's to decide. A recipe is therefore configuration first. ephor ships
the few that are true everywhere — the red gate, the unanswered conversation,
the review, the issue, the branch that has fallen behind — for the same reason
it ships quick actions
([§FS-004-quick-actions](FS-004-quick-actions.md#fs-004-quick-actions-a-problem-ephor-recognizes-arrives-with-the-action-for-it)):
a problem ephor already recognizes should not need to be described to it
before anything can be done about it. Configuration adds recipes, and a
configured recipe that reuses a shipped one's name replaces it.

**Where a recipe may be written, and what happens when two of them share a
name.** Configuration writes recipes at three scopes: the site's own
`work.recipes`; `organizations.<org-id>.work.recipes`, over every project the
registry places in that organization; and that project's
`projects.<id>.work.recipes`. The middle scope is not there for symmetry with
the other two. It is the scope whose membership already decides which projects
share a work root
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project))
and what they may spend
([§FS-015-spend-ceiling.1](FS-015-spend-ceiling.md#1-two-ceilings-in-the-three-scopes-the-registry-already-nests)),
and a way of being worked is the third thing those same projects share — so it
is read by the same membership, the `organization` field on a project's
registry row and nothing else
([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)).
A project the registry places in no organization reads the site's recipes and
its own, and is told nothing about it: an absent membership is an omitted
tier, exactly as it is for the root and for the ceilings.

**Recipes accumulate outward in** — shipped, then the site's, then the
organization's, then the project's — and a recipe reusing an earlier one's id
replaces it *where it already stands* rather than moving to the end. The later
writer decides what the recipe says; the first writer decided where it sits in
the menu, and position is the order dispatch offers in
([§FS-005-dispatch.32.3](FS-005-dispatch.md#323-where-the-sweep-happens-and-what-it-is)). The rule is one rule at
every tier, not a rule about the tier that was added last: it is what a
configured recipe replacing a shipped one has always done, and saying it once
here is what keeps a third scope from being a fourth answer.

**This is not ephor deciding for a repository.** What this section says above —
that what to do about a red gate in one repository is not what to do about it
in another, and neither is ephor's to decide — is an argument for difference
*between* repositories, and it still holds. Every recipe at every one of the
three scopes is the person's own, written in their own configuration, about an
organization they themselves declared in their own registry; writing one recipe
over four repositories they have said are one organization is that person
deciding once instead of four times, and the project scope is there for the
repository that genuinely differs. What is forbidden is ephor choosing the
brief, not a reader choosing it at the width they meant.

A recipe may also carry `root`, the whole work-root template for work handed
over through that recipe. The same flat key may be written on an entry that
asks an agent or lays down a workflow, in each of the three entry homes: site
actions, project offers, and the entry beside a workflow. An agent entry
carries its `root` into the recipe it becomes. A command entry is refused for
carrying `root`, because it runs here and hands no work to a root. Omitting the
key preserves the placement the enclosing configuration would otherwise
choose.

The shipped `implement` recipe is the exception to the otherwise branch-neutral
defaults: it carries `"branch": "fix/issue-{number}"`. With no configured
replacement, an issue with no branch on a project that has
`branch_root_template` is dispatched inside the deterministically minted
`fix/issue-<number>` workspace; on a project without branch workspaces it is
refused by name with the configuration needed to proceed. An issue or pull
request that already has a forge branch, or a registry branch of its own —
never the project's configured main branch, which is the trunk every
workspace is grown from and not a matter's own
([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs))
— keeps that branch, and a configured recipe named `implement` replaces this
default with its own branch semantics.

**A recipe is an action.** The recipes and the quick actions are one menu, not
two lists behind two keys: *what can I do about this row* has one answer, and
which half of it the reader sees does not depend on which key they happened to
learn. A recipe stands among the entries a source, a project and the reader
wrote
([§FS-006-project-interface.9](FS-006-project-interface.md#9-offers-the-projects-actions)),
selected by the same language, ordered by the same provenance, and refused in
the same sentence — marked as work to hand over and saying who would get it
([§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project))
before the key is pressed, because that is the difference the reader is
choosing between: an entry that runs something here, and an entry that opens a
ticket asking somebody else.

It runs both ways. An entry may carry a **brief instead of a command** — the
same selector, the same brief, the same hand — which is how a project offers
agent work of its own without writing a separate list; such an entry is a
recipe under another name, and is dispatched as one. And an entry is offered
only where it would work
([§FS-004-quick-actions.2](FS-004-quick-actions.md#2-offered-only-where-it-would-work)):
work about a change is offered where the change is on the machine, and never
about an item that is finished
([§FS-005-dispatch.6](FS-005-dispatch.md#6-dispatch-is-offered-where-it-would-work-and-refuses-where-it-would-not)).
An issue with an unfinished first-class dependency is likewise offered no
recipe or workflow: the prerequisite ticket is the work to schedule, and
handing the dependent over creates concurrent work whose own forge says it
cannot yet proceed ([§FS-003-feed-categories.4](FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it)). This is a gate over every work
entry, not a selector authors must remember to repeat; non-work quick actions
remain available.

Where an entry already in the menu carries a recipe's name, that recipe is
what the entry hands over when it cannot finish, not a second thing to do
about the row: the key that replays a branch and the ticket about the conflict
it stops at are one operation under one name
([§FS-004-quick-actions.6](FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase),
[§FS-005-dispatch.12](FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)), and a
menu offering both would be asking the reader to tell two spellings of one
thing apart. Because they are one name, they are gated as one: what a recipe
applies to and what the entry that dispatches it is offered on cannot be
different sets, or the entry hands over work its own recipe says does not
apply here.

Handing work over from the menu is the same handing-over the work screen does
— one plan, one ticket, one ledger entry
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch),
[§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)) —
because where the reader pressed is not a fact about the work. And with no
runner bound the entries are still there: a ticket is written whether or not
anything can run it, and where the entry would say who gets it, it says
instead that nobody can be asked
([§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)).

**What a brief may name.** The words are the reader's, and the names in them
are the matter's: `{title}`, `{repo}`, `{number}`, `{branch}` and the rest of
what [§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)'s dossier holds, rendered where the ticket is written
so that nothing is a link the work has to follow. One of those names is open
rather than fixed — **`{meta.<key>}`**, whatever this matter's source said
about this matter ([§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)) — so a store that keeps its own division
of the work can have a brief say which division this ticket belongs to without
the reader writing one recipe per value. The same vocabulary renders a `root`
and an entry's `branch`, under the rules [§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs) already sets for a
name this matter has not got.

## 2. The ticket carries what ephor knows, not a link to it

A ticket that says "look at pull request 42" has handed back the whole job. The
watch already holds what the work needs: the title and state, the branch and
the checkout it lives in, the gate's counts per repository and the forge's own
reasons for refusing the merge, and the conversation as messages with their
authors and times. All of it was fetched already, and it is on disk.

So the ticket carries it — a **dossier** written into the plan, and the ask
written under it. Two things follow. The work starts from what a person would
have read first, instead of spending its opening move re-fetching what ephor
had. And the dossier is a record: it says what the item looked like when the
work was asked for, which is the only way to read the result of that work
later.

A dossier is bounded. A conversation of two hundred messages is not evidence,
it is a transcript; what is quoted is bounded per thread and in total, and
where anything was dropped the ticket says so and links to the whole.

**One field every matter can answer: `{id_slug}`.** Most of this vocabulary is
the forge's. A `{number}` and a `{repo}` belong to a matter a forge filed, and a
project's own task has neither
([§FS-003-feed-categories.1](FS-003-feed-categories.md#1-the-categories)) — so a
template naming one of them describes work about some matters and not others,
which is a distinction the templates are for. But every matter has an id, and so
every matter can answer `{id_slug}`: **that id reduced to a name a branch and a
path will take.** The value is the id lowercased over its ASCII alphanumerics,
with every other run of characters collapsed to a single `-` and trimmed at both
ends, followed by an eight-digit digest of the **whole** id:

| the matter's id | its `{id_slug}` |
|---|---|
| `rhei:window.1` | `rhei-window-1-d8a9c768` |
| `rhei:window.2` | `rhei-window-2-dba9cc21` |
| `rhei:window.retry-1` | `rhei-window-retry-1-17bbeb3b` |
| `rhei:window-retry.1` | `rhei-window-retry-1-5ff4987f` |
| `acmeforge:acme/widget#95` | `acmeforge-acme-widget-95-4ef7cc9e` |
| `github-issues:agent-grounds/ephor#120` | `github-issues-agent-grounds-ephor-120-bac79ee0` |

The third and fourth rows are what the digest is for, and why it is appended
**unconditionally** rather than only where the slug lost something: two
unrelated matters that read down to one slug stay two. An id of punctuation
alone leaves no readable half at all, and renders `item-<digest>` rather than a
name beginning with `-`. Nothing is truncated: the digest already carries the
uniqueness, so a cap would buy tidiness only, and how long a branch name may be
is a rule about branch names rather than about this field
([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)).

**The matter's plan file is named from this field, and not from a second
reduction of the same id.** A plan file's stem *is* the value above, held
*additionally* to the runtime's own grammar for a file stem, which refuses a stem
beginning with anything but an ASCII letter where neither git nor a filesystem
cares ([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)). So the two strings
are one string for every id whose readable half begins with a letter, and differ
in exactly one way where that guard fires — the stem carries `item-` in front of
what this field renders, and nothing else about it moves:

| the matter's id | its `{id_slug}` | its plan file's stem |
|---|---|---|
| `rhei:window.retry-1` | `rhei-window-retry-1-17bbeb3b` | *the same* |
| `rhei:window-retry.1` | `rhei-window-retry-1-5ff4987f` | *the same* |
| `2fa:acme/vault#3` | `2fa-acme-vault-3-b0ad6965` | `item-2fa-acme-vault-3-b0ad6965` |
| `:::` | `item-20bed5dd` | *the same* |

Two grammars, one reduction and one digest. The digest is on the stem for the
same reason it is on the field, and is the same digest: two unrelated matters
that read down to one slug stay two. A stem without it is a name more than one
matter answers to, and where the field's collision would cost a shared branch, a
stem's costs the record itself — the second matter's work is written into the
first's plan rather than merely misnamed
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)).

**Readable is not the same as best, and defined everywhere is not either.**
A matter that has a `{number}` renders both fields and neither wins: `fix/issue-95`
is a name a person reads and `acmeforge-acme-widget-95-4ef7cc9e` is one they
tolerate, so `{number}` stays the right choice wherever the matter has one.
`{id_slug}` is for the matter that has nothing else. A slugged *title* would read
better still and is deliberately not offered here: a name minted from free text
moves when the matter is retitled, and a rendering that moves stops resolving one
item's work to one place ([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)). What
such a retitle would cost is not merely a second workspace but a second
**plan** for one matter: the first is left on disk with whatever it held, the
ledger is repointed at the second ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)),
both roots go on being enumerated and both go on being returned as due, and the
guard against two runs colliding is keyed on the execution root — one per
checkout under the default root — rather than on the matter, so it holds
neither. The refusal and the rule it follows are written down at [§DF-002-path-fields-stable](../decisions/functional/DF-002-path-fields-stable.md#df-002-path-fields-stable-a-field-that-decides-a-path-holds-still-while-the-work-lasts)
rather than left as an omission.

**And one hazard belongs to the field rather than to any caller of it.** Because
`{id_slug}` is never absent, a recipe naming it serves *every* matter its
selector admits — which is the field doing what it was asked for, and also means
a too-wide `sources` or `kinds` list mints branches where a `{number}` template
would have been quietly withheld
([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)). One
case of that is not a wide selector at all: a recipe over a project's own tasks
([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live))
that mints a workspace gets a task store made inside it, dispatch writes its plan
into that store, and the next refresh reads that plan as a new task matter
matching the same recipe. The ledger keys work per item, so no task is dispatched
twice ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)),
but the matters a dispatch created are different items and nothing here bounds
them. Bounding it would change what the tasks seam yields, which is a contract of
its own: it is tracked at agent-grounds/ephor#125 and is not settled by this
point.

The same rule is why a brief kept in a file is read when the ticket is written
rather than named for the run to open
([§FS-005-dispatch.34](FS-005-dispatch.md#34-a-brief-may-be-kept-in-the-file-that-owns-it)): a path is a link,
and a link is the opening move handed back.

## 3. One rhei per item, one ticket per dispatch

An item's recipe work lives in at most one plan named after the item **per
committed root**. Dispatching a second recipe into a root already used by that
item appends a ticket to that root's plan; dispatching into a different root
creates the matter plan there. Ticket ids are unique across the matter's whole
dispatch history, not merely within one root, so two of its plans never name
different tickets alike.

**And the naming is injective.** A plan's name is a function of the matter's id
alone — the stem [§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)
renders — so two different matters never name one plan, however alike their ids
read. The rule above bounds a matter to one plan; this is the other direction of
the same fact, and it is the half that carries the weight. A plan file two
matters both resolve to is not a misnamed file: it is two matters' work in one
record, the second matter's ticket ordered behind the first's
([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)) and held there until work about
something else is finished — silently, since both dispatches did what they were
asked.

**And where two records name one plan file, dispatch refuses and names both
matters and the file.** That is not a hypothetical shape: it is what the
non-injective naming left behind wherever it fired, one file carrying two
matters' tickets and two ledger entries recorded at it. Giving the file to
whichever of them sorts first would leave the other's record naming a file that
is not there, which is the one outcome a rename must not produce
([§FS-005-dispatch.3.1](FS-005-dispatch.md#31-a-plan-named-before-the-digest-is-carried-over)) — so nothing about
those two matters moves, and which of the file's tickets belongs to which matter
is the reader's to say. It is the mirror of the refusal below: two files naming
one matter, and two matters naming one file, are both two records only a person
can separate.

**Where both names hold a plan about this matter, dispatch refuses and names both
files.** A root that has been carried over
([§FS-005-dispatch.3.1](FS-005-dispatch.md#31-a-plan-named-before-the-digest-is-carried-over)) holds one plan per
matter. A root an older ephor wrote into afterwards can hold two: one at the stem
[§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it) renders and one at
the name that ephor computed. Which of the two records of the same work to go on
with is the reader's call and not dispatch's, so dispatch stops, says both paths,
and writes no ticket. It is the one refusal this naming adds, and that mixed pair
is the only way to reach it.

The item first resolves the checkout and branch where its work runs — the same
resolution actions already use
([§FS-004-quick-actions.1](FS-004-quick-actions.md#1-a-quick-action-belongs-to-the-source-that-found-the-problem)) — and then [§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)
selects the root where its plan is recorded. Work about a branch runs in that
branch's working tree: it is where the change is, where the tools run, and
where the runtime will put the agent, even when the configured scope records
its plan in a wider root. Where the branch is not checked out and no branch
template can mint it, dispatch says so and offers the checkout, because a
ticket about code that is not on the machine only moves the problem.
Directory aliases do not select a different recorded checkout: placement is
matched by the directory the root names. When nothing recorded a placement,
the checkout is the directory holding the root, under the spelling by which
the caller supplied it; resolving identity does not rewrite that answer.

**Not all work has an item behind it.** A sweep that replays every idle
checkout onto its project's main branch
([§FS-004-quick-actions.6.1](FS-004-quick-actions.md#61-the-same-replay-over-every-checkout-nobody-is-holding))
is about checkouts rather than about matters, and a conflict it stops on names
no pull request, no issue, and nothing any source filed. That work lives in one
plan named after the sweep, in the project's own work root — where the scope
rule already puts it
([§FS-014-work-root-scopes.2](FS-014-work-root-scopes.md#2-reach-places-and-nothing-else-does):
a sweep over one project's branches sees that project's checkouts) — with one
ticket per conflicted checkout, appended to that same plan by every later sweep
rather than starting a rival copy of the same work somewhere else. It is the
rule above and not an exception to it: one plan per subject, and where no item
is the subject the sweep is.

**What ephor writes into a ticket is prose inside somebody else's document.**
A brief that carries a report — what a replay stopped at, what a gate said —
carries that report's own headings with it, and a heading inside a plan is a
node rather than a line of text: the plan language reads it as a task and
refuses the file. So the headings of an embedded report are flattened to plain
emphasis before it is written into a body, and what is already fenced is left
exactly as it is. The rule is about the writer and not about any one caller,
because the reader who finds out is the runtime, and it finds out by not
loading the plan at all — which on the one writer nobody is watching
([§FS-004-quick-actions.6.1](FS-004-quick-actions.md#61-the-same-replay-over-every-checkout-nobody-is-holding))
means a ticket that can never be worked and a checkout passed over on it
forever. A brief read out of a file is an embedded document like any other and
is flattened the same way ([§FS-005-dispatch.34](FS-005-dispatch.md#34-a-brief-may-be-kept-in-the-file-that-owns-it)). What counts as already fenced is
the plan language's own rule, and nothing looser
([§FS-005-dispatch.3.2](FS-005-dispatch.md#32-what-is-already-fenced-is-what-the-plan-language-fences)).

A project that keeps a single checkout for every branch is not exempt from
that. Its root is the branch's working tree only while it is standing on the
branch; a root standing on another one is a checkout of different code, and a
directory existing is not the same fact as the change being in it. Dispatch
refuses there too, naming the branch the root is actually on — and it offers no
checkout, because there is none to make: the remedies are to put the branch in
that root or to give the project branch workspaces of its own, and both are the
reader's to choose between.

### 3.1 A plan named before the digest is carried over

A plan whose stem was computed before the digest was part of it is **carried
over**, once: the plan file, the results and the artifacts keyed by that stem,
and the recorded plan id all move **together**. Together is the whole of it. Every
reading command answers from the record rather than from the disk
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), so a
record naming a file that is no longer there is the one outcome a rename must not
produce — and a file left behind at a name nothing names any more is the other,
because a sweep that finds plans by looking would run it
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).

**Together includes the bytes, not only the names.** A plan's own text carries its
stem: the result block the runtime writes when a ticket finishes names
`<stem>.<ticket>` and links the file it wrote under that name. So does the text of
every *other* plan in the root that waits on this one or reads what it produced —
an ordering or a consumed export naming `<stem>.<ticket>` resolves across the whole
project, and the plan it is written in is one the carry-over is not moving. Move
the files and leave those references and each of them is a record naming something
that is no longer there; the runtime then refuses the **root** it is asked to
validate rather than the one plan, so a matter that was carried over takes every
plan beside it down — and it does so at some later command, because the move
itself reported success. They are rewritten in the same commit as the renames, and
**only they are**: a stem also appears in prose, in a title and in what ephor
recorded about the item, and a plan whose bytes changed anywhere else would have
been edited rather than carried over.

**What moves is what the stem names, wherever it sits.** Under the work root's
runtime directory, every entry whose name begins with the stem and its separator
moves with the plan, whichever directory below it holds one — a result, an
artifact ephor's own states wrote, an export one ticket handed another — because
this rule cannot know every file a machine put there and finds them by the name
instead. The plan's writer sidecar moves too: it is named after the plan's path
rather than after the work, so it belongs to the path, and a sidecar left at the
old name is the second forbidden outcome in plain sight. What the same rule leaves
alone is what a past run wrote about itself — its transcripts and its spawn
records, named after the invocation rather than after the stem. That run happened
under the old name and nothing reads those by it, so renaming them would falsify
the only thing they say.

It is decided per entry and out of the entry itself: a recorded name that is not
the stem of its own id is carried over, and one that already is, is left alone.
Nothing is written down about whether it has run, because the question is answered
by looking — so it is idempotent, and a root some other binary or a hand edit has
already moved is not skipped for having been touched.

**It happens at the moment ephor is entitled to write in that root, and not
before.** The verbs that recompute a stem are the ones that write with it — the
dispatch, the workflow lay, and the sweep that starts runs — and those are
where the carry-over runs. Every reading command answers from the recorded name
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)) and is
therefore already consistent with the disk, so moving files beneath one would
buy nothing it has not got; what it would cost is the two promises this rule
must not break, a run held above one project writing nothing without `--act`
([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act))
and a dry run writing nothing at all ([§FS-005-dispatch.26](FS-005-dispatch.md#26-an-ordering-already-made-can-be-read-and-a-limit-bounds-what-runs)).
A run that only reports must still report the truth, so where a root has not
been carried over yet it reads the plan the record names and says the move it
is reporting across — the ticket it promises is the ticket the next real
dispatch writes.

**Each entry moves on its own, and what cannot move stops only itself.** The
plan, the results, the artifacts and the recorded name of one matter are one
commit: wherever an error escapes, what that entry had already moved is put
back and the record still says what it said, so the two never disagree. A root
that cannot be carried over — an unreachable directory, or either of the
refusals [§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch) states, or a name
the carry-over would have to write over — is reported and holds back the
matters it names and nothing else. It does stop a
dispatch *of those matters*, because a dispatch that went ahead would open a
second plan at the digested name and orphan the one already there; every other
project is carried over and handed its work as usual.

**A reference it cannot rewrite stops its own entry**, exactly as a file it cannot
move does: the commit unwinds, the record still says what it said, and what is
reported names the file it could not write. Refusing every carry-over that has a
reference in it would be worse than the damage it avoids — this is a one-time
migration, so a root refused for that reason stays refused, and the dispatch of
its matters stays stopped, which is the whole of what this rule exists to
prevent. So the rewrite is made, and only what genuinely cannot be written is
reported.

**And it never writes over a file already at the new name.** A carry-over is a
rename, and a rename onto an occupied name destroys what was there. Whatever is
already at the digested name is another record of this same matter's work —
which the same mixed pair of binaries that puts a plan at both names puts there,
a result or an artifact at a time — so the matter is held and both paths of each
pair are said, exactly as the refusals above do. It is what makes those refusals
safe to obey: a reader told to keep one of two plan files and remove the other
has not been told about the results and the artifacts beside them, and a rule
that lost those on the next write verb would be a refusal that walked its reader
into the loss it exists to prevent. What is held back that way is a record of
work, and the plan's writer sidecar is not one: it holds nothing, nothing outside
the runtime names it and nothing ever removes it, so one already at the digested
name where no plan stands is stale and the rename replaces it — holding a matter
back for an empty lock file, under a message asking which of two records of its
work to keep, would be a refusal nobody could act on. Where a plan *does* stand
at that name, its writer may be holding the sidecar beside it, and that one is
held back with the plan.

**A root a run is holding waits.** Moving a plan out from under a live run is the
one way this could lose work, and there is nothing to gain by hurrying it: the
root is carried over the next time a verb entitled to write in it runs and no
run is there.

**A workflow already laid keeps the name it was recorded under**
([§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)).
A laid plan's name is a record and not a lookup — nothing recomputes it — so
moving it would buy nothing and would break the one path its reader has.

What was carried over is said on the command's own output and carried in `--json`
([§REQ-002-parity](../requirements/REQ-002-parity.md#req-002-parity-every-ability-is-reachable-without-the-screen-and-every-answer-has-a-machine-form)), by the verb that carried
it over. It is a fact only ephor knows and only at that moment: the reader who
had a path to a plan does not have one now, and a rename nobody was told about
reads as work that vanished.

### 3.2 What is already fenced is what the plan language fences

**A plan must never be read by two fence rules, so a body is written by the
reader's.** The flattening exists so that the runtime never meets a heading where
the writer meant prose ([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)). A writer that puts a fence's
edges anywhere the reader does not either flattens a line the reader would have
left alone or leaves a heading standing where the reader finds a task. So the
fence ephor leaves alone is the one rhei's plan language defines for itself, as
agent-grounds/rhei#337 wrote it down:

- A fence **opens** on a line whose first non-whitespace character begins a run
  of three or more backticks, or of three or more tildes. An info string may
  follow the run.
- It **closes** only on a later line that is a run of the *same* character, *at
  least as long* as the run that opened it, followed by nothing but whitespace.
- **Nothing else closes it.** A shorter run, a run of the other character, and
  a run carrying an info string are lines of the block like any other.
- A fence that nothing closes **runs to the end** of the text.

**So a longer fence holds shorter ones.** A run of four backticks, or of tildes,
is how an author quotes a document that has fences of its own — a plan skeleton
with a `markdown` example inside it, a gate's log that printed the code it failed
on — and the inner pair is content of the outer block, not its end. Every line
from the run that opened a fence to the run that closed it, both included,
reaches the body unchanged, the headings of a nested example among them; only
what stands outside every fence is flattened. A brief whose whole purpose is to
show what a plan looks like must not arrive saying that a plan has no headings.

### 3.3 An embedded text ends where its place in the body ends

**A fence an embedded text leaves open is closed at the end of that text.** A
text ephor puts into a body — an instruction file a recipe named, a rebase
report quoting git — may open a fence and never close it. The reading rule of
[§FS-005-dispatch.3.2](FS-005-dispatch.md#32-what-is-already-fenced-is-what-the-plan-language-fences) does not change: inside that text, such a fence still runs to the
end of the text, and nothing after it in the text is flattened. But the end of
the text is not the end of the plan. So the writer adds one line at the end of
the text: a bare run of the character that opened the fence, at least as long
as the run that opened it. A text whose fences are all closed gets no line.

**This is [§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch) from the other side.** The flattening keeps a
heading ephor did not mean from standing as a task. Closing the fence keeps a
heading ephor did mean from being hidden. Without the close, whatever ephor
writes after the text is read as quoted content: the rendered `brief` after an
instruction file, the note after a rebase report, and the next `### Task` that
a reopen appends to the plan
([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)). The runtime
would then never see that ticket, and neither would any ticket appended after
it, while the verb that appended it reports it as handed over.

**The close is written after the hash.** The hash a ticket records is of the
instruction file's bytes as read
([§FS-005-dispatch.34.2](FS-005-dispatch.md#342-which-text-a-ticket-was-given-is-recorded-on-the-ticket)), and it stays that hash. The closing
line belongs to how ephor places the text in the body, not to the file.

## 4. The ledger is ephor's record, and never the truth about the work

ephor keeps a ledger of what it dispatched: the item, the recipe, the plan, and
what the item looked like at that moment. The ledger is what makes the second
question answerable — has this already been handed over? — and it is written
where ephor's other state lives, not in the reader's repositories.

Every dispatch also records the work root, checkout and branch it used. Those
per-dispatch facts are the durable index of every place the matter's work was
committed; a later dispatch cannot replace an earlier placement. A record from
before those fields existed falls back to the entry's item-level root, checkout
and branch. A root reached through another directory alias still belongs to
the recorded dispatch and its recipe; spelling does not sever that provenance.
Existing ledger fields remain readable and machine readings grow only by
additive placement fields.

**The ledger save is the commit point for a hand-off.** Before the first
work-root mutation, ephor journals the first pre-image — prior bytes or absence
— of every path it can change: the matter plan, root manifest, state machine and
ignore file, workflow output, and carried `.ephor` files, together with the
loaded ledger. The journal spans the whole unsaved batch and is cleared only
after the ledger's atomic rename succeeds. If saving fails, ephor restores
existing paths byte for byte, removes only paths that batch created, and
restores the loaded in-memory ledger. This does not undo a minted checkout or a
recipe's deterministic opening move, whose existing contracts remain in force.
If cleanup itself fails, the error retains the original save failure and names
the exact path cleanup could not restore.

But the work's state belongs to the runtime and is read from the plan, never
cached in the ledger. A ledger that remembers "running" when the plan says
"done" is worse than no ledger: it is a watch reporting on itself instead of on
the world, which is the one thing this tool must never do. A ledger entry whose
plan has been deleted is reported as missing rather than repaired.

## 5. An item that moved reopens its work

Work asked about a pull request is answered against the pull request as it was.
New comments arrive; the gate turns red again; the state changes. The ticket
that was finished is now finished about something that no longer exists.

So ephor **fingerprints** the item at dispatch — its last activity, its state,
its gate, how much conversation it had, and the open issue dependencies that
block it — and a change to any of those makes the work **stale**. Closing the
last prerequisite is therefore the same observable move as a gate turning
green: the next sync may hand the dependent over, without a label edit or a
person releasing it. Stale work is reopened by appending a ticket to the same plan
that says what changed since the last one and asks for the difference, ordered
after it — after the last ticket that was not cancelled
([§FS-005-dispatch.16](FS-005-dispatch.md#16-work-that-should-not-go-on-is-cancelled-and-the-plan-says-so))
**and that is about this matter**, since a cancelled prior is one nothing waits
out and a prior about another matter is one this work was never waiting for.
Which matter a ticket is about is read from the `id` the ticket itself records
([§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)), which is a fact
about the work and so is read from the plan rather than from the ledger
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)); a
ticket recording no `id` is no evidence of another matter and stays eligible. Not
by opening a second plan: the point of the record is that one
item's work reads in one place, in order.

What is asked for is chosen against the item as it now is, preferring what was
asked last while that still applies. A change moves between categories as it
goes: the pull request whose gate was red is, two hours later, one whose jobs
pass and whose reviewer has asked a question. Reopening it under the recipe it
was first dispatched with would hand the work a ticket about a problem that is
no longer there. Where nothing applies any more — it merged, it closed — the
work is not reopened at all, and the ledger goes on saying that the item moved
past it.

Reopening is a decision, not a reflex. It is offered where it applies and
performed when asked for — by a person or by whatever runs the sync — and never
as a side effect of merely looking at the feed.

## 6. Dispatch is offered where it would work, and refuses where it would not

The rules of [§FS-004-quick-actions.2](FS-004-quick-actions.md#2-offered-only-where-it-would-work)
hold here and cost more when broken, because a ticket that cannot run is not a
wasted keystroke but a piece of work that looks scheduled and never happens. A
recipe is offered only when it matches the item, the item's project has a root,
and the checkout can be resolved — and where the work edits the change rather
than reading it, only when that change is actually on the machine. Where the
runtime's setup in that checkout cannot run what ephor would write — a state
machine already there that does not declare the state a recipe starts in —
dispatch refuses and names both, rather than writing a ticket that will sit
there unrunnable. It refuses the mirror image too: where the reader's own plans
are already in that directory under no declared machine, ephor does not install
one, because a state machine governs every plan in a project and theirs were
there first.

Fresh plans ephor writes use the machine in the neighboring `states.yaml`;
they emit no `**States:**` declaration. A new work root receives the shipped
`ephor-work` machine, and an existing root keeps its machine unchanged. This
holds at the shared plan writer, including fresh creation through dispatch
and sync; it does not change machine validation or the refusals above. Plans
already carrying the old declaration remain readable without migration.

Matching is on what a gate is doing, not on how red it looks. Jobs that failed
are work for a checkout; a forge that refuses to merge an otherwise green
change is usually waiting on a person, and dispatching an agent at it spends a
pass to be told so.

Finished work is never dispatched. An item under Recent
([§FS-003-feed-categories.2](FS-003-feed-categories.md#2-recent)) is news, and asking an agent to fix a
merged pull request is asking it to invent something to do.

### 6.1 The work root is a template, and it may reach above the project

Where a plan is written is configuration rather than a constant. `root` is a
whole template, rendered from the vocabulary the ticket itself is rendered from
([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)) — the item's
own fields, the resolved checkout, and the project root. It is selected in this
order: the entry being used, the selected recipe, `projects.<id>.work.root`,
`organizations.<org-id>.work.root`, then the site's `work.root`. The first one
written answers and the others are not consulted; none merges with another,
because a path is one answer and a half-overridden one is nobody's. Ad-hoc
`work ask` has no entry or recipe override and continues through the three
configuration tiers.

**Narrowest-wins is the root's own reading and does not carry across the block
it is written in.** `organizations.<org-id>.work.recipes` sits beside
`organizations.<org-id>.work.root` and is read in the opposite direction: a
root is one answer, so the innermost scope that writes one ends the question,
while a recipe list is an ordered menu that accumulates outward in and is read
at every scope ([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)).
The ladder above is also why a *recipe's* own `root` is not a fourth tier of
the ladder: it is read at the recipe rung wherever the recipe was written, so
an organization recipe carrying `root` beats `projects.<id>.work.root` in the
same way a site recipe carrying one does today. That reads like an inversion of
the scopes and is not: it is the second rung of a ladder whose rungs were never
the scopes.

**One of the names answers for every matter.** `{id_slug}` is the matter's own
id as a name a path will take ([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)), so a root naming it gives each matter a work
root of its own — expressible for a matter with no `{number}` and no `{repo}`,
which is where a per-matter root was unreachable before.

**Two of the names reach above the project.** `{org}` is the organization the
project's registry row places it in and `{org_root}` is where that organization
is rooted — the registry has always known both, and it was the placement that
could not reach them. With them, an organization tier writing
`"root": "{org_root}/panta"` gives a whole organization one work root, which is
where work that belongs to no single repository goes: a release that moves
several projects' gates, a sweep across all of them. Membership and the root
are the registry's own facts, read here and never written back
([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)); only the tier that names them is
configuration's.

**A name with no answer refuses, and says which name and which
organization.** A project the registry places in no organization has no
`{org}` to render, and an organization that declares no `root` has no
`{org_root}`. Dispatch refuses both, by name — *organization acme declares no
root* — because what it would otherwise write is a directory literally called
`{org_root}`, or a path with a segment missing where the answer should have
been, and either one is work laid down somewhere nobody meant. The refusal is
about the *path*: the dossier and a recipe's brief are prose, and carry an
empty organization the way they carry any other field a matter has not got
([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)). A brief that
is *named by* a path is both at once, and is read as both: the path it names is
under this rule, and the text that path holds is prose under the other
([§FS-005-dispatch.34](FS-005-dispatch.md#34-a-brief-may-be-kept-in-the-file-that-owns-it)).

A name whose answer will not keep is refused in the same position for the
neighbouring reason — a root rendered from a field this matter can change while
its work is open is a path that moves under work that is still open, where prose
carries such a field the way it carries any other
([§DF-002-path-fields-stable](../decisions/functional/DF-002-path-fields-stable.md#df-002-path-fields-stable-a-field-that-decides-a-path-holds-still-while-the-work-lasts)).
Unlike the refusals above, which dispatch performs, that is a condition on a
name joining the half a path is rendered from rather than a check on the names
already in it, three of which do not meet it today and are named where the
rule is recorded
([§DF-002-path-fields-stable](../decisions/functional/DF-002-path-fields-stable.md#df-002-path-fields-stable-a-field-that-decides-a-path-holds-still-while-the-work-lasts)).

**A script's environment is neither, and is told the gap rather than refused
on it.** A summons carries the organization and its root as `EPHOR_ORG` and
`EPHOR_ORG_ROOT` ([§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)),
and carries them *present and empty* where there is no answer — a project the
registry places in no organization, or an organization that declares no `root`.
It cannot refuse the way a path does, because there is nothing to refuse: the
summons is about the matter, and a check verb that never mentions the
organization would stop running over a fact it does not use. And it may not
leave the name out the way prose does, because a summoned command inherits the
environment ephor itself was launched with — so a name ephor does not set is
not absent but whatever the shell that started ephor held, and an org-less
project's command would read some other organization's name as if it were its
own. The name is therefore always defined, and the check is the caller's:
`[ -n "$EPHOR_ORG" ]` before anything is built from it, never `${EPHOR_ORG:?}`,
which can no longer fire. What that check protects against is the same mistake
this rule refuses for a path — `mkdir -p "$EPHOR_ORG_ROOT/cache"` under an
empty answer makes `cache/` wherever the command happened to be standing — and
the reason it is the caller's to make is that ephor cannot see which of its
names a shell string is about to use as a path. The refusal for a path is
untouched: a work root naming `{org_root}` with no answer is still refused by
name.

Branch placement is resolved before the selected root is rendered. Thus
`{workspace}` names the existing or deterministically minted checkout and
`{root}` remains the registry project root. Offers and other previews, dry
runs, and real writes use the same selection and rendering; a dry run or a
refusal writes no checkout, root, ledger or workflow file. The existing named
placeholder refusals apply equally to recipe and entry templates.

Each agent offer carries its own `roster` in the machine reading, resolved at
that entry's branch and selected root, including an empty list when nobody can
be picked. The picker uses this same reading when it opens on the selected
entry; changing rows or dismissing it carries neither choices nor a pin to
another entry. The existing top-level `roster` remains a compatibility reading
and does not answer for an entry whose root differs from it.

**Which scope a plan belongs in is a different question from which tier may
answer it**, and [§FS-014-work-root-scopes](FS-014-work-root-scopes.md#fs-014-work-root-scopes-a-plan-lives-in-the-smallest-scope-that-can-see-everything-it-touches) is the rule for it.

## 7. Handing over work is the reader's move, and stays inside the machine

Dispatch writes files and nothing else. It opens no pull request, posts no
comment, and pushes no branch — those are the runtime's to do, if a recipe asks
for them, and a recipe that does asks in the ticket's own words where a reader
can see it. What ships asks for none of them: the shipped recipes end at a
local change, and closing the loop out to the forge is one line of
configuration that a person turns on deliberately.

Bulk dispatch — every matching item in a project, in one command — is the same
guarantee at scale: it writes tickets, reports each one, and can be asked what
it would do without doing it.

## 8. The ticket carries the item as data, not only as prose

In the shipped binding, newly inserted frontmatter follows the title and a
blank line. When reading or adding metadata to an older plan, it follows the
title and its legacy `**States:**` line instead. Both header forms support
reading and merging the same metadata block, preserving existing fields,
other tickets and the runtime's bookkeeping.

The dossier is written for a reader — a person or an agent — and a program
cannot read it. Yet the useful thing to put in front of an agent working a
failing gate is usually what a *script* can fetch: the log, the failing job,
the forge's analysis. A state machine can run that script before the agent, but
only if the script is told which item it is about, and prose is not an input.

So every ticket also carries the item's identifiers as **structured metadata**,
under the same names its context takes in a shell action
([§FS-004-quick-actions.1](FS-004-quick-actions.md#1-a-quick-action-belongs-to-the-source-that-found-the-problem)):
project and source, kind and item id, repository and number, branch and ticket,
url and state, the checkout the work belongs to, and the organization the
registry places that project in together with where the organization is rooted
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)).
One vocabulary, whether the thing reading it is a shell command in a menu or a
program in a state machine.

**The organization is a fact about the project rather than about the matter,
and is handed over all the same.** What a site keeps per organization — shared
instructions, a shared cache, a sibling checkout — is then reachable from the
matter a program was handed, instead of the site telling each project
separately which organization it is in and every copy of that answer going
stale on its own. It reaches the three places a brief cannot: a program in a
state machine, a project's own command, and a quick action, none of which has
prose to render `{org}` into. Both names are therefore in both halves of the
vocabulary, under one spelling — `org` and `org_root` on a ticket, `EPHOR_ORG`
and `EPHOR_ORG_ROOT` in a summons — because a vocabulary that spelled the same
fact two ways would be two vocabularies.

Each half says a missing answer its own way, and neither invents a third.
A summons defines both names always and leaves them empty where there is no
answer ([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)),
because an undefined name there is inherited rather than absent. A ticket
writes no key for a value it has not got, so a project the registry places in
no organization carries no `org` and an organization that declares no `root`
carries no `org_root` — exactly as a matter with no branch carries no `branch`.
Nothing reading a ticket has an environment to inherit from, so absence there
says what it means.

**That vocabulary is one set of names and two sets of members, and this point
used to read as though it were one of each.** What a template may name as
`{placeholder}` and what a ticket carries as metadata are chosen separately: the
list above is an **enumerated** one, and a name joins it deliberately rather than
by joining the placeholders. `{id_slug}` is the case that made the difference
visible — it is a placeholder every template may name
([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)) and it is not in
the list above, so a program in a state machine cannot read it as data and a
brief or a branch template can. `{org}` and `{org_root}` are in the same position
from the other direction. Nothing here says that is *right* — bringing the two
back together is tracked at agent-grounds/ephor#117 — but the promise this point
makes is the one it keeps: a name that is in both sets means the same thing in
both, and a name in only one says so here rather than being discovered by a
script that read nothing where it expected a value.

**And a name joins the half a path is rendered from only if its answer keeps.**
The promise just made is about meaning; this is the behaviour half of the same
distinction. A field this matter can change while its work is open may be named
in prose, where the dossier is a record of what the matter looked like when the
work was asked for
([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)), and is refused
where a path names it, because there the rendering is the resolution and a value
that moved repoints work that is still open
([§DF-002-path-fields-stable](../decisions/functional/DF-002-path-fields-stable.md#df-002-path-fields-stable-a-field-that-decides-a-path-holds-still-while-the-work-lasts),
[§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)). That
is a condition on joining the vocabulary rather than a claim about the names
already in it: three of them do not meet it today, and they are named where the
rule is recorded rather than left for a reader to find.

Two consequences. A ticket that is appended to a plan **adds** its metadata
rather than replacing what is there, because the runtime keeps its own
bookkeeping in the same place and a ticket writing over it would break the
plan. And what is written is identifiers only — the prose stays in the dossier,
which is where a reader is looking. One thing here identifies the ask rather
than the item: where the brief was read out of a file, the ticket also records
which file and which version of it
([§FS-005-dispatch.34.2](FS-005-dispatch.md#342-which-text-a-ticket-was-given-is-recorded-on-the-ticket)).

**Identifiers only admits one open map: what the source said about this
matter.** A source may know something about one matter that ephor has no field
for — which slice of a project's work a task belongs to, which customer or
environment it is about — and a store that keeps that in its own files has said
it once and should not have to say it again in every recipe. So a matter also
carries **`meta`**, a map whose keys the source chose rather than ephor, and it
is held to a bound that keeps it identifiers rather than prose: a value is a
scalar — a string, a number or a boolean — a key matches
`[A-Za-z_][A-Za-z0-9_-]*`, a rendered value **is on one line**, and a rendered
value is at most 1 KiB. The line is a condition of its own rather than a
consequence of the size: a value with a line break in it is prose however
short, and prose is neither a path segment, nor a variable a script can read a
line at a time, nor anything a selector compares — the dossier is where a
paragraph about a matter goes. What survives the bound is what a selector ([§FS-005-dispatch.31.1](FS-005-dispatch.md#311-and-it-can-ask-what-the-matters-own-source-said-about-it)), a brief or a template
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)) and a
summoned command's environment ([§FS-006-project-interface.3](FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)) may name.

**The bound holds of the map, wherever the map is read.** It is not one
reader's diligence. A source that reports these facts through its own free
passthrough reaches the same four surfaces as one ephor read out of a file, and
an unbounded value there would be a paragraph in a process environment, or a key
no shell can name listed among the ones this matter answers to. So the guarantee
sits on the accessor every surface reads the map through, and a source with no
channel to be told on is bounded **silently**. Reporting is the other half, and
it belongs to whoever has a channel: a reader that went and looked says what it
dropped, which is where the *once* below is.

What the bound governs is therefore what a matter **promotes**, not what the
source's answer held. That answer's own free passthrough still rides out as the
source gave it — it is passthrough, and printing it whole is the point of it
([§FS-006-project-interface.4](FS-006-project-interface.md#4-the-answer-envelope))
— so a reader of the passthrough may well see the map with keys the bound
refused still in it. Those are not the map: a key visible there that no selector
matches, no template renders and no variable carries is this bound at work
rather than a gap in it.

**A key that breaks the bound is dropped; the matter is not.** The offending
key goes, the rest of the map is carried, and the drop is reported once where
the source's own answer for that read is reported — naming the matter, the key
and which part of the bound it broke. The source has not failed to answer, so
its slot is not marked failed: a task vanishing out of the feed because
somebody wrote a paragraph about it is the worse of the two failures, and a
drop nobody is told about is how a selector silently stops matching.

**The map is read-only inward.** What is written into a ticket is still the
closed list of identifiers above; nothing a source said comes back out through
it. That matters because the runtime keeps its per-task bookkeeping in exactly
the namespace a store may be using to say these things — ephor lays its plans
inside the directory it reads — so the names ephor itself writes there are
**subtracted on the way in**, derived from the list this section already names
rather than kept as a second copy of it that can fall behind. Without the
subtraction the next read hands ephor's own words back as though the store had
said them.

**A subtracted name is declined, not dropped, and is reported to nobody.** The
two look alike and are opposites. A broken bound is a thing the store *said* and
has lost, which is why it is announced — a drop nobody is told about is how a
selector silently stops matching. A name ephor itself wrote there was never the
store's word at all, so nothing was lost and there is nobody to tell. Saying it
anyway would be the noisier mistake as well as the wronger one: ephor's own work
root is a store like any other, so every ticket it has laid would report every
name it wrote, on every refresh, and the drops that do mean something would be
buried under them. The bound's own drops stay reported, exactly as above.

## 9. Work that stops for a person says so where the person is looking

Work handed to a runtime is autonomous until it meets a question that is not
its to answer: a product decision, a trade-off between two things it cannot
weigh, an instruction it cannot read. The honest move then is to stop and ask —
and a machine that can only finish or fail will instead guess, because guessing
is the only move it has.

A runtime that can park work pending a person is therefore something ephor
reads, not something it invents: where the state a ticket sits in is one the
runtime will not leave on its own, that ticket is **waiting on the reader**, and
it is shown that way — ahead of anything else its work is doing, since it is
the one part of it nobody else will move.

The question and its answer stay in the plan. A ticket that asked something
carries the question in the artifact it wrote, and the answer belongs beside it
rather than in a chat window, a comment, or somebody's memory: the plan is the
record of what was decided about this item, and a decision taken anywhere else
is one the next round cannot read.

## 10. What ephor offers is not a limit on what can be asked

Recipes are for the work that repeats. Most work does not: a reader looks at a
change and knows the one thing they want done to it, and that thing has never
come up before and will not come up again. A tool where every ask must first be
written down as a rule, in a configuration file, in another window, has made
the common case the expensive one — and the reader will do it by hand instead,
which is what they were trying to stop doing.

So an item can be asked for **anything, in the reader's own words, where they
are standing**. What that produces is an ordinary ticket: the same dossier, the
same plan, the same place in the order, the same runtime. Only the brief is
different, in that nobody wrote it in advance.

Two things follow from its being asked for rather than offered:

1. **It is never refused for not matching.** Selectors say what ephor
   *volunteers*; they say nothing about what a person may ask for. Finished
   work, an item no recipe covers, a second ask on work already under way — each
   is somebody's deliberate request, and ephor's job is to write it down
   accurately rather than to have an opinion about it.
2. **The same holds for a command.** The action menu
   ([§FS-004-quick-actions](FS-004-quick-actions.md#fs-004-quick-actions-a-problem-ephor-recognizes-arrives-with-the-action-for-it))
   is configuration plus what a source offers; a reader who wants to run
   something once should not have to add it to a file first. A command typed
   into the menu runs exactly as a configured one does — the same checkout, the
   same `EPHOR_*` environment, the same handover of the terminal — because the
   only difference between the two is whether anyone expects to want it again.

## 11. A failure that is not the change's fault is restarted, not fixed

Not every red gate is a broken change. A runner dies, a mirror is unreachable,
a dependency ships a bad artifact, the same flake lands on the same job for the
third day running — and what the item needs then is not a fix but another run.
A loop that cannot tell the two apart pays for the difference twice: it spends
a model on diagnosing something that was never wrong, and then it lands a
commit whose only purpose was to make the gate start again.

So **restarting is a move the loop has**, and it is a different move from
fixing. Four things follow from its being different.

It is **decided by a program, not by prose**. Recognizing infrastructure is a
judgement, and a judgement nobody acts on is the same as no judgement at all:
what the model concluded has to reach a transition. So the verdict is a marker
on a line of its own, the way a question for a person is
([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)),
and a program reads that line and picks the state. An agent that is asked to
notice something, and given nowhere to put the answer, has been asked for
nothing.

It **opens a ticket of its own**. A restart is work done to the item, and the
plan is the record of that work — what was hit, when, and whether the round
after it came back green. A restart that happened as a side effect of some
other state would leave the next round unable to see that this failure has been
retried already, which is the one fact that separates a flake from a gate that
is simply broken.

It **restarts the gate and every gate downstream of it**. A gate that spans
several repositories fails downward: the repository whose job died takes the
rest of the tree with it, and the gates below never ran at all. Re-running only
what failed leaves every one of them exactly as red, so what is restarted is
the failing gate and everything under it that is not green. Nothing is
committed — the change was never the problem.

And it is **bounded**. An unhealthy runner pool answers a restart with the same
failure, and a loop that restarts every round is one that never stops and never
says why. Past a small number of restarts on one item the work stops for a
person, because at that point the infrastructure is the thing that is wrong and
no amount of retrying is going to be the fix.

## 12. Work an algorithm can finish does not start with a model

Not everything the watch turns up is a judgment call. Replaying a branch onto
its main branch is a fetch and a rebase: it either applies or it stops at a
conflict, and it does the same thing every time. Handing that to a model is
paying a pass to have two commands typed, slower and less predictably than the
commands would have run themselves — and the pass that matters is the one after
it, on the part no algorithm can do.

So the deterministic move runs first, and the work starts where it stopped.
Where it finished, nothing is dispatched at all: a clean rebase is a done
thing, not a ticket. Where it stopped, that is the ticket, and what is handed
over is the situation rather than the request to reproduce it — the repository
is left where the algorithm left it, mid-rebase with the conflict in the
working tree, because that is the state resolving it needs, and the ticket says
which repository, which files, and which two sides
([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)).

**Left where it stopped is a rule about a replay with a successor.** *The
state resolving it needs* names somebody who is coming, and what makes them
come is the ticket this dispatch is about to write. Where nobody is coming — a
sweep a timer ran over every idle checkout with no reader in front of it
([§FS-004-quick-actions.6.1](FS-004-quick-actions.md#61-the-same-replay-over-every-checkout-nobody-is-holding))
— the reason has no referent, and what is left behind is not a situation handed
over but a working tree somebody finds broken at nine in the morning with
nothing saying why. So the disposition belongs to the caller: a replay that
hands its stopping point to a successor leaves the conflict standing in the
tree, and a replay nobody is waiting on puts the tree back on the commit it
started from and reports the conflict instead. Where that report becomes a
ticket, the ticket says the tree was restored — otherwise it sends its reader
to look for a conflict that is not there, which is worse than saying nothing.

**The one-implementation clause is what makes that safe, and it is
strengthened rather than loosened.** The disposition is an argument to the
single replay, chosen by the caller at the point of the call, never a second
code path beside it: two dispositions in two implementations would disagree
about what a clean rebase is exactly as two rebases would. And under neither
value is a repository *already* stopped in a rebase touched — it is reported
and left alone, because aborting it would destroy a resolution somebody had
begun, which is the one thing worse than the drift any of this corrects.

A move that costs no model costs no screen either: the replay runs beneath
the interface as a job, and what the reader would have watched is in its log
([§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)).

The rebase is the first of these, not the shape of the only one. Any recipe
whose opening move is deterministic makes that move before it costs a model,
and dispatches what is left over — which is also why the move has to be
runnable on its own ([§FS-004-quick-actions.6](FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase)):
the same rebase the reader presses a key for is the one a state machine runs,
and two implementations of it would eventually disagree about what a clean
rebase is.

## 13. A communication is work too, and its answer comes back as a proposal

Not every matter's next move is a change in a checkout; often it is a reply.
A matter owing a response — a question in a review thread, a mail asking for
a decision, a mention carrying a request
([§FS-003-feed-categories.4](FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it)) — is dispatched like any other: the ticket
carries the discussions as its dossier, and asks the runtime for a
**proposed answer**, drafted in the matter's context. The shipped answer
recipe is this shape, and an ask in the reader's own words ([§FS-005-dispatch.10](FS-005-dispatch.md#10-what-ephor-offers-is-not-a-limit-on-what-can-be-asked)) may request
one for anything.

Three things distinguish it. **It needs no checkout**: the work is about the
conversation, so the plan is written where the matter resolves — the branch
workspace where one exists, because sitting in the change makes a better
answer, and the project root otherwise — and the checkout-able rung is not
required ([§FS-006-project-interface.10](FS-006-project-interface.md#10-capability-rung-by-rung)). **The proposal is a file, never a
post**: [§FS-005-dispatch.7](FS-005-dispatch.md#7-handing-over-work-is-the-readers-move-and-stays-inside-the-machine) holds — the runtime writes the proposed reply into the plan's
results, ephor reads it back and surfaces it beside the discussion it
answers, and nothing reaches the channel by itself. **Posting is one
deliberate move, where the channel can carry it**: on a channel that
declares reply ([§FS-007-matters.4](FS-007-matters.md#4-a-channel-says-what-it-can-do)) the surfaced proposal is offered for
posting, edited or as it stands, exactly as a reaction is posted today; on
a channel that does not, the proposal is what the person copies — a stated
degrade ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)), not a failure.

### 13.1 An issue nobody holds is owed work, not an answer

A matter can wait on the reader for more than one reason, and only one of them
is a conversation. An issue nobody has taken awaits somebody where its source
asks for that, and [§FS-003-feed-categories.4](FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it) is plain that this way of waiting
"is not a conversation at all": nobody asked anything, and the talk may never
have started. What such an issue is owed is the work it describes, which is
the shipped issue recipe's, not a reply.

So **the shipped answer recipe takes a matter only where its conversation
awaits the reader.** An issue whose only reason for waiting is that nobody
holds it is not offered `answer`, and a plain dispatch lays the issue recipe
on it instead. An issue nobody holds whose conversation *also* awaits the
reader — somebody asked a question and nobody has taken the issue — waits for
both reasons, and is offered `answer` as before; so is an issue somebody
holds whose conversation awaits the reader.

Three things do not move. **The feed is unchanged**: an unclaimed issue still
awaits the reader, bolded in the feed and counted in the glance, and
`needs_response` reads exactly as it did
([§FS-003-feed-categories.4](FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it)). **The `needs_response` selector key is
unchanged**: a configured recipe that asks it is still asking whether the
matter waits at all, for whatever reason. **Nothing is lost for a matter whose
source recorded no reason**: a matter that waits and does not say why waits on
its conversation, which is the only reason there was before this one was told
apart.

**The dossier says which reason it is.** Its `waiting on` row
([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)) reads "an answer from me" only where the conversation
awaits the reader. Where the only reason is that nobody holds the issue it
says so — "somebody to take it (nobody holds it)" — and where both hold it
names both. A brief that tells an agent somebody is waiting on a reply when
nobody asked anything spends a run finding that out.

The shipped recipe expresses this with a selector key of its own, `awaits`
([§FS-005-dispatch.31.2](FS-005-dispatch.md#312-and-it-can-ask-why-the-matter-waits)), rather than with a rule hidden in ephor, so that a
configured recipe replacing `answer` ([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)) can say the same
thing.

## 14. Who does the work is chosen, and defaulted per project

Work is handed to an agent carrying a model at an effort, and two of those
three follow from the first: which models an agent can carry and which
efforts it declares are facts about the agent, not free choices beside it.
A chooser built as `agent × model × effort` would be mostly cells nobody can
run, and ephor has no way to know which — the runtime does. So the choice has
one axis the reader picks and one dependent on it, and the set of choices is
the **roster**: the runtime binding's own enumeration of who can be asked,
read from the binding at the moment of asking rather than kept as a list in
ephor's configuration, because a copy is wrong the first time an agent or a
model is added on the other side
([§DA-004-roster-is-asked-not-configured](../decisions/architectural/DA-004-roster-is-asked-not-configured.md#da-004-roster-is-asked-not-configured-the-roster-is-asked-of-the-binding-never-kept-by-ephor)). Every id on the roster is unique —
the runtime's model and agent namespaces are separate, and a model profile
may claim an agent's very name, in which case the profile holds it and the
agent stands alone under a marked spelling of its own — because one name
over two rows can address only one of them.

One entry is a **hand**: a name, the agent it summons and the model that
agent will carry — shown together, because a reader choosing the name is
choosing both — the **efforts** the entry declares, and whether it is
available. An entry may declare no efforts at all, which is an answer rather
than a gap: such a hand is simply asked plainly, in either spelling, because
it has no effort an ask could drop. A hand that does declare efforts is
always asked at one of them. A choice that names none is **completed** where
the hand declares exactly one — a single declared effort is a fact about the
hand, not a choice left open, and the completion is said in a note — and
**refused** where it declares several, with the efforts listed, before
anything is written: the runtime's two spellings do not agree on what an
effort-less ask would mean — one drops the effort silently, the other lets
the state machine's own choice fall in and fails outright where the hand
does not declare it — and neither answer is the reader's choice.
Configuration names a hand by its id and nothing else; the binding's own
spelling of agent, mode, provider and model is the binding's, so a
configuration written under one runtime reads unchanged under another
([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)).

An unavailable hand is **shown with its reason, never hidden**. This is the
opposite of what a menu does ([§FS-004-quick-actions.2](FS-004-quick-actions.md#2-offered-only-where-it-would-work)), and deliberately so:
a menu entry is an offer, and an offer that cannot work costs a keystroke —
but the roster is the answer to "who could I ask", and a hand that silently
vanished because its agent left `PATH` looks exactly like a hand that never
existed, which is the one confusion a reader debugging a dispatch cannot
resolve from the screen. The reason is computed where the roster is read —
the agent's command is looked for, never spawned to fail — and it is one
sentence beside the entry, the same sentence everywhere that hand appears.
The roster is reportable before anything depends on it: `ephor doctor` and
`ephor capabilities` print each hand, what it resolves to, and why an
unavailable one is unavailable ([§FS-010-doctor.2](FS-010-doctor.md#2-the-ladder-is-answerable-on-its-own)).
When that roster contains only agent-default hands, `capabilities` also says
that no model-carrying hands are configured and points to model profiles in
the shipped binding's `models` settings registry as the way to create
nameable model-carrying hands. A choice naming an id absent from the roster is
still refused before anything is written, with both the requested id and the
current roster retained in the refusal; the refusal additionally says that a
model profile bearing that id and an agent carrier in the same registry makes
it a nameable model-carrying hand. A missing name is never passed through as
an ordinary string.

**The hand for a piece of work resolves in seven steps**, each displacing
the ones after it: what the reader picked for this dispatch alone; the pin
the action or recipe itself carries; the project's hand for this action id;
the project's default for everything; the site's hand for this action id;
the site's default; and, where nobody chose at all, whatever the binding
would pick unasked. The order mirrors the
binding's own resolution deliberately, so ephor's answer and the runtime's
cannot come to disagree about what one configuration means. A project may
also narrow the roster — say which hands may be used on it at all, which is
what a repository under a policy about which models may see its code needs —
and a hand outside the narrowing is refused with that reason, never silently
dropped.

**A pin may name an ordered list of hands, and the steps still answer with
one.** Everywhere a hand is named — a table's entry, the pin an action or a
recipe carries, what the reader picked — the value may be an ordered list of
names instead of a single one, and a single name *is* that list with one
member in it. Nothing written before this means anything different, because a
list of one resolves and dispatches exactly as the bare name did. The seven
steps themselves are untouched: they displace one another in the same order
and still answer exactly once, and what the answering step hands on is the
list it carried rather than a name. Choosing among that list is a later stage
with a different question in front of it ([§FS-005-dispatch.29](FS-005-dispatch.md#29-headroom-is-reported-to-ephor-and-vetoes-a-member-it-never-reorders)), and the two are kept apart on
purpose — a step answers *whose work this is*, which is the author's judgment
about the matter, and the stage below answers *which of them can be reached
right now*, which is a fact about the world and no judgment at all. So a step
that answered has answered: no later step is consulted because a member of its
list turned out to be unreachable, and the fallback lives inside the list the
author wrote rather than across the precedence order, where it would silently
promote a table nobody meant to reach.

The order inside a list is the author's, and it ranks by **fitness rather than
by equality**: the first name is first because it is the right hand for this
work, and each name after it is what to do when the one before it cannot be
had. That is the whole reason the stage below may veto a member and may never
reorder the survivors ([§FS-005-dispatch.29](FS-005-dispatch.md#29-headroom-is-reported-to-ephor-and-vetoes-a-member-it-never-reorders)) — a rule that sorted this list by anything else
would be overruling the only judgment in it.

The first step is **made at the moment of dispatch and spent by it**. In the
interface it is a picker over the menu's entry: the roster's hands in one
column and, beside a hand that declares efforts, those efforts in a second —
absent where it declares none, which is every hand on a machine whose
runtime settings declare no model profiles, and a dead column would teach an
axis that is not there. On the command line it is `--hand
<hand-id>[:<effort>]` on the command that dispatches, the same grammar the
tables write ([§FS-006-project-interface.9](FS-006-project-interface.md#9-offers-the-projects-actions)), so the key and the flag are one
operation ([§FS-005-dispatch.12](FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)). The pick lives exactly as long as the one
dispatch it was made for: nothing records it, and the next dispatch of the
same action resolves from the second step down — a pick that outlived its
dispatch would be a configuration layer nothing wrote down. The picker never
assembles a choice the resolution would refuse — a hand that declares
efforts is picked at one of them — and it shows an unavailable hand with its
reason without letting it be chosen. A hand the project's narrowing excludes
does not appear in it at all: the narrowing is the project's policy rather
than a state of the hand, and what is refused loudly is a *named* choice —
offering the name only to refuse it would teach the policy one wasted
keystroke at a time. With an empty roster there is no picker, and the entry
dispatches exactly as if nothing had been picked.

**A chosen hand binds in one of two spellings, never both.** A hand that
carries a model is written onto its ticket, at dispatch, in the runtime's
own per-ticket execution line ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)) — each ticket carrying
its own choice, so two tickets in one plan can go to two hands and the
choice survives every later run. A hand that names an agent and no model of
its own — which is every hand on a machine whose runtime settings declare
no model profiles — has no line in the plan language, so it rides the run
instead: the run invocation carries the choice as the runtime's own per-run
agent flags, the agent and the effort where one was settled, resolved again
at the moment the run is invoked — the same moment the runtime reads its
own configuration, so the two answers cannot drift apart. The two per-ticket
lines rank differently against a run's flags, and ephor follows the
runtime's own ranking exactly. A ticket carrying the full execution line
cannot be re-aimed: the runtime resolves such a ticket from its line alone,
and the run's agent flags are invisible to it — only a per-run model choice
reaches past that line. A ticket carrying a model alone can be: the run's
agent flags supply its carrier, and one run advances several tickets,
including one a person pinned by hand. So the flags ride a run only where
they can re-aim nothing — every ticket the run would advance and that has
no line of its own resolves to the same spelling, and none pins a bare
model. Where one plan's open tickets do not agree, that plan runs with no
flags and the reader is told the hand went unbound for that run; plans that
agree differently are run separately, each under its own spelling. A ticket
somebody has claimed is not the run's to advance at all — a claim makes the
runtime skip it ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)) — and enters none of this. The
cheaper spelling is always available to the reader: a
model profile declared in the runtime's own settings turns an agent-only
hand into a model hand, and the ticket line then carries it everywhere,
with no flags involved at all.

**A run started from the interface is the same run.** The key that hands
one item's plan to the runtime resolves the hand exactly as the command
line does and carries the same flags — which surface a reader started a
run from is not a fact about who did the work, and two resolutions of it
would eventually disagree ([§FS-005-dispatch.12](FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)). Such a run names one
plan and advances no other, so that plan's own open tickets settle its
flags and no other plan's can contradict them. What the resolution has to
say — a hand nothing resolves, an effort completed, a hand left unbound —
is said where the reader can still read it when the run returns: a surface
that cedes the terminal keeps the note for its own message rather than
printing it into a screen the run is about to take, and a refusal is
answered before the terminal is ceded at all, never after
([§FS-004-quick-actions.2](FS-004-quick-actions.md#2-offered-only-where-it-would-work)).

With no runtime bound, or a bound one not on `PATH`, the roster is empty and
says so in the *workable* rung's own words ([§FS-006-project-interface.10](FS-006-project-interface.md#10-capability-rung-by-rung)):
who can be asked is the runtime's knowledge, and where nobody can run work
there is nobody to ask. A runtime settings file that exists and does not
parse empties the roster too, in a sentence of its own that names the file:
a roster read around it would be a list missing whatever the person just
added, which is worse than no list. Nothing else changes — every other rung
resolves, and tickets are still written and read on disk
([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)).

**The work root's overlay is found where the toolchain keeps its own files**
([§FS-006-project-interface.12](FS-006-project-interface.md#12-what-the-toolchain-keeps-in-a-checkout-has-a-home-and-a-deprecated-one)): under `.agent-grounds/` first, then the
deprecated `.agents/` name. A work root carrying only the deprecated name
produces exactly the roster the same file produces from the home, and reading it
is said in a note beside the work rather than as a refusal — every hand the
roster had it keeps, and no command exits differently for it. That is the whole
difference between a deprecation and the unparsable file above: one takes the
hands away because it cannot know them, the other knows them perfectly well and
only found them somewhere else.

## 15. Every operation is visible in one place

The watch can say what is being done about any one item — the lines its work
stands on beneath its row
([§FS-005-dispatch.23](FS-005-dispatch.md#23-work-stands-on-rows-of-its-own-beneath-the-row-it-is-about)), the
work screen behind `w` — but "what is ephor doing right now" should
not require visiting every row that might hold a piece of the answer. So
there is an **operations board**: one screen, reachable from anywhere in the
interface, holding every operation beneath the reading — each live run, each
claim somebody holds, each ticket waiting on a person, and the refresh
itself, which already reports in the header of the screen being read and
appears here *additionally* ([§FS-001-forge-interface.7](FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)). It is where
[§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking) pays off at the scale of the whole watch: work that
stopped for a person is one glance away wherever the person happens to be
looking — and within one operation, what asks something of the reader is
listed ahead of anything else its work is doing — a parked question first,
then what a dead run left holding — then what runs, then claims, then the
queue.

**The rows are found by looking, never by remembering.** What ephor
dispatched is in the ledger, but "every operation" is a claim about the
world, not about the ledger: the work roots themselves are enumerated —
the place a project's work is configured to live, resolved at the
project's own checkout and again in each branch workspace on disk, since
the work root is per branch workspace and each one is its own execution
root, and again at its organization's root where the template reaches
above the project
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project))
— and every plan found in one is watched, whoever wrote it. A plan
written by hand, a project's own planning tickets, and a run somebody
started in another terminal on a root ephor never dispatched into are
operations exactly as dispatched work is, judged row-worthy by the same
artifacts; the ledger still says which matter a plan is about
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), it just no longer decides what exists. And an
operation ephor never dispatched has no matter behind it by construction —
that is the common case for a foreign plan, not an edge — so `Enter`,
which goes to the matter where the feed still carries one, opens the plan
itself there: the same reading `e` offers wherever work is shown, and no
row on the board leads nowhere. Enumerating is a reading of the plan files
on disk and asks nothing of the bound runner: with no runner installed the
plans are still found and still readable — it is only operations that
cannot exist then.

**Liveness is read from the runtime's artifacts, never from a process
table.** The same reasoning that keeps work state out of the ledger
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)) applies to whether work is running at all: the runtime
leaves the truth on disk. It holds a lock on an execution root for exactly
as long as a run is live there, and the operating system lets go of that
lock when a run dies, however it dies. The board probes the lock without
ever waiting on it — the runtime acquires it blockingly, so a probe that
queued would park the watch behind the very run it is asking about. Which
tickets a live run holds is read from what that run itself writes as it
works ([§FS-005-dispatch.15.2](FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream)),
and where a runner writes no such stream, from the journal and the logs it
leaves behind.

**A row is an execution root, not a ticket.** The runtime schedules one run
per root, and ephor's work root is per branch workspace — so two items whose
work lives in one workspace are one operation, and a ticket written into a
root a run already holds is shown as **queued**, never as running: a second
run there would wait for the first. And a ticket is a ticket at whatever
depth the runtime nests it — a subtask parked three headings deep is as much
an operation as the ticket it was split from, run or no run.

**A claim is not a run.** An assignee on a ticket is written when somebody
takes it, and its effect is that the runtime skips it: it says *claimed and
unschedulable*, never *live*. A ticket with an assignee on a root where no
run holds the lock is its own flavour of row — **claimed, not scheduled** —
shown with the bound runner's own command for releasing the claim. The board
reports it; it does not act on it. And under a live run a claim stays a
claim: the run skips it, so *queued* would promise a turn that never comes.

**Parked work outlives the run that parked it.** The usual end of a run
that parks a ticket is the run exiting: nothing else was schedulable, so
the lock goes free — and the runtime wrote no claim on the way out, since
parking is a transition, not a taking. The ticket is waiting on the reader
all the same ([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)), and it keeps its row — **waiting**,
ahead of anything else its operation is doing — whether the run that parked
it is still live, exited, or died. A root holding such a ticket is an
operation with nothing running in it, exactly as a root holding a claim is.

**Silence is a badge, not a verdict.** A long tool call is legitimately
quiet, and the lock — not the last write — is the liveness signal. A live
run that has written nothing for a while carries a **quiet** badge and
nothing more; a run that died has released its lock, and its root simply
stops being a live row — though not always a row: a ticket a dead run was
still holding mid-slot is read out of the journal that run left behind and
keeps a row of its own, **dropped by a run that died** — beside a parked
ticket, deliberately not as one: nobody else will move either, but a parked
ticket asks a question about the work, and a dropped one asks for the run
back. The artifacts tell the two apart without guessing — parked is the
machine's gating word on the ticket's own state, dropped is the journal's
unreleased slot under a lock nobody holds. The journal
outlives every run, so what it says is held is believed only while the
world still agrees: an assignment no run ever released stops counting the
moment the ticket's own state says it moved on, and is never read as
running under a run that came later.

**Watching only, and deliberately so.** The board starts nothing, stops
nothing, and intervenes in nothing — and it stays that way now that something
*can* be started beneath the screen
([§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)): what starts a
job is the menu entry the reader pressed, and the board is only where it is
then seen. Interfering with a live run remains out — it needs a channel to
the run that exists only while a run serves one, and a board that hinted at
it would be promising what it cannot do.

**With no runtime bound, the board holds the refresh row and ephor's own
jobs** — and that is the board being right, not broken ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)):
the operations a *runtime* has are runs, and where nothing can run there are
none, said in the workable rung's own words ([§FS-006-project-interface.10](FS-006-project-interface.md#10-capability-rung-by-rung)). A
job needs no runtime to exist
([§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)) — it is ephor
running a command, which is the one thing that never depended on a binding —
so it keeps its row while the runtime's half of the board says why it is
empty. The tickets themselves stay readable
exactly as everywhere else in dispatch: work state is read from the plan
files on disk, that reading is the floor and is never removed, and where the
bound runner itself can be asked for a sharper listing, its answer may
refine what the files said — it never replaces them. And a plan whose state
machine cannot be read is reported at the same altitude: liveness, running,
claims, and what a dead run dropped are facts the lock, the journal, and the
plans carry on their own, and the board still says them — but nothing in
that plan is called queued or waiting and nothing of it counted finished,
because those are the machine's words and the machine is not there to say
them. The row itself says the machine could not be read, **naming the plans
it happened to**, in so many words: a count left silently at zero would read
as nothing done, which is exactly the guess the withholding exists to avoid.
Which machine that was is the plan's own question
([§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can)), so a
plan judged by a machine of its own is never among them — a row that said
otherwise would deny a count the work beside it really earned.

### 15.1 The board keeps itself current

Nothing here is something the reader has to ask for twice: work that moves
on disk — a ticket advanced, a question parked, a verdict written, a run
starting or dying — surfaces on its own within moments, not at the next
refresh. This is not the refresh ([§FS-001-forge-interface.7](FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)) wearing a new
name: a refresh asks the world's forges and costs what they cost, while this
watches files ephor already knows by name and asks nothing of any forge
([§GOAL-005-costless](../goals.md#goal-005-costless-watching-costs-the-watched-nothing)). It is cheap by construction — nothing is re-read while
nothing has changed, a timestamp answers that, and the timestamps asked
are a fixed handful per root: each plan file the last enumeration found,
the root's own directory — a plan appearing or vanishing is a directory
event — and the artifacts the runtime moves as it works: the one it writes
on every slot it takes or releases, and the live run's own stream
([§FS-005-dispatch.15.2](FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream)), which
is one more name in the same fixed handful and never a sweep of everything
the runtime ever wrote; the bound runner is
asked to list its plans only about a root that holds an operation, and
nothing is ever read while a frame is being put on screen — and it holds
everywhere work is shown, not only on the board: a ticket the runtime
parks resurfaces on the reader's rows when it parks ([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)),
instead of waiting for a refresh that was never going to be about it.

Finding the roots ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)) is the one walk in the design, and
it is not the tick's: the work roots are enumerated when the rows are
built — the board opened over the reading, rebuilt because the glance saw
something move, or a refresh landing — and never merely because time
passed. The walk is bounded by where work is configured to live, not by
what the disk holds: it visits the project checkouts and the branch
workspaces ephor already resolves, to the fixed depth a branch name can
nest, and — where the template names an organization
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project))
— the root of the organization the project belongs to, which lies inside
no checkout and would otherwise be written to and never looked at again.
It costs one directory listing per candidate work root — it never
descends into a repository, and it never reads a plan to find it. What
has no answer here is skipped rather than refused, the way a template
naming a field only an item can fill is: an organization placeholder
nothing answers is a template no dispatch could have written through
either, so there is nothing under it to have missed.

Every root retained on a committed dispatch is another bounded seed. One
normalized recorded-plan reading combines those seeds with configured
candidates and, after a fresh process load, feeds enumeration, list and status,
due classification, named and plain runs, repeat detection, cancellation, and
proposal lookup. It resolves each recorded plan with that dispatch's root,
checkout and branch; no item-level "latest root" may move an earlier plan. The
reading still performs no filesystem search for an unknown root, preserves one
live run per checkout and wrong-branch refusals per recorded placement, and
keeps explicit `work forget` as the act that removes the ledger seed while
leaving files on disk.

### 15.2 What a run is doing is read from the run's own stream

The lock says a run is live and the plan says what state a ticket is in.
Between those two facts sits everything the reader actually wants from a
live row — which ticket the run has in hand right now, what it just
finished, whether it is still moving — and it was reconstructed from a
journal that outlives every run. That journal is the wrong witness for a
question about *this* run: it is append-only across all of them, so a run
that died mid-slot leaves an assignment nobody ever released, and every
later reading has to argue that entry down from evidence elsewhere — the
ticket's own state, the age of a log against the birth of the lock. The
answer is right and the reasoning is a chain of inferences about a file
that was never about one run.

So where the runtime keeps a **record of the run itself** — what that run
took up and let go, in order, from its own beginning — that record is what
a live row is read from. Its properties are the ones the inference chain
was standing in for: it belongs to one run, so nothing in it can be another
run's leavings; it is ordered and numbered, so a reader can say exactly how
much of it it has seen; it says how each slot ended rather than leaving the
end to be deduced; and it says when the run ended, so a reader knows the
difference between a run that finished and a run that stopped existing.

**The journal stays the floor.** A runner that writes no such record is
read exactly as before, with the same inferences and the same caution
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)): this sharpens the reading where the artifact is
there and is never a thing a runtime has to provide to be bound at all
([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)). Where both exist the run's own record answers,
because a witness to one run beats a witness to all of them.

**Liveness is still the lock.** A record that ends saying the run finished
agrees with a lock that is free; a record that simply stops does not mean a
run is gone, only that it has written nothing lately, which is the quiet
badge's business and not a verdict ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). Nothing here is
consulted to decide *whether* a run is live — only what the live one is
doing, and what the last one did.

**And what the last one did is read by the sweep, not only beneath a row.**
*How each slot ended* and *when the run ended* are two of the four properties
above, and they are the two a reader of a **finished** run wants: whether that
run moved anything. The autorun sweep asks exactly that before starting another
run on the same root
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)), so this record answers a
question about a run nobody is watching as well as one about the run in front of
somebody. The reading is the same reading either way — one run's own word about
one run — which is why the sweep gets no witness of its own.

**And it is read where work is shown, not only on the board.** The reader
looking at a matter's rows sees the same word the board would show, because
this is one reading narrowed to one matter rather than a second one of its
own ([§FS-005-dispatch.23](FS-005-dispatch.md#23-work-stands-on-rows-of-its-own-beneath-the-row-it-is-about)).

## 16. Work that should not go on is cancelled, and the plan says so

Not every ask should run to its end. The same recipe was pressed twice; a
ticket was asked for on the wrong item; the item moved past the question
before anyone picked it up. What is wanted then is not a fix and not a
reopen but a **cancel** — and a watch that can hand work over and cannot
take it back leaves the reader with two runs of the same fix in one
checkout, or with an editor open on the plan, guessing at what the runtime
would have written.

So a ticket can be cancelled: from the work screen, on the ticket, and from
the command line. Cancelling moves the ticket into the machine's
**abandonment state** — spelled `cancelled` in the plan language, the one
final state that satisfies no `**Prior:**` — carrying the reader's reason
as the ticket's result. Two things follow from its being a state.

**It is the runtime's move, made in the runtime's own words.** The plan
language reserves a ticket's state, after it is written, to the runtime's
own verbs — the compare-and-swap, the artifact checks, the callbacks, the
audit trail — and a state line ephor rewrote by hand would be a plan the
runtime can no longer vouch for
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work),
[§DA-005-cancel-is-the-runtimes-move](../decisions/architectural/DA-005-cancel-is-the-runtimes-move.md#da-005-cancel-is-the-runtimes-move-cancelling-a-ticket-asks-the-runtime-and-never-rewrites-the-state-line)). So ephor asks the runtime for the
transition, captured rather than watched, and what comes back is what the
reader is told: the ticket cancelled, or the runtime's own refusal in its
own sentence. It follows that with no runner bound cancelling is refused in
the workable rung's words like running is
([§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)): the
plan is still readable and hand-editable, and nobody is there to make the
move.

**The record keeps it.** A cancelled ticket stays in the plan, in its place
in the order, marked as cancelled with its reason beneath it — the same
reading a finished ticket gets. Nothing is deleted: the plan is the record
of what was decided about this item
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)), and taking an ask
back is a decision too.

Cancelling refuses where the move would be wrong, and says why in one
sentence before anything is asked of the runtime. A ticket a **live run
holds** is that run's to finish: pulling the state out from under an agent
is interfering with a run, which is the later section
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)), and the refusal names
the run. A **finished** ticket has nothing left to cancel. A machine that
declares **no abandonment state** is refused with the machine and its file
named, exactly as a recipe naming a state the machine lacks is
([§FS-005-dispatch.6](FS-005-dispatch.md#6-dispatch-is-offered-where-it-would-work-and-refuses-where-it-would-not)):
a ticket moved into a state the machine does not have leaves a plan the
runtime refuses to run at all. The machine ephor ships and the examples
beside it declare the state and a transition into it from anywhere, so a
work root ephor made can cancel from the start; a machine of the reader's
own says whether it can, and the refusal tells them what to add. Everything
else is fair: a ticket that is queued, parked on a question, dropped by a
run that died, or claimed and unscheduled is somebody's to cancel, and the
runtime's own checks are the last word.

Order follows from the state's name. A ticket **ordered after** one now
cancelled will not start — the abandonment state satisfies no `**Prior:**`,
which is what its spelling is for
([§FS-005-dispatch.11](FS-005-dispatch.md#11-a-failure-that-is-not-the-changes-fault-is-restarted-not-fixed))
— so cancelling says which open tickets those are, and cancelling them too
is one more keystroke; ephor does not decide for them. And a ticket ephor
appends afterwards — a reopen, a second ask — is ordered after the last
ticket that is **not** cancelled
([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)), so ephor's own chain never
hangs off abandoned work.

## 17. A move that needs nobody runs beneath the screen

Pressing a key for a deterministic move costs the whole interface for as long
as the move takes. Replaying a forest onto its main branch is a fetch and a
rebase per repository, and on a checkout weeks behind that is minutes of
output — output that asks nothing, decides nothing, and is read afterwards if
it is read at all. Meanwhile the watch is gone: the reader cannot look at the
next item, cannot start the second move, cannot even see that the first one is
still going, because the screen that would say so has been given away. Then it
asks for a keypress to hand the screen back. The reader paid the interface for
a command that never needed it
([§FS-005-dispatch.12](FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)).

So a menu entry that does not need the reader runs as a **job**: started
beneath the screen, with the interface staying exactly where it was, and the
job taking a row of its own among the operations
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). What was a takeover
becomes a line saying the work started and a row saying it is going.

**Which entries these are, the entry says.** ephor's own deterministic moves
are jobs by construction — the rebase is the whole argument of
[§FS-005-dispatch.12](FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model), and a
move that costs no model has no reader to cost either. Everything a person or
a project wrote keeps the terminal unless it asks otherwise, because a menu
entry has always been allowed to *be* the reader's session — `lazygit`, an
editor, a pager ([§FS-006-project-interface.9](FS-006-project-interface.md#9-offers-the-projects-actions)) — and starting one of those
beneath the screen would leave a program nobody can type into, waiting on a
terminal it does not have. Which way an entry runs is therefore the entry's
own word, and its default is the one that was always safe.

**A job outlives the interface that started it.** It is its own process in its
own process group, so quitting ephor does not take it down, and neither does
closing the terminal: a move that needed nobody watching does not suddenly
need somebody staying. This is the same property from the other side — work
handed to the runtime already survives the screen ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), and a
move ephor runs itself should not be the fragile one.

**Everything the reader would have watched is kept.** The job's output —
what it wrote and what it complained about, whole and in order — goes to a
log, and the log is the inspection: from the job's row on the board, from the
work screen of the item it was started about, and from the command line. A
job that is going says what it is doing right now, because "still running" and
"stuck" are the same three words on a row, and the log is the difference.

**Liveness is the lock, exactly as it is for a run.** A job holds one for as
long as it runs, and the operating system releases it when the job dies,
however it dies ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)); the
board probes it and never waits on it. Nothing consults a process table, and
nothing believes a record that says a job started: a job ephor started and
then crashed alongside is not running, and the lock says so without being
asked.

**The rows are found by looking, here too.** Jobs are read from where they are
written, not from anything that remembers having written them, so a job
started from another ephor — a second terminal, an earlier session that has
since exited — is a row like any other, and a record with no job under it is
history rather than a claim.

**The chain travels with the job.** An entry that needs the item's branch
workspace still gets the checkout first ([§FS-004-quick-actions.7](FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout)), inside the
job and against the same verification: the directory is checked for rather
than trusted ([§FS-006-project-interface.8](FS-006-project-interface.md#8-the-checkout-contract)), and a checkout that did not make
it ends the job there with what it said. A job is a sequence because the move
was a sequence; it is not a way to run two unrelated things.

**The outcome comes back to the row it was about.** A job ending is news
exactly as a ticket parking is ([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)), and news read at the top
of the screen is news about nothing in particular: a header line saying a
replay went through names no branch, so a reader with three of them going has
to guess which row just moved. So the line lands **under the subject the job
ran on** — the branch it replayed ([§FS-004-quick-actions.6](FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase)), or the matter it
was started about — beside the facts it changed, and it stays there until the
reader opens that row or a later job about the same subject replaces it. Where
that row is not on the screen at all, the project's own row carries it: news
with nowhere to land is news that is lost.

**Only what has ended lands there.** A job still going is already marked
running where it could be started again
([§FS-005-dispatch.21](FS-005-dispatch.md#21-what-is-already-going-is-shown-where-it-could-be-started-again))
and holds a row among the operations
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)), and a third live mark
on the tree would be one fact said three times. A job that has ended is no
longer an operation and leaves the board — an inbox that accumulated every
finished thing would be the pile this section exists to avoid — while its
record stays with the item it was about, log and all, until it is old enough
to be swept.

**What the move hands over, it still hands over.** A rebase that stops in a
conflict dispatches its ticket exactly as it did when the reader was watching
([§FS-005-dispatch.12](FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)), and
that ticket is a run with a row of its own. The job ends; the work does not.

## 18. The work screen says when, and folds away what is over

What was asked for and what ephor ran are both lists of things that already
happened, and a list of things that happened without times answers half of
what the reader brought to it. "Did I already press this?", "is that the job I
started a minute ago, or the one from yesterday?" — those are questions about
time, and a screen that will not answer them sends the reader to the ledger
and the job directory to read timestamps that were on disk the whole while
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work),
[§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)).

So **every row that already happened says when**, in the age the watch
already spells on its rows rather than a clock time the reader has to
subtract from: a screen read at a glance is read in "12m ago", not in
"14:22". A ticket says when it was **asked for** — the ledger's record of the
dispatch, not anything the plan holds, since the plan is the runtime's and
tracks what the work reached rather than when it was handed over
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)). A
job says when it **ran**, and how long it took; a job still going says how
long it has been going, which is the same question asked of a thing that has
not finished, and the only answer there is.

**An age nothing can source is left off.** A ticket somebody wrote into a
plan by hand was never dispatched, so nothing knows when it was asked for,
and that row simply carries no age. The plan file's own modification time
would be ephor inventing a fact about work it did not start, which is the
same refusal the board makes about rows it did not write
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).

**What is over folds away.** Tickets accumulate, and are meant to: a reopen,
a second ask, a cancel, months of work about one long-lived change, all of it
kept ([§FS-005-dispatch.16](FS-005-dispatch.md#16-work-that-should-not-go-on-is-cancelled-and-the-plan-says-so)).
But a reader opening this screen is looking at what is still going, and the
finished and the cancelled push it down the screen a row at a time until it is
off it. So the screen leads with the tickets that are still open and collects
the ones that are over behind a single line saying how many there are and how
to see them; one key unfolds them in place, in their order in the plan, ages
and all. Nothing is hidden that a keystroke does not show, and nothing is
deleted: this is a reading of the record, and the record is unchanged
([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)).

**A plan with nothing open is not folded to nothing.** Where every ticket is
over, they are all shown: folding the whole list away would leave a heading
above an empty space and a reader wondering where the work went. The fold is
for what is over *beside* what is not.

## 19. A workflow the runtime offers is an action, and its inputs are answered here

A binding brings more than a place to put tickets. It carries **workflows** —
named, parameterized plans that lay down tasks of their own, under a machine of
their own, with fan-out and gates ephor never wrote — and a reader who has one
is in exactly the position
[§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)
describes: the thing worth doing about this row is already written down, and
doing it means leaving, remembering a vocabulary, and coming back. So a
workflow is an entry in the same menu, selected by the same language, ordered
by the same provenance, refused in the same sentence.

Above the binding a workflow is an id, a description, and its **inputs**: each
one a name, a type, whether it is required, and what it stands at when nobody
says. Everything else about it — where workflows live, what a plan rendered
from one looks like, how it is rendered at all — is the binding's, and is
spelled there ([§REQ-001-boundary.5](../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)). Which workflows there are, and what each
one takes, is the binding's own knowledge too: it is asked of the binding at
the moment of asking rather than kept as a list of ephor's, for the reason the
roster is ([§DA-004-roster-is-asked-not-configured](../decisions/architectural/DA-004-roster-is-asked-not-configured.md#da-004-roster-is-asked-not-configured-the-roster-is-asked-of-the-binding-never-kept-by-ephor)) — a copy is wrong the first
time a workflow is added on the other side. So with no runtime bound there are
no workflows, in the *workable* rung's own words
([§FS-006-project-interface.10](FS-006-project-interface.md#10-capability-rung-by-rung)), and nothing else
changes: the plans a workflow already laid down go on being found by looking
and read from disk, like every other plan there
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).

**Asking what the runtime offers is an operation, not a best-effort probe.** A
bound runtime's workflow-listing summons that cannot start or cannot reserve
its answer file, or that finishes with a non-zero exit code, fails the
enumeration wherever it was asked. Ephor reports that the workflow-listing
invocation failed, preserving useful refusal words from the runtime, and
neither caches nor consumes that failure as an empty offer set. A completed
invocation that lists no workflows remains an authoritative empty answer,
just as no bound runtime remains the *workable* rung's refusal; a completed
populated listing remains the binding's answer. This distinction is about
whether the summons ran and completed successfully: it does not redefine an
exit-zero listing whose output cannot be read as workflows
([§FS-006-project-interface.3](FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)).

**An entry names a workflow, and that is what makes it an action.** The entry
is the one the menu already has
([§FS-006-project-interface.9](FS-006-project-interface.md#9-offers-the-projects-actions)) — an id, an
icon, a description, a `when`, its capability requirements — and where it would
carry a command or a brief it carries a workflow's name and the answers to its
inputs instead. It is written in any of three places: beside the workflow
itself, where the binding keeps one somewhere a reader can put a file — there
it travels and versions with the workflow; in the project's manifest; in the
person's own configuration. Narrow beats broad, as everywhere else. A
workflow the binding ships is ranked with what ephor ships, one the person
keeps with the person's own, one the project keeps with the project's offers —
the provenance the menu already orders by, with nothing new to learn, and the
same rule settling two entries that share an id.

**A workflow no entry names is still asked for.** Requiring configuration
before a workflow can be used once is the cost
[§FS-005-dispatch.10](FS-005-dispatch.md#10-what-ephor-offers-is-not-a-limit-on-what-can-be-asked) exists to
refuse — but a menu carrying every workflow the machine can find, on every row,
is [§FS-004-quick-actions.2](FS-004-quick-actions.md#2-offered-only-where-it-would-work) broken at
scale: most of them have nothing to do with the item, and a reader who has to
read past twenty of them to reach the two that do has lost the menu. So the
named ones stand in the menu where they apply, and every other workflow is
behind one entry that opens the list of them, where it is picked, answered, and
run once. What was answered there can be kept as an entry, which is how a
workflow becomes an action without anybody opening a configuration file first.

**The inputs are answered in six steps**, each displacing the ones after it:
what the reader answered explicitly for this instantiation alone; what the
reader supplied in the values files for this instantiation; what the entry
says; what ephor answers for an input about who does the work; the workflow's
own default; and, where an input is required and still unanswered, the reader
— asked, or refused by name where nobody is there to ask. Explicit `--set`
answers therefore displace values-file answers, and both displace the entry,
hand, and workflow defaults. The order is
[§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)'s, deliberately,
so that one resolution order covers everything a dispatch has to settle.

The command-line laying surface accepts repeatable `--values <file>` options.
Each named file is a YAML or JSON mapping, read relative to the directory from
which the command was invoked, and mappings are merged left to right: a later
file replaces an earlier value for the same input. Arrays, records, numbers,
and flags remain their structured types; they are not flattened into words.
Values supplied this way are reader answers and appear as such in the human
and JSON answer accounts, distinct from an explicit `--set` answer. A
non-empty value for an input naming who does the work still resolves as a hand
id through ephor's roster, narrowing, and permitted-hand checks; a values file
never provides a way around that policy.

What the entry says is data, not prose: a string is rendered with the item's
fields where it names them, exactly as a brief is
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)), and
anything that is not a string is passed on as it stands, because an input that
wants a number, a flag, or a list of them is not served by a sentence. Where a
list or a record holds strings, those are rendered too — the fields an item
carries are as useful inside a structure as beside one.

**What ephor knows reaches a workflow as files, not only as words.** The
dossier and the identifiers are already written for exactly this
([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it),
[§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)), and a workflow
has no place in its plan for either — so an entry may answer an input with the
dossier or with the item's identifiers, and what the workflow is given is a
path to a file ephor has already written, or that file's contents where the
input wants the text itself. It is written before the workflow is, so an input
that insists its file exists is answered truthfully, and it is written where a
work root's own reading will not mistake it for a plan.

**Every input is answered on one screen, with its answer already standing in
it.** Answering a workflow is not filling in the blanks. The answers that were
resolved are as likely to be the wrong ones as the missing ones — the
workflow's own defaults most of all, which are a stranger's habits about which
model reviews and how many passes it takes — so what the reader is given is
every input the workflow declares, each carrying the answer the five steps
reached and the name of the step it came from, and each one changeable there.
A screen that showed only the holes would be asking the reader to accept
everything else unseen, which is the opposite of what showing an account is
for.

**What has a known set is chosen from it; the rest is typed.** An input is
edited in the shape it actually has. Where the values it can take are known,
one is picked from them rather than spelled: a flag has two, an input that
names who does the work has the roster, already narrowed and already saying
who is unavailable and why
([§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project),
[§DA-006-hands-fill-a-workflows-targets](../decisions/architectural/DA-006-hands-fill-a-workflows-targets.md#da-006-hands-fill-a-workflows-targets-who-a-workflows-agents-are-is-ephors-answer-not-the-workflows)),
and an input whose own check is a plain set of words has those words. An input
wanting several of them is answered several at a time, from the same set.
Everything else is one line typed on its own row, which is
[§FS-005-dispatch.10](FS-005-dispatch.md#10-what-ephor-offers-is-not-a-limit-on-what-can-be-asked)'s ask with
the input's name already on it. What ephor reads out of a check is a
convenience and never a second authority: a value the offered set does not
hold can still be typed, and the binding remains the one thing that validates
its own inputs. What no row can carry — a record, or a list of them — is the
reader's editor, on that input alone or over the whole set at once, which is
the same handover an offer already takes.

**What is still unanswered blocks the laying and says so, or refuses by name.**
A required input nobody has answered leaves the screen open, naming what is
missing, rather than laying a workflow down with a hole in it. Where nobody is
there to ask — a dispatch of every matching item at once — the entry refuses
and names the inputs it could not answer, because a workflow written with a
hole in it is a piece of work that looks scheduled and never happens
([§FS-005-dispatch.6](FS-005-dispatch.md#6-dispatch-is-offered-where-it-would-work-and-refuses-where-it-would-not)).

**Who does the work is ephor's answer, not the workflow's.** A workflow's
inputs are mostly its agents: which one reviews, which one adjudicates, which
one writes — each defaulted to a model its author happened to have. Left alone,
those defaults are a hole in everything
[§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project) and
[§FS-006-project-interface.9](FS-006-project-interface.md#9-offers-the-projects-actions) settle: the
project's table stops applying, the reader's pick stops applying, and a project
that narrowed which hands may see its code is narrowed right up to the moment a
workflow is instantiated, which is the moment it mattered. So an input that
names who does the work resolves through the seven steps like any other hand,
and is rendered into the binding's words in the one place that renders them
([§DA-006-hands-fill-a-workflows-targets](../decisions/architectural/DA-006-hands-fill-a-workflows-targets.md#da-006-hands-fill-a-workflows-targets-who-a-workflows-agents-are-is-ephors-answer-not-the-workflows)); an
input wanting several is answered with several. A hand a narrowing does not
permit is refused with that reason wherever it was named — the workflow's own
default included, since a default is a naming — and never quietly replaced.
An empty answer, including an empty element in an input wanting several,
resolves to nobody: it passes to the workflow as written, no hand is rendered
into it, and no narrowing binds it because nothing was chosen. The workflow's
own fallback can therefore stand. This is the input-side spelling of the
roster's `Choice::Unasked`, which keeps nobody choosing apart from a refusal.
Each of those resolutions carries the pool its work would be bought against,
so **which pools a laying needs is a fact about the laying** — computed from
the answers rather than declared anywhere, recorded with the laying, and read
back from that record by whatever decides later
([§FS-005-dispatch.33](FS-005-dispatch.md#33-work-that-needs-several-pools-at-once-is-admitted-whole)).

**What it writes is a plan beside the item's, not a ticket inside it.** A
workflow lays down a plan of its own; it cannot be appended to the item's
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)), and pretending otherwise
would mean rewriting somebody else's workflow into ephor's shape and losing
what made it worth having. It is written into the item's own work root all the
same, and everything that follows from that is the point: the operations board
finds it by looking, like every other plan there
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)); it shares the root's one
run, so a workflow and a ticket about the same change queue rather than edit
the same tree at once; and the ledger records the dispatch against the plan it
made, which is the fact that was missing — the record says the item, the entry,
the plan, and what the item looked like
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), and
gains a plan of its own beside a ticket of its own, which is an addition and
costs nothing to what is already written
([§FS-006-project-interface.11](FS-006-project-interface.md#11-the-interface-is-versioned)). An item that
moved is offered the workflow again rather than a ticket appended to it
([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)), ordered after what came before
and named apart from it, because two runs of one workflow about one item are
two records and not a correction of the first.

**Instantiating writes files; running is the move after it.** Everything
[§FS-005-dispatch.7](FS-005-dispatch.md#7-handing-over-work-is-the-readers-move-and-stays-inside-the-machine)
guarantees holds here and holds for the same reason: what a key press does is
write a plan, and what runs it is the reader, from the board where every other
operation is run. A workflow lays down more than a ticket does — a directory,
a machine, sometimes settings — so what is about to be written is shown before
it is, in the binding's own account of it beside ephor's own account of every
input it answered and where the answer came from. A workflow that fails to be
written leaves nothing behind, so the board is never given half of one to
report on, and the refusal read back is the binding's own.

**Repeating the same workflow action does not instantiate it again.** Before
resolving a new destination or writing carried files, the shared action move
reads the ledger for the newest workflow dispatch with the same matter id and
selected entry id. Where that dispatch records the matter's unchanged current
snapshot and its exact recorded plan still resolves to a laid plan on disk,
the move refuses with the entry, that plan, the complete
`ephor work run --item <id>` starter, and
`ephor work lay <entry> --item <id>` as the deliberate way to lay another.
Entry id is the identity here: two entries naming the same runtime workflow
remain two actions. The refusal writes no carried files, plan directory, or
ledger dispatch and requests no runtime start; explicit input answers, carried
values, confirmation, and dry-run do not bypass it.

Only positive evidence refuses. No record, a changed snapshot, a missing plan,
or a record that cannot resolve to a laid plan leaves the existing laying or
lower-level refusal in charge and neither deletes nor repairs the record. A
recorded workflow that has finished still supplies positive evidence while
its matter remains actionable: named running owns the truthful answer about
what remains to advance, and direct `work lay` owns an intentional repeat.
Finished matters themselves remain ineligible for workflow offers. First-time
actions, another entry id, changed matters, command and recipe actions, sweep
suppression, and named-run safety and ceiling policy retain their existing
behavior.

**A values-file refusal leaves no partial laying.** A missing, unreadable,
malformed, or non-mapping values file is reported before a plan or workspace
is written. The same is true when the runtime rejects the effective values:
ephor validates them before creating the destination and reports the runtime's
refusal without leaving a partial workflow workspace. With no `--values`, the
existing laying behavior is unchanged, including a dry run's guarantee that
nothing is written.

**What comes back is what the plan says, and no more.** A ticket ephor wrote
carries the shape a verdict and a proposed answer are read out of
([§FS-005-dispatch.13](FS-005-dispatch.md#13-a-communication-is-work-too-and-its-answer-comes-back-as-a-proposal));
a workflow answers to its author, not to ephor, and reading a verdict out of
one would be ephor inventing a fact about work it did not shape. So workflow
work is read at the altitude every plan is read at — its states, what is
waiting, what is finished — and nothing is attached to the matter that the
matter did not get. In the same spirit, where a workflow brings settings of its
own into a work root, those settings are the root's from then on and govern
every plan in it, ephor's tickets included: what changed is reported when it is
written, because a workflow quietly re-answering how another workflow's agents
run is exactly the kind of fact a watch exists to say out loud.

## 20. A run of the runtime starts beneath the screen, and is watched by attaching

Pressing the key that runs the runtime hands it the whole interface for as
long as the run takes — and a run takes as long as the work does. Everything
[§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen) says about a
replay holds here with more force: the work was handed over precisely so that
nobody had to stay
([§FS-005-dispatch.7](FS-005-dispatch.md#7-handing-over-work-is-the-readers-move-and-stays-inside-the-machine)),
ephor is the half that remembers, and a screen given away to one run cannot
watch the other items, cannot start the next move, and cannot even say that
the first one is still going. A run that the reader *wants* to watch is the
exception that was made the rule.

So **a run starts detached**: where the binding can, the runtime is started in
a session of its own, outliving the screen and the terminal that started it,
and what the reader gets is one line saying the run began and what it is
called. The root turns live on the board
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)) from the lock, as every
run does — nothing new is watched, because nothing about "is a run live here"
changed. A move that needs nobody does not suddenly need somebody staying,
and a run is the longest such move ephor makes.

**Detachment outlives the service that starts it.** Once the binding has said
that a run began, normal completion of the shipped periodic service that
started it does not end it, and neither does explicitly stopping or restarting
that service. Those operations end or replace the finite sweep, not the
runtime's run. Watching remains attaching to the run, and stopping remains the
binding's own command shown for that run; service stop is never a second way to
stop work already detached.

**A run has an identity, and it is the binding's.** A live run names itself —
an id, and while it serves one, the address of its control — and both are
read from the artifacts the binding leaves beside its lock, never from
anything ephor remembers having started: a run somebody started in another
terminal, on a root ephor never dispatched into, has the same identity and is
reached the same way
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). An id is how the reader
and the runtime agree on which run they mean, so the board says it on the
row, the work screen says it on the operation, and the command line prints it
with the rest ([§FS-011-command-line.8](FS-011-command-line.md#8-what-is-going-is-said-and-the-way-in-is-printed)).

**Watching is attaching.** The binding's own surface for a run it did not
start — a reader of the run's files and a client of its control — is opened
on the run, and leaving that surface detaches and never stops the run: the
reflex that ends a foreground command must not end a run another screen may
also be watching. The surface is something the reader types into, so by
[§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)'s own rule it
takes the reader's terminal — or a window of the reader's own, where one is
bound ([§FS-005-dispatch.22](FS-005-dispatch.md#22-a-window-of-the-readers-own-where-one-is-bound)). What the
surface can do — answer a question the run parked, release a gate, intervene
— is the binding's, unchanged, and ephor adds nothing to it and takes nothing
from it.

**Stopping stays out of the screen.** The board starts nothing and stops
nothing ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)), and a detached
run does not change that: where a run can be stopped, the row carries the
runner's own command for stopping it, in the runner's own words, exactly as a
claim carries the command that releases it
([§FS-005-dispatch.10](FS-005-dispatch.md#10-what-ephor-offers-is-not-a-limit-on-what-can-be-asked)). A key that
stopped a run would be a channel to the run ephor promised never to hold.

**A question a detached run asks still reaches the reader.** A run with
nobody at its terminal waits at a human gate rather than exiting — that is
the binding's own contract, and it is the right one, because the person who
releases the gate is expected to arrive later. ephor reads the wait exactly
as it reads a parked ticket
([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)):
the ticket says *waiting on you* wherever the reader is looking, and the way
to answer is to attach.

**Where the binding cannot detach, the run is watched as it was.** A runner
with no detached shape, or a platform that has none, runs attached — the
terminal handed over, the reader watching — and the one line says so rather
than pretending. With no runtime bound there is no run to start, in the
workable rung's own words ([§FS-006-project-interface.10](FS-006-project-interface.md#10-capability-rung-by-rung)).

## 21. What is already going is shown where it could be started again

The menu says what can be done about a row; the board says what is being
done. Kept apart, they forget each other: a reader who opens the menu on an
item whose rebase is already replaying is shown the rebase as something to
start, presses it, and is either refused by a lock or has started a second
one — and has to visit the board to learn which. That is the watch knowing a
fact and not saying it where the reader is looking ([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now)).

So **every entry that has work going about its subject is marked running,
and set apart**: the running entries stand first, under a line that says so,
indented a step further than the rest, in one colour reserved for what is
going and used for nothing else on that screen. Each says how long it has
been going and what it is at right now — the job's own last line, the ticket
a run holds and the state it is in, *waiting on you* where the ticket it
opened is parked, with a run still on the root or without one, *queued* where
the root's run will reach it — in the words the board already uses
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place),
[§FS-005-dispatch.18](FS-005-dispatch.md#18-the-work-screen-says-when-and-folds-away-what-is-over)), because
this is the board's reading narrowed to one row, not a second reading.

**What counts as going is found by looking, here too.** A command entry is
running where a job started from that entry, about this subject, still holds
its lock — so a job records which entry it came from and, on a branch row,
which branch, or nothing could ever match it back
([§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)); the checkout
row is running where the job that is making the workspace is. An entry that
hands work over is running where the ticket it would open, or the plan it
would lay, is open and its root is live or will reach it — the very facts the
work's own lines beneath the row are made of
([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking),
[§FS-005-dispatch.23](FS-005-dispatch.md#23-work-stands-on-rows-of-its-own-beneath-the-row-it-is-about)) —
and a ticket the run parked counts, live root or not, because a question
standing on this subject is exactly what a second dispatch must not be laid
beside.
An entry whose program runs in a window of the reader's own is running while
that window holds it
([§FS-005-dispatch.22](FS-005-dispatch.md#22-a-window-of-the-readers-own-where-one-is-bound)). Nothing here is
remembered from the keypress: a second ephor opening the same menu sees the
same rows, and a job that died is not running, whatever started it.

**Pressing a running entry opens it; it never starts it again.** The key on a
row that says *running* goes to the thing that is running: a job's log,
followed as it writes ([§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen));
a run of the runtime, attached
([§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching));
a program in its own window, that window brought forward
([§FS-005-dispatch.22](FS-005-dispatch.md#22-a-window-of-the-readers-own-where-one-is-bound)). A second copy is
not what a reader pressing a row that says *running* meant, and where
somebody does mean it the command line starts it and the refusal is the
lock's own sentence. The footer says *open* on such a row, not *run*, because
it is built from the row and not from the key ([§FS-004-quick-actions.2](FS-004-quick-actions.md#2-offered-only-where-it-would-work)).

**Both surfaces say it.** The list `ephor actions` prints carries the same
mark with the same facts — what is running, since when, and the way in
([§FS-011-command-line.8](FS-011-command-line.md#8-what-is-going-is-said-and-the-way-in-is-printed)) — so that a program reading the menu cannot start
what a person reading it would have opened ([§REQ-002-parity.2](../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).

## 22. A window of the reader's own, where one is bound

Two kinds of thing above want a terminal: the surface that attaches to a run
([§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)),
and the program an entry *is* — an editor, a pager, a coding agent's own
session ([§FS-006-project-interface.9](FS-006-project-interface.md#9-offers-the-projects-actions)). ephor has one terminal and is sitting
in it. Handing it over works everywhere, and stays the floor: ephor leaves,
the program runs, the reader comes back. But a reader inside a multiplexer,
or in a terminal that opens windows on request, has a better move available
— the program in a window of its own, ephor still on screen, and "open" from
then on meaning *bring that window forward* — and a tool that cannot make
that move sends them to make it by hand, which is the sweep this project
exists to retire ([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)).

So **the window is a seam** ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy), decided with its tradeoff
recorded in [§DA-007-window-is-a-bound-opener](../decisions/architectural/DA-007-window-is-a-bound-opener.md#da-007-window-is-a-bound-opener-a-window-of-the-readers-own-is-a-bound-opener-with-the-terminal-as-the-floor)), with the anatomy every seam
has. The contract is in materials: one command that opens a window
running a given command and prints a handle for the window it made, and one
that brings a handle forward. The binding is configured — `window` in site
configuration names which — and an explicit binding always wins. Ephor ships
bindings for the common shapes: a terminal multiplexer, and terminals that
take remote commands. With no configured choice, their nonempty, trimmed
product markers are read without spawning anything. The multiplexer remains
eligible in an SSH-marked session because its window belongs to the attached
session; GUI-terminal bindings are eligible only when the shared SSH predicate
in [§FS-016-browser-opening.2](FS-016-browser-opening.md#2-automatic-selection-and-truthful-outcomes) says the session is local. Outside SSH the current
product-marker order is retained, while display variables alone select no
window. The degrade rule is the floor: no eligible binding means no window,
and the terminal is handed over as it always was. Where SSH suppressed an
automatic GUI binding, the line says that SSH permits automatic tmux but not
an automatic GUI window. A window is the reader's: ephor opens it and brings
it forward, and never closes it or ends what is in it
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).

**An entry may ask for a window.** An offer or a configured action says
`window` as it already says `background`
([§FS-006-project-interface.9](FS-006-project-interface.md#9-offers-the-projects-actions)): its program runs in a window of its own
instead of taking the terminal, and ephor stays where it was. Such a program
is an operation while it runs — it holds a lock as a job does
([§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)), its record
keeps the window's handle, and the window is its inspection where a log
would have been, because what it writes is on that screen and nowhere else.
That is what makes a coding agent started from the menu a row that says
*running* and opens to the agent
([§FS-005-dispatch.21](FS-005-dispatch.md#21-what-is-already-going-is-shown-where-it-could-be-started-again)),
rather than a program ephor handed the terminal to and forgot. Where no
window can be opened, the entry takes the terminal as it always did, and
says so.

**Attaching goes to a window where one is bound.** The surface on a run
opens in a window when there is one to open, and in the terminal otherwise;
either way leaving it detaches and the run goes on
([§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)).

## 23. Work stands on rows of its own, beneath the row it is about

A matter's work used to ride on the end of the matter's own line — one
phrase, after the title, the state and the gate: *⚙ fix-gate · fix*. It said
the true thing and left the reader nowhere to go with it. The line is not a
row, so the cursor cannot reach it; the keys on the row it rides belong to
the matter, so none of them are the work's; and the phrase is cut to what is
left of the width after everything else on the line has taken its share. A
reader who reads *⚙ fix-gate · fix* and wants to take it back has to open a
second screen to find the thing they were already looking at, which is the
sweep this project exists to retire ([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)).

So **the work comes off the matter's line and stands on lines of its own,
beneath it** — indented a step, selectable like any other row, one per ticket
the plan holds open. The matter's line goes back to being about the matter,
and the work is where a key can reach it.

**What each line says** is what the board and the work screen already say, in
their words, because this is that reading narrowed to one matter and not a
third one of its own
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place),
[§FS-005-dispatch.18](FS-005-dispatch.md#18-the-work-screen-says-when-and-folds-away-what-is-over)): the ticket's
recipe and the state it is in, and how long since it was asked for, where the
ledger knows — a ticket nobody dispatched carries no age rather than a guessed
one. A ticket the runtime parked says *waiting on you* and stands first among
them, since it is the one part nobody else will move
([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)).

**A ticket a run has in hand says so, and says it at a glance.** *Open* and
*being worked on right now* are different facts, and a row that spelled them
the same way left the reader to guess which of the two they were looking at:
the ticket an agent is inside of and the ticket nothing has picked up wore one
marker and one colour, and the only way to tell them apart was to leave for
another screen — which is the sweep this project exists to retire
([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)). So a ticket a live run holds is marked apart from one
merely open, read from the run's own record of itself
([§FS-005-dispatch.15.2](FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream)) and
phrased as the board phrases it, because this is that reading narrowed to one
matter and not a third one of its own
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). A ticket on a root whose
run is live but busy elsewhere is *queued*, for the same reason the board says
so: it will get its turn without anyone doing anything. And a live run that has
gone quiet carries the badge it carries there — a long tool call is legitimately
quiet, so it is a badge and never a verdict.

Nothing here is a new question asked of the world: it is the liveness the watch
already probes and the record the run already writes, said on the row the
reader is already looking at.

**What is over is one line, not many.** Tickets accumulate and are all kept
([§FS-005-dispatch.16](FS-005-dispatch.md#16-work-that-should-not-go-on-is-cancelled-and-the-plan-says-so)), and
a tree that grew a line per finished ticket would bury the matters between
them. So where a plan holds nothing open, its work is one line for what the
last ticket decided — the verdict, or *cancelled* — and where it holds
something open, what is over is not on the tree at all: the work screen is
where the whole record is read
([§FS-005-dispatch.18](FS-005-dispatch.md#18-the-work-screen-says-when-and-folds-away-what-is-over)). An item
that has moved under its work says so on a line of its own, in the words [§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)
gives it, because that is a fact about the work and not about the matter
([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)).

**The keys are the work's, on the row the work is on.** On such a line,
cancel takes *that* ticket back — named, with no second screen to choose it on
([§FS-005-dispatch.16](FS-005-dispatch.md#16-work-that-should-not-go-on-is-cancelled-and-the-plan-says-so)) —
attach watches the run holding it
([§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)),
and the plan opens for reading. What the line is *about* is still the matter,
so the keys that go to the matter — its thread, its gate is not among them,
its work screen, its menu — go there from here too. A key means one thing at
a time and the footer says which, measured against the row the cursor is on
rather than the screen it is in ([§FS-004-quick-actions.2](FS-004-quick-actions.md#2-offered-only-where-it-would-work)): where the cursor
stands on work, the footer offers the work's keys and not the ones they
displaced.

**A line is offered only where the move behind it would work.** Cancelling is
the runtime's move and is refused in the runtime rung's words with nothing
bound ([§FS-005-dispatch.16](FS-005-dispatch.md#16-work-that-should-not-go-on-is-cancelled-and-the-plan-says-so));
attaching needs a run actually holding the root, read at the keypress from the
lock and never remembered
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)); a line for work that is
over has no ticket to take back. Each says so in one sentence rather than
appearing to act.

## 24. Work nobody has to start starts itself

A ticket that needs a person to press a key before anything happens to it is
a ticket waiting on a person, whatever its state says. For the work a reader
has already decided about — *the gate is red, collect what failed and fix
it* — that key is a formality standing between the dispatch and the run, and
the interval it costs is not the reader's attention being spent well: it is
work sitting in a plan on disk while the person who asked for it does
something else, and the row saying *⚙ fix-gate · collect* the whole time,
which is true and reads as *going* when nothing is going at all.

So **a recipe may say that the work it asks for needs nobody to start it**,
and a ticket written from such a recipe gets its run without anyone pressing
anything. The reader's deliberate act moves one step earlier and is made
once: adopting the recipe, rather than starting each of its tickets
([§FS-005-dispatch.7](FS-005-dispatch.md#7-handing-over-work-is-the-readers-move-and-stays-inside-the-machine)).
Everything a recipe already decides — which items deserve work, what to ask
for, whose hand does it — is the same decision, and *and do not wait for me*
belongs beside it.

**Nothing autoruns unasked.** Silence means the key, exactly as before: a
recipe that says nothing about this is started by the reader, and so is
every menu entry, every workflow entry that did not say it, and every plan
somebody wrote by hand. The setting is written on the thing that hands work
over — a recipe, or an entry that lays a workflow down
([§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can)) — and
nowhere else, because the reader who trusts one kind of work to start itself
has said nothing about the rest.

**Starting is a sweep, and the sweep reads the world.** What starts a run is
not a memory of having dispatched something — that would be the ledger
deciding what exists, which is the one thing it never does
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)) —
but the same looking every other reading here does
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)): a root is **due** when
a plan in it holds a ticket that is open, unclaimed, not parked on a
question, and from a recipe that asked to run itself — and no run is live in
the checkout that root's work would run in. A ticket a hand wrote into such a
plan is due exactly as a dispatched one is; the recipe is a fact about the
ticket, not about who appended it.

*Parked on a question* is judged over the ticket's tree, not the ticket alone:
an open ticket in a gating state holds every open ticket of its top-level tree
out of the due reading, and a root whose every would-be-due ticket is held that
way is **waiting on a person** — passed over, never started
([§FS-005-dispatch.24.3](FS-005-dispatch.md#243-a-root-waiting-on-a-person-is-passed-over-not-started)).

**A dispatch or a sync may give the runs it starts arguments, for that
invocation alone.** `ephor work dispatch -- <RUNNER_ARGS>...` and
`ephor work sync -- <RUNNER_ARGS>...` supply the trailing vector to every
runtime run started by that invocation's own sweep, unchanged and in the order
it was given. Ephor neither interprets the arguments nor stores them in the
plan, ledger, or configuration: a later due sweep or sync does not inherit
them. Omitting the vector is the empty vector and preserves the behaviour each
verb had before this form existed. The passthrough already accepted by
`work run` is unchanged, and interface actions still supply no runner
arguments. A dry run, or a sweep held at the `--act` gate
([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)), still starts no runtime, whether or not a trailing
vector was supplied. Sync takes the vector for the reason its sweep is bound
by the budget: it is the trigger a timer runs before `work run --due`
([§FS-015-spend-ceiling.6](FS-015-spend-ceiling.md#6-only-the-sweep-is-bound-and-the-persons-key-never-is)), so a vector that reached only dispatch and the
backstop would miss the sweep that usually starts the work, and which verb
reached a root first would decide the command line its run was given.

**One live run per checkout, because a run is an agent editing a working
tree.** A root a run already holds is left alone: the runtime schedules one
run per root, the live run reaches a ticket written beneath it, and a second
run there would only wait for the first
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). But the root is not what
is really being shared. A run is made *in a checkout*
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)), and two work roots over
one tree — a second panta beside the first, or a root somebody pointed
elsewhere — are two agents editing the same files. So the invariant is over
the checkout, not the root: a root is left alone when a run holds it **or**
when a live run holds the tree its work would run in, whichever root that run
was started from. The trees are compared as the file system resolves them
rather than as somebody spelled them, so a symbolic link or a relative
spelling does not defeat the guard.

**The guard is where runs start, never where plans are written.** Laying a
ticket into a busy checkout's work root stays legal — writing a file is all
ephor does there, and the ticket simply waits in the plan until the tree is
free. That is what makes handing one down safe: a conflict written into the
very tree that is stopped mid-rebase is a note for whoever gets there next,
not a second agent in it. Nothing refuses at dispatch, at lay, or at sync for
this reason.

**A writer that is not a run is held by it all the same.** The permissive half
above justifies itself with *writing a file is all ephor does there*, and that
sentence is false of anything that moves the tree. An unattended sweep that
replays branch checkouts onto main
([§FS-004-quick-actions.6.1](FS-004-quick-actions.md#61-the-same-replay-over-every-checkout-nobody-is-holding))
is exactly such a writer: it is not a run, so nothing above claims it, and it
rewrites the very files a live run's agent has open. So the invariant is read
over the tree and never over the kind of caller — a checkout a live run holds
is passed over by that sweep, said in the same words and the same kind of row a
held work root is passed over in, and it is never forced. `--force` is the
reader's word about a run they asked for by name; it reaches no sweep, and it
does not reach this one. A third such writer is the unattended dispatch itself
where its recipe mints a checkout per matter
([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)):
minting a tree and writing a plan into it is the permissive half's *writing a
file*, so no ceiling here counts it — and it is bounded where it must be,
at what the sweep is offered rather than at what it opens, because a writer
that can create the matters it is next selected for is not bounded by a number
at all ([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)).

**A run the reader asks for by name is refused by name.** `ephor work run` on
a plan whose checkout a live run holds starts nothing and says *a run is live
in this checkout*, naming the run — its own id where it published one
([§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)),
the root holding it where it did not — so the reader is sent to the run that
is in the way rather than to a guess. A run this same invocation has just
started counts: one command over two work roots in one tree starts one of
them and refuses the rest, naming the run it made a moment ago, because a
tree is as busy from a run one second old as from one that was there all
along. `--force` starts anyway, for the reader who knows what that other run
is doing; it lifts this refusal and nothing else, on the run asked for by
name — the sweep below is not a run anybody asked for by name and is never
forced. A refusal is a non-launch outcome of a command that was understood,
not a failure of it.

**The key in the interface is a place a run starts, so it is guarded there
too.** Pressing it on a matter whose checkout a live run holds starts nothing
and says the same sentence, naming the same run: the invariant is over the
tree the agent edits, and it cannot depend on which surface the reader
reached for. There is no forcing it from the screen — the reader who means
to start a second run in a busy tree says so on the command line, where the
flag is.

This is why the sweep can be run as often as anything cares to run it and asks
nothing about what it did last time — a due root gets a run, a root whose
checkout is busy gets nothing, and running the sweep twice in a second is the
same as running it once. Inside one sweep the same holds without a second
look at the world: a checkout a launch has just taken is taken for the rest of
that sweep, and a later due root over the same tree is passed over with the
reason, which is a successful non-launch outcome and not a failed start.

**A root held back by another root's run is passed over, not dropped.** The
sweep says so in the same words and in the same kind of row as the tree it
took mid-sweep: a reader told only that nothing started would go looking for
a ceiling that is not full, and an empty sweep is exactly what a quiet
machine looks like. The root whose *own* run is live is the one exception and
stays silent — it has its run, that run reaches the tickets written beneath
it, and saying so every sweep would report on the ordinary case for as long
as the work takes.

**Autorun may be bounded at three nested scopes without changing the manual
key.** The site's `work.max_concurrent` is the aggregate ceiling on live runs
across all work roots; `organizations.<org-id>.work.max_concurrent` is a
ceiling on the live roots of every project that organization holds, inside
the site's; and `projects.<id>.work.max_concurrent` is a ceiling on that one
project's live roots, inside both. The site holds every organization, an
organization holds its projects, and a project holds its roots — nesting the
registry already declares, because which organization a project belongs to is
the `organization` field on its registry row and nothing else, read here and
never written
([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)).
Leaving any of the three out leaves that ceiling unlimited; writing `0`
admits no new autorun starts under it. An absent `organizations` map is an
omitted ceiling for every organization, so a configuration that has never
heard of this tier starts exactly the runs it started before; a project whose
registry row names no organization is under no organization ceiling. These
limits apply only to this sweep: a run a reader explicitly starts keeps being
the reader's move. `ephor work run --due --max-concurrent N` replaces the
site's configured aggregate ceiling for that invocation, including when `N`
is zero; every organization and project ceiling still applies inside the
command-line ceiling.

**The organization tier carries more than a ceiling.** `work.root` is read at
these same three scopes and by the same registry membership
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)),
and so is `work.recipes`
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)), so an
organization block is where the budget its projects share, the place their work
goes, and the way they are worked are all written. The three are read
differently and have to be. Every ceiling is evaluated and the outermost full
one refuses. A root is a single answer, so the innermost scope that writes one
is the whole answer and nothing above it is asked. A recipe list is neither a
bound nor an answer but an ordered menu: every scope is read, outward in, and a
later scope reusing an earlier scope's id replaces that recipe where it already
stands. Three readings in one block is not an untidiness to be reconciled — a
ceiling is a bound, a root is a place, and a menu is a list, and each is read
the way its own kind of answer is read.

**Every ceiling is evaluated, and the outermost full one is the reason.** A
start is refused by whichever is full first, asked outermost scope inward —
site, then organization, then project — and within one scope by roots in
flight before working roots. So no root begins because an inner ceiling had
room while an outer one did not, the reason a reader is given names the widest
thing that was actually full, and a scope that declared no working ceiling
answers exactly as it did before there was one. None of them replaces another
and none clamps another: they are questions asked of the same start, and the
answer is the first *no*.

**A ceiling written the wrong way round is named, not corrected.** An
organization's number is expected to be the larger of the pair, because it is
the budget its projects share, so a project ceiling above its organization's
— or above the site's — is almost always a mistake. It is warned about by
name at each sweep that reads the ceilings — `ephor work run --due`,
`ephor work dispatch` and `ephor work sync` — and the warning says which
project, which ceiling it is above, and both numbers, so it is seen when it
bites rather than only when someone runs a check over the file. It is not
refused and the project's number is not rewritten: whoever configured the one
project is closer to it than whoever set the number above it, and a tool that
silently clamped them would be deciding something it does not know. Nor is
the warning a licence — the organization's total still binds, because a pair
written the wrong way round is not permission to exceed a ceiling somebody
set on purpose. The site number a pair is measured against is the configured
one: `--max-concurrent` narrows a single sweep deliberately, so a project
ceiling above it is the reader's own choice rather than a configuration
contradicting itself, and nothing is said about it. A ceiling of `0` above is
not a pair at all: it admits no new starts under it, so it is a pause the
reader wrote on purpose rather than a budget a project could be above, and
the numbers beneath a paused site or organization are said nothing about
either — a configuration that pauses the site and leaves its project numbers
where they were is not thereby a configuration that contradicts itself.

**A block over nobody is said out loud.** An `organizations.<org-id>` key
reaches nothing exactly when no registry row places a project inside that
organization — the same reading the ceiling itself binds through, so a key
that is refusing starts is never announced as bounding nobody. Two
configurations arrive at that emptiness: an id no registry row names at all,
which is the typo removing the bound its author believes they set, and an
organization the registry declares that no project has yet joined. They are
one condition and are named the same way, because membership is the
`organization` field on a project's registry row and nothing else. Bounding
nothing is the one thing a ceiling may never quietly be, and a recipe nobody is
ever offered is that same silence one key over, so the block is named — by
`ephor doctor`, in the words an unknown project id is named in, and at the
sweep where the missing bound would have been read. It is named for reaching
nobody rather than for bounding nobody, because what is written in it may be a
ceiling, a root, a list of recipes, or any two of the three, and a block
carrying only recipes must not be announced as a ceiling its author never
wrote. It is not an error and nothing is refused for it: the runs that would
have happened without the key still happen, and the reader is told why the key
they wrote is not the one biting.

**A second ceiling bounds the work an agent is actually doing.** The three
above bound roots in flight — worktrees, processes, and the burst when several
resume at once — which is not the question *how much may be spent at once*. So
`work.max_active` is a second aggregate ceiling over the live roots that are
**active**, and `projects.<id>.work.max_active` is that project's ceiling
inside it. Both read exactly as their `max_concurrent` counterparts: omitted is
unlimited, `0` admits no new autorun starts under it, the project number sits
inside the site one, and a run the reader started by hand is outside both. The
organization tier bounds roots in flight only — an organization is a budget of
machine, and the working ceiling is deliberately left to the site and the one
project until a reader asks for the middle of it. Omission is the default
everywhere, so a configuration naming only `max_concurrent` is bounded exactly
as it was, in behaviour and in wording, and `--max-concurrent N` replaces the
site's roots-in-flight ceiling alone.

**And none of these is money.** What unattended work may *spend* is a ceiling
of its own, written at these same three scopes and evaluated in this same
order ([§FS-015-spend-ceiling](FS-015-spend-ceiling.md#fs-015-spend-ceiling-what-unattended-work-may-spend-is-the-persons-number-and-the-sweep-stops-at-it)).

**A live root is parked when what it waits for is a person.** Every live root
counts toward the flight ceilings — it exists, and that is what those numbers
count — and toward `max_active` too *unless* nothing in it is being worked and
something in it is somebody's turn: no open ticket witnessed held by the run,
and at least one open ticket in a state the machine in force calls gating
([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)) or
whose poll declares whose answer it waits for — one fact, differing only in who
resumes it. It is judged per plan by the machine answering for that plan, as
every reading of what a ticket is doing is
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)); a plan whose machine
cannot be read makes its root active, because a misreading must cost a slot
rather than hand one out. And the ceilings gate starts, not what is under way
already: parked roots resuming together can carry active work above
`max_active` until one finishes, capacity here being live work, not attempts.

**Capacity is live work, not attempts.** One root snapshot supplies both the
due candidates and the live counts. Every already-live root consumes one
aggregate slot, one slot in each project whose plan it holds, and one slot in
each organization holding such a project, whether or not that root is due or
selected by a command's `--project`. A root holding plans from two projects
of one organization spends one of that organization's slots rather than two:
the slot is the live run, and there is one of those. A successful start
consumes those same slots, as active work, only while its runtime lock remains
held. A start
that fails, reports itself finished during the launch handshake, or has
already released its lock leaves the slot for the next candidate in the same
sweep. The failed-start back-off still applies independently.

**The highest-ranked due roots get the available slots.** Where
`work.ranking` names item ids, due roots for those items are considered in the
file's order before the rest; roots the ranking does not distinguish retain
the deterministic root-path order the sweep already had. The ranking orders
and never filters. Every otherwise eligible root not started solely because
an aggregate, organization, or project ceiling is full is returned as one
`passed-over` outcome whose reason names the key it refused on, in prose and
`--json`; and the reading says how many roots are live and how many of those
are parked, so a full ceiling never hides that a person holds one of the
slots. Passing a root over is not a failed launch and does not increase the
reading's `failed` count.

**The sweep is safe where the key was.** Everything dispatch refuses before
writing a ticket, starting refuses before running one
([§FS-005-dispatch.6](FS-005-dispatch.md#6-dispatch-is-offered-where-it-would-work-and-refuses-where-it-would-not)):
a branch that is not in the working tree the plan is about is not run in,
whatever the directory holds
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)). And what it starts is
the run [§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)
already describes — detached, identified by the binding, watched by
attaching. Nothing about how a run is seen, stopped, or answered changes
because nobody pressed the key that began it.

**Both starts in the periodic sweep hand the run over before the service
leaves.** Synchronizing moved work may start its due run, and the final due
sweep starts work born anywhere else; a run detached at either position
survives the service's completion, stop, and restart on the terms
[§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)
gives every detached run. A later activation reads the world again and still
starts nothing in a checkout whose earlier run is live: surviving the launcher
changes none of the opt-in, eligibility, exclusion, ceiling, or back-off guards
above.

**A start that fails is not tried again immediately.** A root that cannot
start a run — a runner that refuses, a workspace that has gone wrong — would
otherwise be retried by every sweep for as long as the ticket stays open,
which is a loop nobody asked for and the most expensive kind of quiet. So a
failed start is remembered as ephor's own record of what ephor did (never as
work state — that is still the plan's
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work))),
and that root is left alone for a while, longer each time it fails. The
failure itself is not swallowed: it lands under the row it was about, the
way a job's outcome does
([§FS-005-dispatch.17](FS-005-dispatch.md#17-a-move-that-needs-nobody-runs-beneath-the-screen)), so a reader
who never pressed anything still learns that the thing they did not press
did not happen.

**A run that started and advanced nothing is not started again immediately
either.** The rest above covers the start that failed, and a root whose runs
keep finishing without moving anything walks through the one door it leaves
open: the start worked, so nothing is remembered, and every sweep for as long
as the tickets stay open makes another run that does the same. It is the same
loop at the same cost, and it reads worse than a failed start does — the row
says *started* every time, so a batch with one stuck root in it looks healthy
while a slot is held and nothing moves. So the sweep reads what the last run on
each root actually did, and a root whose last run advanced nothing is left
alone for a while, longer each time, exactly as a failed start is.

**That is the second exception to *the sweep reads the world*, and it is said
rather than left to be reconciled.** The paragraph above holds that what starts
a run is a looking and not a memory, and then remembers one thing: a start that
failed. This remembers a second, of the same kind and for the same reason — it
is ephor's record of what ephor did, never the work's state, which is still the
plan's ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)).
Two exceptions to one sentence are two too many to leave to inference, so the
sentence is read with both in it: the sweep reads the world for what is *due*,
and consults its own record only for what it has already tried and got nothing
for.

**The witness is the finished run's own record of itself**
([§FS-005-dispatch.15.2](FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream)). A root
the sweep is considering has no live run, by the invariant above, so the record
there is the last run's and is over. Nothing is inferred from the work: an open
ticket count compared across sweeps would be work state cached in the ledger
under another name, and a healthy run's ordinary act is moving a ticket from
one state to another while leaving it open, so a count-based reading would rest
exactly the roots that are working. Nor is it the run's exit status, which
cannot tell a run that advanced two tickets and then stopped from one that
advanced nothing at all. [§FS-005-dispatch.15.2](FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream) already requires that the record say how each
slot ended rather than leave the end to be deduced, and already scopes itself
to the live run *and to what the last one did*; this is that second half read
by the sweep rather than only beneath a row.

**A run advanced something if it says a pass progressed, or if a slot it
released ended in a completing outcome.** Either is enough on its own. The
per-pass answer is the runtime's own answer to this exact question and is the
first one asked; the slot outcomes are the floor beneath it, so a runner that
records how its slots ended and nothing about its passes is still read. Reading
both widens what counts as an advance and can therefore only rest fewer roots,
which is the right direction for a rule whose one real risk is calling a
working root stuck: a partial advance is an advance, and a run whose only
movement was routing a failure onward or rescheduling a poll completes no slot
but has moved. Which outcomes are completing is the binding's own grammar and is
spelled in one place, inside the seam that reads it ([§REQ-001-boundary.5](../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)); of the
handful it uses, one is. Any spelling this reader does not recognize makes the reading
**inconclusive**, and an inconclusive reading rests no root, counts no miss, and
leaves the sweep behaving exactly as it did before this paragraph. Unrecognized
may never mean stuck, or the day the runtime learns a new outcome word is the
day healthy roots start resting.

**What ephor keeps of it is a record of its own beside the failed-start one**,
never inside it: one successful launch would otherwise write and clear the same
field, and one interval would be counting two unrelated things. It holds three
facts and nothing else — which run was judged, so no run is ever counted twice;
how many judged runs in a row advanced nothing; and when the last verdict was
taken, which is what the rest is dated from. Nothing about tickets, states, or
work ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)),
and the record is dropped whole the moment a run there advances, mirroring what
a successful start already does to the failed-start record. A verdict is taken
only by a sweep that acts: a report held at the gate writes no ledger
([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)), and reading one stream twice gives one answer either
way.

**The rest has an end, because a rest that only doubled would hide the stall it
was written to expose.** The interval is the failed start's — five minutes,
doubling with each consecutive miss, capped at two hours — so a root left alone
is always tried again eventually. But past three consecutive runs that advanced
nothing the root stops being rested and stops being admitted at all, until a run
advances there, somebody starts one by hand, or the sweep finds the root waiting
on a person ([§FS-005-dispatch.24.3](FS-005-dispatch.md#243-a-root-waiting-on-a-person-is-passed-over-not-started)), and every sweep from then on says
so in the row where it used to say *started*. That is
[§FS-005-dispatch.11](FS-005-dispatch.md#11-a-failure-that-is-not-the-changes-fault-is-restarted-not-fixed)'s own
fourth clause arriving here: past a small number of restarts the infrastructure
is the thing that is wrong, and no amount of retrying is going to be the fix.
Nothing is written into a plan or a ticket for it and nothing on the operations
board changes — whether a *ticket* waits on a person is the machine's word and
stays [§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)'s.
This is ephor's word about its own starting, and the reading it corrects is the
sweep's own, which is the reading that was wrong.

**It binds the sweep, and never a run asked for by name.** The reader keeps the
key here as everywhere: `ephor work run` on that root starts a run and is never
refused because a sweep has given up on it, and `--force` neither lifts this nor
needs to, since the rest was never applied to a run somebody asked for. A rest
that could refuse a named run would leave the reader no way back into a root
ephor had stopped starting.

**A root the reader excluded is passed over by name.** `ephor work run --due
--except <root|item>` takes a work root on disk or, failing that, a matter id
matched against the ledger, may be given more than once, and excludes each root
it names from that one sweep with no judgement of any kind — it is for the
driver that has worked out for itself which root to leave alone. It says what it
did: every exclusion that actually applied is named in prose and in `--json`,
while one that matched nothing due is silent, having excluded nothing. A value
that resolves to neither a root nor a matter is refused by name, and so is
`--except` without `--due`, which binds nothing and must not read as though it
had ([§FS-011-command-line.9](FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)). What it does **not** do is narrow the width the
`--act` gate is counted over ([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)): a sweep over four
projects that excludes three roots is still a sweep over four.

**Both new skips are `passed-over` rows, and both happen before capacity is
spent.** They are said in the same words and the same kind of row as a root
another run's tree holds, with a reason naming which of them it was: the last
run there having advanced nothing, and when the root is tried again or that it is
now somebody's turn — or the exclusion the reader asked for. Passing a root over
is not a failed launch and does not raise the reading's `failed` count. A root whose plan needs
pools this site cannot have together right now is passed over in that same kind
of row and for the same kind of reason, before any capacity is spent on it
([§FS-005-dispatch.33](FS-005-dispatch.md#33-work-that-needs-several-pools-at-once-is-admitted-whole)). A rested
or excluded root is never a candidate, so it consumes no slot, frees none, and
counts toward no ceiling; an excluded root whose own run is **live** still counts
live, because capacity is live work and not attempts. And a root two of these
would refuse is refused once, by the first: a live run of its own is silent as
ever, then the reader's `--except`, then a root waiting on a person
([§FS-005-dispatch.24.3](FS-005-dispatch.md#243-a-root-waiting-on-a-person-is-passed-over-not-started)), then the no-advance rest, then a tree another
root's run holds, then the pools, then the ceilings — one row, one reason, first
match.

**A runner that writes no such record leaves all of this inert.** Where there is
no stream, or one this reader cannot understand, no verdict is taken, nothing is
remembered, and sweeps behave exactly as they did before — which is the floor
[§FS-005-dispatch.15.2](FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream) already keeps, and never an error — the degrade every seam owes
([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)), stated because a rule that quietly stopped applying
would be indistinguishable from a machine on which nothing ever stalls. A rule
that cannot read its witness says nothing rather than guessing.

**The reader keeps the key.** Starting a run by hand is unchanged and is
never refused on the grounds that a sweep would have got there — a reader
who wants it now says so, and a recipe that autoruns is still work whose run
can be watched, attached to, and stopped in the runner's own words. The
sweep only removes the requirement that somebody be present, and the board
stays what it is: it starts nothing itself, and what it shows is the run,
whoever asked for it
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). The key also reaches work
this sweep never touches, and on none of the terms above
([§FS-005-dispatch.30](FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work)).

### 24.1 The sweep announces each root by the outcome it reached

The sweep that acts prints a block per root, and until the start has been
attempted there is nothing to head that block with. So **the header is chosen
after the outcome is known, and it carries the outcome's own verb** — which
leaves `▶` meaning the one thing it has ever meant on this command: a run
began. A root whose run began keeps the pair it has, the runtime and the root
under `▶` and then `▶ run <id> started`; a root that got no run is headed by
the line that says so and by nothing else. The marker appears once per root
that started, and on no root that did not.

**Both non-starts are bound, not only the ceiling's.** A root passed over is
announced as passed over — whether a full ceiling refused it, the reader's own
`--except` named it, it waits on a person
([§FS-005-dispatch.24.3](FS-005-dispatch.md#243-a-root-waiting-on-a-person-is-passed-over-not-started)),
the last run there having advanced nothing rested it,
another root's run holds its tree, or its plan needs pools this site cannot
have together ([§FS-005-dispatch.33](FS-005-dispatch.md#33-work-that-needs-several-pools-at-once-is-admitted-whole)).
A root whose launch was refused is announced as that refusal. The two are
different outcomes and say so in different words, but they are the same fault
when a start marker is printed above them, and a rule written for one of them
would leave the marker free to lie on the other.

**The ticket list stays under the root in every case.** What made the root due
is this section's answer to *a run nobody asked for still says what it is
about*, and it is worth as much about a root that got no run as about one that
did: it is what names the work the reader has just been told nothing happened
to. Only the header is at issue: the sentence beneath it is the one line the
outcome is worth, it already says the right thing for every outcome, and it is
the same sentence wherever that outcome is met.

**Nothing the readings say moves.** `--json` already answers `passed-over`,
`failed`, `started` and `done` with the reason beside each, and no field, name
or value here changes; the gated report already decides the outcome before it
prints anything about the root ([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)).
This point brings the acting surface's text up to what those two have been
saying all along, which is the direction parity is owed in — the text and the
reading of one command answering one situation in one voice
([§REQ-002-parity.2](../requirements/REQ-002-parity.md#2-parity-runs-both-ways),
[§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).

### 24.2 A passed-over row names its hold as data

The reason a root was passed over is a sentence, and a sentence is for a
reader. A program deciding what to do about the root — fill a free slot with
the roots a slot would start, alert on a root ephor has stopped starting, count
the roots that are resting — needs to know **which** hold it was, and the
sentence's wording is not something a field protects
([§REQ-002-parity.4](../requirements/REQ-002-parity.md#4-the-machine-form-is-a-contract-not-a-dump)). A caller made to match on the words reads the
kind out of text the contract leaves free to change, and the first rewording
breaks it with nothing to warn it. So **every `passed-over` row of
`ephor work run --json` carries `hold`, an object naming the first hold that
stopped the root**, as a `kind` from a declared vocabulary together with the
numbers that hold rests on — the same facts the reason already says, as data
([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)). `reason` stays beside it, word for word
what it was, and it is the sentence rendered from the same hold rather than a
second account of it. A row whose outcome is anything other than `passed-over`
carries no `hold`.

The kinds, in the order the sweep asks them ([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)), and what
each one carries beside its `kind`:

- `excluded` — the reader's `--except` named the root. `except` is the value
  the reader gave, as they gave it.
- `person` — the root waits on a person: every ticket that would have made it
  due is held by an open ticket in a gating state in its own tree
  ([§FS-005-dispatch.24.3](FS-005-dispatch.md#243-a-root-waiting-on-a-person-is-passed-over-not-started)). `tickets` is a list naming each gated ticket that
  holds the root, as `{ticket, state}`, `ticket` plan-qualified as the row's
  ticket list is and `state` the gating state it sits in.
- `rested` — the last run there advanced nothing, and the root is tried again
  later. `run` is the run that was judged, `count` how many runs in a row have
  advanced nothing, and `until` the instant the root is tried again.
- `stopped` — past three runs in a row that advanced nothing, the sweep has
  stopped starting the root until a run advances there or somebody starts one
  by hand. `run` is the last of them and `count` how many there were.
- `tree` — a live run holds the checkout the root's work would run in. `root`
  is the work root that run was started from, and `run` its id where it
  published one.
- `pools` — a plan needs pools this site cannot have together right now
  ([§FS-005-dispatch.33](FS-005-dispatch.md#33-work-that-needs-several-pools-at-once-is-admitted-whole)). `plan` is the plan held, `pools` every pool it needs
  in the order it named them, `pool` the first of them that cannot be had, and
  `until` the instant that pool is free again, where its report named one.
- `concurrency` — a ceiling on roots in flight or on working roots is full.
  `scope` is `site`, `organization` or `project`, `id` the organization or
  project the ceiling is written on (absent at the site), `key` is
  `max_concurrent` or `max_active`, `limit` the ceiling in force — the
  `--max-concurrent` number where the reader gave one — and `count` the live
  roots, or the active ones under `max_active`, that filled it.
- `budget` — a ceiling on what unattended work may spend is full
  ([§FS-015-spend-ceiling.9](FS-015-spend-ceiling.md#9-a-refusal-names-the-scope-the-ceiling-the-total-the-hole-and-the-instant)). `scope` and `id` as for `concurrency`,
  `key` is `max_spend` or `max_tokens`, `limit` the ceiling — dollars under
  `max_spend`, tokens under `max_tokens` — `total` what the window measured,
  where it measured anything, and `until` the instant the window lets work
  start again, absent under a ceiling of `0`, which is a pause rather than a
  window that will pass.

Three things about the field are part of its contract, because a caller will
come to depend on each of them.

**The vocabulary is open.** The kinds above are the ones there are today, and
a kind may be added without that being a breaking change — `person` was the
first one added after the field shipped. So a caller reads a
`kind` it does not recognise as *not startable*, never as *startable*: the safe
reading of a hold one does not understand is that it holds.

**It names the first hold, and only the first.** A root two holds would stop
is stopped once, by the first in the order above — one row, one reason, first
match ([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)), and among the ceilings the outermost scope first
and, within one scope, roots in flight, then working roots, then money, then
tokens ([§FS-015-spend-ceiling.5](FS-015-spend-ceiling.md#5-every-ceiling-is-evaluated-and-the-outermost-full-one-is-the-reason)). So that order is now part of what
the field promises: `concurrency` means every hold before it was asked and
passed, and nothing after it was asked at all. Under `--max-concurrent 0` a row
held by the site ceiling may stand over an organization, project or budget
ceiling that is full as well; the field does not say whether a root would start
if every slot were free, and a caller that needs to know that is not told it
here.

**The gated report holds less.** A sweep at a width that is gated reports
rather than acts ([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)), and what it asks before it
reports is the reader's `--except`, a root waiting on a person, and the
no-advance rest — the holds that need no capacity read. Its `passed-over` rows
carry `hold` exactly as an acting sweep's do, and can only carry `excluded`,
`person`, `rested` or `stopped`. A gated `would-run` therefore does not say that
nothing holds the root; it says that none of those four does.

The published `work-run` shape declares `hold` in the same change, with its
kinds named in its description rather than closed in an enumeration, so the
schema reads the vocabulary as open exactly as a caller is told to.

### 24.3 A root waiting on a person is passed over, not started

A gate is where the machine says a person moves the work next
([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)), and a runtime halts at an open gate rather than
working around it. So a ticket beside the gate in the same tree — a supervisor
in a state that is not a gate, waiting on the subtask that is — is not work a
run would advance either, though it passes the due test taken ticket by ticket.
Judged that way the root is due, the sweep starts it, the run halts at the gate
having done nothing, and every rest the no-advance rule grants ends in another
such start until the root is stopped — on a plan working exactly as designed,
and stopped past the moment the person answers. This point is the due test
taken over the tree instead
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).

#### 24.3.1 A gate holds its own top-level tree

A ticket's **top-level tree** is the top-level ticket it belongs to together
with every ticket whose id extends that ticket's id with a `.` — `fix-gate-1`,
`fix-gate-1.triage` and `fix-gate-1.triage.repro` are one tree, and
`fix-gate-10` is another. An open ticket in a state the machine in force calls
gating holds **every open ticket of its top-level tree** out of the due
reading, itself included, and nothing outside it: a gate never holds a sibling
tree in the same plan, nor anything in another plan of the root. Gating is the
machine's word, read per plan exactly as the due test already reads it, and
nothing else is consulted — not the last run's stream, not its report, not when
the plan file last changed. The plan is the world
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).

A ticket whose poll waits on a person's answer is **not** held by this and
stays due exactly as before: it moves itself only when a run polls it, so
holding it would mean it never moves. Only a gating state holds.

#### 24.3.2 A root with nothing else due is passed over, naming its gates

A root where every ticket that would have been due is held this way **waits on
a person**. The sweep starts nothing there and gives it a `passed-over` row
whose reason says it waits on a person and names the gated ticket and its
state, carrying the hold `person`
([§FS-005-dispatch.24.2](FS-005-dispatch.md#242-a-passed-over-row-names-its-hold-as-data)), with the gated tickets as the ticket list under it
([§FS-005-dispatch.24.1](FS-005-dispatch.md#241-the-sweep-announces-each-root-by-the-outcome-it-reached)). It is a successful non-launch: it raises no
`failed` count, takes no slot, and counts toward no ceiling. Where the reader's
`--except` names the same root the exclusion is the row, the reader's word
coming first. The gated report gives the same row, since nothing here needs a
capacity read
([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)).

A **mixed root** — one tree held, another tree or plan in it with a ticket that
is due — is due, and its row lists the due tickets only. The run made there does
the ready work and stops at the gate; one plan never holds back another.

The moment the gated ticket leaves its gate, the tree is judged as before, and
the next sweep finds the root due with nothing remembered against it.

#### 24.3.3 Waiting on a person is never a strike, and it lifts the stop

A root waiting on a person is never started, so no empty run is made there and
none is judged. And a sweep that acts and finds a root waiting on a person
**drops that root's no-advance record** — a rest or a stop alike — exactly as a
run that advances drops it. A root that was stopped before this point applied,
or that collected strikes from runs that halted at the gate, therefore comes
back by itself once the person moves the ticket, rather than staying stopped
until somebody starts it by hand. What is kept in its place is only a mark that
the root was found waiting, so the run that was last there when it was found is
read as already judged once the gate moves, and never counted as a miss — a
strike it earned before is not counted twice, and one it never earned is not
counted at all; a later run is judged as ever. The mark names no run and no
ticket and is gone with the first verdict after it, so the ledger caches no work state
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), and a gated report writes no ledger at all.

A run asked for by name is blind to all of this, as to every guard the sweep
has because nobody is present
([§FS-005-dispatch.30](FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work)).

## 25. Work about a matter with no branch can mint the branch it needs

A pull request arrives with a branch, and everything above resolves from it:
the workspace it is checked out in, the work root inside that workspace, the
`{branch}` the ticket carries. An issue arrives with none — which is not a gap
in what ephor knows, because an issue *has* no branch until somebody cuts one.
But it leaves the kind of work that most needs a checkout, *do this issue*,
with nowhere to be done: a project whose checkouts are one per branch has no
workspace for a branch nobody has cut, and its root is a directory holding
those workspaces rather than a checkout of anything.

So **an entry that hands work over may say which branch that work belongs on**,
as a template rendered against the matter: `"branch": "fix/issue-{number}"`. It
is written on an entry that asks for a ticket or lays down a workflow — a
configured action carrying `agent` or `workflow`, a project's own offer naming
a workflow, the entry beside a workflow, and a recipe
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for),
[§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here),
[§FS-006-project-interface.9](FS-006-project-interface.md#9-offers-the-projects-actions)) — and never on
one that runs a command here: those say what they need on disk with
`requires_checkout`, and the workspace they need is one somebody else has
already made ([§FS-004-quick-actions.7](FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout)).

The shipped `implement` recipe ([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)) carries this template by default. Therefore
an unconfigured dispatch for a branch-less issue mints `fix/issue-<number>` and
writes its plan inside that workspace when the project has
`branch_root_template`; without that registry field it refuses by name and
explains the configuration required, leaving no work root or partial workspace.
An existing matter branch still wins, and a configured `implement` recipe still
replaces the shipped recipe and chooses its own branch behavior.

**The template is rendered like a brief**, from the same fields — `{number}`,
`{repo}`, `{kind}`, `{title}`, `{ticket}`, `{id_slug}`, and the rest of
[§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)'s vocabulary —
and three of them it may not name, because they are what it produces:
`{branch}`, `{workspace}` and `{reply}`.

**One of those fields serves every matter, and so is never withheld.**
`{id_slug}` is the matter's own id as a name ([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)), and every matter has an id: a
template naming it is never withheld for want of the field, because the field is
never empty, and what it renders is always a name git will take, because the
rendering is `[a-z0-9-]+` by construction. So the refusals below can still fire
on the rest of the template — a `{sprint}` beside it, a rendering that ends in a
`.` — and never on `{id_slug}` itself. This is what makes the field usable
exactly where the distinguishing fields are all absent: **a project's own tasks
can be dispatched with a checkout each.** A recipe over them
([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live))
writing `"branch": "task/{id_slug}"` mints one workspace per task, through the
same one checkout operation as every other branch here, rather than the one
shared workspace that `{project}` or `{source}` — the same for every task of a
project — would have given all of them.

**The digest is not decoration, and a bare slug would be wrong here.** Rendering
*is* the resolution and nothing is written down, so each matter's branch is
decided from that matter's own id and from nothing else. Two ids that read down
to the same slug would therefore resolve to the same workspace, and two
unrelated matters sharing one workspace is the failure this point refuses a
template for. A digest that appeared only on a collision would be worse than
none: there is nowhere to ask whether one was needed, so a second matter
arriving later would silently change what the first already resolves to. Hence
`rhei:window.retry-1` and `rhei:window-retry.1` mint `task/rhei-window-retry-1-17bbeb3b`
and `task/rhei-window-retry-1-5ff4987f` — two names a reader must look twice at,
and two trees.

Here, "from that matter's own id and from nothing else" is the construction and
collision guarantee of `{id_slug}`: what a rendering answers may depend on the
matter it is about and on no other matter. That is why the digest is
unconditional rather than a tiebreaker — a digest appearing only on a collision
would be a function of the population, and there is nowhere to ask whether one
was needed — and it is why two ids that read down to one slug stay two. It does
not constrain a template that explicitly names another field of the same matter.
Whether a field that *this* matter can change while its work lasts may be named,
and where, is a separate question, answered where the vocabulary is specified
([§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose),
[§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)) and
not here.

**No shipped recipe names it, and nothing defaults to it.** `implement` keeps
`fix/issue-{number}`. A store yields one matter per open task in every plan it
holds, so a recipe that named `{id_slug}` by default would mint a tree per task
of every project that keeps its work in its checkout — a forest that grows with
the store rather than with the work anybody asked for. Which scope work belongs
in is read off what that work touches and off nothing else
([§FS-014-work-root-scopes.2](FS-014-work-root-scopes.md#2-reach-places-and-nothing-else-does)),
and a checkout root dies with its branch, which is what makes it cheap and also
what makes an unasked-for one waste
([§FS-014-work-root-scopes.5](FS-014-work-root-scopes.md#5-nothing-durable-lives-in-a-checkout-work-root)).
So a reader who wants a checkout per task writes the field, and one who wants
work about a task beside the project's other work says nothing and keeps what
they have.

**And the minting stops at one generation.** The paragraph above refuses a
forest by *breadth* — a tree per task of every store — and a reader who writes
the field asks for exactly one tree per task, which is what it is for. Depth is
the other way that number grows, and nothing in the template bounds it: the
workspace this mints gets a task store of its own
([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)),
the dispatch writes its plan inside that store
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch),
[§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)), and a recipe
over this source would be offered its own plan back on the next read and mint
again. So what a dispatch wrote is not something a later sweep can dispatch:
the seam yields no matter for a plan ephor caused to exist, absolutely and
without a knob. The sentence above stays true and stays legal — a project's own
tasks can still be dispatched with a checkout each, and the reader gets the one
tree per task they asked for and no generation after it.

**A template that is wrong for every matter is refused by name**, where it is
read and rather than turned into a directory nobody meant, and the refusal
says which of the three things is wrong with it: it names one of the three
fields it decides; it names something that is no field of a matter at all, and
the refusal lists the ones it may name — the fixed vocabulary in full, and
`{meta.<key>}` named as the open one, because the keys under `meta` are
whatever this matter's source reported
([§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)) and no
refusal can enumerate a key a store has yet to invent; or what it renders is
not a name git will take as a branch. The last of those is answered here rather
than left to the checkout: git's own refusal arrives from inside the making, by which time
the directories leading to the workspace are there, so a template git will
not take is held to that before anything is made.

**A template that names a field this matter has not got means the entry does
not serve this matter.** It is withheld from the menu and its readings, and
from dispatch selection — including unattended sweeps — rather than selected
and refused, because another matter can carry the field and render the same
template correctly. A `{meta.<key>}` this matter has not got is such a field
and not an unknown name: the name is one a template may take, and it is this
matter that did not answer it, so the entry is withheld and the next matter
renders the same template. The `work offers` reading names the excluded entry
and the field it needed, as [§FS-005-dispatch.27](FS-005-dispatch.md#27-an-offer-that-a-selector-refused-says-why) requires; a silent disappearance would leave the
reader unable to distinguish an incompatible matter from no configured work.

**The matter's own branch always wins — but the project's main branch is
never a matter's own.** A pull request keeps the branch the forge recorded
and a matter the registry placed keeps the branch it matched — one the matter
names itself, never one its conversation merely quotes
([§FS-008-attribution.2.1](FS-008-attribution.md#21-a-matters-branch-is-one-it-names-never-one-its-conversation-quotes)); the template
applies only where the matter has no branch at all. The project's configured
`main_branch` is the trunk every workspace is grown from, not a branch any
issue or pull request owns, so a registry match to it counts as no branch
here: the template mints exactly as it would for a matter matched to
nothing, and the refusal below fires the same way where none applies. "Here"
is every write that places work: the workspace an edit is made in, and the
work root a dispatch or a lay writes its plan into — laying a ticket is a
write to a directory, not a reading of the change, so it resolves this way
even where the work it lays down only reads, and no plan about such a matter
is written inside the main checkout. What keeps answering from the registry
match itself, main branch included, is the reading: where the matter's code
lives right now — the directory a command about it runs in, and what the
work is told about where it is — and anything a reading shows about the
matter's own branch and checkout. Neither of those places anything. A
forge-recorded branch that happens to equal `main_branch` is the forge's own
fact and keeps winning — only the registry-matched arm is carved out.
Rendering it *is* the resolution, and nothing is written down: a second
dispatch about the same matter renders the same name, resolves the same
workspace, and lands beside the first, and a workspace that is already on
disk is worked in as it stands.

**Saying it means the work needs the checkout.** An entry carrying a `branch`
is work about a change and belongs inside that change's own workspace;
`needs_checkout` and `requires_checkout` go on meaning exactly what they meant
for every entry that says nothing.

**The workspace is made by the one checkout operation**
([§FS-004-quick-actions.7](FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout),
[§FS-005-dispatch.12](FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)): the same
source checkout, the same directory template, the same trees grown from the
project's main branch, the same task store — the third caller of one
implementation, so a workspace dispatch makes and a workspace the reader's key
makes cannot be two different things — where the project binds a checkout
command, that command is the maker here too, and a workspace it did not make is
refused in its own terms with nothing dispatched behind it
([§FS-006-project-interface.8](FS-006-project-interface.md#8-the-checkout-contract)). Nothing is written to the registry: a
workspace ephor made is found on disk like every other
([§FS-008-attribution.2](FS-008-attribution.md#2-two-stages-one-engine)). Nothing is pushed either
— publishing the branch is the work's move, not ephor's
([§FS-005-dispatch.7](FS-005-dispatch.md#7-handing-over-work-is-the-readers-move-and-stays-inside-the-machine)).
And a project with no directory template for its branches is refused by name,
the way a single-checkout project standing on other code already is
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)): nothing is minted into a
root that is itself the checkout.

**It is made after every refusal and before the first write.** Who does the
work is chosen, the machine is vetted, the workflow's inputs are answered —
and only then does the workspace appear, so a refusal still leaves nothing
behind
([§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)).
The machine vetted is the one in force where the work root is already there,
and the one ephor would install where it is not: a workspace that does not
exist declares nothing, and minting one in order to read its machine back is
the leaving-behind this rule forbids.
One case escapes it, and is the only one: a runtime that installs a machine of
its own when it is asked to make the store
([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live))
installs it inside the workspace the mint has just made, and ephor leaves what
the runner left standing. That machine is read after the workspace exists, so a
refusal on it is the one refusal that outlives the mint — and it says so,
naming the workspace it made, because a workspace nothing mentioned is one
nobody knows to look at. Nothing further is made behind it: dispatching into
that workspace again refuses on the same machine, now the one the work root
declares.
A run asked what it would do makes nothing at all: not the workspace, not the
work root inside it, not the files a runtime would be shown — those have
nowhere to go until the workspace exists — and it says the branch and the
directory it would make and the plan path inside it instead. A repository the
checkout refuses is the checkout's own refusal, reported in the checkout's own
words, and nothing is dispatched behind it.

**What the work sees is the minted branch.** `{branch}` and `{workspace}`
render with it, the ticket's identifiers carry it
([§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)), the ledger
records it, and the work root resolves inside the workspace — so the plan lands
in the tree the work will edit rather than beside it. A surface asking about
that work before it is dispatched asks about the same workspace: the hand an
entry would go to and the roster a picker offers are read against the work root
the dispatch will use
([§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)), which for an
entry carrying a `branch` is the root inside the workspace that entry names and
not the project's own.

The entry or recipe's selected `root` is rendered only after this resolution,
so `{workspace}` in an override names the minted checkout while `{root}` still
names the registry project root. The path promised by an offer or dry run is
therefore the path a real dispatch or laying writes, including when the same
project places a checkout-local fix and a project-wide sweep in different
roots.

**Offers follow.** An entry carrying a `branch` is offered on a matter with no
branch, in the *will check out first* shape rather than blocked as *the
matter's branch is unknown*
([§FS-004-quick-actions.2](FS-004-quick-actions.md#2-offered-only-where-it-would-work)), on the menu
and in every reading of the same menu ([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).

**And without a template, the command line refuses what the menu refuses.**
Work that edits the change, about a matter no branch could be found for, on a
project whose checkouts are one per branch, is refused — naming `branch` as the
way out — where it used to be written at the project root. That fallback was
the defect: work about a change, written into a directory that holds no change,
which [§FS-005-dispatch.6](FS-005-dispatch.md#6-dispatch-is-offered-where-it-would-work-and-refuses-where-it-would-not)
does not allow and the menu has always blocked. This is the two surfaces coming
to agree ([§REQ-002-parity.2](../requirements/REQ-002-parity.md#2-parity-runs-both-ways)), and it is the one thing here that changes for a
configuration written before it.

**The same refusal covers a matter matched only to the main branch.** Since
that match counts as no branch above, work that edits the change is refused
exactly as it is for a matter truly unmatched — but the refusal names the
main branch it declined rather than calling the matter's branch unknown, so
the reader is told what was passed over rather than only that nothing was
found.

## 26. An ordering already made can be read, and a limit bounds what runs

A sweep dispatches every eligible item in one order — today, newest
`updated_at` first — and that order decides which items get a ticket first.
Ephor has no opinion of its own about which item matters more: it does not
compute a rank from labels, from reactions, or from anything else, and it
does not ask a project to compute one either
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)).
What it can do is read one a project already wrote.

**The ranking arrives as a file: an ordered list of item ids, one per line,
most important first.** The id is the one `ephor feed` prints and `--item`
already takes everywhere else — `github-issues:agent-grounds/rhei#95` — never a
URL, because a matter without one (a project's own task,
[§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live))
would otherwise be permanently unrankable. Order in the file *is* the rank:
there are no scores, no bands, and nothing here interprets a tie.

**It is named two ways, the second good for one run.** The `work` block of
site configuration takes an optional `"ranking": "<path>"`, and
`ephor work dispatch --ranking <path>` displaces it for that invocation alone
— the same displacement `--hand` already gives a single dispatch
([§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)).

**Ranked items dispatch first, in the file's own order; everything the file
does not name follows, in the order it already had.** The file orders — it
never filters. An item the file is silent about is still eligible, still
offered a recipe, and still dispatched; it merely sorts after every item the
file did name.

**`--limit N` bounds how many items are dispatched — opened, or would-open
under `--dry-run` — taken from the top of that order.** It is the reader's
own number, not the file's: nothing a ranking names causes an item to be
dispatched that a recipe would not already have matched. A recipe decides
which items deserve work at all; a rank only orders the work the reader
already chose to do
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)). An
item skipped for another reason — it already has work, it fails `--kind` or
`--updated-within`, no recipe applies — costs nothing against the bound; only
an item actually dispatched does. An item whose deterministic opening move
finishes with nothing to hand over
([§FS-005-dispatch.12](FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)) opened
nothing either, and for the same reason costs nothing against the bound.

**A file that is absent, empty, or unreadable is not an error.** The sweep
falls back to the order it always used, and says which of the three happened
rather than failing silently. With no ranking configured and no `--limit`,
nothing about the sweep's behaviour or its output changes: this is a
capability turned on by naming it, not a default anyone pays for unasked.

**An id in the file that matches no eligible item is skipped and named, not
fatal.** Eligible here means the sweep's own project and recency-filtered
set — after `--project` and the feed's own recency window, before `--kind`,
`--item`, or `--updated-within` narrow it further — so an id excluded only by
one of those still matches. A ranking outliving the matter it names is
ordinary — an issue closes, a pull request merges — and the sweep continues
past it exactly as it does past everything else it cannot use.

**The reading says which file it used and how old it is, in prose and in
`--json` alike**
([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)),
so a ranking nobody has refreshed in a while is visible rather than
mysterious, and every id the file named that matched nothing is said the same
way.

**The producer of the file is out of scope.** Whatever writes it — a script
over a label, a person with an editor — is not ephor's concern here, and
nothing ephor ships makes one.

## 27. An offer that a selector refused says why

A recipe a selector refused and a recipe nobody wrote are not the same fact,
though an empty offers list has always told them apart with the one sentence
"nothing matches this matter". The first is something a reader can act on —
loosen the selector, or learn that the matter simply does not carry the field
it asked about — and the second is nothing to act on at all.

So **a recipe considered for the matter and refused by its selector or its
branch template is named, beside what refused it**, in the reading `ephor work
offers` returns: which of `roles`, `gate`, `needs_response`, or `sources` did
not hold and what the matter carried instead of what the selector asked for,
or which field its branch template needed and this matter did not carry. Where
the refused template named something that is no field of a matter at all, the
refusal lists the fields it may name instead, and `{id_slug}` is one of them
([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)) — it
is offered on every matter, because it is the one field every matter answers, and
so no recipe is ever named as excluded *for* `{id_slug}`: there is no matter that
has not got it.
"Considered" is narrower than every recipe the project has. A recipe whose
`kinds` refused was never about a matter of this shape at all — a `pr`
recipe has nothing to say about a task — so it names nothing, the same as a
recipe nobody wrote; `kinds` is what decides whether a recipe is considered,
not a reason reported once it is. And `behind` or `behind_upstream` refusing
alone names nothing either: whether a branch trails is a fact about a
checkout on this machine, already its own concept on the menu — the rebase
entries, and the `needs_checkout` gate — and every item with no local
checkout would otherwise report the same "could not be measured" line,
which is noise on nearly every reading rather than the one thing worth
saying. Both forms of the reading carry what is named, in the same words —
the JSON gains it as an additive field beside `offers`, the prose names it
under the same "nothing matches this matter" a reader already looks under —
because neither may know something the other does not
([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).

The role-less case is why this exists. A project's own tasks
([§FS-003-feed-categories.1](FS-003-feed-categories.md#1-the-categories)) carry no role
at all — there is no forge reviewer to be one — and a `roles` selector, being
non-empty by definition once it is written, matches a role-less item only
when it is empty
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)).
That rule does not change here: a `roles: [author]` recipe still refuses
every task, and correctly. What changes is that the refusal stops being
silent — a recipe that plainly covered issues and pull requests no longer
looks, without explanation, like it covers nothing about a project's own
tasks. This is a reading only: dispatch itself, and what it hands over, are
unaffected — the exclusion is `ephor work offers`' own diagnosis of one
matter, not a second thing the selector decides.

**`meta` is the refusal a task is about to get most often, so it is named like
the rest.** A selector asking what this matter's source said about it
([§FS-005-dispatch.31.1](FS-005-dispatch.md#311-and-it-can-ask-what-the-matters-own-source-said-about-it))
refuses every matter whose source said nothing — every pull request, every
issue, and every task in a plan carrying no such block — which is
[§FS-005-dispatch.31](FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)'s
silence rule working exactly as intended, and which makes it the commonest
single reason a reader will see no offers. So the reading names `meta`, the
keys the selector asked for, and what the matter carried under them: the keys
it has, or that its source reported none at all. The distinction is the one
this section exists for — a store nobody has annotated yet and a recipe that
asks the wrong key read identically from an empty list.

## 28. A workflow entry can ask for the same thing a recipe can

[§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself) removed the key from the
front of a recipe's work and left it in front of every workflow's, which is
where the unattended loop actually stops. A workflow is what fixes a matter
end to end — implement, review, ship — and it is exactly the shape of work
nobody should have to be present for. Yet it took two deliberate acts per
matter: laying the plan down, and then starting the run. Both are the
formality [§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself) already refuses to
charge a reader for.

**So a workflow entry may say `autorun`, and it means what it means on a
recipe.** The entry is the one
[§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)
already describes, in any of its three homes, and this is one more thing it
says beside its `when` and its `inputs`. Nothing else may say it: an entry
that runs a command here has no work to start, and an entry that asks for a
ticket says it inside the recipe it already is — two spellings of one fact
would drift, so the second is refused where it is written.

The same entry may carry the flat `root` placement key described by
[§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for). Its
value wins the selected recipe and every configuration tier, and the exact
root, checkout and branch used are recorded as the provenance of the plan it
lays. Repeating the entry consults that recorded placement rather than a later
item-level root.

**The sweep lays it, where nothing else would.** `ephor work dispatch`
already walks the matters that deserve work and hands each one to the first
recipe that applies. A matter no recipe applies to and that has no work at
all is where the entry gets its turn: the first workflow entry that both
matches and asked to run itself is laid down about that matter, through the
one path [§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)
already writes plans through. Recipes keep their priority — a matter a recipe
covers is a ticket, exactly as before — and a matter that already has work is
left alone, `--again` included: a second plan about one matter is something
`ephor work lay` is asked for, never something a sweep decides.

**It counts, reports, and refuses like a dispatch.** Laying is one of the
things `--limit` bounds
([§FS-005-dispatch.26](FS-005-dispatch.md#26-an-ordering-already-made-can-be-read-and-a-limit-bounds-what-runs)),
taken in the ranking's own order like everything else that sweep does. Under
`--dry-run` everything is resolved and nothing is written — not the plan, not
the record of it, and not the files a real laying would put beside it. An
entry that cannot be laid down — a required input nobody answered, a hand a
narrowing refuses — is reported as a refusal with nothing written, the way a
dispatch that could not open a ticket already is, and the sweep goes on to
the next matter. An entry held because the work it would lay needs several
pools at once and one of them cannot be had is reported the same way, by the
same rule and in the same kind of row
([§FS-005-dispatch.33](FS-005-dispatch.md#33-work-that-needs-several-pools-at-once-is-admitted-whole)). Both
forms of the reading carry it in the same words
([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).

**And what it laid is due like anything else.** A plan a workflow wrote is
work in a work root, so
[§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)'s sweep is what starts it:
its root is **due** when that plan holds a task that is open, unclaimed and
not parked on a question, and no run is live on the root. And a reader who
names the matter starts it too, whatever its entry asked for
([§FS-005-dispatch.30](FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work)).
Two things are the plan's own rather than the root's, and both are read where
the plan is. Its tasks are wherever the runtime wrote them — a plan rendered
as a directory keeps them in files beside its index, and those are as much the
plan's tasks as one written inside it. And they run under **the machine in force for that
plan**, which is the machine beside it where it declares one, because a task's
state means whatever the machine in force for its own store says it means
([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)).
Where such a plan declares no machine of its own, **the root's own answers**:
that is the machine the runtime resolves it against — a plan that names a
machine names the project's — and reading it under a default nobody chose
would call its finished work unfinished. A machine that is there and will not
read is neither: nothing in that plan is judged at all, rather than judged by
a machine that answers for other work
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).
A root's own machine answers for the plans the root holds directly, as it
always did.

**What asked for it is what the ledger says asked for it.** A recipe is a
fact about a ticket; the entry is the fact about a laid plan, and ephor's
record of the laying is where it is read from
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)) —
not from the plan, which is the runtime's and says nothing about who asked.
A plan nothing in the record laid — one a reader laid by hand, one that was
simply found in the root — asked for nothing and is nobody's to start, which
is [§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)'s silence again. That
holds however its tasks are named: the ticket id that says which recipe wrote
it ([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)) is a fact about the
tickets ephor itself wrote into the root's own plan, and a store of its own
names its tasks in the runtime's language, where the same spelling means
nothing of the kind.

**Everything else about the sweep is untouched.** The aggregate and project
ceilings, the cross-process reservation, the ranking, the failed-start
back-off, the one-run-per-root rule and the refusal to run in a working tree
standing on another branch all apply to a root a workflow laid exactly as
they apply to one a recipe wrote: it is the same root, reached the same way.
An entry that says nothing about this is a menu entry and only a menu entry,
laid by the reader and started by the reader, as it was.

## 29. Headroom is reported to ephor, and vetoes a member it never reorders

A list of alternates is only worth writing if something can say that the first
of them cannot be had. That something is a **quota**, and a quota is the
provider's fact rather than ephor's: ephor observes and summons, and it never
governs ([§REQ-001-boundary](../requirements/REQ-001-boundary.md#req-001-boundary-every-capacity-ephor-lacks-crosses-a-seam-and-the-seam-has-one-anatomy)). So it consumes a *report* about capacity and
never derives one, and the report is evidence over a list somebody else
ordered — never an ordering of its own ([§DA-009-headroom-vetoes](../decisions/architectural/DA-009-headroom-vetoes.md#da-009-headroom-vetoes-headroom-is-a-report-ephor-is-given-and-it-vetoes-a-member-rather-than-ranking-the-list)).

**The unit is a pool, and a pool is who serves the model.** A hand's pool is
its provider where the roster gives it one, and its agent id where it does
not. That is the smallest thing a window is actually bought against: two hands
served by one provider spend one allowance whichever of them is asked, so
evidence about either is evidence about both, while a hand whose profile names
no provider has nothing smaller than the agent that carries it to be limited
by. Keying evidence any finer would make a refusal that has already happened
look like it happened to somebody else.

**Two channels report it, and one of them costs nothing.** This is a seam, so
it has the four parts [§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy) requires of every seam, and the
first channel is the shipped default that keeps it working unbound.

The first is **the ledger**, which needs no configuration because ephor
already writes it. The one authoritative thing a provider says about its own
window is a refusal, and a refusal names the instant it lifts; it arrives as a
start that failed, which ephor records in its own words already
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)). Where those words carry an instant ephor can read, that start is a
**refusal on that hand's pool**, held until the instant and cleared by any
observed success on the pool. Where they do not, nothing about a pool is
claimed and the failure stays exactly what it was — one root's own back-off
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)) — because a failure ephor cannot date is not a window it may guess at.
The record is site data, kept where ephor's other state is and never written
into a project ([§REQ-001-boundary.4](../requirements/REQ-001-boundary.md#4-the-footprint-rule)); it stays ephor's record of ephor's own
act and never becomes the truth about the work ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)). A count of what ephor
spawned may be shown beside it and may never be read into the rule: counting
one's own spawns is deriving a quota under another name, and it is wrong the
first moment anything else — a session at a terminal, another machine — spends
from the same credential.

The second is **a bound verb**, richer and optional. `work.headroom` binds one
command per pool at the site ([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)), summoned exactly as every
other command this interface names — environment in, exit code out, structure
written to the file `$EPHOR_ANSWER` names ([§FS-006-project-interface.3](FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)) — and
its payload rides `data`, the envelope's own free passthrough
([§FS-006-project-interface.4](FS-006-project-interface.md#4-the-answer-envelope)). Not standard output: stdout is the command's
own and a contract that parsed it would make an honest log a protocol
violation ([§FS-006-project-interface.3](FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)). The payload is a list of **windows**,
each with a name, how much of it is left as a fraction, and when it resets.
Probing is fetching, so it runs where fetching runs — beneath the reading, on
the same freshness discipline as every other source ([§FS-001-forge-interface.7](FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it))
— and never in front of a dispatch, which would put a network call between a
reader and a ticket. How any one vendor is asked is volatile and stays outside
ephor: what ships is a worked example per vendor beside it, never a literal
inside it ([§REQ-001-boundary.5](../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)).

**A number nobody reported is unknown, and unknown is never zero.** A window
with no `remaining` is unknown; a pool with no readable window at all is
unknown. This is the load-bearing rule, and it is load-bearing because absent
is the *ordinary* case: the credentials that reach these providers show their
usage to a person and publish no number a program may ask for, so a rule that
read silence as exhaustion would veto every pool on the machine and stop the
loop it exists to keep moving. ephor already keeps this distinction where the
same mistake was available — an implementation with no notion of assignment
has nothing counted as unclaimed, because nobody's claim is not a claim that
nobody has it ([§FS-001-forge-interface.1](FS-001-forge-interface.md#1-capabilities)) — and the rule below makes it
structural rather than advisory. The rule is a veto over what is *known
spent*, never a ranking over what is known remaining, so there is no place in
it for an unknown to be sorted against anything: no number cannot demote a
member, because demotion is not something the rule can do at all.

**A pool's effective remaining is the least of its known windows.** A window
that is spent is spent whatever the others say — the shortest leash is the
leash — and an unknown window is simply not among them, so one unreadable
window never lowers a pool that another window has reported healthy.

**The rule: known spent is passed over, and everything else keeps its place.**
A member is passed over exactly when its pool is known spent — an unexpired
refusal in the ledger, or an effective remaining at or under a floor, which is
`0` unless the site names another. Every surviving member keeps the order its
author wrote ([§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)). Where *every* member is vetoed the first still gets the
ticket, carrying a note that names the earliest instant any of their pools
resets: a ticket that is written and waits is work a person can see, start by
hand, and reason about, and work that silently never dispatched is none of
those things: who does the work was never what makes a ticket ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)).

**Absence degrades to unknown, out loud.** A pool with no verb bound, a verb
that exits non-zero, output that will not parse, and an answer holding no
window ephor can read all read as unknown, with the reason shown beside the
pool. None of them is an error that stops a dispatch, and none of them is
silent: this is the degrade rule [§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy) requires, and both halves
of it are the requirement. A seam whose absence failed the dispatch would make
an optional verb mandatory; one whose absence said nothing would leave a
reader looking at a choice they cannot explain.

**The choice is recorded where the work is.** Selection runs at every write
ephor makes — a dispatch and a laying — and the member it chose, with whatever
the choosing had to say, is written onto the ticket in ephor's own words,
beside the dossier it already writes there ([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)) rather than as a field in the
runtime's plan language, which is the runtime's ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)).
So the plan itself says who and why, and a reader who was not there can read
both. Mid-plan exhaustion needs nobody steering: the spawn fails carrying the
instant, the sync that reads the failure writes the ledger, and the next write
ephor makes reads it and answers afresh. A pin already on a step is **never
silently replayed** onto it — a hand changing under a reader who is still
reading the plan is the one thing worse than a hand that waited — so where a
pin is answered again it is a **recorded plan edit** a reader can see.

**What is reported grows by addition.** `status` and `capabilities` gain a
line per pool — the pool, its effective remaining or *unknown* with the reason
it is unknown, and any unexpired refusal with the instant it lifts — and
`capabilities --json` gains the matching fields. That is the JSON form that
grows: `status --json` prints the matters a source reported, somebody else's
document rather than ephor's, with nowhere site-wide a pool belongs
([§FS-011-command-line.7](FS-011-command-line.md#7---json-is-the-same-answer-not-a-second-one)). Nothing already printed changes, which is what the
interface's own versioning asks of any growth ([§FS-006-project-interface.11](FS-006-project-interface.md#11-the-interface-is-versioned)).

**What this rule is about is one target, and it has not changed.** Everything
above governs the choice *among alternates for a single target*: the veto still
only strikes a member out, it still never reorders, unknown is still not zero,
and where every member is vetoed the first still takes the ticket and waits.
None of that is narrowed by what follows it. A separate question — whether a
piece of work whose several targets are bought against *several pools* can be
had at all — is not a question about a member and has no answer inside a list,
so it is answered on its own in
[§FS-005-dispatch.33](FS-005-dispatch.md#33-work-that-needs-several-pools-at-once-is-admitted-whole), which runs
after this rule and over the members this rule left standing.

## 30. A run asked for by name reaches the whole of that matter's work

[§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself) took the key off the front of
the sweep's work and left it in front of everything else, and
[§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can) says of an
entry that never asked that it is *laid by the reader and started by the
reader, as it was*. That start is owed to the reader and nothing performed it.
A matter whose only work was a plan a workflow laid had a verb that would not
reach it: `ephor work run --item <id>` read ephor's memory of the tickets it
had opened, found none, and answered a matter with a plan full of open tasks in
the same words, on the same stream, with the same exit code as an id nothing
had ever heard of. So the reader could not hold one matter back while the rest
of the site kept its ceilings — they had to leave ephor and run the runtime
themselves, which is the one thing the ledger exists to make unnecessary
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)).

**The key reaches every plan the record says is that matter's.** A run asked
for by name starts the matter's own plan and every plan a workflow laid beside
it, and names them to the runtime as the record names them
([§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can)) — never
the id of a plan nobody wrote. Which plans those are is read the way the sweep
reads them: the roots on disk, the tasks where the runtime wrote them, judged
by the machine in force for the plan they are in
([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)).
The key is a narrowing of that reading and not a second one, so the two surfaces
cannot come apart on what a matter's work is
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).
Where those plans occupy several committed roots, the key reaches each root
with its recorded checkout and branch; the same normalized reading is used by
a plain run and by the interface key.

**The key is blind to `autorun`.** `autorun` is the condition under which work
starts with *nobody present*, and its silence means the key
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)): a reader typing the
matter's name is the key. So a plan whose entry asked and a plan whose entry
said nothing are started alike by name, which is what
[§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can)'s *started
by the reader* has always required. Nothing here gives the sweep a plan it did
not already have: a plan whose entry said nothing is still no sweep's to start,
and a plan the record never laid — one a reader wrote by hand, one merely found
in a root — is nobody's to start at all, on either surface.

**The key inherits none of the sweep's limits.** The ceilings over roots in
flight and over active work, the spend ceiling's refusal, the failed-start
back-off and the site-wide autorun reservation all bound the sweep and only the
sweep, because each of them is a decision made with nobody present
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself),
[§FS-015-spend-ceiling.6](FS-015-spend-ceiling.md#6-only-the-sweep-is-bound-and-the-persons-key-never-is)).
A reader who names a matter is present and is deciding, so a full ceiling still
warns and still refuses nothing, and a root the sweep is resting on is started
at once. One rule survives, and it is the one that is not about attention:
**one live run per checkout**, because two runs in one working tree are two
agents editing the same files. That refusal is by name, counted as a refusal
rather than reported as an absence of work, and lifted by `--force` — exactly
as [§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself) already writes it.

**Both kinds of work in one root are one run.** Where a matter has a dispatched
ticket in its own plan *and* a plan a workflow laid beside it, the key starts a
single run naming both plans. It cannot be two: one live run per checkout means
the second would be refused by the first, and a single key producing a start
and a refusal of its own making is an answer nobody can act on.

**Three answers, and a reader can tell them apart.** The fault this point fixes
was not only the missing run — it was that a real matter and a typo got the
same bytes. So the key gives three distinguishable answers, in prose, in the
machine form, and in the exit code
([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)):

- **No work is recorded about the id at all.** This is an input that names
  nothing, which is refused by name and exits `2`, like every other refusal of
  something a reader typed
  ([§FS-011-command-line.9](FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)).
  The sentence quotes the id and names the two verbs that would give it work —
  handing some over, or laying a workflow about it. Under `--json` it is the
  refusal shape a refused command already prints, carrying that same sentence.
- **A matter the record knows, holding nothing a run would advance.** This is
  not a refusal: the command was understood and answered, and the answer is
  that the work is over, claimed, or waiting on a person. It exits `0`, names
  the matter, and says which of those it is in the terms
  [§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself) already uses — open,
  unclaimed, not parked. Under `--json` it is the run reading with no runs in
  it, carrying that sentence in `says`.
- **A matter the record knows, holding work.** The run lines, unchanged, and a
  reading whose `plans` name what the record named.

`says` is an addition to the run reading and to nothing else: where no run
started, the reading carries whatever sentence the prose carried, so the
machine form is never the poorer of the two
([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).
Nothing already printed changes shape ([§REQ-002-parity.4](../requirements/REQ-002-parity.md#4-the-machine-form-is-a-contract-not-a-dump)). The prose that
spoke of *a dispatched ticket* goes, because it was only ever half of what a
matter's work is.

**The key in the interface reaches the same work.** The screen's run key and
`ephor work run` are one ability
([§REQ-002-parity.1](../requirements/REQ-002-parity.md#1-an-ability-is-a-key-that-reveals-a-fact-or-changes-the-world)),
so the key names the same plans this point gives the command and starts them the
same way. It read the matter's own plan id and nothing else, which on a matter
whose work was entirely laid pointed the runtime at a plan that is not on disk.
A reader must not have to know which verb wrote the work in front of them.

**And a plain `ephor work run` reaches it too.** With no matter named the
command takes this same reading over every matter the record knows in the
projects it was given, so it starts plans a workflow laid exactly as it starts
the tickets ephor opened. `work run` and `work run --item X` disagreeing about
what X's work is would be this same fault one level up. It is still one live run
per checkout, and it is still a mutating verb, so above one project it reports
and acts only under `--act`
([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)).

**A root whose machine will not read is judged by nobody here either.**
Finality and gating are the machine's words, and with none to say them nothing
in that root can be called runnable
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). The key answers such a
root as the sweep does — it starts nothing there — and says so about the root,
rather than reporting the matter finished. ephor installs a machine in every
root it makes, so this reaches only a root somebody assembled by hand.
The same holds where the root's checkout stands on a branch other than the one
the record says the work belongs on: a run there would edit different code, so
the key starts nothing and names the branch the root is actually on, exactly as
dispatch refuses there
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)).

**A root the key will not start in is refused, never reported as empty.** Both
of those are answered the way the run already in the way is answered: the
sentence names the root, the reading counts it in `refused`, and the command
exits non-zero. Neither may come back as the answer that says the matter holds
nothing a run would advance, because that is the fault this whole point exists
to end — one layer down, and told to a reader who named the matter. `--force`
lifts neither: it lifts a run in the way, and these are facts about the root
that another key press does not change.

**One decision orders every refusal of the key.** For each selected root, the
reading first answers whether the root itself may be started in. Only a root
that passes that answer reaches the live-run safety question, and only a root
that passes both is runnable. `ephor work run --item …`, a plain named
`ephor work run`, and the work screen's `R` key render that one decision; none
orders the guards again at its own surface. Thus a root with both an unreadable
machine or wrong checkout branch and a live run is refused for the root fact
on every surface. `--force` changes only the live-run answer: it never turns a
root refusal into a runnable root.

## 31. A selector can ask who holds a matter, and what it is labelled

[§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for) gave the
selector the vocabulary of the *matter itself* — its kind, the reader's role on
it, what its gate is doing, whether it owes an answer, which source reported
it. That vocabulary cannot express the one distinction a busy tracker is
actually organized around: **which of these is mine to do, and which belongs to
something else already working the queue.** A repository whose automation files
its own issues gives every one of them the same kind, the same role, and the
same source as the reader's own, so a selector over that vocabulary either
takes the automation's queue along with the reader's or takes neither. A reader
who cannot say *this one is mine* dispatches blind, and a sweep that runs with
nobody present ([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)) makes that
the expensive kind of blind.

Two forge facts answer it, and a selector asks about both:

- **`assignees`** — the logins the forge says hold this matter. This is the
  *who*, and it is the difference between a backlog and an assignment.
  [§FS-001-forge-interface.1](FS-001-forge-interface.md#1-capabilities) already
  has `assigned`, but that is a yes-or-no about anybody at all: it can say a
  matter is taken and never say by whom, which is precisely the question a
  reader filtering their own work is asking.
- **`labels`** — the words the forge carries on the matter. This is the *what*,
  and on a tracker whose automation labels its own work it is the boundary
  between one pipeline's queue and another's.

**A positive asks for any; a negative forbids every one.** An entry is either a
plain name, which the matter must carry at least one of, or a name behind `!`,
which the matter must not carry at all. `["enhancement", "!GenAI"]` is
therefore *labelled `enhancement`, and not labelled `GenAI`* — the ordinary
shape of a queue a reader keeps and a machine feeds. The two halves are asked
independently: all negatives must hold, and the positives, where any were
written, must find one. This is the any-of rule `kinds`, `roles` and `sources`
already follow ([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)),
with the refusal the other three have no need of.

**An entry must name something.** `!` on its own names no label and no
login, so it is not a narrower filter — it is no filter at all, and a selector
that reads as one while excluding nothing is the shape a templated recipe
degrades into when the name it interpolates is missing. A field that asks for
nothing is written by omitting the field, so an entry that is empty or is `!`
alone is refused where the recipe is read, and the published schema refuses it
too ([§FS-006-project-interface.11](FS-006-project-interface.md#11-the-interface-is-versioned)).

**A fact nobody reported refuses, and never matches.** A source that says
nothing about labels or assignees has not said the matter is unlabelled or
unheld — it has said nothing, and the distinction is the same one `assigned`
is already careful about
([§FS-001-forge-interface.1](FS-001-forge-interface.md#1-capabilities)). So a
selector that asks either question of a matter carrying no such report refuses
it, exactly as a selector asking about a checkout it could not measure refuses
([§FS-004-quick-actions.6](FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase)).
The negative form refuses on silence too, and deliberately: reading "no labels
were reported" as "this one is not labelled `GenAI`" would hand the automation's
queue to an unattended sweep on the strength of a fact nobody stated. A matter
whose source *did* report, and reported none, is a matter with no labels, and
answers both forms as such.

**The question costs nothing extra to ask.** Both facts ride the search a
refresh already makes, as fields on the request rather than a request of their
own, so a selector gaining this vocabulary does not make a refresh dearer
([§FS-001-forge-interface.8](FS-001-forge-interface.md#8-a-refresh-is-asked-in-the-cheapest-form-the-forge-offers)).

**A refusal names the field, as every other refusal does.** `ephor work offers`
says which of the two refused and what the matter carried instead
([§FS-005-dispatch.27](FS-005-dispatch.md#27-an-offer-that-a-selector-refused-says-why)), including the case where
the source reported nothing — a reader whose recipe silently stopped matching
must be able to tell a matter that failed the filter from a source that never
answered it.

### 31.1 And it can ask what the matter's own source said about it

`assignees` and `labels` are facts a forge keeps, so a selector asking either
of them of a project's own task ([§FS-003-feed-categories.1](FS-003-feed-categories.md#1-the-categories)) asks a question no
store answers. Yet a store is exactly where the remaining distinction lives. A
project whose own work divides into slices — customers, environments,
subsystems — keeps that division in its own files, and until a selector can
read it such a project cannot be swept unattended at all: the sweep takes every
slice or none, and the alternative is a source script per value of one field.

So a selector may also ask **`meta`**: the bounded map of what this matter's
source said about *this matter*
([§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose),
[§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)). It is written as a map rather than a list, because
each key is a different question:

```json
"when": { "kinds": ["task"], "meta": { "context": "acme-labs", "tier": "1" } }
```

**Every key must hold, and each is compared as a string.** This is an `and`
where `assignees` and `labels` are an any-of, and for that reason: two keys are
two questions about the matter, not two spellings of one — *the acme-labs
context, at tier 1*. A number or a boolean the source reported answers by its
canonical spelling, so `"tier": "1"` matches a `tier` the store wrote as `1`; a
store writing its own files should not have to quote a digit to stay
selectable. A selector value that is not a string is refused where the recipe is
read, as an entry naming nothing already is
([§FS-005-dispatch.31](FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)),
and the published schema refuses it too ([§FS-006-project-interface.11](FS-006-project-interface.md#11-the-interface-is-versioned)).

**Silence refuses, and it is [§FS-005-dispatch.31](FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)'s rule rather than a second one.** A matter
whose source reported no such map at all, and a matter reporting one that has
not got the key asked for, are both refused: nobody said this matter is outside
the acme-labs context, and an unattended sweep may not read an absence as a
statement. So a `meta` selector never matches a pull request, an issue, or a
task in a plan that said nothing — and that costs no doctrine per source,
because a source that does not report `meta` is a source that said nothing about
it. The refusal names `meta` and what the matter carried, as
[§FS-005-dispatch.27](FS-005-dispatch.md#27-an-offer-that-a-selector-refused-says-why) requires.

**There is no negative form.** A key's absence already refuses, so `!` here
would have to mean *carried, and not this* — a second rule for a case nobody has
asked for. One rule now; the other can be added later without unsaying
anything here.

### 31.2 And it can ask why the matter waits

`needs_response` says *whether* a matter waits on the reader, and one yes
covers reasons that ask for different work: a conversation awaiting a reply,
and an issue nobody has taken ([§FS-003-feed-categories.4](FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it)). A selector that can
only ask whether cannot send the first to a reply and the second to the work
([§FS-005-dispatch.13.1](FS-005-dispatch.md#131-an-issue-nobody-holds-is-owed-work-not-an-answer)). So a selector may also ask **`awaits`** — the reasons
the matter waits on the reader, as a list of any of:

- **`conversation`** — the conversation awaits the reader, in any of the forms
  [§FS-003-feed-categories.4](FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it) reads the talk for, and every other way a matter
  waits that is not the one below;
- **`unclaimed`** — the matter is an issue nobody has taken, on a source that
  counts that as waiting.

**It asks for any of them.** `["conversation"]` matches a matter whose
conversation awaits the reader, whether or not nobody holds it as well; it
refuses one that waits only because nobody holds it. `["unclaimed"]` is the
other half. This is the any-of rule `kinds`, `roles` and `sources` follow
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)), and there is no negative form.

**A matter that does not wait answers none of them.** Finished work, an issue
blocked by an unfinished one, and a matter whose last word is answered all
carry `needs_response` false, and an `awaits` selector refuses them. **A matter
that waits and records no reason waits on its conversation**, because that was
the only reason a matter waited before the other was told apart; a source has
nothing new to report for the key to be asked of it. **Where several reports
are one matter** ([§FS-003-feed-categories.5](FS-003-feed-categories.md#5-one-subject-is-one-row-however-many-sources-reported-it)), its reasons are every reason any of
them waits for, so a mention owing an answer keeps `conversation` beside an
issue report that saw only that nobody holds it.

**The key is an addition.** The item gains the reasons as a fact beside
`needs_response`; nothing a caller of `feed --json` already reads changes
shape or meaning, and a configured recipe that does not ask `awaits` matches
exactly as before.

**A refusal names it, as every other refusal does**
([§FS-005-dispatch.27](FS-005-dispatch.md#27-an-offer-that-a-selector-refused-says-why)). `ephor work offers` names `awaits` and what the matter
carried instead of what the selector asked for: "the matter waits only because
nobody holds it; the selector asks for one awaiting `conversation`", or, where
it does not wait at all, "the matter is not waiting on me; the selector asks
for one awaiting `conversation`".

## 32. A recipe can ask for its own sweep, and say how often

[§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself) freed the *run* from the
reader's key. The *sweep that writes the ticket* is still the reader's to
perform, and the loop is therefore automatic in its second half only: what a
timer runs reopens matters already dispatched
([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)) and starts tickets that already
exist, and neither of those introduces a matter. A newly assigned issue sits in
the feed until somebody types the sweep, however precisely the recipe describes
the work it deserves.

So **a recipe may say that its own sweep needs nobody, and how often that sweep
should happen**. One field says both, and its presence is the opt-in:

```jsonc
{ "id": "implement",
  "when": { "kinds": ["issue"], "roles": ["author"] },
  "autorun": true,
  "dispatch": "6h" }
```

| `dispatch` | Means |
|---|---|
| omitted | the key stays the reader's, exactly as before |
| `"0h"` | sweep every time ephor is asked |
| any other interval | sweep at most that often |

`"0h"` is the always case and needs no second spelling: an interval of zero has
elapsed by the time anything reads it, so *every tick* falls out of the same
comparison every other value goes through rather than being a mode beside it.

**Deliberately one field, and not a boolean beside an interval.** An interval
already says the sweep needs nobody: a separate `unattended: true` restates it,
and `unattended: false` written beside an interval is a state the schema would
admit and nothing could mean. Silence is how a recipe declines
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)), and a boolean invites the
reader to write that silence out loud as a second, disagreeing answer.

### 32.1 The reader adopts the recipe; the recipe does the rest

[§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)'s argument is made here one step earlier and is the same argument.
Everything a recipe already decides — which matters deserve work, what to ask
for, whose hand does it, and whether the ticket waits for a key — is one
decision, made once, at adoption. *And find them yourself* belongs beside *and
do not wait for me* for the reason *and do not wait for me* belongs beside the
selector: a reader who has written a selector precise enough to trust
unattended has already made the decision that the per-sweep key press re-asks,
and `--dry-run` reports exactly what a sweep would open, so its precision is
knowable before anything is adopted.

The rules [§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself) sets for the run carry over unchanged. Silence means the key. The
setting is written on the thing that hands work over and nowhere else, because
a reader who trusts one recipe unattended has said nothing about the rest. And
a dry run still writes nothing, which here includes the sweep's own record of
having swept.

### 32.2 The rhythm belongs to the work, not to whichever unit calls ephor

Recipes have genuinely different rhythms. A gate that has gone red wants
reacting to quickly, and an hour of staleness is an hour wasted. A sweep over a
backlog does not need asking every half hour, and every sweep that dispatches
spends agents — so the difference between hourly and daily is money rather than
a preference. One interval for every recipe forces the slow work onto the fast
recipe's schedule.

The alternative available without this field is a timer unit per recipe, each
calling the sweep narrowed to one id. That works, and it splits one recipe's
description across two places: the recipe says what the work is, and a unit file
somewhere else says how often to look for it. That is the split [§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself) closed for
`autorun`, reopened one step earlier.

**This does not duplicate the scheduler, because ephor already works this way.**
`work run --due` is the precedent: the unit fires often, and ephor decides what
is genuinely due by reading the world rather than by being woken at the right
moment ([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)). A per-recipe
interval is the same move. The unit keeps firing at whatever rate it likes; a
recipe whose interval has not elapsed is skipped. **The unit sets the
resolution and the recipe sets the rhythm**, and neither has to know what the
other chose.

This is also the whole meaning of `"0h"`. Ephor has no daemon and is only ever
asked when something calls it, so the unit's own rate is the ceiling on every
value here: `"0h"` means *whenever you ask me*, never *continuously*. A reader
whose unit fires twice an hour has said, by writing `"0h"`, that this recipe
goes at whatever rate that unit was set to — which is why a recipe that wants a
floor of its own writes the floor rather than the zero.

### 32.3 Where the sweep happens, and what it is

The sweep is **`ephor work sync`**, which is the unattended verb a timer
already runs against the feed
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)). It needs no unit of its
own and no new line in anyone's: a recipe that adopts this field is swept by
the timer that is already installed, which is the point — the reader's act is
adopting the recipe, and adopting it must not also mean editing a service file.

What changes there is one question, not one job. Sync already walks every
matter in the feed and asks whether ephor has work about it: a matter it has
work about is reopened where it moved ([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)),
and a matter it has no work about is passed over. That second answer stops
being unconditional. A matter with no work is **opened** when a recipe that
asked for its own sweep covers it and that recipe's interval has elapsed, and
passed over otherwise, exactly as before. One walk of the feed, one question
asked of each matter, two answers instead of one.

**The recipe that sweeps is the recipe the reader's own sweep would have
chosen.** Recipes are offered in priority order and one matter wants one piece
of work ([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)),
so this asks for the first recipe that applies and acts only where *that* one
carried an interval. A matter whose best recipe stayed silent is left alone and
is never quietly handed to a lesser recipe further down the list that happened
to say `dispatch` — that would be the field deciding which work a matter
deserves, which is the selector's to decide and the ordering's.

**Everything the reader's own sweep refuses, this refuses.** It is the same
dispatch ([§FS-005-dispatch.6](FS-005-dispatch.md#6-dispatch-is-offered-where-it-would-work-and-refuses-where-it-would-not)):
a matter whose prerequisites are still open is withheld, a branch that is not
checked out refuses, a hand a narrowing will not permit refuses, and a matter
that already has work is left to sync's other answer. What it opens is a
ticket like any other, and the run it gets — or does not get — is
[§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)'s question and not this one:
a recipe may sweep itself and still wait for a key to run, or say `autorun` and
not sweep, because the two settings answer two different questions and neither
implies the other.

**The ordering already made orders this sweep too.** Where `work.ranking`
names item ids, the unattended sweep walks the feed in that order
([§FS-005-dispatch.26](FS-005-dispatch.md#26-an-ordering-already-made-can-be-read-and-a-limit-bounds-what-runs)),
as the reader's own sweep does. It is invisible until a bound stops a sweep
short, and then it is the whole question: a reader who wrote both a ranking and
a limit has said which matters the bound should spend itself on.

**An entry that lays a workflow is not swept.** [§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can) lets such an entry ask for
what a recipe asks for, and `autorun` is what it was given
([§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can)). It is
not given this: a sweep that laid workflows unasked would be opening plans of
their own about matters nobody has looked at, and silence there means the key
for the same reason it means the key everywhere else here.

**The `--act` gate is unchanged**, which is what makes this safe to put in the
verb a timer runs above many projects: a sync that reports rather than acts
reports what it *would* open beside what it would reopen, and writes neither
([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)).

### 32.4 A sweep of one's own may be bounded

Unattended dispatch makes the selector load-bearing in a way it is not
otherwise ([§FS-005-dispatch.31](FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)):
a mislabelled or newly-labelled matter reaches an agent without anyone having
looked at it. Two things stand beside the setting rather than after it.

The first is the interval, which is a safety control and not only a cost one: a
recipe that sweeps daily gives a reader a day to notice a selector that has
started matching the wrong thing.

The second is a bound on one sweep's own appetite. `--limit` already bounds the
reader's sweep ([§FS-005-dispatch.26](FS-005-dispatch.md#26-an-ordering-already-made-can-be-read-and-a-limit-bounds-what-runs)),
and an unattended sweep is the case it was written for — but the reader is not
there to type it, and the verb that hosts the sweep is not the verb the flag is
on. So a recipe that sweeps itself may carry one, in a long spelling of the
same field:

```jsonc
{ "dispatch": { "every": "6h", "limit": 3 } }
```

`"6h"` is sugar for `{ "every": "6h" }` and is the spelling to prefer; the map
is for the recipe that wants the bound. The limit is that recipe's own — it
counts what this recipe opened in this sweep and nothing else, so two
self-sweeping recipes do not spend each other's allowance — and it bounds what
is opened, never what is stepped over, which is the reading `--limit` already
has. Omitted, the recipe is bounded by the ceilings every start is bounded by
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself),
[§FS-015-spend-ceiling](FS-015-spend-ceiling.md#fs-015-spend-ceiling-what-unattended-work-may-spend-is-the-persons-number-and-the-sweep-stops-at-it))
and by nothing nearer. All of that is about what a sweep *runs*. What it is
offered to run at all is decided before any of it, by the source
([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)):
a matter the seam declines to yield never reaches a limit to spend one.

### 32.5 When each recipe last swept is ephor's record of ephor

Deciding whether an interval has elapsed needs one fact nothing here keeps
today: when this recipe last swept. That is a fact about ephor's own activity
and never a claim about the work, so it is kept the way ephor's other records
of its own acts are kept — beside `burn`'s cursors, in ephor's own state
directory, and not in the ledger. [§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)'s
rule is untouched: nothing here decides what exists, and the ledger goes on
answering the one question it answers.

It is kept per project and per recipe id, because that is what a recipe is: the
same id resolves to a different recipe in a project that replaced it
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)), and a
sweep narrowed to one project must not spend another project's clock.

**A missing or unreadable record means due now.** A reader who deletes this
state loses a sweep's worth of waiting rather than the sweep itself, and a
record ephor cannot parse is the same as one that was never written — the
degrade every reading of ephor's own state owes
([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)).
Erring toward sweeping is the right direction: the cost is one early sweep,
bounded by everything above, where erring the other way is a queue that
silently stops being looked at.

**A sweep the reader typed marks the clock too.** `ephor work dispatch` opens
what a self-sweeping recipe would have opened, because it opens everything, so
a record that ignored it would send the timer to look again at a queue a person
had just emptied by hand. The mark is what it always is: this recipe was swept,
at this moment, in this project.

## 33. Work that needs several pools at once is admitted whole

[§FS-005-dispatch.29](FS-005-dispatch.md#29-headroom-is-reported-to-ephor-and-vetoes-a-member-it-never-reorders)
answers one question about one step: given an ordered list of alternates for a
single target, which of them can be had right now. A workflow asks a different
question. Its execution targets are answered one at a time
([§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)),
and a plan whose moderator is bought against one provider while its second
participant is bought against another needs **both** windows open to reach its
end. No list can say that, because a list has a survivor and this has none: a
rule that selected one member would be answering a question nobody asked. So
this is a question about the *work*, asked once, after the members have been
chosen — and its only two answers are *admitted* and *held*.

**The requirement is derived, and nothing declares it.** The pools a piece of
work needs are the distinct pools of the hands ephor itself resolved for it:
every input that names who does the work, together with the hand the entry's
own pin chose. A pool is what
[§FS-005-dispatch.29](FS-005-dispatch.md#29-headroom-is-reported-to-ephor-and-vetoes-a-member-it-never-reorders)
says it is, and every one of these hands already carries the pool its work
would be bought against, because resolving it is what produced the pool. No
configuration key carries the requirement and no site writes one: it follows
the targets, so answering a target with a hand on another pool changes what the
work needs along with it. Because it is derived, **no site opts in** — work
that already resolves to several pools acquires the requirement with nobody
writing anything, which is the behaviour change this point makes and the one
worth saying out loud.

**A set of one is never held.** Where every resolved hand is bought against a
single pool there is nothing here to ask, and
[§FS-005-dispatch.29](FS-005-dispatch.md#29-headroom-is-reported-to-ephor-and-vetoes-a-member-it-never-reorders)'s
answer is kept entire — including its own hardest case, where every alternate
for the one target is spent and the first still takes the ticket and waits. One
pool spent is a question that point already answered, and this one does not
reopen it.

**The rule: two or more pools, and every one of them must be had.** Work whose
resolved hands are bought against two or more distinct pools is admitted only
when none of those pools is known spent and none of them is a pool this site
cannot reach at all. Otherwise it is **held**: nothing is written — no plan, no
record of one, none of the files a laying puts beside it — and the matter stays
in the feed exactly as it was, unclaimed and available to a machine that has
what it needs
([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)).
Holding is not an error: it is a refusal with a reason, the kind
[§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can) already
reports for an entry nothing answered, and the sweep goes on to the next
matter. An unattended start that cannot finish is not a handover
([§GOAL-004-handover](../goals.md#goal-004-handover-routine-moves-leave-the-persons-hands)),
and half of one costs twice: the attempts spent against the shut window, and
the claim that keeps the matter away from whoever could have finished it.

**Absent holds as spent does, and says something else.** A required pool this
site does not reach holds the work exactly as a spent one does, because the
work cannot be finished either way; the two are told apart in the sentence
rather than in the outcome. Since the pools are derived from hands the roster
answered, a pool that is on the roster is reachable by construction, so this
case surfaces as exactly one thing: a required target that resolves to no hand
at all, which already refuses and already writes nothing. What this adds there
is the sentence. Such work parks and nothing lifts it — there is no window to
reopen — and the way out is to answer the target with a hand this site has.

**Unknown is not spent here either.** A pool nobody reported a number for
holds nothing. This rule asks only what
[§FS-005-dispatch.29](FS-005-dispatch.md#29-headroom-is-reported-to-ephor-and-vetoes-a-member-it-never-reorders)'s
veto asks — is this pool *known* spent — and there is no branch in it for a
missing number, because absent is the ordinary case and a rule that read
silence as exhaustion would hold every workflow on the machine and stop the
loop it exists to protect. That is also this rule's degrade under
[§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy): with
nothing reporting, nothing is held, and the pool line already says why it has
no number.

**It runs after the veto, and over what the veto left.** Selection happens
first and is untouched: a target that named alternates still passes over what
is known spent and keeps the survivors in the author's order. Only what
survives is counted into the required set. Where a target's every alternate is
spent the veto still returns the first, and this rule then sees that pool spent
— so multi-pool work in that state is held while single-pool work is written
and waits, which is the one place the two dispositions visibly differ and is
the difference this rule exists to make. That the two answers differ is a decision with a cost
of its own, and it is recorded as one
([§DA-010-work-is-admitted-whole](../decisions/architectural/DA-010-work-is-admitted-whole.md#da-010-work-is-admitted-whole-work-that-needs-several-pools-at-once-is-admitted-whole-or-nothing-is-written-at-all)).

**One clause, and the verb belongs to the surface.** The reason is rendered
once — which pools the work needs together, which of them is unavailable, and
whether it is spent until a named instant or simply not reached here — and each
surface puts its own verb in front of it, so a reader meets the same sentence
wherever they meet it. Admission reports it as a refusal with nothing laid, the
menu row carries it as the reason that entry is blocked, and the unattended
sweep reports it as a root it passed over. Every one of those is a shape these
surfaces already have, and both readings of each carry the same words
([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).

**Admission is one place, and it covers every door that writes.** What is held
is the *admission* of the work, which every writing path already goes through:
the sweep that lays a workflow entry
([§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can)), a
laying the reader asked for by name, and an entry that asked to run itself. A
dry run reports the hold and writes nothing, as it already does for every other
refusal, and there is no flag that overrides it — the escape is to answer a
target with a hand this site can have, because the requirement follows the
target.

**A plan already laid is held at the start, and says so differently.** The
unattended sweep decides about plan roots found on disk and never sees the
entry that laid them, so it reads what the work needs from ephor's own record
of the laying
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)),
which already names the plan, its root, its checkout and its branch. The hold
is a `passed-over` row — the shape a ceiling and a busy tree already take
there, a successful non-launch with its reason in the row rather than a failure
([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)) — and it is checked before
capacity is spent. It reads differently from the admission's: that one is
*held, and still anybody's*, this one is *not started, and still yours*. A
record written before ephor knew to write this carries no requirement and
starts as it always did, which is what growing an interface by addition means
([§FS-006-project-interface.11](FS-006-project-interface.md#11-the-interface-is-versioned)),
and a plan nothing in the record laid carries none either, which is
[§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can)'s silence
again.

**A run asked for by name is warned, never held.** A reader who names a plan or
a matter keeps the key
([§FS-005-dispatch.30](FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work)):
the plan is in front of them, it is already laid, and holding it would leave no
way to run such a plan at all. It carries the same clause as a warning and
starts. Only the sweep nobody typed is held, which is where the attempts this
point exists to stop were being spent.

**Recipes and single-target work are untouched.** A recipe hands over a ticket
rather than a plan with targets of its own, so it has one pool and nothing here
to be held on; a workflow whose hands all land on one pool is a set of one; and
an ordered list of alternates anywhere keeps
[§FS-005-dispatch.29](FS-005-dispatch.md#29-headroom-is-reported-to-ephor-and-vetoes-a-member-it-never-reorders)'s
behaviour exactly. Nothing already written is read differently, no file format
changes, and no configuration becomes invalid.

## 34. A brief may be kept in the file that owns it

The brief is what the ticket asks for, in the reader's own words
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)) — and
some of those words are not about the item at all. How work is done under this
organization, what a house review looks at, the long standing prompt that took
a year to settle: each of them is a document with an owner and a history, and
each of them already lives in a file. Written inline, the copy that reaches a
run is the one pasted into site configuration, so every edit to the instruction
is a second edit somewhere else and a missed one is silent — the ticket carries
last month's words with nothing in its output to say so.

So a recipe may name that file instead. `brief_file` is a **path template**,
rendered from the vocabulary a work root is rendered from
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)):
the item's own fields, the resolved checkout, the project root, and the two
names that reach above it. `{reply}` is not among them — it is a place ephor
writes to rather than a fact about the matter, and a path cannot be one
([§FS-005-dispatch.13](FS-005-dispatch.md#13-a-communication-is-work-too-and-its-answer-comes-back-as-a-proposal)).
A relative path is relative to the directory holding the configuration file
that wrote it, with `~` and `$VAR` expanded first, and never to the working
directory: a recipe that sweeps on its own rhythm
([§FS-005-dispatch.32](FS-005-dispatch.md#32-a-recipe-can-ask-for-its-own-sweep-and-say-how-often)) runs from
wherever the unit that called ephor happened to stand, and a brief that
depended on that would be a different brief on a timer than under a person.

**The file is read when the ticket is written, and its text is the brief.**
Not named for the run to open: that is
[§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)'s rule and it
applies here without amendment — a ticket saying *read the instruction at this
path* has handed back the opening move, and it hands back more than a dossier
would, because the file may be somewhere the work has no checkout of. Reading
it here is also what makes the ticket a record of what was asked for rather
than a pointer at whatever that path holds later.

**Placeholders are rendered in the path and in `brief`, never inside the
file.** A version-controlled document is not a template, and braces in it are
the characters they are: an instruction whose own example names `{title}`
reaches the ticket as those seven characters. Nothing in the file is
substituted, which is what lets the file be written by somebody who has never
heard of ephor.

**A rendered path with no readable file behind it refuses, naming the path,
before anything is written** — no workspace, no work root, no plan
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)).
This is the one thing the key adds to the boundary that section draws between
a path and prose: a brief *named by* a path is under the path rule for its
path and the prose rule for its text.

**Where the path may point is the site's to say, and there is no refusal for
pointing it anywhere.** A template resolving inside a watched checkout is
permitted, and a repository's own words may be what a recipe spends on, because
the person who wrote the template is the person who pays — no different in kind
from a `root` that resolves into a checkout
([§DF-001-manifest-offered.2](../decisions/functional/DF-001-manifest-offered.md#2-recipes-are-excluded)).
What keeps that true is a bound on ephor rather than on the site: **no recipe
ephor ships may default `brief_file` to a well-known in-repository filename**,
because a project that gained a voice in what is asked for merely by containing
a file would be an artifact required of it
([§REQ-001-boundary.3](../requirements/REQ-001-boundary.md#3-requirements-on-a-project-are-capabilities-never-artifacts)).
A repository is heard here only where a site pointed at it.

### 34.1 Both keys compose, and neither is refused where the file loads

A recipe may write `brief`, `brief_file`, or both. **Both is not an error**, and
the two are not two spellings of one fact — the drift
[§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can) refuses is
one key that means what another already means, and these mean different things.
The file says how work is done here; `brief` still says what to do with this
matter, with its `{title}` and its `{url}` in it. So they compose, in a fixed
order: the file's text first, the rendered `brief` after it, and whatever a
deterministic opening move reached last of all, because that is what this run
found ([§FS-005-dispatch.12](FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)).

**Neither is refused where the configuration loads**, naming the recipe, rather
than at the dispatch that would have used it. A recipe can run from a timer
with nobody watching ([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)), so a
dispatch-time refusal lands in a log, while a load-time one stops the next
reading of anything in front of the person who has just edited the file. This
is the same reason the machine a recipe starts in is vetted before a ticket is
written rather than after ([§FS-005-dispatch.6](FS-005-dispatch.md#6-dispatch-is-offered-where-it-would-work-and-refuses-where-it-would-not)).

### 34.2 Which text a ticket was given is recorded on the ticket

A brief read from a file is the one part of a ticket whose source can change
without the matter changing. So each ticket records the rendered path it read
and a hash of the bytes as read, in that ticket's own structured metadata —
the one thing there that identifies the ask rather than the item
([§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)). A reader
holding the file can say whether the ticket got these words or older ones, and
a program in the state machine can too.

**Per ticket, and never in the dossier.** The dossier is rewritten every time
the matter reopens ([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)), and a hash
written there would say what the *latest* dispatch read while sitting above
tickets that were given something else. A hash naming another ticket's text is
worse than no hash at all. Where a plan holds several tickets, each keeps its
own and none is corrected by a later one.

### 34.3 Every writer of a brief reads the file, the sweep included

The key belongs to the recipe rather than to one caller, so every path that
turns a recipe into words honours it. The previews
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project))
show what the hand-over would actually carry, and where the file cannot be read
they **fall back** to whatever can be rendered, as they already do for a
placement that cannot be resolved: a menu row is a row, and a refusal in the
slot where the words go is worse than words that are out of date. A dry run is
not a preview in that sense and does not fall back — it promises what the real
dispatch would do, and a dry run that promises a ticket the real dispatch would
refuse is the most misleading promise of the set
([§FS-005-dispatch.6](FS-005-dispatch.md#6-dispatch-is-offered-where-it-would-work-and-refuses-where-it-would-not)).

**The unattended sweep reads it too**, and it is the caller this point exists
for. That sweep has a checkout and no matter
([§FS-005-dispatch.3](FS-005-dispatch.md#3-one-rhei-per-item-one-ticket-per-dispatch)), so the path renders from
the names a checkout can answer, and one it cannot — a `{title}` where there is
no item, or an `{id_slug}` where there is no id to slug — is refused **by name**,
as any other unanswerable name in a path is
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)). That
`{id_slug}` is never withheld from a matter
([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)) does
not reach here: this sweep has no matter, so the field has nothing to render from
rather than an empty value to render.
Where the file cannot be read, the work is still reported and no ticket is
opened: the conflict stays in the report with the reason on its own row, which
is the shape that path already takes for everything it cannot open a ticket
about ([§FS-004-quick-actions.6.1](FS-004-quick-actions.md#61-the-same-replay-over-every-checkout-nobody-is-holding)).
A conflict that went unreported because an instruction was missing is the
failure this rules out.

**Nothing already written changes.** `brief_file` is absent from every existing
configuration, `brief` alone keeps its whole meaning, and no configured value
is read differently. One error appears where there was none — a recipe with
neither key — and nobody reaches it without first editing the configuration
that had one.

## 35. What a ledger entry may be forgotten for is read from the plans

A matter's work is **every plan the record says is that matter's** — the one
ephor wrote itself and every one a workflow laid beside it
([§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here),
[§FS-005-dispatch.30](FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work)).
Which entries `work forget --done` drops is read from those plans, and never
from which of them ephor wrote: an entry whose laid plan holds a task that is
not final is not over, and is not dropped.

This is [§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)
applied to who may be forgotten. The work's state belongs to the runtime and is
read from the plan; a reading that reports no open tickets while the plan on
disk says otherwise is the watch reporting on itself instead of on the world,
and it is the reading that is at fault rather than the ledger. A count that
cannot see a workflow's plan is that reading, whatever the ledger holds.

**No sweep takes an entry that still has something open.** `--done` and
`--missing` are both read from the plans and from every one of them
([§FS-005-dispatch.30](FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work)),
so a single task that is not final anywhere in a matter's work keeps that
matter's entry in the ledger, whichever verb swept and whatever the rest of its
plans say. Only `--item`, which names one entry whatever its plans say, reaches
past this sentence. What follows says which sweep an entry is *for*, and is
read against it.

**A plan that cannot be read is not evidence the work is over.** A laid workflow
plan that is absent or unreadable makes its entry report as **missing** — the
row says so, `--missing` is the sweep it is for, and `--item` reaches it — and
`--done` leaves it alone. A deleted *recipe* plan keeps the rule
[§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work) gives
it: it counts to `--done` as a finished plan rather than as an unreadable one,
so an entry that has nothing else open is still selected. The asymmetry is
deliberate: ephor wrote the recipe plan, so its absence is ephor's own record of
something gone, while a plan ephor only asked a runtime to write is a file it
can conclude nothing from. An entry is never dropped for a file nobody promised.
Where both stand in one matter — a recipe plan ephor lost, and beside it a laid
plan whose task is going — neither sweep takes the entry, because the sentence
above governs both and something is open.

**The same reading answers every surface**, because a matter has one body of
work and not one per command
([§FS-005-dispatch.30](FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work)). So
`work list --open` shows a matter whose laid workflow plan is still going, and
the badge on its row says what that plan's task is doing rather than that a
workflow exists at all — a workflow stopped at a question was invisible before
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place),
[§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).
It reaches the hand a run is started with too
([§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)): a laid
task takes part in that resolution exactly as a recipe ticket does, and a task
that names its own target is still its own authority.

**So a matter no recipe applies to is not thereby dormant.**
[§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work) keeps its words for the case it is
about — it merged, it closed, nothing is open — including the
`work forget --done` it offers there. Where the plans say something is still
open, the report says that instead, names the plan and the task it is at, and
offers nothing to clear; the machine-readable report calls that outcome
**underway** rather than dormant. Where a plan the record names cannot be read
at all, the report says what it does not know — what that work came to — and
names `--missing`, the verb that does reach the entry; that outcome is
**unread**. Only where the plans really are over is it **dormant**, and only
there is `--done` offered. The discriminator is the plans and never
whether a dispatch was a workflow, so a *recipe* entry with open tickets stops
being offered the same wrong hint by the same sentence. What is not fixed here
is that such a report is made again on every turn: no snapshot is acknowledged
where nothing is written, so the words change and their repetition does not.
