- **`ephor checkout --help` now says that `--from` takes a branch name on the
  project's remote**
  ([§FS-004-quick-actions.7.4](../../functional-spec/FS-004-quick-actions.md#74-what-a-branch-is-grown-from-is-a-branch-on-the-projects-remote)).
  The flag's entry described `FROM` as what a branch the repository does not
  have is grown "from this", naming no kind of value, so a reader who knows git
  reached for `origin/main`. That is itself a legal branch name, so nothing
  refused it: ephor supplied the remote, looked for `origin/origin/main`, and
  the checkout was refused for a branch nobody asked about. The entry now says
  it takes a branch name on the project's remote and that the remote is
  ephor's to supply. What `--from` accepts is unchanged — the bare branch name
  it always took, with `HEAD` and a revision expression refused as before.
  (PR #86)
