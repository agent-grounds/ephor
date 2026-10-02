- **ephor learns who can be asked**
  ([§FS-005-dispatch.14](../../functional-spec/FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)).
  The runtime binding grows a fourth verb beside writing, running and reading
  back: the **roster** — every agent/model pairing the binding's own merged
  settings declare, each a **hand** with an id configuration can name, the
  agent and model it resolves to, the efforts it declares, and, where it
  cannot be used, the computed reason why. The enumeration is read from the
  binding rather than kept as a list of ephor's, which would drift the first
  time an agent or model was added on the other side
  ([§DA-004-roster-is-asked-not-configured](../../decisions/architectural/DA-004-roster-is-asked-not-configured.md#da-004-roster-is-asked-not-configured-the-roster-is-asked-of-the-binding-never-kept-by-ephor));
  the binding's `agent[mode]:provider:model` grammar is rendered inside
  `work/runtime/` and nowhere else, and the read mirrors the binding's own
  semantics exactly: settings overlays merge by field presence, so an
  explicit `null` clears what it inherits; a model's carrier resolves
  `defaults.agent`, then the older top-level `agent`, then the profile's own
  `default_agent`; efforts keep the order the settings declare them in. Ids
  are unique — the binding's model and agent registries are separate
  namespaces, and where a model profile claims an agent's name the profile
  holds it and the agent standing alone is listed as `@<agent>`. `ephor
  doctor` and `ephor capabilities`
  print the roster — an unavailable hand stays on the list with its reason —
  and their `--json` output becomes an object with `projects` and `roster`
  keys, where it was the projects array alone. With no runtime bound, or a
  bound one not on `PATH`, the roster is empty and says so in the workable
  rung's own sentence — a settings file that does not parse empties it too,
  naming the file — and every other rung resolves as before. No action
  chooses a hand yet: the resolution order is specced, and lands with the
  configuration and the picker.
