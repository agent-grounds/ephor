# DF-005-a-row-is-a-sources-fallback-home: a registry row may claim a conversation source as its fallback home, below every venue and reference and above resemblance

**Status:** Accepted
**Date:** 2026-10-08

A registry row can name the conversation sources it is the fallback home for,
`"fallback_sources": ["mail-me"]`, each matched exactly by the name
`status.json` gives the source ([§FS-008-attribution.1](../../functional-spec/FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word)). A conversation from
a claimed source belongs to the claiming row unless a venue or a reference
places it elsewhere, and resemblance never places it
([§FS-008-attribution.3](../../functional-spec/FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance)). What such a source reports and nothing names
goes to the claimer instead of the unattributed bucket, and a claim that can
place nothing is said ([§FS-008-attribution.4](../../functional-spec/FS-008-attribution.md#4-unattributed-is-a-place-not-a-fate)).

This record was written because a personal inbox had no home. Kept as a
site-level source, so that a mail naming a project reaches that project, it
reported three mails one morning. *Before Friday* asked about
`https://github.com/agent-grounds/rhei/issues/12` and went to rhei by
reference, which is right. *Sunday* asked who brings the cake and went to the
bucket, among the mapping failures the bucket exists to show. *Absage* asked in
German why someone is not coming on Saturday, and its *Grund* matched a
project's name, so a family mail became the `grund` project's matter. Binding
the source under the person's own row instead puts all three there, and the
mail about rhei's issue never reaches rhei. A bucket that fills with family
mail stops meaning "add the signal this identity was missing", and the sweep
[§GOAL-003-nothing-lost](../../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep) wants retired comes back. What follows is what was
decided, what was weighed and refused, and what was named and left for later,
so that a reader who finds one of these absent meets a reason rather than an
oversight ([§REQ-002-parity.2](../../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).

## 1. Decided

**A fallback claim is a claim in DF-003's sense.**
[§DF-003-chat-is-a-forge-capability.2](DF-003-chat-is-a-forge-capability.md#2-rejected) closes: "The visible bucket is where
what nothing claims goes." A row that names a source as its fallback home has
claimed what that source reports and nothing firmer places, so that
conversation no longer reaches the bucket. This narrows one clause of
[§DF-003-chat-is-a-forge-capability.1](DF-003-chat-is-a-forge-capability.md#1-decided): for a claimed source, "in the
unattributed bucket when it names none" no longer holds, and the conversation
goes to the claiming row. The record's other clauses stand. A conversation no
room claims is still placed by what it names, several equal claims still go to
the bucket, and direct conversations still take the same path with no exception
for them, because the claim is keyed by the source and not by whether a
conversation is direct. Its rejection of policy on the organization block
stands too: the claim sits on the registry row, whose job is identity
([§REQ-001-boundary.2](../../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)), and nothing moves onto the block. DF-003 is not
edited.

**The rung sits below venue and reference and above resemblance.** A reference
still wins, so the mail about rhei's issue reaches rhei from a claimed inbox. A
resemblance never places a conversation from a claimed source, neither by a
project's name nor by its aliases, so *Absage* stays with the row that claimed
the inbox. The claim says which project a conversation belongs to and never
which subject: it starts a row and never amends one, as resemblance does. It
never breaks a tie above it. Its strength is `fallback`, ordered resemblance <
fallback < reference < venue, and it appears where the strength already does,
in the feed cache.

**`[]` claims nothing, the same as no field.** The presence rule rooms follow
exists so that a row can refuse a checkout's hint. A source's name lives only
in the site's configuration, so no manifest and no project type can hint one,
and an empty list has nothing to refuse. Validation still refuses a value that
is not a list, an entry that is not a string, and an empty name.

**Conversations only.** A notice or an issue from a claimed source is placed as
it was. What either usually names is a repository or a ticket key, and the
bucket's prompt for it, a territory or a ticket pattern to add, is a signal
somebody can add. A conversation that names nothing has no signal anyone could
add, and that is the gap the claim fills. Keeping to conversations also keeps
the narrowing of DF-003 to what DF-003 decided.

**A claim that can place nothing is said.** There are three such claims: one
naming no source `status.json` has, one naming a source bound only under
projects, where attribution never runs
([§FS-001-forge-interface.9](../../functional-spec/FS-001-forge-interface.md#9-a-source-is-bound-to-one-project-or-to-the-site-and-every-move-goes-back-to-the-source)), and one naming a built-in source, none of
which reports conversations. A source renamed in `status.json` is how a claim
usually dies, and without a word everything it placed would drift back to
resemblance and the bucket. `refresh` and `doctor` print the same note, in
prose and in JSON, and the exit code does not move
([§REQ-002-parity.3](../../requirements/REQ-002-parity.md#3-every-reading-answers-a-program), [§FS-010-doctor.5](../../functional-spec/FS-010-doctor.md#5-the-answer-is-in-the-exit-code)): what the claim would have
placed is still in sight, in the bucket or under a project.

**Nothing is re-keyed.** A conversation's row key is the source's name and the
gateway's id for it, with no project and no scope in it, and its done mark is
kept under that key with a fingerprint that holds no placement. So moving a
source from under a project to the site, and claiming it from a row, keeps
every done mark. A thread is re-keyed when its source is renamed, or when it
moves to another source, as it would if one mailbox were split into two. That
was so before this record and stays so.

**The trade at the bucket is deliberate.** A claimed source's conversation
that names nothing goes to the claiming row, so the bucket stops prompting for
the signal that conversation lacked. A mail that is really about grund but
carries no reference lands under the claimer, not on grund as a topic, and what
moves it is a reference in the mail. That is the price of a home for
everything else such an inbox carries.

## 2. Rejected

**Binding the source under the person's row.** It works today, and every
conversation it reports then belongs to that row, references included, so the
mail about rhei's issue never reaches rhei.

**Listing the correspondents' addresses on the row.** An address ranks as a
reference. A mail from a listed sender that names rhei then ties with rhei's
reference and goes to the bucket, and a sender nobody listed goes there too.

**Claiming the inbox as a room, or giving each direct conversation one.** A
room is a venue, and a venue beats a reference, so the mail about rhei's issue
would stay under the person's row. It also invents a room where DF-003 says a
direct conversation takes the ordinary path.

**An alias on the row.** Resemblance fires on words. *Sunday* names nothing and
still goes to the bucket, and *Grund* would tie the row with `grund` and go
there too.

**An inbox field on the forge conversation.** The source already names the
inbox, so the field would carry nothing new.

**A pass after placement that hands what placed nowhere to the claimer.** It
would miss *Absage*, which resemblance has already put on `grund` by then, and
it would be a second placement path beside the one engine.

**An empty list that refuses.** It is what rooms do, and here it has nothing to
refuse: no checkout can name a source.

## 3. Deferred

These are named and not done; none is needed for a claimed source to be
placed.

- A claim over the notices and issues a source reports.
- A row's own `addresses` read as identity, and every identity list read by its
  presence. Taken up by [§DF-006-every-hinted-list-is-read-by-presence](DF-006-every-hinted-list-is-read-by-presence.md#df-006-every-hinted-list-is-read-by-presence-a-registry-rows-word-on-every-list-a-checkout-can-hint-is-its-presence-and-the-row-names-its-own-addresses), except
  forest repositories as identity, which it defers in turn
  ([§DF-006-every-hinted-list-is-read-by-presence.3](DF-006-every-hinted-list-is-read-by-presence.md#3-deferred)).
- Keeping a personal source's matters out of an organization's work roots and
  recipes.
