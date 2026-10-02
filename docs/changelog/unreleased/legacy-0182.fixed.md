- **Keys and menu entries that could not do what they advertised**
  ([§FS-004-quick-actions.2](../../functional-spec/FS-004-quick-actions.md#2-offered-only-where-it-would-work)).
  Six of them, all the same shape — the offer was on the screen and the
  keystroke was refused. The checkout offered on a branch row ran
  `ephor checkout --item "$EPHOR_ITEM_ID"`, and a branch row has no matter
  behind it
  ([§FS-004-quick-actions.6](../../functional-spec/FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase)),
  so the one row that offer was added for answered *Nothing says which branch
  to check out*; it names the branch as well now, and either half may be empty.
  That row also pointed `EPHOR_WORKSPACE` at the directory the checkout had not
  made yet — it falls back to the project root, as an item's menu already did —
  and left `EPHOR_ITEM_ID` unset, which is not empty but whatever the shell
  that launched ephor happened to hold, so a stale one could have bound a
  branch's rebase to somebody else's change; it is now said, and said empty. An
  entry marked *(unavailable)* had no verb in the footer and still took the
  menu down when chosen, to repeat in the header the reason the row was already
  carrying: it now leaves the menu standing. The work screen advertised `R run
  the runtime` on machines with no runtime bound; the screen is told the
  runtime rung's answer
  ([§AR-005-capabilities.2](../../architecture/AR-005-capabilities.md#2-features-declare-needs))
  and drops the key, saying why if you knew it anyway — everything else on that
  screen goes on working, because writing the ticket and running it are
  different capabilities. And `; ops` was taught only by the navigator's
  footer though the key is read on every screen: the thread, gate, work and
  board footers say it too, and the thread's reaction picker is excluded, since
  a board opened over an armed picker leaves it armed underneath.
