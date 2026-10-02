- **A workspace that is not there is offered the checkout**
  ([§FS-004-quick-actions.7](../../functional-spec/FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout)). ephor knew the branch was not on disk — it
  computes the directory from the project's own template and looks — and then
  refused every action that needed it with "no 'checkout' command is
  configured", which is knowing what is wrong and sending the reader to a
  configuration file about it. There is now `ephor checkout`, and the menu
  offers it unasked: the registry already holds every input it takes, so the
  project's `branch_root_template` says where the workspace goes, its type says
  which repositories it holds, and its `main_branch` says what a new branch
  grows from. It is git and nothing else, and it has the rebase's shape — a
  working tree per repository, since a poly-repo workspace is several
  repositories sharing one branch name: the branch itself where that repository
  has it, and a new branch of the same name off the base where it does not,
  which is what a change touching one repository of a tree looks like on disk.
  A repository whose branch another working tree is holding is reported and
  left alone rather than worked around, one already there is reported as
  already there, and the same command runs whether the reader presses the key
  or a state machine calls it ([§FS-005-dispatch.12](../../functional-spec/FS-005-dispatch.md#12-work-an-algorithm-can-finish-does-not-start-with-a-model)). A project that wants its
  own checkout command still configures one and it still wins; the difference
  is only whether anybody expects to want their own.
