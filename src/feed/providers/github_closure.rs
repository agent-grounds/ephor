//! §FS-001-forge-interface.1, Closure, answered by GitHub through `gh`: one
//! GraphQL request for the item and the open pull requests from each head
//! branch asked about, and a second only for the heads of closing pull
//! requests the first one named and nobody asked about yet.
//!
//! A response carrying `errors` is a failure in the forge's words, never a
//! partial answer (§FS-001-forge-interface.6), and a connection that says it
//! has a next page is reported as cut short rather than read as everything.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

use super::github::{gh_command, graphql_error, graphql_string};
use crate::feed::provider::run_json;
use crate::forge::closure::{Closure, ClosureError, Listing, Pull, State, Subject};

/// The most one connection is asked for. A list longer than this is reported
/// as cut short; nothing here pages, because nothing a retirement asks about
/// should come anywhere near it.
const FIRST: u32 = 100;

/// What is read of every pull request named.
const PULL: &str = "number state mergedAt headRefName headRepositoryOwner{login}";

/// Ask GitHub whether `repo#number` is over, and what is open from `heads`
/// and from every head of a pull request that closes it
/// (§FS-001-forge-interface.1).
pub(crate) fn closure(
    host: Option<&str>,
    repo: &str,
    number: u64,
    heads: &[String],
    timeout: Duration,
) -> Result<Closure, ClosureError> {
    let (owner, name) = repo
        .split_once('/')
        .ok_or_else(|| ClosureError::Failed(format!("'{repo}' is not an owner/name")))?;
    let subject = format!(
        "subject:issueOrPullRequest(number:{number}){{__typename \
... on Issue{{state closedAt closedByPullRequestsReferences(first:{FIRST},includeClosedPrs:true)\
{{pageInfo{{hasNextPage}} nodes{{{PULL}}}}}}} \
... on PullRequest{{{PULL} closedAt}}}} "
    );
    let data = ask(host, owner, name, &subject, heads, timeout)?;
    let node = data
        .pointer("/repository/subject")
        .filter(|node| !node.is_null())
        .ok_or_else(|| ClosureError::Failed(format!("{repo}#{number} was not found")))?;

    let mut closure = match node.get("__typename").and_then(Value::as_str) {
        Some("Issue") => Closure {
            subject: Subject::Issue,
            state: state(node)?,
            closed_at: text(node, "closedAt"),
            head: None,
            closing: listing(node.get("closedByPullRequestsReferences"))?,
            open: BTreeMap::new(),
        },
        Some("PullRequest") => Closure {
            subject: Subject::PullRequest,
            state: state(node)?,
            closed_at: text(node, "closedAt"),
            head: Some(pull(node)?),
            closing: Listing::whole(Vec::new()),
            open: BTreeMap::new(),
        },
        _ => return Err(ClosureError::Neither),
    };
    opened(&data, heads, &mut closure.open)?;

    // The heads the forge itself named are checked too: a pull request that
    // closes the issue may come from a branch nothing here recorded.
    let mut more: Vec<String> = closure
        .closing
        .pulls
        .iter()
        .chain(closure.head.iter())
        .map(|pull| pull.branch.clone())
        .filter(|branch| !branch.is_empty() && !closure.open.contains_key(branch))
        .collect();
    more.sort();
    more.dedup();
    if !more.is_empty() {
        let data = ask(host, owner, name, "", &more, timeout)?;
        opened(&data, &more, &mut closure.open)?;
    }
    Ok(closure)
}

/// One request: `subject` (possibly empty) and one aliased open-pull-request
/// list per head.
fn ask(
    host: Option<&str>,
    owner: &str,
    name: &str,
    subject: &str,
    heads: &[String],
    timeout: Duration,
) -> Result<Value, ClosureError> {
    let mut fields = subject.to_string();
    for (at, head) in heads.iter().enumerate() {
        fields.push_str(&format!(
            "h{at}:pullRequests(headRefName:{head},states:[OPEN],first:{FIRST})\
{{pageInfo{{hasNextPage}} nodes{{{PULL}}}}} ",
            head = graphql_string(head),
        ));
    }
    let query = format!(
        "{{repository(owner:{owner},name:{name}){{{fields}}}}}",
        owner = graphql_string(owner),
        name = graphql_string(name),
    );
    let mut command = gh_command(host);
    command.args(["api", "graphql", "-f", &format!("query={query}")]);
    let response =
        run_json(command, timeout, false).map_err(|error| ClosureError::Failed(error.0))?;
    if let Some(message) = graphql_error(&response) {
        return Err(ClosureError::Failed(message));
    }
    response
        .get("data")
        .filter(|data| !data.is_null())
        .cloned()
        .ok_or_else(|| ClosureError::Failed("the forge answered no data".to_string()))
}

/// The open-pull-request lists of `heads`, read from their aliases.
fn opened(
    data: &Value,
    heads: &[String],
    into: &mut BTreeMap<String, Listing>,
) -> Result<(), ClosureError> {
    for (at, head) in heads.iter().enumerate() {
        let found = listing(data.pointer(&format!("/repository/h{at}")))?;
        into.insert(head.clone(), found);
    }
    Ok(())
}

fn listing(connection: Option<&Value>) -> Result<Listing, ClosureError> {
    let connection = connection
        .filter(|value| !value.is_null())
        .ok_or_else(|| ClosureError::Failed("the forge answered no list".to_string()))?;
    let pulls = connection
        .get("nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(pull)
        .collect::<Result<Vec<_>, _>>()?;
    let more = connection
        .pointer("/pageInfo/hasNextPage")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    Ok(Listing {
        pulls,
        complete: !more,
    })
}

fn pull(node: &Value) -> Result<Pull, ClosureError> {
    Ok(Pull {
        number: node
            .get("number")
            .and_then(Value::as_u64)
            .ok_or_else(|| ClosureError::Failed("a pull request came back unnumbered".into()))?,
        state: state(node)?,
        merged_at: text(node, "mergedAt"),
        branch: text(node, "headRefName").unwrap_or_default(),
        owner: node
            .pointer("/headRepositoryOwner/login")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn state(node: &Value) -> Result<State, ClosureError> {
    match node.get("state").and_then(Value::as_str) {
        Some("OPEN") => Ok(State::Open),
        Some("CLOSED") => Ok(State::Closed),
        Some("MERGED") => Ok(State::Merged),
        other => Err(ClosureError::Failed(format!(
            "the forge reported a state this does not know: {other:?}"
        ))),
    }
}

fn text(node: &Value, field: &str) -> Option<String> {
    node.get(field).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_connection_with_a_next_page_is_cut_short_and_one_without_is_whole() {
        let cut = listing(Some(&json!({
            "pageInfo": { "hasNextPage": true },
            "nodes": [{ "number": 7, "state": "OPEN", "headRefName": "fix/x",
                        "headRepositoryOwner": { "login": "someone" } }]
        })))
        .unwrap();
        assert!(!cut.complete);
        assert_eq!(cut.pulls[0].owner.as_deref(), Some("someone"));
        let whole = listing(Some(
            &json!({ "pageInfo": { "hasNextPage": false }, "nodes": [] }),
        ))
        .unwrap();
        assert!(whole.complete);
        // A connection that does not say whether it has more is not whole.
        assert!(!listing(Some(&json!({ "nodes": [] }))).unwrap().complete);
        assert!(listing(Some(&Value::Null)).is_err());
    }
}
