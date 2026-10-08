//! The forge protocol's published schema, held to the types that speak it
//! (§FS-001-forge-interface.1, §FS-001-forge-interface.2, §REQ-002-parity.4).
//!
//! `assets/ephor-forge.schema.json` is written by hand, for the person writing
//! an implementation, and nothing used to make it follow the types: `notices`
//! joined [`Capabilities`] and never the schema, and neither did a pull
//! request's `review` or a message's `react`. A gateway author who reads
//! `ephor schema forge` is reading the only description of the protocol they
//! get, so a field missing there is a field they cannot know to send.
//!
//! What is held here. Every capability is declared, and every declared
//! capability is one ephor reads. Every key the out-of-process transport puts
//! on stdin is declared as part of the request. Every subcommand it runs has a
//! response definition, and every response definition is a subcommand it
//! runs. And a report populated in every field, serialized by the types,
//! validates against its definition, carries no key the schema does not
//! declare, and leaves no top-level property of the definition unsaid.
//!
//! The schema stays hand-written: these are tests of it, not a generator for
//! it, so the prose written for gateway authors stays where they read it.

use std::collections::BTreeSet;

use chrono::{TimeZone, Utc};
use serde_json::{json, Value};

use super::{
    Attachment, Capabilities, Conversation, Issue, IssueDependency, Message, Notice, PullRequest,
    Reaction, Reason, Review, Role, SubjectKind, Thread,
};
use crate::feed::gate::{Gate, RepoGate};

const FORGE_SCHEMA: &str = include_str!("../../assets/ephor-forge.schema.json");
const ANSWER_SCHEMA: &str = include_str!("../../assets/ephor-answer.schema.json");
const ANSWER_ID: &str = "https://github.com/agent-grounds/ephor/schemas/answer/v1";

fn forge() -> Value {
    serde_json::from_str(FORGE_SCHEMA).expect("the forge schema parses")
}

fn answer() -> Value {
    serde_json::from_str(ANSWER_SCHEMA).expect("the answer schema parses")
}

/// One definition of the forge schema, or a failure naming it.
fn definition(name: &str) -> Value {
    forge()["$defs"]
        .get(name)
        .cloned()
        .unwrap_or_else(|| panic!("the forge schema declares no `$defs/{name}`"))
}

fn declared(schema: &Value) -> BTreeSet<String> {
    schema["properties"]
        .as_object()
        .map(|properties| properties.keys().cloned().collect())
        .unwrap_or_default()
}

fn keys(value: &Value) -> BTreeSet<String> {
    value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

/// Every place `value` carries a key that `schema` does not declare, as a
/// path from the top. References are followed into either document; a schema
/// that says nothing about an object's properties, or that allows any, is the
/// implementation's own and is not looked into.
fn undeclared(value: &Value, schema: &Value) -> Vec<String> {
    let mut found = Vec::new();
    walk(value, schema, &forge(), "", &mut found);
    found
}

fn walk(value: &Value, schema: &Value, document: &Value, at: &str, found: &mut Vec<String>) {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let (target, pointer) = reference.split_once('#').unwrap_or((reference, ""));
        let document = match target {
            "" => document.clone(),
            ANSWER_ID => answer(),
            other => panic!("{at}: a reference into a document this test does not know: {other}"),
        };
        let resolved = document
            .pointer(pointer)
            .cloned()
            .unwrap_or_else(|| panic!("{at}: `{reference}` resolves to nothing"));
        return walk(value, &resolved, &document, at, found);
    }
    for alternatives in ["oneOf", "anyOf"] {
        if let Some(branches) = schema.get(alternatives).and_then(Value::as_array) {
            let mut best: Option<Vec<String>> = None;
            for branch in branches {
                let mut here = Vec::new();
                walk(value, branch, document, at, &mut here);
                if best.as_ref().is_none_or(|best| here.len() < best.len()) {
                    best = Some(here);
                }
            }
            found.extend(best.unwrap_or_default());
            return;
        }
    }
    match value {
        Value::Object(object) => {
            if schema.get("additionalProperties") == Some(&Value::Bool(true)) {
                return;
            }
            let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
                return;
            };
            for (key, field) in object {
                let path = format!("{at}.{key}");
                match properties.get(key) {
                    Some(property) => walk(field, property, document, &path, found),
                    None => found.push(path),
                }
            }
        }
        Value::Array(items) => {
            if let Some(item) = schema.get("items") {
                for (index, element) in items.iter().enumerate() {
                    walk(element, item, document, &format!("{at}[{index}]"), found);
                }
            }
        }
        _ => {}
    }
}

/// Validate `value` against one forge definition, with the envelope's
/// document registered so its shared definitions resolve without a network.
fn violations(value: &Value, name: &str) -> Vec<String> {
    let mut root = forge();
    let _ = definition(name);
    root["$ref"] = json!(format!("#/$defs/{name}"));
    let validator = jsonschema::options()
        .with_resource(
            ANSWER_ID,
            jsonschema::Resource::from_contents(answer()).expect("the answer schema is a resource"),
        )
        .build(&root)
        .expect("the forge schema compiles");
    validator
        .iter_errors(value)
        .map(|err| format!("{name}{}: {err}", err.instance_path))
        .collect()
}

/// Typed, wire and default outcomes share the published transport contract
/// (§FS-001-forge-interface.2).
#[test]
fn reply_outcomes_validate_and_legacy_empty_object_decodes_as_acceptance() {
    use super::ReplyOutcome;
    for (wire, expected) in [
        (json!({}), ReplyOutcome::Accepted),
        (json!({"status":"accepted"}), ReplyOutcome::Accepted),
        (
            json!({"status":"unknown","note":"check channel"}),
            ReplyOutcome::Unknown {
                note: "check channel".into(),
            },
        ),
    ] {
        assert!(violations(&wire, "reply_response").is_empty());
        assert_eq!(ReplyOutcome::from_wire(wire).unwrap(), expected);
        assert!(violations(&serde_json::to_value(&expected).unwrap(), "reply_response").is_empty());
    }
    assert_eq!(ReplyOutcome::default(), ReplyOutcome::Accepted);
    for wire in [
        json!({"status":"queued"}),
        json!({"status":"unknown"}),
        json!([]),
    ] {
        assert!(!violations(&wire, "reply_response").is_empty());
        assert!(ReplyOutcome::from_wire(wire).is_err());
    }
}

/// Hold one fully populated report to its definition: it validates, it
/// carries nothing undeclared, and nothing declared at its top is left out.
fn holds(value: &Value, name: &str) -> Vec<String> {
    let schema = definition(name);
    let mut problems = violations(value, name);
    problems.extend(
        undeclared(value, &schema)
            .into_iter()
            .map(|path| format!("{name}{path}: serialized, and not declared")),
    );
    problems.extend(
        declared(&schema)
            .difference(&keys(value))
            .map(|key| format!("{name}.{key}: declared, and never serialized")),
    );
    problems
}

fn at() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 1, 9, 12, 0).unwrap()
}

/// A message with every field the type has, set.
fn message() -> Message {
    Message {
        author: "dana".into(),
        text: "Can we ship on Friday?".into(),
        when: Some(at()),
        reactions: vec![Reaction {
            emoji: "+1".into(),
            users: vec!["eli".into()],
            mine: true,
        }],
        react: json!({ "subject": "c1" }),
        task: json!({ "id": "t1", "state": "open" }),
        mine: false,
        attachments: Some(vec![Attachment {
            name: "IMG_2041.jpg".into(),
            media_type: Some("image/jpeg".into()),
            size: Some(1843211),
            id: json!("att:dana:2041"),
        }]),
    }
}

fn thread() -> Thread {
    Thread {
        messages: vec![message()],
        reply: json!({ "kind": "pr", "id": "7" }),
    }
}

fn pull_request() -> PullRequest {
    PullRequest {
        id: "acme/app/7".into(),
        repo: "acme/app".into(),
        number: "7".into(),
        title: "Widen the retry window".into(),
        url: Some("https://forge.example/acme/app/pull/7".into()),
        branch: Some("widen-retry".into()),
        draft: Some(false),
        updated_at: at(),
        role: Role::Reviewer,
        state: Some("open".into()),
        reasons: vec![Reason::ReviewRequested, Reason::InThread],
        cited: true,
        review: Some(Review::ChangesRequested),
        threads: vec![thread()],
        gate: Some(Gate {
            repos: vec![RepoGate {
                repo: "acme/app".into(),
                passed: 3,
                failed: 1,
                running: 2,
            }],
            blocked: true,
            blockers: vec!["a required check failed".into()],
        }),
        assignees: Some(vec!["you".into()]),
        labels: Some(vec!["bug".into()]),
    }
}

fn issue() -> Issue {
    Issue {
        key: "APP-12".into(),
        title: "Retries reset per attempt".into(),
        status: Some("In Progress".into()),
        url: Some("https://tracker.example/APP-12".into()),
        updated_at: at(),
        role: Role::Author,
        assigned: Some(true),
        assignees: Some(vec!["you".into()]),
        labels: Some(vec!["bug".into()]),
        blocked_by: Some(vec![IssueDependency {
            key: "APP-11".into(),
            title: "Measure the retry window".into(),
            status: Some("Open".into()),
            url: Some("https://tracker.example/APP-11".into()),
        }]),
        messages: vec![message()],
    }
}

fn notice() -> Notice {
    Notice {
        id: "n-1".into(),
        title: "You were mentioned".into(),
        url: Some("https://forge.example/acme/app/issues/3".into()),
        reason: "mentioned".into(),
        subject: SubjectKind::Issue,
        repo: Some("acme/app".into()),
        number: Some("3".into()),
        updated_at: at(),
        read: true,
    }
}

/// A conversation from a chat room, every field set, with threads in the
/// shape a pull request's take.
fn conversation() -> Conversation {
    Conversation {
        id: "whatsapp/acme#120363@g.us".into(),
        title: "Widget rollout: can we ship Friday?".into(),
        url: Some("https://chat.example/acme/120363".into()),
        updated_at: at(),
        room: Some("whatsapp/acme#120363@g.us".into()),
        reasons: vec![Reason::Mentioned],
        threads: vec![Thread {
            messages: vec![message()],
            reply: json!({ "chat": "120363@g.us" }),
        }],
    }
}

fn wire<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("a forge type serializes")
}

/// Every capability ephor reads is one an implementation can find in the
/// schema, and every one the schema offers is one ephor reads.
#[test]
fn every_capability_is_declared_and_every_declared_one_is_read() {
    let fields = keys(&wire(&Capabilities::default()));
    let schema = declared(&definition("capabilities_response"));
    let unpublished: Vec<_> = fields.difference(&schema).collect();
    let unread: Vec<_> = schema.difference(&fields).collect();
    assert!(
        unpublished.is_empty() && unread.is_empty(),
        "`$defs/capabilities_response` and `Capabilities` disagree.\n  \
         read by ephor, not in the schema: {unpublished:?}\n  \
         in the schema, never read: {unread:?}"
    );
}

/// A pull request with every field set says nothing the schema does not, and
/// the schema promises nothing it never says.
#[test]
fn a_full_pull_request_is_exactly_what_the_schema_declares() {
    let problems = holds(&wire(&pull_request()), "pull_request");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_full_issue_is_exactly_what_the_schema_declares() {
    let problems = holds(&wire(&issue()), "issue");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_full_notice_is_exactly_what_the_schema_declares() {
    let problems = holds(&wire(&notice()), "notice");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The `messages` row a gateway author reads about is the one ephor reads
/// (§FS-001-forge-interface.1).
#[test]
fn a_full_conversation_is_exactly_what_the_schema_declares() {
    let problems = holds(&wire(&conversation()), "conversation");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The machinery itself: a key nobody declared is found, at the top and
/// through a reference into the envelope's document. Without this, the tests
/// above would pass just as well on a walker that looked at nothing.
#[test]
fn a_key_the_schema_does_not_declare_is_caught() {
    let schema = definition("pull_request");
    let mut value = json!({
        "id": "acme/app/7", "repo": "acme/app", "number": "7", "title": "t",
        "surprise": true,
        "threads": [{ "messages": [{ "author": "a", "text": "b", "aside": 1 }] }]
    });
    let found = undeclared(&value, &schema);
    assert!(found.contains(&".surprise".to_string()), "{found:?}");
    assert!(
        found.contains(&".threads[0].messages[0].aside".to_string()),
        "followed into the envelope's message: {found:?}"
    );
    let object = value.as_object_mut().expect("an object");
    object.remove("surprise");
    object.remove("threads");
    assert!(undeclared(&value, &schema).is_empty());
}

/// A file on a message the schema refuses is one the decode refuses, so a
/// gateway author who validates against the schema is never refused for less
/// at the interface; and a key neither knows is ignored by both, so the entry
/// can grow (§FS-001-forge-interface.1).
#[test]
fn the_schema_and_the_type_refuse_the_same_files() {
    let conversation = |file: &Value| {
        json!({
            "id": "c", "title": "t", "updated_at": "2026-10-01T09:12:00Z",
            "threads": [{ "messages": [{ "author": "dana", "text": "", "attachments": [file] }] }]
        })
    };
    for file in [
        json!({}),
        json!({ "size": 3 }),
        json!({ "name": 7 }),
        json!({ "name": "a", "size": -1 }),
        json!({ "name": "a", "size": 1.5 }),
        json!({ "name": "a", "media_type": 7 }),
    ] {
        let wire = conversation(&file);
        assert!(
            !violations(&wire, "conversation").is_empty(),
            "the schema takes {file}"
        );
        assert!(
            serde_json::from_value::<Conversation>(wire).is_err(),
            "the type takes {file}"
        );
    }
    let grown = conversation(&json!({ "name": "a", "thumbnail": "x" }));
    assert!(violations(&grown, "conversation").is_empty());
    let read: Conversation = serde_json::from_value(grown).expect("an unknown key is ignored");
    let files = read.threads[0].messages[0].attachments.as_ref().unwrap();
    assert_eq!(files[0].name, "a");
}

/// What the out-of-process transport actually sends and runs, recorded by an
/// implementation that answers every subcommand with the empty case.
#[cfg(unix)]
mod transport {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use serde_json::{json, Value};

    use super::{declared, definition, forge, keys};
    use crate::feed::gate::Scope;
    use crate::forge::external::{ExternalForge, SUBCOMMANDS};
    use crate::forge::{Forge, Request};

    const RECORDER: &str = r#"#!/bin/sh
here=$(dirname "$0")
echo "$1" >> "$here/calls"
cat > "$here/$1.json"
case "$1" in
  pull-requests | issues | notices | messages | failures) printf '[]' ;;
  *) printf '{}' ;;
esac
"#;

    /// Drive every move the transport has through a recorder, and return what
    /// it ran, in order, with what each was handed on stdin.
    fn recorded(dir: &Path) -> Vec<(String, Value)> {
        use std::os::unix::fs::PermissionsExt;
        let recorder = dir.join("recorder");
        fs::write(&recorder, RECORDER).expect("write the recorder");
        fs::set_permissions(&recorder, fs::Permissions::from_mode(0o755))
            .expect("make the recorder runnable");
        let forge = ExternalForge::new("recorder", Some(recorder.to_string_lossy().into()));
        let request = Request {
            config: json!({ "token_file": "~/.config/acme/token" }),
            project: "widget".into(),
            tickets: vec!["APP-12".into()],
            user: Some("you".into()),
            timeout_seconds: 10,
        };
        let target = json!({ "subject": "c1" });
        forge.capabilities().expect("capabilities");
        forge.pull_requests(&request).expect("pull-requests");
        forge.issues(&request).expect("issues");
        forge.notices(&request).expect("notices");
        forge.messages(&request).expect("messages");
        forge.failures(&request, "acme/app", "7").expect("failures");
        forge
            .restart(&request, "acme/app", "7", Scope::Failed)
            .expect("restart");
        forge.react(&request, &target, "+1").expect("react");
        forge.resolve_task(&request, &target).expect("resolve-task");
        forge.reply(&request, &target, "on it").expect("reply");
        forge.settle(&request, "k-B").expect("settle");
        fs::read_to_string(dir.join("calls"))
            .expect("the recorder was run")
            .lines()
            .map(|call| {
                let sent = fs::read_to_string(dir.join(format!("{call}.json")))
                    .expect("the recorder kept what it was sent");
                let sent = serde_json::from_str(&sent).expect("stdin was JSON");
                (call.to_string(), sent)
            })
            .collect()
    }

    /// Every key ephor puts on an implementation's stdin is one the schema
    /// tells its author about.
    #[test]
    fn every_key_on_stdin_is_declared_in_the_request() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let schema = declared(&definition("request"));
        let mut unpublished: Vec<String> = Vec::new();
        for (call, sent) in recorded(dir.path()) {
            for key in keys(&sent).difference(&schema) {
                unpublished.push(format!("{call}: {key}"));
            }
        }
        assert!(
            unpublished.is_empty(),
            "sent on stdin, and not declared in `$defs/request`: {unpublished:?}"
        );
    }

    /// Every subcommand the transport runs has an answer an author can look
    /// up, and every answer the schema describes is to a subcommand it runs.
    /// The transport's own list is read, rather than the moves a test thought
    /// to drive, so a subcommand added to the transport is held here without
    /// anyone having to remember it; the transport refuses, in a debug build,
    /// to run one the list does not name.
    #[test]
    fn every_subcommand_has_a_response_and_every_response_a_subcommand() {
        let run: BTreeSet<String> = SUBCOMMANDS
            .iter()
            .map(|call| call.replace('-', "_"))
            .collect();
        let described: BTreeSet<String> = forge()["$defs"]
            .as_object()
            .expect("the schema has definitions")
            .keys()
            .filter_map(|name| name.strip_suffix("_response").map(String::from))
            .collect();
        let unanswered: Vec<_> = run.difference(&described).collect();
        let unasked: Vec<_> = described.difference(&run).collect();
        assert!(
            unanswered.is_empty() && unasked.is_empty(),
            "run, with no `<subcommand>_response` in the schema: {unanswered:?}\n\
             described, and never run: {unasked:?}"
        );
    }
}
