//! A stale draft is shown against what moved since it was drafted, and offers a
//! new draft or the person's own words (§FS-005-dispatch.13.2,
//! §FS-005-dispatch.5). The reading, its JSON and both refusals carry the same
//! review (§FS-011-command-line.4, §FS-011-command-line.7). Ported from the
//! issue's reproducer: the reply is aimed at the issue itself, so its target is
//! the same before and after the conversation moves, and only the saved words
//! can tell what changed.

use super::*;
use support::shaped;

const DRAFT: &str = "I accept €100 and can start on Monday.";
const ASKED: &str = "Can you do the fence for €100?";
const EDITED: &str = "Can you do the fence for €150? (edited: the posts went up)";
const RAISED: &str = "Sorry, the posts went up: it is €150 now.";
const TUESDAY: &str = "And Monday does not work, Tuesday?";
const ELSEWHERE: &str = "Yes, €150 is fine, see you Monday.";

/// The reproducer's source: one thread whose reply descriptor is the issue.
fn issue_like() -> MailWorld {
    let world = MailWorld::new(false);
    let mut row = world.get("conversation.json");
    row["threads"][0]["reply"] = json!({"issue": 7});
    world.put("conversation.json", row);
    world.refresh();
    world
}

fn said(id: &str, author: &str, text: &str, when: &str) -> Value {
    json!({"id": id, "author": author, "mine": author == "me", "text": text, "when": when})
}

/// Change the bound thread at its source, and record what the source now says.
fn moved(world: &MailWorld, change: impl FnOnce(&mut Vec<Value>)) {
    let mut row = world.get("conversation.json");
    change(row["threads"][0]["messages"].as_array_mut().unwrap());
    row["updated_at"] = json!("2026-10-07T12:00:00Z");
    world.put("conversation.json", row);
    world.refresh();
}

/// Every surface a stale draft is read on: the draft in `thread --json`, the
/// `thread` reading, the `reply --dry-run --json` refusal and the real one.
struct Seen {
    draft: Value,
    reading: String,
    outcome: Value,
    refusal: String,
}

fn seen(world: &MailWorld) -> Seen {
    let draft = world.thread()["draft"].clone();
    let reading = says(&world.ok(&["thread", ITEM]));
    let dry = world.run(&["reply", ITEM, "--dry-run", "--json"]);
    assert!(
        !dry.status.success(),
        "a stale draft must refuse: {}",
        says(&dry)
    );
    let outcome = shaped("outcome", &dry);
    assert_eq!(outcome["ok"], false, "{outcome}");
    let real = world.reply(None);
    assert!(
        !real.status.success(),
        "a stale draft must refuse: {}",
        says(&real)
    );
    assert!(
        world.requests().is_empty(),
        "a stale draft must reach no forge"
    );
    Seen {
        draft,
        reading,
        outcome,
        refusal: says(&real),
    }
}

fn since(value: &Value) -> Vec<Value> {
    value["since"].as_array().cloned().unwrap_or_default()
}

/// Each entry as what every surface shows of it: its kind, its position and
/// its words.
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

/// The lines the reading prints under `Since the draft:`, up to the blank line
/// that ends them.
fn review(text: &str) -> String {
    let (_, rest) = text
        .split_once("Since the draft:")
        .unwrap_or_else(|| panic!("no review under `Since the draft:` in:\n{text}"));
    rest.lines()
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn redraft_command() -> String {
    format!("ephor work dispatch --item {ITEM} --recipe answer --again")
}

fn typed_command() -> String {
    format!("ephor reply {ITEM} '<words>'")
}

#[test]
fn an_edit_is_named_with_its_words_before_and_after_on_every_surface() {
    let world = issue_like();
    world.drafted(DRAFT);
    moved(&world, |messages| messages[0]["text"] = json!(EDITED));
    let seen = seen(&world);
    let entries = since(&seen.draft);
    assert_eq!(
        listed(&entries),
        [("edited".to_string(), json!(0), json!(EDITED))],
        "one edit and nothing else: {}",
        seen.draft
    );
    assert_eq!(entries[0]["author"], "dana");
    assert_eq!(entries[0]["before"], ASKED);
    assert!(
        entries[0]["at"]
            .as_str()
            .is_some_and(|at| at.starts_with("2026-10-07T09:00")),
        "{}",
        entries[0]
    );
    assert_eq!(
        seen.draft["stale_reason"], "Draft is stale: dana edited [0] after it was drafted",
        "{}",
        seen.draft
    );
    assert_eq!(seen.outcome["says"], seen.draft["stale_reason"]);
    assert_eq!(
        since(&seen.outcome),
        entries,
        "the refusal carries the same review"
    );
    for text in [&seen.reading, &seen.refusal] {
        let review = review(text);
        assert!(review.contains("dana edited [0]"), "{review}");
        assert!(review.contains(&format!("- {ASKED}")), "{review}");
        assert!(review.contains(&format!("+ {EDITED}")), "{review}");
        assert!(!text.contains("missing, reordered or ambiguous"), "{text}");
    }
}

#[test]
fn every_arrival_is_listed_not_only_the_first() {
    let world = issue_like();
    world.drafted(DRAFT);
    moved(&world, |messages| {
        messages.push(said("<c1>", "dana", RAISED, "2026-10-07T09:30:00Z"));
        messages.push(said("<c2>", "dana", TUESDAY, "2026-10-07T09:35:00Z"));
    });
    let seen = seen(&world);
    let entries = since(&seen.draft);
    assert_eq!(
        listed(&entries),
        [
            ("new".to_string(), json!(1), json!(RAISED)),
            ("new".to_string(), json!(2), json!(TUESDAY)),
        ],
        "{}",
        seen.draft
    );
    let reason = seen.draft["stale_reason"].as_str().unwrap();
    assert!(
        reason.starts_with("Draft is stale: dana") && reason.contains(RAISED),
        "{reason}"
    );
    assert!(reason.contains("(and 1 more)"), "{reason}");
    assert_eq!(
        since(&seen.outcome),
        entries,
        "the refusal carries the same review"
    );
    for text in [&seen.reading, &seen.refusal] {
        let review = review(text);
        assert!(review.contains("[1]") && review.contains("[2]"), "{review}");
    }
}

#[test]
fn the_persons_own_reply_from_elsewhere_is_listed_as_theirs() {
    let world = issue_like();
    world.drafted(DRAFT);
    moved(&world, |messages| {
        messages.push(said("<c1>", "me", ELSEWHERE, "2026-10-07T10:00:00Z"))
    });
    let seen = seen(&world);
    let entries = since(&seen.draft);
    assert_eq!(
        listed(&entries),
        [("yours".to_string(), json!(1), json!(ELSEWHERE))],
        "{}",
        seen.draft
    );
    assert_eq!(
        since(&seen.outcome),
        entries,
        "the refusal carries the same review"
    );
}

#[test]
fn a_send_accepted_here_is_listed_once_before_and_after_a_refresh() {
    let world = MailWorld::new(false);
    world.drafted(DRAFT);
    let sent = world.reply(Some(ELSEWHERE));
    assert!(sent.status.success(), "{}", says(&sent));
    for refreshed in [false, true] {
        if refreshed {
            world.refresh();
        }
        let draft = world.thread()["draft"].clone();
        let entries = listed(&since(&draft));
        let position = if refreshed { json!(1) } else { Value::Null };
        assert_eq!(
            entries,
            [("yours".to_string(), position, json!(ELSEWHERE))],
            "listed once, refreshed: {refreshed}: {draft}"
        );
    }
}

#[test]
fn a_window_that_slid_is_new_and_no_longer_shown_never_an_edit() {
    let world = issue_like();
    moved(&world, |messages| {
        messages.push(said(
            "<c1>",
            "dana",
            "The posts are in.",
            "2026-10-07T09:10:00Z",
        ));
        messages.push(said(
            "<c2>",
            "dana",
            "Gate as well?",
            "2026-10-07T09:20:00Z",
        ));
    });
    world.drafted(DRAFT);
    moved(&world, |messages| {
        messages.remove(0);
        messages.push(said("<c3>", "dana", RAISED, "2026-10-07T09:30:00Z"));
    });
    let seen = seen(&world);
    let entries = since(&seen.draft);
    assert_eq!(
        listed(&entries),
        [
            ("no-longer-shown".to_string(), Value::Null, json!(ASKED)),
            ("new".to_string(), json!(2), json!(RAISED)),
        ],
        "{}",
        seen.draft
    );
    let reason = seen.draft["stale_reason"].as_str().unwrap();
    assert!(
        reason.starts_with("Draft is stale:")
            && !reason.contains("missing, reordered or ambiguous"),
        "a slid window lines up: {reason}"
    );
    assert_eq!(
        since(&seen.outcome),
        entries,
        "the refusal carries the same review"
    );
    for text in [&seen.reading, &seen.refusal] {
        let review = review(text);
        assert!(review.contains("no longer shown"), "{review}");
        assert!(
            !review.contains("edited") && !review.contains("deleted"),
            "{review}"
        );
    }
}

#[test]
fn a_thread_that_cannot_be_lined_up_keeps_its_sentence_and_lists_nothing() {
    for case in ["reordered", "same author and time", "a missing time"] {
        let world = issue_like();
        moved(&world, |messages| match case {
            // The source reports no time for the message the draft answers.
            "a missing time" => {
                messages[0].as_object_mut().unwrap().remove("when");
            }
            "reordered" => messages.push(said(
                "<c1>",
                "dana",
                "The posts are in.",
                "2026-10-07T09:10:00Z",
            )),
            _ => messages.push(said(
                "<c1>",
                "dana",
                "The posts are in.",
                "2026-10-07T09:00:00Z",
            )),
        });
        world.drafted(DRAFT);
        moved(&world, |messages| match case {
            "reordered" => messages.reverse(),
            "a missing time" => messages[0]["text"] = json!(EDITED),
            _ => messages[1]["text"] = json!(RAISED),
        });
        let seen = seen(&world);
        assert_eq!(
            seen.draft["stale_reason"],
            "Draft is stale: bound thread is missing, reordered or ambiguous",
            "{case}: {}",
            seen.draft
        );
        assert!(since(&seen.draft).is_empty(), "{case}: {}", seen.draft);
        assert!(since(&seen.outcome).is_empty(), "{case}: {}", seen.outcome);
        assert!(
            !seen.reading.contains(" edited ["),
            "{case}: {}",
            seen.reading
        );
    }
}

#[test]
fn a_new_draft_is_offered_as_the_reopen_and_supersedes_the_stale_one() {
    let world = issue_like();
    world.drafted(DRAFT);
    moved(&world, |messages| messages[0]["text"] = json!(EDITED));
    let draft = world.thread()["draft"].clone();
    assert_eq!(draft["redraft"]["recipe"], "answer", "{draft}");
    assert_eq!(
        draft["redraft"]["command"],
        redraft_command().as_str(),
        "{draft}"
    );
    assert!(
        draft.get("redraft_refused").is_none_or(Value::is_null),
        "{draft}"
    );
    let reading = says(&world.ok(&["thread", ITEM]));
    assert!(reading.contains(&redraft_command()), "{reading}");
    assert!(reading.contains(&typed_command()), "{reading}");
    assert!(
        reading.contains("answer-1.reply.md"),
        "where the old words stay: {reading}"
    );

    let ledger = || read_json(&world.world.path().join("state/ephor/work.json"));
    let dispatches = |ledger: &Value| {
        ledger["entries"][ITEM]["dispatches"]
            .as_array()
            .unwrap()
            .len()
    };
    assert_eq!(
        dispatches(&ledger()),
        1,
        "looking at a stale draft lays nothing"
    );
    let command = redraft_command();
    let args: Vec<&str> = command.split_whitespace().skip(1).collect();
    world.ok(&args);
    let ledger = ledger();
    assert_eq!(dispatches(&ledger), 2);
    assert_eq!(
        ledger["entries"][ITEM]["dispatches"][1]["ticket"],
        "answer-2"
    );
    let plan = fs::read_to_string(ledger["entries"][ITEM]["plan"].as_str().unwrap()).unwrap();
    let ticket = plan
        .split("### Task answer-2")
        .nth(1)
        .unwrap_or_else(|| panic!("answer-2 appended to the plan:\n{plan}"));
    let changed = ticket
        .lines()
        .find(|line| line.starts_with("Since the previous ticket:"))
        .unwrap_or_else(|| panic!("the reopen says what changed:\n{ticket}"));
    assert!(changed.contains("dana edited [0]"), "{changed}");
    assert!(
        !changed.contains("fence for"),
        "no contact's words outside the dossier's fences: {changed}"
    );
    assert!(
        world.thread()["draft"].is_null(),
        "the new request supersedes the stale draft"
    );
}

#[test]
fn where_the_recipe_no_longer_applies_no_new_draft_is_offered_and_the_reason_is_said() {
    let world = issue_like();
    world.drafted(DRAFT);
    moved(&world, |messages| {
        messages.push(said("<c1>", "me", ELSEWHERE, "2026-10-07T10:00:00Z"))
    });
    let draft = world.thread()["draft"].clone();
    assert!(draft.get("redraft").is_none_or(Value::is_null), "{draft}");
    let reason = draft["redraft_refused"]
        .as_str()
        .unwrap_or_else(|| panic!("the draft says why no new draft is offered: {draft}"));
    assert!(!reason.trim().is_empty(), "{draft}");
    let reading = says(&world.ok(&["thread", ITEM]));
    assert!(reading.contains(reason), "{reading}");
    assert!(!reading.contains("--again"), "{reading}");
    assert!(
        reading.contains(&typed_command()),
        "typing is still offered: {reading}"
    );
}

#[test]
fn the_published_shapes_declare_the_review_and_the_ways_on() {
    let world = MailWorld::new(false);
    let out = world.ok(&["schema", "views"]);
    let schema: Value = serde_json::from_slice(&out.stdout).unwrap();
    let draft = &schema["properties"]["thread"]["properties"]["draft"]["properties"];
    for field in ["since", "redraft", "redraft_refused", "stale_reason"] {
        assert!(
            draft.get(field).is_some(),
            "the draft declares `{field}`: {draft}"
        );
    }
    let outcome = &schema["properties"]["outcome"]["properties"];
    assert!(
        outcome.get("since").is_some(),
        "the outcome declares `since`: {outcome}"
    );
}
