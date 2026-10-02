- **The dossier's message budget was a total that could be exceeded**
  ([§FS-005-dispatch.2](../../functional-spec/FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)).
  Two messages were reserved per thread *before* the total was applied, so
  twenty threads quoted forty messages against a budget of twenty-four. The
  reservation is spent out of the total now, and a thread the budget cannot
  reach is counted as dropped rather than silently omitted from the tally.
