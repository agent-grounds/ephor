- **The replay onto the published copy has its environment spelling**
  ([§FS-004-quick-actions.8](../../functional-spec/FS-004-quick-actions.md#8-a-branch-that-trails-its-own-published-copy-is-offered-the-rebase-onto-it)).
  Every argument of `ephor rebase` could arrive as an environment variable —
  which is how a state machine passes `{meta.*}` — except the one that picks
  the other rebase, so a program state could ask for `--onto` via `ONTO` and
  could not ask for the published-copy replay at all. `UPSTREAM` set to any
  non-empty value now asks for it. And because the flag parser's
  `--upstream`/`--onto` conflict cannot see the environment, the same refusal
  is repeated across both spellings: asked for together in any combination,
  the rebase refuses rather than silently preferring one and running a
  different rebase than the state asked for.
