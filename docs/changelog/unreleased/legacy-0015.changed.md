- **Every plan file `work dispatch` has written changes name once, and ephor
  moves it for you**
  ([§FS-005-dispatch.2](../../functional-spec/FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it),
  [§FS-005-dispatch.3.1](../../functional-spec/FS-005-dispatch.md#31-a-plan-named-before-the-digest-is-carried-over)).
  A plan's stem *is* what `{id_slug}` renders, held additionally to the runtime's
  rule that a file stem begins with a letter, so
  `github-prs-acme-widget-42.rhei.md` is now
  `github-prs-acme-widget-42-922ddbdc.rhei.md`. The first time after the upgrade
  that ephor writes in a root — a `work dispatch`, a `work lay`, or a sweep
  starting a run — a plan named there before the digest is carried over: the plan
  file, the results and the artifacts named after its old stem, and ephor's own
  record of the name all move together, and what moved is said on the command's
  output and in `--json`. Reading moves nothing, and neither does a `--dry-run`
  or a sweep the `--act` gate is holding; a dry run over a root that is behind
  still reports the ticket the real dispatch will write, and says the move it is
  reporting across. Each root moves on its own, so one that cannot be reached
  leaves the record and the disk agreeing and holds back only the matters it
  names. A root a run is holding waits for the next such command; a workflow
  already laid keeps the name it was recorded under, because nothing recomputes
  it. Three shapes are refused by name rather than resolved, because each is two
  records of work only a person can separate: one matter holding a plan at the
  digested name *and* at a pre-digest name, which only a mixed pair of binaries
  can produce; one plan file that two matters are both recorded at, which is
  what the naming this release fixes left wherever it fired; and a carry-over
  that would write over a file already at the new name, because a rename never
  destroys what is there — so the results and the artifacts a mixed pair leaves
  at both stems are held and named the way the plan file is, rather than lost on
  the next write verb by a reader doing what the first refusal told them. A workflow laid from
  now on takes its digest over the matter and the entry as a pair, since joining
  two ids with a `-` before a reduction that collapses punctuation to a `-` is no
  name for a pair. (PR #137)
