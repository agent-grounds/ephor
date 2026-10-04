"""The release workflows run the release as it is now: nothing to stamp, hold on, or read first. §FS-002-release.2

No step stamps entries, counts them, or holds the schedule on a written section,
and nothing reads `## Unreleased` or a store of entries, because there is none
(§FS-002-release.1). The schedule's gate on observable changes calls the same
substantive-path predicate the release selects pull requests with
(§FS-002-release.1.3), and the step that prepares the release carries the forge
credentials it reads them with. No hook or step asks a change for a changelog
entry (§FS-002-release.6).
"""

import re
import unittest

from release_forge import REPOSITORY_ROOT, list_items


WORKFLOWS = REPOSITORY_ROOT / ".github" / "workflows"
BUMPS = ("auto-bump.yml", "release-minor.yml")


def text_of(name: str) -> str:
    return (WORKFLOWS / name).read_text(encoding="utf-8")


def steps_of(name: str) -> list:
    return list_items(text_of(name), "steps")


class ReleaseWorkflowTests(unittest.TestCase):
    def test_no_step_stamps_counts_or_holds_on_entries(self) -> None:
        found = []
        for workflow in sorted(WORKFLOWS.glob("*.yml")):
            for step in list_items(workflow.read_text(encoding="utf-8"), "steps"):
                label = step.keys.get("name") or step.keys.get("uses") or "?"
                for word in ("prepare_changelog_release.py stamp", "prepare_changelog_release.py pending", "gate_pending"):
                    if word in step.text:
                        found.append(f"{workflow.name}: step `{label}` still carries `{word}`")
        self.assertEqual(found, [], "\n".join(found))

    def test_nothing_reads_unreleased_or_the_entry_store(self) -> None:
        found = []
        for workflow in sorted(WORKFLOWS.glob("*.yml")):
            for number, text in enumerate(workflow.read_text(encoding="utf-8").splitlines(), 1):
                if "## Unreleased" in text or "changelog/unreleased" in text:
                    found.append(f"{workflow.name}:{number}: {text.strip()}")
        self.assertEqual(found, [], "\n".join(found))

    def test_the_schedule_gate_calls_the_shared_predicate(self) -> None:
        gates = [step for step in steps_of("auto-bump.yml") if step.keys.get("id") == "gate_substantive"]
        self.assertEqual(len(gates), 1, "Auto bump has no `gate_substantive` step")
        self.assertIn("scripts/release_paths.py", gates[0].text, "the schedule gate does not call the shared predicate")
        self.assertNotRegex(gates[0].text, r"grep -Ev", "the schedule gate keeps a path list of its own")

    def test_the_step_that_prepares_a_release_has_forge_credentials_and_full_history(self) -> None:
        for name in BUMPS:
            with self.subTest(workflow=name):
                text = text_of(name)
                self.assertIn("fetch-depth: 0", text)
                preparing = [step for step in steps_of(name) if "prepare_changelog_release.py prepare" in step.text]
                self.assertEqual(len(preparing), 1, f"{name} does not prepare the release in one step")
                job_env = re.search(r"(?m)^ {0,4}env:\n(?: {2,6}\S.*\n)*? {2,6}GH_TOKEN:", text)
                self.assertTrue(
                    "GH_TOKEN:" in preparing[0].text or job_env,
                    f"{name}: the step running `prepare` cannot reach the forge; it has no GH_TOKEN",
                )

    def test_every_step_after_the_gates_reads_only_the_substantive_gate(self) -> None:
        """A guard on the hold's removal: the dev advance still carries the release's own condition."""
        steps = steps_of("auto-bump.yml")
        conditions = {step.keys.get("name"): step.keys.get("if") for step in steps}
        self.assertEqual(
            conditions.get("Advance main to the next dev version"),
            conditions.get("Publish release commit and dispatch release"),
            "the dev-version advance does not carry the release's own condition",
        )
        self.assertNotIn("gate_pending", conditions.get("Publish release commit and dispatch release") or "")


# A script under `scripts/` whose name says it is about the changelog. Only what
# a hook or a step runs counts: a hook's name, a comment, or `git add docs/changelog`
# is not a check.
CHANGELOG_SCRIPT_RE = re.compile(r"[\w./-]*scripts/[\w.-]*changelog[\w.-]*\.(?:py|sh)\b")
# The release itself, which is the release and not a gate on a change.
RELEASE_SCRIPT = "prepare_changelog_release.py"


def changelog_checks(text: str) -> list[str]:
    """Every changelog script `text` runs other than the release's own."""
    return [m.group(0) for m in CHANGELOG_SCRIPT_RE.finditer(text) if m.group(0).rsplit("/", 1)[-1] != RELEASE_SCRIPT]


class NoGateTests(unittest.TestCase):
    """No change is gated on the changelog. §FS-002-release.6

    A guard, and it passes before the change as after: it moved here unchanged
    from the store's tests, which the change deletes.
    """

    def test_no_hook_and_no_workflow_step_asks_a_change_for_an_entry(self) -> None:
        self.assertEqual(changelog_checks("python3 release-tools/scripts/prepare_changelog_release.py notes 1.0.0"), [])
        self.assertEqual(changelog_checks("id: changelog-lint\nentry: cargo test\ngit add docs/changelog"), [])
        self.assertEqual(changelog_checks("python scripts/lint_changelog.py"), ["scripts/lint_changelog.py"])

        found = []
        hooks = list_items((REPOSITORY_ROOT / ".pre-commit-config.yaml").read_text(encoding="utf-8"), "hooks")
        self.assertIn("cargo-test", [hook.keys.get("id") for hook in hooks], "the pre-commit hooks were not read")
        for hook in hooks:
            found += [f".pre-commit-config.yaml: hook `{hook.keys.get('id')}` runs {s}" for s in changelog_checks(hook.text)]
        for workflow in sorted(WORKFLOWS.glob("*.yml")):
            for step in list_items(workflow.read_text(encoding="utf-8"), "steps"):
                label = step.keys.get("name") or step.keys.get("uses") or "?"
                found += [f"{workflow.name}: step `{label}` runs {s}" for s in changelog_checks(step.text)]
        self.assertEqual(found, [], "a change is still asked for a changelog entry:\n  " + "\n  ".join(found))


if __name__ == "__main__":
    unittest.main()
