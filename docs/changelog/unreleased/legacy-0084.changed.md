- **The shipped `implement` recipe isolates branch-less issue work**
  ([§FS-005-dispatch.1](../../functional-spec/FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for),
  [§FS-005-dispatch.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs), PR #38).
  With no configured replacement, an issue with no branch is dispatched on
  `fix/issue-{number}` inside a workspace minted when the project has
  `branch_root_template`; without branch workspaces it is refused by name with
  configuration guidance instead of writing at the project root. Existing
  matter-branch precedence and configured `implement` replacements are
  unchanged.
