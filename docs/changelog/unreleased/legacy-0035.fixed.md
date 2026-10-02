- **A fenced block reaches a ticket as its author wrote it, nested fences and
  all**
  ([§FS-005-dispatch.3.2](../../functional-spec/FS-005-dispatch.md#32-what-is-already-fenced-is-what-the-plan-language-fences)).
  The flattening that turns an embedded document's headings into emphasis
  ended a fence at any line opening with three backticks, so a four-backtick
  block quoting a `markdown` example — a recipe's `brief_file` showing what a
  plan looks like — ended at the example's first fence, and the
  example's headings reached the ticket as `**…**`: a brief whose
  purpose is to show a plan's shape said a plan has no headings,
  and nothing failed. A `~~~` fence was not seen at all, and a
  ```` ```rust ```` line inside an open fence closed it. Ticket bodies now read
  a fence by the plan language's own rule: a run of three or more backticks or
  tildes opens one, only a bare run of the same character at least as long
  closes it, and one nothing closes runs to the end. (PR #151)
