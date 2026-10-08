# FS-018-private-sources: a source the person declares private keeps its work theirs

Attribution decides which project a conversation is about, and so where it
shows ([§FS-008-attribution](FS-008-attribution.md#fs-008-attribution-every-conversation-finds-its-project-or-says-that-it-could-not)). A person's own mail and chat accounts carry
conversations about an organization's projects, and a direct message about
`agent-grounds/rhei#12` belongs on rhei's row: that is what a site-wide source
and its attribution are for ([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now), [§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep)). What
followed from that placement was not right. Every matter on a project's row was
the project's to work, so the message was written into the project's work root
with its words in the dossier, and the project's autorun recipe started an agent
on it with nobody present. The boundary that lets one tool watch an employer's
estate and a person's own business without either noticing the other
([§GRUND-001-overseer.2](../grund.md#2-what-this-project-does-about-it)) held between projects and broke inside one.

So the site may say that a source is the person's own. Placement does not move.
What moves is whose roots the work on such a matter goes in
([§FS-014-work-root-scopes.2](FS-014-work-root-scopes.md#2-reach-places-and-nothing-else-does)) and what may write or start that work with nobody
present ([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)). A source the site does not list behaves exactly
as it did before this point existed.

## 1. The site lists private sources

The site configuration's `work.private` names the sources that are the
person's own, and the root their work goes in:

```json
{ "work": { "private": { "sources": ["chatgw"],
                         "root": "~/me/private/{org}/{project}" } } }
```

- **`sources`** lists provider names: the same names a recipe's
  `when.sources` selects on, `{source}` renders, and every matter id begins
  with. A matter is **private** when the source that reported it is listed.
- **`root`** is a template with the vocabulary and the refusals of `work.root`
  ([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)).

It is read at the site and nowhere else. Whose an account is does not vary by
project, so an organization's or a project's `work` block carrying `private` is
refused when the file loads, by name, the way either block refuses any key it
does not take. Nor is it a key on the source's own block: ephor hands that
block to the source verbatim, and whose a source is belongs to the site that
binds it, never to the source ([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)). `ephor doctor` names a
listed source that no source in the site binds, because a name that matches
nothing makes nothing private, and a protection that silently protects nothing
is the one thing it may not be.

**This is a classification, not a selector.** A selector says what ephor
volunteers, and a person's ask is never refused for not matching
([§FS-005-dispatch.10](FS-005-dispatch.md#10-what-ephor-offers-is-not-a-limit-on-what-can-be-asked)), so no selector can keep a matter out of `work ask`. A
negation on `when.sources` would protect only the recipes that remembered to
write it, and none of the shipped ones does. So `sources` in a selector keeps
no `!` ([§FS-005-dispatch.31](FS-005-dispatch.md#31-a-selector-can-ask-who-holds-a-matter-and-what-it-is-labelled)), and the classification is read wherever work on a
matter is written or started, whatever selected it.

## 2. The private root answers first and alone

For a private matter, `work.private.root` is the first rung of the root ladder
and the only one consulted: no entry, recipe, project, organization or site
root is read ([§FS-005-dispatch.6.1](FS-005-dispatch.md#61-the-work-root-is-a-template-and-it-may-reach-above-the-project)). It outranks a recipe's or an entry's own
`root` because that key describes the work, not whose matter it is; if it could
win, any recipe naming one would put the person's conversation back in the
project's root. The rung is the same at every move that writes a plan —
`work dispatch`, `work ask`, `work lay`, the key on the matter's row — and it is
the root `work offers` previews.

The template repeats the organization and the project in its own path, so the
person's forest mirrors the organization's and reach still picks the scope
inside it ([§FS-014-work-root-scopes.2](FS-014-work-root-scopes.md#2-reach-places-and-nothing-else-does)). A private answer is written there even
where a branch workspace exists, and its run still starts from the checkout the
matter resolves to ([§FS-005-dispatch.13](FS-005-dispatch.md#13-a-communication-is-work-too-and-its-answer-comes-back-as-a-proposal)).

Three cases are refused by name, before anything is written, at every move that
would write, and `work offers` carries the same refusal on the entry in place of
a root ([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)):

1. **`sources` lists the matter's source and no `root` is declared.** The
   refusal names the source and `work.private.root`. The file still loads —
   the feed, its placement and every other source are unaffected — because the
   refusal is about where this work would go, and only a move that writes work
   asks that.
2. **The root renders inside the project's registry root.** That directory
   holds the project's checkouts and its own work root, so a private root there
   is the organization's root under another name. The refusal names
   `work.private.root`, what it rendered, and the project root it is inside.
   The path is judged as it will stand once made: through every symlink above
   it, and with every `..` in it taken, whether or not the directory it passes
   through exists yet.
3. **A `branch` template would make a workspace for the matter in the
   project's tree** ([§FS-005-dispatch.25](FS-005-dispatch.md#25-work-about-a-matter-with-no-branch-can-mint-the-branch-it-needs)). The workspace would be a new
   directory and a new branch among the project's checkouts, named from the
   matter — a correspondent's name, a number — and made by the project's own
   checkout command where one is bound, which is handed the whole matter. The
   refusal names the source as private, the recipe or entry and its `branch`
   template, and the workspace it would have made; nothing is run to make it.
   A workspace already on disk is used as it is, since nothing is written to
   it, and so is a branch the matter has of its own.

## 3. Only a named move writes or starts work on a private matter

A move is the person's when it names the matter: `--item` on `work dispatch`,
`work ask` and `work lay`; `work run --item` without `--due`; or a key pressed
on the matter's row. Everything else is a sweep, whoever typed it:

- `work dispatch` without `--item`;
- `work sync`, whether it would open work on the matter or reopen it;
- `work run --due`, with or without `--item`, because that sweep walks every
  due root ([§FS-011-command-line.10](FS-011-command-line.md#10-a-mutating-verb-above-one-project-reports-and-acts-under---act));
- a plain `work run`, which names no matter, with or without `--project` and
  `--act` ([§FS-005-dispatch.30](FS-005-dispatch.md#30-a-run-asked-for-by-name-reaches-the-whole-of-that-matters-work));
- the sweep `work dispatch --item` runs after it has written
  ([§FS-005-dispatch.24](FS-005-dispatch.md#24-work-nobody-has-to-start-starts-itself)).

**A sweep never writes or starts work on a private matter, and says so.**
`work sync` and a `work dispatch` without `--item` report the matter as
`passed-over`, with `hold: {"kind": "private", "source": <provider>}` beside a
sentence naming the source as private. `work run --due` and a plain `work run`
pass over a root whose every would-be-due ticket is about a private matter,
with the `private` hold, and run a root that holds other work beside it for that
work alone ([§FS-005-dispatch.24.2](FS-005-dispatch.md#242-a-passed-over-row-names-its-hold-as-data)). A plain run keeps everything else it is: blind
to `autorun`, and bound by none of the sweep's ceilings. A plan the person laid
is never started by a sweep, then or later: it starts on `work run --item` or
the run key, which nothing here refuses.

**Whose a ticket is, is read off the ticket.** The sweep takes the source from
what the ticket records about its matter ([§FS-005-dispatch.8](FS-005-dispatch.md#8-the-ticket-carries-the-item-as-data-not-only-as-prose)), never from the
ledger, so a ticket laid in a project's root before its source was listed stops
autorunning too. Its plan stays where it was written. Unlisting a source sends
new work on it back to the ladder every other matter climbs.

Nothing else changes for a private matter. The feed, the glance and the
placement are attribution's, and stay so. Who may do the work is the project's
choice as before ([§FS-005-dispatch.14](FS-005-dispatch.md#14-who-does-the-work-is-chosen-and-defaulted-per-project)): the matter is still the project's
matter, so its `permitted_hands` binds, and the roster is read at the private
root as at any root. The ceilings, the budget and the back-off bind the sweep,
which starts nothing here, and never a named run. Every reading says the same
thing in its text and in `--json`, the hold as data added to a vocabulary that
was declared open ([§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program), [§REQ-002-parity.4](../requirements/REQ-002-parity.md#4-the-machine-form-is-a-contract-not-a-dump)).

## 4. What is not yet held

The rule above closes the leak a private source opened: no work on its matter in
the organization's root, and none started with nobody present. Five things the
same classification will want are not held yet, and each is its own change.

- **A fixed hand per private source.** A `work.private.permitted_hands` would
  apply the narrowing a project can already put on the roster to a source
  instead. Nothing unattended picks a hand for a private matter now — every run
  of one starts on the person's key, with the hand shown on the offer — so
  neither half of the leak needs it.
- **What a private copy means for the feed cache and the reply records.** The
  feed cache and the send records sit in ephor's own state directory rather
  than in any organization's root, and hold a private conversation the way they
  hold any other. What they should keep of one is a decision of its own. The
  logs and proposals of private work already live in the plan's root, which is
  now the private one.
- **Reading plans in the private root as the project's own tasks.** The
  project's task stores are read where they live, behind a loop guard
  ([§FS-006-project-interface.7](FS-006-project-interface.md#7-the-projects-own-tasks-are-read-where-they-live)); a private root is not yet one of them. Ephor's own
  work there is already seen, through the ledger and the board.
- **An unlisted source is private.** A site that wants a newly added source to
  stay the person's until said otherwise needs a way to mark a source as the
  organization's, which nothing here offers.
- **A file's contents, brought in by the person.** A private matter's dossier
  names the files on its messages as any matter's does, by name, type and size,
  and fetches none of them ([§FS-005-dispatch.2](FS-005-dispatch.md#2-the-ticket-carries-what-ephor-knows-not-a-link-to-it)). What nothing offers yet is a
  person choosing to bring a file's contents into the work. The rule for
  whoever adds it: on a private matter the contents arrive only on the person's
  own named move, and only into the private root.
