- **A recipe the projects of one organization share.**
  `organizations.<org-id>.work.recipes` is read on every project the registry
  places in that organization, between the site's `work.recipes` and the
  project's own, so the way a person works four repositories they have said are
  one organization is written once instead of once per repository. Membership is
  the `organization` field on a project's registry row and nothing else — the
  same reading the work root and the ceilings already resolve through — so a
  project the registry places in no organization is offered the site's recipes
  and its own, silently, and a configuration with no
  `organizations.<org-id>.work.recipes` resolves exactly as it did. Recipes
  accumulate outward in — shipped, then the site's, then the organization's,
  then the project's — and one reusing an earlier id replaces that recipe
  *where it already stands* rather than moving to the end: the later writer
  decides what the recipe says, the first writer decided where it sits in the
  menu, and position is the order dispatch offers in. That rule held between the
  two scopes that already existed and is now written down, because with three
  scopes it stopped being inferable from two. A recipe written there may not
  squat ephor's own namespace and is refused by name, which is *narrower* than
  the refusal before this: the organization work block refuses unknown keys, so
  until now a `recipes` key under an organization refused the whole site
  configuration and everything else it said. An organization *recipe* carrying
  its own `root` still sits at the recipe rung of entry → recipe → project →
  organization → site, so it beats `projects.<id>.work.root` the way a site
  recipe's does — the rungs of that ladder were never the configuration scopes
  ([§FS-005-dispatch.1](../../functional-spec/FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for),
  [§FS-005-dispatch.24](../../functional-spec/FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself),
  [§FS-005-dispatch.6.1](../../functional-spec/FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)).
  (PR #123)
