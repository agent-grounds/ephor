- `doctor` says what it is doing while it does it
  ([§FS-010-doctor.3](../../functional-spec/FS-010-doctor.md#3-two-passes-the-site-and-ephor-itself)).
  Asking every source of every project takes as long as the slowest forge, and
  the first version printed nothing at all until it was finished — a minute of
  silence that reads as hung, which is the same failure the tool exists to
  name with ephor as the source that did not answer. The site pass announces
  each project before it asks and answers it when it comes back, on the error
  stream so that what a program reads stays the report; the self pass narrates
  by being incremental instead, each check printing its own line as it
  finishes.
