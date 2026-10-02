- **A project can speak for itself, and the interface is published**
  ([§FS-006-project-interface.2](../../functional-spec/FS-006-project-interface.md#2-the-manifest-is-offered-never-required),
  [§FS-006-project-interface.11](../../functional-spec/FS-006-project-interface.md#11-the-interface-is-versioned),
  manual §4.2.1). ephor reads `ephor.json` at a forest root: identity hints,
  the forest's own layout, check and gate verbs, ticket stores, and offers.
  Offered, never required — every field optional, `{}` valid, and a project
  that places nothing is watched exactly as it stands. Two rules keep it safe:
  the registry row is authoritative over identity hints, adopting them only
  where it says nothing itself, and the row's new **`manifest_trust`** decides
  how much is believed at all — `full`, `descriptions` (read what it says
  about itself, run none of it), or `ignore`. Resolution everywhere is site
  configuration over manifest over probe, in one shared lookup rather than
  per caller. The three schemas are printable — `ephor schema
  manifest|answer|registry|forge` — and a manifest is checkable where it sits
  with `ephor validate --manifest .`, offline: no schema refers to another by
  URL, so a project can validate what it says with no ephor present.
