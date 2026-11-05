"""A forge for ephor.55: one repository whose closed history has a chosen size.

It holds a few standing issues and $REPRO_CLOSED closed ones, all with comments,
answers the three things `ephor refresh` asks of github-issues, and appends one
line per call to $REPRO_GH_LOG: `<start_ns> <end_ns> <kind> <detail>`.

  kinds   login | search | comments <n> | batch <n,n,...> | unknown <what>

It applies a search's qualifiers rather than answering by rote, so a fix that
asks differently (a window on closed answers, `state:open` on a role question)
gets the answer GitHub would give it. `involves:@me` includes issues the reader
opened, as GitHub documents and as the real counts in the write-up show. A
qualifier or a query it does not know is logged as `unknown`, and run.sh exits 2
on that rather than reading a refused call as a saved one.
"""

import json
import os
import re
import shlex
import sys
import time
from datetime import datetime, timedelta, timezone

ME = "repro"
THEM = "someone"
REPO = "demo/javelin"
NOW = datetime.now(timezone.utc)
STARTED = time.time_ns()


def stamp(days):
    return (NOW - timedelta(days=days)).strftime("%Y-%m-%dT%H:%M:%SZ")


def issue(number, state, author, days, talk):
    return {"number": number, "state": state, "author": author, "days": days, "talk": talk}


def history():
    closed = int(os.environ["REPRO_CLOSED"])
    issues = [
        # The quiet open backlog `updated_within_days: 0` exists to keep. All three
        # are older than any window a site would pick.
        issue(1, "OPEN", ME, 200, [ME, THEM]),  # somebody else has the last word
        issue(2, "OPEN", THEM, 210, [THEM]),  # found by the open question alone
        issue(3, "OPEN", THEM, 220, []),  # no comments: no read is owed
        # Closed yesterday with somebody else's last word: §FS-003-feed-categories.2.
        issue(4, "CLOSED", ME, 1, [ME, THEM]),
    ]
    for index in range(closed):
        # Nine in ten opened by the reader, the rest only joined, as on the site.
        author = THEM if index % 10 == 9 else ME
        issues.append(issue(100 + index, "CLOSED", author, 300 + index, [THEM, ME]))
    return issues


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
        # The unchanged search selection asks for a count, not message nodes.
        # Supplying unsolicited payloads would let a missing hydration pass.
        "comments": {"totalCount": len(found["talk"])},
        "blockedBy": {"nodes": []},
    }


class Unknown(Exception):
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


def answer_searches(fields):
    data = {}
    asked = []
    for alias, query, first, after in fields:
        query = json.loads(query)
        start = int(json.loads(after)) if after else 0
        found = sorted(
            (each for each in history() if matches(each, query)), key=lambda each: each["days"]
        )
        page = found[start : start + int(first)]
        end = start + len(page)
        data[alias] = {
            "pageInfo": {"hasNextPage": end < len(found), "endCursor": str(end)},
            "nodes": [search_node(each) for each in page],
        }
        asked.append(alias)
    return {"data": data}, ",".join(asked)


def by_number():
    return {each["number"]: each for each in history()}


def answer_batch(query):
    """Comment reads batched by alias, the remedy the report names: either
    `r:repository(...){i1:issue(number:1){...} ...}` or one aliased repository
    per issue. Anything else falls through to `unknown`."""
    issues = by_number()
    data = {}
    numbers = []
    spans = list(re.finditer(r"(?:(\w+)\s*:\s*)?repository\(", query))
    for index, span in enumerate(spans):
        end = spans[index + 1].start() if index + 1 < len(spans) else len(query)
        reads = re.findall(r"(?:(\w+)\s*:\s*)?issue\(number:\s*(\d+)\)", query[span.start() : end])
        if not reads:
            return None, ""
        holder = data.setdefault(span.group(1) or "repository", {})
        for alias, number in reads:
            found = issues.get(int(number))
            holder[alias or "issue"] = (
                {"comments": {"nodes": comment_nodes(found)}} if found else None
            )
            numbers.append(number)
    return ({"data": data}, ",".join(numbers)) if numbers else (None, "")


def log(kind, detail=""):
    path = os.environ.get("REPRO_GH_LOG")
    if not path:
        return
    line = f"{STARTED} {time.time_ns()} {kind} {detail}\n".encode()
    fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644)
    try:
        os.write(fd, line)
    finally:
        os.close(fd)


def refuse(what):
    log("unknown", what.replace("\n", " ")[:300])
    print(f"gh (ephor.55 forge): cannot answer {what[:200]}", file=sys.stderr)
    return 1


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
            answer, asked = answer_searches(fields)
        except Unknown as unknown:
            return refuse(str(unknown))
        log("search", asked)
    elif "issue(number:$number)" in query and "number" in variables:
        found = by_number().get(int(variables["number"]))
        comments = {"nodes": comment_nodes(found) if found else []}
        answer = {"data": {"repository": {"issue": {"comments": comments}}}}
        # A real comment read takes the network's time. This is small on purpose:
        # it makes two reads in flight at once visible in the log, and no more.
        time.sleep(0.01)
        log("comments", variables["number"])
    else:
        answer, numbers = answer_batch(query)
        if answer is None:
            return refuse("query " + query)
        log("batch", numbers)
    print(json.dumps(answer))
    return 0


def main(args):
    if args[:1] == ["--version"]:
        print("gh version 2.45.0 (ephor.55 forge)")
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
