"""A `gh` that answers GraphQL from a forge model, for §FS-005-dispatch.36.1.

`work retire` asks the forge whether a matter is over and whether any pull
request is still open from a branch of it (§FS-001-forge-interface.1). The
question is one GraphQL request whose exact text is the implementation's to
write, so this stand-in does not match it by rote: it parses the query it is
handed and answers each field from the model in $CLOSURE_FORGE, the way GitHub
would. A query that asks for a field the model cannot answer gets GraphQL's own
"doesn't exist" error back, which is what a misspelt field gets from GitHub;
extend `RESOLVERS` rather than the tests when an implementation reads more.

The model, a JSON file:

  login               the reader, for `gh api user`
  repos.<owner/name>  issues.<n>: title state closedAt updatedAt author closedBy[<pr>]
                      pulls.<n>:  title state mergedAt closedAt headRefName headOwner
                                  updatedAt closes[<n>]
  cut_short           head branch names whose open-pull-request list says
                      hasNextPage, as a forge does when it has more to give
  closure_errors      a message: every query that is not a feed search is
                      answered with `errors[]` carrying it
  closure_fails       a message: every query that is not a feed search makes
                      `gh` exit 1 with it on stderr

Feed searches (`search(query: ...)`) are answered from the same issues, so the
refresh that puts a matter in the feed reads the forge the retirement reads.
Every call is appended to $CLOSURE_FORGE.calls as one JSON line.
"""

import json
import os
import re
import shlex
import sys
from datetime import datetime, timezone

MODEL_PATH = os.environ["CLOSURE_FORGE"]
with open(MODEL_PATH) as handle:
    MODEL = json.load(handle)
NOW = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def log(kind, detail):
    with open(MODEL_PATH + ".calls", "a") as handle:
        handle.write(json.dumps({"kind": kind, "detail": detail}) + "\n")


# --- the query language, as much of it as a read uses -----------------------

TOKEN = re.compile(
    r'\s+|,|#[^\n]*|(?P<punct>\.\.\.|[{}():!$=\[\]@])|(?P<string>"(?:\\.|[^"\\])*")'
    r"|(?P<number>-?\d+(?:\.\d+)?)|(?P<name>[_A-Za-z][_0-9A-Za-z]*)"
)


def tokenize(text):
    tokens, at = [], 0
    while at < len(text):
        match = TOKEN.match(text, at)
        if not match:
            raise SyntaxError("cannot read the query at %r" % text[at : at + 20])
        at = match.end()
        for kind in ("punct", "string", "number", "name"):
            if match.group(kind) is not None:
                tokens.append((kind, match.group(kind)))
    return tokens


class Parser:
    def __init__(self, text, variables):
        self.tokens = tokenize(text)
        self.at = 0
        self.variables = variables

    def peek(self, value=None):
        if self.at >= len(self.tokens):
            return None
        token = self.tokens[self.at]
        return token if value is None or token[1] == value else None

    def take(self, value=None):
        token = self.peek()
        if token is None or (value is not None and token[1] != value):
            raise SyntaxError("expected %r, found %r" % (value, token))
        self.at += 1
        return token

    def document(self):
        if self.peek("query"):
            self.take()
            if self.peek() and self.peek()[0] == "name":
                self.take()
            if self.peek("("):
                self.take("(")
                while not self.peek(")"):
                    self.take()  # variable definitions: their values come from -f/-F
                self.take(")")
        return self.selection_set()

    def selection_set(self):
        self.take("{")
        fields = []
        while not self.peek("}"):
            if self.peek("..."):
                self.take("...")
                self.take("on")
                kind = self.take()[1]
                fields.append({"fragment": kind, "selection": self.selection_set()})
                continue
            name = self.take()[1]
            alias = name
            if self.peek(":"):
                self.take(":")
                name = self.take()[1]
            arguments = {}
            if self.peek("("):
                self.take("(")
                while not self.peek(")"):
                    key = self.take()[1]
                    self.take(":")
                    arguments[key] = self.value()
                self.take(")")
            selection = self.selection_set() if self.peek("{") else None
            fields.append({"alias": alias, "name": name, "args": arguments, "selection": selection})
        self.take("}")
        return fields

    def value(self):
        kind, text = self.take()
        if text == "$":
            return self.variables.get(self.take()[1])
        if kind == "string":
            return json.loads(text)
        if kind == "number":
            return float(text) if "." in text else int(text)
        if text == "[":
            items = []
            while not self.peek("]"):
                items.append(self.value())
            self.take("]")
            return items
        if text == "{":
            items = {}
            while not self.peek("}"):
                key = self.take()[1]
                self.take(":")
                items[key] = self.value()
            self.take("}")
            return items
        return {"true": True, "false": False, "null": None}.get(text, text)


# --- the forge ---------------------------------------------------------------


class Missing(Exception):
    pass


def repo_of(name):
    found = MODEL.get("repos", {}).get(name)
    if found is None:
        raise Missing("Could not resolve to a Repository with the name '%s'." % name)
    return name, found


def issue_node(repo, number):
    issue = repo[1].get("issues", {}).get(str(number))
    if issue is None:
        return None
    return ("Issue", dict(issue, number=number, _repo=repo))


def pull_node(repo, number):
    pull = repo[1].get("pulls", {}).get(str(number))
    if pull is None:
        return None
    return ("PullRequest", dict(pull, number=number, _repo=repo))


def every_pull(repo):
    return [pull_node(repo, int(n)) for n in sorted(repo[1].get("pulls", {}), key=int)]


def connection(nodes, more=False):
    return ("Connection", {"nodes": nodes, "more": more})


def search(args):
    terms = shlex.split(args.get("query", ""))
    if "is:pr" in terms:
        return connection([])
    repos = [t[5:] for t in terms if t.startswith("repo:")]
    found = []
    for name, repo in sorted(MODEL.get("repos", {}).items()):
        if repos and name not in repos:
            continue
        for number in sorted(repo.get("issues", {}), key=int):
            node = issue_node((name, repo), int(number))
            if "state:open" in terms and node[1]["state"] != "OPEN":
                continue
            if "author:@me" in terms and node[1].get("author", MODEL.get("login")) != MODEL.get("login"):
                continue
            found.append(node)
    return connection(found[: int(args.get("first", 100))])


def root(field, args):
    if field == "repository":
        return ("Repository", repo_of("%s/%s" % (args["owner"], args["name"])))
    if field == "search":
        return search(args)
    if field == "viewer":
        return ("User", {"login": MODEL.get("login", "tester")})
    raise Missing(None)


def repository(obj, field, args):
    name, repo = obj
    if field == "nameWithOwner":
        return name
    if field == "name":
        return name.split("/")[1]
    if field == "owner":
        return ("User", {"login": name.split("/")[0]})
    if field == "issue":
        return issue_node(obj, args["number"])
    if field == "pullRequest":
        return pull_node(obj, args["number"])
    if field == "issueOrPullRequest":
        return issue_node(obj, args["number"]) or pull_node(obj, args["number"])
    if field == "pullRequests":
        states = args.get("states")
        states = [states] if isinstance(states, str) else states
        head = args.get("headRefName")
        nodes = [
            pull
            for pull in every_pull(obj)
            if (head is None or pull[1]["headRefName"] == head)
            and (not states or pull[1]["state"] in states)
        ]
        return connection(nodes, more=head in MODEL.get("cut_short", []))
    raise Missing(None)


def issue(obj, field, args):
    if field in ("closedByPullRequestsReferences",):
        nodes = [pull_node(obj["_repo"], number) for number in obj.get("closedBy", [])]
        if not args.get("includeClosedPrs", False):
            nodes = [node for node in nodes if node[1]["state"] == "OPEN"]
        return connection(nodes)
    return common(obj, field, "Issue")


def pull(obj, field, args):
    if field == "merged":
        return obj["state"] == "MERGED"
    if field == "headRepositoryOwner":
        return ("User", {"login": obj.get("headOwner", obj["_repo"][0].split("/")[0])})
    if field == "headRepository":
        owner = obj.get("headOwner", obj["_repo"][0].split("/")[0])
        name = "%s/%s" % (owner, obj["_repo"][0].split("/")[1])
        return ("HeadRepository", {"nameWithOwner": name, "name": name.split("/")[1], "owner": owner})
    if field == "isCrossRepository":
        return obj.get("headOwner", obj["_repo"][0].split("/")[0]) != obj["_repo"][0].split("/")[0]
    if field == "baseRefName":
        return obj.get("baseRefName", "main")
    if field == "closingIssuesReferences":
        return connection([issue_node(obj["_repo"], number) for number in obj.get("closes", [])])
    return common(obj, field, "PullRequest")


def common(obj, field, kind):
    if field == "repository":
        return ("Repository", obj["_repo"])
    if field == "url":
        noun = "issues" if kind == "Issue" else "pull"
        return "https://github.com/%s/%s/%s" % (obj["_repo"][0], noun, obj["number"])
    if field == "author":
        return ("User", {"login": obj.get("author", MODEL.get("login", "tester"))})
    if field == "updatedAt":
        return obj.get("updatedAt", NOW)
    if field in ("assignees", "labels", "blockedBy"):
        return connection([])
    if field == "comments":
        return ("Connection", {"nodes": [], "more": False})
    if field in ("number", "title", "state", "closedAt", "mergedAt", "headRefName"):
        return obj.get(field)
    raise Missing(None)


def connection_field(obj, field, args):
    if field == "nodes":
        return obj["nodes"]
    if field == "edges":
        return [("Edge", {"node": node}) for node in obj["nodes"]]
    if field == "totalCount":
        return len(obj["nodes"])
    if field == "pageInfo":
        return ("PageInfo", {"hasNextPage": obj["more"], "endCursor": "next" if obj["more"] else None,
                             "hasPreviousPage": False, "startCursor": None})
    raise Missing(None)


def plain(obj, field, args):
    if field in obj:
        value = obj[field]
        return ("User", {"login": value}) if field == "owner" else value
    raise Missing(None)


RESOLVERS = {
    "Repository": repository,
    "Issue": issue,
    "PullRequest": pull,
    "Connection": connection_field,
    "PageInfo": plain,
    "User": plain,
    "Edge": plain,
    "HeadRepository": plain,
}


def resolve(kind, obj, fields, errors, path):
    out = {}
    for field in fields:
        if "fragment" in field:
            if field["fragment"] == kind:
                out.update(resolve(kind, obj, field["selection"], errors, path))
            continue
        name = field["name"]
        try:
            if name == "__typename":
                value = kind
            elif kind == "Query":
                value = root(name, field["args"])
            else:
                value = RESOLVERS[kind](obj, name, field["args"])
        except Missing as missing:
            message = missing.args[0] or "Field '%s' doesn't exist on type '%s'" % (name, kind)
            errors.append({"message": message, "path": path + [field["alias"]]})
            out[field["alias"]] = None
            continue
        out[field["alias"]] = shape(value, field["selection"], errors, path + [field["alias"]])
    return out


def shape(value, selection, errors, path):
    if isinstance(value, list):
        return [shape(item, selection, errors, path) for item in value]
    if isinstance(value, tuple):
        if selection is None:
            raise SyntaxError("a field of type %s needs a selection" % value[0])
        return resolve(value[0], value[1], selection, errors, path)
    return value


def graphql(args):
    variables, query = {}, None
    at = 0
    while at < len(args):
        flag = args[at]
        if flag in ("-f", "--raw-field", "-F", "--field") and at + 1 < len(args):
            key, _, text = args[at + 1].partition("=")
            if key == "query":
                query = text
            elif flag in ("-F", "--field"):
                variables[key] = json.loads(text) if re.fullmatch(r"-?\d+|true|false|null", text) else text
            else:
                variables[key] = text
            at += 2
            continue
        at += 1
    if query is None:
        print("gh: this forge reads the query from -f query=", file=sys.stderr)
        sys.exit(1)
    searching = "search(" in query.replace(" ", "")
    log("search" if searching else "read", query)
    if not searching:
        if MODEL.get("closure_fails"):
            print(MODEL["closure_fails"], file=sys.stderr)
            sys.exit(1)
        if MODEL.get("closure_errors"):
            print(json.dumps({"data": None, "errors": [{"message": MODEL["closure_errors"]}]}))
            return
    errors = []
    data = resolve("Query", None, Parser(query, variables).document(), errors, [])
    answer = {"data": data}
    if errors:
        answer["errors"] = errors
    print(json.dumps(answer))


def main():
    args = sys.argv[1:]
    if args[:2] == ["api", "user"]:
        log("login", "")
        print(MODEL.get("login", "tester"))
        return
    if args[:2] == ["api", "graphql"]:
        graphql(args[2:])
        return
    log("unknown", " ".join(args))
    print("gh: this forge answers GraphQL only, not %r" % " ".join(args), file=sys.stderr)
    sys.exit(1)


main()
