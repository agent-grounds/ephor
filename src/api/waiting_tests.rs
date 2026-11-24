//! The first screen's reading, held to §FS-019-waiting without a session:
//! the board's half is fed a walk by hand, the feed's half a list of matters.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use serde_json::json;

use super::{listed, names_reader, reading, unprocessed};
use crate::api::read::{Asked, BoardReading, Running};
use crate::feed::model::{Item, ItemKind, ItemRole};
use crate::work::recipe::WorkConfig;
use crate::work::runtime::watch::{BoardTicket, Doing, Operation, PersonWait, RunIdentity};

fn matter(id: &str, title: &str, minute: u32) -> Item {
    Item {
        id: id.to_string(),
        project: "widget".to_string(),
        source: "forge".to_string(),
        kind: ItemKind::Issue,
        role: None,
        title: title.to_string(),
        url: Some(format!("https://forge.example/{id}")),
        state: Some("open".to_string()),
        needs_response: false,
        updated_at: Utc.with_ymd_and_hms(2026, 11, 20, 10, minute, 0).unwrap(),
        raw: json!({}),
    }
}

fn ticket(plan_id: &str, id: &str, state: &str, doing: Doing, heading: &str) -> BoardTicket {
    BoardTicket {
        plan_id: plan_id.to_string(),
        ticket: id.to_string(),
        state: Some(state.to_string()),
        doing,
        item: Some(format!("forge:{plan_id}")),
        title: "The plan's own title".to_string(),
        ticket_title: heading.to_string(),
        plan: PathBuf::from(format!("/w/{plan_id}/panta/{plan_id}.md")),
    }
}

fn operation(project: &str, root: &str, live: bool, tickets: Vec<BoardTicket>) -> Operation {
    Operation {
        project: project.to_string(),
        root: PathBuf::from(root),
        live,
        dashboard: None,
        quiet: None,
        identity: None,
        stop: None,
        tickets,
        done: 0,
        cancelled: 0,
        machine_unread: None,
        plans: Vec::new(),
    }
}

fn ask(person: &str, item: Option<Item>) -> Asked {
    Asked {
        ask: PersonWait {
            project: "widget".to_string(),
            root: PathBuf::from("/w/widget-9/panta"),
            plan_id: "widget-9".to_string(),
            ticket: "approve-1".to_string(),
            ticket_title: "Approve the plan".to_string(),
            state: "plan-approval".to_string(),
            waiting_on: person.to_string(),
            item: item.as_ref().map(|item| item.id.clone()),
            plan: PathBuf::from("/w/widget-9/panta/widget-9.md"),
        },
        item,
    }
}

fn read(board: BoardReading) -> crate::api::views::Waiting {
    reading(
        board,
        &|item, _, _| item.map(str::to_string),
        &WorkConfig::default(),
    )
}

/// A poll is the reader's only where the person it declares is the reader:
/// whoever the work is for, or the reader's own role on the matter
/// (§FS-019-waiting.1.1). Anyone or anything else waits on the world.
#[test]
fn a_poll_names_the_reader_only_by_the_readers_own_name_or_role() {
    let mut mine = matter("forge:widget/1", "Mine", 0);
    mine.role = Some(ItemRole::Author);
    let mut reviewing = matter("forge:widget/2", "Reviewing", 0);
    reviewing.role = Some(ItemRole::Reviewer);
    let theirs = matter("forge:widget/3", "Theirs", 0);

    assert!(names_reader("you", None));
    assert!(names_reader("Owner", None));
    assert!(names_reader("author", Some(&mine)));
    assert!(!names_reader("author", Some(&reviewing)));
    assert!(!names_reader("author", Some(&theirs)));
    assert!(!names_reader("author", None));
    assert!(names_reader("reviewer", Some(&reviewing)));
    assert!(!names_reader("reviewer", Some(&mine)));
    assert!(!names_reader("the forge", Some(&mine)));
    assert!(!names_reader("a release tag", None));
}

/// A gate row is called by its ticket's own heading, not by the plan's
/// title every ticket of the plan shares, and falls back on the plan's only
/// where the ticket has none (§FS-019-waiting.1.1). Gates are found under a
/// root no run holds, polls on the reader sit in the same section marked not
/// movable, and the rows keep the enumeration's order.
#[test]
fn gates_and_polls_on_the_reader_are_one_section_in_the_walks_order() {
    let mut authored = matter("forge:widget-9", "Widen the retry window", 0);
    authored.role = Some(ItemRole::Author);
    let board = BoardReading {
        running: vec![
            Running {
                op: operation(
                    "widget",
                    "/w/widget-7/panta",
                    false,
                    vec![
                        ticket(
                            "widget-7",
                            "ask-1",
                            "needs-human",
                            Doing::Waiting,
                            "Approve it",
                        ),
                        ticket("widget-7", "ask-2", "needs-human", Doing::Waiting, " "),
                    ],
                ),
                item: None,
                plan: None,
            },
            Running {
                op: operation(
                    "alpha",
                    "/w/alpha-1/panta",
                    true,
                    vec![ticket(
                        "alpha-1",
                        "fix-1",
                        "implement",
                        Doing::Running,
                        "Fix it",
                    )],
                ),
                item: None,
                plan: None,
            },
        ],
        asks: vec![
            ask("author", Some(authored.clone())),
            ask("the forge", None),
        ],
        refusal: None,
        matters: BTreeMap::from([(authored.id.clone(), authored)]),
    };

    let waiting = read(board);
    let rows: Vec<(&str, &str, &str)> = waiting
        .on_you
        .iter()
        .map(|row| (row.kind, row.ticket.as_str(), row.title.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("gate", "widget-7.ask-1", "Approve it"),
            ("gate", "widget-7.ask-2", "The plan's own title"),
            ("poll", "widget-9.approve-1", "Approve the plan"),
        ]
    );
    let gate = &waiting.on_you[0];
    assert!(gate.movable);
    assert_eq!(gate.state, "needs-human");
    assert_eq!(gate.review.as_deref(), Some("forge:widget-7"));
    let poll = &waiting.on_you[2];
    assert!(!poll.movable);
    assert_eq!(poll.waiting_on.as_deref(), Some("author"));
    assert_eq!(poll.matter.as_deref(), Some("Widen the retry window"));
    assert_eq!(poll.review, None);

    // Only the live root is running, holding only what it runs.
    assert_eq!(waiting.running.len(), 1);
    assert_eq!(waiting.running[0].project, "alpha");
    assert_eq!(waiting.running[0].tickets[0].title, "Fix it");
}

/// The age of a run is the start it published and nothing else: a run that
/// published none carries none, and the field is absent from the view, not
/// null (§FS-019-waiting.1.3, §FS-019-waiting.6).
#[test]
fn a_running_row_is_as_old_as_the_run_says_and_no_older() {
    let mut named = operation("alpha", "/w/alpha-1/panta", true, Vec::new());
    named.identity = Some(RunIdentity {
        id: Some("3f9a2c".to_string()),
        control_url: None,
        started_at: Some("2026-11-20T09:00:00Z".to_string()),
    });
    let unnamed = operation("beta", "/w/beta-1/panta", true, Vec::new());
    let board = BoardReading {
        running: vec![
            Running {
                op: named,
                item: None,
                plan: None,
            },
            Running {
                op: unnamed,
                item: None,
                plan: None,
            },
        ],
        asks: Vec::new(),
        refusal: None,
        matters: BTreeMap::new(),
    };

    let waiting = read(board);
    assert_eq!(
        waiting.running[0].started_at,
        Some(Utc.with_ymd_and_hms(2026, 11, 20, 9, 0, 0).unwrap())
    );
    assert_eq!(waiting.running[0].run.as_deref(), Some("3f9a2c"));
    assert!(waiting.running[0].attach.is_some());
    let json = serde_json::to_value(&waiting.running[1]).unwrap();
    for absent in ["started_at", "run", "attach", "item", "matter", "plan"] {
        assert!(json.get(absent).is_none(), "{absent} in {json}");
    }
}

/// Where the runtime cannot run anything, its sentence is the answer and
/// there is nothing on the reader or running (§FS-019-waiting.2).
#[test]
fn a_refusal_is_carried_in_place_of_the_walk() {
    let waiting = read(BoardReading {
        running: Vec::new(),
        asks: Vec::new(),
        refusal: Some("Nothing can run here".to_string()),
        matters: BTreeMap::new(),
    });
    assert_eq!(waiting.refusal.as_deref(), Some("Nothing can run here"));
    assert!(waiting.on_you.is_empty() && waiting.running.is_empty());
}

/// New is every matter the feed shows that is not finished, not in the
/// ledger, not a ticket of the runtime's own store and not held — newest
/// first; a held one is listed apart with the label that holds it
/// (§FS-019-waiting.1.2).
#[test]
fn new_is_what_nothing_has_been_done_about_and_held_is_apart() {
    let old = matter("forge:widget/1", "Old", 1);
    let newest = matter("forge:widget/2", "Newest", 9);
    let mut finished = matter("forge:widget/3", "Finished", 5);
    finished.state = Some("closed".to_string());
    let ledgered = matter("forge:widget/4", "Ledgered", 5);
    let mut own = matter("own:widget-4.fix-1", "Own task", 5);
    own.source = crate::work::runtime::RUNNER.to_string();
    own.kind = ItemKind::Task;
    let mut discussed = matter("forge:widget/5", "Discussed", 5);
    discussed.raw = json!({ "labels": ["bug", "human-discussion"] });
    let mut labelled = matter("forge:widget/6", "Labelled", 3);
    labelled.raw = json!({ "labels": ["bug"] });

    let holds = BTreeSet::from(["human-discussion".to_string()]);
    let (new, held) = unprocessed(
        [old, newest, finished, ledgered, own, discussed, labelled]
            .into_iter()
            .map(|item| (item, true)),
        &|item| item.id == "forge:widget/4",
        &holds,
    );
    let titles: Vec<&str> = new.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles, vec!["Newest", "Labelled", "Old"]);
    assert_eq!(held.len(), 1);
    assert_eq!(held[0].title, "Discussed");
    assert_eq!(held[0].label, "human-discussion");

    // A site whose selectors exclude no label holds nothing.
    let mut discussed = matter("forge:widget/5", "Discussed", 5);
    discussed.raw = json!({ "labels": ["human-discussion"] });
    let (new, held) = unprocessed([(discussed, false)], &|_| false, &BTreeSet::new());
    assert_eq!(new.len(), 1);
    assert!(held.is_empty());
}

/// What the reader decided today is counted from the processing marks and the
/// dismissals made since local midnight, and from nothing older
/// (§FS-019-waiting.2).
#[test]
fn decided_today_counts_marks_and_dismissals_since_local_midnight() {
    use crate::work::ledger::{Dismissal, Ledger, Processed, Snapshot};
    let now = chrono::Local::now();
    let midnight = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_local_timezone(chrono::Local)
        .earliest()
        .unwrap()
        .with_timezone(&Utc);
    let today = midnight + chrono::Duration::seconds(1);
    let yesterday = midnight - chrono::Duration::seconds(1);
    let mark = |at| Processed {
        entry: "implement".to_string(),
        kind: "agent".to_string(),
        matter: "issue".to_string(),
        project: "widget".to_string(),
        title: "t".to_string(),
        at,
        queued: false,
        says: None,
        root: None,
        run: None,
    };
    let item = matter("forge:widget/9", "Rename the lock", 1);
    let dismissal = |at| Dismissal {
        project: "widget".to_string(),
        title: "t".to_string(),
        reason: "a duplicate".to_string(),
        at,
        snapshot: Snapshot::of(&item),
    };
    let mut ledger = Ledger::default();
    ledger.processed.insert("a".to_string(), mark(today));
    ledger.processed.insert("b".to_string(), mark(today));
    ledger.processed.insert("c".to_string(), mark(yesterday));
    ledger.dismissed.insert("d".to_string(), dismissal(today));
    ledger
        .dismissed
        .insert("e".to_string(), dismissal(yesterday));
    let decided = super::decided(&ledger, now);
    assert_eq!((decided.laid, decided.dismissed), (2, 1));
}

/// Three gates under two roots and a poll, as the enumeration finds them.
fn three_gates_and_a_poll() -> crate::api::views::Waiting {
    let board = BoardReading {
        running: vec![
            Running {
                op: operation(
                    "widget",
                    "/w/widget-7/panta",
                    false,
                    vec![
                        ticket("widget-7", "ask-1", "needs-human", Doing::Waiting, "First"),
                        ticket("widget-7", "ask-2", "needs-human", Doing::Waiting, "Second"),
                    ],
                ),
                item: None,
                plan: None,
            },
            Running {
                op: operation(
                    "widget",
                    "/w/widget-8/panta",
                    false,
                    vec![ticket(
                        "widget-8",
                        "ask-1",
                        "review",
                        Doing::Waiting,
                        "Third",
                    )],
                ),
                item: None,
                plan: None,
            },
        ],
        asks: vec![ask("you", None)],
        refusal: None,
        matters: BTreeMap::new(),
    };
    read(board)
}

fn gate(ticket: &str, state: &str, since: Option<&str>, holds: u64) -> Listed {
    serde_json::from_value(json!({
        "ticket": ticket, "state": state, "title": format!("Brief of {ticket}"),
        "since": since, "holds": holds
    }))
    .unwrap()
}

use crate::work::runtime::gate::Listed;

/// The runtime's listing is asked once per root that holds a gate row, and
/// lends a row its brief's title, its start and what it holds only where it
/// names that ticket in that state. It adds no row, and a root it cannot list
/// keeps the enumeration's rows as they were. Known starts lead, oldest first;
/// an unknown start is not zero, and sorts behind them with every row the
/// listing said nothing about (§FS-019-waiting.1.1.1).
#[test]
fn a_listing_names_each_gate_and_puts_the_oldest_first() {
    let mut waiting = three_gates_and_a_poll();
    let asked = std::cell::RefCell::new(Vec::new());
    listed(&mut waiting.on_you, &|root| {
        asked.borrow_mut().push(root.to_path_buf());
        match root.to_str() {
            Some("/w/widget-7/panta") => Some(vec![
                gate("widget-7.ask-1", "needs-human", None, 0),
                gate(
                    "widget-7.ask-2",
                    "needs-human",
                    Some("2026-11-22T10:00:00Z"),
                    6,
                ),
                // Not found by the enumeration, so never a row.
                gate(
                    "widget-7.ask-9",
                    "needs-human",
                    Some("2026-11-01T10:00:00Z"),
                    1,
                ),
            ]),
            Some("/w/widget-8/panta") => Some(vec![
                // Listed in another state: the row is left as it was.
                gate(
                    "widget-8.ask-1",
                    "elsewhere",
                    Some("2026-11-20T10:00:00Z"),
                    2,
                ),
            ]),
            _ => None,
        }
    });
    assert_eq!(
        *asked.borrow(),
        vec![
            PathBuf::from("/w/widget-7/panta"),
            PathBuf::from("/w/widget-8/panta")
        ],
        "one listing per root that holds a gate, and none for the poll's"
    );
    let rows: Vec<(&str, &str)> = waiting
        .on_you
        .iter()
        .map(|row| (row.ticket.as_str(), row.title.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            ("widget-7.ask-2", "Brief of widget-7.ask-2"),
            ("widget-7.ask-1", "Brief of widget-7.ask-1"),
            ("widget-8.ask-1", "Third"),
            ("widget-9.approve-1", "Approve the plan"),
        ]
    );
    let now = Utc.with_ymd_and_hms(2026, 11, 24, 12, 0, 0).unwrap();
    let says: Vec<String> = waiting.on_you.iter().map(|row| row.says_at(now)).collect();
    assert_eq!(
        says,
        vec![
            "needs-human · waited 2d · holds 6 tasks",
            "needs-human · waited unknown · holds nothing",
            "review",
            "waiting on you · not yours to move here",
        ]
    );
    let view = serde_json::to_value(&waiting.on_you).unwrap();
    assert_eq!(view[0]["since"], "2026-11-22T10:00:00Z");
    assert_eq!(view[0]["holds"], 6);
    assert!(view[1]["since"].is_null() && view[1].get("since").is_some());
    assert!(view[2].get("since").is_none() && view[2].get("holds").is_none());
}

/// A runtime that lists nothing leaves every row exactly as the enumeration
/// gave it, in the enumeration's order (§FS-019-waiting.1.1.1).
#[test]
fn without_a_listing_the_rows_stay_as_the_enumeration_gave_them() {
    let before = serde_json::to_value(&three_gates_and_a_poll().on_you).unwrap();
    let mut waiting = three_gates_and_a_poll();
    listed(&mut waiting.on_you, &|_| None);
    assert_eq!(serde_json::to_value(&waiting.on_you).unwrap(), before);
    assert_eq!(waiting.on_you[0].says(), "needs-human");
}
