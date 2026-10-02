- [§FS-002-release.5](../../functional-spec/FS-002-release.md#5-between-releases-main-carries-a-dev-version):
  after a release, main opens the next patch as `X.Y.Z-dev` instead of keeping
  the version it just published. A build from main previously reported the tag it
  was already ahead of, so a merged-but-uninstalled fix looked exactly like an
  installed one. (PR #53)
