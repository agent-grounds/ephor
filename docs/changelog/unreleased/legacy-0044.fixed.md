- **The checkout a project declared is the maker on every path that makes a
  branch workspace**
  ([§FS-004-quick-actions.7](../../functional-spec/FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout),
  [§FS-006-project-interface.8](../../functional-spec/FS-006-project-interface.md#8-the-checkout-contract)).
  A project binds one `checkout` command — the documented way a site substitutes
  a sparse slice for ephor's plain `git worktree add` — and only the action chain
  ran it: `ephor checkout` typed by name and the workspace a dispatch mints both
  used ephor's git instead, silently, so a site whose repository needs a slice got
  a whole tree per dispatched issue and the agent that ran there stood in a tree
  the site deliberately did not want it to see. For an issue with no branch the
  dispatch is the only maker there is, so there was no way to get the checkout
  that was declared. The binding is now honoured inside the one checkout
  operation, so all three callers reach it: it is summoned only where the
  workspace is absent, with `EPHOR_BRANCH` as the branch being made and the
  matter's own names set from the matter where there is one — the ticket key of
  the registry branch it was matched to among them — and empty rather than absent
  where there is none, from the project's root, and `EPHOR_CHECKOUT_MAKING` lets a
  command wrap `ephor checkout` without summoning itself for ever. What it returns
  is verified — the directory and every declared repository — and a workspace it
  did not make is refused with its absent repositories named, nothing dispatched
  behind it and no store put into it; ephor's git never fills in a tree it did not
  make. That refusal holds of the bare directory it leaves behind, on whichever
  path asks next: it is named and refused again rather than read as checked out, so
  a second dispatch cannot put a plan in a tree holding none of the project's
  repositories, and a refused checkout writes `--report` as git's refusal always
  has. *Whichever path* is every branch workspace of the project — the one a
  `branch` template minted and the one a matter already on a branch resolves to
  alike, since they are the same directory to whoever asks the filesystem — and
  the refusal comes before the recipe's opening move rather than after it. The
  project's own checkout is never judged this way: a project that keeps one
  checkout at its root has no branch workspace to make, so a repository missing
  from that root refuses nothing. *Whole* is also one question rather than three,
  so a project whose declared forest is empty — every declared repository skipped
  — no longer has a bare directory refused on one ask and reported *already
  checked out* on the next. A workspace the
  command did make gets its work store, on the first ask as much as the second,
  because the plan a dispatch is writing lands there. `--from` is refused by name
  where a command is bound, the base being that command's to decide, and
  `ephor checkout` says which maker made the workspace in prose and as a `maker`
  field under `--json`. A project with no command bound is untouched. (PR #128)
