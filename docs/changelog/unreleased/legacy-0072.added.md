- **Burn — what this machine is spending on agents, as a page and a command**
  ([§FS-013-burn](../../functional-spec/FS-013-burn.md#fs-013-burn-what-this-machine-spends-on-agents-is-a-reading-like-any-other)).
  `$` from any screen opens the Burn page and `ephor burn [--window
  1h|6h|24h|7d] [--by project|model|session|plan|matter] [--rescan]` is the
  same reading on the command line, with the published `burn` shape under
  `--json`. Ephor had no cost surface at all: the agent command-line tools
  were already writing down every token and the runtime was already metering
  its own runs, and neither fact was reachable. **Two lenses, and nothing
  sums them** — the machine lens is built from the tools' own transcripts
  (`~/.claude/projects`, `~/.codex/sessions`) and is the ground truth for a
  total, while the work lens is built from the runtime's accounting records
  under each work root and reaches a matter through ephor's own ledger. A run
  measured by both appears in both, so adding them would double-count every
  run the runtime started, and the reading says which lens answered. The work
  lens carries how many invocations recorded no usage rather than letting a
  reader mistake a short number for a cheap plan. Ingest reads each log the
  one way that is right: only the outer counters of a call, never the
  per-iteration breakdown that restates them; the session counter diffed
  rather than summed, because it is cumulative; each delta attributed to the
  model in force at that event; and a dollar rollup filed under the model its
  own session called, so tokens and dollars are one row instead of two.
  A response written as one record per content block is charged once, under
  the request its records all name, so a reply that calls two tools is not
  billed three times. The runtime's records are read where it writes them —
  one directory per plan under each work root — and their cached counters are
  taken out of the input where the tool that reported them put them inside it
  and left alone where it put them beside it, since both shapes arrive and no
  field says which. A matter's burn reaches the plans a workflow laid down
  for it as well as the one its ledger entry wrote itself.
  Records are attributed to a project by matching the directory they ran in
  against the registry, longest match first, and anything under no registered
  root lands in `other` rather than being dropped. Scanning is incremental —
  a per-file cursor of offset, size and modification time, so the first pass
  is a backfill and every later one reads only what was appended — and what
  it reads is aggregated into five-minute buckets under
  `~/.local/state/ephor/burn/`, one file per day, thirty days kept, swept on
  the next scan. Neither surface scans while drawing: the page refreshes from
  the same tick that watches the work artifacts and the command refreshes
  inline only when the store is over thirty seconds stale, both local file
  reads, so `refresh` remains the only verb that asks the world. Tokens are
  the reading and dollars are opportunistic — ephor computes no prices and
  ships no price book — so **`unpriced` is never rendered as `$0.00`**, and
  `cost_usd: null` with `priced: false` stays distinct from a priced zero in
  the machine form. Deferred and written down rather than dropped: a site
  price book, per-invocation live detail, and drilling into a row. (PR #60)
