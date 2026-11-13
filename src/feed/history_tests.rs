//! What a refresh commits, history by history (§FS-001-forge-interface.6.1),
//! and the one order a slot's reports are read in (§AR-008-pipeline.2).

use super::*;
use crate::matter::{Matter, SubjectKey};

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_790_000_000 + seconds, 0).unwrap()
}

fn report(key: &str) -> Matter {
    Matter {
        key: SubjectKey::stated(key),
        kind: crate::feed::model::ItemKind::Pr,
        placement: crate::matter::Placement::on("widget"),
        source: "gdev".to_string(),
        title: key.to_string(),
        role: None,
        url: None,
        state: None,
        needs_response: false,
        updated_at: at(0),
        links: Vec::new(),
        discussions: Vec::new(),
        events: Vec::new(),
        fingerprint: Default::default(),
        raw: serde_json::Value::Null,
        settles: false,
    }
}

fn keys(slot: &ProviderSlot) -> Vec<String> {
    slot.matters
        .iter()
        .map(|matter| matter.key.as_str().to_string())
        .collect()
}

const FAILED: Failure = Failure {
    message: "gdev: HTTP 503",
    unreachable: false,
};

/// A plain refresh of a source with two blocks, each answering pull requests
/// and issues.
fn answered_whole() -> ProviderSlot {
    commit_whole(
        None,
        at(0),
        Ok(vec![
            (
                Some("pull-requests"),
                vec![vec![report("a-pr")], vec![report("b-pr")]],
            ),
            (Some("issues"), vec![vec![report("a-issue")], vec![]]),
        ]),
        &[Some("pull-requests"), Some("issues")],
    )
}

/// The slot is read by block, then capability in fetch order, whichever run
/// wrote each history (§AR-008-pipeline.2): a narrowed run that replaced
/// `issues` leaves every report where a full refresh would have put it.
#[test]
fn a_slot_reads_by_block_then_capability_whichever_run_wrote_it() {
    let full = answered_whole();
    assert_eq!(keys(&full), ["a-pr", "a-issue", "b-pr"]);

    let narrowed = commit_one(
        Some(&full),
        at(60),
        "issues",
        &["pull-requests", "issues"],
        Ok(vec![vec![report("a-issue")], vec![report("b-issue")]]),
    );
    assert_eq!(keys(&narrowed), ["a-pr", "a-issue", "b-pr", "b-issue"]);
    assert_eq!(
        narrowed.history(Some("pull-requests")).unwrap().asked_at,
        Some(at(0)),
        "the history the run did not ask keeps its time"
    );
    assert_eq!(
        narrowed.history(Some("issues")).unwrap().asked_at,
        Some(at(60))
    );
}

/// A complete answer replaces the history it answers, an empty one included
/// (§FS-001-forge-interface.6.1).
#[test]
fn an_empty_answer_to_one_capability_replaces_it() {
    let full = answered_whole();
    let emptied = commit_one(
        Some(&full),
        at(60),
        "issues",
        &["pull-requests", "issues"],
        Ok(vec![vec![], vec![]]),
    );
    assert_eq!(keys(&emptied), ["a-pr", "b-pr"]);
    let issues = emptied.history(Some("issues")).unwrap();
    assert!(issues.answered() && !issues.stale);
    assert_eq!(issues.answered_at, Some(at(60)));
    assert!(emptied.ok && emptied.error.is_none());
}

/// A failed or partial answer to one capability keeps that history's last-good
/// reports and time, marks it, and touches no other history
/// (§FS-001-forge-interface.6.1).
#[test]
fn a_failed_answer_to_one_capability_keeps_its_last_good() {
    let full = answered_whole();
    let failed = commit_one(
        Some(&full),
        at(60),
        "issues",
        &["pull-requests", "issues"],
        Err(FAILED),
    );
    assert_eq!(keys(&failed), keys(&full), "no report is lost or invented");
    let issues = failed.history(Some("issues")).unwrap();
    assert_eq!(issues.asked_at, Some(at(60)), "asked, though it failed");
    assert_eq!(issues.answered_at, Some(at(0)), "last-good time kept");
    assert!(issues.stale, "its reports are last-good");
    assert_eq!(issues.error.as_deref(), Some("gdev: HTTP 503"));
    let pulls = failed.history(Some("pull-requests")).unwrap();
    assert_eq!(pulls, full.history(Some("pull-requests")).unwrap());
    assert!(!failed.ok && failed.stale);
    assert_eq!(failed.error.as_deref(), Some("gdev: HTTP 503"));
}

/// A source asked whole that did not answer completely rolls back whole:
/// every history keeps its last-good reports, is marked stale and failed, and
/// is stamped asked (§FS-001-forge-interface.6.1).
#[test]
fn a_source_asked_whole_rolls_back_every_history() {
    let full = answered_whole();
    let failed = commit_whole(Some(&full), at(60), Err(FAILED), &[Some("pull-requests")]);
    assert_eq!(keys(&failed), keys(&full));
    for history in &failed.histories {
        assert_eq!(history.asked_at, Some(at(60)), "{history:?}");
        assert_eq!(history.answered_at, Some(at(0)), "{history:?}");
        assert!(history.stale, "{history:?}");
        assert_eq!(history.error.as_deref(), Some("gdev: HTTP 503"));
    }
    assert_eq!(failed.histories.len(), 2, "no history dropped or added");
    // Read as the slot always read: one diagnosis, last-good time kept.
    assert_eq!(failed.error.as_deref(), Some("gdev: HTTP 503"));
    assert_eq!(failed.fetched_at, Some(at(0)));

    // With nothing to keep, failure is explicit and nothing is invented.
    let first = commit_whole(None, at(60), Err(FAILED), &[None]);
    assert!(first.matters.is_empty() && !first.ok && !first.stale);
    assert_eq!(first.histories.len(), 1);
    assert_eq!(first.histories[0].asked_at, Some(at(60)));
    assert_eq!(first.histories[0].answered_at, None);
}

/// A capability the slot has no history for is recorded as never asked, not
/// as an empty answer, and a whole answer from a source never probed gives
/// way to the declaration (§FS-001-forge-interface.6.1).
#[test]
fn a_capability_not_asked_is_recorded_never_asked() {
    let never_probed = commit_whole(None, at(0), Err(FAILED), &[None]);
    let narrowed = commit_one(
        Some(&never_probed),
        at(60),
        "issues",
        &["pull-requests", "issues", "notices"],
        Ok(vec![vec![report("a-issue")]]),
    );
    let capabilities: Vec<Option<&str>> = narrowed
        .histories
        .iter()
        .map(|history| history.capability.as_deref())
        .collect();
    assert_eq!(
        capabilities,
        [Some("pull-requests"), Some("issues"), Some("notices")]
    );
    assert!(!narrowed.history(Some("pull-requests")).unwrap().asked());
    assert!(!narrowed.history(Some("notices")).unwrap().asked());
    assert!(narrowed.history(Some("issues")).unwrap().answered());
    // Never asked is not failed, and not answered either.
    assert!(narrowed.ok);
    assert_eq!(narrowed.fetched_at, None);
}

/// Counts that do not add up — a slot written by hand — never lose a report.
#[test]
fn a_slot_whose_counts_do_not_add_up_keeps_every_report() {
    let mut odd = answered_whole();
    odd.matters.push(report("stray"));
    let narrowed = commit_one(
        Some(&odd),
        at(60),
        "issues",
        &["pull-requests", "issues"],
        Err(FAILED),
    );
    assert_eq!(narrowed.matters.len(), odd.matters.len());
}
