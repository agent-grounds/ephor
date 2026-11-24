//! A prepared gate's form, keyed and drawn the way screens 3 and 4 show it
//! (§FS-005-dispatch.9.3).

use super::*;
use crate::work::runtime::human_gate::Gate;
use serde_json::json;

fn item() -> Item {
    Item {
        id: "tasks:inbox.2".to_string(),
        project: "demo".to_string(),
        source: "tasks".to_string(),
        kind: crate::feed::model::ItemKind::Task,
        role: None,
        title: "Vet the storage migration".to_string(),
        url: None,
        state: Some("vetting".to_string()),
        needs_response: true,
        updated_at: chrono::Utc::now(),
        raw: json!({}),
    }
}

/// The brief a runtime prepares for a gate asking all four kinds, the file to
/// read first written beside it.
fn brief(dir: &std::path::Path) -> Brief {
    let display = dir.join("plan.md");
    std::fs::write(
        &display,
        "# The migration\n\nTwo tables move.\nNothing else.\n",
    )
    .unwrap();
    serde_json::from_value(json!({
        "version": 1, "ticket": "inbox.2", "title": "Vet the storage migration",
        "state": "vetting", "visit": 2, "since": null, "source": "static",
        "summary": "Vet the storage migration, in Inbox Sample",
        "display": {"path": "plan.md", "absolute": display, "readable": true, "problem": null},
        "attachments": [],
        "questions": [
            {"id": "approve", "kind": "boolean", "prompt": "Go ahead?", "required": true},
            {"id": "scope", "kind": "choice", "prompt": "How much of it?", "required": true,
             "options": [{"value": "small", "label": "The first table only"},
                         {"value": "all", "label": "Every table"}]},
            {"id": "channels", "kind": "multi", "prompt": "Where?", "required": false,
             "options": [{"value": "blog", "label": "The blog"},
                         {"value": "mail", "label": "The mailing list"}]},
            {"id": "note", "kind": "text", "prompt": "Anything to respect?", "required": false}
        ],
        "destinations": [{"state": "apply", "description": null, "condition": null, "final": false},
                         {"state": "cancelled", "description": null, "condition": null, "final": true}],
        "routes": [{"to": "apply", "when": {"approve": true}}, {"to": "cancelled"}],
        "digest": "sha256:feed", "answer_path": "runtime/gates/inbox.2/vetting.2.answer.json",
        "problems": []
    }))
    .unwrap()
}

fn review(dir: &std::path::Path, brief: Option<Brief>) -> Review {
    let plan = dir.join("inbox.rhei.md");
    Review {
        gates: vec![Gate {
            ticket: "inbox.2".to_string(),
            title: "Vet the storage migration".to_string(),
            state: "vetting".to_string(),
            root: dir.to_path_buf(),
            plan: plan.clone(),
            machine: None,
            destinations: vec![human_gate::Destination {
                state: "apply".to_string(),
                condition: None,
                description: None,
            }],
            brief,
            unbriefed: None,
        }],
        files: vec![Artifact {
            name: "inbox.2 · log".to_string(),
            path: dir.join("inbox.2.log"),
            problem: None,
        }],
        finished: Vec::new(),
    }
}

fn screen(dir: &std::path::Path) -> BriefScreen {
    BriefScreen::open(item(), &review(dir, Some(brief(dir))), "inbox.2", "runner")
        .expect("a briefed gate opens its form")
        .returning(Return::Waiting, Return::Waiting)
}

fn key(screen: &mut BriefScreen, code: KeyCode) -> Action {
    screen.handle_key(code, KeyModifiers::NONE)
}

fn typed(screen: &mut BriefScreen, text: &str) {
    for ch in text.chars() {
        key(screen, KeyCode::Char(ch));
    }
}

fn drawn(screen: &mut BriefScreen) -> String {
    let backend = ratatui::backend::TestBackend::new(120, 40);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| screen.draw(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    buffer
        .content
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A gate with no brief has no form: the review is what it opens instead.
#[test]
fn a_gate_without_a_brief_has_no_form() {
    let dir = tempfile::tempdir().unwrap();
    assert!(BriefScreen::open(item(), &review(dir.path(), None), "inbox.2", "runner").is_none());
    assert!(BriefScreen::open(
        item(),
        &review(dir.path(), Some(brief(dir.path()))),
        "inbox.9",
        "runner"
    )
    .is_none());
}

/// Every kind is answered from the form: space takes one of a choice, puts one
/// of several in and out in the brief's order, says yes then no, and opens a
/// line for a text; the count says how many are answered, and the document is
/// the digest copied and the answers by id (§FS-005-dispatch.9.3).
#[test]
fn each_kind_is_answered_and_counted_into_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(dir.path());
    assert_eq!(screen.stops().len(), 1 + 2 + 2 + 1);
    assert_eq!(screen.count(), (0, 4));

    key(&mut screen, KeyCode::Char(' '));
    assert_eq!(screen.answers["approve"], json!(true));
    key(&mut screen, KeyCode::Char(' '));
    assert_eq!(
        screen.answers["approve"],
        json!(false),
        "space says no the second time"
    );

    key(&mut screen, KeyCode::Char('j'));
    key(&mut screen, KeyCode::Char('j'));
    key(&mut screen, KeyCode::Enter);
    assert_eq!(
        screen.answers["scope"],
        json!("all"),
        "enter chooses as space does"
    );
    key(&mut screen, KeyCode::Char('k'));
    key(&mut screen, KeyCode::Char(' '));
    assert_eq!(
        screen.answers["scope"],
        json!("small"),
        "a choice holds one"
    );

    // The second of several taken first, then the first.
    for _ in 0..3 {
        key(&mut screen, KeyCode::Char('j'));
    }
    key(&mut screen, KeyCode::Char(' '));
    assert_eq!(screen.answers["channels"], json!(["mail"]));
    key(&mut screen, KeyCode::Char('k'));
    key(&mut screen, KeyCode::Char(' '));
    assert_eq!(
        screen.answers["channels"],
        json!(["blog", "mail"]),
        "in the brief's order"
    );
    key(&mut screen, KeyCode::Char(' '));
    assert_eq!(screen.answers["channels"], json!(["mail"]));
    key(&mut screen, KeyCode::Char('j'));
    key(&mut screen, KeyCode::Char(' '));
    assert!(
        !screen.answers.contains_key("channels"),
        "none of several is no answer"
    );
    assert_eq!(screen.count(), (2, 4));

    key(&mut screen, KeyCode::Char('j'));
    key(&mut screen, KeyCode::Char(' '));
    assert!(screen.is_typing());
    typed(&mut screen, "keep the backup; s and q are letters");
    key(&mut screen, KeyCode::Enter);
    assert!(!screen.is_typing());
    assert_eq!(
        screen.answers["note"],
        json!("keep the backup; s and q are letters")
    );
    key(&mut screen, KeyCode::Enter);
    typed(&mut screen, " — dropped");
    key(&mut screen, KeyCode::Esc);
    assert_eq!(
        screen.answers["note"],
        json!("keep the backup; s and q are letters")
    );
    assert_eq!(screen.count(), (3, 4));

    assert_eq!(
        screen.document(),
        json!({"version": 1, "request_digest": "sha256:feed", "answers": {
            "approve": false, "scope": "small", "note": "keep the backup; s and q are letters"
        }})
    );
    key(&mut screen, KeyCode::Backspace);
    assert!(
        !screen.answers.contains_key("note"),
        "backspace takes an answer back"
    );
}

/// `s` asks the runtime first; its verdict is the confirmation, and `enter`
/// there sends the same document for real while `esc` goes back to the form
/// (§FS-005-dispatch.9.3).
#[test]
fn submitting_asks_the_runtime_first_and_sends_the_same_document() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(dir.path());
    key(&mut screen, KeyCode::Char(' '));
    let sent = screen.document();
    match key(&mut screen, KeyCode::Char('s')) {
        Action::AnswerGate {
            ticket,
            from,
            document,
            dry_run,
            ..
        } => {
            assert_eq!(
                (ticket.as_str(), from.as_str(), dry_run),
                ("inbox.2", "vetting", true)
            );
            assert_eq!(document, sent);
        }
        _ => panic!("s asks for a dry run"),
    }
    let verdict: Verdict = serde_json::from_value(json!({
        "ok": true, "dry_run": true, "to": "apply",
        "route": {"index": 0, "when": {"approve": true}},
        "answer_path": "runtime/gates/inbox.2/vetting.2.answer.json", "result": "Go ahead?: yes",
        "errors": []
    }))
    .unwrap();
    screen.dry_ran(Ok(verdict));
    let text = drawn(&mut screen);
    assert!(text.contains("Submit this answer?"), "{text}");
    assert!(text.contains("Go ahead?"), "{text}");
    assert!(text.contains("yes"), "{text}");
    assert!(
        text.contains("What runner says it will do (dry run)"),
        "{text}"
    );
    assert!(text.contains("inbox.2  vetting → apply"), "{text}");
    assert!(text.contains("approve = true"), "{text}");
    assert!(
        text.contains("runtime/gates/inbox.2/vetting.2.answer.json"),
        "{text}"
    );
    assert!(
        text.contains("Ephor sends the answers and runner performs the move"),
        "{text}"
    );
    assert!(screen
        .footer()
        .contains("enter submit   esc back to the form"));
    assert!(screen
        .footer()
        .contains("ephor work gate answer --answers -"));

    match key(&mut screen, KeyCode::Enter) {
        Action::AnswerGate {
            document, dry_run, ..
        } => {
            assert!(!dry_run, "enter on the confirmation sends it");
            assert_eq!(document, sent, "the same document the dry run judged");
        }
        _ => panic!("enter answers"),
    }
    key(&mut screen, KeyCode::Esc);
    assert!(screen.confirm.is_none(), "esc goes back to the form");
    assert!(drawn(&mut screen).contains("Your answer"));
}

/// What the runtime refused stays on the form beside the question it names,
/// and a field it left unknown is drawn as unknown rather than guessed
/// (§FS-005-dispatch.9.3).
#[test]
fn a_refusal_stays_beside_its_question_and_unknowns_stay_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(dir.path());
    screen.dry_ran(Ok(serde_json::from_value(json!({
        "ok": false, "dry_run": true, "to": null, "route": null, "answer_path": null,
        "result": null,
        "errors": [
            {"kind": "invalid", "question": "scope", "message": "a required question is unanswered"},
            {"kind": "stale", "question": null, "message": "the brief changed since it was read"}
        ]
    }))
    .unwrap()));
    assert!(screen.confirm.is_none());
    let text = drawn(&mut screen);
    assert!(
        text.contains("invalid: a required question is unanswered"),
        "{text}"
    );
    assert!(
        text.contains("refused: the brief changed since it was read"),
        "a refusal that names no question is said above the form\n{text}"
    );
    assert!(
        !text.contains("refused: scope"),
        "one beside its question is not said twice\n{text}"
    );
    let scope = text
        .lines()
        .position(|line| line.contains("How much of it?"))
        .unwrap();
    let refusal = text
        .lines()
        .position(|line| line.contains("invalid: a required"))
        .unwrap();
    assert!(refusal > scope, "the refusal follows its question\n{text}");

    // Ok with nothing known: still nothing guessed.
    screen.dry_ran(Ok(serde_json::from_value(
        json!({"ok": true, "dry_run": true}),
    )
    .unwrap()));
    assert!(screen.refused.is_empty() && screen.says.is_none());
    let text = drawn(&mut screen);
    assert!(text.contains("inbox.2  vetting → unknown"), "{text}");
    assert!(text.contains("the destination you chose"), "{text}");

    screen.refuse(Err(crate::error::EphorError::Command(
        "no such gate".into(),
    )));
    assert!(drawn(&mut screen).contains("refused: no such gate"));

    // A question the form does not draw has no line of its own to stand
    // beside, so its refusal is said above the form.
    screen.dry_ran(Ok(serde_json::from_value(json!({
        "ok": false, "dry_run": true,
        "errors": [{"kind": "invalid", "question": "who",
                    "message": "`who` is not a question this brief asks"}]
    }))
    .unwrap()));
    assert_eq!(
        screen.says.as_deref(),
        Some("who: `who` is not a question this brief asks")
    );
}

/// Screen 3: the title with the state and how long it waited, the matter, the
/// ticket and its visit, the summary, the file read first with its line
/// position, the other files, and the form with its markers (§FS-005-dispatch.9.3).
#[test]
fn the_form_draws_the_brief_and_marks_what_is_answered() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(dir.path());
    assert_eq!(
        screen.title(),
        " ephor — gate — Vet the storage migration   vetting · waiting unknown"
    );
    key(&mut screen, KeyCode::Char(' '));
    let text = drawn(&mut screen);
    for expected in [
        "matter Vet the storage migration · ticket inbox.2 · visit 2",
        "Vet the storage migration, in Inbox Sample",
        "plan.md",
        "line 1 of 4",
        "# The migration",
        "Two tables move.",
        "f other files (1) · inbox.2.log",
        "Your answer   1 of 4 answered",
        "✓ Go ahead?",
        "[x] yes   [ ] no",
        "? How much of it?  required",
        "( ) The first table only",
        "· Where?  optional",
        "[ ] The blog",
        "type here, or e for the editor",
    ] {
        assert!(text.contains(expected), "{expected}\n{text}");
    }
    assert!(
        screen.footer().contains("s submit  esc waiting"),
        "{}",
        screen.footer()
    );

    key(&mut screen, KeyCode::Tab);
    key(&mut screen, KeyCode::Char('j'));
    assert!(
        drawn(&mut screen).contains("line 2 of 4"),
        "tab moves the keys to the file"
    );
    key(&mut screen, KeyCode::Char('G'));
    key(&mut screen, KeyCode::Char('j'));
    assert_eq!(screen.scroll, 3, "the file does not scroll past its end");
}

/// `f` lists the attachments and the files the review infers, `enter` reads
/// one, and `esc` steps back one level at a time.
#[test]
fn other_files_are_listed_and_read() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("inbox.2.log"), "ran twice\n").unwrap();
    let mut screen = screen(dir.path());
    key(&mut screen, KeyCode::Char('f'));
    let text = drawn(&mut screen);
    assert!(text.contains("other files"), "{text}");
    assert!(text.contains("inbox.2 · log"), "{text}");
    key(&mut screen, KeyCode::Enter);
    assert!(drawn(&mut screen).contains("ran twice"));
    assert!(screen.title().contains("local file"));
    key(&mut screen, KeyCode::Esc);
    assert!(screen.files.is_some());
    key(&mut screen, KeyCode::Esc);
    assert!(screen.files.is_none());
}

/// `esc` goes back where the form was opened from: the floor, the work
/// screen, or the review it was sent from (§FS-019-waiting.4).
#[test]
fn esc_returns_where_the_form_was_opened() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(dir.path());
    assert!(matches!(key(&mut screen, KeyCode::Esc), Action::Back));
    let mut screen = screen.returning(Return::Work, Return::Work);
    assert!(matches!(
        key(&mut screen, KeyCode::Esc),
        Action::OpenWork(_)
    ));
    let mut screen = screen.returning(Return::Review, Return::Overview);
    assert!(
        screen.footer().contains("esc review"),
        "{}",
        screen.footer()
    );
    assert!(matches!(
        key(&mut screen, KeyCode::Esc),
        Action::ReviewHumanGate(_)
    ));
}

/// `e` hands a text answer to the editor, starting from what is answered, and
/// what comes back is the answer; the scratch file sits under a private
/// directory of its own (§FS-005-dispatch.9.3).
#[test]
fn a_text_answer_goes_to_the_editor_and_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut screen = screen(dir.path());
    match key(&mut screen, KeyCode::Char('e')) {
        Action::SetMessage(says) => assert!(says.contains("text answer"), "{says}"),
        _ => panic!("e on a yes-or-no says what it is for"),
    }
    screen.cursor = screen.stops().len() - 1;
    screen.set_text("note", "first draft");
    match key(&mut screen, KeyCode::Char('e')) {
        Action::EditGateText { question, text } => {
            assert_eq!((question.as_str(), text.as_str()), ("note", "first draft"));
        }
        _ => panic!("e on a text question opens the editor"),
    }
    let (private, path) = scratch("note", "first draft").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first draft");
    assert!(path.starts_with(&private));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&private).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }
    std::fs::remove_dir_all(private).unwrap();
    screen.set_text("note", "second draft\n\n");
    assert_eq!(screen.answers["note"], json!("second draft"));
    screen.set_text("note", "  \n");
    assert!(!screen.answers.contains_key("note"));
}

/// At a gate whose workflow asks its own required questions, the review's
/// plain move goes to the form instead; where the brief asks only the
/// runtime's stand-ins, the move with a reason is what it always was
/// (§FS-005-dispatch.9.3).
#[test]
fn the_review_sends_a_typed_gate_to_its_form() {
    let dir = tempfile::tempdir().unwrap();
    let mut review_screen =
        super::super::HumanGateScreen::new(item(), review(dir.path(), Some(brief(dir.path()))));
    match review_screen.handle_key(KeyCode::Enter) {
        Action::OpenGateBrief { ticket, .. } => assert_eq!(ticket, "inbox.2"),
        _ => panic!("a typed gate opens its form"),
    }
    let mut plain = brief(dir.path());
    plain.questions = serde_json::from_value(json!([
        {"id": "destination", "kind": "choice", "prompt": "Where?", "required": true,
         "options": [{"value": "apply"}]},
        {"id": "reason", "kind": "text", "prompt": "Why?", "required": true}
    ]))
    .unwrap();
    let mut review_screen =
        super::super::HumanGateScreen::new(item(), review(dir.path(), Some(plain)));
    assert!(matches!(
        review_screen.handle_key(KeyCode::Enter),
        Action::TransitionHumanGate { .. }
    ));
}
