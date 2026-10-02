- **The shipped CI workflows could not run for anyone but ephor**
  ([§FS-009-shipped-actions.1](../../functional-spec/FS-009-shipped-actions.md#1-the-set)). Inside a
  reusable workflow a relative `uses: ./.github/actions/…` resolves against
  the *caller's* checkout, so `ephor-check.yml` and `ephor-validate.yml`
  failed for every repository that wired them in. They fetch their own steps
  at the version the caller pinned. The composite-action form was unaffected.
