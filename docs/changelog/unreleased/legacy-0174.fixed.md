- **A refresh no longer spends more of the forge's search allowance than the
  forge will give**
  ([§FS-001-forge-interface.8](../../functional-spec/FS-001-forge-interface.md#8-a-refresh-is-asked-in-the-cheapest-form-the-forge-offers)).
  `github-prs` asked one search per role, per repository, per project, and
  `github-issues` asked two or more of its own — so a registry of seven tracked
  projects spent forty-five requests of GitHub's thirty-a-minute search
  allowance on every refresh. It never failed outright: the sources asked first
  answered and the ones asked last were refused, so the feed came back short by
  a different amount each run, for a reason nowhere in it. Every role is now one
  aliased search in a **single** GraphQL request per source — the graph is
  metered five thousand points an hour and a whole request costs one — and the
  repositories a source watches ride in that one question rather than one
  question each. Nothing about the answer changed: each role is still its own
  search under its own alias, so a pull request still arrives with every reason
  it is the reader's ([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities)).

  **A pull request's branch and review decision arrive with it.** They were a
  second request each, one per pull request the reader authored, on top of the
  searches; the graph hands them over with the row that needed them
  ([§FS-001-forge-interface.8.3](../../functional-spec/FS-001-forge-interface.md#83-what-is-already-in-hand-is-not-asked-for-again)).
