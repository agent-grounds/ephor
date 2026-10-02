- **Periodic work sync leaves the detached runs it starts alive when its
  oneshot ends**
  ([§FS-005-dispatch.20](../../functional-spec/FS-005-dispatch.md#20-a-run-of-the-runtime-starts-beneath-the-screen-and-is-watched-by-attaching),
  [§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).
  Runs started while synchronization reopens opted-in work and by the final
  due sweep now survive service completion, explicit stop, and restart;
  stopping one still uses the runtime's displayed command. The process-only
  policy is unit-wide, so an explicit service stop also leaves any other
  descendant already spawned by the current ephor command to finish. (PR #97)
