- **A value the command line was given is used or refused, never quietly
  dropped, and the checkout settles where the workspace goes before it makes
  anything**
  ([§FS-004-quick-actions.7.3](../../functional-spec/FS-004-quick-actions.md#73-where-the-workspace-goes-is-decided-before-anything-is-made),
  [§FS-011-command-line.9](../../functional-spec/FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)).
  `ephor checkout` and `ephor rebase` put every flag and environment spelling
  through a branch-name guard and turned whatever failed it into *nothing
  given*: `--branch '{branch}'` was answered with "nothing says which branch to
  check out", so the caller who passed a branch was told they had passed none,
  and `CHECKOUT='{workspace}'` quietly became "rebase the working directory".
  Each is now refused naming the input it came in on — the flag, or the
  environment name a program state set it under — quoting what it held and
  exiting 2, with the refusal on standard output as an outcome under `--json`.
  This is a behaviour change for a caller who was passing values ephor ignored,
  and the point of it: an unfilled `{meta.branch}` in a program state now stops
  the run instead of running a different one. It reaches one value ephor was
  honouring rather than ignoring: `--from` (`FROM`) is documented as a branch
  and is now held to being one, so `HEAD` or a revision expression such as
  `HEAD~1` is refused instead of being passed to git, which had been resolving
  `origin/HEAD` for it. Absent, and empty once trimmed,
  are still nothing given and still fall through, which is what a state machine
  handing a program `BRANCH: "{meta.branch}"` about a branchless matter depends
  on. `ephor checkout` also settles the branch name before the first directory
  can be created: git will take it, the path it renders to stays inside the area
  the project's `branch_root_template` puts branch workspaces in, and it is
  neither the project's work root nor inside it. `--branch '../escaped'` used to
  leave a directory above the project root while git was still being asked, and
  `--branch panta`, on a project whose work root renders beside its checkouts
  rather than inside each of them, put a working tree on top of the work root
  and exited zero. The shipped `{workspace}/panta` renders inside each workspace
  and cannot collide, so a project on the default configuration keeps `panta` as
  an ordinary branch name. (PR #68)
