- **Being asked is now a reason a pull request is yours**
  ([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities)). `github-prs`
  searched `--author`, `--commenter`, and `--mentions` — all three of which
  find pull requests you have *already spoken in*. A review requested of you and
  a pull request assigned to you leave nothing behind in the conversation, so
  they looked exactly like work that was none of your business. Both are now
  searched (`--review-requested`, `--assignee`), every reason a pull request is
  yours rides on the item as `raw.reasons`, and a review asked for and not yet
  given needs a response on its own — no thread rule can find that one.
