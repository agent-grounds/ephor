//! The processing screen over accounts made by hand (§FS-019-waiting.7.2,
//! §FS-019-waiting.7.3, §FS-019-waiting.7.5).

use chrono::Utc;
use ratatui::crossterm::event::KeyCode;
use serde_json::json;

use super::{Choice, Place, ProcessScreen};
use crate::api::views;
use crate::feed::model::{Item, ItemKind};
use crate::feed::tui::Action;

fn matter() -> Item {
    Item {
        id: "forge:widget/31".to_string(),
        project: "widget".to_string(),
        source: "forge".to_string(),
        kind: ItemKind::Pr,
        role: None,
        title: "Widen the retry window".to_string(),
        url: None,
        state: Some("open".to_string()),
        needs_response: false,
        updated_at: Utc::now(),
        raw: json!({}),
    }
}

fn proposal(others: usize) -> views::Proposal {
    views::Proposal {
        entry: "fix-gate".to_string(),
        kind: "agent",
        icon: "🛠".to_string(),
        description: "fix the red gate".to_string(),
        gate: "needs-checkout",
        workspace: Some("/w/widget/you/retry".into()),
        root: Some("/w/widget/you/retry/panta".into()),
        others,
        unambiguous: true,
        ..views::Proposal::default()
    }
}

fn account(ok: bool, says: &str) -> views::ProcessRow {
    views::ProcessRow {
        item: "forge:widget/31".to_string(),
        title: "Widen the retry window".to_string(),
        entry: "fix-gate".to_string(),
        kind: "agent",
        ok,
        says: says.to_string(),
        plan: Some("/w/widget/you/retry/panta/widget-31.rhei.md".into()),
        tickets: vec!["widget-31.fix-gate-1".to_string()],
        start: None,
        assignees: None,
        labels: Vec::new(),
        answers: Vec::new(),
    }
}

fn choices() -> Vec<Choice> {
    vec![
        Choice {
            key: "fix-gate".to_string(),
            kind: "agent",
            description: "fix the red gate".to_string(),
            refusal: None,
        },
        Choice {
            key: "review-change".to_string(),
            kind: "workflow",
            description: "review this change".to_string(),
            refusal: None,
        },
    ]
}

/// The screen shows the proposal, why it is the one, where the work would go
/// and what a dry run says it would lay; `Enter` confirms it
/// (§FS-019-waiting.7.2).
#[test]
fn the_proposal_is_shown_with_its_account_and_enter_confirms_it() {
    let mut next = matter();
    next.id = "forge:widget/32".to_string();
    let mut said = account(true, "would lay widget-31.fix-gate-1");
    said.assignees = Some(Vec::new());
    said.labels = vec!["bug".to_string()];
    let mut proposed = proposal(1);
    proposed.hand = Some("cld".to_string());
    proposed.branch = Some("fix/issue-31".to_string());
    let mut screen = ProcessScreen::one(
        matter(),
        Place {
            at: Some((1, 5)),
            next: Some(Box::new(next)),
        },
        proposed,
        said,
        false,
        choices(),
    );
    // The title names the matter and where it stands among the new rows.
    assert_eq!(
        screen.title(),
        " ephor — new matter — widget/31 · 1 of 5 new"
    );
    let text = screen.text();
    assert!(text.contains("labels bug"), "{text}");
    assert!(
        text.contains("held by    nobody · no assignee on the forge"),
        "{text}"
    );
    assert!(
        text.contains("work       none laid · no plan in the ledger"),
        "{text}"
    );
    assert!(text.contains("PROPOSED"), "{text}");
    assert!(text.contains("fix-gate"), "{text}");
    assert!(
        text.contains("the first entry in menu order · 1 other would take it"),
        "{text}"
    );
    assert!(text.contains("hand       cld"), "{text}");
    assert!(
        text.contains(
            "plan       /w/widget/you/retry/panta/widget-31.rhei.md · written on confirm"
        ),
        "{text}"
    );
    assert!(
        text.contains("checkout   fix/issue-31 at /w/widget/you/retry · will be created"),
        "{text}"
    );
    assert!(text.contains("WHAT IT WOULD DO"), "{text}");
    assert!(text.contains("widget-31.fix-gate-1"), "{text}");
    assert!(
        text.contains("records it against widget/31 in the ledger"),
        "{text}"
    );
    assert!(text.contains("Ephor posts nothing to the forge."), "{text}");
    assert_eq!(
        screen.footer(),
        " enter lay and start  ·  a other workflow  ·  x needs no work  ·  n next  ·  l open  ·  esc waiting"
    );

    // `n` opens the next new row's screen, this one left undecided.
    match screen.handle_key(KeyCode::Char('n')) {
        Action::ProcessMatter {
            item,
            entry,
            choosing,
        } => {
            assert_eq!(item.id, "forge:widget/32");
            assert_eq!(entry, None);
            assert!(!choosing);
        }
        _ => panic!("n opens the next new matter"),
    }

    match screen.handle_key(KeyCode::Enter) {
        Action::ConfirmProcess { item, entry, wants } => {
            assert_eq!(item.id, "forge:widget/31");
            assert_eq!(entry, "fix-gate");
            assert!(!wants);
        }
        _ => panic!("Enter confirms the proposal"),
    }
    assert!(matches!(
        screen.handle_key(KeyCode::Char('x')),
        Action::DismissMatter(_)
    ));
    assert!(matches!(
        screen.handle_key(KeyCode::Char('l')),
        Action::OpenThread { or_url: true, .. }
    ));
    assert!(matches!(screen.handle_key(KeyCode::Esc), Action::Back));
}

/// What the dry run refused is not laid: `Enter` says why instead. A workflow
/// that still wants inputs is the one exception — `Enter` opens them
/// (§FS-019-waiting.7.2).
#[test]
fn a_refused_account_says_why_and_wanted_inputs_are_asked_for() {
    let mut refused = ProcessScreen::one(
        matter(),
        Place::default(),
        proposal(0),
        account(false, "× the runtime refused it"),
        false,
        Vec::new(),
    );
    match refused.handle_key(KeyCode::Enter) {
        Action::SetMessage(says) => assert_eq!(says, "× the runtime refused it"),
        _ => panic!("a refusal is said, not laid"),
    }
    match refused.handle_key(KeyCode::Char('a')) {
        Action::SetMessage(says) => assert!(says.contains("No other entry"), "{says}"),
        _ => panic!("nothing to choose among"),
    }
    // On the last new row there is no next one, and `n` says so.
    assert!(!refused.footer().contains("n next"), "{}", refused.footer());
    match refused.handle_key(KeyCode::Char('n')) {
        Action::SetMessage(says) => assert_eq!(says, super::LAST),
        _ => panic!("n on the last row says so"),
    }
    assert_eq!(refused.title(), " ephor — new matter — widget/31");

    let mut wants = ProcessScreen::one(
        matter(),
        Place::default(),
        proposal(0),
        account(false, "review-change still wants:\n  --set change_ref=…"),
        true,
        Vec::new(),
    );
    assert!(wants.footer().contains("enter answers its inputs"));
    match wants.handle_key(KeyCode::Enter) {
        Action::ConfirmProcess { wants, .. } => assert!(wants),
        _ => panic!("Enter opens the inputs"),
    }
}

/// `a` lists every entry that would take the matter and every workflow no
/// entry names; `Enter` on one processes with it, and `Esc` closes the list
/// without leaving the screen (§FS-019-waiting.7.3).
#[test]
fn a_chooses_another_workflow() {
    let mut screen = ProcessScreen::one(
        matter(),
        Place::default(),
        proposal(1),
        account(true, "would lay"),
        false,
        choices(),
    );
    assert!(matches!(
        screen.handle_key(KeyCode::Char('a')),
        Action::None
    ));
    assert!(screen.text().contains("CHOOSE ANOTHER"));
    assert!(screen.footer().contains("enter processes with it"));
    screen.handle_key(KeyCode::Char('j'));
    match screen.handle_key(KeyCode::Enter) {
        Action::ProcessMatter {
            item,
            entry,
            choosing,
        } => {
            assert_eq!(item.id, "forge:widget/31");
            assert_eq!(entry.as_deref(), Some("review-change"));
            assert!(!choosing);
        }
        _ => panic!("Enter on a choice processes with it"),
    }
    assert!(matches!(screen.handle_key(KeyCode::Esc), Action::None));
    assert!(!screen.text().contains("CHOOSE ANOTHER"));

    // Opened from `a` on the Waiting row, the list is open at once.
    let opened = ProcessScreen::one(
        matter(),
        Place::default(),
        proposal(1),
        account(true, "would lay"),
        false,
        choices(),
    )
    .choosing();
    assert!(opened.text().contains("▸ fix-gate"), "{}", opened.text());
}

/// `A`'s preview names what each unambiguous row would lay and what it
/// leaves; `Enter` is the reader's act, and an empty preview has nothing to
/// confirm (§FS-019-waiting.7.5).
#[test]
fn the_preview_of_all_is_the_confirmation() {
    let mut row = account(true, "would lay widget-31.fix-gate-1\nand more");
    row.start = None;
    let mut screen = ProcessScreen::all(views::Processing {
        dry_run: true,
        processed: vec![row],
        left: vec![views::LeftMatter {
            item: "forge:widget/32".to_string(),
            title: "Wants an input".to_string(),
            says: "it still wants change_ref".to_string(),
        }],
        gated: false,
        says: None,
    });
    assert_eq!(screen.title(), " ephor — process 1 new matter");
    let text = screen.text();
    assert!(text.contains("WOULD PROCESS · 1"), "{text}");
    assert!(
        text.contains("→ fix-gate · would lay widget-31.fix-gate-1"),
        "{text}"
    );
    assert!(!text.contains("and more"), "{text}");
    assert!(text.contains("LEFT FOR YOU · 1"), "{text}");
    assert!(text.contains("it still wants change_ref"), "{text}");
    assert!(matches!(
        screen.handle_key(KeyCode::Enter),
        Action::ConfirmProcessAll
    ));

    let mut empty = ProcessScreen::all(views::Processing {
        dry_run: true,
        processed: Vec::new(),
        left: Vec::new(),
        gated: false,
        says: None,
    });
    assert_eq!(empty.footer(), " esc back");
    assert!(matches!(
        empty.handle_key(KeyCode::Enter),
        Action::SetMessage(_)
    ));
}
