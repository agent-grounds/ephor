- **A pin may name alternates, and reported headroom vetoes a spent pool**
  ([§FS-005-dispatch.14](../../functional-spec/FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project),
  [§FS-005-dispatch.29](../../functional-spec/FS-005-dispatch.md#29-headroom-is-reported-to-ephor-and-vetoes-a-member-it-never-reorders),
  [§FS-006-project-interface.9](../../functional-spec/FS-006-project-interface.md#9-offers-the-projects-actions),
  [§DA-009-headroom-vetoes](../../decisions/architectural/DA-009-headroom-vetoes.md#da-009-headroom-vetoes-headroom-is-a-report-ephor-is-given-and-it-vetoes-a-member-rather-than-ranking-the-list)).
  Everywhere a hand is named — `work.hands` at the site and per project, the
  pin a recipe or an action carries, `--hand` on `work dispatch`, `work lay`,
  `actions run` and `rebase --dispatch` — the value may be an ordered list of
  hands instead of one, written as a JSON array in configuration and as commas
  on the command line; an empty member is refused with what was written. One
  name *is* that list with a single member, so nothing already written means
  anything different, and a list of one is still serialized as the bare name.
  The seven precedence steps are untouched: they answer exactly once, and what
  the answering step hands on is the list it carried. Permission is checked
  against every member and one unpermitted name refuses the list whole with
  that name in the message, never filtered to the permitted members.
  Which member gets the ticket is then decided from evidence ephor is *given*,
  never from a number it derives. Two channels report on a **pool** — a hand's
  provider where the roster gives it one, its agent id where it does not. The
  ledger channel needs no configuration: a start that failed becomes a refusal
  on that pool where its words carry an instant ephor can read, held until the
  instant and cleared by any observed success; where they carry none, nothing
  about a pool is claimed and the failure stays that root's own back-off. The
  optional `work.headroom` binds one command per pool, summoned like every
  other command and answering the standard envelope on `$EPHOR_ANSWER` with its
  windows on `data`; it runs under `refresh`, on the same freshness discipline
  as every other source, and never in front of a dispatch. `remaining` null or
  absent is **unknown and never zero**, a pool's effective remaining is the
  least of its *known* windows, and the rule is a veto over what is known spent
  rather than a ranking over what is known remaining — survivors keep the order
  their author wrote, and where every member is vetoed the first still gets the
  ticket carrying the earliest instant any of their pools resets. The floor is
  `work.headroom_floor`, `0` unless the site names another. An unbound pool, a
  verb that exits non-zero, output that will not parse and an answer with no
  readable window all degrade to unknown with the reason beside the pool, never
  an error that stops a dispatch. The choice is recorded on the ticket in
  ephor's own words beside the dossier, rather than as a field of the runtime's
  plan language. `ephor status` and `ephor capabilities` grow a line per pool
  and `ephor caps --json` a `pools` array, both by addition;
  `config/headroom.example.sh` and `config/headroom-metered.example.sh` are the
  worked examples the seam ships beside ephor. (PR #73)
