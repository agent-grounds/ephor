- **A workflow's inputs are answered on one screen, and what has a known set
  is chosen from it**
  ([§FS-005-dispatch.19](../../functional-spec/FS-005-dispatch.md#19-a-workflow-the-runtime-offers-is-an-action-and-its-inputs-are-answered-here),
  [§DA-006-hands-fill-a-workflows-targets](../../decisions/architectural/DA-006-hands-fill-a-workflows-targets.md#da-006-hands-fill-a-workflows-targets-who-a-workflows-agents-are-is-ephors-answer-not-the-workflows),
  PR #5). Laying a workflow down used to ask only about the holes: one missing
  scalar was one line typed where you stood, and anything more was a JSON file
  in `$EDITOR` — which meant the answers ephor had resolved, the workflow's own
  defaults most of all, went in unseen. Choosing a workflow now opens **every
  input on one screen**, each row carrying the answer the five steps reached
  and the step it came from, and each one changeable there. Where the values an
  input can take are known it is **picked** rather than spelled: a flag has two,
  an input that names who does the work has the roster — narrowed, and saying
  who is unavailable and why — at the effort the hand declares, and an input
  whose own check is a plain set of words has those words. An input wanting
  several hands takes several, in the order you take them. `p` asks the runtime
  what it would write before it writes it; `e` still opens everything resolved
  in `$EDITOR`, which is where a record is answered; and the last row lays it
  down, refusing while a required input is unanswered and naming it.

  **An input naming who does the work is now recognized where it always
  should have been.** Which inputs those are was read out of the workflow's
  `template.yaml` — so it worked for a workflow kept as a directory and for
  nothing else, and every workflow the runtime ships inside itself had its
  targets left at its author's models, quietly, which is the one thing
  [§DA-006-hands-fill-a-workflows-targets](../../decisions/architectural/DA-006-hands-fill-a-workflows-targets.md#da-006-hands-fill-a-workflows-targets-who-a-workflows-agents-are-is-ephors-answer-not-the-workflows) refuses. ephor now reads it from the
  runtime's own listing where the listing says it (rhei publishes the whole
  input schema as of its next release), keeps the manifest reading as the
  fallback for an older one, and understands a **list** of execution targets as
  an input answered with hands too. `ephor work lay --set review_targets='["luna","sol"]'`
  answers such an input with several from the command line, the same call the
  screen makes.
