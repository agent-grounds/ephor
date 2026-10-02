- **A worked chat gateway ships beside ephor.**
  `config/chat-gateway.example.sh` is a complete `messages` and `reply` forge
  over the spool an always-on listener keeps, meant to be linked as
  `ephor-forge-chatgw` and declared once in `sources`. It answers empty only
  while it can show the listener is still hearing the network, and otherwise
  fails as never paired, unreachable, or unable to vouch for its spool
  ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)). (PR #155)
