#!/usr/bin/env python3
"""Same subject with separate reasons/evidence (§FS-001-forge-interface.6.1)."""

import json
import sys

request = json.load(sys.stdin)
if sys.argv[1] == "capabilities":
    print('{"issues": true}')
    sys.exit(0)
assert sys.argv[1] == "issues", sys.argv
mode = request["config"]["mode"]
report = {"key": "remote/other#9", "title": "one issue", "status": "open",
          "updated_at": "2026-11-01T00:00:00Z", "assigned": False, "messages": []}
if mode != "thin":
    report["messages"] = [{"author": "someone", "mine": False,
                           "text": {"detailed": "demo needs an answer",
                                    "other": "second needs an answer",
                                    "reference": "remote/repo#1 needs an answer"}[mode]}]
print(json.dumps([report]))
