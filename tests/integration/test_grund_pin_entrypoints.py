"""The pinned grund and the entrypoint blocks it generates agree.

§FS-002-release.7: the pin is a concrete published release, the checked-in
managed blocks are that release's own output, and CI's install, cache identity
and compatibility note name the same pairing as the documented contributor
setup. The regeneration half calls the pinned generator rather than restating
the template, so a block that drifts is caught by the tool that writes it.
"""

from pathlib import Path
import os
import re
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CI = ROOT / ".github" / "workflows" / "ci.yml"
JUSTFILE = ROOT / "justfile"
ENTRYPOINTS = ("CLAUDE.md", ".claude/CLAUDE.md")
GRUND_CONFIG = "grund.toml"
SETUP_DOCS = ("README.md", "docs/manual.md", ".pre-commit-config.yaml")
GATE_DEFINITIONS = (
    CI.relative_to(ROOT).as_posix(),
    JUSTFILE.relative_to(ROOT).as_posix(),
    ".pre-commit-config.yaml",
)
SELECTION_FLAGS = ("--ignore", "--only")

BEGIN = "<!-- BEGIN GRUND MANAGED BLOCK -->"
END = "<!-- END GRUND MANAGED BLOCK -->"

INSTALL = re.compile(r"cargo install grund --version (\S+) --locked")
CACHE_KEY = re.compile(r"cargo-grund-(\S+?)\s*$", re.MULTILINE)
RELEASE = re.compile(r"^\d+\.\d+\.\d+$")
BLOCK_HEADING = re.compile(r"^## Grounding with grund \(v(\d+)\)$", re.MULTILINE)
BLOCK_TOKEN = re.compile(r"\bv(\d+)\b")


class HarnessError(Exception):
    """The check could not be run — not a verdict about the repository."""


def install_version(ci_text: str) -> str:
    match = INSTALL.search(ci_text)
    if match is None:
        raise HarnessError("no `cargo install grund --version … --locked` step in ci.yml")
    return match.group(1)


def cache_key_version(ci_text: str) -> str:
    match = CACHE_KEY.search(ci_text)
    if match is None:
        raise HarnessError("no `cargo-grund-<version>` cache key in ci.yml")
    return match.group(1)


def pin_comment(ci_text: str) -> str:
    """The comment lines immediately above the grund install step.

    The step's own `- name:` line sits between the comment and the `run:` the
    version is written on, so walking up skips the step header before it starts
    collecting.
    """
    lines = ci_text.splitlines()
    for index, line in enumerate(lines):
        if INSTALL.search(line) is None:
            continue
        comment: list[str] = []
        for cursor in range(index - 1, -1, -1):
            stripped = lines[cursor].strip()
            if stripped.startswith("#"):
                comment.insert(0, stripped.lstrip("# "))
            elif not stripped:
                continue
            elif stripped.startswith("- ") and not comment:
                continue  # the step's own `- name:` header
            else:
                break
        return "\n".join(comment)
    raise HarnessError("no grund install step in ci.yml")


def managed_region(text: str) -> str:
    start = text.find(BEGIN)
    end = text.find(END)
    if start < 0 or end < 0:
        raise HarnessError(f"no grund managed block delimited by {BEGIN} … {END}")
    return text[start : end + len(END)]


def outside_managed_region(text: str) -> tuple[str, str]:
    start = text.find(BEGIN)
    end = text.find(END)
    if start < 0 or end < 0:
        raise HarnessError(f"no grund managed block delimited by {BEGIN} … {END}")
    return text[:start], text[end + len(END) :]


def block_version(region: str) -> str:
    match = BLOCK_HEADING.search(region)
    if match is None:
        raise HarnessError("the managed block carries no `## Grounding with grund (vN)` heading")
    return match.group(1)


def binary_version(binary: Path) -> str:
    try:
        reported = subprocess.run(
            [str(binary), "--version"], capture_output=True, text=True, check=True
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError) as error:
        raise HarnessError(f"{binary} does not answer `--version`: {error}") from error
    if not reported.startswith("grund "):
        raise HarnessError(f"{binary} --version says {reported!r}, which is not a grund")
    return reported[len("grund ") :].strip()


def resolve_pinned_binary(pinned: str, candidates: list[Path]) -> Path:
    """The first candidate reporting exactly the pinned release.

    A missing or differently-versioned binary is a harness condition: the
    repository is not what failed, the machine running the check is.
    """
    seen: list[str] = []
    for candidate in candidates:
        if candidate is None or not Path(candidate).exists():
            continue
        found = binary_version(Path(candidate))
        if found == pinned:
            return Path(candidate)
        seen.append(f"{candidate} is grund {found}")
    raise HarnessError(
        f"no grund {pinned} available to run the generator"
        + (f" ({'; '.join(seen)})" if seen else "")
        + "; install it or point GRUND_PINNED_BIN at it"
    )


def pinned_binary_candidates() -> list[Path]:
    """An explicit override, then `grund` as a shell would find it, then cargo's."""
    found: list[Path] = []
    for candidate in (
        os.environ.get("GRUND_PINNED_BIN"),
        shutil.which("grund"),
        Path.home() / ".cargo" / "bin" / "grund",
    ):
        if candidate is None:
            continue
        resolved = Path(candidate)
        if resolved not in found:
            found.append(resolved)
    return found


def regenerate(binary: Path, sources: dict[str, str], config: str) -> dict[str, str]:
    """Run the pinned generator over scratch copies and return what it wrote."""
    with tempfile.TemporaryDirectory(prefix="grund-pin-") as scratch:
        root = Path(scratch)
        (root / GRUND_CONFIG).write_text(config, encoding="utf-8")
        for name, text in sources.items():
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding="utf-8")
        result = subprocess.run(
            [str(binary), "init", "--claude", "--no-vcs", str(root)],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise HarnessError(
                f"`grund init --claude` exited {result.returncode}: "
                f"{(result.stderr or result.stdout).strip()}"
            )
        return {name: (root / name).read_text(encoding="utf-8") for name in sources}


def check_invocations(text: str) -> list:
    """Every executable `grund check` command line in a gate definition.

    A YAML gate writes the command after `run:` or `entry:`, either on its own
    line or inline on the list item, and the justfile writes it as a bare recipe
    line — so all four shapes reduce to the same thing. Comment lines are
    skipped: prose explaining a selection is not the selection, and
    `name: grund check` is a label rather than a command.
    """
    found = []
    for line in text.splitlines():
        command = line.strip()
        if command.startswith("#"):
            continue
        if command.startswith("- "):
            command = command[2:].strip()
        for prefix in ("run:", "entry:"):
            if command.startswith(prefix):
                command = command[len(prefix) :].strip()
                break
        if command == "grund check" or command.startswith("grund check "):
            found.append(command)
    return found


def check_selection(text: str) -> frozenset:
    """Which findings a gate's `grund check` reports, as a comparable set.

    A code is tagged with the flag that named it, so holding one out and
    selecting only it never read as the same selection. An empty set is the
    whole report, which is what every gate carries once a deferral is cleared.
    """
    invocations = check_invocations(text)
    if not invocations:
        raise HarnessError("no executable `grund check` invocation")
    codes = set()
    for command in invocations:
        tokens = command.split()
        for index, token in enumerate(tokens):
            flag, separator, attached = token.partition("=")
            if flag not in SELECTION_FLAGS:
                continue
            if separator:
                codes.add((flag.lstrip("-"), attached))
            elif index + 1 < len(tokens):
                codes.add((flag.lstrip("-"), tokens[index + 1]))
            else:
                raise HarnessError(f"`{flag}` names no code in `{command}`")
    return frozenset(codes)


def read(name: str) -> str:
    return (ROOT / name).read_text(encoding="utf-8")


class PinnedCheckerContract(unittest.TestCase):
    """The repository's own files, held to §FS-002-release.7."""

    def setUp(self) -> None:
        self.ci = read(CI.relative_to(ROOT).as_posix())

    def test_the_pin_is_a_concrete_published_release(self) -> None:
        version = install_version(self.ci)
        self.assertRegex(
            version,
            RELEASE,
            "§FS-002-release.7: the pin must be a published release, not a dev build",
        )

    def test_install_cache_and_comment_name_one_pairing(self) -> None:
        version = install_version(self.ci)
        self.assertEqual(
            cache_key_version(self.ci),
            version,
            "§FS-002-release.7: the cache identity must key on the installed release",
        )
        comment = pin_comment(self.ci)
        carried = block_version(managed_region(read(ENTRYPOINTS[0])))
        self.assertIn(
            version,
            comment,
            "§FS-002-release.7: the compatibility note must name the pinned release",
        )
        self.assertIn(
            carried,
            BLOCK_TOKEN.findall(comment),
            "§FS-002-release.7: the compatibility note must name the block version "
            "the entrypoints carry",
        )

    def test_contributor_setup_names_the_pinned_release(self) -> None:
        version = install_version(self.ci)
        command = f"cargo install grund --version {version} --locked"
        naming = [name for name in SETUP_DOCS if command in read(name)]
        self.assertTrue(
            naming,
            "§FS-002-release.7: the documented contributor setup must name the pinned "
            f"release; none of {list(SETUP_DOCS)} carries `{command}`",
        )

    def test_both_entrypoints_carry_the_same_block_version(self) -> None:
        versions = {name: block_version(managed_region(read(name))) for name in ENTRYPOINTS}
        self.assertEqual(
            len(set(versions.values())),
            1,
            f"§FS-002-release.7: the entrypoints disagree on the block version: {versions}",
        )

    def test_every_gate_invocation_carries_the_same_check_selection(self) -> None:
        """§FS-002-release.7: the local gate reaches the verdict the job will.

        Asserted as equality rather than against a particular code: the gates
        agreeing is the lasting invariant, while any one selection is temporary,
        so clearing a deferral is the deletion of the flags and nothing else.
        """
        selections = {name: check_selection(read(name)) for name in GATE_DEFINITIONS}
        self.assertEqual(
            len(set(selections.values())),
            1,
            "§FS-002-release.7: the gates disagree about which findings "
            "`grund check` reports, so a contributor's hook and the job reach "
            f"different verdicts on one tree: {selections}",
        )

    def test_entrypoints_are_the_pinned_generators_own_output(self) -> None:
        version = install_version(self.ci)
        try:
            binary = resolve_pinned_binary(version, pinned_binary_candidates())
        except HarnessError as error:
            self.skipTest(f"harness: {error}")
        checked_in = {name: read(name) for name in ENTRYPOINTS}
        try:
            generated = regenerate(binary, dict(checked_in), read(GRUND_CONFIG))
        except HarnessError as error:
            self.fail(f"harness: {error}")
        for name in ENTRYPOINTS:
            with self.subTest(entrypoint=name):
                self.assertEqual(
                    managed_region(checked_in[name]),
                    managed_region(generated[name]),
                    f"§FS-002-release.7: {name} is not grund {version}'s own output "
                    f"(checked in v{block_version(managed_region(checked_in[name]))}, "
                    f"generated v{block_version(managed_region(generated[name]))})",
                )
                self.assertEqual(
                    outside_managed_region(checked_in[name]),
                    outside_managed_region(generated[name]),
                    f"§FS-002-release.7: regenerating {name} changed bytes outside the block",
                )


class PairingCheckerCases(unittest.TestCase):
    """The checker above, against inputs the repository is not expected to hold."""

    CI_SHAPE = (
        "          key: ${{{{ runner.os }}}}-cargo-grund-{cache}\n"
        "\n"
        "      # Pinned to grund {comment} and the v{block} block it writes.\n"
        "      - name: Install grund\n"
        "        run: cargo install grund --version {install} --locked\n"
    )

    def ci(self, install="1.2.3", cache="1.2.3", comment="1.2.3", block="10") -> str:
        return self.CI_SHAPE.format(
            install=install, cache=cache, comment=comment, block=block
        )

    def test_a_cache_key_left_behind_is_a_mismatch(self) -> None:
        text = self.ci(install="1.2.3", cache="1.2.2")
        self.assertNotEqual(install_version(text), cache_key_version(text))

    def test_a_comment_left_behind_is_a_mismatch(self) -> None:
        text = self.ci(install="1.2.3", comment="1.2.2")
        self.assertNotIn(install_version(text), pin_comment(text))

    def test_a_dev_build_is_not_a_pin(self) -> None:
        self.assertNotRegex(install_version(self.ci(install="1.2.3-dev")), RELEASE)

    def test_one_stale_entrypoint_is_found(self) -> None:
        current = f"{BEGIN}\n## Grounding with grund (v10)\n{END}\n"
        stale = f"{BEGIN}\n## Grounding with grund (v8)\n{END}\n"
        self.assertEqual(block_version(managed_region(current)), "10")
        self.assertEqual(block_version(managed_region(stale)), "8")
        self.assertNotEqual(managed_region(current), managed_region(stale))

    def test_content_around_the_block_is_compared_separately(self) -> None:
        kept = f"preamble\n\n{BEGIN}\nbody\n{END}\n\nrepository notes\n"
        moved = f"preamble\n\n{BEGIN}\nbody\n{END}\n\nrepository notes changed\n"
        self.assertEqual(managed_region(kept), managed_region(moved))
        self.assertNotEqual(outside_managed_region(kept), outside_managed_region(moved))

    def test_a_selection_in_one_gate_and_missing_from_another_is_drift(self) -> None:
        job = "    steps:\n      - run: grund check --ignore local-section-citation\n"
        hook = "        entry: grund check\n"
        self.assertNotEqual(check_selection(job), check_selection(hook))

    def test_the_attached_form_of_the_flag_is_the_same_selection(self) -> None:
        spaced = "    grund check --ignore local-section-citation\n"
        attached = "        run: grund check --ignore=local-section-citation\n"
        self.assertEqual(check_selection(spaced), check_selection(attached))

    def test_holding_a_code_out_is_not_selecting_only_it(self) -> None:
        ignored = "    grund check --ignore local-section-citation\n"
        only = "    grund check --only local-section-citation\n"
        self.assertNotEqual(check_selection(ignored), check_selection(only))

    def test_prose_about_a_selection_is_not_a_selection(self) -> None:
        commented = (
            "      # grund check --ignore local-section-citation is held out\n"
            "      - name: grund check\n"
            "        run: grund check\n"
        )
        self.assertEqual(check_selection(commented), frozenset())

    def test_a_gate_with_no_check_invocation_is_a_harness_error(self) -> None:
        with self.assertRaises(HarnessError):
            check_selection("        run: grund fmt --check\n")

    def test_a_flag_naming_no_code_is_a_harness_error(self) -> None:
        with self.assertRaises(HarnessError):
            check_selection("    grund check --ignore\n")

    def test_a_block_with_no_markers_is_a_harness_error(self) -> None:
        with self.assertRaises(HarnessError):
            managed_region("no managed block here\n")

    def test_a_wrong_binary_is_a_harness_error_not_a_verdict(self) -> None:
        with tempfile.TemporaryDirectory(prefix="grund-pin-bin-") as scratch:
            fake = Path(scratch) / "grund"
            fake.write_text("#!/bin/sh\necho 'grund 0.0.1'\n", encoding="utf-8")
            fake.chmod(0o755)
            with self.assertRaises(HarnessError) as caught:
                resolve_pinned_binary("1.2.3", [fake])
            self.assertIn("grund 0.0.1", str(caught.exception))

    def test_a_missing_binary_is_a_harness_error(self) -> None:
        with self.assertRaises(HarnessError):
            resolve_pinned_binary("1.2.3", [Path("/nonexistent/grund")])


if __name__ == "__main__":
    unittest.main()
