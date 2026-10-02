- A cache no refresh produced now says exactly that, rather than reporting
  every source as silent
  ([§FS-006-project-interface.10](../../functional-spec/FS-006-project-interface.md#10-capability-rung-by-rung)).
  The *observable* rung is "at least one source **answering**", and a refresh
  that has not run is not every source having failed: one is a command to run,
  the other is a credential or a network to go and look at. The case that
  actually bites is a cache stored under an older model, which is dropped on
  load and leaves a feed with no providers in it — every project on a site
  that predates the model bump read as totally silent. `fetched_at` is what
  tells them apart, and the rung names the remedy.
