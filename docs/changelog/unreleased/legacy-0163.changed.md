- **Tasks**: a forge that tracks tasks — a checklist item, a blocker comment, a
  review task — reports each one's state on the message carrying it, and ephor
  draws it as the box it is. `t` on the thread screen ticks the selected task
  through the source that reported it, and the box fills in without waiting for
  a refresh ([§FS-004-quick-actions.5](../../functional-spec/FS-004-quick-actions.md#5-a-task-is-ticked-where-it-is-read)).
  New capability `tasks`, new subcommand `ephor-forge-<name> resolve-task`.
