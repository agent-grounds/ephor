- **A finished matter could still be counted as awaiting an answer**
  ([§FS-003-feed-categories.2](../../functional-spec/FS-003-feed-categories.md#2-recent)). `forge::policy`
  settles each report it builds, but a merge folds two of them: a notice's
  state is the reason the forge sent it, never a terminal state, so nothing
  settled the thin report before it reached `absorb`, and a merged pull
  request the notice list also mentioned came back `finished` *and*
  `needs_response`. The model settles now — on the way in and after a merge —
  so the summary's Respond column, the unread counts and `status --check`
  stop counting work that is over.
