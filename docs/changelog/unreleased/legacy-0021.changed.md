- **The TUI is supported over SSH, with browser and window actions kept in
  front of the remote reader**
  ([§FS-016-browser-opening](../../functional-spec/FS-016-browser-opening.md#fs-016-browser-opening-a-browser-action-reaches-the-reader-or-leaves-the-address-with-them),
  [§FS-005-dispatch.22](../../functional-spec/FS-005-dispatch.md#22-a-window-of-the-readers-own-where-one-is-bound)).
  Browser opening is now a bound seam: `defaults.browser` accepts the shipped
  opener, a custom `{url}` command, or `false`, with omission selecting the
  local graphical default. Automatic opening is bypassed under SSH and without
  a display; every bypass or failure restores the terminal with the complete
  URL and waits for Enter. Invocation and capture share a five-second deadline,
  even when a detached descendant holds an output pipe. Outcomes describe the
  shell invocation: every nonzero code, including 126/127 for downstream exec
  failures, reports `failed (<code>)`; `could not start` is reserved for
  preparation/spawn failure, and later execution errors say `execution failed`.
  Exit zero claims only that the opener exited successfully. Automatic window
  recognition now keeps remote tmux but suppresses inherited WezTerm and kitty
  markers under SSH. Explicit browser and window bindings remain authoritative,
  which is how deliberate GUI forwarding is preserved. (PR #105)
