- **A registry match to the project's main branch is not a matter's own
  branch, for placement**
  ([§FS-005-dispatch.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs),
  [§FS-005-dispatch.1](../../functional-spec/FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for), PR #40).
  An issue or pull request the registry matched only to the configured
  `main_branch` — the trunk every workspace is grown from — used to keep that
  branch like any other of the matter's own, so a `branch` template was
  skipped and dispatch resolved inside the main checkout itself. Such a
  matter is now placed exactly as a branch-less one is: a `branch` template
  mints its own workspace and the offer follows on both surfaces, and work
  that edits the change with no template is refused — naming the main branch
  it declined rather than calling the matter unmatched. Work that only reads
  the change — a reply, a review, anything needing no checkout — still runs
  where the matter's code is, the main checkout included, and lays its ticket
  beside the project all the same: laying a plan is a write, and every write
  is placed. A forge-recorded branch that happens to equal `main_branch` is
  the forge's own fact and keeps winning.
