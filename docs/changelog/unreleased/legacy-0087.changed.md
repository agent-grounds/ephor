- **A ticket a run has in hand says so on its own row**
  ([§FS-005-dispatch.23](../../functional-spec/FS-005-dispatch.md#23-work-stands-on-rows-of-its-own-beneath-the-row-it-is-about),
  PR #6). *Open* and *being worked on right now* were one yellow `⚙`: the
  ticket an agent was inside of and the ticket nothing had picked up looked
  identical, and telling them apart meant leaving for the board. A ticket a
  live run holds is now `▶` in green, one it will reach says `· queued`, and a
  live run that has gone silent carries `· quiet Nm` — the board's own words,
  narrowed to one matter rather than invented a second time. It asks nothing
  new of the world: the liveness is the lock the watch already probes and the
  holding is the run's own record ([§FS-005-dispatch.15.2](../../functional-spec/FS-005-dispatch.md#152-what-a-run-is-doing-is-read-from-the-runs-own-stream)), taken once per work
  root across the whole feed rather than once per matter. `running`, `queued`,
  and `quiet` join the readings too, so a command sees what a row does.
