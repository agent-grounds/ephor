#!/usr/bin/env python3
"""Log real Git intervals for the standing regressions (§AR-004-forest.2)."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time

home = Path(os.environ["HOME"])
args = sys.argv[1:]
probe = "for-each-ref" in args and "refs/heads" in args


def record(event):
    entry = {"event": event, "pid": os.getpid(), "probe": probe,
             "args": args, "time": time.monotonic_ns()}
    fd = os.open(home / "git-events.jsonl", os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        os.write(fd, (json.dumps(entry) + "\n").encode())
    finally:
        os.close(fd)


record("start")
if probe:
    # Measure intervals rather than imposing an overall startup time budget.
    time.sleep(0.2)
result = subprocess.run([(home / "real-git").read_text().strip(), *args])
record("end")
sys.exit(result.returncode)
