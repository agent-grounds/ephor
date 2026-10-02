- **Autorun capacity has an organization tier between the site's and each
  project's**
  ([§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).
  The feed configuration gains a top-level `organizations` map mirroring
  `projects`: `organizations.<org-id>.work.max_concurrent` bounds the live
  autorun roots of every project whose registry row carries that
  `organization` id, inside the site's aggregate ceiling and outside each
  project's own. Membership stays the registry's; the ceiling is read-only
  against it. All three ceilings are evaluated and a sweep refuses by
  whichever is full first, naming the outermost one in the `passed-over`
  reason in prose and `--json`. A project ceiling written above its
  organization's — or above the site's — is a note at the sweep giving the
  project, the ceiling above it and both numbers; it is never refused, never
  rewritten, and the ceiling above it still bounds its total. A ceiling of `0`
  above is a pause somebody wrote on purpose rather than a budget, so the
  numbers under it are not noted. Omission is unchanged in both directions: a
  configuration with no `organizations` map, or a project whose registry row
  names no organization, starts exactly what it started before. An
  `organizations` id no registry row places a project in reaches nobody and is
  named for it — by `ephor doctor`, in the words an unknown project id gets,
  and in a note at the sweep — rather than quietly ignored. That covers an id
  the registry never declared and one it declares that no project has joined
  alike: membership is the project row's `organization` field and nothing
  else, so the ceiling and the note read the same thing. Note that the feed
  configuration refuses unknown keys, so a `status.json` carrying
  `organizations` needs this version. (PR #54)
