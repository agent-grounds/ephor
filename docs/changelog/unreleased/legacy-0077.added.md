- **A second autorun ceiling bounds the work an agent is actually doing**
  ([§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).
  `work.max_active` and `projects.<id>.work.max_active` mirror the
  `max_concurrent` pair over the live roots that are *working*, so the two
  questions a single number had to answer — how much may be spent at once,
  and how many worktrees and processes this machine may hold — can be set
  apart. A live root is **parked**, and outside `max_active`, when nothing in
  it is witnessed running and an open ticket in it waits on a person: a
  `gating` state, or a poll declaring `waiting_on`, which the runtime now
  lets a machine say. A root whose machine cannot be read counts as working,
  because a misreading must not hand out capacity. Omitting `max_active` is
  unlimited, so a configuration that names only `max_concurrent` starts
  exactly what it started before and is refused in exactly the words it was;
  `--max-concurrent N` still overrides the roots-in-flight ceiling and that
  one alone. A refusal now names the key it refused on, and the sweep's
  reading says how many roots are live and how many of those are parked, in
  prose and under `--json`, so a full ceiling never hides that a person is
  holding one of the slots. Ceilings gate starts, so parked runs resuming
  together can carry working roots above `max_active` until one finishes.
  (PR #56)
