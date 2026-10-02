- **Rhei task stores include direct directory workspaces in the Tasks feed**
  ([§FS-006-project-interface.7](../../functional-spec/FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live),
  [§AR-007-runtime.1](../../architecture/AR-007-runtime.md#1-what-the-module-owns)).
  Alongside existing flat `.rhei.md` and `.panta.md` plans, ephor now reads a
  direct non-hidden workspace's `index.rhei.md` and `tasks/*.md`, keys its
  matters by the workspace directory, and uses its declared `states.yaml`
  before the store root or runtime default. An unreadable declared workspace
  machine fails the source instead of silently borrowing another machine.
  (PR #103)
