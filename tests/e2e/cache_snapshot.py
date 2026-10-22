"""Current-screen proof of §FS-001-forge-interface.7.1, driven by Cargo.

Port of the retained #244 reproducer: same forge, cache, PTY and pyte grid.
No unittest skip: the Rust caller owns the temporary red-test ignore marker.
"""
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import termios
import time

import pyte

binary, root = sys.argv[1], Path(sys.argv[2])
world = root / "terminal-world"
world.mkdir()
for name in ("demo", "home", "state", "config", "data", "cache", "runtime", "bin"):
    (world / name).mkdir()
(world / "runtime").chmod(0o700)
env = os.environ.copy()  # Rust's shared constructor removed inherited flag spellings.
env.update(HOME=str(world / "home"), XDG_STATE_HOME=str(world / "state"),
           XDG_CONFIG_HOME=str(world / "config"), XDG_DATA_HOME=str(world / "data"),
           XDG_CACHE_HOME=str(world / "cache"), XDG_RUNTIME_DIR=str(world / "runtime"),
           EPHOR_REGISTRY=str(world / "workspaces.json"),
           EPHOR_STATUS_CONFIG=str(world / "status.json"),
           EPHOR_244_WORLD=str(world), TERM="xterm-256color", COLUMNS="140", LINES="40",
           PATH=str(world / "bin") + os.pathsep + env.get("PATH", ""))
(world / "agents.tmpl").write_text("{display_name}\n")
(world / "workspaces.json").write_text(json.dumps({
    "project_types": [{"id": "monorepo", "layout": "monorepo", "repos": [{
        "id": "repo", "path": ".", "role": "Repository root", "required": True,
        "update_mode": "branch", "default_branch": "main"}],
        "agents": {"template": str(world / "agents.tmpl"), "structure_intro": "One root:",
                   "summary_template": "{display_name} on {branch}."}}],
    "hook_sets": [], "projects": [{"id": "demo", "type": "monorepo",
        "display_name": "Demo", "root": str(world / "demo"),
        "main_branch": "main", "branches": []}]}))
(world / "status.json").write_text(json.dumps({
    "defaults": {"ttl_seconds": 600, "provider_timeout_seconds": 5},
    "projects": {"demo": {"providers": [{"provider": "cache-demo"}]}}}))
forge = world / "bin/ephor-forge-cache-demo"
forge.write_text("#!" + sys.executable + "\n" + r'''
import json, os, pathlib, sys
w = pathlib.Path(os.environ["EPHOR_244_WORLD"])
json.load(sys.stdin)
with (w / "calls").open("a") as log:
    log.write(sys.argv[1] + "\n")
if sys.argv[1] == "capabilities":
    print(json.dumps({"pull_requests": True}))
elif sys.argv[1] == "pull-requests":
    print(json.dumps([{"id": "demo/1", "repo": "demo", "number": "1",
        "title": (w / "title").read_text(), "branch": "example",
        "url": "https://fixture.invalid/demo/pull/1", "role": "author",
        "state": "open", "updated_at": "2026-10-22T12:00:00Z"}]))
else:
    raise RuntimeError("unexpected forge call: " + sys.argv[1])
''')
forge.chmod(0o755)


def refresh(label):
    out = subprocess.run([binary, "refresh", "demo"], env=env, cwd=world / "demo",
                         capture_output=True, text=True, timeout=20)
    assert out.returncode == 0, (label, out.stdout, out.stderr)
    print(label + " refresh succeeded: " + out.stdout.strip())


cache = world / "state/ephor/feed/demo.json"
warning = "Displayed feed outdated: demo — newer cache available; press r to refresh"


def healthy_cache(title):
    raw = cache.read_bytes()
    feed = json.loads(raw)
    slots = feed["providers"].values()
    assert [m["title"] for s in slots for m in s["matters"]] == [title], feed
    assert all(s["ok"] and not s.get("stale", False) for s in slots), feed
    return raw, feed["fetched_at"]


process = None
master = None
try:
    (world / "title").write_text("OLD cache row")
    refresh("Initial")
    old_bytes, old_stamp = healthy_cache("OLD cache row")
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 140, 0, 0))

    def terminal_session():
        os.setsid()
        fcntl.ioctl(0, termios.TIOCSCTTY, 0)

    process = subprocess.Popen([binary, "tui"], env=env, cwd=world / "demo",
                               stdin=slave, stdout=slave, stderr=slave,
                               preexec_fn=terminal_session)
    os.close(slave)
    screen = pyte.Screen(140, 40)
    stream = pyte.Stream(screen)
    decoder = codecs.getincrementaldecoder("utf-8")("replace")

    def current():
        return "\n".join(line.rstrip() for line in screen.display)

    def pump(seconds, until=None):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            assert process.poll() is None, ("TUI exited early", current())
            ready, _, _ = select.select([master], [], [], min(0.05, max(0, deadline - time.monotonic())))
            if ready:
                data = os.read(master, 65536)
                if b"\x1b[6n" in data:
                    os.write(master, b"\x1b[1;1R")
                stream.feed(decoder.decode(data))
            if until is not None and until(current()):
                return True
        return False

    assert pump(12, lambda text: "OLD cache row" in text), current()
    print("Initial current screen contains OLD cache row")
    # The displayed rows and warning stay unchanged over an idle tick. The
    # focused Rust test checks tick's redraw request; cursor protocol writes
    # are not screen changes and are not counted here.
    pump(0.3)
    initial = current()
    pump(2.5)
    assert current() == initial and warning not in initial, current()
    print("Unchanged tick leaves the current screen alone")

    # Neither losing the file nor a changed malformed cache is a successful
    # refresh. The last-good row must survive both, without a false warning.
    cache.unlink()
    pump(2.5)
    assert "OLD cache row" in current() and "Displayed feed outdated:" not in current(), current()
    cache.write_text("{malformed feed cache")
    pump(2.5)
    assert "OLD cache row" in current() and "Displayed feed outdated:" not in current(), current()
    print("Missing/malformed caches preserve last-good rows without a false warning")
    # Restore the valid old document so external refresh reads a healthy cache.
    cache.write_bytes(old_bytes)
    (world / "title").write_text("NEW cache row")
    refresh("External")
    new_bytes, new_stamp = healthy_cache("NEW cache row")
    assert new_bytes != old_bytes and new_stamp > old_stamp, "cache did not advance"
    print("External process wrote a strictly newer healthy NEW cache")
    warned = pump(6, lambda text: warning in text)
    assert "OLD cache row" in current() and "NEW cache row" not in current(), current()
    assert cache.read_bytes() == new_bytes, "observation rewrote the cache"
    calls = (world / "calls").read_text().splitlines()
    assert calls == ["capabilities", "pull-requests"] * 2, calls
    print("Current screen retains OLD; cache retains NEW; exactly two forge fetches")
    assert warned, (
        "missing project-attributed outdated-display warning after newer healthy cache write; "
        f"expected {warning!r}; current screen:\n{current()}"
    )
    print("Current screen names demo, the outdated display, newer cache and r; only two fetches")
    pump(0.3)
    pump(2.5)
    assert warning in current(), "warning disappeared while OLD remained"

    # A later broken file cannot erase established evidence of a newer cache.
    cache.write_text("{malformed again")
    pump(2.5)
    assert warning in current() and "OLD cache row" in current(), current()
    cache.write_bytes(new_bytes)
    os.write(master, b"r")
    assert pump(12, lambda text: "NEW cache row" in text and warning not in text), current()
    assert "OLD cache row" not in current(), current()
    healthy_cache("NEW cache row")
    assert (world / "calls").read_text().splitlines() == ["capabilities", "pull-requests"] * 3
    print("Existing r action updates the displayed snapshot and clears its warning")
finally:
    if process is not None:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
    if master is not None:
        os.close(master)
