- **A carried-over plan no longer leaves the records that name it behind**
  ([§FS-005-dispatch.3.1](../../functional-spec/FS-005-dispatch.md#31-a-plan-named-before-the-digest-is-carried-over)).
  Carrying a plan named before the digest over to the stem its matter's id
  renders now moved the files and nothing else, so every reference keyed to the
  old stem was left naming something that was no longer there: the plan's own
  result block, and the ordering and the consumed export of every plan beside it
  that waited on this one. The runtime refuses the **root** it is asked to
  validate rather than the one plan, so one carried-over matter took every plan
  in the work root down — silently, because the move itself reported success and
  the damage surfaced at some later command. The carry-over now rewrites those
  references in the same commit as the renames, and only them: a stem in prose
  or in a title is a sentence that is still true and is left alone. Two things
  the move set missed come with it — an export one ticket handed another, now
  found by sweeping every directory the runtime keeps rather than the two ephor
  reads itself, and the plan's writer sidecar, which is named after the plan's
  path and so is renamed with it rather than removed — and a stale sidecar
  already at the new name is replaced, because an empty lock file holds no work
  and holding a matter back for one is a refusal nobody could act on. A
  reference that cannot be written stops its own entry exactly as an unmovable
  file does, and the refusal names the file; a plan source that cannot be *read*
  stops nothing and is said instead, because a source ephor cannot read holds no
  reference the runtime reads either. What was carried over is reported whenever
  a reader loses a path, in the bytes as much as in a name: an entry that only
  rewrote a reference says so. (PR #144)
