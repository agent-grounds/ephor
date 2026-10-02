- **Sources stop placing what they find; one engine does it**
  ([§FS-008-attribution](../../functional-spec/FS-008-attribution.md#fs-008-attribution-every-conversation-finds-its-project-or-says-that-it-could-not),
  [§AR-003-attribution](../../architecture/AR-003-attribution.md#ar-003-attribution-one-matching-engine-evidence-against-identity-at-two-scopes),
  manual §4.2). A source that asks nothing about any one project — GitHub's
  notification list, and mailboxes when they arrive — is declared at the top
  level in `sources` now and fetched **once** per refresh instead of once per
  project. Where each finding belongs is decided afterwards, by weighing what
  the conversation carries (its venue, the repository it is on, ticket keys and
  repositories named in what was said, who spoke) against what each registry
  row declares. Rows gained **`territory`**: repositories and organizations
  that are a project's business without being in its forest, either
  `"acme/plugin"` or a whole `"acme"`. That is what places the general case —
  a mention of you on some repository of the project's ecosystem, an issue
  filed there, none of it in any checkout.
  An explicit venue wins outright, a reference places next, resemblance only
  argues, and resemblance is whole words, so a project called `api` cannot
  claim "rapid". Two projects claiming the same thing equally is **not**
  settled by order: it goes to a bucket you can read with
  **`ephor feed --unattributed`**, because a guess that lands wrong amends
  someone else's row silently. Declaring a shared source under a project still
  works and says once per refresh where it belongs now.
