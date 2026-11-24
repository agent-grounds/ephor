//! The Waiting screen over a reading made by hand (§FS-019-waiting).

use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::Utc;
use ratatui::crossterm::event::KeyCode;
use serde_json::json;

use super::{WaitingScreen, NO_REVIEW, NO_RUN_ID};
use crate::api::views;
use crate::feed::model::{Item, ItemKind};
use crate::feed::tui::Action;

fn matter(id: &str, title: &str) -> Item {
    Item {
        id: id.to_string(),
        project: "widget".to_string(),
        source: "forge".to_string(),
        kind: ItemKind::Issue,
        role: None,
        title: title.to_string(),
        url: None,
        state: Some("open".to_string()),
        needs_response: false,
        updated_at: Utc::now(),
        raw: json!({}),
    }
}

fn gate(ticket: &str, review: Option<&str>) -> views::OnYou {
    views::OnYou {
        kind: "gate",
        ticket: ticket.to_string(),
        title: "Approve the fix for the release gate".to_string(),
        state: "human-gate".to_string(),
        project: "widget".to_string(),
        root: PathBuf::from("/w/widget-7/panta"),
        plan: PathBuf::from("/w/widget-7/panta/widget-7.md"),
        item: review.map(str::to_string),
        matter: review.map(|_| "release-minor fails on push".to_string()),
        waiting_on: None,
        movable: true,
        review: review.map(str::to_string),
    }
}

fn poll() -> views::OnYou {
    views::OnYou {
        kind: "poll",
        ticket: "widget-9.approve-1".to_string(),
        title: "Approve the plan".to_string(),
        state: "plan-approval".to_string(),
        project: "widget".to_string(),
        root: PathBuf::from("/w/widget-9/panta"),
        plan: PathBuf::from("/w/widget-9/panta/widget-9.md"),
        item: None,
        matter: None,
        waiting_on: Some("author".to_string()),
        movable: false,
        review: None,
    }
}

fn new_row(id: &str, title: &str) -> views::NewMatter {
    views::NewMatter {
        id: id.to_string(),
        project: "widget".to_string(),
        kind: ItemKind::Issue,
        title: title.to_string(),
        url: None,
        updated_at: Utc::now(),
        unread: true,
        proposal: views::Proposal {
            entry: "fix".to_string(),
            kind: "agent",
            description: "Fix it".to_string(),
            gate: "ready",
            unambiguous: true,
            ..views::Proposal::default()
        },
    }
}

fn running(run: Option<&str>) -> views::RunningRoot {
    views::RunningRoot {
        root: PathBuf::from("/w/widget-3/panta"),
        project: "widget".to_string(),
        item: None,
        matter: Some("Leave a fan-out with a free arm".to_string()),
        plan: Some(PathBuf::from("/w/widget-3/panta/widget-3.md")),
        entry: Some("implement".to_string()),
        branch: Some("fix/issue-3".to_string()),
        tickets: vec![views::RunningTicket {
            ticket: "widget-3.review-1".to_string(),
            title: "Review round 2".to_string(),
            state: Some("review".to_string()),
        }],
        run: run.map(str::to_string),
        started_at: None,
        attach: None,
    }
}

fn reading(
    on_you: Vec<views::OnYou>,
    new: Vec<views::NewMatter>,
    run: Vec<views::RunningRoot>,
) -> views::Waiting {
    views::Waiting {
        refusal: None,
        on_you,
        new,
        held: Vec::new(),
        running: run,
        queued: Vec::new(),
        adopt: None,
        decided: views::Decided::default(),
        overview: views::Overview {
            matters: 143,
            unread: 12,
            refreshed_at: None,
        },
    }
}

/// The three sections in order, each headed by its name and count, with the
/// header counting them and the footer opening on the overview line
/// (§FS-019-waiting.1, §FS-019-waiting.3).
#[test]
fn the_screen_is_three_sections_in_order_under_a_counting_header() {
    let screen = WaitingScreen::new(
        reading(
            vec![gate("widget-7.ask-1", Some("forge:widget/7")), poll()],
            vec![new_row(
                "forge:widget/31",
                "transition refuses a valid Prior",
            )],
            vec![running(Some("3f9a2c"))],
        ),
        BTreeMap::new(),
    );
    assert_eq!(
        screen.title(),
        " ephor — waiting   2 on you · 1 new · 1 running · never refreshed"
    );
    assert!(
        screen
            .footer()
            .starts_with(" overview: 143 matters · 12 changed since you looked"),
        "{}",
        screen.footer()
    );
    let text = screen.text();
    let at = |needle: &str| {
        text.find(needle)
            .unwrap_or_else(|| panic!("{needle} missing from:\n{text}"))
    };
    assert!(at("WAITING ON YOU · 2") < at("NEW · 1"));
    assert!(at("NEW · 1") < at("RUNNING · 1"));
    assert!(at("Approve the fix for the release gate") < at("NEW · 1"));
    assert!(text.contains("human-gate"), "{text}");
    assert!(
        text.contains("waiting on author · not yours to move here"),
        "{text}"
    );
    assert!(text.contains("transition refuses a valid Prior"), "{text}");
    assert!(text.contains("widget-3.review-1 [review]"), "{text}");
    // The running row names the entry that laid the work and its branch
    // (§FS-019-waiting.1.3).
    assert!(text.contains("implement  fix/issue-3"), "{text}");
    assert!(!text.contains(crate::api::waiting::QUIET), "{text}");
}

/// The header says how old the feed under the reading is, and a run between
/// two turns says so rather than showing nothing (§FS-019-waiting.3,
/// §FS-019-waiting.1.3).
#[test]
fn the_header_says_how_old_the_feed_is_and_a_run_between_turns_says_so() {
    let mut idle = running(Some("3f9a2c"));
    idle.tickets.clear();
    let mut view = reading(Vec::new(), Vec::new(), vec![idle]);
    view.overview.refreshed_at = Some(chrono::Utc::now() - chrono::Duration::minutes(2));
    let screen = WaitingScreen::new(view, BTreeMap::new());
    assert_eq!(
        screen.title(),
        " ephor — waiting   0 on you · 0 new · 1 running · refreshed 2m ago"
    );
    let text = screen.text();
    assert!(text.contains("between turns"), "{text}");
}

/// With nothing on the reader and nothing new, the screen says so and still
/// shows what is running and what is held; the overview line stays in the
/// footer (§FS-019-waiting.2).
#[test]
fn an_empty_screen_is_an_answer_that_still_shows_running() {
    let mut view = reading(Vec::new(), Vec::new(), vec![running(None)]);
    view.held = vec![views::HeldMatter {
        id: "forge:widget/88".to_string(),
        project: "widget".to_string(),
        kind: ItemKind::Issue,
        title: "Rename the lock".to_string(),
        label: "human-discussion".to_string(),
    }];
    view.decided = views::Decided {
        laid: 5,
        dismissed: 1,
    };
    let screen = WaitingScreen::new(view, BTreeMap::new());
    let text = screen.text();
    assert!(
        text.starts_with(&format!(
            "  {} No gate is open and every matter not held has work laid or a recorded decision.",
            crate::api::waiting::QUIET
        )),
        "{text}"
    );
    assert!(
        text.contains("decided today: 5 laid · 1 needs no work"),
        "{text}"
    );
    assert!(text.contains("WAITING ON YOU · 0"), "{text}");
    assert!(
        text.contains("1 matter is held, and offered nothing: Rename the lock (human-discussion)"),
        "{text}"
    );
    assert!(text.contains("RUNNING · 1"), "{text}");
    assert!(screen.footer().contains("overview: 143 matters"));
}

/// Where the runtime cannot run anything, its sentence stands in for the
/// empty sections it would have filled (§FS-019-waiting.2).
#[test]
fn a_refusal_stands_in_for_the_sections_the_runtime_fills() {
    let mut view = reading(
        Vec::new(),
        vec![new_row("forge:widget/31", "A new one")],
        Vec::new(),
    );
    view.refusal = Some("Nothing can run here: the runner is not on PATH".to_string());
    let text = WaitingScreen::new(view, BTreeMap::new()).text();
    assert_eq!(
        text.matches("Nothing can run here").count(),
        2,
        "once under each section it empties:\n{text}"
    );
    assert!(text.contains("A new one"), "{text}");
}

/// Enter does what the row is (§FS-019-waiting.4): a gate opens its review,
/// a gate no matter on the feed is about says so, a poll with no matter
/// reads its plan, a new matter is processed and `l` opens it as the overview
/// opens it (§FS-019-waiting.7.1), and a run
/// attaches — or says it left nothing to attach to. Esc on the floor does
/// nothing, and q leaves.
#[test]
fn enter_does_what_the_row_is() {
    let reviewed = matter("forge:widget/7", "release-minor fails on push");
    let fresh = matter("forge:widget/31", "transition refuses a valid Prior");
    let items = BTreeMap::from([
        (reviewed.id.clone(), reviewed.clone()),
        (fresh.id.clone(), fresh.clone()),
    ]);
    let mut screen = WaitingScreen::new(
        reading(
            vec![
                gate("widget-7.ask-1", Some("forge:widget/7")),
                gate("loose-1.ask-1", None),
                poll(),
            ],
            vec![new_row(
                "forge:widget/31",
                "transition refuses a valid Prior",
            )],
            vec![running(Some("3f9a2c")), running(None)],
        ),
        items,
    );

    match screen.handle_key(KeyCode::Enter) {
        Action::ReviewHumanGate(item) => assert_eq!(item.id, "forge:widget/7"),
        _ => panic!("a gate opens its review"),
    }
    screen.handle_key(KeyCode::Char('j'));
    match screen.handle_key(KeyCode::Enter) {
        Action::SetMessage(says) => assert_eq!(says, NO_REVIEW),
        _ => panic!("a gate with no matter says so"),
    }
    match screen.handle_key(KeyCode::Char('e')) {
        Action::ReadPlan(plan) => assert_eq!(plan, PathBuf::from("/w/widget-7/panta/widget-7.md")),
        _ => panic!("e reads the plan behind a gate"),
    }
    screen.handle_key(KeyCode::Down);
    match screen.handle_key(KeyCode::Enter) {
        Action::ReadPlan(plan) => assert_eq!(plan, PathBuf::from("/w/widget-9/panta/widget-9.md")),
        _ => panic!("a poll with no matter reads its plan"),
    }
    screen.handle_key(KeyCode::Char('j'));
    match screen.handle_key(KeyCode::Enter) {
        Action::ProcessMatter {
            item,
            entry,
            choosing,
        } => {
            assert_eq!(item.id, fresh.id);
            assert_eq!(entry, None, "the proposal, not a pick");
            assert!(!choosing);
        }
        _ => panic!("a new matter is processed (§FS-019-waiting.7.1)"),
    }
    match screen.handle_key(KeyCode::Char('l')) {
        Action::OpenThread { item, or_url } => {
            assert_eq!(item.id, fresh.id);
            assert!(or_url);
        }
        _ => panic!("l opens a new matter as the overview opens it"),
    }
    screen.handle_key(KeyCode::Char('j'));
    match screen.handle_key(KeyCode::Enter) {
        Action::AttachRun { id, root } => {
            assert_eq!(id, "3f9a2c");
            assert_eq!(root, PathBuf::from("/w/widget-3/panta"));
        }
        _ => panic!("a running row attaches"),
    }
    screen.handle_key(KeyCode::Char('G'));
    match screen.handle_key(KeyCode::Enter) {
        Action::SetMessage(says) => assert_eq!(says, NO_RUN_ID),
        _ => panic!("a run with no id says so"),
    }
    assert!(matches!(screen.handle_key(KeyCode::Esc), Action::None));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('q')),
        Action::Quit
    ));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('r')),
        Action::Refresh
    ));
}

/// A reading taken again keeps the cursor on the row it was on, whatever
/// appeared above it (§FS-005-dispatch.15.1).
#[test]
fn a_fresh_reading_keeps_the_cursor_on_its_row() {
    let mut screen = WaitingScreen::new(
        reading(
            vec![gate("widget-7.ask-1", None)],
            vec![new_row("forge:widget/31", "Thirty-one")],
            Vec::new(),
        ),
        BTreeMap::new(),
    );
    screen.handle_key(KeyCode::Char('j'));
    screen.replace(
        reading(
            vec![gate("widget-6.ask-1", None), gate("widget-7.ask-1", None)],
            vec![
                new_row("forge:widget/32", "Thirty-two"),
                new_row("forge:widget/31", "Thirty-one"),
            ],
            Vec::new(),
        ),
        BTreeMap::new(),
    );
    let text = screen.text();
    let cursor = text
        .lines()
        .find(|line| line.contains('▸'))
        .unwrap_or_else(|| panic!("no cursor in:\n{text}"));
    assert!(cursor.contains("Thirty-one"), "{cursor}");
}

/// A few held matters are named on their line; more than a line has room for
/// are counted by the label that holds them, and the command lists each
/// (§FS-019-waiting.1.2).
#[test]
fn many_held_matters_are_counted_by_the_label_that_holds_them() {
    let held = |n: usize, label: &str| views::HeldMatter {
        id: format!("forge:widget/{n}"),
        project: "widget".to_string(),
        kind: ItemKind::Issue,
        title: format!("Held {n}"),
        label: label.to_string(),
    };
    let mut view = reading(Vec::new(), Vec::new(), Vec::new());
    view.held = (1..=4).map(|n| held(n, "human-discussion")).collect();
    view.held.push(held(5, "on-hold"));
    let text = WaitingScreen::new(view, BTreeMap::new()).text();
    assert!(
        text.contains(
            "5 matters are held, and offered nothing: 4 by human-discussion · 1 by on-hold"
        ),
        "{text}"
    );
    assert!(!text.contains("Held 1"), "{text}");
}

/// Before the first reading lands the screen says it is reading, and shows
/// neither the quiet sentence nor an empty section: those are answers, and
/// there is none yet (§FS-019-waiting.2). The first reading replaces it.
#[test]
fn a_screen_not_yet_read_says_so_rather_than_nothing() {
    let mut screen = WaitingScreen::pending();
    assert!(!screen.is_read());
    let text = screen.text();
    assert!(text.contains(super::READING), "{text}");
    assert!(!text.contains(crate::api::waiting::QUIET), "{text}");
    assert!(!text.contains("WAITING ON YOU"), "{text}");
    assert!(!screen.title().contains("0 on you"), "{}", screen.title());
    assert!(
        !screen.footer().contains("overview:"),
        "{}",
        screen.footer()
    );

    screen.replace(reading(Vec::new(), Vec::new(), Vec::new()), BTreeMap::new());
    assert!(screen.is_read());
    assert!(screen.text().contains(crate::api::waiting::QUIET));
}

/// A new row says what processing it would do; `a` picks another workflow,
/// `x` asks why it needs none, and `A` previews every unambiguous row
/// (§FS-019-waiting.7.1, §FS-019-waiting.7.3, §FS-019-waiting.7.4,
/// §FS-019-waiting.7.5). Off a new row the three keys do nothing of the kind.
#[test]
fn a_new_row_proposes_and_its_keys_process_it() {
    let fresh = matter("forge:widget/31", "transition refuses a valid Prior");
    let items = BTreeMap::from([(fresh.id.clone(), fresh.clone())]);
    let mut wants = new_row("forge:widget/32", "Wants an input");
    wants.proposal = views::Proposal {
        entry: "review-change".to_string(),
        kind: "workflow",
        gate: "ready",
        refusal: Some("it still wants change_ref".to_string()),
        ..views::Proposal::default()
    };
    let mut screen = WaitingScreen::new(
        reading(
            vec![gate("widget-7.ask-1", None)],
            vec![
                new_row("forge:widget/31", "transition refuses a valid Prior"),
                wants,
            ],
            Vec::new(),
        ),
        items,
    );
    let text = screen.text();
    assert!(text.contains("→ fix"), "{text}");
    assert!(
        text.contains("→ review-change · it still wants change_ref"),
        "{text}"
    );
    assert!(
        screen.footer().contains("A process all"),
        "{}",
        screen.footer()
    );

    // On the gate, a and x are not processing.
    assert!(!matches!(
        screen.handle_key(KeyCode::Char('x')),
        Action::DismissMatter(_)
    ));
    screen.handle_key(KeyCode::Char('j'));
    match screen.handle_key(KeyCode::Char('a')) {
        Action::ProcessMatter { item, choosing, .. } => {
            assert_eq!(item.id, fresh.id);
            assert!(choosing, "a opens the chooser");
        }
        _ => panic!("a picks another workflow"),
    }
    match screen.handle_key(KeyCode::Char('x')) {
        Action::DismissMatter(item) => assert_eq!(item.id, fresh.id),
        _ => panic!("x records that it needs none"),
    }
    assert!(matches!(
        screen.handle_key(KeyCode::Char('A')),
        Action::PreviewProcessAll
    ));
    // A row whose matter left the feed since the reading says so.
    screen.handle_key(KeyCode::Char('j'));
    match screen.handle_key(KeyCode::Enter) {
        Action::SetMessage(says) => assert_eq!(says, super::LEFT_THE_FEED),
        _ => panic!("a row with no matter says so"),
    }
}

/// With no new row, `A` says there is nothing to process rather than opening
/// an empty preview (§FS-019-waiting.7.5).
#[test]
fn process_all_with_nothing_new_says_so() {
    let mut screen = WaitingScreen::new(
        reading(vec![gate("widget-7.ask-1", None)], Vec::new(), Vec::new()),
        BTreeMap::new(),
    );
    match screen.handle_key(KeyCode::Char('A')) {
        Action::SetMessage(says) => assert!(says.contains("Nothing new"), "{says}"),
        _ => panic!("nothing to preview"),
    }
}

/// Work laid and waiting for a slot is one line under Running, with the
/// admission's reason; a proposal confirmed often enough names the setting
/// that would do it without the reader, in the footer (§FS-019-waiting.7.6,
/// §FS-019-waiting.7.7).
#[test]
fn queued_work_and_the_adopt_sentence_are_shown() {
    let mut view = reading(Vec::new(), Vec::new(), vec![running(None)]);
    view.queued = vec![views::QueuedMatter {
        id: "forge:widget/44".to_string(),
        project: "widget".to_string(),
        title: "Queued".to_string(),
        at: Utc::now(),
        says: Some("global work.max_active 1 is full".to_string()),
    }];
    view.adopt = Some(
        "fix-gate confirmed for 3 pull requests · \"autorun\": true on fix-gate would do it \
         without you"
            .to_string(),
    );
    let screen = WaitingScreen::new(view, BTreeMap::new());
    let text = screen.text();
    assert!(
        text.contains("queued 1 · widget/44 · global work.max_active 1 is full"),
        "{text}"
    );
    assert!(text.find("RUNNING · 1") < text.find("queued 1"), "{text}");
    assert!(
        screen
            .footer()
            .starts_with(" fix-gate confirmed for 3 pull requests"),
        "{}",
        screen.footer()
    );
}
