- **`--all` belongs to the verbs that read it.** It was a global flag with two
  meanings: "every branch entry rather than only the active ones" to
  `validate`, `ensure-agents` and `update`, and "every project" to
  `mark-read`. It is now declared on each of those four verbs and says there
  what that verb means by it, so `ephor validate --all` and `ephor mark-read
  --all` are unchanged while `ephor --all validate` — the flag before the verb
  — is no longer accepted, and no other verb advertises it. `mark-read --all`
  is narrowed by `--org`, `--tag` and `--workspace` like every other project
  selection, and prunes the read-marks of vanished items only when the sweep
  really covered every watched project. (PR #58)
