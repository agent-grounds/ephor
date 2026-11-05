"""Combine site reports before attribution (§FS-001-forge-interface.6.1).

Derived from the review's thin/detailed offline fixture. Also keep evidence
from equally detailed reports: the normal tie and precedence rules still apply.
"""

import json
from pathlib import Path
import shlex
import subprocess
import sys


def run(binary, root):
    def write(path, value):
        path.write_text(json.dumps(value))

    registry_path = root / "workspaces.json"
    registry = json.loads(registry_path.read_text())
    registry["projects"][0]["territory"] = ["remote/repo"]
    other = dict(registry["projects"][0], id="second", display_name="Second",
                 root=str(root / "second"), territory=[])
    (root / "second").mkdir()
    registry["projects"].append(other)
    write(registry_path, registry)
    fixture = Path(__file__).with_name("fixtures") / "overlapping-reports-forge.py"
    command = root / "overlap-forge"
    command.write_text("#!/bin/sh\nexec " + shlex.join([sys.executable, "-I", str(fixture)]) + ' "$@"\n')
    command.chmod(0o755)

    def block(mode):
        return {"provider": "review", "command": str(command), "mode": mode,
                "unclaimed": mode == "thin"}

    def invoke(*args):
        result = subprocess.run([binary, *args], capture_output=True, text=True, timeout=45)
        assert result.returncode == 0, (args, result.returncode, result.stdout, result.stderr)
        return result

    failures = []
    # Single-block controls, then both orders of the original review case.
    # Equal-detail reports pin union of evidence even when absorb picks one
    # report's title/raw fields. A reference must beat resemblance; equal
    # resemblance must retain ambiguity rather than following block order.
    cases = [
        (["thin"], "unattributed", {"unclaimed"}),
        (["detailed"], "demo", {"conversation"}),
        (["thin", "detailed"], "demo", {"conversation", "unclaimed"}),
        (["detailed", "thin"], "demo", {"conversation", "unclaimed"}),
        (["detailed", "other"], "unattributed", {"conversation"}),
        (["other", "detailed"], "unattributed", {"conversation"}),
        (["other", "reference"], "demo", {"conversation"}),
        (["reference", "other"], "demo", {"conversation"}),
    ]
    for modes, expected_home, expected_reasons in cases:
        label = "+".join(modes)
        write(root / "status.json", {"projects": {"demo": {"providers": []},
                                                   "second": {"providers": []}},
                                      "sources": [block(mode) for mode in modes]})
        refreshed = invoke("refresh", "--quiet")
        assert not refreshed.stderr, refreshed.stderr
        found = {home: json.loads(invoke("feed", "--json", *args).stdout)
                 for home, args in (("demo", ["--workspace", "demo"]),
                                    ("second", ["--workspace", "second"]),
                                    ("unattributed", ["--unattributed"]))}
        summary = {home: [(row["id"], row["raw"].get("awaits", ["conversation"]))
                          for row in reports] for home, reports in found.items()}
        print(f"{label}: refresh exit 0; {summary}", flush=True)
        rows = found[expected_home]
        ok = len(rows) == 1 and all(not rows for home, rows in found.items() if home != expected_home)
        if ok:
            row = rows[0]
            ok = (row["id"] == "review:remote/other#9" and row["needs_response"]
                  and set(row["raw"].get("awaits", ["conversation"])) == expected_reasons)
        if ok and expected_home == "unattributed" and len(modes) == 2:
            cache = json.loads((root / "state/ephor/feed/unattributed.json").read_text())
            candidates = cache["providers"]["review"]["matters"][0]["placement"]["unattributed"]["candidates"]
            ok = ok and set(candidates) == {"demo", "second"}
        if len(modes) == 1:
            assert ok, ("single-block control", label, summary)
        elif not ok:
            failures.append(label)
    assert not failures, "overlapping reports lost placement evidence/reasons: " + ", ".join(failures)
