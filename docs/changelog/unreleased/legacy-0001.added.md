- **The organization is handed over with the matter.** A summons now carries
  `EPHOR_ORG` and `EPHOR_ORG_ROOT` — the organization the registry places the
  project in and where that organization is rooted — and a ticket carries the
  same two facts as `org` and `org_root` beside `project`, so a program in a
  state machine, a project's own `custom-status` command and a quick action
  reach what a site keeps per organization without being told it again in every
  recipe. The two halves say a gap differently, on purpose: a summons defines
  both names always and leaves them empty, because a name ephor does not set is
  inherited from the shell that launched it rather than absent, so test with
  `[ -n "$EPHOR_ORG" ]` and never `${EPHOR_ORG:?}`; a ticket writes no key for a
  value it has not got, the way a matter with no branch carries no `branch`.
  Membership and the root are absent independently, so an organization that
  declares no `root` still names itself. One observable change for anyone
  already exporting these names: a command ephor summons now reads ephor's
  answer rather than the launching shell's. And because these two names now
  join the list a ticket's own metadata is written from, a watched task store
  that keeps its own `meta` key spelled `org` or `org_root` has it subtracted
  on the way in like every other name ephor writes there, rather than carried
  as `{meta.org}`. (PR #136)
