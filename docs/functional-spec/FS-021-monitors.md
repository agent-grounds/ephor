# FS-021-monitors: a quantity a project declares is read on its own cadence and kept, and its drop is a matter

Ephor watches what a forge or a script says happened. It could not watch a
quantity: whether the main branch is releasable, how many reports wait, line
coverage, a benchmark. A person kept those in their head or swept for them by
hand, which is the sweep [§GOAL-003-nothing-lost](../goals.md#goal-003-nothing-lost-the-watch-is-trusted-enough-to-retire-the-sweep) wants retired. A **monitor**
is such a quantity, declared in the small published protocol a project
already speaks ([§FS-006-project-interface](FS-006-project-interface.md#fs-006-project-interface-a-project-and-ephor-meet-over-one-interface-in-three-homes)). Ephor reads each monitor at its
own cadence, keeps its history, and draws it on a page. When a monitor with a
condition stops meeting it, the drop becomes a matter in Waiting and is
processed like any other ([§FS-019-waiting.7](FS-019-waiting.md#7-a-new-matter-is-processed-where-it-is-shown)).

Watching a quantity costs the watched what watching anything does: nothing
beyond what the project chooses to say ([§GOAL-005-costless](../goals.md#goal-005-costless-watching-costs-the-watched-nothing)). A repository
declares its monitors, or the person declares them at the site without
touching it, and the person has the last word on what healthy means
([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order), [§DF-008-ephor-judges-a-monitors-health](../decisions/functional/DF-008-ephor-judges-a-monitors-health.md#df-008-ephor-judges-a-monitors-health-ephor-decides-a-monitors-health-from-the-declared-condition-site-over-manifest-and-from-the-samples-own-ok-only-where-none-is-declared)). A dropped
monitor is one glance away, in the Status category and on Waiting's overview
line ([§GOAL-002-glance](../goals.md#goal-002-glance-one-glance-answers-what-needs-me-now)). Every ability the page has is a command with a
machine form ([§REQ-002-parity.1](../requirements/REQ-002-parity.md#1-an-ability-is-a-key-that-reveals-a-fact-or-changes-the-world), [§REQ-002-parity.3](../requirements/REQ-002-parity.md#3-every-reading-answers-a-program)).

## 1. A monitor is one declared quantity, and a condition makes it a status

### 1.1 The nouns

- A **monitor** is one quantity of one project: an id, a title, a kind, where
  its value comes from, and optionally a condition.
- A **sample** is one observation of a monitor: a value and when it was
  measured.
- A **reading** is a sample ephor accepted at a read and records
  ([§FS-021-monitors.9.2](FS-021-monitors.md#92-what-a-reading-is)).
- A monitor's **cadence** is how often it is read. Each read is due at a
  **slot**.
- A monitor's **health** is what its condition makes of its readings.

The id is lowercase letters, digits and `-`, starts with a letter or digit,
and is unique within its project. A monitor is named across the site as
`<project>/<id>`.

### 1.2 A KPI is a monitor nothing judges

A monitor nothing judges is a **KPI**: no home gives it a `healthy`, its kind
gives it none by default, and its last accepted sample states no `ok`, or a
`"healthy": null` says it is one ([§FS-021-monitors.3.6](FS-021-monitors.md#36-ephor-decides-health-from-the-declared-condition)). A KPI is read, kept
and drawn, and it never becomes a matter. A monitor something judges is a
**status**, and its drop is a matter. What judges it is almost always the one
field `healthy`, so one declaration shape serves both, and a KPI becomes a
status by gaining a condition and nothing else.

### 1.3 Health is one of four states

A status's health is `ok`, `warn`, `down` or `unknown`. A KPI has no health:
it is shown as `KPI`. A KPI can still turn unknown, and that is said on the
page only: it opens no matter, and neither Waiting's overview line nor
`--check` counts it ([§FS-021-monitors.7.4](FS-021-monitors.md#74-unknown), [§FS-021-monitors.18.2](FS-021-monitors.md#182-the-header)). `warn` is
amber and never a matter ([§FS-021-monitors.3.7](FS-021-monitors.md#37-warn-is-amber-and-never-a-matter)). `unknown` is never `down`: a
broken probe is not a broken project.

### 1.4 A project has monitors when one is declared or brought

A project has monitors when its manifest or the site declares one
([§FS-021-monitors.4](FS-021-monitors.md#4-where-a-monitor-is-declared)), or when an answer has brought a sample nothing declared
and the store still keeps that monitor ([§FS-021-monitors.4.3](FS-021-monitors.md#43-a-sample-may-describe-its-own-monitor),
[§FS-021-monitors.5.6](FS-021-monitors.md#56-which-samples-an-answer-gives)). Everything below applies to such a project only. A
project with none is watched exactly as before.

## 2. A value is one of four kinds

| Kind | Value | Default condition | Drawn as |
| --- | --- | --- | --- |
| `boolean` | `true` or `false` | healthy when `true` | a strip of health over the span |
| `number` | a number, optionally `of` another | none | the value, its change, a sparkline |
| `level` | one of the monitor's `levels`, best first | healthy at the first level | the level, and a strip of past levels |
| `series` | `points`; the value is the last point | none | the points the repository supplied |

### 2.1 `boolean`

A boolean is shown as `yes` or `no`.

### 2.2 `number`

A number carries a `unit` (`%`, `ms`, `s`, `bytes`, `count`, `$`, or free
text) and `better`, which is `higher` or `lower`. `better` decides the colour
of a change and which direction is worse. `of` makes the value a fraction,
shown as `12/15`. `of` is part of the measurement: a sample's `of` is that
reading's, and the declaration's stands where a sample states none. A
condition is written against the value, never against the fraction.

A number may also carry `points`, each `{t, v}` ([§FS-021-monitors.2.4](FS-021-monitors.md#24-series)): the
history a repository already keeps. Ephor draws them over the span before its
own first reading of the monitor, and its own readings after, so a repository
hands its history over instead of waiting for ephor to gather one. A number's
points are those of the last sample that carried any. Points labelled by `x`
make a number's sample unreadable ([§FS-021-monitors.8.2](FS-021-monitors.md#82-one-bad-sample-is-that-monitor-unreadable-never-the-answer-refused)), because a number is
drawn against time.

### 2.3 `level`

A level is one of the strings the monitor's `levels` lists, best first, such
as `["clean", "degraded", "failed"]`. A value the list does not hold is an
unreadable read of the monitor ([§FS-021-monitors.8.2](FS-021-monitors.md#82-one-bad-sample-is-that-monitor-unreadable-never-the-answer-refused)).

### 2.4 `series`

A series is history the repository already keeps and ephor never saw: a
benchmark per commit, a build's duration per run. Its sample carries
`points`, each `{t, v}` with `t` a time or `{x, v}` with `x` a label such as a
commit or a version. Every point of one sample uses the same form; points
that mix the two, or no points at all, make the sample unreadable
([§FS-021-monitors.8.2](FS-021-monitors.md#82-one-bad-sample-is-that-monitor-unreadable-never-the-answer-refused)). The value is the last point's `v`, in the order the
points are listed, and a `value` beside them is ignored. The condition is
judged on that value.

Ephor draws the points as given and never copies them into its history: a
reading of a series records one line, whose value is the last point's
([§FS-021-monitors.9.2](FS-021-monitors.md#92-what-a-reading-is)). A read whose last point is the one already held, the
same `t` or `x` and the same `v`, brings the measurement already held.

## 3. A condition says what healthy means

### 3.1 The language

A condition is an object written against the value:

- `{"is": true}` or `{"is": false}` for a boolean;
- `{"min": 80}`, `{"max": 50}`, or both, for a number or a series, each
  bound inclusive;
- `{"in": ["clean"]}` for a level, healthy at any level listed;
- `{"at_most": "degraded"}` for a level, healthy at that level or a better
  one.

`healthy` is the condition, and a value meets it when it meets every key it
holds. A key that does not fit the monitor's kind, a level its `levels` does
not hold, an empty condition, or one no value can meet, such as
`{"min": 80, "max": 50}`, is a declaration error ([§FS-021-monitors.4.6](FS-021-monitors.md#46-a-declaration-ephor-cannot-use-is-shown-and-not-read)).

### 3.2 Each kind has a default, and `null` removes it

A boolean without `healthy` is healthy when `true`, and a level is healthy at
its first level. A number and a series have no default: without `healthy` in
any home, they are judged by their samples' `ok` where those state one, and
are KPIs otherwise ([§FS-021-monitors.3.6](FS-021-monitors.md#36-ephor-decides-health-from-the-declared-condition)). `"healthy": null` makes any
monitor a KPI.

### 3.3 `warn` narrows what is shown ok

`warn` is a second condition in the same language that a value must also meet
to be shown `ok`:

- `down` when the value fails `healthy`;
- `warn` when it meets `healthy` and fails `warn`;
- `ok` when it meets both.

So `"healthy": {"max": 5}, "warn": {"max": 0}` on a count of waiting reports
is `ok` at 0, `warn` from 1 to 5, and `down` above 5.

### 3.4 A condition is said in words

The page, the command and a matter's title say a condition the same way. A
number's `healthy` is said as the values it accepts: `healthy ≥ 80%`,
`healthy ≤ 5`, `healthy 40–60 ms`. A condition that leaves one value is said
with `at`: `healthy at 0` for `{"max": 0}` on a count, which is never below
zero, and `healthy at 4/4` for `{"min": 4}` on a fraction of 4, which is said
as a fraction. A boolean's is said `healthy when yes`. A level's is said as
the levels it accepts, whether written with `in` or `at_most`: `healthy when
clean` for one, and `healthy: published or pending` for more.

`warn` is said as the values it marks, the negation of what it is written as:
`warn above 0` for `{"max": 0}`, `warn below 90%` for `{"min": 90}`, and for
a level the levels it marks, `warn: slow`. Together they read
`healthy ≤ 5 · warn above 0`. A KPI's condition is said `KPI`.

### 3.5 A condition that cannot hold together is a declaration error

These rules are judged on the monitor as its homes resolve it
([§FS-021-monitors.4.4](FS-021-monitors.md#44-the-homes-overlay-field-by-field)), never on one home alone. A `warn` needs a `healthy`
to narrow: a `warn` where none applies, neither declared, brought, nor its
kind's default, is a declaration error, while `"healthy": null` sets any
`warn` aside, because a KPI has no band. A `warn` laxer than `healthy` is a
declaration error too, where that can be decided: both have a numeric bound
on the same side, and the warn bound lets through a value the healthy bound
refuses, such as `healthy {"max": 5}` with `warn {"max": 7}`. Such a monitor
is not read until its declaration is mended ([§FS-021-monitors.4.6](FS-021-monitors.md#46-a-declaration-ephor-cannot-use-is-shown-and-not-read)).

### 3.6 Ephor decides health from the declared condition

What judges a monitor is the first of these that exists:

1. the site's `healthy`;
2. the manifest's;
3. the one its samples bring ([§FS-021-monitors.4.3](FS-021-monitors.md#43-a-sample-may-describe-its-own-monitor));
4. the sample's own `ok`: `true` is `ok`, `false` is `down`;
5. the kind's default ([§FS-021-monitors.3.2](FS-021-monitors.md#32-each-kind-has-a-default-and-null-removes-it)).

`"healthy": null` in any of the first three stops the search there, and the
monitor is a KPI whatever its samples say. `warn` is resolved the same way
among the first three, field by field like everything a monitor declares
([§FS-021-monitors.4.4](FS-021-monitors.md#44-the-homes-overlay-field-by-field)), and narrows whichever `healthy` applies.

A monitor that only its samples' `ok` judges is a status while its last
accepted sample states one, and a KPI once a sample states none; its open
episodes then end ([§FS-021-monitors.14.4](FS-021-monitors.md#144-a-monitor-that-stops-being-watched-as-it-was-ends-its-episodes)).

The site has the last word because the watch is the person's. The sample's
own `ok` comes after every condition said, because a script that reports a
value should not also be the one to decide it is fine. The kind's default
comes last, below even the sample's `ok`, because it is the shipped default,
and a shipped default sits below what the project says
([§REQ-001-boundary.2](../requirements/REQ-001-boundary.md#2-three-homes-one-resolution-order)). This is [§DF-008-ephor-judges-a-monitors-health](../decisions/functional/DF-008-ephor-judges-a-monitors-health.md#df-008-ephor-judges-a-monitors-health-ephor-decides-a-monitors-health-from-the-declared-condition-site-over-manifest-and-from-the-samples-own-ok-only-where-none-is-declared).

### 3.7 Warn is amber and never a matter

`warn` is shown, in amber, on the page and in the command. It opens no matter
and holds none open. It exists so a reader sees a value heading for its
bound before the bound is crossed.

## 4. Where a monitor is declared

These follow the order of [§FS-006-project-interface.1](FS-006-project-interface.md#1-the-three-homes), site over manifest,
with what a sample says of itself in place of the probe.

### 4.1 The manifest

`ephor.json` gains `monitors`, a list of declarations. A repository that wants
its numbers watched says so in the repository, and states how often each is
worth reading, because it knows how often its number changes and what a read
costs.

```json
{
  "v": 1,
  "monitors": [
    { "id": "releasable", "title": "Releasable", "kind": "boolean",
      "from": { "command": "scripts/kpis.sh" },
      "workflow": "release-readiness" },
    { "id": "reports-waiting", "title": "Reports waiting",
      "kind": "number", "unit": "count", "better": "lower",
      "healthy": { "max": 5 }, "warn": { "max": 0 },
      "from": { "command": "scripts/kpis.sh" } },
    { "id": "coverage", "title": "Line coverage", "kind": "number",
      "unit": "%", "better": "higher", "healthy": { "min": 80 },
      "from": { "file": "target/kpi/coverage.json" },
      "every": "1d", "at": "06:00" }
  ]
}
```

The manifest that declares a project's monitors is the one in the project's
checkout of its default branch where one resolves, and the forest root's
otherwise ([§FS-006-project-interface.2](FS-006-project-interface.md#2-the-manifest-is-offered-never-required)): the place a monitor runs
([§FS-021-monitors.5.3](FS-021-monitors.md#53-the-place)). A manifest that cannot be parsed is said, never
passed over as if it declared nothing ([§FS-021-monitors.17.3](FS-021-monitors.md#173-the-slot-fails-only-when-the-machinery-does)).

A declaration's fields: `id`, `title`, `kind`, `unit`, `better`, `of`,
`levels`, `healthy`, `warn`, `from` ([§FS-021-monitors.5](FS-021-monitors.md#5-where-a-value-comes-from-from)), `every` and `at`
([§FS-021-monitors.6.1](FS-021-monitors.md#61-every-and-at)), `timeout` ([§FS-021-monitors.5.5](FS-021-monitors.md#55-exit-codes-and-the-timeout)), `drop_after` and
`recover_after` ([§FS-021-monitors.10.1](FS-021-monitors.md#101-drop_after-and-recover_after)), `workflow` or `action`
([§FS-021-monitors.16.1](FS-021-monitors.md#161-the-monitors-own-workflow-comes-first)), and `description`, one line saying what the number
is for. Only `id` is required. A title defaults to the id.

### 4.2 The site

A project's block in the site configuration gains the same `monitors` list.
It adds monitors to a project whose repository says nothing, which is how a
person watches a repository without touching it ([§GOAL-005-costless](../goals.md#goal-005-costless-watching-costs-the-watched-nothing)), and it
overrides the fields of a manifest's monitor by naming its id.

A site declaration is held to the shape the manifest schema publishes for a
declaration, and a field that shape refuses refuses the site configuration,
as any malformed site field does. The rules a shape cannot see cost only the
one monitor ([§FS-021-monitors.4.6](FS-021-monitors.md#46-a-declaration-ephor-cannot-use-is-shown-and-not-read)).

The site configuration refuses a key it does not know, so a site that writes
`monitors` needs an ephor that reads this specification installed first. An
older one refuses the whole configuration rather than ignoring the key. A
manifest's `monitors` costs older ephors nothing: they ignore what they do
not know ([§FS-006-project-interface.11](FS-006-project-interface.md#11-the-interface-is-versioned)).

### 4.3 A sample may describe its own monitor

An answer may carry a sample whose id nothing declared. It may bring its own
`title`, `kind`, `unit`, `better`, `levels`, `healthy`, `warn` and
`description`, and it is then a monitor like any other: a script that reports
what it knows is a source, as a custom-status script already is
([§FS-006-project-interface.4](FS-006-project-interface.md#4-the-answer-envelope)). Its kind, where nothing states one, follows
from the value: `true` or `false` is a boolean, a number is a number, and
points with no value are a series. A string value with no `levels` makes the
sample unreadable ([§FS-021-monitors.8.2](FS-021-monitors.md#82-one-bad-sample-is-that-monitor-unreadable-never-the-answer-refused)).

A self-described monitor's fields are its last accepted sample's, so a
script that changes its title changes the title. The site may declare the
same id to add or override any field, a condition included.

### 4.4 The homes overlay field by field

A monitor's fields are resolved one by one: the site's where it names the
field, the manifest's otherwise, the sample's own where neither does. So a
site that writes only `{"id": "coverage", "healthy": {"min": 75}}` changes
the bound and keeps everything else the repository said. `healthy`, `warn`
and `from` are each taken whole from the home that names them, never merged
key by key, and `workflow` and `action` are one choice
([§FS-021-monitors.16.1](FS-021-monitors.md#161-the-monitors-own-workflow-comes-first)). The site may write `null` for `warn`, `at`, `from`,
`workflow` or `action` to set aside what the manifest says.

What a sample says of its monitor is the fields that describe one
([§FS-021-monitors.8.1](FS-021-monitors.md#81-a-sample)). The rest of a sample is the measurement, and no
declaration overrides it; the one field both may carry, `of`, is the
sample's where it states one ([§FS-021-monitors.2.2](FS-021-monitors.md#22-number)).

A declaration that names `from` is read only from it. A declaration that
names none is read from the answer that brings its samples
([§FS-021-monitors.5.6](FS-021-monitors.md#56-which-samples-an-answer-gives)).

A monitor whose kind no home states takes it from its `levels`, which make it
a level, or from its condition, `is` making it a boolean and `in` or
`at_most` a level, and otherwise from its first accepted sample
([§FS-021-monitors.4.3](FS-021-monitors.md#43-a-sample-may-describe-its-own-monitor)). A sample whose value fits another kind than the
monitor's is an unreadable read of it ([§FS-021-monitors.8.2](FS-021-monitors.md#82-one-bad-sample-is-that-monitor-unreadable-never-the-answer-refused)). When a
monitor's kind changes, its history starts again and its open episodes end
([§FS-021-monitors.14.4](FS-021-monitors.md#144-a-monitor-that-stops-being-watched-as-it-was-ends-its-episodes)).

### 4.5 A row that trusts only descriptions reads files and runs no command

A registry row may narrow a manifest's trust to its descriptions
([§FS-006-project-interface.2](FS-006-project-interface.md#2-the-manifest-is-offered-never-required)). Such a row keeps the manifest's monitor
declarations and the monitors read `from` a file, because a file is data.
It drops `from.command`: such a monitor is shown as **not run**, with the
sentence `not run: this registry row reads descriptions only`, and its
health is `unknown`. It has no value, opens no matter, and is never overdue,
and no header, overview line or `--check` counts it, because the person chose
it. A site declaration of the same id that names its own `from` is read as
the site declares it.

### 4.6 A declaration ephor cannot use is shown and not read

A declaration that breaks a rule of this specification is not read: one that
does not fit the shape the manifest schema publishes for a declaration, such
as a misspelt kind, an empty condition or a duration in seconds; a condition
that does not fit its kind ([§FS-021-monitors.3.1](FS-021-monitors.md#31-the-language)); a `warn` that cannot hold
with its `healthy` ([§FS-021-monitors.3.5](FS-021-monitors.md#35-a-condition-that-cannot-hold-together-is-a-declaration-error)); an `at` that does not fit its
`every` ([§FS-021-monitors.6.1](FS-021-monitors.md#61-every-and-at)); or an id declared twice in one home. The page
and the command show it with what is wrong, its health is `unknown`, and
every refresh notes it. It opens no matter: the error is in a declaration,
which the person or the repository mends, not in what the monitor measures.
The rest of the project's monitors are read as usual.

So a manifest's declarations are checked one by one, and one that does not
fit its shape costs that monitor and nothing else: not the manifest's other
monitors, and not its checks, offers or task stores. `ephor validate
--manifest` holds every declaration to the shape and refuses the manifest,
naming each one that fails, so a repository's own check still catches the
mistake before ephor reads it.

A key a declaration does not know is noted on its row, such as
`unknown field heathy`, and the declaration is read without it. A misspelt
`healthy` is then seen, rather than silently turning a status into a KPI.

## 5. Where a value comes from: `from`

### 5.1 A command is a summons, run once however many monitors name it

`"from": {"command": "scripts/kpis.sh"}` is a summons
([§FS-006-project-interface.3](FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)): the command runs through a shell, receives the
`EPHOR_*` environment, and answers with its exit code and in the file named
by `$EPHOR_ANSWER`. Several monitors may name the same `from`. Ephor runs it
once per read and takes each monitor's sample out of the one answer, so one
script can answer for ten numbers. Two `from`s are the same when they name
the same command and the same `cwd`.

### 5.2 A file is read and nothing is run

`"from": {"file": "target/kpi/coverage.json"}` names a file in the answer
shape ([§FS-021-monitors.8](FS-021-monitors.md#8-the-answer-carries-monitors)). Ephor reads it and runs nothing. This is the
cheapest binding: whatever already produces the number, a build step or a
hook, writes the file, and ephor only reads it. A missing file, or one that
is not an answer, is an unreadable read ([§FS-021-monitors.7.2](FS-021-monitors.md#72-four-causes)).

### 5.3 The place

A monitor runs in its project's checkout of the default branch: the default
branch's workspace where one resolves, and the forest root otherwise. A
monitor is about no item, so no item's workspace takes part. `from.cwd` may
name another repository of the forest as `repo:<name>`, or the forest root as
`root`, as a manifest entry may elsewhere ([§FS-006-project-interface.3](FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)), each
resolved from that checkout. A command runs there, and a file's path
resolves against it. A file's path that is absolute or climbs out of that
place with `..`, and a `repo:` that names `.` or `..`, break a rule, and the
declaration is shown and not read ([§FS-021-monitors.4.6](FS-021-monitors.md#46-a-declaration-ephor-cannot-use-is-shown-and-not-read)).

### 5.4 The environment

A monitor command receives the dossier of its project with every variable
about an item set empty, because a monitor is about no item
([§FS-006-project-interface.3](FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)). It also receives `EPHOR_ANSWER`, and
`EPHOR_MONITORS`: the ids this run reads, separated by commas, so a script
that answers for many monitors may compute only those.

### 5.5 Exit codes and the timeout

Exit `0` is a read, and the answer is taken. `75` is **parked**: the command
cannot answer now, no sample is taken, and the last one stands. Any other
exit is an unreadable read. A command gets 30 seconds, and `timeout`, a whole
number of seconds, sets another limit for a monitor; a run that reads several
monitors gets the longest limit among them. A command still running at its
limit is stopped, and that is an unreadable read saying it timed out.

A monitor command must not run `ephor refresh`: the refresh that runs it has
claimed the monitors it reads ([§FS-021-monitors.6.9](FS-021-monitors.md#69-a-due-monitor-is-read-once-however-many-refreshes-find-it-due)), so the inner one would
find them claimed and read none of them, and would refresh the whole project
again inside one monitor's time limit.

### 5.6 Which samples an answer gives

An answer gives each monitor the sample carrying its id:

- A declared monitor that names a `from` takes its sample from that `from`'s
  answer only, and is read when its `from` runs while it is due
  ([§FS-021-monitors.6.6](FS-021-monitors.md#66-one-command-several-cadences)). A sample for it in any other answer is ignored.
- A declared monitor that names no `from`, and a monitor nothing declared
  ([§FS-021-monitors.4.3](FS-021-monitors.md#43-a-sample-may-describe-its-own-monitor)), are read from the answer that brings them: the
  project's custom-status answer ([§FS-021-monitors.8.3](FS-021-monitors.md#83-a-custom-status-answer-may-carry-samples-too)), or the answer of a
  `from` other monitors name. The first answer to bring one **owns** it, and
  the store records which. A sample for it in any other answer is ignored,
  and it is read whenever its owner's answer is taken
  ([§FS-021-monitors.6.8](FS-021-monitors.md#68-the-cadence-of-a-monitor-read-from-the-answer-that-brings-it)).

An answer taken to read a monitor that lacks its sample is an unreadable read
of it, saying `no sample for <id>`. An answer that fails is an unreadable
read of every monitor it was taken to read, and one that parks is a parked
read of each ([§FS-021-monitors.5.5](FS-021-monitors.md#55-exit-codes-and-the-timeout)). Two samples for one id in one answer are
an unreadable read of that monitor, saying `two samples for <id>`. A run that
read nothing for a monitor that was not due records nothing for it
([§FS-021-monitors.6.6](FS-021-monitors.md#66-one-command-several-cadences)).

Until an answer owns a declared monitor that names no `from`, every answer a
plain refresh of its project takes is taken to read it, and each refresh
that finds it in none is an unreadable read saying `no answer carries <id>`.
So a declaration nothing answers for turns unknown after its first grace,
like any monitor whose first reads fail ([§FS-021-monitors.7.1](FS-021-monitors.md#71-a-value-holds-until-the-next-slot-plus-a-grace)).

A monitor nothing declared that no answer has brought for 30 days is
forgotten: its record and its history go, and its open episodes end
([§FS-021-monitors.14.4](FS-021-monitors.md#144-a-monitor-that-stops-being-watched-as-it-was-ends-its-episodes)). A declared monitor is never forgotten. It stays
unknown until an answer brings it or its declaration goes.

## 6. Cadence: when a monitor is read

### 6.1 `every` and `at`

`every` is a duration: a whole number and a unit, `m` for minutes, `h` for
hours, `d` for days and `w` for weeks, such as `10m`, `4h`, `1d` or `1w`. It
defaults to `10m`, or, for a monitor that states an `at`, to `1d`, and to
`1w` where that `at` names a day.

`at` pins the slots to the clock, in the machine's local time zone. It is
`HH:MM`, or a day and a time such as `Mon 06:00`, with the days `Mon` to
`Sun`. An `at` with a day needs an `every` of whole weeks. An `at` without a
day needs an `every` that divides a day evenly or is a whole number of days.
Any other pairing is a declaration error, because its slots would wander off
the clock `at` pins them to.

- `{"every": "1d", "at": "06:00"}` reads after a nightly job;
- `{"every": "1w", "at": "Mon 06:00"}` reads once a week;
- `{"every": "8h", "at": "00:00"}` reads at 00:00, 08:00 and 16:00.

### 6.2 The manifest states the cadence, and the site may change it either way

The manifest states `every` and `at`, because the repository knows how often
its number changes and what a read costs. The site may change either, faster
or slower, and may write `"at": null` to unpin the manifest's `at`, because
the machine is the person's. The **effective cadence** is the result.

### 6.3 Slots

Without `at`, slots count from the monitor's **anchor**, ephor's first
attempt to read it, whatever that attempt found, one `every` apart. With
`at`, they fall on the local clock at the time `at` names and every `every`
from it. Slots are times on the wall clock, so a daily `06:00` stays at 06:00
across a change of clocks: a slot the change skips is due at the first
instant after the gap, and a slot it repeats is due at its first occurrence.

A monitor's **interval** is its effective `every`. The grace
([§FS-021-monitors.7.1](FS-021-monitors.md#71-a-value-holds-until-the-next-slot-plus-a-grace)) and the hold's defaults ([§FS-021-monitors.10.1](FS-021-monitors.md#101-drop_after-and-recover_after)) are
counted from it, and a change of clocks changes no interval.

A monitor ephor has not read yet is due at once. When the effective cadence
changes, the next slot is counted again under the new cadence, from the
anchor or the clock, and a slot the new cadence has already passed is due at
once. A next slot more than one interval after now, which only a clock set
back can leave, is counted again as the first slot after now.

### 6.4 A refresh reads the monitors that are due, and no others

A plain refresh of a project reads its monitors whose slot has come, and no
others ([§FS-011-command-line.9.2](FS-011-command-line.md#92-a-refresh-narrowed-to-one-source-or-one-capability)). After a read, the monitor's next slot is
the first one after the read. So a slot that passed while the machine was
off, or while no refresh ran, is read once on the next refresh, not once per
missed slot, and a refresh every quarter hour costs a weekly monitor one
read a week.

### 6.5 Cadence is a floor between reads, not a promise

A refresh is the tick. Ephor reads a monitor no more often than its cadence,
and no sooner than a refresh comes. A slot is therefore read up to one
refresh interval late: on a machine whose refresh runs about every thirty
minutes, a `10m` monitor is read about every thirty minutes. That costs a
weekly monitor nothing and is why a fast monitor needs a fast refresh. How
often refresh runs is the site's to arrange; nothing here installs a timer.

### 6.6 One command, several cadences

A command that answers several monitors runs when any of them is due. Only
the due monitors record a reading, so each keeps its own cadence, and the
samples of the others are dropped.

### 6.7 Reading out of cadence

Two things read a monitor whether or not it is due:

- `r` on the Monitors page, which reads every monitor of the row's project
  now ([§FS-021-monitors.18.10](FS-021-monitors.md#1810-keys)), and `ephor refresh --monitors-now`, which is
  the same read for every project the refresh refreshes
  ([§FS-021-monitors.21.4](FS-021-monitors.md#214-reading-is-refresh));
- the end of work laid on the monitor's matter, so a fix is confirmed when it
  lands rather than at the next weekly slot ([§FS-021-monitors.15](FS-021-monitors.md#15-work-ended-still-down)).

An out-of-cadence read is a reading and counts toward a drop or a recovery.
It does not move the schedule: the next slot stays where it was.

### 6.8 The cadence of a monitor read from the answer that brings it

A monitor read from the answer that brings it ([§FS-021-monitors.5.6](FS-021-monitors.md#56-which-samples-an-answer-gives)) takes
the cadence of that answer. For the answer of a `from`, that is the fastest
effective cadence among the declared monitors that name that `from`, and
every run of the `from` reads it. For a custom-status answer, and before any
answer owns it, that is the refresh itself, judged as `every: 10m` for
freshness. A site declaration of the id may state its own cadence for
freshness, and the read that brings it still decides when it is read.

### 6.9 A due monitor is read once, however many refreshes find it due

Two refreshes may overlap: a screen's refresh beneath it, one from the
command line, one from a timer, one from `ephor doctor`. Each would find the
same monitor due, and one failing build would count as two failing readings.
So a read **claims** its monitors first:

1. Under the project's store lock, it re-reads the record, takes the due
   monitors nobody has claimed, writes a claim naming the process, the time
   and the slot, and releases the lock.
2. It runs the reads.
3. Under the lock again, it records the readings, the health, the last
   sample and the next slot, and clears the claim.

A claim older than the command's timeout plus a minute is void, because the
process that wrote it is gone, and the next refresh reads the monitor as due.
An out-of-cadence read claims too. A refresh that finds a monitor claimed
reports its previous state.

## 7. Freshness: one rule for every way a value goes missing

### 7.1 A value holds until the next slot plus a grace

A monitor's value holds until the first slot after the reading that brought
it, plus a **grace**. The grace is a quarter of the interval, and at least an
hour. So a value read at its slot holds for one interval and the grace:

- a `10m` monitor's value holds for 1h 10m;
- a daily one's for 30 hours;
- a weekly one's for 8 days and 18 hours.

A read that brings nothing measured since the reading held, or only a sample
measured more than one interval before the read, is **stale**: it is not a
reading, it moves the schedule, and it does not extend the hold
([§FS-021-monitors.9.2](FS-021-monitors.md#92-what-a-reading-is)). A sample is measured at its `at`, and at the time it
was read where it states none. A sample whose `at` is later than the moment
it is read counts as measured when it was read, and the row notes the skew.

A monitor with no reading yet holds as if ephor had read it, with no value,
when it first saw the monitor, so one whose first reads fail turns unknown
after its first interval and the grace like any other. Until then it is
`not read yet` ([§FS-021-monitors.18.7](FS-021-monitors.md#187-the-health-column)).

### 7.2 Four causes

When a slot passes without a new reading, the monitor is **late**, and the
cause is what the attempts since then found:

| Cause | What happened | Whose problem |
| --- | --- | --- |
| `unreadable` | the read failed: an exit other than `0` or `75`, a timeout, a missing file, an answer the schema refuses, or no sample for the monitor in the answer taken to read it | the project's |
| `parked` | the command exited `75` at every try since the slot | the project's, once it outlasts the grace |
| `stale` | the read worked, but its sample was measured more than one interval before the read, or is the measurement already held, so whatever writes it did not run | the project's |
| `overdue` | nothing tried: no refresh has read the project since the slot | the watch's |

The last attempt decides, so a monitor overdue for a day and then read and
refused is unreadable. While the monitor slot itself fails
([§FS-021-monitors.17.3](FS-021-monitors.md#173-the-slot-fails-only-when-the-machinery-does)), every monitor of the project is late with the slot's
error as an unreadable cause, never overdue: a refresh did reach the project,
and the slot already says what stopped it.

### 7.3 Late: the last health holds and the cause is named

While the value still holds, the monitor keeps its health, and the page names
the cause and how long it has been late: `parked 3h 50m`, `overdue 2h 50m`.
Nothing opens and nothing moves.

### 7.4 Unknown

Once the value no longer holds, the monitor is **unknown**, whatever the
cause, and the page says which. A status that turns unknown opens a matter of
its own ([§FS-021-monitors.11.2](FS-021-monitors.md#112-an-unknown-status-opens-an-episode-of-its-own)), unless the cause is `overdue`. A KPI that
turns unknown says so on the page and opens nothing, as a KPI never does
([§FS-021-monitors.1.3](FS-021-monitors.md#13-health-is-one-of-four-states)). A status whose drop is open keeps that matter, still
`down`: nothing has shown it recovered ([§FS-021-monitors.14.1](FS-021-monitors.md#141-a-drop-ends-after-recover_after)).

### 7.5 Overdue is the watch's own failure

`overdue` means ephor did not look, which is no fault of the project, so it
is never laid on the project as a matter. It is said instead where the watch
is judged: in the Monitors page's header ([§FS-021-monitors.18.2](FS-021-monitors.md#182-the-header)), on
Waiting's overview line ([§FS-021-monitors.20.1](FS-021-monitors.md#201-waitings-overview-line-names-the-monitors)), and by `ephor doctor`
([§FS-021-monitors.20.2](FS-021-monitors.md#202-the-doctor-reads-the-store-and-names-what-was-overdue)). Those count a monitor as overdue once it is unknown
for that cause.

## 8. The answer carries `monitors`

### 8.1 A sample

The answer envelope gains `monitors`, a list of samples, by addition, so every
verb that does not read it ignores it ([§FS-006-project-interface.4](FS-006-project-interface.md#4-the-answer-envelope)).

```json
{
  "v": 1,
  "monitors": [
    { "id": "releasable", "value": false,
      "detail": "widget#12 is open at high priority",
      "url": "https://forge.example.org/acme/widget/issues/12" },
    { "id": "reports-waiting", "value": 6 },
    { "id": "coverage", "value": 82.4, "at": "2026-11-25T05:40:00Z" },
    { "id": "parse-bench", "kind": "series", "unit": "ms", "better": "lower",
      "title": "Parse benchmark",
      "points": [ { "x": "a4f5f87", "v": 43.1 }, { "x": "1ee55dc", "v": 41.0 } ] }
  ]
}
```

A sample's fields:

- `id`, the monitor it is for; required.
- `value`, a boolean, a number or a level's name; a series may leave it out.
- `of`, for a fraction; the sample's is that reading's
  ([§FS-021-monitors.2.2](FS-021-monitors.md#22-number)).
- `at`, when it was measured, as a time with its offset.
- `ok`, the sample's own judgement, used only where no `healthy` applies from
  any home; it comes before the kind's default ([§FS-021-monitors.3.6](FS-021-monitors.md#36-ephor-decides-health-from-the-declared-condition)).
- `detail`, one line, shown on the row and carried by the matter.
- `url`, where to look.
- `points`, a series' points, or a number's own history.
- `title`, `kind`, `unit`, `better`, `levels`, `healthy`, `warn` and
  `description`, which describe a monitor, and which a declaration overrides
  ([§FS-021-monitors.4.4](FS-021-monitors.md#44-the-homes-overlay-field-by-field)).

Paths in a sample resolve against the place the monitor ran, as every
answer's do.

### 8.2 One bad sample is that monitor unreadable, never the answer refused

A sample is checked on its own. One that does not fit its shape, or whose
value does not fit its monitor's kind, is an unreadable read of that monitor.
A sample with no `id`, or with one that is not an id, is noted on the refresh
and read for no monitor. The rest of the answer is taken, so one script's
mistake about one number never blinds the watch to the others, and never
refuses a custom-status answer's matters.

An answer that is not JSON is still refused whole, as any answer that is not
JSON is, and `NaN` and `Infinity` are not JSON. A script with no number to
give writes its sample without `value`, which is an unreadable read of that
one monitor.

### 8.3 A custom-status answer may carry samples too

A project's custom-status answer in the `answer` format may carry `monitors`,
so one site script can report matters and samples together. Its samples are
self-described, or fill declarations that name no `from`
([§FS-021-monitors.5.6](FS-021-monitors.md#56-which-samples-an-answer-gives)). They are read on a plain refresh only, as every
monitor is ([§FS-021-monitors.17.4](FS-021-monitors.md#174-a-narrowed-refresh-reads-no-monitor)), and their matters belong to the monitor
source, never to the custom-status one ([§FS-021-monitors.11.4](FS-021-monitors.md#114-a-matter-of-the-monitor-source-of-kind-status)).

### 8.4 The shapes are published

The manifest schema publishes the declaration, and the answer schema
publishes the sample (`ephor schema`). Each accepts any entry in its
`monitors` and publishes the strict shape of one entry beside it, which is
what each declaration and each sample is checked against on its own
([§FS-021-monitors.4.6](FS-021-monitors.md#46-a-declaration-ephor-cannot-use-is-shown-and-not-read), [§FS-021-monitors.8.2](FS-021-monitors.md#82-one-bad-sample-is-that-monitor-unreadable-never-the-answer-refused)). `ephor guide monitors`
explains the protocol on one page ([§FS-020-guide.6](FS-020-guide.md#6-monitors-how-a-repository-makes-its-numbers-watchable)).

## 9. History

### 9.1 Where it is kept

Ephor keeps monitors in its state directory, under `monitors/<project>/`,
beside the feed cache and apart from it, so a change of the cache's model
never loses history. `monitors.json` holds, per monitor:

- its schedule: when ephor first saw it, its anchor, the last read, the next
  slot, and since when it has been parked;
- its health: the health after the hold and since when, the last reading's
  own health and how many readings in a row have had it, the last ok value
  and its time, and the cause of an unknown;
- its open drop episode and its open unknown episode, each with when it
  opened and when its matter last moved ([§FS-021-monitors.13.1](FS-021-monitors.md#131-what-is-movement)), and for a
  drop the worst `level` value it has reached and the condition it was last
  judged under;
- the episodes it has had, each with its start, its end and how it ended,
  kept as long as the history they fall in;
- the work laid on its matters that it has seen end ([§FS-021-monitors.15](FS-021-monitors.md#15-work-ended-still-down));
- which answer owns it, where it is read from the answer that brings it
  ([§FS-021-monitors.5.6](FS-021-monitors.md#56-which-samples-an-answer-gives));
- any claim ([§FS-021-monitors.6.9](FS-021-monitors.md#69-a-due-monitor-is-read-once-however-many-refreshes-find-it-due)); and
- the last accepted sample whole.

`<id>.jsonl` holds the monitor's readings in the order ephor took them, one
line each: `{t, v, health}`, where `t` is when the sample was measured and
`health` is the reading's own, before the hold of [§FS-021-monitors.10.1](FS-021-monitors.md#101-drop_after-and-recover_after).
A reading taken out of cadence ([§FS-021-monitors.6.7](FS-021-monitors.md#67-reading-out-of-cadence)) also carries
`out_of_cadence: true`, which the detail view notes ([§FS-021-monitors.19](FS-021-monitors.md#19-the-detail-view)).

### 9.2 What a reading is

| Event | Counts toward a drop or recovery | Appended to history | Moves the schedule |
| --- | --- | --- | --- |
| a read at a slot that brings a valid sample | yes | yes | yes |
| a read out of cadence that brings a valid sample | yes | yes | no |
| exit `75`, parked | no | no | the slot passes, and parked-since is set |
| an unreadable read | no: the health is unknown, not down | a `{t, health: "unknown"}` marker, `t` when it was read | yes |
| a stale read: the measurement already held, one measured before it, or one measured more than one interval before the read; for a file, one whose content and modification time have not changed | no | no | yes |
| a repository's `points` | no | no: the repository keeps them | no |

An out-of-cadence read never moves the schedule, whatever it brings
([§FS-021-monitors.6.7](FS-021-monitors.md#67-reading-out-of-cadence)). There is one line per reading, so a monitor's history
grows with its cadence, not with how often refresh runs.

### 9.3 Retention

Retention counts readings as well as days, because a weekly monitor's year is
52 lines:

- every reading is kept for 30 days;
- after that, one line per hour holds the hour's least, mean and greatest
  value, `{t, min, mean, max, n, health}`, up to 90 days;
- after that, one line per day, up to two years, and nothing older;
- a monitor whose interval is a day or longer is never thinned, and is still
  cut at two years;
- the last 104 readings are kept whatever their age: two years of a weekly
  monitor.

A thinned line's health is the worst among its readings, an unknown marker
included. A boolean's or a level's thinned line has no `mean`: its `min` and
`max` are the first and the last value among its readings in the kind's
order, `false` before `true`, and levels in the order `levels` lists them.
Unknown markers are not readings: they are thinned and cut by age alone, and
never kept as one of the last 104. Thinning runs under the store lock, at
most once a day per project. `ephor doctor` reports the store's size
([§FS-021-monitors.20.2](FS-021-monitors.md#202-the-doctor-reads-the-store-and-names-what-was-overdue)).

### 9.4 What deleting the store loses

The state directory may be deleted. For monitors that loses ephor's own
history, so the graphs start again (a repository's `points` survive, since
the repository keeps them); it loses each schedule, so every monitor is due
at once; and it loses every hold count, every open episode, and which answer
owns a monitor read from the answer that brings it. A monitor still down
after that opens a new episode, which is a new matter, and the matter of the
old one leaves the feed. The work laid on it stays in the ledger.

## 10. A monitor drops and recovers by readings in a row

### 10.1 `drop_after` and `recover_after`

A status moves between passing and `down` by counts of readings, set per
monitor:

- `drop_after` is how many failing readings in a row make it `down`. It
  defaults to 1: a red main branch is news at once.
- `recover_after` is how many passing readings in a row end a drop. It
  defaults to 2 when the interval is shorter than an hour, so one good
  reading in the middle of a flap does not close the matter, and to 1 at an
  hour or longer, so a weekly monitor does not hold a fixed matter open for
  another week.

A reading fails when its value fails the `healthy` that judges it, or its
`ok` is false where `ok` judges, and passes otherwise.

### 10.2 Only readings count

The counts run over readings ([§FS-021-monitors.9.2](FS-021-monitors.md#92-what-a-reading-is)). A reading of the other
kind starts the count again. An unreadable, parked or stale read is not a
reading, so it neither adds to a count nor breaks it.

### 10.3 Between ok and warn there is no hold

A passing monitor is `warn` or `ok` by its latest reading alone. Neither is a
matter, so neither needs holding. A down monitor stays `down` until it
recovers, whatever the band of its passing readings.

## 11. Every drop is an episode, and every episode a matter

### 11.1 A drop opens an episode

When a status turns `down`, an **episode** opens, named by the time ephor
read the reading that turned it. Ephor lays work once per matter, so the next
drop after a recovery is a new episode, a new matter, and new work.

### 11.2 An unknown status opens an episode of its own

When a status turns unknown for a cause other than `overdue`
([§FS-021-monitors.7.4](FS-021-monitors.md#74-unknown)), an unknown episode opens beside any drop episode,
named by the moment its value stopped holding. It is a separate matter,
because the work it asks for is mending the read, not the number.

### 11.3 The keys

An episode is named by its time in UTC, `%Y%m%dT%H%M%SZ`, such as
`20261125T060000Z`. An episode is never named by a time at or before the one
the monitor's previous episode of its sort was named by; where it would be,
it takes the second after. The keys are:

- `monitor:<project>/<id>@<episode>` for a drop;
- `monitor:<project>/<id>.unknown@<episode>` for an unknown monitor.

A key carries no `:` after its source's, so a short id that cuts at the last
`:` keeps the whole subject.

### 11.4 A matter of the `monitor` source, of kind status

A monitor's matters come from the `monitor` source and are of kind `status`,
so they sit in the Status category of the overview
([§FS-003-feed-categories.1](FS-003-feed-categories.md#1-the-categories)). The source is never `custom-status`, even for a
sample a custom-status answer brought, so an entry selecting custom-status
matters does not take them.

## 12. What a matter carries

### 12.1 The title

A drop's title names the monitor, its value and its condition, then the
sample's `detail` where there is one:

- `Line coverage dropped: 79.2% (healthy ≥ 80%)`
- `Releasable dropped: no (healthy when yes) · widget#12 is open at high priority`

An unknown matter's title names the cause:

- `Line coverage cannot be read: target/kpi/coverage.json is missing`
- `Mutation score is stale: measured Nov 16`
- `Parse benchmark is parked: every read since Nov 24 12:00 exited 75`

### 12.2 Its state is its episode's, and its time is when it last moved

A drop's matter is in the state `down` for its whole episode, and an unknown
matter in the state `unknown`, whatever the monitor's freshness meanwhile.
Its time is when it last moved: when its episode opened, or the latest move
of [§FS-021-monitors.13.1](FS-021-monitors.md#131-what-is-movement). Once its episode ends, a drop's state is
`recovered`, an unknown episode's is `readable`, and that of an episode
ended because its monitor is no longer watched is `unwatched`
([§FS-021-monitors.14](FS-021-monitors.md#14-recovery-resolves-the-matter)). The value is in the title, the detail and the meta,
and nowhere the feed compares, so a number moving inside its band is not
movement ([§FS-021-monitors.13](FS-021-monitors.md#13-a-matter-moves-on-its-health-not-its-number)). A monitor's matter carries no gate. Its link
is the sample's `url`.

### 12.3 Its meta

The matter's meta carries the facts, so what takes it does not have to find
them again: `monitor`, the id; `value` and `unit`; `condition`, as words
([§FS-021-monitors.3.4](FS-021-monitors.md#34-a-condition-is-said-in-words)); `since`, when its health began; `last_ok_value` and
`last_ok_at`; `detail`; `url`; and on an unknown matter, `cause`. A command
summoned about the matter receives each as `EPHOR_META_<KEY>`, and a workflow
names them in its brief ([§FS-006-project-interface.3](FS-006-project-interface.md#3-a-summons-environment-in-exit-code-and-answer-out)).

### 12.4 In Waiting it is New, with a proposal

A drop and an unknown monitor are New matters with a proposal
([§FS-021-monitors.16](FS-021-monitors.md#16-the-proposal)), counted in Waiting's header like any other
([§FS-019-waiting.1.2](FS-019-waiting.md#12-new-every-matter-nothing-has-been-done-about)).

## 13. A matter moves on its health, not its number

### 13.1 What is movement

Coverage wobbling between 79.1% and 79.3% is not movement. If it were, every
refresh would mark the work stale and append a ticket
([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)). A monitor's matter moves only when:

- a `level` monitor's value reaches a level worse than any its open drop has
  reached, such as `degraded` to `failed`;
- the `healthy` that judges it changes, from whichever home, while its drop
  is open; a changed `warn` is not movement, because `warn` is never a
  matter;
- it is still down, or still unknown, after the work laid on it ended
  ([§FS-021-monitors.15](FS-021-monitors.md#15-work-ended-still-down)).

A status crossing from `warn` into `down` is a drop: it opens the episode
rather than moving one. A changed condition judges from the next reading on,
and the health recorded and the hold's count stand until then.

### 13.2 Resurfacing names the reason

A matter that moves says why, as every resurfaced matter does
([§FS-007-matters.5](FS-007-matters.md#5-an-event-moves-state-and-resurfacing-names-its-reason)): `level worse: degraded → failed`, `condition changed:
healthy ≥ 80% → healthy ≥ 85%`, or `still down after its work ended`. Moving
lifts a dismissal and makes the work laid on the matter stale, as any
movement does ([§FS-005-dispatch.5](FS-005-dispatch.md#5-an-item-that-moved-reopens-its-work)). Still down after its work ended does
more: that work is over, so the matter is New again and is offered work again
([§FS-019-waiting.1.2](FS-019-waiting.md#12-new-every-matter-nothing-has-been-done-about)). Moving never opens a new episode.

## 14. Recovery resolves the matter

### 14.1 A drop ends after `recover_after`

When a down status has had `recover_after` passing readings in a row, its
episode ends. The matter turns terminal, as an answer's stated finality does
([§FS-003-feed-categories.2](FS-003-feed-categories.md#2-recent)), and says `recovered at <time> (<value>)`, such
as `recovered at Nov 25 06:02 (81.0%)`. While the monitor is unknown it
cannot recover, because nothing has been read.

### 14.2 An unknown episode ends on the next reading

An unknown episode ends with the first reading, whatever its health, and its
matter turns terminal the same way, saying `read again at <time> (<value>)`.
A stale read is not a reading, so it ends nothing ([§FS-021-monitors.9.2](FS-021-monitors.md#92-what-a-reading-is)).

### 14.3 What happens to it then

A finished matter is reported as long as the feed's recency window keeps
finished matters ([§FS-003-feed-categories.3](FS-003-feed-categories.md#3-the-recency-window-is-configured)), and then dropped. Whether it
shows in Recent is Recent's own rule: it does while work laid on it is still
open ([§FS-003-feed-categories.2](FS-003-feed-categories.md#2-recent)). Work still running on a recovered monitor
is the runtime's to finish or cancel; ephor records only that the matter
recovered.

### 14.4 A monitor that stops being watched as it was ends its episodes

A monitor's open episodes end when it is no longer declared or brought
([§FS-021-monitors.5.6](FS-021-monitors.md#56-which-samples-an-answer-gives)), when nothing judges it any more
([§FS-021-monitors.1.2](FS-021-monitors.md#12-a-kpi-is-a-monitor-nothing-judges)), when its kind changes ([§FS-021-monitors.4.4](FS-021-monitors.md#44-the-homes-overlay-field-by-field)), or when
it stops being read because its declaration is in error
([§FS-021-monitors.4.6](FS-021-monitors.md#46-a-declaration-ephor-cannot-use-is-shown-and-not-read)) or its row runs no command for it
([§FS-021-monitors.4.5](FS-021-monitors.md#45-a-row-that-trusts-only-descriptions-reads-files-and-runs-no-command)). Each matter turns terminal saying
`no longer watched at <time>: <reason>`, and is reported as
[§FS-021-monitors.14.3](FS-021-monitors.md#143-what-happens-to-it-then) says. A monitor's id is its identity: a renamed monitor
is the old one gone and a new one declared.

## 15. Work ended, still down

The first plain refresh after work laid on a monitor's matter has ended reads
that monitor out of cadence ([§FS-021-monitors.6.7](FS-021-monitors.md#67-reading-out-of-cadence)), so a fix is confirmed
when it lands. That holds while the matter's episode is open. If that read
brings no passing reading, a drop's matter resurfaces with `still down after
its work ended`; if it brings no reading at all, an unknown matter resurfaces
with `still <cause> after its work ended`, such as `still unreadable after
its work ended`. Either is then New again and is offered work again, in the
same episode ([§FS-021-monitors.13.2](FS-021-monitors.md#132-resurfacing-names-the-reason)), and resurfaces once each time work laid
on it ends. A passing reading counts toward recovery as any does
([§FS-021-monitors.14.1](FS-021-monitors.md#141-a-drop-ends-after-recover_after)), and any reading ends an unknown episode
([§FS-021-monitors.14.2](FS-021-monitors.md#142-an-unknown-episode-ends-on-the-next-reading)).

## 16. The proposal

### 16.1 The monitor's own workflow comes first

A monitor may name a `workflow`, one of the runtime's workflows, or an
`action`, the id of an entry the menu already holds that hands work over, a
recipe or a workflow ([§FS-019-waiting.7.1](FS-019-waiting.md#71-every-new-row-carries-a-proposal)). The two are one choice, and the
site's wins over the manifest's. On the monitor's drop matter that entry
heads the menu, ahead even of what ephor itself recognized
([§FS-004-quick-actions.3](FS-004-quick-actions.md#3-quick-actions-come-first-and-configuration-adds-to-them)) and of the project's offers and the person's
actions, so it is the proposal ([§FS-019-waiting.7.1](FS-019-waiting.md#71-every-new-row-carries-a-proposal)). So "main is red" can
propose a gate fix, and "releasable is false" a release-readiness workflow.
An `action` that names no entry handing work over is passed over, the detail
view says why, and the proposal is what [§FS-021-monitors.16.2](FS-021-monitors.md#162-without-one-triage) says.

### 16.2 Without one, triage

A drop whose monitor names nothing, and every unknown matter, are proposed what
any matter is: the first entry that hands it over, and otherwise the bound
runtime's `triage` workflow ([§FS-019-waiting.7.1](FS-019-waiting.md#71-every-new-row-carries-a-proposal)). An unknown matter is not
proposed the monitor's workflow, because what it needs is the read mended.

### 16.3 It is only a proposal

A monitor never dispatches work by itself. Its matter is processed where it
is shown, as any matter is, or by a recipe the site writes for it
([§FS-005-dispatch.1](FS-005-dispatch.md#1-a-recipe-decides-which-items-deserve-work-and-what-to-ask-for)), because spending agent time on its own match is the
site's to decide.

## 17. The monitor slot

### 17.1 One answer on every plain refresh

A project that has monitors has one more source in its feed, named
`monitor`. Every plain refresh of the project answers it exactly once, with
every open episode's matter and every finished one the recency window still
keeps ([§FS-021-monitors.14.3](FS-021-monitors.md#143-what-happens-to-it-then)), even when no monitor was due. A refresh that
answered nothing would make the slot look gone and its matters leave the feed
until the next slot.

### 17.2 One monitor's failure is that monitor's

An unreadable, parked or stale monitor is that monitor's state, carried in
the slot's answer. It never fails the slot: a slot that failed would keep
every monitor of the project at its last good answer, the ones that just
read fine included, and would turn the refresh's exit code red for one
script's mistake ([§FS-001-forge-interface.6.1](FS-001-forge-interface.md#61-every-contribution-to-one-provider-answer-counts)).

### 17.3 The slot fails only when the machinery does

The slot fails when ephor cannot do the reading at all: the store cannot be
read or locked, the manifest cannot be parsed, or the monitor configuration
cannot be used. It then fails as any source does
([§FS-001-forge-interface.6](FS-001-forge-interface.md#6-a-source-that-did-not-answer-says-so-and-says-which-kind-of-not)): it says why, its last good answer stands, and
its monitors are late with that error as their cause ([§FS-021-monitors.7.2](FS-021-monitors.md#72-four-causes)).

A manifest that cannot be parsed fails the slot only where the project has
monitors without it: the site declares some, or the store keeps a record of
one. Otherwise nobody can tell whether it declares any, so the refresh notes
the manifest's error and reads no monitor of the project.

### 17.4 A narrowed refresh reads no monitor

A refresh narrowed to one source or one capability asks that and nothing else
([§FS-011-command-line.9.2](FS-011-command-line.md#92-a-refresh-narrowed-to-one-source-or-one-capability)), so it reads no monitor and takes no sample from a
custom-status answer. `monitor` is not a source `--source` can name, nor a
provider block site configuration can configure: it is ephor's own reading.

### 17.5 It counts as one source, and for nothing more

Once a project has monitors, declared or kept in its store
([§FS-021-monitors.1.4](FS-021-monitors.md#14-a-project-has-monitors-when-one-is-declared-or-brought)), the slot counts as one of its configured sources
wherever sources are counted, in `ephor doctor` and in `ephor status` alike,
so the sources answering never outnumber the sources configured. Its answer
is ephor's own reading, not the world answering, so it counts for nothing
more: it never spares a refresh total failure ([§FS-001-forge-interface.6.1](FS-001-forge-interface.md#61-every-contribution-to-one-provider-answer-counts)),
never makes a project reachable to the doctor ([§FS-010-doctor.5](FS-010-doctor.md#5-the-answer-is-in-the-exit-code)), and never
makes it observable ([§FS-006-project-interface.10](FS-006-project-interface.md#10-capability-rung-by-rung)). Those verdicts are judged
on the project's other sources alone: one whose only source is the slot has
no source configured for them, so it is not watched rather than unreachable.

## 18. The Monitors page

### 18.1 `M` opens it from any screen

`M` opens the Monitors page from any screen, and `M` or `esc` closes it back
to where the reader was. It reads the store and the cache, as every reading
does, and runs nothing.

### 18.2 The header

The header counts the monitors of the watched projects by what each shows,
then the window and the feed's age:

```text
ephor — monitors   1 down · 2 unknown · 1 failing · 1 warn · 11 ok · 4 KPIs · 1 overdue   window 7d · refreshed 4m ago
```

Each monitor counts once, under `down`, `unknown`, `failing`, `warn`, `ok` or
`KPIs`, by the rank its row shows ([§FS-021-monitors.18.3](FS-021-monitors.md#183-rows-are-grouped-by-project-worst-first)), so an unknown KPI
counts under `unknown`. `overdue` counts the unknown monitors whose cause is
overdue a second time, because that is the watch's own failure, and appears
only when it is not zero. A monitor not run because its row trusts only
descriptions ([§FS-021-monitors.4.5](FS-021-monitors.md#45-a-row-that-trusts-only-descriptions-reads-files-and-runs-no-command)) counts in none of them, and neither does
a monitor with no reading yet whose first grace has not ended
([§FS-021-monitors.7.1](FS-021-monitors.md#71-a-value-holds-until-the-next-slot-plus-a-grace)): neither is yet a failure. The age is the one
Waiting's header prints ([§FS-019-waiting.3](FS-019-waiting.md#3-the-header-and-the-overview-line)).

With no monitors, the page shows the header with every count at zero, and
one line: `no monitors — ephor guide monitors says how to declare one`.

### 18.3 Rows are grouped by project, worst first

Rows are grouped by project. Each monitor shows by a rank, and a glyph that
carries the rank in shape as well as colour, so the page reads without
colour:

| Rank | Glyph | Shows |
| --- | --- | --- |
| 0 | `✖` | down, recovering included, and down while it is also unknown |
| 1 | `?` | unknown, a KPI's included; not run, not read, and not read yet |
| 2 | `▼` | failing: past its condition, short of `drop_after` |
| 3 | `▲` | warn |
| 4 | `●` | ok |
| 5 | `◆` | KPI |

Projects are ordered by their worst monitor, then by name, and monitors
within a project by rank, then by title. While a drop is open the row is
down, whatever the monitor's freshness: its health column names the cause,
and the matter column and `p` take the drop's matter before an unknown one.

### 18.4 The columns

A row shows the glyph, the title, the value, the cadence, the condition with
the sample's `detail`, a strip of 24 cells, the change, the health and the
matter. The cadence is said `10m`, `1h`, `4h`, `8h`, `12h`, `daily` or
`weekly`, and any other as its duration. The matter column says `New`,
`running`, `Recent`, `dismissed`, or `—` where there is none.

### 18.5 The strip covers the window, or the last 24 readings

The strip and the change cover the window, or the monitor's last 24 readings
when those reach further back, so a weekly monitor always shows its last 24
weeks. A boolean draws a strip of health; a level a strip of its past levels,
each cell's height its level and its colour its health; a number a sparkline
of its values; and a series its last 24 points.

### 18.6 The change

A boolean and a level say how often they dropped: `no drops` or `3 drops`. A
number says the difference between the first and the last value of the span,
and a series between the first and the last of the points its strip draws,
with the unit, such as `▾ 2 ms`; a percentage's difference is in points,
`▴ 1.2 pt`. Equal values say `±0`, and a span with no value says `—`. The
change is coloured by `better`: green when it went the better way, amber when
it went the worse way, and plain where nothing says which way is better.

### 18.7 The health column

The health column says, in this order of what applies:

- the cause and how long, for a late or unknown monitor: `parked 3h 50m`,
  `overdue 2h 50m`, `unreadable 1h`, `stale 2d`;
- `not run` or `not read`, with its sentence, for a monitor ephor does not
  read ([§FS-021-monitors.4.5](FS-021-monitors.md#45-a-row-that-trusts-only-descriptions-reads-files-and-runs-no-command), [§FS-021-monitors.4.6](FS-021-monitors.md#46-a-declaration-ephor-cannot-use-is-shown-and-not-read)), and `not read yet` for
  one with no reading yet ([§FS-021-monitors.7.1](FS-021-monitors.md#71-a-value-holds-until-the-next-slot-plus-a-grace));
- `failing 1 of 2` or `recovering 1 of 2`, while a hold is counting;
- `recovered 3h`, while its finished matter is still reported;
- the health and how long it has held: `down 2d 4h`, `warn 3h 20m`,
  `ok 12d`, where a series drawn by `x` labels counts it in points,
  `ok 25 points`;
- `KPI`.

### 18.8 The window

The window is `6h`, `1d`, `7d`, `30d`, `90d` or `1y`, and defaults to `7d`.
`w` cycles it.

### 18.9 History is drawn under today's condition

The page judges the values it draws under the condition that holds today, so
a strip agrees with its threshold after a condition changes. A thinned line
takes the worse of its least and greatest value. What was recorded is never
recomputed: the health in the store, the episodes and their matters stay as
they were judged when they happened.

### 18.10 Keys

- `Enter` opens the row's detail view ([§FS-021-monitors.19](FS-021-monitors.md#19-the-detail-view)).
- `w` cycles the window.
- `r` refreshes the row's project and reads every monitor of it now, out of
  cadence, beneath the screen like every refresh
  ([§FS-001-forge-interface.7](FS-001-forge-interface.md#7-a-fetch-runs-beneath-the-reading-never-in-front-of-it)).
- `o` opens the row's `url`.
- `p` on a row with an open matter processes it in place, as `Enter` does on
  Waiting ([§FS-019-waiting.7.2](FS-019-waiting.md#72-confirming-lays-it-records-it-and-starts-it)).
- `M` and `esc` close the page.

## 19. The detail view

`Enter` on a row opens one monitor, titled
`ephor — monitors › <project> › <title>`. It shows:

- the health, as the health column says it, and since when it has held; the
  value, and the condition in words;
- the sample's `detail`; where it is read `from`, and where that runs; and
  where it is declared, naming each field the site overrides;
- the cadence, as `cadence: every 1d at 06:00 · last read … · next slot … ·
  grace 6h`;
- the hold, as `hold: drop after 1 failing reading · recover after 2 passing
  readings`;
- a chart of the span, with labelled axes, today's `healthy` and `warn`
  drawn as threshold lines, a cursor that walks the readings, and a strip of
  health beneath it;
- the episodes inside the window, each with its start, its end and its
  matter;
- the last readings, each with its time, value, health, and a note where it
  was read out of cadence or failed;
- the matter line: the open matter, its state and its proposal, or why there
  is none.

A series drawn by `x` labels has no time to set a window on, so the window
does not apply to it: its chart draws every point the repository supplied,
and the header says `window n/a: series`.

`←` and `→` move the chart's cursor, and `[` and `]` step to the previous and
the next monitor in the page's order. `w`, `r` and `o` work as on the page,
`p` processes the matter in place, and `esc` comes back to the page.

## 20. Waiting's overview line and the doctor

### 20.1 Waiting's overview line names the monitors

Where any watched project has a status, Waiting's overview line
([§FS-019-waiting.3](FS-019-waiting.md#3-the-header-and-the-overview-line)) gains a segment for them:

```text
overview: 144 matters · 12 changed since you looked   monitors: 1 down · 2 unknown · 1 overdue
```

The segment counts statuses only. `down` and `unknown` are always printed,
and `overdue` only when it is not zero; a monitor the page's header leaves
out is left out here too ([§FS-021-monitors.18.2](FS-021-monitors.md#182-the-header)). So a quiet Waiting screen
still says when the watch itself has degraded. On the screen the footer adds
`M opens them`; `ephor waiting` prints the line without it, and its `--json`
carries the counts as `overview.monitors`, `{down, unknown, overdue}`
([§FS-019-waiting.6](FS-019-waiting.md#6-ephor-waiting-is-the-same-reading)).

### 20.2 The doctor reads the store and names what was overdue

`ephor doctor` reports, as notes that move neither a project's health nor
the exit code ([§FS-010-doctor.5](FS-010-doctor.md#5-the-answer-is-in-the-exit-code)):

- the store's size, its files, and where it is;
- the monitors that were overdue when it began. Its own refresh reads them
  ([§FS-010-doctor.3](FS-010-doctor.md#3-two-passes-the-site-and-ephor-itself)), so after it they would look well, and a watch that
  works only while the doctor runs is the failure worth naming.

## 21. The commands

### 21.1 `ephor monitors` is the page as a table

`ephor monitors [--project P]... [--window W] [--check] [--json]` prints the
page: the header, then each project and its monitors, one row each, with the
columns `value every health since change condition` after the title, and the
footnote `change: over the window, or over the last 24 readings when those
reach further back`. It honours the scope selectors, as every verb that
reads a set of projects does, and refuses a project outside them
([§FS-011-command-line.9](FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)). It reads the store and the cache; it fetches
nothing, runs no monitor and writes nothing. With no monitors it prints the
header and the line the page shows ([§FS-021-monitors.18.2](FS-021-monitors.md#182-the-header)), and a project
named with `--project` that has none says so.

### 21.2 `--check` turns down and unknown into an exit code

`ephor monitors` exits `0`. With `--check`, it exits `4` when any status it
prints is down or unknown, overdue included, so a script or a build step can
fail on a dropped monitor as `status --check` does on unread needs-response
items. A status whose declaration is in error ([§FS-021-monitors.4.6](FS-021-monitors.md#46-a-declaration-ephor-cannot-use-is-shown-and-not-read)) counts
as unknown. A KPI never counts, and neither does a monitor the page's header
leaves out ([§FS-021-monitors.18.2](FS-021-monitors.md#182-the-header)): one not run because its row trusts only
descriptions, which the person chose ([§FS-021-monitors.4.5](FS-021-monitors.md#45-a-row-that-trusts-only-descriptions-reads-files-and-runs-no-command)), and one with no
reading yet whose first grace has not ended ([§FS-021-monitors.7.1](FS-021-monitors.md#71-a-value-holds-until-the-next-slot-plus-a-grace)).

### 21.3 `ephor monitors show` is the detail view

`ephor monitors show <project>/<id> [--window W] [--json]` prints the detail
view: the health and since when it has held, the value, the condition and
where each part of it came from, the cadence and freshness, the readings in
the window, the repository's points, the episodes and the matter. It is
about one monitor, so it refuses the scope selectors. An id the project does
not have is refused, naming the ones it has.

### 21.4 Reading is refresh

There is no separate verb that reads a monitor. `ephor refresh` reads the due
monitors of every project it refreshes, on a plain refresh only
([§FS-021-monitors.17.4](FS-021-monitors.md#174-a-narrowed-refresh-reads-no-monitor)). `ephor refresh [<project>...] --monitors-now` reads
every monitor of every project the refresh refreshes, now, out of cadence,
which is what `r` does on the page ([§REQ-002-parity.2](../requirements/REQ-002-parity.md#2-parity-runs-both-ways)). It cannot be combined
with `--source` or `--only`. Neither form adds a field to what
`refresh --json` prints.

### 21.5 The machine forms

`ephor monitors --json` prints the published `monitors` view, and
`ephor monitors show --json` the `monitor` view (`ephor schema views`). A
monitor in either carries its id, title, kind, value, unit and `better`; its
`health`, `since` when that health began, and what the page shows for it;
its cadence and freshness: `{state, last_read, next_slot, holds_until}`, or
for an unknown monitor `{state: "unknown", cause, since, measured,
holds_until, matter}`; its condition, with the home each part came from and
the site's overrides; and its matter. The views add fields freely, as every
view does ([§REQ-002-parity.4](../requirements/REQ-002-parity.md#4-the-machine-form-is-a-contract-not-a-dump)).

## 22. What this version leaves out

These are named and not done, so that a reader who finds one absent meets a
reason rather than an oversight ([§REQ-001-boundary.1](../requirements/REQ-001-boundary.md#1-the-anatomy)):

- **Monitors ephor builds itself** from the feeds it already has, such as a
  main branch's gate or issues open at a priority. A script answers for them
  meanwhile.
- **A matter for `warn`.** The band is shown and never laid.
- **A refresh timer.** How often refresh runs stays the site's to arrange
  ([§FS-021-monitors.6.5](FS-021-monitors.md#65-cadence-is-a-floor-between-reads-not-a-promise)).
- **Re-reading a file monitor while the page is open.** A read records
  readings and claims its monitors ([§FS-021-monitors.6.9](FS-021-monitors.md#69-a-due-monitor-is-read-once-however-many-refreshes-find-it-due)), and the page
  writes nothing ([§FS-021-monitors.18.1](FS-021-monitors.md#181-m-opens-it-from-any-screen)); a page that read files on its own
  tick would also show what `ephor monitors` cannot. A file is read when a
  refresh reads it, like every monitor.
- **The monitor's workflow for its unknown matter.** It is proposed for the
  drop alone ([§FS-021-monitors.16.2](FS-021-monitors.md#162-without-one-triage)).
- **A condition on a fraction**, such as "all of them". A condition reads the
  value.
