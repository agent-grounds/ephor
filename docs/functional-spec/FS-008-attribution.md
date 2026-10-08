# FS-008-attribution: every conversation finds its project, or says that it could not

Conversations arrive from places that know nothing of the registry: a
mailbox serves every project a person has, a discussion sits on an adjacent
repository, a notice names a subject nobody configured. Attribution is
ephor's own move — taking a conversation and deciding which project it is
about — and it is data matching, never code: evidence the conversation carries
against identity the registry declares ([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)). Whose the
conversation is, the person's or the organization's, is a different question,
and the site answers it about the source the conversation came through
([§FS-018-private-sources](FS-018-private-sources.md#fs-018-private-sources-a-source-the-person-declares-private-keeps-its-work-theirs)).

## 1. Identity is declared, and the row has the last word

A project's identity is the set of signals by which its matters are
recognized: ticket patterns, the forest's repositories, the wider
**territory** the project claims — repositories and organizations that are
its business without being in its forest — names and aliases, addresses, the
**rooms** it claims: venues on a conversation source, named exactly by the
id the source states, and the **fallback sources** it is the home for:
conversation sources, named exactly as `status.json` names them. It lives in
the registry row; a manifest may hint it
([§FS-006-project-interface.2](FS-006-project-interface.md#2-the-manifest-is-offered-never-required)), and the row adopts or overrides — a checkout
must not be able to claim another project's conversations. Territory is what
places the general case: a mention of the person on some repository of the
project's ecosystem, an issue filed there, a discussion opened there —
none of it in any forest, all of it the project's business
([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)).

A room is to a conversation what a repository is to a pull request: the venue
it happens in, a group or a channel. It is matched only as the source spells
it — no prefix, and no claim on everything under one organization — because
chat ids share no grammar ephor could read without naming the networks that
issue them ([§REQ-001-boundary.5](../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)). The row says which rooms are the
project's by writing the list, even an empty one, as it does for every list a
checkout can hint ([§FS-008-attribution.1.1](FS-008-attribution.md#11-the-rows-word-on-a-hinted-list-is-its-presence)).

A fallback source is the one signal no checkout can offer. A source's name
exists only in the site's configuration, so neither a manifest nor a project
type can hint one, and the claim lives on the row alone
([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)). With no hint to refuse, the presence rule has nothing to do
here ([§FS-008-attribution.1.1](FS-008-attribution.md#11-the-rows-word-on-a-hinted-list-is-its-presence)): `fallback_sources: []` claims nothing,
exactly as a row without the field does. Only a row whose project the site watches claims, as
for every other signal. What a claim places, and what still beats it, is
[§FS-008-attribution.3](FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance); a claim that can place nothing is said
([§FS-008-attribution.4](FS-008-attribution.md#4-unattributed-is-a-place-not-a-fate)).

### 1.1 The row's word on a hinted list is its presence

Of what a manifest's `identity` offers, four lists are adopted as hints: the
project's aliases, its territory, its addresses and its rooms. For each of
them the row's word is whether it writes the list, never whether the list is
empty. A list the row writes is final, `[]` included: what it names is the
project's, and nothing the checkout hints for that list is added to it, so
`"addresses": []` refuses every address the checkout claims. Only a row
without the key adopts the manifest's hint. A row that could say *none* only
by leaving the key out could not refuse a checkout's claim, and the row is
authoritative over the manifest because attribution keys must not be
forgeable by a checkout ([§FS-006-project-interface.2](FS-006-project-interface.md#2-the-manifest-is-offered-never-required), [§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)).
Whoever can push to a repository can edit its manifest, and a claimed
address or organization places a person's own mail and chat; a site that
cannot refuse that claim is back to sweeping by hand
([§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)).

The rule holds one list at a time. A row that refuses one hint keeps the rest
of what the manifest offers — its other hints, its checks, its gate, its
actions. How far the row trusts the manifest decides only whether there is a
hint at all, so a list present on the row wins at every trust level.

A row's own addresses are matched as written and place at the strength of a
reference, exactly as hinted ones do
([§FS-008-attribution.3](FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance)). A row whose `addresses` is anything but a list of
non-empty strings is refused by name, as one with such `rooms` is: read as
silent, it would let the checkout's hint stand in for what the row tried to
say.

**Fallback sources are the one list where `[]` is the same as no key.** The
rule exists so that a row can refuse a hint, and no checkout can hint a
source's name, so an empty list has nothing to refuse and claims nothing.

**The forest's repositories are never adopted from a manifest.** They come
from the project type's layout, which every row has, so the row always
speaks for them and a manifest's `identity.repos` is not read as identity. A
repository a row wants to claim beyond its forest goes in its territory,
which places it as a forest repository would.

## 2. Two stages, one engine

Attribution runs discussion → matter, then matter → project and branch. It
is one matching engine at two scopes: the branch matching that already
places items under a project's branches is this engine confined to one
project, and it is promoted, not duplicated.

**A project's branches are the row's and the disk's together.** The row names
the branches somebody wrote down; the workspaces are wherever branches were
actually checked out, and the two are not the same list — the same gap task
stores are read across ([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)). A branch whose
workspace is on disk is one ephor can place work on, so it is one items are
placed under, whether or not the row names it. Anything else is ephor
contradicting itself about a fact it measured: a row reading `✓ checked out`
under a heading that says the item is linked to no branch, and — worse — a
checkout ephor made itself ([§FS-004-quick-actions.7](FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout)) staying invisible to the
grouping the moment after it was made.

A branch found this way is named by the directory it was found in, never by
what its checkout has at `HEAD`, so the directory a branch resolves to and
the directory it was found in are always the same one. The row keeps the last
word on everything else about a branch — its ticket, whether it is active,
whether it is a release branch — and on identity, which no checkout may widen
([§FS-008-attribution.1](FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word)).

### 2.1 A matter's branch is one it names, never one its conversation quotes

At the second stage a matter is placed on a branch by what it says of itself,
and by nothing its conversation says about it. The branch the forge recorded
for it is the firm answer; after that, a branch whose ticket key the matter's
own id or title carries, and a branch its title names outright. A comment
quoting a branch — a lifecycle record naming the checkout of the run that filed
the issue, a reviewer pointing at where a neighbour's fix landed — is the
conversation talking, and it places the matter on no branch: an issue whose
thread quotes `fix/issue-274` is not work on `fix/issue-274`, and work about it
is placed as for any matter with no branch of its own, which a `branch`
template mints ([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)).

The first stage still reads the whole conversation, because a discussion
with no subject of its own that names one belongs to the named matter ([§FS-008-attribution.3](FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance)). Which
branch is a different question — which tree the matter's own work is in — and
the tree a quote names is the one somebody else's work is in. Reading it as the
matter's would let resemblance amend a row, which [§FS-008-attribution.3](FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance)
forbids, and it would get worse the longer a matter lived: the more of an
issue's history the toolchain writes back onto it, the more checkout paths its
thread quotes.

## 3. Venue beats reference beats fallback beats resemblance

A discussion *on* a subject belongs to that subject's matter, and a
conversation whose source stated its own key is on that subject
([§FS-007-matters.1](FS-007-matters.md#1-a-matter-is-a-subject-with-a-stated-identity)): a chat conversation its gateway keyed is a matter of its
own. A discussion that *names* a subject but has none of its own — a ticket
key in an unkeyed mail's text — belongs to the named matter. What a discussion
on a subject names is linked onward ([§FS-007-matters.2](FS-007-matters.md#2-same-subject-one-matter-related-subjects-linked-matters)), never moved: a
conversation that mentions a pull request stays the row it is, beside the pull
request's. Only where neither holds may declared aliases place a conversation,
and then as a topic matter, never onto an existing subject: resemblance may
start a new row, it may not amend one. At the second stage the venue itself is
the explicit signal: a matter whose subject sits on a repository of a
project's forest or declared territory, or in a room the project claims
([§FS-008-attribution.1](FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word)), is that project's before any reference or alias is
consulted. A conversation no claimed room places — a direct one, which has no
room to claim, or one in a room nobody claimed — goes through the same stages
as anything else: what it references places it, a row that claims its source
places what references nothing, and it lands in the bucket only where nothing
matches or several projects match equally
([§FS-008-attribution.4](FS-008-attribution.md#4-unattributed-is-a-place-not-a-fate)).

A row's **fallback claim** ranks below venue and reference and above
resemblance ([§FS-008-attribution.1](FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word)). A conversation from a source a row claims
belongs to that row unless a venue or a reference places it elsewhere: a
claimed room, a repository of a forest or a territory, a ticket key, a
repository named in its text, an address. A mail asking about
`agent-grounds/rhei#12` still goes to the project claiming that repository.
Resemblance never places a conversation from a claimed source, neither by a
project's name nor by its aliases, so a family mail whose German *Grund*
matches a project's name stays with the row that claimed the inbox. The claim
says which project a conversation belongs to and never which subject: like
resemblance, it may start a row and may never amend one. It never breaks a tie
above it, so a conversation that references two projects equally goes to the
bucket with those two, whoever claims its source. It covers conversations
only. A notice or an issue is placed as it was: what either usually names is a
repository or a ticket key, and the bucket's prompt for it, a territory or a
ticket pattern to add, is a signal somebody can add.

In a conversation's title or message text, `owner/name#N`, where `N` is one
or more ASCII decimal digits, references the repository `owner/name`, just as
the plain repository name or its issue URL does. Ordinary surrounding sentence
punctuation does not change that reference. With no claimed room or other
venue placing it, the conversation belongs to the project claiming that
repository at Reference, even when the project's name differs from the
repository's name. This short form does not admit arbitrary suffixes such as
`owner/name#note` or `owner/name#12extra` as repository references. A
conversation's own key alone is never a repository reference, even when it
has the same `owner/name#N` spelling. Text references are extracted from its
title and messages separately from that key.

## 4. Unattributed is a place, not a fate

A conversation that matched nothing lands in a visible unattributed bucket,
in the interactive view and on demand — never dropped. The bucket is the
attribution seam's degrade rule ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)): mapping failures are
seen where they can be fixed, by adding the signal the identity was
missing.

A claimed source has a home for what names nothing. Its conversation that no
venue and no reference places belongs to the claiming row, not to the bucket
([§FS-008-attribution.3](FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance)). The trade is deliberate: for such a conversation
the bucket stops prompting for the signal it lacked, so a mail that is really
about one project but names none lands under the claimer, and what moves it is
a reference in the mail rather than a signal on an identity. Where several rows
claim one source, nothing is resolved by order: a conversation only their
claims reach goes to the bucket with every claimer as a candidate.

A claim that can place nothing is said, never shrugged. That is a name no
source in `status.json` has, a source bound only under projects, whose reports
are that project's and are never weighed ([§FS-001-forge-interface.9](FS-001-forge-interface.md#9-a-source-is-bound-to-one-project-or-to-the-site-and-every-move-goes-back-to-the-source)), and a
built-in source, none of which reports conversations. Each is a note naming the
row, the source, and why the claim can place nothing. `ephor refresh` prints it
as a `note:` and carries it under `notes` in `--json`, and `ephor doctor`
prints the same sentence in its report and in its JSON
([§FS-010-doctor.1](FS-010-doctor.md#1-it-reports-what-is-already-judged-and-judges-nothing-itself), [§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)). It is news rather than a fault,
so it moves neither a project's health nor the exit code
([§FS-010-doctor.5](FS-010-doctor.md#5-the-answer-is-in-the-exit-code)): nothing is lost, because what the claim would have
placed is still in sight, in the bucket or under a project.
