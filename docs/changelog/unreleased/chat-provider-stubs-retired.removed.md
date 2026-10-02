- The `slack`, `discord` and `email` provider stubs, which were skipped while
  no secret existed for them and failed as not implemented once one did. Chat
  and mail come in through a forge that answers `messages` instead, and a
  provider block that still names one of them now resolves like any other
  name, to `ephor-forge-<name>` on `PATH`. (PR #155)
