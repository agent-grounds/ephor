- **Tickets a project keeps in its checkout are read where they live**
  ([§FS-006-project-interface.7](../../functional-spec/FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)).
  A `panta/` plan directory — or one the manifest points at — is now a source
  like any other: its open tasks arrive in the feed beside what the forges
  reported, under the store's own ids, attributed to the checkout's project
  because a store in a checkout is about that checkout and nothing has to
  guess. `.beads/` is recognized and reserved; a store ephor can see but
  cannot read yet reports nothing rather than pretending. Declaring a store
  does not hide a probed one — a project may keep two, and both are read — and
  the *ticketed* rung counts a declared store as well as a well-known name.
