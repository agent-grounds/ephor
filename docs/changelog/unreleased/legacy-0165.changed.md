- **ephor binds its own gate, so it holds every rung of its own ladder**
  ([§FS-006-project-interface.6](../../functional-spec/FS-006-project-interface.md#6-the-gate-is-the-projects-in-three-verbs)).
  `scripts/gate-status.sh`, `scripts/gate-failures.sh` and
  `scripts/gate-restart.sh` ask GitHub Actions what the gate is doing, what
  failed, and to run the failures again; `ephor.json` binds them as the three
  gate verbs. The shipped forge default would have answered too, but only for a
  matter it has cached — which is a pull request, and this project is worked by
  pushing to a branch as often as by opening one. The verbs ask about a commit
  instead, so the question is answerable from the checkout alone, on a branch
  with no pull request on it. A forge that cannot be reached is refused rather
  than reported green: silence and a clean gate have the same shape, and only
  the forge's exit code tells them apart.
