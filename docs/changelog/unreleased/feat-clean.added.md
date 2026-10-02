- **`ephor clean` gives back what builds took in every idle branch checkout,
  through each project's own clean verb** ([§FS-017-clean](../../functional-spec/FS-017-clean.md#fs-017-clean-an-idle-checkout-gives-back-what-its-builds-took-through-the-projects-own-verb)). The verb is bound
  the way a check verb is: `./clean.sh` at the checkout's root, a manifest's
  `clean` key, or the site's `projects.<id>.clean`. The sweep covers every
  project in the registry, or the ones `--workspace`, `--tag` or `--org` name.
  It skips the main-branch checkout and passes over a checkout a live run
  holds or one with no clean verb declared. Without `--act` it lists what it
  would summon where. Under `--act` it reports what each verb reclaimed, and a
  total. A new *cleanable* rung on the capability ladder says whether a project
  binds the verb, and ephor ships a `clean.sh` of its own.
