- **A project declared only by `update_mode: skip` rows keeps the role the
  registry gave it**
  ([§AR-004-forest.2](../../architecture/AR-004-forest.md#2-probes-not-declarations),
  [§FS-006-project-interface.8](../../functional-spec/FS-006-project-interface.md#8-the-checkout-contract)).
  A site that keeps a checkout by hand writes `skip` on every row, and ephor
  dropped every such row before building the forest — so the project resolved as
  though it had declared nothing. It lost the `role` each row carried and was
  called *the checkout itself*; `ephor branches` read a nested `fix/issue-9`
  checkout as the branch `fix`, because an undeclared layout accepts any
  directory merely containing a repository; and a directory holding none of the
  project's repositories was refused with *no repository of this project is in
  it* rather than by name. A row saying `skip` says not to **update** the
  repository, which is `ephor update`'s business, so it still declares it: such
  a project now has a declared forest, keeps its roles, reads its nested
  checkouts whole, and is refused with *ce, ee not on disk there*. A project
  mixing `branch` and `skip` rows folds over the rows it tracks and no others,
  exactly as before, so no vendored tree joins a checkout that is whole without
  it. `ephor update` skips exactly the rows it skipped. (PR #147)
