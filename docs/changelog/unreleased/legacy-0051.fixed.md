- **Stable custom-status answer matters keep their source activity and
  finality across refreshes**
  ([§FS-006-project-interface.4](../../functional-spec/FS-006-project-interface.md#4-the-answer-envelope),
  [§FS-003-feed-categories.2](../../functional-spec/FS-003-feed-categories.md#2-recent),
  [§FS-005-dispatch.5](../../functional-spec/FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)).
  A supplied matter time is now the activity snapshot, while an omitted time
  retains the same provider-slot/key observation's first-seen or earlier
  supplied value. Successful disappearance ends that observation, failure
  keeps the stale last-good one, and both explicit terminal values govern
  finality before state spelling. Unchanged refreshes therefore leave current
  work alone, while later source activity and state movement still reopen it.
  (PR #94)
