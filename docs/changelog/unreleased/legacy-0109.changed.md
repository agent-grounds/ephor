- The *ticketed* rung is now **local-issues**
  ([§FS-006-project-interface.10](../../functional-spec/FS-006-project-interface.md#10-capability-rung-by-rung)).
  A *ticket* is what a remote tracker keys — a Jira key, a forge issue number —
  and these are the project's own, kept in its checkout, so one name for one
  thing ([§FS-001-forge-interface.3](../../functional-spec/FS-001-forge-interface.md#3-policy-lives-above-the-interface-never-in-an-implementation)). `requires: ["ticketed"]` still resolves,
  so nothing anybody already wrote stops meaning what it meant.
