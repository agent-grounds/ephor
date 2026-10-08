# AR-006-matters: the core types of the watch

The nouns of [§FS-007-matters](../functional-spec/FS-007-matters.md#fs-007-matters-the-feed-is-made-of-matters-and-a-matter-knows-why-it-is-there) as data. These types are core-layer
([§AR-001-layers.1](AR-001-layers.md#1-the-layers)): no source, seam, or surface adds fields of its own —
what a provider knows beyond the model rides in `raw` passthrough and comes
back out in `EPHOR_RAW`.

That sentence is read as a refusal and as a gap in the same breath, so it says
which it is. A passthrough blob reaches exactly one surface: the command that
gets `EPHOR_RAW` and parses it again. When a fact a source reported has to be
named by a *selector*, a *template* and that environment alike, passthrough is
not enough — and growing a field for it would be the sentence above being
crossed. **So a `raw` key becomes reserved vocabulary instead, and no type
grows a field.** A reserved key is one ephor names, any source may fill, every
surface reads through one accessor, and that still rides in `raw`: the cache
model does not move, `EPHOR_RAW` carries it for free, and the sentence above
stays literally true.

Three things have to hold before a key is reserved, and they are the whole
test. The fact is about **this matter specifically** rather than about the
source that reported it. **More than one surface has to name it by name**, so
passthrough is genuinely short. And **no type grows a field for it** — if the
fact belongs to every matter whatever reported it, it belongs on the model and
this rule is not the way in. The reserved set is `assignees` and `labels`
([§FS-005-dispatch.31](../functional-spec/FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)), and `meta`, the bounded map of what one source said
about one matter under keys that source chose
([§FS-005-dispatch.31.1](../functional-spec/FS-005-dispatch.md#311-and-it-can-ask-what-the-matters-own-source-said-about-it), [§FS-005-dispatch.8](../functional-spec/FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)). A key no source reported is
**absent rather than empty**, which is the distinction every reader of a
reserved key is built on and what a selector refuses on
([§FS-005-dispatch.31](../functional-spec/FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)).

## 1. The types

- `Matter { key, kind, placement, state, links, discussions, events,
  fingerprint, seen }` — `key` is the stated subject
  ([§FS-007-matters.1](../functional-spec/FS-007-matters.md#1-a-matter-is-a-subject-with-a-stated-identity)): `gh:owner/repo#123`, `ticket:GR-73955`, a store's
  own id, `topic:<digest>`. `placement` is project and branch or
  unattributed-with-candidates. `links` are referenced keys
  ([§FS-007-matters.2](../functional-spec/FS-007-matters.md#2-same-subject-one-matter-related-subjects-linked-matters)).
- `Discussion { channel, messages, needs_response }`;
  `Message { author, time, text, reactions, task, attachments }` — task state
  carried where a channel tracks one ([§FS-003-feed-categories.4](../functional-spec/FS-003-feed-categories.md#4-a-conversation-is-answered-in-whatever-form-the-forge-recorded-it)).
  `attachments` is the files the source named on the message, each
  `Attachment { name, media_type, size, id }` exactly as reported
  ([§FS-001-forge-interface.1](../functional-spec/FS-001-forge-interface.md#1-capabilities)). It is a field on the model and not a key in
  `raw`, by the lead's own exit: mail, chat and a forge's comments all carry
  files, so the fact belongs to every matter whatever reported it, and `raw`
  belongs to the whole matter where a file belongs to one message. It is
  optional, and **absent stays distinct from empty** through every layer: absent
  is a source that did not report files, `[]` one that reported none. A cache
  written before the field existed therefore reads as not reported until the
  next refresh, with no rebuild
  ([§AR-006-matters.4](AR-006-matters.md#4-the-cache-is-a-cache)), and the published
  machine forms gain the field without losing one
  ([§REQ-002-parity.4](../requirements/REQ-002-parity.md#4-the-machine-form-is-a-contract-not-a-dump)).
- `Channel { id, capabilities }` — react, tick, reply
  ([§FS-007-matters.4](../functional-spec/FS-007-matters.md#4-a-channel-says-what-it-can-do)).
- `Event { kind, time, payload }` — gate counts per repository, state
  transitions, check results ([§FS-007-matters.5](../functional-spec/FS-007-matters.md#5-an-event-moves-state-and-resurfacing-names-its-reason)).

## 2. Merging and fingerprints

Merge happens on identical `key` at the pipeline's merge stage, the richer
report surviving and the loser's unique facts carried over
([§FS-003-feed-categories.5](../functional-spec/FS-003-feed-categories.md#5-one-subject-is-one-row-however-many-sources-reported-it)). The fingerprint digests state, each
discussion's (last activity, message count, task states), and the event
tail; comparing fingerprints is how sync finds moved matters, and the
differing component is how the row names its reason for resurfacing
([§FS-005-dispatch.5](../functional-spec/FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work), [§FS-007-matters.5](../functional-spec/FS-007-matters.md#5-an-event-moves-state-and-resurfacing-names-its-reason)).

## 3. The dossier is a view

A dossier is materialized from a matter on demand — state, placement,
forest and workspace, gate breakdown, discussions quoted under the bounds
of [§FS-005-dispatch.2](../functional-spec/FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it) — in two renderings from one source: prose for the
ticket, `EPHOR_*` identifiers for programs ([§FS-005-dispatch.8](../functional-spec/FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)), the
materials that cross the seam ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)). It is never stored: a
stored dossier would be a second truth about the matter.

## 4. The cache is a cache

The feed store under the state directory persists matters between
refreshes and nothing else is derived from it that cannot be re-fetched.
Model changes rebuild it; `seen` state (read, done) is the one part carried
across rebuilds, keyed by matter key, because it is the reader's and not
the world's.
