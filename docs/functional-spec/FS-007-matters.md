# FS-007-matters: the feed is made of matters, and a matter knows why it is there

The unit of the watch is the **matter**: the subject under discussion or
observation — a pull request, an issue, a task in the project's own store, a
periodic build, a status subject, a conversation its source keyed, or a bare
topic. What the spec has so far called an item is a matter seen through one
source's report. A matter is the feed's row, the unit of attribution, of
state, of fingerprinting, and of dispatch — the dossier is the dossier of a
matter — and the reason for the noun is that the same matter is discussed in
more than one place: the pull request's review threads, a mail about it, a
line of chat naming it. One subject, several venues, one row
([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now)): a fragment with no subject of its own joins the row of the
subject it names. A conversation its source keyed is a subject itself, so it
is a row of its own, and what it names is linked to it rather than folded into
it ([§FS-007-matters.2](FS-007-matters.md#2-same-subject-one-matter-related-subjects-linked-matters)). Reports of one subject are still one row, however many
sources made them.

## 1. A matter is a subject with a stated identity

A matter's identity is the subject key its source stated — the pull request
the forge names, the ticket by its key, the store's own id — or, where a
conversation matched a project but no known subject, an identity synthesized
for it as a topic. Identity is never guessed from resemblance: two pull
requests may share a title, and a subject whose identity cannot be
established is left alone ([§FS-003-feed-categories.5](FS-003-feed-categories.md#5-one-subject-is-one-row-however-many-sources-reported-it)).

## 2. Same subject, one matter; related subjects, linked matters

Reports of the same subject key merge into one matter, however many sources
made them, under the survival rules of [§FS-003-feed-categories.5](FS-003-feed-categories.md#5-one-subject-is-one-row-however-many-sources-reported-it). Matters
that *reference* each other — the pull request implementing a ticket, the
project's own task tracking a gate — stay distinct and are **linked**, presented
together under the branch that relates them. Merging what is one thing and
linking what is related is the difference between a readable pile and a
lossy one.

## 3. A discussion is messages grouped in a channel

A matter's conversation arrives as **discussions**: ordered messages with
authors, times, reactions, task boxes, and the files on them, grouped within
one channel. Whether a discussion awaits the reader is decided per discussion,
by the calculus of [§FS-003-feed-categories.4](FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it), identically in every channel. A
matter awaits its reader while any of its discussions does.

A file on a message is named and never fetched
([§FS-001-forge-interface.1](FS-001-forge-interface.md#1-capabilities)), and it takes no part in that calculus.
Whether a discussion waits is read from who spoke and what was ticked or reacted
to, never from what was attached, so a photo sent with no words is still the
last word.

What a row shows of the files is bounded like everything else on a row
([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now)). While the matter awaits its reader, the row carries one
mark, `📎N`, and nothing else about files. N is the number of files on the
**newest turn** of each discussion that awaits the reader, summed. A turn is a
discussion's last message together with the messages its author sent just
before it, as a photo is followed by "is this the one?". The mark is one token
whatever the number: one file reads `📎1` and twelve read `📎12`. A row that
does not await its reader shows no mark, and neither does one whose newest
turns carry no file, because a logo on a mail already answered is nothing to
do. The conversation itself names every file
([§FS-011-command-line.4](FS-011-command-line.md#4-a-conversation-and-the-moves-inside-it)), and the screen and `ephor feed` show the same mark
([§REQ-002-parity.2](../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).

## 4. A channel says what it can do

The venue a discussion lives in — review threads, an issue's comments, a
mail thread, a chat thread — declares its capabilities in the pattern of
[§FS-001-forge-interface.1](FS-001-forge-interface.md#1-capabilities): whether a reaction can be posted, a task ticked,
a reply sent, a conversation settled at its source. What grouping means is the channel's own policy; what the
reader can do about a message is offered only where the channel declared it
([§FS-004-quick-actions.2](FS-004-quick-actions.md#2-offered-only-where-it-would-work)) — an undeclared capability narrows the offer by
the degrade rule of [§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy), never silently.

Reply capability alone does not make a saved proposal sendable. Its bound
thread must still match, and an uncertain saved send takes precedence over
proposal freshness ([§FS-005-dispatch.13](FS-005-dispatch.md#13-a-communication-is-work-too-and-its-answer-comes-back-as-a-proposal)). The offer distinguishes posting a
fresh bound proposal, refusing a stale or unbound one with its reason,
retrying the exact saved send on a reconciling carrier, and checking the
channel before resolving a held outcome. These facts accompany the words
without removing read-only copyability.

## 5. An event moves state, and resurfacing names its reason

Everything about a matter that is not conversation arrives as **events**:
the gate's counts changed, the state closed, a check finished, a ticket
resolved. Events fold into the matter's state, and the matter's fingerprint
digests state, discussions, and the event tail — so when a done matter
moves and resurfaces ([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)), the row can say *what* moved:
resurfacing is always accompanied by its reason, because a row that
reappears without one sends the reader to re-read everything, which is the
sweep this tool exists to retire ([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)).
