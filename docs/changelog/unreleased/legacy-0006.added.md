- **A recipe's brief may be kept in the file that owns it.** `brief_file` names
  that file: a path template rendered from the same vocabulary a work root is
  rendered from, read when the ticket is written, and its text is the brief. A
  standing instruction — how work is done under an organization, a house review
  checklist — stays in the document with an owner and a history instead of
  being pasted into `status.json` and re-pasted whenever it changes. `brief`
  keeps its whole meaning and both keys compose, in that order: the file's
  words first, the rendered `brief` after them. Nothing inside the file is
  substituted, so a document whose own example names `{title}` reaches the
  ticket as those seven characters, and its headings arrive flattened like any
  other embedded document. `{reply}` is not among the names a path may take; a
  relative path is relative to the directory holding the configuration file
  that wrote it, never the working directory. Every writer of a brief reads it
  — the menu previews fall back where the file cannot be read, the dry run and
  the dispatch refuse naming the path before a workspace, a work root or a plan
  is made, and the unattended rebase sweep reads it too, with its path held to
  the names a checkout can answer and a conflict still reported with the reason
  on its row. Each ticket records the path it read and a sha256 of the bytes as
  read in its own `metadata:`, so a reader holding the file can say whether it
  got these words or older ones. An entry that asks for work says `brief_file`
  the same way, and a recipe or entry writing neither key is refused where the
  configuration loads, naming it
  ([§FS-005-dispatch.34](../../functional-spec/FS-005-dispatch.md#34-a-brief-may-be-kept-in-the-file-that-owns-it)).
  (PR #124)
