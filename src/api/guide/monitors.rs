//! The guide's `monitors` topic: the monitor protocol in ephor's own words,
//! with no runtime named, for the reader asked to add a number to a
//! repository's watch (§FS-020-guide.6).
//!
//! The protocol is ephor's and reads nothing of the site's, so the text is
//! the same on every machine. The worked example is held to the protocol it
//! explains: `monitors_tests.rs` takes the script, the manifest and the answer
//! out of the text itself, holds the manifest to the published manifest
//! schema, runs the script, as printed and as copied, and holds what it
//! writes to the published answer schema, and holds what the closing
//! paragraph says validation leaves to a refresh; E2E-086 reads the example's
//! monitors with ephor itself, a stale night of the nightly job included.

use crate::work::runtime::guide::indent;

/// The contract, before the example: one screen of 80 columns and 48 lines
/// (§FS-020-guide.1), covering what §FS-020-guide.6 lists; the commands that
/// check the result close the example, as the `gate` topic's do.
const CONTRACT: &str = r#"MONITORS — how a repository makes its numbers watchable

A monitor is one quantity of a project, read at its own cadence and kept. One
something judges is a status, ok, warn, down or unknown, whose drop is a matter
in Waiting; one nothing judges is a KPI. Declare them in ephor.json `monitors`;
only `id` is required. A sample for an id nothing declares is a monitor too.

Kinds, their values, and the condition each has unless it says otherwise:
  boolean  true or false; healthy when true
  number   a number, `unit` (%, ms, count, any), `better` (higher, lower); none
  level    one of `levels`, best first; healthy at the first level
  series   `points`, all {t, v} with t a time as `at` is, or all {x, v}; none
`of` makes a number a fraction, 12/15, and a series' value is its last v; a
condition is on the value, never the fraction. `healthy` is the condition:
{"is": true} for a boolean; {"min": 80}, {"max": 5} or both, inclusive, for a
number or series; {"in": ["clean"]} or {"at_most": "degraded"} for a level.
`warn`, stricter and needing a `healthy` (a kind's default counts), must also
hold to show ok, never a matter: `"healthy": {"max": 5}, "warn": {"max": 0}` is
ok at 0, warn 1 to 5, down above. What judges is the first of the site's
`healthy` (the person's own), the manifest's, one a sample brings, the sample's
`ok` (false is down), the kind's; `"healthy": null` makes any monitor a KPI.

`from` is where the value comes from; with none, the answer that brings its id.
{"command": "scripts/kpis.sh"} runs in a shell in the default branch's checkout,
once per read however many monitors name it, with the project's EPHOR_*
variables, those about an item empty; EPHOR_MONITORS, the ids this run reads,
comma-separated; and EPHOR_ANSWER, the file to write. Exit 0 is a read; 75 is
parked, and the last value stands; any other, or 30 s gone (`timeout` beside
`from`, whole seconds, sets another), is unreadable. Never run `ephor refresh`
in it: its monitors are claimed, so it would read none. A file, {"file":
"target/kpi/coverage.json"}, is an answer a build step writes, only read: a
regular file of 16 MiB at most, its path relative, never climbing out by `..`.

The answer is {"v": 1, "monitors": [...]}, one sample per monitor: `id`,
`value`, and where known `of`, `at` (when measured, with its offset), `ok`,
`detail` (one line), `url`, `points`, and any of `title`, `kind`, `unit`,
`better`, `levels`, `healthy`, `warn`, `description`. A sample missing or not
fitting its shape or kind makes that monitor alone unreadable. NaN and Infinity
are not JSON, and refuse the whole answer: with no number, leave `value` out. A
read is stale, not a reading, when its sample was measured over an interval
before it or no later than the last, its series' last point is the one held, or
its file is unchanged: what a job leaves needs the job's `at` and cadence.

`every` is a whole number and m, h, d or w: 10m (the default), 4h, 1d, 1w. `at`
pins the slots to the local clock: "06:00" with an `every` that divides a day or
is whole days (1d unless said), "Mon 06:00" with whole weeks (1w). Cadence is a
floor, not a promise: a refresh reads what is due, and how often one runs is the
site's to arrange, so under one every 30 minutes a 10m monitor is read every 30.
"#;

/// The example's command, `scripts/kpis.sh`: one run answers all three
/// monitors (§FS-021-monitors.5.1), a value it cannot give is left out rather
/// than written as something JSON is not (§FS-021-monitors.8.2), and what the
/// nightly job left behind carries the job's own time (§FS-021-monitors.7.1).
/// It writes its answer with `printf` rather than a here-document, so it runs
/// as printed, two-space indent and all.
const SCRIPT: &str = r###"#!/bin/sh
# Answers the monitors ephor.json declares, in the checkout ephor runs it in.
# Each is cheap, so it answers all three on every run; only the due ones,
# which $EPHOR_MONITORS names, record a reading.
version=$(cat VERSION 2>/dev/null | tr -cd '0-9A-Za-z.-')
if [ -n "$version" ] && grep -qF "## $version" CHANGELOG.md 2>/dev/null; then
  releasable='"value": true'
else
  releasable='"value": false, "detail": "no CHANGELOG.md entry for VERSION"'
fi
todos=$(find src -type f -exec cat {} + 2>/dev/null | grep -c TODO)
if [ -f target/nightly.status ]; then
  # The nightly job writes its result and when it ran, so a night it does
  # not run is a stale read rather than an old result read as new.
  read -r result ran < target/nightly.status
  nightly=$(printf '"value": "%s", "at": "%s"' "$result" "$ran")
else
  # No value to give: leave "value" out, and never write NaN.
  nightly='"detail": "the nightly job left no target/nightly.status"'
fi
{
  printf '{"v": 1, "monitors": [\n'
  printf '  {"id": "releasable", %s},\n' "$releasable"
  printf '  {"id": "todo-markers", "value": %s},\n' "$todos"
  printf '  {"id": "nightly", %s}\n' "$nightly"
  printf ']}\n'
} > "$EPHOR_ANSWER"
"###;

/// The example's manifest: a boolean, a number and a level, each read from
/// the one command at its own cadence (§FS-021-monitors.4.1,
/// §FS-021-monitors.6.6).
const MANIFEST: &str = r#"{
  "v": 1,
  "monitors": [
    { "id": "releasable", "title": "Releasable", "kind": "boolean",
      "description": "CHANGELOG.md has an entry for the version in VERSION",
      "from": { "command": "scripts/kpis.sh" } },
    { "id": "todo-markers", "title": "TODO markers", "kind": "number",
      "unit": "count", "better": "lower",
      "healthy": { "max": 20 }, "warn": { "max": 10 },
      "from": { "command": "scripts/kpis.sh" }, "every": "4h" },
    { "id": "nightly", "title": "Nightly build", "kind": "level",
      "levels": ["passed", "flaky", "failed"],
      "healthy": { "at_most": "flaky" }, "warn": { "in": ["passed"] },
      "from": { "command": "scripts/kpis.sh" }, "every": "1d", "at": "06:00" }
  ]
}
"#;

/// What the example's monitors mean, said once beside them: their bands,
/// their cadences (§FS-021-monitors.6.6), and why nightly carries an `at`
/// (§FS-021-monitors.7.1).
const READING: &str = "\
releasable is judged by its kind's default, healthy when true. todo-markers
is ok up to 10, warn from 11 to 20 and down above. nightly is ok when passed,
warn when flaky and down when failed. The script runs whenever one of them is
due, so as often as releasable, every 10m by default; only the due ones keep
their sample, so todo-markers records a reading every 4h, and nightly one a
day, at the first refresh after 06:00. releasable and todo-markers are
measured as the script runs, and carry no `at`. nightly carries the time the
job ran, so on a night the job does not run, the 06:00 read is stale, and
nightly turns unknown when its 6h grace ends rather than show the last result
as new.
";

/// What the script writes in a checkout the sentence before it describes
/// (§FS-021-monitors.8.1).
const ANSWER: &str = r#"{"v": 1, "monitors": [
  {"id": "releasable", "value": true},
  {"id": "todo-markers", "value": 3},
  {"id": "nightly", "value": "passed", "at": "2026-11-26T05:40:00+01:00"}
]}
"#;

/// The commands that check the result, and what each one checks
/// (§FS-020-guide.6): the manifest's shape, then the rules a shape cannot
/// see, which only the resolved monitor can judge (§FS-021-monitors.4.6,
/// §FS-021-monitors.3.5).
const CHECK: &str = r#"Check it, in the checkout (the shapes are `ephor schema manifest` and
`ephor schema answer`; <project> is the name `ephor list` gives the project):
  ephor validate --manifest ephor.json  each declaration's shape, and no more
  ephor refresh <project>               reads what is due; --monitors-now, all
  ephor monitors show <project>/<id>    its value and health, why a read failed
Validate holds each declaration to its shape alone. A rule a shape cannot see,
a condition that does not fit its kind, a `warn` laxer than its `healthy`, an
`at` its `every` does not fit, is judged on the monitor as every home declares
it, and a monitor that breaks one is not read: each refresh says so in a
`note:` line, and `ephor monitors` shows it as not read, with why. A `warn`
with no `healthy` declared or its kind's waits for its first sample to bring
one, and a read whose sample brings none is unreadable, saying so. A key ephor
does not know, a misspelt `heathy` or a `timeout` inside `from`, is noted the
same way and set aside, never refused.
"#;

/// The `monitors` topic (§FS-020-guide.6): the contract, then one complete
/// worked example after it, which ends with the commands that check it
/// (§FS-020-guide.1).
pub(super) fn text() -> String {
    format!(
        "{CONTRACT}\n\
         Worked example. Each file below is indented by two spaces; copy it\n\
         without them. scripts/kpis.sh, executable (chmod +x), answers three\n\
         monitors of three kinds:\n{}\n\
         ephor.json, at the root of the checkout, declares them:\n{}\n\
         {READING}\n\
         With 1.4.0 in VERSION, a \"## 1.4.0\" heading in CHANGELOG.md, three\n\
         lines holding TODO under src/ and \"passed 2026-11-26T05:40:00+01:00\" in\n\
         target/nightly.status, the script writes:\n{}\n\
         Where target/nightly.status is missing, the nightly sample carries no\n\
         value, and that read of nightly alone is unreadable.\n\n\
         {CHECK}",
        indent(SCRIPT),
        indent(MANIFEST),
        indent(ANSWER),
    )
}

#[cfg(test)]
#[path = "monitors_tests.rs"]
mod tests;
