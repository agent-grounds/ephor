# FS-019-waiting: ephor opens on what waits on the reader, and every matter is one key away

The feed is the complete answer to "what is there": every matter, sorted into
its categories ([§FS-003-feed-categories.1](FS-003-feed-categories.md#1-the-categories)). As the first screen it is too much
answer. The question a person opens ephor with is "what needs me now"
([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now)), and the feed answers it only after the reader has read
past everything that does not.

So the interface opens on the **Waiting screen**: the short list of what waits
on the reader, what nothing has been done about yet, and what is running. A
matter nothing has been done about is decided about where it is shown: each
one carries the rhei ephor would assign it, and one key lays that rhei down and
starts it ([§FS-019-waiting.7](FS-019-waiting.md#7-a-new-matter-is-processed-where-it-is-shown)). The
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
3. **Running**: every live run, and the work processing laid that waits for a
   slot.

The order is the order of what costs the reader most to leave: a gate holds
work nobody else will move ([§FS-005-dispatch.9](FS-005-dispatch.md#9-work-that-stops-for-a-person-says-so-where-the-person-is-looking)), a new matter is work nobody
has started, and a run is work going on without them. A paused run is in the
last place too: what it waits on from the reader is already a row of the
first, and what else it waits on is the world's to answer
([§FS-019-waiting.1.3.1](FS-019-waiting.md#131-a-run-with-nothing-it-can-do-says-it-is-paused-what-it-waits-on-and-when-it-next-looks)). Each section is headed by its name and its count;
Running names its paused runs beside it, with their count.

### 1.1 Waiting on you: every gate, and every poll on the reader

The rows come from the enumeration the operations board reads
([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)): every execution root found by looking, and every plan
in each, plans ephor never dispatched included. There is no second scanner.
A ticket whose current state is one the runtime will not leave on its own is
a **gate row**, under every root, live or not, because the usual end of the
run that parked a ticket is that run exiting.

A gate row carries the ticket's own title, the heading the ticket has in its
plan, and not the plan's title, which is the same for every ticket in it;
where the bound runtime lists its gates, it carries the title the gate's own
brief gives instead ([§FS-019-waiting.1.1.1](FS-019-waiting.md#111-a-runtime-that-lists-its-gates-names-each-one-says-how-long-it-has-waited-and-what-it-holds)). It
also carries the state the ticket is parked in, the ticket's id, the project,
and the matter the plan is about, where the ledger names one
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)) and the feed still carries it.

How long a gate has waited and what it holds back are carried only where the
bound runtime reports them, which is what its listing of gates does
([§FS-019-waiting.1.1.1](FS-019-waiting.md#111-a-runtime-that-lists-its-gates-names-each-one-says-how-long-it-has-waited-and-what-it-holds)). A runtime that reports neither leaves both off the
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
reader. It is not a row here. It stays where it already is: on the matter's
own row and on the board, and, where the run it belongs to is paused, in what
that run's row under Running says it waits on ([§FS-019-waiting.1.3.1](FS-019-waiting.md#131-a-run-with-nothing-it-can-do-says-it-is-paused-what-it-waits-on-and-when-it-next-looks)). A poll row says `waiting on <declared person>` and `not yours to move
here`. The poll moves itself the moment its answer lands, so the row offers no
destination and no transition.

#### 1.1.1 A runtime that lists its gates names each one, says how long it has waited, and what it holds

Where the bound runtime prepares gate briefs ([§FS-005-dispatch.9.3](FS-005-dispatch.md#93-a-prepared-gate-is-answered-in-its-own-form)), it also
lists the gates under a work root, and the reading asks it for that listing
once per work root that holds a gate row: never once per row, and never for a
root that holds none. The listing answers the three things the enumeration
cannot know:

- the gate's **title**, the one its brief gives, which the row carries in
  place of the ticket's heading;
- **since**, when the ticket began waiting, as the runtime recorded it, or that
  the runtime does not know; and
- **holds**, how many unfinished tasks cannot move until the ticket leaves the
  gate.

The listing adds no row and takes none away. The rows are still the
enumeration's: a gate the listing names that the enumeration did not find is
not a row, and a row the listing does not name, names in another state, or
belongs to a root whose listing could not be had keeps exactly what the
enumeration gave it, its title included, and carries neither fact.

A gate whose start the runtime does not know has waited an *unknown* time,
never no time, and it sorts after every gate whose start is known. Rows whose
start is known come first, oldest first; every other row, polls included,
keeps the enumeration's order behind them. On the row the facts follow the
state, in the one sentence both surfaces print: `human-judgment · waited 2d ·
holds 6 tasks`, and `waited unknown` where the runtime does not know.

A runtime that prepares no briefs, or none bound, is not asked, and every row
is exactly what the enumeration gives ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)).

### 1.2 New: every matter nothing has been done about

A matter is **new** when all of these hold:

- the feed shows it ([§FS-003-feed-categories.2](FS-003-feed-categories.md#2-recent));
- it is not finished;
- it is not processed: the ledger records no work about it
  ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), and no dismissal of it still stands
  ([§FS-019-waiting.7.4](FS-019-waiting.md#74-x-records-that-a-matter-needs-no-rhei));
- it is not itself a ticket of the bound runtime's own task store, which is
  work already; and
- it is not held.

A matter a recipe dispatched on its own is therefore never new: its dispatch
is in the ledger. A monitor's matter still down, or still unknown, after the
work laid on it ended is the one exception: it is new again, though the ledger
records that work, and processing it adds to the work already recorded
([§FS-021-monitors.15](FS-021-monitors.md#15-work-ended-still-down)). Rows are listed newest first and carry the matter's kind,
project, title, how long ago it last moved, and the proposal: the rhei ephor
would assign it ([§FS-019-waiting.7.1](FS-019-waiting.md#71-every-new-row-carries-a-proposal)). On the screen the proposal reads after
the title as the entry, the hand the work would go to and the branch its
checkout is on — `→ implement  cld  fix/issue-431` — each where the proposal
names it, so a row says who would do the work and where before it is opened.
Where a row is wider than the screen, the title is what is cut, with `…`,
never the proposal after it; a run's row on Running is cut the same way, so
its work and its tickets stay in sight.

A matter that carries a label some selector of the site excludes by name, such
as `"labels": ["!human-discussion"]` on an action or a recipe
([§FS-005-dispatch.31](FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)), is **held**. It is listed apart from New, once, with
the label that holds it, and it is offered nothing. Ephor names no label
itself: which labels hold a matter is what the site's own selectors say, and
a site whose selectors exclude no label holds nothing.

On the screen the held matters are one line, under New. While there are three
or fewer it names each, with its label; past that it counts them by the label
that holds them — `25 matters are held, and offered nothing: 25 by
human-discussion` — because a line of titles is cut by the edge of the screen
long before it says anything. `ephor waiting` lists each held matter either
way ([§FS-019-waiting.6](FS-019-waiting.md#6-ephor-waiting-is-the-same-reading)).

### 1.3 Running: every live run

One row per execution root whose run is live, read from the runtime's lock
exactly as the board reads it ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). The row carries the
project, the matter where the ledger names one, the root, the entry the ledger
says the work was handed over with and the branch it is about
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), the tickets the run is holding with the state each is in,
and how long the run has been going. A run between two turns holds no ticket,
and its row says `between turns` rather than showing an empty list that reads
as a run doing nothing. A run that holds no ticket because nothing is left that
it can do until a person or the world answers is not between turns: it is
**paused**, and its row says what it waits on and when it next looks
([§FS-019-waiting.1.3.1](FS-019-waiting.md#131-a-run-with-nothing-it-can-do-says-it-is-paused-what-it-waits-on-and-when-it-next-looks)).
The age is counted from the start the run published beside its lock
([§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)), so a run that published no start shows no age. The
hand a run works with is carried only where the runtime reports it, by the
same rule as a gate's wait ([§FS-019-waiting.1.1](FS-019-waiting.md#11-waiting-on-you-every-gate-and-every-poll-on-the-reader)).

Under the live runs, one line names the work processing laid and asked to
start that has not started yet, and why
([§FS-019-waiting.7.6](FS-019-waiting.md#76-running-names-what-waits-for-a-slot)).

#### 1.3.1 A run with nothing it can do says it is paused, what it waits on and when it next looks

A run with nothing left it can do until a person or the world answers keeps
its lock and its process for as long as the wait lasts: a gate on the reader,
a poll on a release tag asked again every half hour. Read as running, or as
between turns, it tells the reader that work is going on without them, when it
is waiting on them or on the world ([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now)). So such a run is
called **paused**. The word is taken from the runtime's facts alone, once,
below the screen, the board and `ephor waiting`, so that all three say it of
the same runs ([§REQ-002-parity.2](../requirements/REQ-002-parity.md#2-parity-runs-both-ways)). A live run is paused when all four hold:

1. its lock is held: it is live, exactly as the board reads it
   ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place));
2. it holds no agent slot: its root is free under [§FS-005-dispatch.24.4](FS-005-dispatch.md#244-a-live-root-holds-an-agent-slot-while-something-in-it-runs-or-is-about-to), the same verdict the
   slot count takes. No open ticket is witnessed held by the run, as its own
   stream or journal says ([§FS-005-dispatch.15.2](FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream)); no open ticket is on a poll of one
   minute or less, or on one whose interval cannot be read, which that rule
   calls about to run; and every plan in the root and its machine could be
   read;
3. no task of its root is ready: no open ticket is on a poll whose recorded
   next attempt has come, and the bound runner's own ready set for the root
   is empty. That set is the runtime's rule, with prerequisites, required
   inputs, poll deadlines and supervision in it, and it is asked of the runner
   rather than worked out again here;
4. at least one open ticket of the root is **waiting**, in the sense the slot
   rule opens with: at a gate, in a state its machine calls gating, or between
   a poll's attempts, in a poll state whose next attempt the runtime recorded
   and which is still ahead.

The fourth asks for one waiting ticket, not for every open ticket to wait.
The steps a workflow has not reached yet are held back by the step before
them, and the ticket that supervises them sits in a state of its own; the
ready set is what says that none of those can move, so they neither stop a
run from being paused nor are named as what it waits on.

Every paused run is therefore free under [§FS-005-dispatch.24.4](FS-005-dispatch.md#244-a-live-root-holds-an-agent-slot-while-something-in-it-runs-or-is-about-to), parked or polling as that
rule tells them apart, so the slot count and this row never disagree about
whether a run is about to work: the one-minute line is that rule's, and it is
not drawn a second time here. The converse does not hold. The slot rule does
not ask the runner what is ready, so a free root whose runner names a ready
task is a run about to take it up, and it reads between turns.

No state name is guessed, and no length of silence counts: the quiet badge
stays a badge ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)). Where any of the four fails, the row reads as it did. A
ready task, or a poll whose next attempt has come, is a run about to take a
turn, and it says `between turns`. A runner that cannot be asked for its ready
set, or gives an answer that does not read, is not a runner that said nothing
is ready, and those rows say `between turns` too, because paused is a positive
finding, never the absence of one. A poll with no recorded next attempt is not
known to be waiting, so it is not counted as waiting, and whether it is about
to run is the ready set's to say.

A reading is drawn some time after it is taken: the runner's answer takes a
moment, a command may take a minute to read everything else, and the screen
redraws a reading until something on disk changes. A next look that comes in
between is a poll whose next attempt has come, so the run is measured again
once its runner has answered, and a row drawn after its next look has come
reads `between turns` and is counted running, as a fresh reading would read it
until the runtime records the attempt after. No row says a next look that has
passed.

The row says `paused`, then each waiting ticket, the gates first. A gate is
`waiting on you (<task>)`. A poll that declares whose answer it awaits is
`waiting on <who> (<task>)`, in the machine's own words, and a poll that
declares nobody is `polling in <state> (<task>)`. `<task>` is the ticket's own
name, the last part of its id. Then comes `next look` and the earliest next
attempt among the polls: `in 45s` under a minute, `in 4m` under an hour, the
local clock time `23:13` later the same day, and the date with it,
`Nov 28 09:13`, on another day. A run waiting only on gates has no next look,
because nothing but a person moves it. The row is marked `⏸` where a running
row is marked `▶`, and the rest of it is a running row's: the age, the
project, the matter, the entry and the branch.

```text
⏸ 2h    grund      Read Markdown without v1_* definitions   implement  fix/issue-1384   paused · waiting on a release tag (ship) · next look 23:13
⏸ 6h    grund      Spell the lock in one place   implement  fix/issue-1460   paused · waiting on prerequisite tickets (dependencies) · next look in 4m
⏸ 3d    rhei       Count visits per state   implement  fix/issue-760   paused · waiting on you (specify)
```

`ephor waiting` prints the same sentence under the row, where a run between
turns prints `between turns`.

Paused rows stay in Running, after the runs that are taking turns, and the
section's heading names both, each with its count, the two adding up to the
section's rows: `RUNNING · 3 · PAUSED · 3` on the screen, `Running (3) ·
Paused (3)` from the command, and `RUNNING · 0 · PAUSED · 6` when every live
run is paused. With no paused run the heading is `RUNNING · 6`, as before. A
paused run is still a live run. It holds its root's lock and its process,
`Enter` attaches to it, and `e` reads its plan, exactly as on any other running
row ([§FS-019-waiting.4](FS-019-waiting.md#4-keys)). A section of its own would split one list of live runs
in two and add a fourth place to the screen's order, which is ordered by what
costs the reader most to leave ([§FS-019-waiting.1](FS-019-waiting.md#1-three-sections-in-this-order)). A paused run costs the
reader nothing of its own: what it asks of them, a gate or a poll on them, is
already a row in Waiting on you, and what else it waits on is the world's to
answer. So the row changes what it says, and nothing about where it stands or
what its keys do.

## 2. An empty section is one line

A section with no rows collapses to its heading and one line saying that it
is empty. When neither Waiting on you nor New has a row, the screen opens with
`Nothing is waiting on you. No gate is open and every matter has work laid or a
recorded decision.` — `every matter not held` where a matter is held — and
still shows Running, the held line where a matter is held, and the overview
line ([§FS-019-waiting.3](FS-019-waiting.md#3-the-header-and-the-overview-line)). An empty screen is an answer, not a failure, and it says
what it is the answer to.

Under that answer, one line says what the reader decided today:
`decided today: 5 laid · 1 needs no work`. Both are counted from ephor's own
records of processing ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), the processing marks and the
dismissals made since local midnight. An answered gate is the runtime's record
and not ephor's, so it is not counted here.

So the screen never shows that answer before it has one. Reading the work
roots takes as long as the operations board takes, and the interface draws
its first frame before that read rather than after it: until the first
reading lands the screen says it is reading, with no section under it, and
neither `Nothing is waiting on you.` nor an empty section. Before the screen
has anything to read from at all, the interface says what it is reading in
its place ([§FS-019-waiting.5.1](FS-019-waiting.md#51-the-interface-is-on-the-screen-before-it-reads-anything)).

Where the runtime cannot run anything at all, the workable rung's own sentence
is shown in place of the empty Waiting on you and Running sections, exactly as
the board shows it ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place)).

## 3. The header and the overview line

The header names the screen, counts the three sections, and says how old the
reading's feed is:
`ephor — waiting   3 on you · 5 new · 3 running · 3 paused · refreshed 2m ago`.
`running` counts the live runs taking turns and `paused` the live runs that are
paused ([§FS-019-waiting.1.3.1](FS-019-waiting.md#131-a-run-with-nothing-it-can-do-says-it-is-paused-what-it-waits-on-and-when-it-next-looks)), so the header never calls a run that waits
on a release tag a run at work. With no paused run the header says no
`· 0 paused`. The age is
the newest fetch the feed's cache records ([§FS-001-forge-interface.7](FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)), so
a quiet screen over a feed nobody has refreshed says so. Under a minute it
says `refreshed just now`, and a feed never fetched says `never refreshed`.

The footer opens with one line about the overview:
`overview: 143 matters · 12 changed since you looked`. The first number is how
many matters the overview would show, and the second is how many of them are
unread. Both are counted from the same feed the overview draws, so the two
screens never disagree about how much there is.

Where a watched project has a status, a monitor something judges, the line
goes on to count the statuses, `monitors: 1 down · 2 unknown · 1 overdue`, so
a quiet screen still says when the watch itself has degraded
([§FS-021-monitors.20.1](FS-021-monitors.md#201-waitings-overview-line-names-the-monitors)). On the screen the footer adds `M opens them`;
`ephor waiting` prints the line without it.

## 4. Keys

One cursor runs over the rows of all three sections, top to bottom, with the
motion keys every screen has.

- `Enter` on a gate row opens the review of that gate ([§FS-005-dispatch.9.1](FS-005-dispatch.md#91-a-human-gate-is-reviewed-and-moved-where-its-matter-is-read)),
  the same screen `H` opens from the matter. Where the bound runtime prepares
  gate briefs, it opens that gate's own brief and form instead
  ([§FS-005-dispatch.9.3](FS-005-dispatch.md#93-a-prepared-gate-is-answered-in-its-own-form)), and `esc` there comes back to the row. A gate
  whose plan no matter on the feed is about cannot be reviewed by matter, so
  the row says so rather than doing nothing, and `e` reads the plan.
- `Enter` on a poll row opens the matter's conversation, or the plan where
  the feed carries no matter.
- `Enter` on a new row opens the processing screen for its proposal
  ([§FS-019-waiting.7.2](FS-019-waiting.md#72-confirming-lays-it-records-it-and-starts-it)); `l` opens the matter, as `Enter` on the overview
  does.
- `a` on a new row chooses another rhei for it ([§FS-019-waiting.7.3](FS-019-waiting.md#73-a-chooses-another)), and
  `x` records that it needs none ([§FS-019-waiting.7.4](FS-019-waiting.md#74-x-records-that-a-matter-needs-no-rhei)).
- `A` processes every new row whose proposal is unambiguous, after one
  preview ([§FS-019-waiting.7.5](FS-019-waiting.md#75-a-processes-every-unambiguous-row-after-one-preview)).
- `Enter` on a running row attaches to the run ([§FS-005-dispatch.20](FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching)). A run
  that left no id beside its lock says so rather than doing nothing.
- `e` reads the plan behind a gate, poll or running row.
- `O` opens the overview from any screen. On the overview, `O` comes back to
  Waiting.
- `;` and `$` open the operations board and the burn page over the screen, as
  they do everywhere ([§FS-005-dispatch.15](FS-005-dispatch.md#15-every-operation-is-visible-in-one-place), [§FS-013-burn.8](FS-013-burn.md#8-the-page-and-its-command)).
- `M` opens the Monitors page, as it does everywhere ([§FS-021-monitors.18.1](FS-021-monitors.md#181-m-opens-it-from-any-screen)).
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

### 5.1 The interface is on the screen before it reads anything

`ephor tui` enters the terminal and draws its first frame before it opens the
session its screens read: the registry, every cached feed, where each
checked-out branch stands, what each project can do, and the work handed over.
On a site that watches many projects that reading takes seconds, and much
longer when the disk has not read those files lately, and a terminal left
blank for that long cannot be told from a program that hung — the failure
[§FS-001-forge-interface.7](FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it) names for a fetch, arriving before there is a screen
to keep live. So until the session has loaded, the screen shows one line under
its header naming what it is reading now, in ephor's words and with a count
wherever it has one:

- `Starting…`, before the first of these has begun
- `Reading the registry…`
- `Reading the cached feeds of 15 projects…`
- `Reading the branches of 29 checkouts — 12 done…`, the count rising as each
  checkout is measured
- `Reading what 15 projects can do…`
- `Reading the work handed over…`, `Reading the jobs ephor started…` and
  `Reading the work roots…`

A line wider than the screen goes on to the next line rather than being cut,
so the count, the one part that moves, is never the part lost. A step too
quick to be drawn is not waited for. Nothing a reading brings is drawn before
that reading has landed: no section, no count of matters, and no `No cached
data` message. Once the session has loaded the start screen opens, and the
Waiting screen goes on to say its own `Reading what waits on you…` while it
walks the work roots ([§FS-019-waiting.2](FS-019-waiting.md#2-an-empty-section-is-one-line)).

While the session loads, `q` and `ctrl-c` quit. Either leaves the terminal as
it found it, off the screen ephor drew on and with its cursor shown, and ends
ephor with success, as `q` does on every screen. No other key does anything
yet, because nothing is on the screen to act on.

Saying what it reads changes nothing about the reading: the session it opens
is the one `ephor waiting` opens, the same files and the same measurements in
the same order ([§FS-019-waiting.6](FS-019-waiting.md#6-ephor-waiting-is-the-same-reading)). The configuration is read first, before the terminal is
entered, so a setting it refuses is said on the plain terminal like every
other command's. A load that fails leaves the terminal as it found it, then
says why in the sentence and with the exit code `ephor waiting` would.

## 6. `ephor waiting` is the same reading

`ephor waiting` prints the three sections, the held line and the overview
line, in the screen's order. `ephor waiting --json` prints the published
`waiting` view (`ephor schema views`):

- `on_you`: the gate and poll rows of [§FS-019-waiting.1.1](FS-019-waiting.md#11-waiting-on-you-every-gate-and-every-poll-on-the-reader), each with its
  `kind` (`gate` or `poll`), and on a gate the runtime lists, its `since` and
  `holds` ([§FS-019-waiting.1.1.1](FS-019-waiting.md#111-a-runtime-that-lists-its-gates-names-each-one-says-how-long-it-has-waited-and-what-it-holds));
- `new`: the rows of [§FS-019-waiting.1.2](FS-019-waiting.md#12-new-every-matter-nothing-has-been-done-about), each with its `proposal`
  ([§FS-019-waiting.7.1](FS-019-waiting.md#71-every-new-row-carries-a-proposal));
- `held`: each held matter with the label that holds it;
- `running`: the rows of [§FS-019-waiting.1.3](FS-019-waiting.md#13-running-every-live-run), every live run, paused or not.
  A paused row carries `paused` ([§FS-019-waiting.1.3.1](FS-019-waiting.md#131-a-run-with-nothing-it-can-do-says-it-is-paused-what-it-waits-on-and-when-it-next-looks)): the row's sentence
  `says`, its `waits`, and `next_look_at`, the earliest next attempt, where a
  poll is among them. Each wait has its `ticket`, `task`, `state` and `kind`
  (`gate` or `poll`), and on a poll the `waiting_on` it declares and its
  `next_attempt_at`. A run taking turns carries no `paused`, and neither does
  one whose next look came before the reading was printed;
- `queued`: the work processing laid that waits for a slot
  ([§FS-019-waiting.7.6](FS-019-waiting.md#76-running-names-what-waits-for-a-slot));
- `adopt`: the sentence naming the recipe a repeated confirmation would make
  automatic, where there is one ([§FS-019-waiting.7.7](FS-019-waiting.md#77-a-proposal-confirmed-again-and-again-names-the-setting-that-would-make-it-automatic));
- `decided`: what the reader decided today, `laid` and `dismissed`
  ([§FS-019-waiting.2](FS-019-waiting.md#2-an-empty-section-is-one-line));
- `overview`: the two counts of [§FS-019-waiting.3](FS-019-waiting.md#3-the-header-and-the-overview-line), `monitors`, the
  statuses' counts `{down, unknown, overdue}` where the line carries them
  ([§FS-021-monitors.20.1](FS-021-monitors.md#201-waitings-overview-line-names-the-monitors)), and `refreshed_at`, the
  newest fetch, where the feed was ever fetched; and
- `refusal`: the workable rung's sentence, where the runtime cannot run
  anything.

A field the runtime does not report is absent from the view rather than null
or guessed ([§FS-019-waiting.1.1](FS-019-waiting.md#11-waiting-on-you-every-gate-and-every-poll-on-the-reader)). A gate's `since` is the one field that may be
`null`: the runtime listed the gate and said it does not know when it began
waiting, which is a different answer from listing nothing
([§FS-019-waiting.1.1.1](FS-019-waiting.md#111-a-runtime-that-lists-its-gates-names-each-one-says-how-long-it-has-waited-and-what-it-holds)). A later field is added freely, as the view
schema promises.

The command reads the cache and the disk, like the screen. It fetches
nothing, because `refresh` remains the only verb that asks the world
([§FS-001-forge-interface.7](FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)), and it writes nothing. Like the operations board
it answers for the whole site, so the scope selectors are refused rather than
quietly ignored ([§FS-011-command-line.9](FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)).

`O` is the key both commands carry: `ephor waiting` is the Waiting screen,
and `ephor feed` is the overview ([§REQ-002-parity.5](../requirements/REQ-002-parity.md#5-the-parity-list-is-checked-not-remembered)).

## 7. A new matter is processed where it is shown

A matter is **processed** when a rhei is assigned to it, which is work the
ledger records about it ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), or when a person has recorded
that it needs none ([§FS-019-waiting.7.4](FS-019-waiting.md#74-x-records-that-a-matter-needs-no-rhei)). New is every matter that is
neither processed nor held, so processing a row is what takes it off the
screen, and the screen is done when New is empty.

Every move here is a move the rest of ephor already makes. The proposal is the
menu's, the laying is the one `ephor actions run` and `ephor work lay` make,
and the start is the sweep's own admission. The screen computes no answer of
its own, and every key has a command that makes the same call
([§REQ-002-parity.2](../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).

### 7.1 Every new row carries a proposal

The **proposal** is the first entry `ephor actions --item <id>` would offer for
the matter, in menu order ([§FS-011-command-line.1](FS-011-command-line.md#1-what-may-be-done-here-listed-and-run)), among the entries that
hand work over: a recipe, or a workflow
([§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)). An entry the menu refuses, or one that already has
work going about the matter ([§FS-005-dispatch.21](FS-005-dispatch.md#21-what-is-already-going-is-shown-where-it-could-be-started-again)), is passed over. The
entries, their order, their gates, the hand, the branch a template would mint
and the workspace it belongs in ([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)), and the work root
([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)) are all the menu's, read through the same session.
A monitor's drop carries its monitor's own `workflow` or `action` at the head
of that order, ahead even of what ephor itself recognized, the site's choice
over the manifest's, so that is its proposal ([§FS-021-monitors.16.1](FS-021-monitors.md#161-the-monitors-own-workflow-comes-first)).

The proposal carries the entry's id, kind and description, the hand, the
branch and workspace where the entry mints them and whether that workspace is
there yet, the work root, and how many other entries would also hand the
matter over. Those others are what `a` chooses among.

A matter no entry hands over is offered the runtime's `triage` workflow, where
the project's runtime offers a workflow by that name. Where it offers none, the
proposal still names `triage` and says that it is not available. Ephor ships no
`triage` workflow of its own: which one stands behind the name is the
runtime's.

A proposal is **unambiguous** when it is available and its entry is ready to
hand work over. One that is not is still shown, with why.

A held matter carries no proposal: it is offered nothing
([§FS-019-waiting.1.2](FS-019-waiting.md#12-new-every-matter-nothing-has-been-done-about)), on the screen and on the command line alike.

### 7.2 Confirming lays it, records it, and starts it

`Enter` on a new row opens the **processing screen**. Its title names the
matter and where it stands among the new rows, `new matter — rhei#431 · 1 of 5
new`, and it shows:

- the matter: its kind, title, project, how long ago it last moved, and its
  labels;
- who holds it: the assignees the feed reports, `nobody · no assignee on the
  forge` where it reports none, and nothing where the source does not say;
- `work: none laid · no plan in the ledger`, which is what makes it new;
- the proposed rhei and why it is the one proposed: the first entry in menu
  order, how many others accept the matter, and `a` to choose;
- the hand, the plan the move would write, and the checkout: its branch and
  whether that workspace is there or will be created;
- the inputs a workflow would be laid with, and what answered each;
- the account a dry run of the same move gives of it; and
- one sentence saying what confirming does: it lays the plan, records it
  against the matter in the ledger, and asks for its start, and ephor posts
  nothing to the forge.

The screen's own words say `work` where the runtime's name would go, as
every string ephor shows does ([§REQ-001-boundary.5](../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)).

Nothing is written until the reader confirms with `Enter`. `n` moves to the
next new row's processing screen without deciding this one, and `esc` comes
back to Waiting. Opened from the Monitors page, the screen is about the one
matter that row names: it counts no new rows and offers no `n`, and `esc` and
a confirmed move come back to the page
([§FS-021-monitors.18.10](FS-021-monitors.md#1810-keys)). What the screen reads is what `ephor work process --item <id>
--dry-run` reads, and each line of it is that command's or the menu's.

Confirming makes the move `ephor actions run` makes with that entry
([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)): a recipe entry writes its ticket, and a workflow entry
lays its plan down through the same laying `ephor work lay` uses
([§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)). The ledger records the work against the matter,
which is what makes it processed. Then a start is asked of the sweep's own
admission, narrowed to the roots that hold this matter's work
([§FS-005-dispatch.24.5](FS-005-dispatch.md#245-work-processed-from-waiting-waits-for-its-slot)): `max_concurrent`, `max_active`, a pool that
cannot be had, a root resting after a failed start, and the spend ceiling
decide whether it starts now ([§FS-015-spend-ceiling.6](FS-015-spend-ceiling.md#6-only-the-sweep-is-bound-and-the-persons-key-never-is)). Work they do not
start now is laid all the same and **queued**.

Processing judges the matter against the ledger as it is on disk, not as the
screen last read it, and it holds the lock a sweep holds from that reading
until its record is written. A matter a sweep or another process handed over
since the screen was read is refused as already having work, and a sweep
running at the moment of confirmation is waited for rather than written over.
A dismissal is written the same way. The lock is let go before the start is
asked for, which takes it again as any sweep does.

A workflow whose inputs are not all answered is not laid with a guess. The
processing screen says what it still wants, and `Enter` opens the workflow's
own input screen ([§FS-005-dispatch.19](FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here)); laying it there processes the
matter with those answers. `ephor work process` refuses it by naming each
input, as `ephor actions run` does, and `--set` answers them.

Ephor posts nothing to the forge. Where the issue is assigned, the plan's own
claim task assigns it ([§FS-005-dispatch.7](FS-005-dispatch.md#7-handing-over-work-is-the-readers-move-and-stays-inside-the-machine)).

`ephor work process --item <id>` is the same move about the proposal, and
`--dry-run` is the account the processing screen shows.

### 7.3 `a` chooses another

`a` on a new row, or on its processing screen, lists every entry the matter's
menu would hand it over with, in menu order, followed by every workflow the
runtime offers there that no entry names. Choosing one opens the processing
screen for it, exactly as the proposal's does. `ephor work process --item <id>
--entry <entry>` is the same choice, by the name `ephor actions` prints or a
workflow's own id.

### 7.4 `x` records that a matter needs no rhei

`x` on a new row, or on its processing screen, asks for a reason and records a
**dismissal**: a ledger record about the matter with the reason, when it was
made, and the matter's fingerprint ([§FS-005-dispatch.4](FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)). A reason is
required; an empty one is refused, because a decision nobody can read back is
not a record of one.

A dismissed matter is processed and leaves New. **The dismissal is lifted the
moment the matter moves**, by the comparison that reopens work
([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)): a matter whose fingerprint differs from the one its
dismissal took is new again, and nothing is written to lift it. A second
dismissal of the same matter replaces the first.

A dismissal is about this screen and nothing else. It holds back no recipe, no
action and no `lay`: a matter a recipe hands over is still handed over, and
then it is processed for that reason instead.

`ephor work dismiss --item <id> --reason <text>` makes the same record.

### 7.5 `A` processes every unambiguous row after one preview

`A` previews every new row: for each one whose proposal is unambiguous, the
account a dry run gives of processing it; for each other, why it is left for
the reader. Nothing is written by the preview. Confirming it processes each
unambiguous row in the screen's order, exactly as confirming one does, and
says what each came to. A row whose dry run asks for an input nobody answered
is not unambiguous and is left.

The admission decides how many start
([§FS-019-waiting.7.2](FS-019-waiting.md#72-confirming-lays-it-records-it-and-starts-it)). The rest are laid all the same and queued, so `A` on
a full site lays everything and starts what fits.

`ephor work process --all` is the same move, and `--dry-run` is its preview.
It lays work in every project its rows belong to, so it is a sweep and is held
to the gate every sweep is ([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)): above one project it
prints the preview, writes nothing and names `--act`, and `--project` and the
selectors narrow which projects' rows it takes
([§FS-011-command-line.9](FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)). On the screen the preview is the confirmation, and
`Enter` on it is the reader's act.

### 7.6 Running names what waits for a slot

Work processing laid and asked to start, that has not started, is **queued**.
Under the live runs, one line names it:
`queued 2 · ephor#119, fissile#78 · waiting for a slot (max_active 4)`. The
reason is the admission's own sentence from when the start was asked for.

A queued matter is shown while its root has no live run and its work still
holds a ticket a run would advance. Its record lasts until a run starts on that
work's root, whoever starts it ([§FS-005-dispatch.24.5](FS-005-dispatch.md#245-work-processed-from-waiting-waits-for-its-slot)), and a matter whose work
was finished some other way is not shown, whatever the record says.

### 7.7 A proposal confirmed again and again names the setting that would make it automatic

When the reader has confirmed the same entry, from Waiting, for three or more
matters of the same kind, the footer names it and the setting that would make
it automatic: `implement confirmed for 3 issues · "autorun": true and
"dispatch" on implement would do it without you`. A recipe needs both
([§FS-005-dispatch.32](FS-005-dispatch.md#32-a-recipe-can-ask-for-its-own-sweep-and-say-how-often)), and a workflow entry needs `autorun`
([§FS-005-dispatch.28](FS-005-dispatch.md#28-a-workflow-entry-can-ask-for-the-same-thing-a-recipe-can)). A setting the entry already has is not named, and an
entry that already has every one is not mentioned.

The sentence changes nothing. Adopting a recipe is the reader's act, made in
the configuration where every other one is written
([§FS-005-dispatch.7](FS-005-dispatch.md#7-handing-over-work-is-the-readers-move-and-stays-inside-the-machine), [§FS-005-dispatch.32.1](FS-005-dispatch.md#321-the-reader-adopts-the-recipe-the-recipe-does-the-rest)). The count is read
from ephor's record of what processing did, and `ephor waiting` carries the
same sentence.
