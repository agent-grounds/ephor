# REQ-003-test-isolation: the suite acts only on repositories it created, and a git hook changes nothing about it

The test suite is run in two places that must give one verdict: by CI, and by
the pre-commit hook before every commit (`.pre-commit-config.yaml` runs
`cargo test --all-targets --locked` and the Python line, and says every hook
mirrors a CI step). A suite that passes in one and fails in the other is a gate
nobody can trust, and a suite that writes outside what it made costs the
repository it is run from — the one thing watching must never do
([§GOAL-005-costless](../goals.md#goal-005-costless-watching-costs-the-watched-nothing)). This law binds every test, every fixture, and every
helper the suite has or gains. It says nothing about what the shipped `ephor`
does with the environment it is given; that is a behaviour, and behaviour is
specified in the FS points.

## 1. A repository no test named is never written

Every repository a test acts on is one it created, under a directory it made.
The measure is a repository no test named: after the suite has run, it has the
configuration it had (no `core.bare`, no `[user]`, no remote a fixture added),
the branches and the `HEAD` it had, and the index entries it had. A test that
passes having written into such a repository has not passed.

## 2. Under a git hook exactly as under a plain run

The suite behaves under a git hook exactly as it does from a shell: the same
tests pass, the same fail, and nothing outside them changes. A suite that
holds [§REQ-003-test-isolation.1](REQ-003-test-isolation.md#1-a-repository-no-test-named-is-never-written) everywhere except under the hook holds it
nowhere it was needed, because the hook is where the repository it would write
is the one being committed to.

That configuration is git's by design. githooks(5) says git exports `GIT_DIR`,
`GIT_INDEX_FILE` and their companions to every hook so the hook's own `git`
finds the repository, and that a hook which runs git "in a foreign repository
or in a different working tree of the same repository" should clear them first.
Every `git` the suite causes inherits them through the hook, and `-C <fixture>`
does not override an explicit `GIT_DIR`: it names a work tree and settles
nothing about which repository is written. From a linked worktree `GIT_DIR`
names the worktree's directory inside the *shared* `.git`, so a fixture's
`git init` leaves `core.bare = true` and its `git config user.name` a `[user]`
section that every checkout of the project then reads. From a plain clone git
exports a relative `GIT_INDEX_FILE`, which breaks every `git` run anywhere but
the checkout's root.

## 3. Every git the suite causes runs without git's repository variables

Every `git` the suite causes starts with git's repository variables removed
from the child's environment. That is all of them, not the ones a fixture
happens to name:

### 3.1 The rule is a prefix, not a list

What is removed is every variable `git rev-parse --local-env-vars` names —
`GIT_DIR`, `GIT_INDEX_FILE`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `GIT_PREFIX`,
`GIT_OBJECT_DIRECTORY`, `GIT_CONFIG_PARAMETERS` and the rest — and, where that
is the simpler way to be complete, every `GIT_`-prefixed variable the child
would otherwise inherit. A list is what the next variable is left off;
`GIT_CONFIG_PARAMETERS` is the one that carries an outer `git -c …` down the
same inheritance, so no flag on the outer command can contain this either.

### 3.2 Only what a test sets on the spawn survives

A value a test sets deliberately on a spawn — `GIT_AUTHOR_NAME` and the other
identity variables a fixture authors with, `GIT_CEILING_DIRECTORIES` bounding a
fixture's own discovery — is set after the removal and is kept. Nothing the
suite inherited is kept by accident.

### 3.3 Every git the suite causes, not only the helpers

The removal reaches every `git` a test causes:

- a unit test's own fixture (`run_in` in `src/git.rs`, `git_in` in
  `src/forest.rs`, and their kin);
- the code under test, in a test build: `git::create`'s `git worktree add` runs
  in the test process's environment exactly like a fixture's `git`;
- an end-to-end or integration case's own `git`, and the `ephor` binary it
  spawns, which is not a test build and so inherits whatever the case hands it;
- every subprocess the Python integration fixtures start.

### 3.4 One place per tree applies it

Each tree has one place that builds a `git` for the suite and applies the
removal — one constructor in the library, one helper under `tests/`, one in
the Python fixtures — and every caller goes through it. A call site corrected
in place leaves the next caller the same mistake to make. The removal is never
done with a process-wide `std::env::remove_var`, which races every other test
thread in the binary.
