# FS-002-release: ephor releases from a tag, with a changelog entry per change

Versions are semver, and a version exists exactly when a `vX.Y.Z` tag does. The
version in `Cargo.toml` and the tag agree or the release refuses to run. Between
releases the manifest carries the *next* version with a `-dev` suffix, which is
not a version in that sense and is never publishable.

## 1. Changelog

[docs/changelog.md](../changelog.md) holds the most recent release inline;
older releases move one-per-file under `docs/changelog/` with a one-line
pointer, so the common question — what changed lately — is one file deep.
Sections per release are the Keep-a-Changelog set, then `Note`.

What has merged and not yet shipped is not in that file. A section every pull
request appended to put any two open pull requests on the same lines, so
whichever landed second conflicted, and resolving that by hand is exactly the
routine move [§GOAL-004-handover](../goals.md#goal-004-handover-routine-moves-leave-the-persons-hands) wants out of a person's hands. Each pending change is
instead one file under `docs/changelog/unreleased/`, so two pull requests that
each record a change add two files and merge or rebase in either order without
a line in common. `## Unreleased` stays in `docs/changelog.md` as a fixed
pointer to that directory and holds no bullet: the links that name its anchor
keep working, and an ordinary change never edits the file.

Every pull request adds at least one entry of its own. The number is not the
contributor's to know: an entry may end with its own `(PR #12)`, with a
`(PR #TBD)` placeholder, or with neither, and a number that *is* written must
be that pull request's own — but an entry with no number is complete, because
the release stamps it ([§FS-002-release.2.2](FS-002-release.md#22-the-release-stamps-only-the-trailing-number-and-stamping-never-fails-it)). A field renamed or removed from
the machine form is still noted here, in an entry like any other
([§REQ-002-parity.4](../requirements/REQ-002-parity.md#4-the-machine-form-is-a-contract-not-a-dump)).

### 1.1 One file per pending change, in grund's format

The format is not ephor's own. `docs/changelog/unreleased/README.md` copies
everything above part two of grund's README byte for byte, as it stands at
grund commit `16fa32b4c087583254f757e52480fda966da0621`, so a contributor who
has written an entry for one of these repositories has written one for all of
them. Its part two is how ephor uses the format:

- An entry is `<slug>.<category>.md`. The slug is lowercase letters, digits
  and hyphens, unique across every category in the directory, and stays with
  the entry for life, because it is what the release orders and attributes by
  ([§FS-002-release.2.1](FS-002-release.md#21-an-entry-is-ordered-and-attributed-by-where-it-landed)).
- The category is required and is one of `added`, `changed`, `deprecated`,
  `removed`, `fixed`, `security` or `note`, released in that order under the
  same word capitalized; a category with no entry is omitted. `note` is
  ephor's: this changelog has always carried a `### Note` section, and a
  format without it would drop history.
- A file holds exactly one bullet, as published: its first line opens with
  `- `, and every later line is indented two spaces. A blank line followed by
  more indented text continues the bullet with another paragraph, as Markdown
  reads it, because bullets this changelog already holds run to several
  paragraphs and an entry moved from it must say what it said.
- Links are relative to the entry's own file, and the release rewrites them
  for wherever the entry lands.
- `README.md` describes the format and is never an entry.

### 1.2 The switch-over moves what was pending, and loses nothing

The change that introduces the directory empties `## Unreleased` into it once.
It first runs the old `stamp` over the section, so every number the old
release could still recover is written while each bullet's history is still
the history of its lines. It then copies every bullet pending at the final
switch-over base — the base the change lands on, not the one it was planned
against — into an entry of its own, keeping its category, its complete text
across every line and paragraph, every number it carries, and the target of
every link, rewritten to resolve from the entry's directory. Legacy slugs are
numbered so that each category keeps the order the section had.

A copy is not a new change, and it is recognized without anything a
contributor writes: an entry is a copy when its category and its complete
bullet match a bullet under that base's `## Unreleased`, allowing only for the
moved link locations and the numbers the old stamper writes. A copy neither
satisfies the gate's demand for an added entry nor is held to its number check
([§FS-002-release.6](FS-002-release.md#6-the-changelog-gate-runs-before-the-pull-request-exists)), so a moved `(PR #104)` is not a foreign number; the
switch-over's own entry is ordinary and checked like any other. There is no
exemption a contributor can claim. A bullet still unnumbered after the old
stamp is released once without a number, and never with the switch-over's own
([§FS-002-release.2.2](FS-002-release.md#22-the-release-stamps-only-the-trailing-number-and-stamping-never-fails-it)).

## 2. Cutting a release

Collecting the pending entries into a numbered release, bumping the manifest,
and tagging is done by workflow, not by hand: a patch release on a schedule
when main has shipped observable changes and its CI is green, and a minor
release on demand. Each first runs the whole release on a candidate branch with
publishing disabled, and only fast-forwards main if that dry run passed; that
candidate run is the release's dry run, and there is no other. The release
script keeps its three commands and their arguments — `stamp`,
`prepare VERSION --date DATE` and `notes VERSION --output PATH` — and the
workflows that call them check out main's full history, which is what ordering
and attribution read.

### 2.1 An entry is ordered and attributed by where it landed

An entry's lifetime begins at the commit on main's first-parent history that
added its slug, and ends when a release consumes it. Within a category entries
are released oldest-landed first, with the file name breaking ties, and an
entry no commit has added yet follows every landed one. Editing an entry, or
moving it to another category, keeps where it landed and whose pull request it
is: the slug carries the lifetime, not the file name and not the lines, which is
why a change of slug is refused
([§FS-002-release.6](FS-002-release.md#6-the-changelog-gate-runs-before-the-pull-request-exists)). A slug used again after a release starts a new
lifetime. Where the history is not there to read — a shallow clone — the release
says so in a warning, orders by file name alone, and attributes nothing it
could not see.

### 2.2 The release stamps only the trailing number, and stamping never fails it

The release stamps the numbers the contributors did not write, from the commit
that began the entry's lifetime. Where that commit resolves to exactly one pull
request, a trailing `(PR #TBD)` is replaced with it, or `(PR #N)` is appended to
the entry's last line; a trailing number already written is left alone.
Nothing but the end is touched, so an entry whose prose mentions `(PR #TBD)` or
another pull request keeps it. Anything else — no pull request, more than one, a
forge that refuses or cannot be reached, an entry that cannot be read or
written — leaves the entry exactly as written, with a warning in the workflow
log that names the entry and carries the failing tool's own words. Stamping
never fails a release, so a released section may keep an entry that carries no
number at all.

A copy moved by the switch-over
([§FS-002-release.1.2](FS-002-release.md#12-the-switch-over-moves-what-was-pending-and-loses-nothing)) began its lifetime in the switch-over, whose
pull request is not where it came from. The release recognizes it by that
addition and the bullet in the addition's parent, even after the copy has been
edited, and never stamps it with the switch-over's number or an editor's: it
keeps the number it was moved with, or is released once without one, with a
warning.

### 2.3 Preparing a release consumes the entries, and refuses before it loses one

`prepare` reads every entry before it writes anything. It writes the new
numbered release inline beneath the `## Unreleased` pointer — one section per
category, each entry's bullet as published with its links rebased for `docs/` —
rotates the previous release whole into `docs/changelog/<version>.md` with its
links rebased again and a one-line pointer under the older releases, and
deletes the entry files it released, keeping `README.md` and the pointer. What
`notes` writes for the GitHub release is still that inline section
([§FS-002-release.3](FS-002-release.md#3-artifacts)). A link keeps its target, fragment included, wherever the
entry is read: in its own file, inline, and in an archive.

It refuses, with nothing consumed and nothing written, when an entry is
malformed, when `## Unreleased` holds a bullet — written the old way, and lost
if the release went on —, when the archive it would write already exists, or
when there is no entry to release; `README.md` alone is no entry.

## 3. Artifacts

A release publishes the crate and a self-checked binary per supported target,
each built profile-guided, archived with its `sha256`, and attached to a GitHub
release whose notes are the changelog section for that version. Re-running a
partially-failed release skips what already exists rather than failing.

**Self-checked means the binary did its job, not that it started.** What every
artifact is held to is `doctor`'s self pass ([§FS-010-doctor.3](FS-010-doctor.md#3-two-passes-the-site-and-ephor-itself)): it builds a
project of its own and walks the seams against it, so a binary that links,
prints its version and cannot refresh a source is caught here rather than by
the first person to install it. A check that only asks for `--version` tests
the argument parser.

**And it is what the profile is trained on.** A profile-guided build is only
as good as the workload it profiled, so the training run is the same self
pass rather than a list of commands assembled for the occasion — one workload,
exercising the paths a reader actually waits on. It must also be hermetic: a
training run that reads the registry of whoever is building reads a private
site on one machine and finds nothing on a build runner
([§FS-001-forge-interface.5](FS-001-forge-interface.md#5-no-site-specific-data-in-the-repository)), and a profile gathered from commands that all
exited early is no profile at all.

## 4. Publication is gated on carrying nothing site-specific

No artifact is published while the tree still violates
[§FS-001-forge-interface.5](FS-001-forge-interface.md#5-no-site-specific-data-in-the-repository) or the
literal confinement of [§REQ-001-boundary.5](../requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter). The checks are mechanical and run
before anything is uploaded.

## 5. Between releases, main carries a dev version

A release leaves main holding the version it just published, so every build from
main until the next release reports the tag it is already ahead of. Nothing on
the machine can then tell a binary built from main from the released one, and a
fix that is merged but not installed looks identical to one that is installed.

So the release advances main as its last act: after publishing `X.Y.Z` it
commits `X.Y.(Z+1)-dev`. The suffix is what makes `--version` say which side of
the tag a build came from. It is deliberately not a releasable version — the
release path sets the clean version on its candidate branch and verifies it
there, so a `-dev` manifest can never be published.

## 6. The changelog gate runs before the pull request exists

The entry is checked twice by one rule: in the pre-push hook, against the base
branch, and in CI, against the pull request's own base commit. Both ask the same
question — does this change add an entry whose slug the base did not have, are
the entries it adds or changes well-formed
([§FS-002-release.1.1](FS-002-release.md#11-one-file-per-pending-change-in-grunds-format)), and is any pull request number it newly writes this pull
request's own? A file that was already there does not count however it changes:
not an edit, not a change of category, and not a change of slug, which is
refused outright because the slug carries the entry's lifetime
([§FS-002-release.2.1](FS-002-release.md#21-an-entry-is-ordered-and-attributed-by-where-it-landed)). Neither does `README.md`, a bullet added under
`## Unreleased`, or an entry main gained after the branch left it. A number is
foreign when this change writes it — as `PR #N`, as `pull request #N`, or as a
link to a pull request — and it is not this pull request's; one an entry
already carried at the base is not this change's to fix. Copies moved by the
switch-over are judged as
[§FS-002-release.1.2](FS-002-release.md#12-the-switch-over-moves-what-was-pending-and-loses-nothing) says. Neither moment asks for a number that does not exist
yet. It is not advisory: the push is refused, and the refusal names the
directory, a file to add there, and the single hook to skip rather than all of
them.

Where no base can be resolved — a fork clone that has never fetched this
repository — the hook falls back to requiring a well-formed entry in the
directory, and says that it could not check the entry was added and which bases
it tried. The local gate never refuses a push that CI would have passed.

## 7. A pinned checker and the blocks it generates move together

A checker this repository pins is a concrete published version — a tag that
exists and an artifact installable by exact number — never a development build
and never whatever is newest. Every checked-in managed entrypoint block is then
that version's own output: `CLAUDE.md` and `.claude/CLAUDE.md` carry what the
pinned generator writes, and every byte outside the block's markers survives the
regeneration untouched.

The pin is not one value. CI's install step, the cache identity it keys on, and
the comment that explains the compatibility name one pairing — the release and
the block version it speaks — and they move in a single change. The contributor
setup and the PATH-based local gates name that same release, so the hook a
person runs before pushing reaches the verdict the job will. A tree where they
disagree is green only because CI is the one holding the older checker, and the
local gate stops handing anything over ([§GOAL-004-handover](../goals.md#goal-004-handover-routine-moves-leave-the-persons-hands)): the person is back
to reading two answers and deciding which one counted.

Compatibility is promised for the pinned release and for no other. Moving the
pin is a deliberate change that regenerates the blocks in the same commit; a
contributor on an older checker is told the block is unsupported and upgrades,
and nothing is claimed about a version this repository has not pinned.
