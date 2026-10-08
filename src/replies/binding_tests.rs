//! The review of what moved under a bound draft, and that it explains what
//! freshness refuses rather than deciding it (§FS-005-dispatch.13.2).

use super::*;
use crate::forge::policy::conversation_item;
use crate::forge::{Conversation, Message, Thread};
use crate::replies::storage::tests::{item, sources};
use crate::replies::storage::Accepted;

const ASKED: &str = "Can you do the fence for €100?";
const EDITED: &str = "Can you do the fence for €150?";
const RAISED: &str = "Sorry, the posts went up: it is €150 now.";
const TUESDAY: &str = "And Monday does not work, Tuesday?";
const MINE: &str = "Yes, €150 is fine.";
const SIGNED: &str = "Yes, €150 is fine.\n\n-- \nSent from my phone";

fn said(author: &str, when: Option<&str>, text: &str) -> Value {
    let mut message = json!({"author": author, "text": text, "mine": author == "me"});
    if let Some(when) = when {
        message["when"] = json!(when);
    }
    message
}

fn at(time: &str) -> Option<String> {
    Some(format!("2026-10-07T{time}:00Z"))
}

/// One thread aimed at the issue itself, so its reply target never moves.
fn row(messages: Vec<Value>) -> Item {
    let mut row = item();
    row.raw = json!({"threads": [{"messages": messages, "reply": {"issue": 7}}]});
    row
}

/// The row as a refresh records it: the source's messages written through the
/// policy that fills the cache, whose each one is included where the source
/// said it is the person's (§FS-001-forge-interface.3).
fn recorded(messages: &[(&str, &str, &str)]) -> Item {
    let conversation = Conversation {
        id: "k-B".into(),
        title: "Fence".into(),
        url: None,
        updated_at: chrono::Utc::now(),
        room: None,
        reasons: vec![],
        threads: vec![Thread {
            messages: messages
                .iter()
                .map(|(author, time, text)| Message {
                    author: author.to_string(),
                    text: text.to_string(),
                    when: Some(format!("2026-10-07T{time}:00Z").parse().unwrap()),
                    mine: *author == "me",
                    ..Message::default()
                })
                .collect(),
            reply: json!({"issue": 7}),
        }],
    };
    conversation_item("mail", "demo", &conversation)
}

/// The same row as it was recorded before ownership reached the recorded
/// message: no message says whose it is.
fn unowned(row: &Item) -> Item {
    let mut row = row.clone();
    for message in row.raw["threads"][0]["messages"].as_array_mut().unwrap() {
        message.as_object_mut().unwrap().remove("mine");
    }
    row
}

fn bound(row: &Item) -> Binding {
    Binding::capture(
        row,
        &sources(),
        &Defaults::default(),
        &Record::new(row),
        "draft.reply.md".into(),
    )
    .expect("a shown thread binds")
}

fn review(binding: &Binding, now: &Item, record: &Record) -> Vec<Value> {
    match serde_json::to_value(binding.review(now, record)).unwrap() {
        Value::Array(entries) => entries,
        other => panic!("the review is a list: {other}"),
    }
}

/// Each entry as every surface shows it: its kind, its position and its words.
fn listed(entries: &[Value]) -> Vec<(String, Value, Value)> {
    entries
        .iter()
        .map(|entry| {
            (
                entry["change"].as_str().unwrap_or("").to_string(),
                entry.get("message").cloned().unwrap_or(Value::Null),
                entry["text"].clone(),
            )
        })
        .collect()
}

fn entry(change: &str, message: Option<usize>, text: &str) -> (String, Value, Value) {
    (change.to_string(), json!(message), json!(text))
}

const UNALIGNED: &str = "Draft is stale: bound thread is missing, reordered or ambiguous";

/// A scenario: what the draft saw, the conversation now, the row's saved
/// sends, and what the review must list.
type Moved = (
    &'static str,
    Item,
    Item,
    Record,
    Vec<(String, Value, Value)>,
);

/// The scenarios whose review lists something.
fn moved() -> Vec<Moved> {
    let q = || said("dana", at("09:00").as_deref(), ASKED);
    let a = || said("dana", at("09:30").as_deref(), RAISED);
    let b = || said("dana", at("09:35").as_deref(), TUESDAY);
    let saw = row(vec![q()]);
    let accepted = {
        let mut record = Record::new(&saw);
        record.accepted.insert(
            bound(&saw).key(),
            Accepted {
                generation: 1,
                text: MINE.into(),
                target: Some(json!({"issue": 7})),
            },
        );
        record
    };
    vec![
        (
            "every arrival, the person's own as theirs",
            saw.clone(),
            row(vec![
                q(),
                a(),
                said("me", at("09:40").as_deref(), MINE),
                b(),
            ]),
            Record::new(&saw),
            vec![
                entry("new", Some(1), RAISED),
                entry("yours", Some(2), MINE),
                entry("new", Some(3), TUESDAY),
            ],
        ),
        (
            "an edit",
            row(vec![q(), a()]),
            row(vec![said("dana", at("09:00").as_deref(), EDITED), a()]),
            Record::new(&saw),
            vec![entry("edited", Some(0), EDITED)],
        ),
        (
            "a slid window",
            row(vec![q(), a(), b()]),
            row(vec![
                a(),
                b(),
                said("dana", at("09:50").as_deref(), "Gate too?"),
            ]),
            Record::new(&saw),
            vec![
                entry("no-longer-shown", None, ASKED),
                entry("new", Some(2), "Gate too?"),
            ],
        ),
        (
            "a deletion",
            row(vec![q(), a(), b()]),
            row(vec![q(), b()]),
            Record::new(&saw),
            vec![entry("no-longer-shown", None, RAISED)],
        ),
        (
            "a send accepted here that no refresh has shown",
            saw.clone(),
            saw.clone(),
            accepted.clone(),
            vec![entry("yours", None, MINE)],
        ),
        (
            "that send once a refresh shows it",
            saw.clone(),
            row(vec![q(), said("me", at("10:00").as_deref(), MINE)]),
            accepted.clone(),
            vec![entry("yours", Some(1), MINE)],
        ),
        (
            "that send once a refresh shows it as the forge rewrote it",
            saw.clone(),
            row(vec![q(), said("me", at("10:00").as_deref(), SIGNED)]),
            accepted,
            vec![entry("yours", Some(1), SIGNED)],
        ),
    ]
}

#[test]
fn the_review_lists_every_kind_of_movement() {
    for (case, saw, now, record, expected) in moved() {
        let binding = bound(&saw);
        assert_eq!(listed(&review(&binding, &now, &record)), expected, "{case}");
    }
}

#[test]
fn an_edit_carries_its_author_time_and_words_before() {
    let saw = row(vec![said("dana", at("09:00").as_deref(), ASKED)]);
    let now = row(vec![said("dana", at("09:00").as_deref(), EDITED)]);
    let entries = review(&bound(&saw), &now, &Record::new(&saw));
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0]["author"], "dana");
    assert_eq!(entries[0]["before"], ASKED);
    assert_eq!(entries[0]["text"], EDITED);
    assert!(
        entries[0]["at"]
            .as_str()
            .is_some_and(|at| at.starts_with("2026-10-07T09:00")),
        "{}",
        entries[0]
    );
}

#[test]
fn arrivals_need_no_time_while_the_thread_still_begins_with_what_was_seen() {
    // No time at all, and the empty time a refresh records for one.
    for when in [None, Some("")] {
        let saw = row(vec![said("dana", when, ASKED)]);
        let now = row(vec![said("dana", when, ASKED), said("dana", when, RAISED)]);
        assert_eq!(
            listed(&review(&bound(&saw), &now, &Record::new(&saw))),
            [entry("new", Some(1), RAISED)],
            "{when:?}"
        );
    }
}

#[test]
fn the_persons_own_reply_is_theirs_as_a_refresh_records_it() {
    let saw = recorded(&[("dana", "09:00", ASKED)]);
    let now = recorded(&[("dana", "09:00", ASKED), ("me", "10:00", MINE)]);
    let messages = &now.raw["threads"][0]["messages"];
    assert_eq!(messages[1]["mine"], true, "the person's own is recorded so");
    assert!(
        messages[0].get("mine").is_none(),
        "and nothing is written where it is not theirs: {messages}"
    );
    let binding = bound(&saw);
    assert_eq!(
        listed(&review(&binding, &now, &Record::new(&saw))),
        [entry("yours", Some(1), MINE)]
    );
    let mut record = Record::new(&saw);
    record.accepted.insert(
        binding.key(),
        Accepted {
            generation: 1,
            text: MINE.into(),
            target: Some(json!({"issue": 7})),
        },
    );
    assert_eq!(
        listed(&review(&binding, &saw, &record)),
        [entry("yours", None, MINE)],
        "a send accepted here, before a refresh shows it"
    );
    assert_eq!(
        listed(&review(&binding, &now, &record)),
        [entry("yours", Some(1), MINE)],
        "and once one does, listed once"
    );
}

#[test]
fn whose_a_message_is_never_moves_a_draft() {
    // A thread the person began, bound before ownership was recorded: its
    // baseline says the first message is not theirs, and the source now says
    // it is. Nothing moved.
    let now = recorded(&[
        ("me", "08:00", "Can we talk fence?"),
        ("dana", "09:00", ASKED),
    ]);
    let before = bound(&unowned(&now));
    assert_eq!(before.messages[0]["mine"], false, "{:?}", before.messages);
    let record = Record::new(&now);
    assert_eq!(before.freshness(&now, &record), Ok(()));
    assert!(review(&before, &now, &record).is_empty());
    // The generation key is the one every send accepted before was saved
    // under, whoever captures the thread now.
    let captured = bound(&now);
    assert_eq!(captured.messages[0]["mine"], true, "the baseline holds it");
    assert_eq!(captured.key(), before.key());
    assert_eq!(
        before.key(),
        json!([0, before.messages[0]]).to_string(),
        "the key a pre-upgrade send was accepted under"
    );
    assert_eq!(captured.freshness(&unowned(&now), &record), Ok(()));
    let mut sent = record.clone();
    sent.accepted.insert(
        captured.key(),
        Accepted {
            generation: 1,
            text: MINE.into(),
            target: Some(json!({"issue": 7})),
        },
    );
    assert!(
        before.freshness(&now, &sent).is_err(),
        "a send accepted now still advances a draft bound before"
    );
}

#[test]
fn a_message_is_named_by_the_position_the_reading_prints() {
    let earlier = json!({"messages": [
        said("eli", at("08:00").as_deref(), "Which fence?"),
        said("eli", at("08:10").as_deref(), "The garden one."),
    ], "reply": {"issue": 6}});
    let thread = |messages: Vec<Value>| {
        let mut row = item();
        row.raw =
            json!({"threads": [earlier.clone(), {"messages": messages, "reply": {"issue": 7}}]});
        row
    };
    let saw = thread(vec![said("dana", at("09:00").as_deref(), ASKED)]);
    let now = thread(vec![
        said("dana", at("09:00").as_deref(), ASKED),
        said("dana", at("09:30").as_deref(), RAISED),
    ]);
    let binding = bound(&saw);
    assert_eq!(binding.thread, 1, "the last sendable thread is bound");
    assert_eq!(
        listed(&review(&binding, &now, &Record::new(&saw))),
        [entry("new", Some(3), RAISED)],
        "two messages of the thread before it come first in the reading"
    );
}

#[test]
fn nothing_is_guessed_where_the_messages_cannot_be_lined_up() {
    let cases = [
        (
            "a missing time",
            row(vec![said("dana", None, ASKED)]),
            row(vec![said("dana", None, EDITED)]),
        ),
        (
            "a missing time as a refresh records it",
            row(vec![said("dana", Some(""), ASKED)]),
            row(vec![said("dana", Some(""), EDITED)]),
        ),
        (
            "two messages with one author and time",
            row(vec![
                said("dana", at("09:00").as_deref(), ASKED),
                said("dana", at("09:00").as_deref(), RAISED),
            ]),
            row(vec![
                said("dana", at("09:00").as_deref(), ASKED),
                said("dana", at("09:00").as_deref(), TUESDAY),
            ]),
        ),
        (
            "a changed order",
            row(vec![
                said("dana", at("09:00").as_deref(), ASKED),
                said("dana", at("09:30").as_deref(), RAISED),
            ]),
            row(vec![
                said("dana", at("09:30").as_deref(), RAISED),
                said("dana", at("09:00").as_deref(), ASKED),
            ]),
        ),
    ];
    for (case, saw, now) in cases {
        let binding = bound(&saw);
        let record = Record::new(&saw);
        assert!(review(&binding, &now, &record).is_empty(), "{case}");
        assert_eq!(
            binding.freshness(&now, &record),
            Err(UNALIGNED.to_string()),
            "{case}"
        );
    }
}

#[test]
fn an_edit_and_more_arrivals_name_themselves_in_the_one_line_reason() {
    let saw = row(vec![said("dana", at("09:00").as_deref(), ASKED)]);
    let edited = row(vec![said("dana", at("09:00").as_deref(), EDITED)]);
    assert_eq!(
        bound(&saw).freshness(&edited, &Record::new(&saw)),
        Err("Draft is stale: dana edited [0] after it was drafted".to_string())
    );
    let arrived = row(vec![
        said("dana", at("09:00").as_deref(), ASKED),
        said("dana", at("09:30").as_deref(), RAISED),
        said("dana", at("09:35").as_deref(), TUESDAY),
    ]);
    let reason = bound(&saw)
        .freshness(&arrived, &Record::new(&saw))
        .unwrap_err();
    assert!(
        reason.starts_with("Draft is stale: dana") && reason.contains(RAISED),
        "{reason}"
    );
    assert!(reason.contains("(and 1 more)"), "{reason}");
}

#[test]
fn whatever_the_review_lists_freshness_refuses() {
    for (case, saw, now, record, _) in moved() {
        let binding = bound(&saw);
        assert!(
            !review(&binding, &now, &record).is_empty(),
            "{case}: the review lists what moved"
        );
        assert!(
            binding.freshness(&now, &record).is_err(),
            "{case}: a draft whose review lists anything is refused"
        );
    }
}
