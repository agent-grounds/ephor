- **A refresh no longer takes the screen**
  ([§FS-001-forge-interface.7](../../functional-spec/FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)).
  `r` ran the whole fetch on the thread that draws and reads keys, so the
  interface froze until the last provider answered — nothing repainted, no key
  was read, and `^C` is a key event in raw mode, so there was no way out
  either. Projects are asked one after another and a provider may set its own
  ceiling, so a site with one forge allowed ten minutes could hold the screen
  for the length of a coffee break. The run now lives on a thread of its own:
  the screen stays yours for all of it, each project takes its place in the
  feed as its sources answer rather than the whole run landing at the pace of
  the slowest one, and the header carries `Refreshing <project> (3/7)…` so a
  live screen is not read as a finished one. Pressing `r` during a run says so
  instead of starting a second. What a refresh costs the forge is unchanged —
  the projects are still asked one at a time, and it is the waiting that moved,
  not the load.
