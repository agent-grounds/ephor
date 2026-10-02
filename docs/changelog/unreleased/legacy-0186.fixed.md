- **A project whose type declares a base per repository showed no branch a
  distance, and was never offered the rebase**
  ([§AR-004-forest.2](../../architecture/AR-004-forest.md#2-probes-not-declarations)).
  Such a type writes that base as a template — `{branch}`, expanded per branch
  workspace — and the forest carried it into the fold verbatim, so every
  repository was measured against a ref literally called `{branch}`, which no
  repository has. Every count came back unmeasurable, the checkout's total was
  *nothing to ask* rather than a number, no branch row carried a distance, and
  the quick action offered only on a branch that has fallen behind
  ([§FS-004-quick-actions.6](../../functional-spec/FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase))
  was therefore never offered on that project at all — silently, because a row
  showing no count and a row that is up to date look the same. A declared base
  that is still a template is passed over now, falling through to the project's
  main branch and then to what the repository's own remote calls its default:
  on the tree this was found in, one branch went from no count to
  `33 behind (ce 17, ee 16)`, and twenty-four workspaces that had never been
  measured are measured. The remote went the same way — it was the literal
  `origin` in five places, and is read off each repository once where the
  layout is already probed (the branch's own upstream, else the sole remote,
  else `origin`), so a clone whose remote is called something else is fetched,
  measured and replayed like any other, and the reports name the ref they used.
