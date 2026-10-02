- Rebased onto main after the move to the `agent-grounds` organization. Its
  end-to-end case was renumbered from `E2E-016` to `E2E-020`, because main took
  016 through 019 while this was open, and the new section in the command-line
  specification became §10 beside the scope-selector rule rather than replacing
  it. `due_over` now propagates the error `due_in` began returning. (PR #61)
