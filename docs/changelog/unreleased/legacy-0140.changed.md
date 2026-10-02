- **The rebase ephor already knew you needed**
  ([§FS-004-quick-actions.6](../../functional-spec/FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase)).
  The inbox has always said `3 behind` on a branch row and then left you to go
  elsewhere about it. Now a pull request whose branch workspace is on disk and
  trails its `main_branch` is offered **`⤴ rebase onto <main> (N behind)`** in
  its action menu, and `ephor rebase` is the command behind it: fetch and
  replay every repository in the checkout, an answer per repository, no forge
  and no vendor CLI anywhere in it. Uncommitted work is reported and left
  alone rather than stashed.
