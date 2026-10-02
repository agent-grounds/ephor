- **`ephor rebase` honours a scope selector, and joins the `--act` gate**
  ([§FS-011-command-line.9](../../functional-spec/FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused),
  [§FS-011-command-line.10](../../functional-spec/FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act),
  [§FS-005-dispatch.12](../../functional-spec/FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)).
  `ephor rebase --org X`, `--workspace X` and `--tag X` exited 2 as refused
  selectors and now sweep, so anyone reading that exit code as a permanent
  refusal is answered differently; it is the one verb in the honouring
  enumeration on a condition. A bare `ephor rebase` inside a checkout is byte
  for byte what it was. `--project` keeps its meaning — which project the one
  checkout belongs to — and is refused by name beside a selector, as are
  `--checkout`, `--item`, `--dispatch`, `--hand`, `--onto` and `--upstream`;
  every one of those command lines already exited 2 for the selector alone, so
  nothing that works today gains a refusal. The gate is counted differently for
  it than for `work dispatch` and its siblings: a verb whose pre-rule unit was
  narrower than one project is above the gate the moment it sweeps, at any
  width, because `rebase`'s byte-for-byte was one checkout rather than one
  project. Underneath, the disposition of a conflict is now an argument to the
  single replay rather than a fixed rule — *leave*, which every caller that
  exists today passes and which changes nothing, or *restore*, which only the
  sweep asks for — and a repository found already stopped in a rebase is
  touched under neither. (PR #89)
