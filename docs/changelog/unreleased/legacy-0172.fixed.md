- **Branch templates that need a field absent from a matter no longer turn a
  match into a dispatch refusal**
  ([§FS-005-dispatch.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs),
  [§FS-005-dispatch.27](../../functional-spec/FS-005-dispatch.md#27-an-offer-that-a-selector-refused-says-why)).
  The entry does not serve that matter: menus and readings withhold both agent
  and workflow entries, dispatch sweeps step over recipes and autorunning
  workflows without recording a refusal, and `ephor work offers` explains
  which field the branch template needed. Templates with author errors remain
  visible and refuse by name. (PR #51)
