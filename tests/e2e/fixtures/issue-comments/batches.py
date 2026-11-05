"""Extend #252's original forge with repository identity and failure cases.

The original cost fixture is kept in forge.py. This adapter answers both the
old per-issue reads and alias batches, so a red assertion measures the defect
rather than a stub refusing today's request syntax.
"""
import json
import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import forge

fixture = json.loads(Path(os.environ["BATCH_FIXTURE"]).read_text())
issues = fixture["issues"]
query = next((a[6:] for a in sys.argv if a.startswith("query=")), "")
variables = dict(a.split("=", 1) for a in sys.argv if "=" in a and not a.startswith("query="))
assert os.environ.get("GH_HOST") == fixture["host"], "lost configured host"


def record(kind, keys):
    with open(os.environ["BATCH_LOG"], "a") as stream:
        stream.write(json.dumps({"kind": kind, "keys": keys, "query": query}) + "\n")


def key(issue):
    return f"{issue['repo']}#{issue['number']}"


def comments(issue):
    # The search deliberately carries only a count. Its answer cannot supply
    # the payload that this independent comment endpoint holds.
    return [{
        "id": f"IC_{key(issue)}_{i}", "author": {"login": "repro" if i % 2 == 0 else "someone"},
        "body": f"{key(issue)} comment {i}", "createdAt": "2001-01-01T01:02:03Z",
        "reactions": {"nodes": [{"content": "THUMBS_UP", "user": {"login": "repro"}},
                                 {"content": "THUMBS_UP", "user": {"login": "someone"}}]}
    } for i in range(max(0, issue["count"] - 30), issue["count"])]


fields = forge.FIELD.findall(query)
if fields:
    data = {}
    for alias, literal, first, after in fields:
        terms = json.loads(literal).split()
        repos = [t[5:] for t in terms if t.startswith("repo:")]
        base_query = " ".join(t for t in terms if not t.startswith("repo:"))
        selected = [i for i in issues if (not repos or i["repo"] in repos)
                    and forge.matches(i, base_query)]
        start = int(json.loads(after)) if after else 0
        page = selected[start:start + int(first)]
        nodes = []
        for issue in page:
            node = forge.search_node(issue)
            node["repository"]["nameWithOwner"] = issue["repo"]
            node["url"] = f"https://{fixture['host']}/{issue['repo']}/issues/{issue['number']}"
            node["comments"] = {"totalCount": issue["count"]}
            nodes.append(node)
        data[alias] = {"nodes": nodes, "pageInfo": {
            "hasNextPage": start + len(page) < len(selected), "endCursor": str(start + len(page))}}
    record("search", [json.loads(f[1]) for f in fields])
    print(json.dumps({"data": data}))
    sys.exit(0)

# Both literal and variable arguments are legal. Repository grouping and
# aliases are free choices; only canonical keys and bounded work are asserted.
def arg(text, name):
    value = re.search(r"\b" + name + r"\s*:\s*(\$\w+|\d+|\"(?:[^\"\\]|\\.)*\")", text).group(1)
    return variables[value[1:]] if value.startswith("$") else json.loads(value)


data = {}
reads = []
spans = list(re.finditer(r"(?:(\w+)\s*:\s*)?repository\s*\(([^)]*)\)", query))
paths = []
for index, span in enumerate(spans):
    repo = f"{arg(span[2], 'owner')}/{arg(span[2], 'name')}"
    end = spans[index + 1].start() if index + 1 < len(spans) else len(query)
    holder = data.setdefault(span[1] or "repository", {})
    for match in re.finditer(r"(?:(\w+)\s*:\s*)?issue\s*\(([^)]*)\)", query[span.end():end]):
        number = int(arg(match[2], "number"))
        canonical = f"{repo}#{number}"
        issue = next(i for i in issues if key(i) == canonical)
        alias = match[1] or "issue"
        holder[alias] = {"comments": {"nodes": comments(issue)}}
        reads.append(canonical)
        paths.append((holder, alias, [span[1] or "repository", alias]))
assert reads, "unknown comment query: " + query
assert re.search(r"comments\s*\(\s*last\s*:\s*30\s*\)", query), query
assert re.search(r"reactions\s*\(\s*first\s*:\s*50\s*\)", query), query
for field in ["id", "body", "createdAt", "author", "login", "content", "user"]:
    assert re.search(r"\b" + field + r"\b", query), "missing comment field: " + field
record("comments", reads)
errors = []
mode = fixture.get("failure", "")
if mode and any(k == "demo/javelin#1" for k in reads):
    if mode == "invalid-json":
        print("{broken")
        sys.exit(0)
    if mode == "whole-exit":
        print("forge unavailable", file=sys.stderr)
        sys.exit(1)
    for canonical, (holder, alias, path) in zip(reads, paths):
        if canonical == "demo/javelin#1":
            if mode == "missing":
                del holder[alias]
            else:
                holder[alias] = None
            errors.append({"message": "unreadable issue", "path": path})
print(json.dumps({"data": data, "errors": errors}))
sys.exit(1 if errors and mode == "partial-exit" else 0)
