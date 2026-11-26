//! The guide's `monitors` topic: the monitor protocol in ephor's own words,
//! with no runtime named, for the reader asked to add a number to a
//! repository's watch (§FS-020-guide.6).
//!
//! The protocol is ephor's and reads nothing of the site's, so the text is
//! the same on every machine. The worked example is held to the protocol it
//! explains: `monitors_tests.rs` takes the script, the manifest and the answer
//! out of the text itself, holds the manifest to the published manifest
//! schema, runs the script, and holds what it writes to the published answer
//! schema; E2E-086 reads the example's monitors with ephor itself.

use crate::work::runtime::guide::indent;

/// The contract, before the example: one screen of 80 columns and 48 lines
/// (§FS-020-guide.1), covering what §FS-020-guide.6 lists.
const CONTRACT: &str = r#"MONITORS — how a repository makes its numbers watchable

A monitor is one quantity of a project, read at its own cadence, kept and
drawn. One something judges is a status, ok, warn, down or unknown, whose
drop is a matter in Waiting; one nothing judges is a KPI, never a matter.
Declare them in ephor.json under `monitors`. Only `id` is required: lowercase
letters, digits and `-`, starting with a letter or digit.

Kinds, their values, and the condition each has unless it says otherwise:
  boolean  true or false; healthy when true
  number   a number, with `unit` (%, ms, s, bytes, count, $ or any text),
           `better` (higher or lower) and `of` for a fraction, 12/15; none
  level    one of `levels`, best first; healthy at the first level
  series   `points`, all {t, v} by time or all {x, v} by label; the last v; none
`healthy` is the condition: {"is": true}; {"min": 80}, {"max": 5} or both,
inclusive; {"in": ["clean"]}; {"at_most": "degraded"}. `warn` is one more a
value must meet to show ok, never a matter: `"healthy": {"max": 5}, "warn":
{"max": 0}` is ok at 0, warn from 1 to 5, down above 5. What judges is the
first there is of the site's `healthy`, the manifest's, one a sample brings,
the sample's own `ok`, the kind's; `"healthy": null` makes any monitor a KPI.

`from` is where the value comes from. {"command": "scripts/kpis.sh"} is run
by a shell in the checkout of the default branch, once per read however many
monitors name it, with the project's EPHOR_* variables, those about an item
empty; EPHOR_MONITORS, the ids this run reads, comma-separated; and
EPHOR_ANSWER, the file to write. Exit 0 is a read; 75 is parked, and the
last value stands; any other, or 30 s gone (`timeout` sets another), is
unreadable. Never run `ephor refresh` in it: the refresh running it has
claimed its monitors, so the inner one would read none of them. A file,
{"file": "target/kpi/coverage.json"}, is an answer a build step writes, read
and never run: a regular file of 16 MiB at most, its path relative, no `..`.

The answer is {"v": 1, "monitors": [...]}, one sample per monitor: `id`,
`value`, and where known `of`, `at` (when measured, with its offset), `ok`,
`detail` (one line), `url`, `points`. A sample that does not fit its shape
or its kind makes that monitor unreadable, and no other. NaN and Infinity are
not JSON, and refuse the whole answer: with no number, leave `value` out.

`every` is a whole number and m, h, d or w: 10m (the default), 4h, 1d, 1w.
`at` pins the slots to the local clock: "06:00" with an `every` that divides
a day or is whole days (1d unless said), "Mon 06:00" with whole weeks (1w).
Cadence is a floor, not a promise: a refresh reads only what is due, so with
a refresh about every 30 minutes, a 10m monitor is read about as often.

Check it (the shapes are `ephor schema manifest` and `ephor schema answer`):
  ephor validate --manifest ephor.json  holds each declaration to its shape
  ephor refresh <project>               reads what is due; --monitors-now, all
  ephor monitors show <project>/<id>    the value, its health, what is wrong
"#;

/// The example's command, `scripts/kpis.sh`: one run answers all three
/// monitors (§FS-021-monitors.5.1), and a value it cannot give is left out
/// rather than written as something JSON is not (§FS-021-monitors.8.2).
const SCRIPT: &str = r###"#!/bin/sh
# Answers for the monitors ephor.json declares, in the checkout ephor runs it
# in. Each is cheap, so it answers all three; ephor keeps the samples of the
# ones $EPHOR_MONITORS names and drops the rest.
version=$(cat VERSION 2>/dev/null | tr -cd '0-9A-Za-z.-')
if [ -n "$version" ] && grep -qF "## $version" CHANGELOG.md 2>/dev/null; then
  releasable='"value": true'
else
  releasable='"value": false, "detail": "no CHANGELOG.md entry for VERSION"'
fi
todos=$(find src -type f -exec cat {} + 2>/dev/null | grep -c TODO)
if [ -f target/nightly.status ]; then
  nightly=$(printf '"value": "%s"' "$(tr -cd 'a-z' < target/nightly.status)")
else
  # No value to give: leave "value" out, and never write NaN.
  nightly='"detail": "the nightly job left no target/nightly.status"'
fi
cat > "$EPHOR_ANSWER" <<EOF
{"v": 1, "monitors": [
  {"id": "releasable", $releasable},
  {"id": "todo-markers", "value": $todos},
  {"id": "nightly", $nightly}
]}
EOF
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

/// What the example's monitors mean, said once beside them.
const READING: &str = "\
releasable is judged by its kind's default, healthy when true. todo-markers
is ok up to 10, warn from 11 to 20 and down above. nightly is ok when passed,
warn when flaky and down when failed. The script runs whenever one of them is
due, so as often as releasable, every 10m by default; only the due ones keep
their sample, so todo-markers records a reading every 4h, and nightly one a
day, at the first refresh after 06:00.
";

/// What the script writes in a checkout the sentence before it describes
/// (§FS-021-monitors.8.1).
const ANSWER: &str = r#"{"v": 1, "monitors": [
  {"id": "releasable", "value": true},
  {"id": "todo-markers", "value": 3},
  {"id": "nightly", "value": "passed"}
]}
"#;

/// The `monitors` topic (§FS-020-guide.6): the contract, then one complete
/// worked example after it (§FS-020-guide.1).
pub(super) fn text() -> String {
    format!(
        "{CONTRACT}\n\
         Worked example. scripts/kpis.sh, executable (chmod +x), answers three\n\
         monitors of three kinds:\n{}\n\
         ephor.json, at the root of the checkout, declares them:\n{}\n\
         {READING}\n\
         With 1.4.0 in VERSION, a \"## 1.4.0\" heading in CHANGELOG.md, three\n\
         lines holding TODO under src/ and passed in target/nightly.status, the\n\
         script writes:\n{}\n\
         Where target/nightly.status is missing, the nightly sample carries no\n\
         value, and that read of nightly alone is unreadable.\n",
        indent(SCRIPT),
        indent(MANIFEST),
        indent(ANSWER),
    )
}

#[cfg(test)]
#[path = "monitors_tests.rs"]
mod tests;
