- **A project's own issues are read where they live, and the rung is named
  for what it holds**
  ([§FS-006-project-interface.7](../../functional-spec/FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)).
  The default work root is `{workspace}/panta`, so dispatch on a
  branch-addressable project writes its plans into the branch's own tree — and
  the reader looked only at the forest root, so it never read them back. A
  project with nine stores on disk reported holding none and showed none of
  that work in its feed, one of them a ticket parked in `needs-human`; the
  single-checkout case worked only because workspace and root are the same
  directory there. Stores are verified **on disk** now, at the root and in
  every branch workspace that has one — the row names the branches somebody
  wrote down, and the work is wherever branches were actually checked out,
  which are not the same list. And `ephor checkout` initializes a store in a
  workspace it makes, so the first dispatch has somewhere to land. That is not
  an artifact required of the project: the store ignores itself, so what it
  holds is ephor's own planning state that happens to live in a checkout
  ([§REQ-001-boundary.3](../../requirements/REQ-001-boundary.md#3-requirements-on-a-project-are-capabilities-never-artifacts)).
