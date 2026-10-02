- **Items arriving mid-refresh were filed under no branch**
  ([§FS-008-attribution.2](../../functional-spec/FS-008-attribution.md#2-two-stages-one-engine),
  [§FS-001-forge-interface.7](../../functional-spec/FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)).
  Each project takes its place in the feed as its own sources answer, and that
  landing did everything the full reload does except place the new items on
  their branches — a pass the tree stopped doing for itself when the answer was
  moved off the draw path. So everything a running refresh brought in sat under
  *(not linked to a branch)*, and the count on the branch above undercounted,
  until the whole run finished. The landing folds the placements too; it asks
  the world nothing, which is why it belongs in the cheap half.
