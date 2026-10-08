# integration

Integration tests prove the How: that the parts fit as designed. This is the
home of the non-citable `integration` kind — no file here carries an ID, and
`[citations.integration]` in `grund.toml` says the home should cite
`AR`. A test belongs here when its subject spans more than one part: a command
end to end against a real checkout and registry, or a repository-hygiene
script that holds the tree itself to a rule. A claim about one module is a
unit test beside it; black-box proof of a spec point is an e2e case under
`tests/e2e/`.

## Rust

`cargo test --all-targets`, declared by path in `Cargo.toml` because Cargo does
not auto-discover tests in a subdirectory. `common/` is the shared harness
(`mod common;`); `golden/` holds recorded provider responses fixtures compare
against.

- `check_test.rs` — `ephor check` end to end ([§FS-006-project-interface.5](../../docs/functional-spec/FS-006-project-interface.md#5-checks-are-verbs-and-every-script-is-self-contained)).
- `checkout_test.rs` — `ephor checkout` end to end ([§FS-004-quick-actions.7](../../docs/functional-spec/FS-004-quick-actions.md#7-a-workspace-that-is-not-there-is-offered-the-checkout)).
- `doctor_test.rs` — `ephor doctor` and `ephor capabilities` ([§FS-010-doctor](../../docs/functional-spec/FS-010-doctor.md#fs-010-doctor-ephor-can-be-asked-whether-it-still-works-and-answers-in-one-screen)).
- `rebase_test.rs` — `ephor rebase` end to end ([§FS-004-quick-actions.6](../../docs/functional-spec/FS-004-quick-actions.md#6-a-branch-that-trails-its-main-branch-is-offered-the-rebase)).
- `work_test.rs` — `ephor work` end to end ([§FS-005-dispatch](../../docs/functional-spec/FS-005-dispatch.md#fs-005-dispatch-what-ephor-watches-it-can-hand-to-an-agent-runtime)).
- `scope_test.rs` — the scope selectors across the registry and the site's
  watch list, over a world with two organizations
  ([§FS-011-command-line.9](../../docs/functional-spec/FS-011-command-line.md#9-a-scope-selector-is-honoured-or-refused), [§AR-009-surfaces.2](../../docs/architecture/AR-009-surfaces.md#2-the-session-is-built-once-and-shared)).
- `forge_extension_test.rs` — an out-of-process forge extension, a real shell
  script and nothing else ([§FS-001-forge-interface.2](../../docs/functional-spec/FS-001-forge-interface.md#2-two-transports-one-interface)).
- `agents_test.rs`, `feed_test.rs`, `registry_test.rs`, `update_test.rs` — the
  registry and feed engine driven through the CLI: project registration,
  fetch/refresh, and the AGENTS.md rendering path.

## Python

`python -m unittest discover -s tests/integration -p 'test_*.py'`, the same
line CI and the pre-commit hook run.

- `test_check_boundary.py` — the boundary check itself ([§REQ-001-boundary.5](../../docs/requirements/REQ-001-boundary.md#5-no-product-literal-outside-its-adapter)).
- `test_check_parity.py` — Cargo-aware parity executable selection, standalone
  build fallback, and actionable operational failures
  ([§AR-009-surfaces.5.1](../../docs/architecture/AR-009-surfaces.md#51-cargo-chooses-the-executable-the-parity-gate-inspects)).
- `test_grund_pin_entrypoints.py` — the pinned grund and the entrypoint blocks
  it generates ([§FS-002-release.7](../../docs/functional-spec/FS-002-release.md#7-a-pinned-checker-and-the-blocks-it-generates-move-together)). The generator half runs the pinned binary, so
  it skips with a `harness:` message where that binary is not installed — which
  is why CI runs this line in the `grund` job as well as the `cargo test` job:
  only the former has the pin on `PATH`.
- `test_prepare_changelog_release.py` — the release script writing its own
  notes: the range, the pull requests that qualify, complete listings, order
  and rendering, rotation, and every refusal leaving the tree untouched
  ([§FS-002-release.1.3](../../docs/functional-spec/FS-002-release.md#13-the-release-lists-the-pull-requests-merged-since-the-previous-tag-and-every-one-of-them), [§FS-002-release.2.3](../../docs/functional-spec/FS-002-release.md#23-preparing-a-release-reads-everything-before-it-writes-and-refuses-rather-than-guess)).
- `test_release_forge.py` — the release harness's own `git`, below: one
  `unable to create temporary file` is outlasted, the retry says so on stderr,
  and every other failure is raised after one call. A `git` stand-in on the
  fixture's `PATH` fails the calls a case arms (#202).
- `test_release_notices.py` — the compatibility notices a later release writes
  for a schema that lost or changed a field ([§FS-002-release.1.4](../../docs/functional-spec/FS-002-release.md#14-compatibility-notices-from-the-previous-tag-onwards)).
- `test_release_workflows.py` — the release workflows: nothing stamped, counted
  or held on, the shared path predicate behind the schedule's gate, and no hook
  or step that asks a change for a changelog entry ([§FS-002-release.2](../../docs/functional-spec/FS-002-release.md#2-cutting-a-release),
  [§FS-002-release.6](../../docs/functional-spec/FS-002-release.md#6-no-change-is-gated-on-the-changelog)).
- `release_forge.py` — not a test, and not collected as one: the throwaway git
  repository, with a `gh` stand-in on `PATH` that answers as GitHub's REST API
  does, those three share. The release reads first-parent history and the pull
  requests that landed on it, so it cannot be shown with loose files. The
  fixture's own `git` call is retried when git fails with its transient
  `unable to create temporary file`, and on nothing else: on the hosted macOS
  runner, about one run in twelve, one loose-object write was refused with
  `Invalid argument` and killed the heaviest `CompletenessTests` before the
  release ever ran (#202).

Unit tests stay beside the code under `code`'s rule; there is no third kind
for them.
