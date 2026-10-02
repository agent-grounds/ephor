- **A ledger entry whose work is a plan a workflow laid is an entry with work**
  ([§FS-005-dispatch.35](../../functional-spec/FS-005-dispatch.md#35-what-a-ledger-entry-may-be-forgotten-for-is-read-from-the-plans)).
  The status reading counted tickets over the plans ephor wrote itself, so a
  matter whose only dispatch is a workflow reported zero open tickets whatever
  its laid plan on disk said. It now counts every plan the record says is that
  matter's: `work forget --done` no longer untracks work that is running,
  `work list --open` shows it, and `work sync` says which plan it is going in
  and what its task is at instead of calling the matter dormant and offering a
  command that would drop it. A laid plan nobody can read is not a finished
  one either — the entry reports `⚠ plan missing`, `sync` says what its work
  came to is unknown, and `--missing` is the verb both name — while a plan
  ephor wrote and lost keeps the rule it had. Neither sweep drops an entry
  that still has something open, so a matter with one plan unreadable and
  another going is untracked by `--item` and by nothing else; its row says
  both facts, and a run over it still resolves its hand from the plans that
  did read. (PR #135)
