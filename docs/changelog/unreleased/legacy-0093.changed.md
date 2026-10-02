- **A window of the reader's own, where one is bound**
  ([§FS-005-dispatch.22](../../functional-spec/FS-005-dispatch.md#22-a-window-of-the-readers-own-where-one-is-bound),
  [§AR-002-summons.6](../../architecture/AR-002-summons.md#6-windowed-the-readers-own-window),
  [§DA-007-window-is-a-bound-opener](../../decisions/architectural/DA-007-window-is-a-bound-opener.md#da-007-window-is-a-bound-opener-a-window-of-the-readers-own-is-a-bound-opener-with-the-terminal-as-the-floor),
  PR #4). ephor has one terminal and is sitting in it, and handing it over
  stays the floor — but a reader inside a multiplexer, or in a terminal that
  opens windows on request, had to make the better move by hand. The window is
  now a **seam**: two commands, one that opens a window running a given command
  and prints a handle, one that brings a handle forward. `window` under
  `defaults` in `status.json` names which — a shipped binding (a terminal
  multiplexer's new window, and two terminals' remote-control spawn) or a pair
  of commands of your own — and with nothing configured ephor recognizes the
  environment it was started in from the variable each product sets for exactly
  this, never by spawning one to find out. Nothing bound and nothing recognized
  means the terminal, with the line saying so.

  An action or an offer that *is* a program you type into says `"window": true`
  and runs there instead of taking the terminal: a job like any other, with the
  lock as its liveness and the handle in its record, so it is a row that says
  *running* and opens to the program rather than something ephor handed the
  terminal to and forgot. Such a job leaves **no log** — what its program wrote
  is on a screen you were watching and is not duplicated into a file — so the
  window is its inspection, and every surface says so rather than offering an
  empty file. Attaching to a run goes through the same opener, from the key and
  from `ephor actions open` / `ephor operations attach` alike. ephor opens a
  window and brings one forward; it never closes one and never ends what is in
  it.
