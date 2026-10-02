- Posting a reaction only ever reached GitHub. `Forge::react` and the
  `ephor-forge-<name> react` subcommand were implemented and documented but had
  no caller, so an out-of-process forge that answered them could not be reached:
  a descriptor ephor did not recognize was dropped rather than handed back to
  the implementation that wrote it. Reactions now route through the source that
  reported the message.
