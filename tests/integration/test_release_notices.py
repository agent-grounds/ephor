"""A schema that lost or changed a field since the previous tag is noticed in the release. §FS-002-release.1.4

The release compares every `assets/*.schema.json` from the previous tag through
each first-parent commit to `HEAD`, and writes what it finds as a
`### Compatibility notices` block after the list, so a field renamed or removed
from the machine form still reaches the release that ships it
(§REQ-002-parity.4). The first release writes none. Each case lands one pull
request that edits a schema, prepares the release, and reads the block.
"""

import copy
import json
import unittest

from release_forge import COMMIT_URL, FIRST_CHANGELOG, ReleaseCase, line, release_section


SCHEMA = "assets/ephor-x.schema.json"
BASE = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$id": "https://github.com/agent-grounds/ephor-fixture/schemas/x/v1",
    "title": "a fixture schema",
    "type": "object",
    "required": ["a"],
    "properties": {
        "a": {"type": "string", "description": "A."},
        "b": {"type": "integer"},
    },
}
NOTICES = "### Compatibility notices"


def later_changelog() -> str:
    section = f"## 2. [0.1.0] — 2026-09-01\n\n{line(1, 'feat: the schema ships')}\n\n"
    return FIRST_CHANGELOG.replace("## 3. Older releases\n", section + "## 3. Older releases\n")


class NoticeCase(ReleaseCase):
    def tagged(self, schema: dict = BASE):
        """A repository whose `v0.1.0` tag ships `schema`."""
        repo = self.repo(later_changelog())
        repo.write(SCHEMA, json.dumps(schema, indent=2) + "\n")
        repo.land_squash(1, "feat: the schema ships", ("src/one.rs",))
        repo.tag("0.1.0")
        return repo

    def change(self, repo, schema: dict, number: int = 2) -> str:
        """Land one pull request that rewrites the schema; return the commit it landed as."""
        repo.write(SCHEMA, json.dumps(schema, indent=2) + "\n")
        return repo.land_squash(number, f"feat: schema change {number}", (("keep", SCHEMA),)).sha

    def notices(self, repo) -> list[str]:
        """The bullets of the release's notices block, failing when there is no block."""
        section = release_section(self.prepare(repo, "0.1.1"), "0.1.1")
        self.assertIn(NOTICES, section, "the release carries no compatibility notices:\n" + section)
        before, block = section.split(NOTICES, 1)
        self.assertIn("(PR #2)", before, "the notices do not follow the list:\n" + section)
        return [text for text in block.splitlines() if text.startswith("- ")]

    def assert_noticed(self, notices: list[str], *parts: str) -> str:
        matching = [notice for notice in notices if SCHEMA in notice and all(part in notice for part in parts)]
        self.assertTrue(matching, f"no notice names {SCHEMA} with {parts}:\n" + "\n".join(notices))
        return matching[0]


class NoticeTests(NoticeCase):
    def test_a_removed_property_is_noticed_by_its_pointer(self) -> None:
        repo = self.tagged()
        schema = copy.deepcopy(BASE)
        del schema["properties"]["b"]
        self.change(repo, schema)
        notice = self.assert_noticed(self.notices(repo), "/properties/b")
        self.assertIn("removed", notice.lower())

    def test_a_rename_is_a_removal_and_no_guess(self) -> None:
        repo = self.tagged()
        schema = copy.deepcopy(BASE)
        schema["properties"]["c"] = schema["properties"].pop("b")
        self.change(repo, schema)
        notices = self.notices(repo)
        self.assertIn("removed", self.assert_noticed(notices, "/properties/b").lower())
        self.assertFalse([n for n in notices if "renamed" in n.lower()], "\n".join(notices))

    def test_a_changed_constraint_names_old_and_new_and_links_its_commit(self) -> None:
        repo = self.tagged()
        schema = copy.deepcopy(BASE)
        schema["properties"]["a"]["type"] = "integer"
        sha = self.change(repo, schema)
        self.assert_noticed(self.notices(repo), "/properties/a/type", "string", "integer", COMMIT_URL.format(sha=sha))

    def test_a_changed_required_set_is_noticed(self) -> None:
        repo = self.tagged()
        schema = copy.deepcopy(BASE)
        schema["required"] = ["a", "b"]
        self.change(repo, schema)
        self.assert_noticed(self.notices(repo), "/required")

    def test_a_changed_version_marker_is_noticed(self) -> None:
        repo = self.tagged()
        schema = copy.deepcopy(BASE)
        schema["$id"] = schema["$id"].replace("/v1", "/v2")
        self.change(repo, schema)
        self.assert_noticed(self.notices(repo), "/$id", "v1", "v2")

    def test_any_other_structural_change_is_potentially_incompatible(self) -> None:
        repo = self.tagged()
        schema = copy.deepcopy(BASE)
        schema["additionalProperties"] = False
        self.change(repo, schema)
        notice = self.assert_noticed(self.notices(repo), "/additionalProperties")
        self.assertIn("potentially incompatible", notice.lower())

    def test_a_schema_without_a_version_marker_is_labelled_unversioned(self) -> None:
        unversioned = {key: value for key, value in BASE.items() if key != "$id"}
        repo = self.tagged(unversioned)
        schema = copy.deepcopy(unversioned)
        del schema["properties"]["b"]
        self.change(repo, schema)
        self.assertIn("unversioned", self.assert_noticed(self.notices(repo), "/properties/b").lower())

    def test_a_version_marker_given_to_an_unversioned_schema_is_noticed(self) -> None:
        unversioned = {key: value for key, value in BASE.items() if key != "$id"}
        repo = self.tagged(unversioned)
        schema = copy.deepcopy(BASE)
        schema["$id"] = schema["$id"].replace("/v1", "/v2")
        self.change(repo, schema)
        notice = self.assert_noticed(self.notices(repo), "/$id", "v2")
        self.assertIn("added", notice.lower())

    def test_a_removal_undone_before_head_is_still_noticed(self) -> None:
        repo = self.tagged()
        schema = copy.deepcopy(BASE)
        del schema["properties"]["b"]
        self.change(repo, schema, 2)
        self.change(repo, BASE, 3)
        self.assert_noticed(self.notices(repo), "/properties/b")

    def test_notes_carry_the_notices(self) -> None:
        repo = self.tagged()
        schema = copy.deepcopy(BASE)
        del schema["properties"]["b"]
        self.change(repo, schema)
        section = release_section(self.prepare(repo, "0.1.1"), "0.1.1")
        notes = repo.root / "release-notes.md"
        self.assert_exit(repo.release("notes", "0.1.1", "--output", str(notes)), 0)
        text = notes.read_text(encoding="utf-8")
        self.assertEqual(text, section.strip() + "\n")
        self.assertIn(NOTICES, text)


class NoNoticeTests(NoticeCase):
    def test_an_optional_addition_and_a_prose_edit_are_not_noticed(self) -> None:
        repo = self.tagged()
        schema = copy.deepcopy(BASE)
        schema["properties"]["d"] = {"type": "boolean", "description": "Optional, and new."}
        schema["properties"]["a"]["description"] = "A, reworded."
        schema["title"] = "a fixture schema, retitled"
        self.change(repo, schema)
        section = release_section(self.prepare(repo, "0.1.1"), "0.1.1")
        self.assertNotIn(NOTICES, section)
        self.assertIn("(PR #2)", section)

    def test_the_first_release_carries_no_notices(self) -> None:
        repo = self.repo()
        repo.write(SCHEMA, json.dumps(BASE, indent=2) + "\n")
        repo.land_squash(1, "feat: the schema arrives", ("src/one.rs", ("keep", SCHEMA)))
        schema = copy.deepcopy(BASE)
        del schema["properties"]["b"]
        repo.write(SCHEMA, json.dumps(schema, indent=2) + "\n")
        repo.land_squash(2, "feat: and loses a field before anything shipped", (("keep", SCHEMA),))
        section = release_section(self.prepare(repo), "0.1.0")
        self.assertNotIn(NOTICES, section)
        self.assertIn("(PR #2)", section)


if __name__ == "__main__":
    unittest.main()
