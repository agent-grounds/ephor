# DF-003-chat-is-a-forge-capability: chat reaches the feed as a capability of the one forge interface

**Status:** Accepted
**Date:** 2026-10-02

Chat and mail reach the feed through the interface pull requests and issues
already use: a gateway declares **Messages by reason** and **Replies**
([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities)), is bound once for the site
([§FS-001-forge-interface.9](../../functional-spec/FS-001-forge-interface.md#9-a-source-is-bound-to-one-project-or-to-the-site-and-every-move-goes-back-to-the-source)), and its conversations are placed by the rooms the
registry says each project claims ([§FS-008-attribution.1](../../functional-spec/FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word)). Nothing about chat
is compiled into ephor. [§DA-002-fetch-attribution-split](../architectural/DA-002-fetch-attribution-split.md#da-002-fetch-attribution-split-fetch-normalizes-attribution-places) expected that "after the split, mail and Slack are
providers"; the split held, and they arrive instead as one more answer from a
source ephor does not own, because how a chat network is listened to changes
on the network's schedule rather than ephor's ([§REQ-001-boundary.5](../../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)). That record is not edited: its
split is what lets a site-level source be placed at all.

This record was written because a chat conversation had no way into the feed
— the capability set had no row for it and a room was not an identity signal —
while chat is one of the four venues the watch exists to cover
([§GOAL-003-nothing-lost](../../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)). What follows is what was decided, what was weighed
and refused, and what was named and left for later, so that a reader who finds
one of these absent meets a reason rather than an oversight
([§REQ-002-parity.2](../../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).

## 1. Decided

**Two rows join the capability set, and no row changes.** Messages by reason
returns each conversation addressed to the user whole, with its threads in the
shape a pull request's take; Replies sends to a thread through the descriptor
the gateway put on it. Whether a conversation waits on the user stays ephor's
call, made by the rule it already applies to a pull request's threads; the
gateway's only say is which messages were the user's own
([§FS-001-forge-interface.3](../../functional-spec/FS-001-forge-interface.md#3-policy-lives-above-the-interface-never-in-an-implementation)).

**A source is bound to a project or to the site, and every move goes back to
the source that reported it** ([§FS-001-forge-interface.9](../../functional-spec/FS-001-forge-interface.md#9-a-source-is-bound-to-one-project-or-to-the-site-and-every-move-goes-back-to-the-source)). A chat gateway is
the case that needs this, since its conversations are about many projects, but
the rule is general: a reply, a reaction or a restart on a row from a
site-level source reaches that source, and a dry run refuses where the move
would.

**Rooms are claimed exactly, on the registry row.** A room matches the id the
source states and nothing longer or shorter, because chat ids share no grammar
ephor could read without naming vendors. The row's say is its presence: a row
that lists rooms, even none, has the last word over a checkout's hint
([§FS-008-attribution.1](../../functional-spec/FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word)). A claimed room is a venue, as strong as a repository
or a territory, so it beats whatever the conversation mentions
([§FS-008-attribution.3](../../functional-spec/FS-008-attribution.md#3-venue-beats-reference-beats-resemblance)).

**A conversation its source keyed is a matter of its own.** It is a row, and
what it names is linked to it rather than folded into it ([§FS-007-matters](../../functional-spec/FS-007-matters.md#fs-007-matters-the-feed-is-made-of-matters-and-a-matter-knows-why-it-is-there)). A
conversation no room claims is placed like anything else: under the one
project it names, and in the unattributed bucket when it names none or several
equally ([§FS-008-attribution.4](../../functional-spec/FS-008-attribution.md#4-unattributed-is-a-place-not-a-fate)). Direct conversations take the same path; there
is no exception for them.

**A gateway that cannot show it is still hearing the network fails.** An
empty answer means nothing is waiting, so a gateway answering from a spool it
cannot vouch for fails instead of answering ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)). How it
vouches is its own configuration, which ephor hands it and never reads.

**The forge protocol has one thread definition.** Pull-request threads and
conversation threads are the same shape, written once, and the schema that
documents them is held to the types that produce them by a test rather than by
care.

## 2. Rejected

**Chat as notices only.** A notice carries the gateway's own claim that
something needs the user, and ephor keeps that claim. A conversation needs the
opposite: the messages, so that ephor decides, and the reply descriptor, so
that the answer can go back.

**custom-status as the home.** It answers for one project's own state, and its
discussions are dropped today. Making it carry a site's conversations would
grow a second fetch path with its own placement.

**A second protocol.** A chat interface beside the forge one would duplicate
the transport, the capability probe, the failure kinds and the reply path, to
carry a shape the forge already has.

**A built-in chat provider, or the listener inside ephor.** Either compiles a
product into ephor ([§REQ-001-boundary.5](../../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)) and makes ephor hold a connection,
where it otherwise only asks on refresh.

**A separate fetch per conversation.** Asking for the list and then for each
conversation multiplies calls for no answer the whole conversation does not
already give.

**The three-list response.** An answer of channels, discussions and matters,
as the custom-status envelope has, was weighed and found the costlier shape
for what one conversation needs.

**Prefix matching of rooms.** A prefix would let a project claim every room
whose id happens to start alike, and organization-wide claims already exist
for repositories, where the grammar is known.

**A stale spool served as current.** An answer from a record nobody is
keeping current reads exactly like a quiet room, which is the one confusion the
failure kinds exist to prevent ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).

**A room ranked below references.** A conversation in a project's room that
mentions another project's pull request is still the first project's
conversation; ranking the room lower would move it on a passing mention.

**Policy on the organization block.** Routing direct or unclaimed
conversations by an organization-level setting would give the block a second
job; it configures work and nothing else ([§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)). The visible
bucket is where what nothing claims goes.

## 3. Deferred

These are named and not done; none is needed for chat to reach the feed.

- Renaming the forge interface, its declaration title included.
- `addresses` adopted as a bare hint the row cannot override.
- A real mail gateway.
- The accidental repository match on a notice whose id is shaped like one.
- `[]` unable to refuse a hint for `repos`, `territory` and `aliases`.
- custom-status reading an answer's channels and discussions, which owns the
  envelope's reply descriptor and makes its discussion and the forge's thread
  one definition.
