- **A summoned command was handed paths its own shell could not read**
  ([§FS-006-project-interface.3](../../functional-spec/FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)).
  A summons runs through a shell, so `$EPHOR_ANSWER` and the rest are strings
  that shell parses before anything opens them — and where the native separator
  is the shell's own escape character, a path handed over verbatim stops being
  a path. `cat > "$EPHOR_ANSWER"` wrote somewhere else, or nowhere, and the
  answer came back empty with nothing saying why, which is exactly the silence
  [§REQ-001-boundary.1](../../requirements/REQ-001-boundary.md#1-the-anatomy) refuses: every structured answer from every check verb,
  gate verb and offer was lost on Windows, and lost quietly. Path-valued
  variables use `/` between their segments on every platform now. Where the two
  spellings already agree this is the identity, so nothing changes anywhere
  else.
