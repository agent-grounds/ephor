- **Deciding what a pull request means moved out of `github-prs` and into
  policy**, where every forge gets the same treatment
  ([§FS-001-forge-interface.3](../../functional-spec/FS-001-forge-interface.md#3-policy-lives-above-the-interface-never-in-an-implementation)).
  The provider reports roles, reasons, conversation, and gate; `role`, the
  displayed state, and `needs_response` are composed above it.
