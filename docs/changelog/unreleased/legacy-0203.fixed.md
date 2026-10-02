- **A gate verb could never run where it said it did**
  ([§FS-006-project-interface.3](../../functional-spec/FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)).
  `seams::gate::run` built a rootless site, so a verb declaring
  `"cwd": "workspace"` silently ran at the forest root instead of in the
  branch workspace the change resolves to. It takes the caller's site now.
