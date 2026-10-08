# DF-006-every-hinted-list-is-read-by-presence: a registry row's word on every list a checkout can hint is its presence, and the row names its own addresses

**Status:** Accepted
**Date:** 2026-10-08

A registry row says which aliases, territory, addresses and rooms are its
project's by writing the list. A list it writes is final, `[]` included, and
only a row without the key adopts what the checkout's manifest hints
([§FS-008-attribution.1.1](../../functional-spec/FS-008-attribution.md#11-the-rows-word-on-a-hinted-list-is-its-presence)). The row gains the `addresses` field it lacked. This
is the rule rooms already followed, extended to every list a checkout can hint,
so the row is authoritative over the manifest wherever the manifest can speak
([§FS-006-project-interface.2](../../functional-spec/FS-006-project-interface.md#2-the-manifest-is-offered-never-required), [§REQ-001-boundary.2](../../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)).

This record was written because a row could not refuse a checkout's claim. A
project's `ephor.json`, which anyone with push access to its repository can
edit, hinted the alias `the widget`, the organization `acme`, the address
`alice@example.org` and a room. A row that wrote `[]` for all four still
adopted the alias, the organization and the address, and refused only the
room. A row that wrote its own lists got its own alias, territory and room, and
still got Alice's address, because nothing read a row's `addresses`. Every
message author is address evidence and places at the strength of a reference,
and a hinted organization places at the strength of a venue. So whoever could
edit a repository's manifest decided where the owner's mail went, and the owner
could not refuse it from their own registry. A misplaced mail is a reason to
sweep by hand again ([§GOAL-003-nothing-lost](../../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)). Both gaps had been named and
left for later by [§DF-003-chat-is-a-forge-capability.3](DF-003-chat-is-a-forge-capability.md#3-deferred) and
[§DF-005-a-row-is-a-sources-fallback-home.3](DF-005-a-row-is-a-sources-fallback-home.md#3-deferred). What follows is what was decided,
what was weighed and refused, and what was named and left for later, so that a
reader who finds one of these absent meets a reason rather than an oversight
([§REQ-002-parity.2](../../requirements/REQ-002-parity.md#2-parity-runs-both-ways)).

## 1. Decided

**Presence, on all four lists.** For aliases, territory, addresses and rooms,
a list the row writes is the row's, and `[]` says *none*. Only a row without
the key adopts the manifest's hint, and adopts it whole. The rule holds one
list at a time, so a row that refuses one hint keeps the manifest's other
hints, its checks, its gate and its actions. Trust decides only whether there
is a hint at all, so a list present on the row wins at every trust level.

**A row `addresses` field.** It is matched as written and places at the
strength of a reference, exactly as a hinted address does
([§FS-008-attribution.3](../../functional-spec/FS-008-attribution.md#3-venue-beats-reference-beats-fallback-beats-resemblance)). The schema declares it as a list of non-empty
strings, and a row that writes anything else is refused by name, as one with
such `rooms` is. Read as silent, it would let the checkout's hint stand in for
what the row tried to say. DF-005's reason for not listing correspondents to
home a personal inbox still holds: an address is a reference, not a fallback.

**`fallback_sources` keeps `[]` as absence.** The presence rule exists so that
a row can refuse a hint, and no checkout can hint a source's name
([§DF-005-a-row-is-a-sources-fallback-home.1](DF-005-a-row-is-a-sources-fallback-home.md#1-decided)). That decision stands.

**`repos` stays the layout's.** The schema requires every row to have a type
and every type to have a non-empty layout, so on any registry that loads, the
layout speaks for the row and a manifest's `identity.repos` is never adopted.
There is nothing for a row to refuse. A row that wants to claim repositories
beyond its forest already can, under `territory`, which places them as a forest
repository would.

**The registry schema is versioned, as `…/schemas/registry/v2`.** A row that
wrote `"aliases": []` or `"territory": []` adopted the hint before this record
and refuses it after, on an unchanged JSON shape. That is an incompatible
change ([§FS-006-project-interface.11](../../functional-spec/FS-006-project-interface.md#11-the-interface-is-versioned)), and the release's schema diff does not
see a change to a description ([§FS-002-release.1.4](../../functional-spec/FS-002-release.md#14-compatibility-notices-from-the-previous-tag-onwards)). So the registry schema
takes the `$id` `https://github.com/agent-grounds/ephor/schemas/registry/v2`,
the unversioned schema counting as the first version, and the release notices
the marker. The descriptions state the rule, so `ephor schema registry`
documents it. Whoever breaks fixes it by deleting the key. A row that wrote
`addresses` gets what it wrote. Rows that write none of these keys, and every
manifest, are unaffected.

## 2. Rejected

**A row `repos` list read by presence.** Every existing row would count as
silent on it, so on upgrade every row would start adopting its manifest's
`identity.repos` at the strength of a venue, the strongest rung. That is the
widening this record exists to stop.

**A second meaning for the type's `repos[]` layout.** The layout is set per
type and holds checkout paths, so every project of a type would claim the same
repositories.

**A row `addresses` that keeps the emptiness rule.** A row could replace the
manifest's addresses with its own, but a row that wants none would still adopt
them through `[]`. That is the trap rooms were taken out of.

**A *none* sentinel, such as `null` or `["!"]`.** It invents syntax where
presence already works for rooms and is documented for them.

**`manifest_trust: "ignore"` as the remedy.** It works, and refuses every hint
at once. It also drops everything else the manifest offers, its checks, gate
and actions, so it cannot refuse one list and keep the rest.

**A `doctor` or validation note on a refusal.** A refusal is the row doing what
it was written to do, and a note on every deliberate refusal would never go
away.

## 3. Deferred

This is named and not done; nothing above needs it.

- Forest repositories as identity. The layout feeds checkout paths such as `.`
  into identity, and no conversation carries a path, so a forest repository
  places nothing. The likely remedy is to read each repository's fetch remote,
  which is already probed ([§AR-004-forest.2](../../architecture/AR-004-forest.md#2-probes-not-declarations)). Until then, a repository a row
  wants placed goes in its territory.
