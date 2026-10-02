- **The feed is made of matters**
  ([§FS-007-matters](../../functional-spec/FS-007-matters.md#fs-007-matters-the-feed-is-made-of-matters-and-a-matter-knows-why-it-is-there),
  [§AR-006-matters](../../architecture/AR-006-matters.md#ar-006-matters-the-core-types-of-the-watch)).
  The store now holds the model rather than a flat row per report: a `Matter`
  with a stated subject key, a placement (project and branch, or unattributed
  carrying the projects that claimed it), the conversation as `Discussion`s of
  `Message`s in channels that declare what they can carry, everything else as
  `Event`s, the keys it references as links, and a fingerprint digesting state,
  each discussion's last activity and message and task counts, and the event
  tail — which is what will let a resurfacing row say *what* moved instead of
  only that something did. A gate is an observation of a matter now, not a row
  of its own. The surfaces still read the flat rendering while they are ported
  onto the model, and a round-trip test keeps that rendering honest.
  **The cache is a cache**: it carries the model it was written in, and an
  older one is dropped rather than migrated, so the first run after upgrading
  shows an empty feed until `ephor refresh` fills it again. What you marked
  read is not in that store — it is keyed by matter key in `seen.json` and
  survives the rebuild untouched.
