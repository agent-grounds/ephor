- **Everything the screen holds is a command, and every answer has a machine
  form** ([§REQ-002-parity](../../requirements/REQ-002-parity.md#req-002-parity-every-ability-is-reachable-without-the-screen-and-every-answer-has-a-machine-form),
  [§FS-011-command-line](../../functional-spec/FS-011-command-line.md#fs-011-command-line-every-ability-is-a-command-and-every-answer-has-a-json-form),
  [§AR-009-surfaces](../../architecture/AR-009-surfaces.md#ar-009-surfaces-one-api-beneath-both-surfaces-and-one-schema-per-answer), PR #2). The interactive
  interface is a convenience over the watch, never the place the watch lives —
  but a good third of what it could do had no command behind it, so a runtime
  ephor hands work to could read a feed and not finish a move. That is now a
  **law**: every ability the interface offers is also a command, every command
  that prints a reading takes `--json`, and a key on neither the ability list
  nor the stated presentation list fails the build (`just check`, and CI).

  **New commands.** `ephor actions` prints the menu a matter or a branch
  carries — the source's offers, ephor's, the project's, your own, the work
  that can be handed over — in provenance order, each row saying whether it can
  run and, where it cannot, the ladder's own sentence. `ephor actions run <id>`
  runs one, in the same place and with the same `EPHOR_*` context a keystroke
  would, with `--hand` for who gets the work, `--set` for a workflow's inputs,
  `--yes` where an entry asks to be confirmed, and `--command` for the freehand
  row. `ephor branches` says where every branch stands — checked out or not,
  how far behind its base and its published copy, and as of when.
  `ephor operations` is the whole board, the runtime's half and ephor's own
  jobs together. `ephor thread` prints a matter's conversation, numbered, and
  `ephor react`, `ephor tick` and `ephor reply` take those numbers — including
  sending the reply a run drafted, which until now only a keystroke could do.
  `ephor work offers` is one matter's work screen.

  **`--json` everywhere.** `react`, `tick`, `reply`, `failures`, `restart`,
  `rebase`, `checkout`, `mark-read`, `refresh`, `list`, `validate` — including
  `validate --manifest` — `job log`, `check`, `update`, `ensure-agents` and
  every `work` subcommand, `work states` included, now answer as JSON, printing
  what they *changed* rather than a re-description of the request: which
  repositories rebased and which conflicted, what a restart asked for and what
  it skipped, which tickets opened, which managed workspaces were rewritten,
  and — where a replay's conflict was handed over — the ticket that opened for
  it. Under `--json` the reading is alone on standard output: notes and
  progress go to stderr, and so does the output of anything ephor runs for you,
  including the runtime under `work run`, an entry under `actions run` and a
  project's own check verbs under `check`. A command that is *refused* answers
  too, with `{"ok": false, "says": …}` on standard output and the exit code it
  always had — a script reading only the reading used to see an empty stream,
  which is also what a move that worked silently looks like. `ephor doctor
  --json` prints **one** document with both passes in it rather than two
  objects on one stream, which no parser could read. Every shape is published —
  `ephor schema views` prints the document, a walk of the command tree fails
  the build on a `--json` that declared none, the end-to-end suite runs every
  one of those commands and validates what it actually prints against its own
  entry, and they are a stability surface like the manifest and forge schemas.
  `failures` and `restart` also take `--item ID` now, instead of only the four
  coordinates a quick action passes them; `job log` takes `--follow`, and with
  `--json` it waits for the job to end and then answers with the whole log and
  the state it ended in, which is how a script waits on a job.

  **Refused rather than dropped.** `--hand` on an entry that hands no work
  over, `--set` on one that lays no workflow down, and a configured entry or
  recipe whose id claims one of the names ephor mints for its own rows
  (`@command`, `@workflows`) are all refused by name, where you can still see
  what you wrote. `--dry-run` on `actions run` reports the whole chain an entry
  would walk — the checkout first where the branch workspace is missing, then
  the entry itself, in the workspace that checkout is about to create.

  **One implementation underneath.** The menu's assembly, the capability
  gating, the branch standings, the board's phrasing, the conversation walk and
  every move now live in `src/api/`, below both surfaces: the interface renders
  what a command prints, so the number `ephor thread` gives a message is the
  number `ephor react --message` takes, and a greyed row and a JSON `refusal`
  are the same sentence. One of everything, too — one session, one work ledger,
  one read of the registry per run. The work screen and `ephor work offers` now
  answer from the same derivation, so a row one of them offers is a row the
  other has — workflows included, which the screen used to drop; `+`, `t` and
  `p` in a conversation go through the same calls `ephor react`, `ephor tick`
  and `ephor reply` do, so posting a reply retires the draft on both surfaces
  or on neither; the interface's dispatch writes the session's own ledger
  rather than a second copy of it; and an entry runs the same chain, in the
  same place, whether it takes your terminal, runs beneath it, or is only being
  described by `--dry-run`. Where a reading cannot be given at all — a project
  the registry cannot place — the reason travels with the empty list rather
  than being the empty list.
