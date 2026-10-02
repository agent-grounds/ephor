- **`github-prs` no longer needs a repository list, and no longer hides
  finished work.** `repos` is now optional: empty searches the whole forge, as
  `github-issues` already did, bounded by `updated_within_days` (30) instead —
  a pull request in a repository nobody configured is yours just as much as one
  on your own. Closed and merged pull requests come back too and land under
  Recent; a question asked of you does not stop being asked when the branch
  lands. Nothing further is fetched about a finished one, so the extra coverage
  costs no extra API calls. `reviews` now defaults to **on**.
