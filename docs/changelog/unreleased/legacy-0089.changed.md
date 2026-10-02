- **The workspace a row says is missing is made from that row, and what ephor
  makes is a place work can go**
  ([§FS-004-quick-actions.7.1](../../functional-spec/FS-004-quick-actions.md#71-a-workspace-that-is-there-is-still-owed-its-store),
  [§FS-004-quick-actions.7.2](../../functional-spec/FS-004-quick-actions.md#72-the-offer-is-a-key-on-the-row-that-says-the-workspace-is-missing),
  [§FS-006-project-interface.7](../../functional-spec/FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live),
  PR #6). A row that says `∅ not checked out` now answers **`C`** with the
  checkout, on the matter's row, on the work rows beneath it, and on the
  branch's own — including the rows under *(not linked to a branch)*, which is
  where a matter whose branch nobody has checked out usually sits. It runs the
  same entry the `x` menu holds, the project's own `checkout` command
  included, so the key and the entry cannot come apart; the footer teaches it
  only where the workspace is actually missing.

  **And the workspace it makes is one the runtime can be handed work in.**
  ephor used to write the work store's files itself out of its own idea of what
  a runtime project is. It now asks the **runner** for one — `<runner> init
  --here <work root>` — at the work root ephor resolves rather than wherever
  the runner would put it left to itself, and installs ephor's state machine
  beside what the runner wrote. The store still ignores itself, so `git status`
  in the checkout is unchanged by it, whatever the runner's own project says
  about version control. Where the runner is not on `PATH` the checkout is
  unharmed: ephor writes the store it can and says what it could not do.

  **A workspace that was already there is owed the same.** *Already checked
  out* answered the question about repositories and not the one about work, so
  a workspace made before ephor made stores at all — or made by a project's own
  `checkout` command — held every repository it should and had nowhere for a
  plan to land, with nothing to do about it. Asking for the checkout again now
  makes whatever is missing and says so, and `--json` carries the same answer
  under `store`.
