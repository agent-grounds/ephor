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

## 4. Every ephor the suite causes runs without ephor's own flag spellings

`ephor checkout`, `ephor rebase` and the rebase sweep read each value a reader
types as a flag from an environment variable when the flag is absent —
`--report` from `REPORT`, `--upstream` from `UPSTREAM`, and the rest — because
that is how a program state hands a program its values
([§FS-011-command-line.9](../functional-spec/FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused)). That is the shipped tool's behaviour, and this point
leaves it as it is. What it rules out is the suite handing those names on: an
`ephor` a test starts inherits whatever the suite's caller exported unless the
suite takes it off. A rhei agent exports a relative `REPORT` naming its own
result to everything it runs, so the suite run under one wrote a test's rebase
report into the checkout it was run from — over the agent's result, where the
two paths met — and passed. An inherited `PROJECT`, `ITEM` or `UPSTREAM` turned
a sweep a case runs into a refusal and failed a case that passes clean, and the
failure pointed its reader at the sweep rather than at the environment. The
first writes outside what the suite made; the second makes the verdict depend
on who ran the suite, which is what [§REQ-003-test-isolation.2](REQ-003-test-isolation.md#2-under-a-git-hook-exactly-as-under-a-plain-run) rules out for a
hook. So every `ephor` the suite causes starts with these names removed from
the child's environment.

### 4.1 The names are written down once

What is removed is every name ephor reads as a flag's environment spelling —
today `BRANCH`, `CHECKOUT`, `FROM`, `HAND`, `ITEM`, `ONTO`, `PROJECT`, `REPORT`
and `UPSTREAM` — and the list the removal takes them from is held to what
`src/` reads: a check fails when ephor reads a spelling the removal lacks,
whether it reads it through `given` or around it with `std::env::var`. A list
kept by hand is what the next spelling is left off; the first one written for
this point named seven and left `BRANCH` and `FROM` out.

### 4.2 Only what a test sets on the spawn survives

A spelling a test sets deliberately on a spawn — `CHECKOUT`, `ONTO` and
`REPORT` where `rebase_test` reads a program state's handover, `PROJECT` and
`BRANCH` where `checkout_test` and E2E-016 do — is set after the removal and is
kept, as [§REQ-003-test-isolation.3.2](REQ-003-test-isolation.md#32-only-what-a-test-sets-on-the-spawn-survives) keeps a fixture's identity. Nothing the
suite inherited is kept by accident.

### 4.3 Every ephor the suite causes

The removal reaches every `ephor` a test causes: the one an end-to-end case
starts through its world, the one an integration test starts through its
helpers, the one a driver program starts with a copy of such a command's
environment, removals included, and the one a case binary re-run as a child
starts through those same helpers. A driver that starts `ephor` from an empty
environment has nothing left to remove.

A library call made in-process is out of a spawn's reach: it reads the test
process's own environment, whatever any spawn removes. Nothing in the suite
makes one today — no unit test in `src/` and no case under `tests/` calls
`checkout`, `rebase` or the sweep in-process, and only `main` reaches them —
and a test that needs one of those paths starts the binary rather than calling
it.

### 4.4 One place applies it

Only `tests/` starts the `ephor` binary — no unit test does, and the Python
fixtures start it only as a Rust driver hands them that command's environment —
so the one place is the constructor [§REQ-003-test-isolation.3.4](REQ-003-test-isolation.md#34-one-place-per-tree-applies-it) already names
there for `ephor`. It removes the names when it builds the command, before any
caller's own value is set, and every spawn of `ephor` goes through it. Neither
the removal nor a test that exercises a spelling changes the test process's own
environment with `std::env::remove_var` or `set_var`: that races every other
test thread in the binary, and an in-process read would see it.
