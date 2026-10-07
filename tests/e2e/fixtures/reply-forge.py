"""Mail-like carrier adapted from ephor.34 (§FS-005-dispatch.13).

The optional reconciliation ledger implements §FS-001-forge-interface.1 in
the fixture only: lookup precedes freshness, and changed payload refuses.
Every reply records its wire bytes and site records visible before invocation.
"""

import json
import os
from pathlib import Path
import signal
import sys
import time


root = Path(os.environ["REPLY_WORLD"])
mail = root / "mail"
command = sys.argv[1]
wire = sys.stdin.read()
request = json.loads(wire)


def read(name, default=None):
    path = mail / name
    return json.loads(path.read_text()) if path.exists() else default


def write(name, value):
    (mail / name).write_text(json.dumps(value, ensure_ascii=False))


def consume(name):
    path = mail / name
    if not path.exists():
        return False
    path.unlink()
    return True


def append(name, value):
    with (mail / name).open("a") as stream:
        stream.write(json.dumps(value, ensure_ascii=False) + "\n")
        stream.flush()
        os.fsync(stream.fileno())


capabilities = read("capabilities.json")
if command == "capabilities":
    if consume("fail-capabilities"):
        print("cannot read declaration", file=sys.stderr)
        sys.exit(1)
    print(json.dumps(capabilities))
elif command == "messages":
    print(json.dumps([] if (mail / "absent").exists() else [read("conversation.json")]))
elif command == "reply":
    # Capture BEFORE changing the fixture, delivering, or replying. Records are
    # site-owned, not work.json or artifacts in the project's work root.
    records = {}
    for path in sorted((root / "state/ephor/replies").glob("*.json")):
        records[path.name] = json.loads(path.read_text())
    append("before.jsonl", {"request": request, "records": records})
    with (mail / "requests.jsonl").open("a") as stream:
        stream.write(wire + "\n")
        stream.flush()
        os.fsync(stream.fileno())
    if consume("unknown"):
        print(json.dumps({"status": "unknown", "note": "Check the channel: delivery unknown"}))
        sys.exit(0)

    key = json.dumps(request["target"], sort_keys=True)
    ledger = read("operations.json", {})
    reconciles = capabilities.get("reply_reconciliation", False)
    if reconciles and key in ledger:
        if ledger[key] != request["text"]:
            print("changed payload on a used descriptor", file=sys.stderr)
            sys.exit(1)
        # Recover known acceptance even if the conversation has moved on.
        print(json.dumps({"status": "accepted"}))
        sys.exit(0)

    conversation = read("conversation.json")
    threads = conversation["threads"]
    target = next((t for t in threads if t.get("reply") == request["target"]), None)
    if target is None:
        print("descriptor is no longer fresh", file=sys.stderr)
        sys.exit(1)
    deliveries = read("deliveries.json", [])
    deliveries.append(request)
    write("deliveries.json", deliveries)
    message_id = f"<s{len(deliveries)}@me.example>"
    target["messages"].append({
        "id": message_id, "author": "me", "mine": True,
        "text": request["text"], "when": "2026-10-07T10:00:00Z",
    })
    target["reply"] = {"in_reply_to": message_id}
    conversation["updated_at"] = "2026-10-07T10:00:00Z"
    write("conversation.json", conversation)
    ledger[key] = request["text"]
    write("operations.json", ledger)

    if consume("fail-ack"):
        print("accepted but acknowledgement lost", file=sys.stderr)
        sys.exit(1)

    # Accepted remotely, then the process dies before ephor can confirm it.
    if consume("crash-caller"):
        os.kill(os.getppid(), signal.SIGKILL)
        sys.exit(0)
    # No descendants: ephor's timeout kills this one fixture process.
    if consume("lose-ack"):
        time.sleep(60)
    if (mail / "block").exists():
        (mail / "entered").touch()
        deadline = time.monotonic() + 5
        while (mail / "block").exists() and time.monotonic() < deadline:
            time.sleep(0.01)
    print(json.dumps({"status": "accepted"} if reconciles else {}))
else:
    print(f"unsupported: {command}", file=sys.stderr)
    sys.exit(64)
