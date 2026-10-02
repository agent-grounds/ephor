- **CI no longer runs a Windows leg, and [§RM-004-windows](../../roadmap.md#rm-004-windows-ephor-runs-where-there-is-no-posix-shell)
  says what a port would take.** `windows-latest` was in the matrix from the
  first commit and was never once green: every test binary failed to compile,
  so nothing behind that had ever run. Compiling it took one line, and what
  came out from behind it is a port — an out-of-process forge extension is a
  shell script, and Windows cannot exec a file whose executability is a `#!`
  line, with `work`, `feed`, `forge_extension`, `checkout`, `update` and six
  e2e cases still unexamined behind that. Four real defects the compile break
  had been hiding are fixed and listed above. The leg is off until the rest is
  decided, because a leg that is always red is one nobody reads, and it costs
  the two that mean something.
