- **The rebase is offered on every branch that is here, and every distance says
  how fresh it is**
  ([§FS-004-quick-actions.6](../../functional-spec/FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase),
  [§FS-004-quick-actions.8](../../functional-spec/FS-004-quick-actions.md#8-a-branch-that-trails-its-own-published-copy-is-offered-the-rebase-onto-it)).
  A workspace whose repositories were last fetched nineteen days ago read
  `✓ checked out · up to date` and offered no rebase, while master had moved
  the whole time: the count is measured against the *last-fetched*
  `origin/<main>` and nothing in the watch fetches, so the offer was gated on a
  stale reading of the one move that would have refreshed it. Both entries —
  onto the main branch and onto the branch's own published copy — are now
  offered wherever there is something to replay onto, level or behind; a level
  branch replays onto nothing and is told so, in the register a current
  repository is always told it in. Rows and entries read `13 behind as of
  Jul 28` and `level as of Jul 28`, the day taken from the ref's own reflog —
  the last time that copy actually moved here, which a fetch that found nothing
  and a fetch that failed to connect both leave alone — per repository, oldest
  wins across a forest, and omitted entirely where no day was ever recorded.
  *Up to date* is retired: it claimed the branch was current when all that was
  measured was a match against a copy of unstated age. The `behind` recipe
  selector is unchanged and still means *behind*: dispatching a level rebase to
  an agent is a ticket to do nothing, while pressing the key runs git first
  ([§FS-005-dispatch.12](../../functional-spec/FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)).
