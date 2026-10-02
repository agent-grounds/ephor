- **Autorun sweeps can cap live work globally and per project**
  ([§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).
  `work.max_concurrent` sets an aggregate ceiling and
  `projects.<id>.work.max_concurrent` adds a project ceiling inside it;
  omission stays unlimited and zero starts no new runs. `ephor work run
  --due --max-concurrent N` overrides the aggregate ceiling for one sweep
  while retaining project ceilings. Existing live roots consume capacity,
  failed or already-finished launches leave room for the next ranked root,
  and eligible roots omitted only because capacity is full are reported as
  non-failing `passed-over` outcomes in prose and JSON. (PR #25)
