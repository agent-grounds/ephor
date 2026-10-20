#!/usr/bin/env python3
"""Observe a real cached screen, then refresh it (§AR-009-surfaces.2).

This driver is called by the ignored Rust regression, not unittest discovery.
It returns observations; Rust owns the assertions about the reported defect.
"""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import sys
import termios
import time

binary, root = sys.argv[1], Path(sys.argv[2])
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 180, 0, 0))
process = subprocess.Popen([binary, "tui", "--workspace", "demo"], cwd=root,
                           stdin=slave, stdout=slave, stderr=slave,
                           env={**os.environ, "TERM": "xterm-256color"})
os.close(slave)
transcript = bytearray()
ansi = re.compile(rb"\x1b\[[0-?]*[ -/]*[@-~]")


def wait_for(predicate):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if predicate():
            return
        if process.poll() is not None:
            raise AssertionError(f"TUI exited {process.returncode}: {transcript!r}")
        if select.select([master], [], [], 0.05)[0]:
            try:
                transcript.extend(os.read(master, 65536))
            except OSError as error:
                raise AssertionError(f"PTY closed: {transcript!r}") from error
    raise AssertionError(f"TUI did not reach expected screen: {transcript!r}")


def counts():
    log = root / "git-events.jsonl"
    events = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
    return [sum(event["probe"] and event["event"] == "end"
                and str(root / "demo" / branch) in event["args"] for event in events)
            for branch in ["main", "feature-1", "feature-2", "feature-3"]]


try:
    wait_for(lambda: b"Startup223" in ansi.sub(b"", transcript))
    startup = counts()
    answer_path = root / "answer.json"
    answer = json.loads(answer_path.read_text())
    answer["matters"][0]["title"] = "Freshreading"
    answer_path.write_text(json.dumps(answer))
    os.write(master, b"r")
    wait_for(lambda: b"Freshreading" in ansi.sub(b"", transcript))
    # Arrival can be drawn before the completed refresh's standing sweep.
    wait_for(lambda: all(after >= before + 1 for before, after in zip(startup, counts())))
    after = counts()
    os.write(master, b"q")
    assert process.wait(timeout=10) == 0
    print(json.dumps({"first_frame": True, "refreshed_frame": True,
                      "startup_sweeps": startup,
                      "refresh_sweeps": [a - b for a, b in zip(after, startup)]}))
finally:
    if process.poll() is None:
        process.kill()
        process.wait()
    (root / "terminal.log").write_bytes(transcript)
    os.close(master)
