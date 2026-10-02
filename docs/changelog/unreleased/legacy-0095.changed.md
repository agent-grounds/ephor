- **A workflow the runtime offers is an action**
  ([§FS-005-dispatch.19](../../functional-spec/FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here),
  [§FS-006-project-interface.9](../../functional-spec/FS-006-project-interface.md#9-offers-the-projects-actions),
  [§DA-006-hands-fill-a-workflows-targets](../../decisions/architectural/DA-006-hands-fill-a-workflows-targets.md#da-006-hands-fill-a-workflows-targets-who-a-workflows-agents-are-is-ephors-answer-not-the-workflows)).
  The runtime carries more than a place to put tickets: it carries whole
  **workflows** — parameterized plans that lay down tasks of their own, under a
  machine of their own, with fan-out and gates ephor never wrote. Using one
  meant leaving the watch, remembering another vocabulary, and coming back with
  a workspace nothing here knew about. They are now entries in the same menu,
  selected by the same `when` language and refused in the same sentence. An
  **entry** is what makes a workflow an action, and it may be written in any of
  three places: beside the workflow itself, in the project's manifest, or in
  your own configuration — ranked by where the workflow was found, so what the
  runtime ships ranks with what ephor ships and the project's with the
  project's offers.

  **Its inputs are answered here.** Five steps, each displacing the ones after
  it: what you answered for this laying alone, what the entry says, ephor's
  answer for an input that names who does the work, the workflow's own default,
  and — where an input is required and still unanswered — you, asked or refused
  by name. What the entry says is data rather than prose: a string is rendered
  with the matter's own fields (`{branch}`, `{repo}#{number}`, and `{dossier}`
  and `{item}` for the files ephor writes beside the plan), anything else is
  passed on as it stands, and strings nested inside a list or a record are
  rendered too. One missing scalar is one line typed where you are standing;
  anything more, or anything wanting a list or a record, opens a file in your
  editor with everything already resolved in it.

  **Who does the work is ephor's answer, not the workflow's.** Half of a
  workflow's input surface is usually its agents, each defaulted to whatever
  model its author was running — so `work.hands`, the hand picker and
  `work.permitted_hands` would all have stopped applying at exactly the
  keystroke that mattered. An input the workflow declares an execution target
  for, or that an entry lists under `hands`, now resolves through the same
  seven steps a ticket's hand does and is refused under a narrowing wherever it
  was named, the workflow's own default included.

  What lands is a **plan of its own beside the matter's**, never a ticket
  inside it, written into the matter's own work root — so the operations board
  finds it by looking like every other plan there, and a workflow and a ticket
  about one change share the root's one run rather than editing the same tree
  at once. Laying one down writes files and nothing else
  ([§FS-005-dispatch.7](../../functional-spec/FS-005-dispatch.md#7-handing-over-work-is-the-readers-move-and-stays-inside-the-machine)):
  running it is the move after, from the board. A second laying of the same
  entry is a second record, not a correction of the first. From the command
  line, `ephor work workflows` lists what the runtime offers and what each one
  takes, and `ephor work lay <entry> --item <id> [--set <input>=<value>]…`
  lays one down, with `--dry-run` showing what would answer every input before
  anything is written. With no runtime bound there are no workflows, in the
  *workable* rung's own words — what the binding offers is the binding's to
  say.
