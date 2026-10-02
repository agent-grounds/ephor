- **Quick actions**: the source that produced an item offers what it knows to
  do about it, so the action menu is worth pressing before anyone has
  configured it. A pull request whose gate is red gets `✗ see the CI failures`,
  and it leads the menu, ahead of the configured actions
  ([§FS-004-quick-actions](../../functional-spec/FS-004-quick-actions.md#fs-004-quick-actions-a-problem-ephor-recognizes-arrives-with-the-action-for-it)).
  The condition is the red gate rather than the kind of item, so the action is
  on the row that shows the red count whichever source reported it: `github-prs`
  and `github-ci` page the failing job's log through `gh`, and a forge that
  answers the new `failures` capability is asked directly.
