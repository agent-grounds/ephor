- **`github-prs` reads the whole conversation before deciding you answered.**
  Answered-detection looked at the conversation tab only, so a reply left on a
  line of the diff never counted and the citation stayed pending forever.
  Review threads are now fetched in the same call and count as answers. A
  mention is also matched as a whole handle rather than as a substring:
  `@vjovanovic` is no longer a mention of `@vjovanov`, and a team named in the
  conversation now cites everyone on it.
