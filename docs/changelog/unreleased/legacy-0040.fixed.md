- **A recipe over a project's own tasks that mints a checkout each no longer
  feeds itself**
  ([§FS-006-project-interface.7](../../functional-spec/FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live),
  [§FS-005-dispatch.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)).
  A `sources: ["rhei"]` recipe naming `{id_slug}` minted a workspace per task,
  each workspace got a task store of its own, the dispatch wrote its plan inside
  that store, and the next refresh read those plans back as fresh tasks of the
  same project — matched by the same recipe, minting again, until the filesystem
  objected. The ledger could not stop it: it keys work per item, and every
  turn's items were new ones the turn before had created. The tasks seam now
  yields no matter for a plan ephor caused to exist, absolutely, with no depth
  and no recipe key to relax it. Which plans those are is read from what ephor
  wrote on disk beside them and never from the ledger, in two marks: the dossier
  block in a plan ephor authored, and ephor's own hidden corner
  `.ephor/<plan id>/` beside a plan the runtime rendered for it. A plan the
  project wrote that a dispatch merely appended a ticket to stays a task and
  keeps all of its tasks, including the appended one — appending is not causing
  a plan to exist. One user-visible consequence: a store already holding plans
  ephor wrote loses those rows from **Tasks** on the next refresh. That work is
  unchanged and stays where it is watched, on the work screen and the operations
  board, which still see every plan a work root holds whoever wrote it.
  (PR #138)
