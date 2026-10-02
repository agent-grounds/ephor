- **Why there is no slugged title, written down.** A field that resolves a
  matter's branch or its work root now has a stated stability rule — it answers
  the same for as long as that matter's work lasts — and a decision point
  recording why a slugged title is refused
  ([§DF-002-path-fields-stable](../../decisions/functional/DF-002-path-fields-stable.md#df-002-path-fields-stable-a-field-that-decides-a-path-holds-still-while-the-work-lasts)). The rule is positional rather than about the
  name: in a dossier or a brief a field is a record of what the matter looked
  like when the work was asked for, and a value that has since moved is that
  record doing its job; in a path the same field *is* the resolution, because
  rendering is the lookup and nothing is written down, so a value that moved
  repoints live work at a second place while the first is still there. Both
  shapes that were offered for the field fail that rule — a digest of the title
  moves whole on a retitle, and a digest of the id holds one component still
  inside a string that changed anyway — and what the proposal did not meet is
  therefore a burden rather than an impossibility: a narrowed field whose unsafe
  uses are refused at dispatch is coherent and decidable from two template
  strings, so a later proposal starts from this record rather than being barred
  by it. Where the vocabulary is specified the rule now rides along as a
  condition on joining the half a path is rendered from
  ([§FS-005-dispatch.8](../../functional-spec/FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose), [§FS-005-dispatch.6.1](../../functional-spec/FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)), and the dispatch
  specification's account of what a retitle would cost is corrected
  ([§FS-005-dispatch.2](../../functional-spec/FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)) — it cited the wrong point, and described the effect as
  milder than it is: not merely a second workspace but a second **plan** for one
  matter, with the first left on disk, the ledger repointed at the second, both
  roots still enumerated and still returned as due, and the guard against two
  runs colliding keyed on the execution root rather than on the matter. **No field was
  added and nothing under `src/` changed**: `{title_slug}` is already refused by
  the unknown-name arm of the branch minter, and the end-to-end case that pins
  the offered vocabulary whole now names it, so it is a test that breaks the day
  the field joins. Three names already in the vocabulary — `{state}`,
  `{meta.<key>}` and `{title}` — move while the work is open and are recorded as
  exceptions the rule has not yet reached, explicitly not as precedent.
  (PR #145)
