- A provider block's `timeout_seconds` was read from the configuration and
  then ignored: every provider ran under the shared
  `defaults.provider_timeout_seconds`. A forge behind a VPN, configured with
  the longer ceiling it needs, timed out on every refresh and its whole
  section of the feed stayed empty
  ([§FS-001-forge-interface.6](../../functional-spec/FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)).
