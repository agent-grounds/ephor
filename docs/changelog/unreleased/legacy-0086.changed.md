- **Work that edits the change, about a matter on no branch, is refused on the
  command line instead of written at the project root**
  ([§FS-005-dispatch.25](../../functional-spec/FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs),
  [§FS-005-dispatch.6](../../functional-spec/FS-005-dispatch.md#6-dispatch-is-offered-where-it-would-work-and-refuses-where-it-would-not),
  PR #13). A recipe with `needs_checkout` or an entry with `requires_checkout`,
  dispatched about a matter no branch could be found for on a project whose
  checkouts are one per branch, used to resolve its work root from the project
  root — which on such a project is the directory those workspaces sit in and
  holds no change to edit, so an agent started there stood in the wrong tree.
  It now refuses, naming `branch` as the way out. The menu has always blocked
  exactly that entry, so this is the two surfaces coming to agree
  ([§REQ-002-parity.2](../../requirements/REQ-002-parity.md#2-parity-runs-both-ways)). Everything with a branch of its own, every project
  keeping one checkout at its root, and all work that reads the change rather
  than editing it are placed exactly as before.
