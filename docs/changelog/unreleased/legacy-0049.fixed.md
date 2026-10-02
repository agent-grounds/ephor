- **A failed ledger save no longer leaves undiscoverable work behind**
  ([§FS-005-dispatch.4](../../functional-spec/FS-005-dispatch.md#4-the-ledger-is-ephors-record-and-never-the-truth-about-the-work)).
  Hand-off mutations are journalled until the atomic ledger replacement
  commits; failure restores the whole unsaved batch, including prior plan and
  root-file bytes, workflow output and carried files. Root discovery now keeps
  every recorded placement for listing, runs and repeat detection. (PR #101)
