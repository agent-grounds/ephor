- **A mutating verb above one project reports, and acts only under `--act`**
  ([§FS-011-command-line.10](../../functional-spec/FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act)). Once a selector actually scopes a verb, `ephor
  work dispatch --org foundation` hands work over in every project the
  organization holds, and the only thing between that width and the act was
  the caller remembering `--dry-run`. Reading at any width is free; writing at
  a width above one checkout is a different kind of act. `work dispatch`,
  `work sync` and `work run` now report what they would do and write nothing
  when the **resolved** project set names more than one project, and the
  report names `--act`, the new global flag beside the scope selectors, as the
  word that does it. The report is each verb's own dry run; `work run`, which
  had none, prints the work roots and the tickets it would run. A turn that
  resolves to one project or one `--item` behaves exactly as before, and
  `--act` is refused by name with exit 2 — a JSON outcome under `--json`,
  like a refused selector — wherever the gate cannot fire, `update` and
  `ensure-agents` included: they sweep every managed workspace their scope
  reaches and are deliberately left outside the gate for now, which their
  refusal says rather than denying. **Callers who swept more than one project
  bare now get a report instead of an act** — `systemd/ephor-work-sync.service`
  passes `--act` on `work sync` and `work run --due` for exactly that reason.
  (PR #61)
