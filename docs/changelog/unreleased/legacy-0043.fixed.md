- **The acting sweep heads each root by the outcome it reached**
  ([§FS-005-dispatch.24.1](../../functional-spec/FS-005-dispatch.md#241-the-sweep-announces-each-root-by-the-outcome-it-reached)).
  `ephor work run --due --act` printed `▶ <runtime> <root>` above every root
  it walked, so a root passed over for capacity and a root whose launch the
  runner refused each opened with the marker that means a run began, and only
  the line underneath said otherwise. The header is now chosen after the
  outcome is known: a root that started keeps the pair it had, a root that was
  passed over is headed by the pass-over, and a refused launch by the refusal.
  What made each root due stays under it in every case, and `--json` and the
  gated report are untouched — both already answered by the outcome. (PR #134)
