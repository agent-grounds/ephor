"""CLI scenarios for complete combined answers (§FS-001-forge-interface.6.1).

The four-issue search fixture and one-block controls come from the retained
issue #253 reproducer. No command reaches a live forge. Collect independent
assertions so a red run records both completion orders and all failure modes.
Infrastructure errors and failed controls stop immediately instead.
"""

import copy
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys


BIN, ROOT, CASE = sys.argv[1], Path(sys.argv[2]), sys.argv[3]
ERRORS = []
PROVIDER = "rhei" if CASE == "task_stores" else "github-issues"
CONTROL = ROOT / "control.json"
LOG = ROOT / "forge.log"
SITE = CASE.startswith("site_")


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value))


def check(ok, message):
    if not ok:
        ERRORS.append(message)


def command(*args):
    out = subprocess.run([BIN, *args], capture_output=True, text=True, timeout=45)
    if "unknown " in LOG.read_text():
        raise AssertionError("fixture refused a forge call: " + LOG.read_text())
    return out


def refresh(expected, label):
    out = command("refresh", "--quiet")
    check(out.returncode == expected,
          f"{label}: refresh exit {out.returncode}, expected {expected}; stderr={out.stderr.strip()!r}")
    return out


def configure(blocks, projects=True):
    settings = {"defaults": {"github_user": "repro", "provider_timeout_seconds": 20},
                "projects": {"demo": {"providers": []}} if projects else {}}
    if SITE:
        settings["sources"] = blocks
    else:
        settings["projects"]["demo"]["providers"] = blocks
    write(ROOT / "status.json", settings)


def blocks():
    base = {"provider": "github-issues", "repos": ["demo/javelin"],
            "limit": 1000, "comments": False, "unclaimed": True}
    return [dict(base, open=True, authored=False, participating=False, updated_within_days=0),
            dict(base, authored=True, participating=True, updated_within_days=30)]


def turn(**mode):
    write(CONTROL, mode)
    LOG.write_text("")
    for name in ("open", "role"):
        Path(str(CONTROL) + "." + name).unlink(missing_ok=True)


def slot(home="demo"):
    path = ROOT / "state/ephor/feed" / (home + ".json")
    return json.loads(path.read_text()).get("providers", {}).get(PROVIDER, {}) if path.exists() else {}


def rows(home="demo"):
    args = ["feed", "--json"]
    args += ["--unattributed"] if home == "unattributed" else ["--workspace", home]
    out = command(*args)
    assert out.returncode == 0, out.stderr
    return json.loads(out.stdout)


def ids(items):
    return sorted(int(i.get("id", i.get("key")).rsplit("#", 1)[1]) for i in items)


def order(last):
    found = [line.split()[1] for line in LOG.read_text().splitlines()
             if line.startswith(("search ", "failed "))]
    assert found == (["role", "open"] if last == "open" else ["open", "role"]), found


def controls():
    for config, want in zip(blocks(), ([1, 2, 3], [4])):
        configure([config])
        turn()
        out = command("refresh", "--quiet")
        assert out.returncode == 0, out.stderr
        assert ids(rows()) == want, ("one-block control", ids(rows()), want)
    print("PASS controls: open [1, 2, 3]; roles [4]", flush=True)


def answers():
    controls()
    configure(blocks())
    for last in ("open", "role"):
        label = f"{CASE} {last}-last"
        turn(last=last)
        out = refresh(0, label)
        order(last)
        check(not out.stderr, label + ": complete answer must not warn: " + out.stderr)
        actual = ids(rows())
        check(actual == [1, 2, 3, 4], f"{label}: feed {actual}, expected [1, 2, 3, 4]")
        if SITE:
            check(not rows("unattributed"), label + ": claimed issues must not leak into bucket")
            check(all(r["placement"]["on"]["project"] == "demo" for r in slot()["matters"]),
                  label + ": repository territory places every retained issue")

    # The same open issue is found by the role block. Its unclaimed policy
    # adds one reason while the other block's comment read adds conversation.
    # A one-block reference establishes each reason independently.
    overlap = blocks()
    overlap[0]["unclaimed"] = False
    overlap[0]["comments"] = True
    for last in ("open", "role"):
        expected = set()
        for config in overlap:
            configure([config])
            turn(overlap=True)
            out = command("refresh", "--quiet")
            assert out.returncode == 0, out.stderr
            reference = rows()
            assert len(reference) == 1, reference
            expected.update(reference[0]["raw"].get("awaits", ["conversation"])
                            if reference[0]["needs_response"] else [])
        assert expected == {"conversation", "unclaimed"}, expected
        configure(overlap)
        turn(last=last, overlap=True)
        refresh(0, f"overlap {last}-last")
        order(last)
        actual = rows()
        check(len(actual) == 1, f"overlap {last}-last: expected one subject, got {len(actual)}")
        reasons = set(actual[0]["raw"].get("awaits", ["conversation"])) if actual else set()
        check(reasons == expected, f"overlap {last}-last: reasons {sorted(reasons)}, expected {sorted(expected)}")

    if SITE:
        # The same two disjoint answers must survive when no project claims
        # their venue. Correct placement includes the unattributed home.
        # Use an unrelated venue too: "demo/javelin" still resembles the
        # project id demo even with no territory claim.
        unrelated = blocks()
        for block in unrelated:
            block["repos"] = ["remote/other"]
        configure(unrelated)
        for last in ("open", "role"):
            turn(last=last, repo="remote/other")
            refresh(0, f"bucket {last}-last")
            order(last)
            actual = ids(rows("unattributed"))
            check(actual == [1, 2, 3, 4], f"bucket {last}-last: feed {actual}, expected [1, 2, 3, 4]")
            check(not rows(), f"bucket {last}-last: removed placement left old project rows")


def failure_state(label, failures, previous=None):
    current = slot()
    for name in failures:
        check(name + " block's search" in current.get("error", ""),
              f"{label}: cached diagnosis lost {name}: {current.get('error')!r}")
    if current:
        check(current.get("ok") is False, label + ": incomplete slot marked successful")
    if previous:
        check(current.get("matters") == previous["matters"], label + ": replaced complete last-good with subset")
        check(current.get("fetched_at") == previous["fetched_at"], label + ": advanced last-good timestamp")
        check(current.get("stale") == bool(previous["matters"]), label + ": last-good stale state incorrect")
        if previous["matters"]:
            reading = command("status", "demo", "--cached")
            check("(stale)" in reading.stdout, label + ": cached status must label last-good data")
    else:
        # Shared failures need not allocate a never-populated slot; their
        # diagnostics are still mandatory on stderr and in refresh --json.
        if not SITE:
            check(bool(current), label + ": missing explicit failed project slot")
        check(not current.get("matters"), label + ": invented data from incomplete answer")


def failures():
    for last in ("open", "role"):
        for empty in (False, True):
            # No prior slot: each turn has its own cold cache.
            for path in (ROOT / "state/ephor/feed").glob("*.json"):
                path.unlink()
            configure(blocks())
            turn(last=last, fail=["open"], empty=empty)
            label = f"{CASE} {last}-last empty={empty}"
            out = refresh(4, label)
            order(last)
            check("github-issues" in out.stderr and "open block's search" in out.stderr,
                  label + ": failure hidden on stderr")
            # Failure without history must be diagnosed even if shared slots
            # are intentionally absent. Never require shared cache invention.
            if SITE and not slot():
                check(not rows(), label + ": published successful subset without prior data")
            else:
                failure_state(label, ["open"])
        # With only site sources attempted, no empty project masks total loss.
        configure(blocks(), projects=not SITE)
        turn(last=last, fail=["open", "role"])
        label = f"all failed {last}-last"
        out = refresh(3, label)
        order(last)
        for name in ("open", "role"):
            check(name + " block's search" in out.stderr, label + ": stderr lost " + name)
        if not SITE:
            failure_state(label, ["open", "role"])


def last_good():
    for last in ("open", "role"):
        # Seed through a real complete single-block refresh, never through the
        # broken multi-block collector. One block asks both questions at 0 days.
        seed = dict(blocks()[1], open=True, updated_within_days=0)
        configure([seed])
        turn()
        out = command("refresh", "--quiet")
        assert out.returncode == 0, out.stderr
        assert ids(rows()) == [1, 2, 3, 4], rows()
        previous = copy.deepcopy(slot())
        configure(blocks())
        turn(last=last, fail=["open"], fresh=True)
        label = f"{CASE} {last}-last"
        out = refresh(4, label)
        order(last)
        check("open block's search" in out.stderr, label + ": lost failure")
        failure_state(label, ["open"], previous)
        turn(last=last, fail=["open", "role"])
        label = f"seeded all failed {last}-last"
        out = refresh(3, label)
        order(last)
        for name in ("open", "role"):
            check(name + " block's search" in out.stderr, label + ": stderr lost " + name)
        failure_state(label, ["open", "role"], previous)
        for empty in (False, True):
            turn(last=last, empty=empty)
            refresh(0, f"recovered empty={empty} {last}-last")
            order(last)
            current = slot()
            check(current.get("ok") is True and current.get("error") is None and not current.get("stale"),
                  f"recovered empty={empty}: failure state not cleared")
            want = [] if empty else [1, 2, 3, 4]
            check(ids(rows()) == want, f"recovered empty={empty}: feed {ids(rows())}, expected {want}")
            check(current.get("fetched_at") != previous["fetched_at"], "complete recovery did not advance timestamp")
        previous = copy.deepcopy(slot())
        turn(last=last, fail=["open"], fresh=True)
        out = refresh(4, f"incomplete after empty {last}-last")
        order(last)
        check("open block's search" in out.stderr, "empty last-good failure hidden")
        failure_state(f"empty last-good {last}-last", ["open"], previous)


def task_stores():
    configure([])
    forest = ROOT / "demo"
    write(forest / "ephor.json", {"version": 1, "tasks": [
        {"kind": "rhei", "path": "first"}, {"kind": "rhei", "path": "second"}]})
    first, second = forest / "first/alpha.rhei.md", forest / "second/beta.rhei.md"
    plan = "# Rhei: work\n\n## Tasks\n\n### Task 1: Work\n**State:** pending\n"
    for path in (first, second):
        path.parent.mkdir(parents=True)
        path.write_text(plan)
    out = command("refresh", "--quiet")
    assert out.returncode == 0, out.stderr
    assert len(slot()["matters"]) == 2, slot()
    previous = copy.deepcopy(slot())
    for bad in (first, second):
        bad.write_bytes(b"\xff")  # A real store read failure, even as root.
        label = f"task stores {bad.parent.name} fails"
        out = refresh(4, label)
        check(str(bad) in out.stderr, label + ": stderr lost failed store path")
        current = slot()
        check(current.get("ok") is False, label + ": successful sibling cleared failed state")
        check(str(bad) in current.get("error", ""), label + ": cached failure diagnosis lost")
        check(current.get("matters") == previous["matters"], label + ": lost whole-slot last-good")
        check(current.get("fetched_at") == previous["fetched_at"] and current.get("stale") is True,
              label + ": last-good timestamp/stale state lost")
        bad.write_text(plan)
        refresh(0, "task-store recovery")
        check(slot().get("ok") is True and slot().get("error") is None and not slot().get("stale"),
              "task-store recovery did not clear failure")
        previous = copy.deepcopy(slot())
    for path in (first, second):
        path.write_bytes(b"\xff")
    out = refresh(3, "all task stores failed")
    for path in (first, second):
        check(str(path) in out.stderr, "all stores failed: stderr lost " + path.parent.name)
        check(str(path) in slot().get("error", ""), "all stores failed: cache lost " + path.parent.name)
    for path in (first, second):
        path.write_text("# Rhei: empty\n")
    refresh(0, "task stores complete empty recovery")
    check(slot().get("ok") is True and slot().get("error") is None and not slot().get("stale")
          and slot().get("matters") == [], "task stores complete empty recovery retained old state")


LOG.write_text("")
os.environ["REPEATED_CONTROL"] = str(CONTROL)
os.environ["REPRO_GH_LOG"] = str(LOG)
forge = Path(__file__).with_name("fixtures") / "repeated-provider-forge.py"
stub = ROOT / "fakebin/gh"
stub.write_text("#!/bin/sh\nexec " + shlex.quote(sys.executable) + " -I " + shlex.quote(str(forge)) + ' "$@"\n')
stub.chmod(0o755)
registry_path = ROOT / "workspaces.json"
registry = json.loads(registry_path.read_text())
registry["projects"][0]["territory"] = ["demo/javelin"]
write(registry_path, registry)
turn()
if CASE == "site_overlap_evidence":
    from shared_provider_overlap import run
    run(BIN, ROOT)
elif CASE == "task_stores":
    task_stores()
elif CASE.endswith("_answers"):
    answers()
elif CASE.endswith("_failures"):
    failures()
elif CASE.endswith("_last_good"):
    last_good()
else:
    raise AssertionError("unknown scenario " + CASE)
for error in ERRORS:
    print("FAIL " + error, flush=True)
print(f"{CASE}: {len(ERRORS)} contract violation(s)", flush=True)
sys.exit(1 if ERRORS else 0)
