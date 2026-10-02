- **A registry row names the rooms that are its project's.** `rooms` on a row,
  and `identity.rooms` as a manifest's hint, list the venues on a chat source a
  project claims, spelled exactly as the source states them. A conversation in
  a claimed room is that project's before any reference or alias is consulted;
  one in a room nobody claimed, or a direct one, is placed by what it refers
  to. A room matches only when equal, and the row's word is its presence:
  `"rooms": []` says *none* and refuses a checkout's claim
  ([§FS-008-attribution.1](../../functional-spec/FS-008-attribution.md#1-identity-is-declared-and-the-row-has-the-last-word),
  [§FS-008-attribution.3](../../functional-spec/FS-008-attribution.md#3-venue-beats-reference-beats-resemblance)). (PR #155)
