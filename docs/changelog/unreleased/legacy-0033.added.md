- **Dispatch autoruns accept per-invocation runtime arguments**
  ([§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself),
  [§FS-011-command-line.8](../../functional-spec/FS-011-command-line.md#8-what-is-going-is-said-and-the-way-in-is-printed)).
  Arguments after `--` on `ephor work dispatch` now reach every autorun that
  dispatch starts, unchanged and in order. Ephor neither interprets nor stores
  them, so dispatches without arguments and later sweeps keep their previous
  behavior. (PR #70)
