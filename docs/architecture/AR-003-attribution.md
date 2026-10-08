# AR-003-attribution: one matching engine, evidence against identity, at two scopes

The engine that realizes [§FS-008-attribution](../functional-spec/FS-008-attribution.md#fs-008-attribution-every-conversation-finds-its-project-or-says-that-it-could-not) is pure matching: it takes
**evidence** a conversation carries and **identity** the registry declares,
and returns a placement or the reason there is none. It runs inside the
pipeline's attribute stage ([§AR-008-pipeline](AR-008-pipeline.md#ar-008-pipeline-the-engine-is-seven-stages-legible-as-one-function)) and nowhere else — no source
places its own items, a split decided with its configuration cost in
[§DA-002-fetch-attribution-split](../decisions/architectural/DA-002-fetch-attribution-split.md#da-002-fetch-attribution-split-fetch-normalizes-attribution-places).

## 1. Evidence

Extracted once per discussion or event, at fetch normalization: the venue's
own subject key where the source stated one (the pull request the thread is
on, the store the ticket lives in, the conversation a gateway keyed); the room
a conversation states it happened in; the source that reported a
conversation; referenced keys found in text — ticket patterns, pull request
URLs, repository names; addresses and participants; and the plain words that
may hit an alias. Evidence is data on the item, inspectable in `EPHOR_RAW`, so
a misplacement can be debugged by looking.

The reporting source is the name the site configuration gives the source,
never anything the conversation says about itself, and it is set for
conversations only, as the room is: a notice or an issue carries none, so no
fallback claim reaches it ([§FS-008-attribution.3](../functional-spec/FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance)).
It places nothing by itself. It is what a row's fallback claim is matched
against, and only the claim names a project, so the split that keeps every
source from placing its own items holds
([§DA-002-fetch-attribution-split](../decisions/architectural/DA-002-fetch-attribution-split.md#da-002-fetch-attribution-split-fetch-normalizes-attribution-places)).

Repository references in title and message text include the numeric short
form `owner/name#N`: it contributes `owner/name`, as a plain repository name
or issue URL does, including with ordinary surrounding sentence punctuation
([§FS-008-attribution.3](../functional-spec/FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance)). The suffix is one or more ASCII decimal digits;
arbitrary suffixes such as `#note` or `#12extra` are not this form.

The room is kept apart from the conversation's own key and from what its text
references, because the three answer different questions: the key is what the
conversation is, the room is where it happened, and a reference is what it
mentions. A conversation's key is never read for a repository. A chat id can
happen to be spelled like `owner/name#number`, and reading a venue out of that
spelling would let a project's organization-wide territory tie with the room
that actually claims the conversation. This prohibition applies to the key,
including a numeric `owner/name#N` key, rather than to references in title or
message text. Notices keep the older reading for now.

## 2. Identity

Per project, compiled from the registry row with manifest hints adopted
where the row does not override ([§FS-008-attribution.1](../functional-spec/FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word)) — identity lives in
the row per the three homes ([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)): ticket patterns,
the forest's repositories, the declared territory — repositories and
organizations that are the project's business without being in its forest,
which is what places a general mention or a stray issue — names and
aliases, addresses, rooms, and the fallback sources the row claims. Compiled
identities form one table the engine matches against — attribution is a
function of (evidence, identity table), no IO.

Every list a manifest hints — aliases, territory, addresses and rooms — is
compiled from the row's presence rather than its emptiness: a list the row
states, even `[]`, is the project's, and only a row with no field of that name
adopts the manifest's. One rule, one list at a time, so the four cannot drift
apart. An emptiness rule cannot tell `[]` from an absent field, and a bare
hint cannot be refused at all; either would let a checkout claim what its row
refused ([§FS-008-attribution.1.1](../functional-spec/FS-008-attribution.md#11-the-rows-word-on-a-hinted-list-is-its-presence)). The forest's repositories are the project type's layout,
which every row that loads has, and never the manifest's `identity.repos`.

Rooms differ from the lists before them only in how they match: by exact
equality with the id the source states, never by prefix and never
organization-wide ([§FS-008-attribution.1](../functional-spec/FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word)).

Fallback sources are compiled from the row alone. They match the evidence's
reporting source by exact equality, as rooms match the stated room, but no
manifest field is read for them: a source's name lives only in the site
configuration, so nothing a checkout says can name one, and an absent field
and `[]` both compile to no claim ([§FS-008-attribution.1.1](../functional-spec/FS-008-attribution.md#11-the-rows-word-on-a-hinted-list-is-its-presence)).
Whether a claim can place anything is a question about the site rather than
the row, so registry validation, which reads the registry alone, cannot ask
it. One function beside the one that names an organization work block over
nobody reads the claims against the configured sources and names each claim
that reaches no conversation: an unknown name, a source bound only under
projects, a built-in source. `refresh` and `doctor` print what it says
([§FS-008-attribution.4](../functional-spec/FS-008-attribution.md#4-unattributed-is-a-place-not-a-fate)).

## 3. Two scopes, one precedence

Stage one places a discussion or event on a matter; stage two places a
matter on a project and branch. Both apply [§FS-008-attribution.3](../functional-spec/FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance): an
explicit venue wins outright; a reference places what has no subject of its
own on the named matter, and links what has one onward without moving it;
resemblance may only synthesize a topic matter. At stage two a claimed room is
a venue, weighed at the strength of the forest and the territory, so a
conversation in a project's room is that project's whatever it mentions; a
conversation no room claims is placed by its references like anything else.
A row's fallback claim names a project and never a matter, so at stage one its
rung is empty: it never attaches a discussion to a matter. At stage two it sits
below every reference and above resemblance, and like resemblance it only
starts rows and never amends one. The strengths a placement carries are
therefore four, `Strength::Resemblance` < `Strength::Fallback` <
`Strength::Reference` < `Strength::Venue`, serialized as `resemblance`,
`fallback`, `reference` and `venue`.
Ambiguity —
two projects claim the same evidence with equal strength — is not resolved
by order: the item goes to the unattributed bucket carrying its candidates,
because a guess that lands wrong amends someone's matter silently
([§FS-008-attribution.4](../functional-spec/FS-008-attribution.md#4-unattributed-is-a-place-not-a-fate)). Branch matching inside a project is the same
engine with the project's branches as the identity table — the code that
matches ticket keys and branch names today is this function's seed, promoted
rather than duplicated. What it is shown about a matter is the matter's own
word — the ticket keys in its id and title, and its title — and not the
conversation stage one read, so a branch a comment quotes places the matter on
none ([§FS-008-attribution.2.1](../functional-spec/FS-008-attribution.md#21-a-matters-branch-is-one-it-names-never-one-its-conversation-quotes)).
