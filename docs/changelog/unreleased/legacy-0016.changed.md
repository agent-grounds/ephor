- **A checkout report reaches a terminal as prose and a file as markdown.**
  `ephor checkout` prints what it made as plain sentences — a headline, one
  indented line per repository, and git's own words indented under the ones it
  refused — and the note `ephor work dispatch` prints beside an item whose
  workspace could not be made carries that same prose rather than a markdown
  document folded onto a one-line prefix. Every report also names a repository
  by its role, failing that by the handle its declaration gave it (the registry
  row's `id`, a manifest's `name`), and only failing that by its path, so a
  project whose one repository is the root of its checkout no longer heads a
  section with a full stop. The markdown form is unchanged and is where it was
  declared to be: the file `--report <path>` writes and the `report` field of
  `--json`, whose `repos[].repo` goes on carrying the path a program opens
  ([§FS-011-command-line.11](../../functional-spec/FS-011-command-line.md#11-a-report-reaches-a-terminal-as-prose-and-a-file-as-markdown)).
  (PR #133)
