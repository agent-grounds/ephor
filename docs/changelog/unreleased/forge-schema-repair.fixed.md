- **`ephor schema forge` describes what the forge types send and read.** The
  hand-written schema had drifted from them
  ([§FS-001-forge-interface.2](../../functional-spec/FS-001-forge-interface.md#2-two-transports-one-interface)).
  It now declares the `review` and `notices` capabilities, a pull request's
  `review`, an issue's `blocked_by`, the `project`, `user` and `emoji` keys of
  a request, the `notice` and `notices_response` shapes, the five words
  `reasons` may hold, and one `thread` definition. It no longer declares
  `terminal` on a pull request or an issue, or `label` and `url` on a thread:
  ephor never read them, and an answer that still sends them still validates.
  Every answer ephor accepted before validates against it. The one place it is
  narrower is `reasons`, which used to admit any string although ephor refused
  every word but those five. (PR #155)
