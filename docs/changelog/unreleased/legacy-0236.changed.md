- **The rebase is offered wherever there is a branch**
  ([§FS-004-quick-actions.6](../../functional-spec/FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase)).
  Both replays were offered only on pull requests, and only on projects whose
  registry row named a `main_branch` — so an issue about the same change, a
  status a source filed about it, and every branch row in the detail view got
  nothing, and a project that names no main branch got nothing at all. What the
  offer is about is a branch on disk that trails something, never the kind of
  row that mentions it: any row resolving to a branch workspace now carries
  both entries, and so does the branch row itself, which is where the `13
  behind · ↓2` a reader is reacting to is actually written — `x` on it opens
  the same menu, built by the same code, carrying ephor's own offers only,
  since a source's, a project's and a person's entries are selected against an
  item a branch row does not have. The two replays are also gated apart: the
  one onto main has to name the branch it replays onto and is offered only
  where the project declares one, while the one onto the branch's published
  copy resolves its ref inside each repository and needs no such name, so a
  project with no `main_branch` is offered it — and its rows show `↓2` alone
  rather than nothing at all. Where a branch cannot be resolved to a checkout
  the offer is withheld rather than made and left to fail on the keystroke
  ([§FS-004-quick-actions.2](../../functional-spec/FS-004-quick-actions.md#2-offered-only-where-it-would-work)).
