- **`github-notifications`: the source whose job is to be exhaustive**
  ([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities), manual §5.2).
  Every other source asks a question you composed, and a question never asked
  looks on screen exactly like one answered "nothing". This one asks nothing —
  it reads GitHub's own notification list and reports what is on it: team
  mentions (`@acme/reviewers` names you, and no search qualifier returns that),
  discussions, releases, advisories, invitations, and pull requests in
  repositories you never configured. One paginated call per refresh. It is the
  difference between an empty feed meaning "nothing is waiting" and meaning
  "nothing is waiting in what I was told to look at". `reasons` keeps it
  readable — the default is the set that means somebody is waiting on you by
  name, and `assign` is off because on a busy repository assignment is a bulk
  mechanism and what is genuinely assigned already arrives through `github-prs`
  and `github-issues` with its conversation attached.
