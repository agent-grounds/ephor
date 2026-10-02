- **A conflict becomes work, and nothing else does**
  ([§FS-005-dispatch.12](../../functional-spec/FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)).
  Replaying a branch is a fetch and a rebase; paying a model to type those is
  paying for something a script does the same way every time. So `ephor
  rebase` runs first and exits `3` where it stopped, leaving the repository
  mid-rebase — the state resolving it needs — and `--dispatch` opens the
  ticket about that conflict on the spot, carrying which files and which two
  sides. A clean replay opens no ticket at all. The shipped `rebase` recipe
  and a new `behind` recipe selector go with it, and
  `config/ci-green.example.states.yaml` wires `rebase → resolve-conflicts →
  verify-rebase → land-rebase`, where landing forces `--force-with-lease`
  because a replayed branch cannot fast-forward.
