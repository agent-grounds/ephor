- **The replay a person watches is prose, and it calls every repository what
  the registry calls it**
  ([§FS-011-command-line.11.1](../../functional-spec/FS-011-command-line.md#111-what-a-terminal-is-handed-carries-no-markup-it-does-not-render),
  [§FS-011-command-line.11.2](../../functional-spec/FS-011-command-line.md#112-a-report-names-a-repository-for-its-reader)).
  `ephor rebase` printed its markdown document straight to the terminal — a `#`
  headline, a `##` section per repository whose whole text was often the `.`
  path, and two rows of backticks around what git said — and so did the sweep
  that wraps it. Both now hand a terminal prose: a headline, one indented line
  per repository, and git's own words indented under the line they belong to.
  Every repository is named by its role, failing that by the handle its registry
  row gave it, and only failing that by its path — in the replay, in the sweep,
  in the report a repository the disk has not got gets, and in the ticket a
  conflict opens, which is the third reader of the same report and now goes
  through the one flattening rule the other two do. A replay nested inside the
  sweep's report takes the form of the document carrying it rather than its own,
  so the sweep's file nests markdown and the sweep's terminal nests prose. The
  document is not lost: `--report <path>` and the `report` field of `--json`
  keep their markdown form, and `repos[].repo` keeps the path a program opens.
  Their per-repository headings do move, because the rule above is the
  document's too — a section that read `## . — fix/x` now reads
  `## the project — fix/x` — so anything that was reading a heading for a path
  must read `repos[].repo` instead. (PR #147)
