"""A forge extension that answers each capability and logs every call
(§FS-001-forge-interface.2), for issue #263's narrowed refresh
(§FS-011-command-line.9.2).

Invoked by a stub on PATH as

    capability-forge.py <source> <log> <control> <subcommand>

with the request on stdin. Every call appends one line to <log>:

    <source> <block> <subcommand> [ticket,ticket,...]

where <block> is the `block` key of the configuration block that asked (the
key is opaque to ephor and handed through, like every other key), and the
tickets are the ones the request carried, which is how a ticket registered on
a branch reaches the forge's `issues` (§FS-001-forge-interface.2).

<control> is JSON: `{"fail": [<source>, ...]}` makes every data call of that
source fail. A missing control file is the empty one.

`gdev` declares pull requests (with threads and an inline gate), branch gates,
issues and notices. Both of its blocks report the same pull request at the same
instant with different titles, so the order in which a slot's reports are read
is visible in the merged row (§AR-008-pipeline.2). Its `issues` answers every
ticket it is handed. `acme` declares pull requests only.
"""

import json
import os
import sys
from pathlib import Path

SOURCE, LOG, CONTROL, SUBCOMMAND = sys.argv[1:5]
REQUEST = json.loads(sys.stdin.read() or "{}")
CONFIG = REQUEST.get("config", {})
BLOCK = CONFIG.get("block", "-")
TICKETS = REQUEST.get("tickets", [])
STAMP = "2026-07-30T12:00:00Z"


def log():
    line = f"{SOURCE} {BLOCK} {SUBCOMMAND}"
    if TICKETS:
        line += " " + ",".join(TICKETS)
    fd = os.open(LOG, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644)
    try:
        os.write(fd, (line + "\n").encode())
    finally:
        os.close(fd)


def control():
    path = Path(CONTROL)
    return json.loads(path.read_text()) if path.exists() else {}


CAPABILITIES = {
    "gdev": {"pull_requests": True, "conversation": True, "gate": True,
             "branch_gate": True, "issues": True, "notices": True},
    "acme": {"pull_requests": True, "conversation": True, "gate": True},
}


def pull_requests():
    if SOURCE == "acme":
        return [{
            "id": "app/9", "repo": "app", "number": "9",
            "title": "Acme change", "url": "https://acme.example/pr/9",
            "branch": "you/acme-9", "updated_at": STAMP,
            "role": "author", "state": "open", "cited": False,
            "threads": [{"messages": [
                {"author": "Ada", "text": "one question", "when": STAMP, "mine": False}]}],
        }]
    return [{
        "id": "G/7", "repo": "G", "number": "7",
        "title": f"Retry window (block {BLOCK})",
        "url": "https://gdev.example/pr/7",
        "branch": "you/GR-45001-retry", "updated_at": STAMP,
        "role": "author", "state": "open", "cited": False,
        "threads": [{"messages": [
            {"author": "Bo", "text": "does it reset?", "when": STAMP, "mine": False}]}],
        "gate": {"repos": [{"repo": "G", "passed": 4, "failed": 1, "running": 0}]},
    }]


def issues():
    return [{
        "key": ticket, "title": f"Ticket {ticket}", "status": "open",
        "url": f"https://gdev.example/browse/{ticket}", "updated_at": STAMP,
    } for ticket in TICKETS]


def main():
    log()
    if SUBCOMMAND == "capabilities":
        print(json.dumps(CAPABILITIES[SOURCE]))
        return 0
    if SOURCE in control().get("fail", []):
        print(f"{SOURCE}: HTTP 503 from the forge (issue #263 fixture)", file=sys.stderr)
        return 1
    answers = {
        "pull-requests": pull_requests,
        "issues": issues,
        "branch-gates": lambda: [],
        "notices": lambda: [],
        "messages": lambda: [],
    }
    if SUBCOMMAND not in answers:
        print(f"{SOURCE}: no answer for {SUBCOMMAND}", file=sys.stderr)
        return 1
    print(json.dumps(answers[SUBCOMMAND]()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
