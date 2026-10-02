- **The cursor follows the row it was on across a rebuild**, rather than the
  line number that row happened to occupy
  ([§FS-001-forge-interface.7](../../functional-spec/FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)).
  Selection was kept by index, which was harmless while the tree only changed
  when you asked it to; with answers arriving underneath a reader who is still
  moving, an index is the wrong thing to keep — rows sort in above the cursor
  and `x` opens the menu for a matter you were not looking at. A row that is
  gone still leaves the index standing, so marking a pile done walks down it as
  before.
