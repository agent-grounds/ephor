- Recipes and action entries select on `assignees` and `labels`: a plain name
  is one the matter must carry, and `!name` one it must not, so
  `{"labels": ["enhancement", "!GenAI"], "assignees": ["kimeta"]}` takes a
  reader's own queue and leaves the automation's behind. A source that reported
  neither fact refuses both forms rather than matching on silence.
  An entry names something — `!` alone is refused, on the selector and in the
  published schema, because a field that asks for nothing is written by
  omitting it.
  `github-prs` and `github-issues` report both as fields on the search they
  already make, and the forge interface carries them on `issue` and
  `pull_request`
  ([§FS-005-dispatch.31](../../functional-spec/FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)).
  (PR #99)
