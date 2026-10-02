- **A site may write a spend ceiling beside the concurrency ones, and the
  autorun sweep stops where it says**
  ([§FS-015-spend-ceiling.1](../../functional-spec/FS-015-spend-ceiling.md#1-two-ceilings-in-the-three-scopes-the-registry-already-nests),
  [§FS-015-spend-ceiling.5](../../functional-spec/FS-015-spend-ceiling.md#5-every-ceiling-is-evaluated-and-the-outermost-full-one-is-the-reason),
  [§FS-015-spend-ceiling.6](../../functional-spec/FS-015-spend-ceiling.md#6-only-the-sweep-is-bound-and-the-persons-key-never-is),
  [§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).
  `max_spend` and `max_tokens` sit beside `max_concurrent` and `max_active` at
  the same three scopes — the site's `work`, `organizations.<id>.work`, and
  `projects.<id>.work` — omission unlimited and `0` a pause exactly as the
  concurrency ceilings already read. The ceiling binds on what `ephor burn`
  already measures rather than a new price book of ephor's own, so it sees a
  run started by hand as well as one ephor dispatched, and it may be
  denominated in dollars or in tokens because a dollar can go quietly missing
  where a token count cannot. Every ceiling — roots in flight, working roots,
  money, tokens — is evaluated together and the outermost full scope is the
  one named; a window nothing was priced in binds no dollars, but the token
  ceiling over it still can. Only the sweeps nobody asked for are bound —
  `ephor work run --due` and the sweep at the end of `ephor work sync` — and
  a person is never refused for budget: `work dispatch`, `work lay`, a named
  `work run`, and the interface key all warn on stderr and proceed. While a
  ceiling binds, `ephor status` says so in one line naming the scope and the
  resume instant, in text and as `autorun_paused` under `--json`. (PR #74)
