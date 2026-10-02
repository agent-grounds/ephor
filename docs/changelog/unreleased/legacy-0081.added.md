- **Work about a matter with no branch can mint the branch it needs**
  ([§FS-005-dispatch.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs),
  [§FS-004-quick-actions.7](../../functional-spec/FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout),
  PR #13). An entry that hands work over may now say which branch that work
  belongs on: `"branch": "fix/issue-{number}"`, on a configured action carrying
  `agent` or `workflow`, a project's own offer naming a workflow, the entry
  beside a workflow, or a recipe. It is a template rendered from the matter
  exactly as a brief is — and refused by name, before anything is made, where
  it names `{branch}`, `{workspace}` or `{reply}`, which are what it produces,
  something that is no field of a matter at all, or a field this matter has not
  got, and where what it renders is not a name git will take as a branch. The matter's own branch always wins: a pull request keeps the
  branch the forge recorded, and the template applies only where there is none.
  Rendering it *is* the resolution and nothing is written down, so a second
  dispatch about the same matter lands in the same workspace, and one already
  on disk is worked in as it stands. Saying it means the work needs the
  checkout, and the dispatch makes that workspace with the operation
  `ephor checkout` already is — one implementation with a third caller, not a
  second copy of it — after the hand is chosen, the machine vetted and the
  inputs answered, so a refusal still leaves nothing behind. The machine vetted
  is the one in force where the work root is already there and the one ephor
  would install where it is not, since a workspace minted in order to read its
  machine back is the thing left behind. One case escapes that, and says so: a
  runtime that installs a machine of its own when it is asked to make the store
  installs it inside the workspace just made, so a refusal on it outlives the
  mint — and it names the workspace it left, and nothing further is made behind
  it. A dry run makes
  nothing at all, including the work root and the files the runtime would be
  shown, and names the branch and the directory it would have made instead.
  Nothing is written to the registry and nothing is pushed; a project with no
  `branch_root_template` is refused by name. The offer follows on both
  surfaces: such an entry stands on a branch-less matter in the *will check out
  first* shape, and the reading carries the branch and the workspace — on the
  menu, in `ephor actions`, and on the workflows sub-screen, which is one
  reading of the same menu. A surface asking where that work would go asks
  about the same workspace: the hand shown on a row and the roster its picker
  offers are read at the work root the dispatch will use, which for such an
  entry is inside the workspace it names.
