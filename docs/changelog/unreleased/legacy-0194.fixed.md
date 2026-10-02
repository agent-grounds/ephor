- **CI installed a `grund` that could not read this tree.** The pin was 0.7.0,
  which speaks init block v4, while the entrypoints carry v7 — so the gate
  failed on the documentation rather than on anything a change did. Pinned to
  0.9.0, and the pin now says what it is for.
