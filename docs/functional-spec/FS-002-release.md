# FS-002-release: ephor releases from a tag, and the release writes its changelog from the pull requests it ships

Versions are semver, and a version exists exactly when a `vX.Y.Z` tag does. The
version in `Cargo.toml` and the tag agree or the release refuses to run. Between
releases the manifest carries the *next* version with a `-dev` suffix, which is
not a version in that sense and is never publishable.

Nobody writes anything before a release, by hand or by agent. A change already
says what it did, in the title of the pull request that landed it, and the
pull requests merged since the last tag are the list that misses nothing. So the
release reads that list from the forge and writes it down itself: what was
left for a person — a write-up to start, a section to word, a number to get
right — is a mechanical move an algorithm finishes, which is what
[§GOAL-004-handover](../goals.md#goal-004-handover-routine-moves-leave-the-persons-hands) asks to be handed over rather than performed.

## 1. Changelog

[docs/changelog.md](../changelog.md) holds its conventions and then the most
recent release inline; older releases move one-per-file under `docs/changelog/`
with a one-line pointer, so the common question — what changed lately — is one
file deep. There is no pending section and no store of pending entries: between
releases the file does not change, and no change writes to it.

A release's notes are a flat list, one line per pull request it ships: the pull
request's title, linked to the pull request, then its number,
`- [<title>](<url>) (PR #N)`, newest first. Which pull requests those are, and
how each line is written, is [§FS-002-release.1.3](FS-002-release.md#13-the-release-lists-the-pull-requests-merged-since-the-previous-tag-and-every-one-of-them). When a published schema lost
or changed a field since the previous release, a block of compatibility notices
follows the list ([§FS-002-release.1.4](FS-002-release.md#14-compatibility-notices-from-the-previous-tag-onwards)); that block is how a field renamed or
removed from the machine form still reaches the release that ships it
([§REQ-002-parity.4](../requirements/REQ-002-parity.md#4-the-machine-form-is-a-contract-not-a-dump)).

### 1.3 The release lists the pull requests merged since the previous tag, and every one of them

**The range is history, never time.** Its upper bound is the committed `HEAD`
the release runs on, which must lie on the first-parent history of the fetched
default branch; its lower bound is the nearest `vX.Y.Z` tag on that same
first-parent history, excluded, or — before the first release, when there is
none — the root commit, included. A tag on a commit off that history, or one
that is not `vX.Y.Z`, is no bound. The version asked for must be greater than
the lower bound's tag and than the release already inline.

**A pull request is in the range by where it landed.** It counts when it was
merged into the default branch and the commit it landed as — `merge_commit_sha`,
which is the merge commit for a merge, the squashed commit for a squash, and the
last replayed commit for a rebase — is one of the range's first-parent commits.
Nothing is selected by a timestamp: a pull request merged after the commit the
release runs on, or merged into any other branch, is never listed, whatever
its merge time says.

**The listing is complete or there is no release.** Every page of the merged
pull requests is read, and their count is checked against the forge's own total
for merged pull requests into the default branch; every page of each listed
pull request's changed files is read, and their count is checked against the
pull request's `changed_files`. A mismatch, a page that fails, or a pull request
whose `changed_files` is past what the forge will list (3,000 files) refuses
the release ([§FS-002-release.2.3](FS-002-release.md#23-preparing-a-release-reads-everything-before-it-writes-and-refuses-rather-than-guess)): a listing that stopped early would silently
drop an older merge, and a truncated file list would guess at whether a pull
request was substantive.

**A pull request qualifies by its paths, judged as the schedule judges them.**
One predicate decides what is substantive, and both this selection and the
schedule's gate ([§FS-002-release.2](FS-002-release.md#2-cutting-a-release)) call it, so the two cannot disagree about a
path. A path is not substantive when it is under `docs/`, `.github/`, `.agents/`,
`.agent-grounds/` or `tests/e2e/`, when it ends in `.md`, or when it is the root
`LICENSE`, `AGENTS.md`, `CLAUDE.md` or `grund.toml`; every other path is. A
renamed file is judged by where it went. A pull request with any substantive
path is listed; one with none is not.

**Order and rendering are fixed.** Pull requests are listed newest first by
merge time, and those merged at the same second by the higher number first.
Each line is `- [<title>](<url>) (PR #N)`. The title is written as literal link
text: line breaks and runs of whitespace become one space, the ends are
trimmed, and each of `\`, `` ` ``, `*`, `_`, `[`, `]`, `<` and `>` is
escaped with a backslash, so a title reads as its author typed it and cannot
close the link early. The URL is the pull request's own `html_url`, and it
must be `https://github.com/<owner>/<repo>/pull/<N>` for this repository and
that number, or the release refuses rather than link elsewhere.

The repository is the one `GITHUB_REPOSITORY` names, as it is set in the
workflows, and otherwise the GitHub repository `origin` points at.

### 1.4 Compatibility notices, from the previous tag onwards

A release that follows a tag compares every published schema,
`assets/*.schema.json`, from that tag through each first-parent commit to
`HEAD`, and writes what it finds as a `### Compatibility notices` block after the
list, one bullet per notice, each naming the schema's file. The first release
writes none: with no tag nothing was released, so nothing it ships can break a
caller.

- A property or shape that was removed is noticed by its JSON Pointer.
- A rename is a removal and an addition. The removal is noticed; the release
  does not guess which two belong together.
- A changed constraint, reference, required set or version marker is noticed
  with its JSON Pointer, the old and the new value, and a link to the commit
  that changed it.
- Only what is provably compatible goes unnoticed: a newly added optional
  property, and an edit to prose alone — `title`, `description`, `$comment`,
  `examples`. Any other structural change is noticed as potentially
  incompatible, never dropped.
- A schema's version marker is the trailing `/v<N>` of its `$id`; a schema
  without one is labelled unversioned in each of its notices.

A release with nothing to notice writes no block.

## 2. Cutting a release

Writing the notes into a numbered release, bumping the manifest, and tagging is
done by workflow, not by hand: a patch release on a schedule when main has
shipped observable changes and its CI is green, and a minor release on demand.
The schedule's gate on observable changes is the substantive-path predicate of
[§FS-002-release.1.3](FS-002-release.md#13-the-release-lists-the-pull-requests-merged-since-the-previous-tag-and-every-one-of-them) over the net diff since the last tag, so a range whose
changes were reverted is a quiet skip, as is a range with no new commits. Each
release first runs whole on a candidate branch with publishing disabled, and
only fast-forwards main if that dry run passed; that candidate run is the
release's dry run, and there is no other.

The release script has two commands, `prepare VERSION --date DATE` and
`notes VERSION --output PATH`, with `--changelog PATH` before either. There is
nothing to stamp, count or hold on: no release waits for a section to be
written, so a scheduled release runs whenever its gates pass. The workflows that
call it check out main's full history, which the range is read from, and give it
the forge credentials it reads the pull requests with. The first `vX.Y.Z` tag
is still bootstrapped by hand before either workflow can bump from it; `prepare`
itself supports a first release.

### 2.3 Preparing a release reads everything before it writes, and refuses rather than guess

`prepare` fetches and validates everything it will write — the range, every
pull request and every file listing, the schema comparisons — before its first
write. It then writes the new numbered release, `## 2. [VERSION] — DATE`, into
the slot between `## 1. Conventions` and `## 3. Older releases`. Where a
release is already there, it rotates that release whole into
`docs/changelog/<version>.md`, with its links rebased for the archive's
directory, and adds a one-line pointer to it under the older releases, which
replaces the `_None yet._` placeholder; where the slot is empty, as before the
first release, it archives nothing. What `notes` writes for the GitHub release
is still that inline section, notices included ([§FS-002-release.3](FS-002-release.md#3-artifacts)).

It refuses — exiting non-zero, naming the reason, and leaving every release file
byte for byte as it found it — when:

- `gh` is missing, a forge response fails, or a listing is incomplete;
- the history is shallow, or `HEAD` is not on the default branch's first-parent
  history;
- the version or the date is malformed, or the version is not greater than the
  previous tag and the inline release;
- `docs/changelog.md` does not have the layout above, or the archive it would
  write already exists;
- no pull request qualifies — the range is empty, holds only pull requests with
  no substantive path, or holds only commits pushed without one.

Without the forge there are no notes, so a release offline is refused rather
than published with an empty or partial list. That is a different case from the
schedule's quiet skip, which `prepare` never reaches.

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

## 6. No change is gated on the changelog

Neither a hook nor CI asks a change for a changelog entry, and nothing reads a
release write-up, because there is none: a pull request records what it did in
its title, once, and the release reads it from there ([§FS-002-release.1.3](FS-002-release.md#13-the-release-lists-the-pull-requests-merged-since-the-previous-tag-and-every-one-of-them)). A
step that runs `prepare` or `notes`, as the release workflows do, is the release
and not a gate on a change.

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
