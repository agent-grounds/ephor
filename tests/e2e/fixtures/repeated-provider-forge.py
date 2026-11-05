"""Port of ephor #253's retained reproducer (§FS-001-forge-interface.6.1).
One repository holding four issues, and a knob for which
of two github-issues blocks answers last.

  #1 #2 #3  open and quiet, 200+ days old: only the block with `open: true` at
            `updated_within_days: 0` can see them
  #4        closed yesterday by the reader: only the role block can see it, since
            a closed issue is no answer to `state:open`

So a feed holding all four holds both blocks' answers, and neither block alone can
produce it. The two sets do not overlap, so a lost answer is visible as a set.

It answers the two things `ephor refresh` asks of github-issues, plus the login, and
appends one line per call to $REPRO_GH_LOG: `<kind> <detail>`.

  kinds   login | search <block> <query> | failed <block> | comments <n> | unknown <what>

$REPRO_LAST names the block held back so it finishes last: `open`, `role`, or
unset for no delay. The delay is what makes the order a fixture's choice rather than
the machine's. $REPRO_FAIL names a block whose search the forge answers with an
error instead. It applies a search's qualifiers rather than answering by rote, and
refuses one it does not know, which run.sh reads as exit 2.
"""

import json
import os
import re
import shlex
import sys
import time
from pathlib import Path
from datetime import datetime, timedelta, timezone

ME = "repro"
THEM = "someone"
REPO = "demo/javelin"
NOW = datetime.now(timezone.utc)
HOLD_SECONDS = 1.0
CONTROL = json.loads(Path(os.environ["REPEATED_CONTROL"]).read_text())
REPO = CONTROL.get("repo", REPO)


def stamp(days):
    return (NOW - timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")


ISSUES = [
    {"number": 1, "state": "OPEN", "author": ME, "days": 200, "talk": [ME, THEM]},
    {"number": 2, "state": "OPEN", "author": THEM, "days": 210, "talk": [THEM]},
    {"number": 3, "state": "OPEN", "author": THEM, "days": 220, "talk": []},
    {"number": 4, "state": "CLOSED", "author": ME, "days": 1, "talk": [ME, THEM]},
]


def comment_nodes(found):
    return [
        {
            "id": f"IC_{found['number']}_{index}",
            "author": {"login": who},
            "body": f"comment {index} on #{found['number']}",
            "createdAt": stamp(found["days"]),
            "reactions": {"nodes": []},
        }
        for index, who in enumerate(found["talk"])
    ]


def search_node(found):
    number = found["number"]
    return {
        "id": f"I_{number}",
        "number": number,
        "title": f"issue {number}",
        "url": f"https://github.com/{REPO}/issues/{number}",
        "updatedAt": stamp(found["days"]),
        "state": found["state"],
        "repository": {"nameWithOwner": REPO},
        "author": {"login": found["author"]},
        "assignees": {"nodes": []},
        "labels": {"nodes": []},
        "comments": {"totalCount": len(found["talk"]), "nodes": comment_nodes(found)},
        "blockedBy": {"nodes": []},
    }


class Unknown(Exception):
    pass


class Failed(Exception):
    pass


def since(term):
    return datetime.strptime(term.split(">=", 1)[1], "%Y-%m-%d").replace(tzinfo=timezone.utc)


def matches(found, query):
    age = NOW - timedelta(days=found["days"])
    for term in shlex.split(query):
        if term in ("is:issue", "sort:updated-desc", f"repo:{REPO}"):
            continue
        if term == "author:@me":
            keep = found["author"] == ME
        elif term == "involves:@me":
            keep = found["author"] == ME or ME in found["talk"]
        elif term in ("state:open", "is:open"):
            keep = found["state"] == "OPEN"
        elif term in ("state:closed", "is:closed"):
            keep = found["state"] == "CLOSED"
        elif term.startswith("updated:>="):
            keep = age >= since(term)
        elif term.startswith("closed:>="):
            keep = found["state"] == "CLOSED" and age >= since(term)
        elif term.startswith("label:"):
            keep = False  # nothing on this forge carries a label
        else:
            raise Unknown(f"qualifier {term!r} in {query!r}")
        if not keep:
            return False
    return True


FIELD = re.compile(
    r'(\w+):search\(query:("(?:[^"\\]|\\.)*"),type:ISSUE,first:(\d+)(?:,after:("(?:[^"\\]|\\.)*"))?\)'
)


def log(line):
    path = os.environ.get("REPRO_GH_LOG")
    if not path:
        return
    fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644)
    try:
        os.write(fd, (line + "\n").encode())
    finally:
        os.close(fd)


def refuse(what):
    log("unknown " + what.replace("\n", " ")[:300])
    print(f"gh (ephor.56 forge): cannot answer {what[:200]}", file=sys.stderr)
    return 1


def block_of(queries):
    """Which block asked: a role question marks the role block."""
    roles = any("author:@me" in q or "involves:@me" in q for q in queries)
    return "role" if roles else "open"


def searches(fields):
    data = {}
    queries = []
    for alias, query, first, after in fields:
        query = json.loads(query)
        queries.append(query)
        start = int(json.loads(after)) if after else 0
        issues = [] if CONTROL.get("empty") else ISSUES
        if CONTROL.get("overlap"):
            issues = [dict(ISSUES[0], days=1)]
        if CONTROL.get("fresh"):
            issues = [dict(ISSUES[3], number=5)]
        found = sorted((each for each in issues if matches(each, query)), key=lambda e: e["days"])
        page = found[start : start + int(first)]
        end = start + len(page)
        data[alias] = {
            "pageInfo": {"hasNextPage": end < len(found), "endCursor": str(end)},
            "nodes": [search_node(each) for each in page],
        }
    block = block_of(queries)
    # This fixture's scenarios disable comment reads, so search is the final
    # subprocess in each fetch. Rendezvous before delaying the named finisher;
    # the parent verifies the completion log as well as the resulting feed.
    if CONTROL.get("last") == block:
        other = Path(os.environ["REPEATED_CONTROL"] + "." + ("role" if block == "open" else "open"))
        deadline = time.monotonic() + 10
        while not other.exists():
            if time.monotonic() > deadline:
                raise Unknown("sibling search did not complete")
            time.sleep(0.01)
        time.sleep(HOLD_SECONDS)
    Path(os.environ["REPEATED_CONTROL"] + "." + block).touch()
    if block in CONTROL.get("fail", []):
        log(f"failed {block}")
        raise Failed(f"HTTP 500: the forge failed the {block} block's search (ephor.56 fixture)")
    log(f"search {block} {' | '.join(queries)}")
    return {"data": data}


def graphql(args):
    query = None
    variables = {}
    index = 0
    while index < len(args):
        if args[index] in ("-f", "-F") and index + 1 < len(args):
            key, _, value = args[index + 1].partition("=")
            if key == "query":
                query = value
            else:
                variables[key] = value
            index += 2
        else:
            index += 1
    if query is None:
        return refuse("graphql with no query: " + " ".join(args))

    fields = FIELD.findall(query)
    if fields:
        try:
            answer = searches(fields)
        except Unknown as unknown:
            return refuse(str(unknown))
        except Failed as failed:
            print(f"gh: {failed}", file=sys.stderr)
            return 1
    elif "issue(number:$number)" in query and "number" in variables:
        found = next((e for e in ISSUES if e["number"] == int(variables["number"])), None)
        comments = {"nodes": comment_nodes(found) if found else []}
        answer = {"data": {"repository": {"issue": {"comments": comments}}}}
        log(f"comments {variables['number']}")
    else:
        return refuse("query " + query)
    print(json.dumps(answer))
    return 0


def main(args):
    if args[:1] == ["--version"]:
        print("gh version 2.45.0 (ephor.56 forge)")
        return 0
    if args[:2] == ["api", "user"]:
        print(ME)
        log("login")
        return 0
    if args[:2] == ["api", "graphql"]:
        return graphql(args[2:])
    return refuse(" ".join(args))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
