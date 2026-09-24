# DA-010-work-is-admitted-whole: work that needs several pools at once is admitted whole, or nothing is written at all

**Status:** Accepted
**Date:** 2026-09-24

[§DA-009-headroom-vetoes](DA-009-headroom-vetoes.md#da-009-headroom-vetoes-headroom-is-a-report-ephor-is-given-and-it-vetoes-a-member-rather-than-ranking-the-list)
settled what capacity may do to a list: it may strike a member out and it may
never reorder, and where every member is struck out the first still gets the
ticket and waits. That disposition is deliberate and it is right for what it
covers — one target, several alternates, a survivor to fall back to and a
person who can start the ticket by hand.

It is the wrong disposition for a different shape of work, and the shape exists:
a workflow whose targets are answered separately can need **two providers at
once**, and a list has nothing to say about that. The question is not *which of
these members* but *can this work be done here at all*, and it has no survivor
to fall back to. This record fixes the answer for that shape — the work is
admitted whole or not written at all — and records why the two dispositions
differ rather than pretending one rule covers both.

## 1. The decision

**A piece of work whose resolved hands are bought against two or more distinct
pools is admitted only when every one of those pools can be had.** Otherwise
nothing is written: no plan, no record of one, no claim on the matter. The
matter stays in the feed where a machine that has what it needs can take it
([§FS-005-dispatch.33](../../functional-spec/FS-005-dispatch.md#33-work-that-needs-several-pools-at-once-is-admitted-whole)).

**The requirement is derived, never declared.** It is the set of pools of the
hands ephor itself resolved for the work. Nothing is configured to acquire it,
which means it applies to work that already exists the moment it ships, and it
means answering a target with a hand on another pool changes what the work
needs along with it. A declared list was the alternative and is rejected below.

**It is the same evidence, asked a different question.** This consumes the
report [§DA-009-headroom-vetoes](DA-009-headroom-vetoes.md#da-009-headroom-vetoes-headroom-is-a-report-ephor-is-given-and-it-vetoes-a-member-rather-than-ranking-the-list)
already consumes and derives no quota of its own, so it opens no seam and owes
no new binding ([§REQ-001-boundary.1](../../requirements/REQ-001-boundary.md#1-the-anatomy)).
It asks that evidence one question about a piece of work rather than one
question about a member, and it still only ever refuses: nothing is ranked,
nothing is reordered, and no list is touched. **Unknown stays unknown** here for
the reason it is unknown there — absent is the ordinary case, and a rule that
read silence as exhaustion would hold everything.

**One rule, one clause, every door.** The requirement is answered in one place
and rendered in one sentence, which each surface wears with its own verb, so
the menu row, the laying and the unattended sweep cannot disagree
([§AR-005-capabilities.2](../../architecture/AR-005-capabilities.md#2-features-declare-needs)).
A gate at one door with three ways around it is worse than no gate.

## 2. Why the two dispositions differ

[§DA-009-headroom-vetoes](DA-009-headroom-vetoes.md#da-009-headroom-vetoes-headroom-is-a-report-ephor-is-given-and-it-vetoes-a-member-rather-than-ranking-the-list)
writes the ticket anyway because *a ticket that is written and waits is work a
person can see, start by hand, and reason about*. That argument holds exactly
where it was made and fails here, on both halves:

**There is nothing to fall back to.** An ordered list has a survivor by
construction — the first member — and writing the ticket to it means the work
is still described, still assigned, still startable the moment the window
lifts. Work needing two pools has no such member. Writing it down produces a
plan that cannot be run, only attempted.

**Writing it costs more than not writing it.** A laid plan is a claim: the
matter leaves the feed, the issue takes an assignee, and the machine that laid
it is the machine that owns it. So the half-run is not merely a plan that
waits — it spends attempts against a window it was already told was shut, and
it holds the matter away from a machine that could have finished it in one
pass. That is the opposite of what a ticket-and-wait buys, and it is what this
record exists to stop.

**A person is not at the terminal.** The disposition [§DA-009-headroom-vetoes](DA-009-headroom-vetoes.md#da-009-headroom-vetoes-headroom-is-a-report-ephor-is-given-and-it-vetoes-a-member-rather-than-ranking-the-list)
chose is a bet that somebody will see the waiting ticket. The case this record
covers is the unattended one, where nobody will. Where a person *is* present and
names the work themselves, this rule warns and does not hold, for the reason
[§FS-005-dispatch.30](../../functional-spec/FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work)
keeps their key everywhere else.

## 3. The rejected alternatives

**Feed the required pools through the veto as a list of alternates.** The
cheapest change by far: the machinery exists and takes a list. It also cannot
answer the question, because the veto always returns a member. Handing it
*anthropic* and *openai* gets back whichever of them survived, which is not an
answer to "can both be had" and would lay the plan in exactly the case this
record is about.

**Declare the pools on the entry.** The shape the report that produced this
first illustrated — a `pools` key beside the workflow entry. Rejected because
it is a second copy of something ephor already resolves, and a copy is wrong the
first time a target is answered differently: a `--set` naming a hand on another
provider would leave the declaration describing a requirement the work no
longer has. It also needs a rule for a declared pool the site has never heard
of, which a derived requirement cannot produce at all.

**Narrow it to a label a site opts into.** It would make the change invisible
to everyone who did not ask for it, which is genuinely safer. It also leaves
every site that did not ask paying the cost this record exists to remove, and
it puts the requirement in a place that cannot see which targets the work
actually resolved to. The behaviour change is accepted deliberately instead,
and recorded as a change rather than an addition.

**Check it in whatever lays the cross-family workflow.** The script that lays
an agora could probe both pools itself. That duplicates the decision, and
duplicates it in the one place that learns about the problem too late — after
the entry was chosen and about to be written — while leaving the menu row and
the unattended sweep saying something else.

## 4. The cost

**A single-family site holds such work indefinitely.** A machine with no hand on
one of the required pools has no window to wait for, so the work parks and
nothing lifts it. It parks visibly, in its own sentence rather than the spent
one, and the way out is to answer the target with a hand the site has — but it
is a stall where there used to be a run that got halfway, and somebody who
wanted the halfway will notice.

**It is a behaviour change to configuration nobody edited.** Because the
requirement is derived, every cross-family workflow on every site acquires it
on upgrade. That is the point, and it is still a surprise: a dispatch that used
to lay a plan now reports a hold.

**The requirement is only as good as the evidence.** A stale refusal in the
ledger holds work that would have run, and ephor has no way to check the claim —
the same trust [§DA-009-headroom-vetoes](DA-009-headroom-vetoes.md#da-009-headroom-vetoes-headroom-is-a-report-ephor-is-given-and-it-vetoes-a-member-rather-than-ranking-the-list)
accepts, with a heavier consequence, since a wrong report here stops the work
rather than moving it to the next name. The bound is that a refusal names the
instant it lifts and is evidence only until then, and that unknown holds
nothing.

**Two dispositions is one more thing to know.** A reader now has to know that
one spent pool means *written and waiting* for a single-target action and
*nothing written* for a multi-pool workflow. The cost is real; the alternative
was one rule that is wrong for one of the two.
