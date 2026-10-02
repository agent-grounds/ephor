- **A source follows a label**
  ([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities)).
  `github-issues` could ask two questions — the issues you opened, the issues
  you are in — and neither reaches a repository where the work you follow is
  named by the project rather than by you: on one with 518 open issues the
  role searches flood, and the 17 labelled `priority` you actually watch
  include ones nobody has spoken in, which is exactly what those searches
  filter out. A block now takes `labels`, and each label is one `gh search
  issues --label <name> --state open` — open only, since following a label is
  following work and the closed would spend the `limit` on history. What it
  finds arrives whoever is in it, under the role its author gives it: **My
  Issues** where you opened it, **Participating** otherwise. `authored` joins
  `participating` as a switch, so a source can follow labels alone; a block
  with both off and no label asks nothing and is refused when it is read,
  rather than answering "nothing" forever. And a label search that comes back
  with exactly `limit` issues fails the source, naming the count, the label,
  and the two ways out — it delivered a prefix nobody can size, and a queue
  shown as a fraction of itself reads as the queue
  ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).
  The two role searches are otherwise unchanged, and still truncate silently
  at their own limit.
