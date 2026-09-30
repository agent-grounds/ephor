# DF-002-path-fields-stable: a field that decides a path holds still while the work lasts

**Status:** Accepted
**Date:** 2026-09-30

A field used to resolve a matter's branch or its work root answers the same for
as long as that matter's work lasts. A field that may move while the work is
open is refused where a path names it, and may still be named in prose.

The rule is positional rather than about the name. In prose — a dossier, a brief
— a field is a record of what the matter looked like when the work was asked for
([§FS-005-dispatch.2](../../functional-spec/FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)), and a value that has since changed is that record doing its
job. In a path the same field is the resolution: rendering *is* the lookup and
nothing is written down ([§FS-005-dispatch.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)), so a value that moved repoints
live work at a second place while the first is still there, and one item's work
stops reading in one place, in order ([§FS-005-dispatch.5](../../functional-spec/FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)). This record was
written because a slugged title was asked for and refused; what it fixes is the
rule the refusal follows, so that a reader who goes looking meets a stated reason
where the ability is missing rather than an absence, which reads exactly like an
oversight ([§REQ-002-parity.2](../../requirements/REQ-002-parity.md#2-parity-runs-both-ways), [§REQ-001-boundary.1](../../requirements/REQ-001-boundary.md#1-the-anatomy)).

## 1. Why a slugged title was refused

What was asked for was a branch a person reads: `{title_slug}`, the matter's
title reduced to a name a branch and a path will take, so that a recipe over a
project's own tasks mints `task/widen-the-retry-window` where `{id_slug}` mints
`task/rhei-window-1-d8a9c768`. The reading is the whole of the case for it, and
it is a real one.

**Two shapes were offered, and the choice between them was not the question.**
One carries a digest of the title, so the whole name moves when the matter is
retitled. The other carries a digest of the matter's id, so the digest holds and
the readable half drifts from the title it was minted from. The second was
offered as the safe one and is not: the rendered string is the path, so holding
one component still while another moves changes the answer exactly as much.
`rhei:window.1` titled *Widen the retry window* renders
`task/widen-the-retry-window-d8a9c768`, and after a retitle it renders
`task/widen-the-retry-window-and-the-backoff-d8a9c768` — a second path, one
constant component inside it. Both shapes fail the rule above, so the question
was never which of them to take.

**The default is not to offer, and the burden was the proposal's.** What a
template may name is an enumerated vocabulary, and a name joins it deliberately
rather than by being derivable ([§FS-005-dispatch.8](../../functional-spec/FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)). So a field is not refused
here for being impossible or forbidden. It is refused because the case for it was
not made, and what it would have had to carry is on the record: a coherent
narrowed field does exist — the readable title with an unconditional digest of
the matter's **id**, never of the title — and refusing its unsafe uses is
decidable at dispatch, from the branch template and the work-root template alone,
without reaching the forge or the disk. A later proposal is not barred by this
record; it starts from here.

**It loses because its obvious configuration is the unsafe one.** A reader who
wants a branch they can read writes the field into the branch template, and under
a project with a `branch_root_template` the branch is the checkout and the
checkout is where the plan goes ([§FS-005-dispatch.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)). The configuration in which
the field is harmless — the plan's path fixed, only the branch and the working
tree moving — needs two things this project has not got: a work root that does
not follow the checkout, and a validator that refuses the pairings that are not
that one. Offering the field today would therefore ship the unsafe shape as the
default one and the safe shape as unreachable.

**The scope of `{id_slug}`'s guarantee is narrower than it looks, and did not
settle this.** What [§FS-005-dispatch.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs) fixes is construction and collision: what
a rendering answers may depend on the matter it is about and on no other matter,
which is why the digest is unconditional and why two ids that read down to one
slug stay two. That sentence is about *other matters*, not about a field of this
matter moving over time, so the proposal was not refused by it. The rule it fails
is the one recorded here.

**What a retitle would actually cost, which is why re-render stability outweighed
an explicit readability option.** The temptation is to offer the field anyway,
with a warning, to a reader who accepts the risk. The risk is not a second
working tree. It is a second **plan** for one matter: the first is left on disk
with whatever it held, the ledger is repointed at the second
([§FS-005-dispatch.4](../../functional-spec/FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)), both roots go on being enumerated and both go on being
returned as due, and the guard against two runs colliding is keyed on the
checkout rather than on the matter, so it holds neither. Nothing announces any of
it, and the person who pays has done nothing but rename an issue. An option whose
failure is silent, unattended and paid by someone who did not choose it is not an
option a reader can meaningfully accept.

**One architectural cost, recorded as a reason and not as a change.** With no
exact handle to recover a matter by, *is this matter already on a branch?* falls
through to the matching engine and is answered by resemblance between the new
title and the old branch ([§AR-003-attribution.1](../../architecture/AR-003-attribution.md#1-evidence)) — which is the thing `{id_slug}`
was added to stop doing. That point is untouched by this record; it is named here
because a field whose readable half drifts puts weight back on it.

**Readability is not refused with the field.** `{number}` is the readable name
wherever a matter has one and stays the right choice there
([§FS-005-dispatch.2](../../functional-spec/FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)); prose may name `{title}` freely, which is where a person
reading a ticket meets the title anyway; and what `{id_slug}` promises — a name
for the matter that has nothing else — is a different promise from a name that
reads well.

## 2. The boundary: three names the rule does not hold of today

The rule above is not kept by the vocabulary as it stands. Three names in it can
be put in a path today, and each can move while the work is open:

- **`{state}`**, and it is the worst of the three, because it needs no
  coincidence: the state change is both what moves the rendered name and what
  wakes the sweep that renders it again ([§FS-005-dispatch.5](../../functional-spec/FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)).
- **`{meta.<key>}`**, which is as free as a title and freer in one way — its
  values are chosen by the matter's own source rather than by ephor, and nothing
  anywhere promises they keep ([§FS-005-dispatch.8](../../functional-spec/FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)).
- **`{title}`**. It is commonly described as refused in a branch, and it is
  refused only for a title git's own ref grammar will not take: the branch
  template is rendered and the rendering is then held to that grammar, with no
  title-specific check between the two. A title of *Widen the retry window* fails
  on its spaces; a title of *Backoff* renders `fix/Backoff`, mints, and moves on
  the next retitle.

They are named here as exceptions the rule has not yet reached, not as precedent
for the next one. Each is a name somebody may be rendering today, so closing the
gap is a change of its own with its own account of who breaks; this record's job
is that the gap is written down where the rule is, rather than found by a reader
who then reads the rule as decoration.
