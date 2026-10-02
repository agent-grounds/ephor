- **`ephor checkout` no longer claims a local-only branch tracks the branch
  the forge has**
  ([§FS-004-quick-actions.7](../../functional-spec/FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout),
  PR #8). A repository that already had the branch was checked out and
  reported as tracking the branch the forge has whether or not the forge
  actually had a copy of it — a local branch cut from the main branch and
  never pushed came out identical to one the forge actually published. Such a
  repository is now reported **published nowhere**, naming what its tracking
  configuration records instead of claiming a copy that is not there
  ([§DA-003-upstream-is-the-published-copy](../../decisions/architectural/DA-003-upstream-is-the-published-copy.md#da-003-upstream-is-the-published-copy-a-branchs-upstream-is-its-published-copy-not-its-tracking-config));
  a repository whose branch the forge does have is unaffected. The checkout
  changes no tracking configuration and pushes nothing, either way.
