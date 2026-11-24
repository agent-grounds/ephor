# FS-019-waiting: ephor opens on what waits on the reader, and every matter is one key away

The feed is the complete answer to "what is there": every matter, sorted into
its categories ([§FS-003-feed-categories.1](FS-003-feed-categories.md#1-the-categories)). As the first screen it is too much
answer. The question a person opens ephor with is "what needs me now"
([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now)), and the feed answers it only after the reader has read
past everything that does not.

So the interface opens on the **Waiting screen**: the short list of what waits
on the reader, what nothing has been done about yet, and what is running. The
feed stays exactly as it is, as the **overview**, one key away from anywhere.
Nothing is hidden, only not upfront, and the screen says how much the
overview holds so that a quiet Waiting screen is never mistaken for a quiet
feed ([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)).

The screen is presentation over one reading, and `ephor waiting` prints the
same reading, so the screen and the command cannot describe one fact two ways
([§REQ-002-parity.2](../requirements/REQ-002-parity.md#2-parity-runs-both-ways), [§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).

## 1. Three sections, in this order

1. **Waiting on you**: every gate, and every poll that waits on the reader.
2. **New**: every matter nothing has been done about.
3. **Running**: every live run.

The order is the order of what costs the reader most to leave: a gate holds
work nobody else will move ([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)), a new matter is work nobody
has started, and a run is work going on without them. Each section is headed
by its name and its count.

### 1.1 Waiting on you: every gate, and every poll on the reader

The rows come from the enumeration the operations board reads
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)): every execution root found by looking, and every plan
in each, plans ephor never dispatched included. There is no second scanner.
A ticket whose current state is one the runtime will not leave on its own is
a **gate row**, under every root, live or not, because the usual end of the
run that parked a ticket is that run exiting.

A gate row carries the ticket's own title, the heading the ticket has in its
plan, and not the plan's title, which is the same for every ticket in it. It
also carries the state the ticket is parked in, the ticket's id, the project,
and the matter the plan is about, where the ledger names one
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)) and the feed still carries it.

How long a gate has waited and what it holds back are carried only where the
bound runtime reports them. A runtime that reports neither leaves both off the
row and out of the reading. No file timestamp stands in for when a ticket
began waiting: a plan file is written for many reasons, and a guess at how
long a person has kept work waiting is a claim the reading cannot back. Rows
are listed oldest first where the runtime says when each began waiting, and
otherwise in the enumeration's own order: project, root, then plan order.

A poll that declares whose answer it awaits is a person wait
([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)). It is a row here only when the person it names is the
reader:

- `you` or `owner`, which name whoever the work is being done for, and on
  this machine that is the reader; or
- the reader's own role on the matter: `author` on a matter the feed says the
  reader wrote, `reviewer` on one the feed says the reader is reviewing.

A poll that names anyone or anything else, such as an author who is somebody
else, the forge, or a release tag, waits on the world rather than on the
reader. It stays where it already is: on the matter's own row and on the
board. A poll row says `waiting on <declared person>` and `not yours to move
here`. The poll moves itself the moment its answer lands, so the row offers no
destination and no transition.

### 1.2 New: every matter nothing has been done about

A matter is **new** when all of these hold:

- the feed shows it ([§FS-003-feed-categories.2](FS-003-feed-categories.md#2-recent));
- it is not finished;
- the ledger records no work about it ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work));
- it is not itself a ticket of the bound runtime's own task store, which is
  work already; and
- it is not held.

A matter a recipe dispatched on its own is therefore never new: its dispatch
is in the ledger. Rows are listed newest first and carry the matter's kind,
project, title and how long ago it last moved.

A matter that carries a label some selector of the site excludes by name, such
as `"labels": ["!human-discussion"]` on an action or a recipe
([§FS-005-dispatch.31](FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)), is **held**. It is listed apart from New, once, with
the label that holds it, and it is offered nothing. Ephor names no label
itself: which labels hold a matter is what the site's own selectors say, and
a site whose selectors exclude no label holds nothing.

### 1.3 Running: every live run

One row per execution root whose run is live, read from the runtime's lock
exactly as the board reads it ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). The row carries the
project, the matter where the ledger names one, the root, the tickets the run
is holding with the state each is in, and how long the run has been going.
The age is counted from the start the run published beside its lock
([§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)), so a run that published no start shows no age. The
hand a run works with is carried only where the runtime reports it, by the
same rule as a gate's wait ([§FS-019-waiting.1.1](FS-019-waiting.md#11-waiting-on-you-every-gate-and-every-poll-on-the-reader)).

## 2. An empty section is one line

A section with no rows collapses to its heading and one line saying that it
is empty. When neither Waiting on you nor New has a row, the screen opens with
`Nothing is waiting on you.` and still shows Running, the held line where a
matter is held, and the overview line ([§FS-019-waiting.3](FS-019-waiting.md#3-the-header-and-the-overview-line)). An empty screen is
an answer, not a failure, and it says what it is the answer to.

Where the runtime cannot run anything at all, the workable rung's own sentence
is shown in place of the empty Waiting on you and Running sections, exactly as
the board shows it ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).

## 3. The header and the overview line

The header names the screen and counts the three sections:
`ephor — waiting   3 on you · 5 new · 4 running`.

The footer opens with one line about the overview:
`overview: 143 matters · 12 changed since you looked`. The first number is how
many matters the overview would show, and the second is how many of them are
unread. Both are counted from the same feed the overview draws, so the two
screens never disagree about how much there is.

## 4. Keys

One cursor runs over the rows of all three sections, top to bottom, with the
motion keys every screen has.

- `Enter` on a gate row opens the review of that gate ([§FS-005-dispatch.9.1](FS-005-dispatch.md#91-a-human-gate-is-reviewed-and-moved-where-its-matter-is-read)),
  the same screen `H` opens from the matter. A gate whose plan no matter on
  the feed is about cannot be reviewed by matter, so the row says so rather
  than doing nothing, and `e` reads the plan.
- `Enter` on a poll row opens the matter's conversation, or the plan where
  the feed carries no matter.
- `Enter` on a new row opens the matter, as `Enter` on the overview does.
- `Enter` on a running row attaches to the run ([§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)). A run
  that left no id beside its lock says so rather than doing nothing.
- `e` reads the plan behind a gate, poll or running row.
- `O` opens the overview from any screen. On the overview, `O` comes back to
  Waiting.
- `;` and `$` open the operations board and the burn page over the screen, as
  they do everywhere ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place), [§FS-013-burn.8](FS-013-burn.md#8-the-page-and-its-command)).
- `r` refreshes, and `q` leaves.

`esc` from a screen opened from Waiting comes back to Waiting, and `esc` from
one opened from the overview comes back to the overview. The screen `esc`
returns to is whichever of the two the reader was last on, so reviewing a gate
from Waiting never drops the reader into the feed they did not open.

The Waiting screen is the floor: `esc` on it does nothing, the way it does on
the overview.

## 5. The start screen is the reader's choice

`defaults.start_screen` in `status.json` names the screen `ephor tui` opens
on: `"waiting"`, the default, or `"overview"`. Any other value is a
configuration error, refused at load like every other unknown setting.

`ephor tui --overview` opens on the overview for one launch, whatever the
setting says.

## 6. `ephor waiting` is the same reading

`ephor waiting` prints the three sections, the held line and the overview
line, in the screen's order. `ephor waiting --json` prints the published
`waiting` view (`ephor schema views`):

- `on_you`: the gate and poll rows of [§FS-019-waiting.1.1](FS-019-waiting.md#11-waiting-on-you-every-gate-and-every-poll-on-the-reader), each with its
  `kind` (`gate` or `poll`);
- `new`: the rows of [§FS-019-waiting.1.2](FS-019-waiting.md#12-new-every-matter-nothing-has-been-done-about);
- `held`: each held matter with the label that holds it;
- `running`: the rows of [§FS-019-waiting.1.3](FS-019-waiting.md#13-running-every-live-run);
- `overview`: the two counts of [§FS-019-waiting.3](FS-019-waiting.md#3-the-header-and-the-overview-line); and
- `refusal`: the workable rung's sentence, where the runtime cannot run
  anything.

A field the runtime does not report is absent from the view rather than null
or guessed ([§FS-019-waiting.1.1](FS-019-waiting.md#11-waiting-on-you-every-gate-and-every-poll-on-the-reader)). A later field is added freely, as the view
schema promises.

The command reads the cache and the disk, like the screen. It fetches
nothing, because `refresh` remains the only verb that asks the world
([§FS-001-forge-interface.7](FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)), and it writes nothing. Like the operations board
it answers for the whole site, so the scope selectors are refused rather than
quietly ignored ([§FS-011-command-line.9](FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)).

`O` is the key both commands carry: `ephor waiting` is the Waiting screen,
and `ephor feed` is the overview ([§REQ-002-parity.5](../requirements/REQ-002-parity.md#5-the-parity-list-is-checked-not-remembered)).
