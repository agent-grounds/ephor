- **Chat and mail reach the feed through a forge that answers `messages`.**
  A forge extension may now declare `"messages": true` and answer the
  conversations addressed to you in the other places people talk, each one
  whole — a stable id, title, url, when it last moved, the room it is in, the
  reasons it is yours, and its threads with a reply descriptor on each
  ([§FS-001-forge-interface.1](../../functional-spec/FS-001-forge-interface.md#1-capabilities)).
  Each lands in Messages as `<source>:<id>`, and whether it waits on you is
  ephor's to read off the messages, as it is for a pull request's threads.
  `ephor schema forge` describes the new `messages_response` and
  `conversation`, and now also the `notices` request and answer it was
  missing. (PR #155)
