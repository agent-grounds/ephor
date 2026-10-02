- **`ephor work offers` explains why a selector refused a recipe**
  ([§FS-005-dispatch.27](../../functional-spec/FS-005-dispatch.md#27-an-offer-that-a-selector-refused-says-why),
  [§FS-003-feed-categories.1](../../functional-spec/FS-003-feed-categories.md#1-the-categories),
  PR #22). A project's own tasks carry no role, so a `roles` selector — non-empty
  by definition once written — excluded every one of them the same as any other
  selector refusal: an offer that never appeared, and a "nothing matches this
  matter" that could not say why. `ephor work offers` now names every recipe
  considered for a matter whose selector refused it, and which field refused —
  the role-less case worded plainly: the matter carries no role, the selector
  asks for one. Both readings carry it in the same words: the JSON gains an
  additive `excluded` array beside `offers`, and the prose lists it under the
  same "nothing matches this matter" a reader already looks under. Selector
  semantics do not change — a role-less item still matches only an empty
  `roles`, and the dispatch dry run is unaffected. `task` is also added to the
  `--kind` help across the three commands that take it, the manual's selector
  table, and the `Selector` kinds rustdoc, since the feed already accepted and
  returned it undocumented.
