- **A JSON array under a hand is a list of hands, not a pair read
  positionally**
  ([§FS-006-project-interface.9](../../functional-spec/FS-006-project-interface.md#9-offers-the-projects-actions)).
  `HandPin` was deserialized through an untagged enum over a string and the
  long `{ "agent", "model", "effort" }` form, and serde fills a struct from a
  sequence positionally — so `"hands": { "default": ["a", "b"] }` was
  **accepted**, silently, as agent `a` carrying model `b`, and
  `["a", "b", "c"]` as that pair at effort `c`. A hand is now read by a
  visitor that takes a string and a map and refuses a sequence by name, saying
  where a list belongs. (PR #73)
