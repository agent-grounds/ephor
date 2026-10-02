- **Work placement can reach above a project, and discovery can find it there**
  ([§FS-005-dispatch.6.1](../../functional-spec/FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)).
  The registry has always given an organization a `root`, and nothing in work
  placement could name it: `work.root` was site-wide or per-project, its
  placeholders stopped at project scope, and a template naming `{org_root}`
  wrote a directory literally called `{org_root}`. `{org}` and `{org_root}`
  are now placeholders like any other, resolved from the organization the
  project's registry row places it in, and `organizations.<org-id>.work.root`
  is the tier between the site's `work.root` and a project's — precedence
  project, then organization, then site, with the innermost one written the
  whole answer. So `"root": "{org_root}/panta"` on an organization makes one
  work root for all of its projects, for work that belongs to no single
  repository. The board finds a plan laid there: the organization's root is
  one of the places the work-root walk probes, where the template reaches for
  it, so such a plan is enumerated and swept rather than written to and never
  looked at. Where the answer is missing the dispatch refuses by name —
  *organization acme declares no root*, and as explicitly for a project whose
  registry row names no organization — instead of writing a literal
  `{org_root}` directory or a path with the segment missing; discovery skips
  the same template rather than guessing, since nothing could have been
  written through it. `ephor checkout` resolves the work root through the
  same three tiers, so the placement is one answer for both. Nothing changes
  for a configuration that names no organization placeholder and no
  organization-tier root. (PR #59)
