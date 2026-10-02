- **`refresh` accepts the project selector used by sibling sweeps**
  ([§FS-011-command-line.9](../../functional-spec/FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)).
  Repeatable `--project PROJECT` selectors now reach the same refresh path as
  the compatible positional project list instead of being rejected by the
  argument parser with an escaping tip. Mixed named and positional inputs form
  one de-duplicated set, still constrained by `--workspace`, `--tag` and
  `--org`, with unknown, unwatched and out-of-scope projects refused. (PR #69)
