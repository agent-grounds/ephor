- **The project's own work is called a task, everywhere**
  ([§FS-006-project-interface.7](../../functional-spec/FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live),
  [§FS-003-feed-categories.1](../../functional-spec/FS-003-feed-categories.md#1-the-categories)).
  A **ticket** is what a remote tracker keys, an **issue** is what a forge
  files, and what a project keeps in its own checkout is neither — so it is a
  **task**, and one name for one thing ([§FS-001-forge-interface.3](../../functional-spec/FS-001-forge-interface.md#3-policy-lives-above-the-interface-never-in-an-implementation)). Tasks get
  a row of their own: a `task` kind and a **Tasks** category between
  Participating and Messages, in the navigator, the plain renderer and the
  manual's table, instead of sitting in **My Issues** among things other
  people filed. The rung is **tasks**; `requires: ["ticketed"]` and
  `requires: ["local-issues"]` still resolve, so nothing anybody already
  wrote stops meaning what it meant. The manifest key is `tasks`, with
  `tickets` still read as the older spelling — evolution by addition, so the
  schema gains the new key rather than breaking the old
  ([§FS-006-project-interface.11](../../functional-spec/FS-006-project-interface.md#11-the-interface-is-versioned)).
  The feed cache model is bumped, so a cache holding these as issues is
  rebuilt rather than shown stale. What is not one of these keeps its name:
  the ticket ephor writes to dispatch work and the ticket keys a forge is
  asked for are other things and are still called tickets.
