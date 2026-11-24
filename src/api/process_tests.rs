//! The sentences processing says, held to §FS-019-waiting.7 without a
//! session: what a row says about its proposal, the queued line, the summary
//! of `A`, and what holds a matter. The moves themselves are proven end to end
//! by E2E-073.

use std::collections::BTreeSet;

use chrono::{TimeZone, Utc};
use serde_json::json;

use super::{holding_label, no_triage, plural, summary};
use crate::api::views;
use crate::feed::model::{Item, ItemKind};

fn ready(entry: &str) -> views::Proposal {
    views::Proposal {
        entry: entry.to_string(),
        kind: "agent",
        description: "fix the red gate".to_string(),
        gate: "ready",
        unambiguous: true,
        ..views::Proposal::default()
    }
}

fn row(item: &str, ok: bool, start: Option<&'static str>) -> views::ProcessRow {
    views::ProcessRow {
        item: item.to_string(),
        title: "t".to_string(),
        entry: "fix-gate".to_string(),
        kind: "agent",
        ok,
        says: String::new(),
        plan: None,
        tickets: Vec::new(),
        start,
        assignees: None,
        labels: Vec::new(),
        answers: Vec::new(),
    }
}

/// A row names the entry, says how many others would take the matter, and
/// says why where it cannot be processed as it stands (§FS-019-waiting.7.1).
#[test]
fn a_proposal_says_its_entry_its_others_and_why_not() {
    assert_eq!(ready("fix-gate").says(), "→ fix-gate");
    let mut more = ready("fix-gate");
    more.others = 2;
    more.gate = "needs-checkout";
    assert_eq!(more.says(), "→ fix-gate · makes its checkout · 2 more");

    let mut wants = ready("review-change");
    wants.kind = "workflow";
    wants.unambiguous = false;
    wants.refusal = Some("it still wants\n change_ref".to_string());
    assert_eq!(wants.says(), "→ review-change · it still wants change_ref");

    // No entry takes it and the runtime offers no triage: the name stands,
    // and says it is not available.
    let none = views::Proposal {
        entry: super::TRIAGE.to_string(),
        kind: "workflow",
        gate: "blocked",
        fallback: true,
        refusal: Some(no_triage("widget")),
        ..views::Proposal::default()
    };
    assert_eq!(none.says(), "→ triage · not available");
    assert!(no_triage("widget").contains("not available"));
}

/// One line under the live runs names what waits for a slot, by the ids a
/// person says, with the admission's own reason (§FS-019-waiting.7.6).
#[test]
fn the_queued_line_names_the_matters_and_the_admissions_reason() {
    let queued = |id: &str, says: Option<&str>| views::QueuedMatter {
        id: id.to_string(),
        project: "ephor".to_string(),
        title: "t".to_string(),
        at: Utc::now(),
        says: says.map(str::to_string),
    };
    let mut view = views::Waiting {
        refusal: None,
        on_you: Vec::new(),
        new: Vec::new(),
        held: Vec::new(),
        running: Vec::new(),
        queued: Vec::new(),
        adopt: None,
        decided: views::Decided::default(),
        overview: views::Overview::default(),
    };
    assert_eq!(view.queued_line(), None);
    view.queued = vec![
        queued("github-issues:agent-grounds/ephor#119", None),
        queued(
            "github-issues:agent-grounds/fissile#78",
            Some("global work.max_active 4 is full (4 active run(s), 0 parked)"),
        ),
    ];
    assert_eq!(
        view.queued_line().as_deref(),
        Some(
            "queued 2 · ephor#119, fissile#78 · global work.max_active 4 is full \
             (4 active run(s), 0 parked)"
        )
    );
}

/// `A`'s summary counts what was laid, what started, what queued, and what
/// was left (§FS-019-waiting.7.5).
#[test]
fn processing_all_is_summed_up_in_one_line() {
    let view = views::Processing {
        dry_run: false,
        processed: vec![
            row("a", true, Some("started")),
            row("b", true, Some("queued")),
            row("c", true, Some("queued")),
            row("d", false, None),
        ],
        left: vec![views::LeftMatter {
            item: "e".to_string(),
            title: "t".to_string(),
            says: "it still wants change_ref".to_string(),
        }],
        gated: false,
        says: None,
    };
    assert_eq!(
        summary(&view),
        "processed 3: 1 started, 2 queued · 1 refused · 1 left for you"
    );
}

/// A held matter is held by its own label, and only by a label some selector
/// excludes (§FS-019-waiting.1.2).
#[test]
fn a_matter_is_held_by_its_own_label() {
    let item = Item {
        id: "forge:widget/5".to_string(),
        project: "widget".to_string(),
        source: "forge".to_string(),
        kind: ItemKind::Issue,
        role: None,
        title: "Discussed".to_string(),
        url: None,
        state: Some("open".to_string()),
        needs_response: false,
        updated_at: Utc.with_ymd_and_hms(2026, 11, 20, 10, 0, 0).unwrap(),
        raw: json!({ "labels": ["bug", "human-discussion"] }),
    };
    let holds = BTreeSet::from(["human-discussion".to_string()]);
    assert_eq!(
        holding_label(&item, &holds).as_deref(),
        Some("human-discussion")
    );
    assert_eq!(holding_label(&item, &BTreeSet::new()), None);
}

/// The kinds of matter, counted the way the adopt sentence counts them
/// (§FS-019-waiting.7.7).
#[test]
fn a_kind_of_matter_is_counted_in_words() {
    assert_eq!(plural("issue"), "issues");
    assert_eq!(plural("pr"), "pull requests");
    assert_eq!(plural("msg"), "messages");
    assert_eq!(plural("whatever"), "whatever matters");
}
