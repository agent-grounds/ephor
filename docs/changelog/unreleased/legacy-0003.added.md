- **`{id_slug}` joins the placeholder vocabulary: a matter's own id as a name a
  branch and a path will take, so a recipe can mint one checkout per task.**
  Most of that vocabulary is the forge's, and a project's own task carries none
  of it — no `{number}`, no `{repo}`, and `{project}`, `{source}`, `{kind}` and
  `{state}` the same for every task of the project — so every branch template
  that could tell one task from another was refused and the one that was
  accepted minted a single shared branch for all of them. `{id_slug}` is the id
  lowercased over its ASCII alphanumerics with every other run collapsed to a
  single `-`, and an eight-digit digest of the whole id appended: unconditional,
  because rendering *is* the resolution and nothing is written down, so two ids
  that read down to one slug stay two branches rather than two unrelated matters
  landing in one tree. It is never withheld, because every matter has an id, and
  what it renders is always a name git will take. It reaches the brief and the
  work-root template too, so a per-matter work root is expressible for a matter
  with no `{number}`. No shipped recipe names it — `implement` keeps
  `fix/issue-{number}` — and `{number}` stays the better choice wherever the
  matter has one
  ([§FS-005-dispatch.2](../../functional-spec/FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it),
  [.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs))
  (PR #129).
