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
        &[Some("pull-requests"), Some("issues")],
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
        &[Some("pull-requests"), Some("issues")],
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
        &[Some("pull-requests"), Some("issues")],
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
        &[Some("pull-requests"), Some("issues"), Some("notices")],
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
        &[Some("pull-requests"), Some("issues")],
        Err(FAILED),
    );
    assert_eq!(narrowed.matters.len(), odd.matters.len());
}

/// Declarations survive whole-source rollback, independently of report
/// ownership (§FS-001-forge-interface.6.1, §FS-011-command-line.12).
#[test]
fn a_failed_whole_answer_keeps_new_requirements_and_removed_reports() {
    let issues = commit_whole(
        None,
        at(0),
        Ok(vec![(Some("issues"), vec![vec![report("ticket")]])]),
        &[Some("issues")],
    );
    let failed = commit_whole(
        Some(&issues),
        at(60),
        Err(FAILED),
        &[Some("pull-requests"), Some("issues")],
    );
    let pulls = failed.history(Some("pull-requests")).unwrap();
    assert!(pulls.required() && !pulls.answered());
    assert_eq!(pulls.asked_at, Some(at(60)));
    assert_eq!(pulls.answered_at, None);
    assert_eq!(pulls.error.as_deref(), Some(FAILED.message));
    assert_eq!(keys(&failed), ["ticket"]);
    assert_eq!(
        failed.history(Some("issues")).unwrap().answered_at,
        Some(at(0))
    );

    // The reverse change removes the requirement even on a failed full run.
    let removed = commit_whole(Some(&failed), at(120), Err(FAILED), &[Some("issues")]);
    assert!(!removed.history(Some("pull-requests")).unwrap().required());
    assert_eq!(removed.required_histories().count(), 1);
    assert_eq!(keys(&removed), ["ticket"]);

    // Removing a declaration on a narrowed run never removes its old reports
    // or lets an obsolete never-asked placeholder hold back age/mark-read.
    for previous in [answered_whole(), failed] {
        let narrowed = commit_one(
            Some(&previous),
            at(180),
            "issues",
            &[Some("issues")],
            Ok(vec![vec![]]),
        );
        let old_pulls = previous.history(Some("pull-requests")).unwrap();
        let mut retained = old_pulls.clone();
        retained.retained_only = true;
        assert_eq!(narrowed.history(Some("pull-requests")), Some(&retained));
        let feed = crate::feed::cache::ProjectFeed {
            providers: std::collections::BTreeMap::from([("gdev".to_string(), narrowed)]),
            ..Default::default()
        };
        assert_eq!(crate::feed::age::age(Some(&feed), &["gdev"]), Some(at(180)));
        assert!(!crate::feed::age::never_asked(Some(&feed), &["gdev"]));
    }
}

/// A real whole history can hold opaque reports; a first capability commit
/// preserves both healthy and failed last-good metadata, without inventing
/// a PR answer (§FS-001-forge-interface.6.1).
#[test]
fn a_nonempty_whole_history_survives_the_first_capability_answer() {
    let mut whole = commit_whole(
        None,
        at(0),
        Ok(vec![(
            None,
            vec![vec![report("opaque-a")], vec![report("opaque-b")]],
        )]),
        &[None],
    );
    whole.histories[0].cursor = Some("position".to_string());
    for previous in [
        whole.clone(),
        commit_whole(
            Some(&whole),
            at(60),
            Err(Failure {
                message: "destination unreachable",
                unreachable: true,
            }),
            &[None],
        ),
    ] {
        let narrowed = commit_one(
            Some(&previous),
            at(120),
            "issues",
            &[Some("pull-requests"), Some("issues")],
            Ok(vec![vec![report("ticket")], vec![]]),
        );
        assert_eq!(keys(&narrowed), ["opaque-a", "ticket", "opaque-b"]);
        let mut retained = previous.history(None).unwrap().clone();
        retained.retained_only = true;
        assert_eq!(narrowed.history(None), Some(&retained));
        assert!(!narrowed.history(Some("pull-requests")).unwrap().asked());
        assert!(!narrowed.history(Some("pull-requests")).unwrap().answered());
        assert_eq!(
            serde_json::to_value(&narrowed.matters[0]).unwrap(),
            serde_json::to_value(&previous.matters[0]).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&narrowed.matters[2]).unwrap(),
            serde_json::to_value(&previous.matters[1]).unwrap()
        );

        // A later empty selected answer replaces only its own reports. It
        // cannot resurrect the previous selected answer from retained data.
        let empty = commit_one(
            Some(&narrowed),
            at(180),
            "issues",
            &[Some("pull-requests"), Some("issues")],
            Ok(vec![vec![], vec![]]),
        );
        assert_eq!(keys(&empty), ["opaque-a", "opaque-b"]);
        assert_eq!(empty.history(None), Some(&retained));
        assert!(empty.history(Some("issues")).unwrap().answered());
    }
}

/// The stale marker follows contributing histories, including reports merged
/// across capabilities and sources (§FS-001-forge-interface.6.1).
#[test]
fn a_fresh_issue_does_not_inherit_an_unrelated_pr_failure() {
    let full = answered_whole();
    let failed = commit_whole(
        Some(&full),
        at(60),
        Err(FAILED),
        &[Some("pull-requests"), Some("issues")],
    );
    let fresh = commit_one(
        Some(&failed),
        at(120),
        "issues",
        &[Some("pull-requests"), Some("issues")],
        Ok(vec![vec![report("ticket")]]),
    );
    let mut feed = crate::feed::cache::ProjectFeed {
        providers: std::collections::BTreeMap::from([("gdev".to_string(), fresh)]),
        ..Default::default()
    };
    assert!(feed.is_stale(&report("a-pr").as_item()));
    assert!(!feed.is_stale(&report("ticket").as_item()));
    // An unrelated failure in a different source does not taint the ticket.
    let other = commit_whole(
        None,
        at(0),
        Ok(vec![(None, vec![vec![report("other")]])]),
        &[None],
    );
    let stale = commit_whole(Some(&other), at(60), Err(FAILED), &[None]);
    feed.providers.insert("other".to_string(), stale);
    assert!(!feed.is_stale(&report("ticket").as_item()));
    // But a stale contributor to this subject does, even if the merged row
    // takes the fresh report's source and content.
    let mut other_report = report("ticket");
    other_report.source = "other".to_string();
    let ticket = commit_whole(
        None,
        at(0),
        Ok(vec![(None, vec![vec![other_report]])]),
        &[None],
    );
    feed.providers.insert(
        "other".to_string(),
        commit_whole(Some(&ticket), at(60), Err(FAILED), &[None]),
    );
    let rows = feed.matters();
    let ticket = rows
        .iter()
        .find(|matter| matter.key.as_str() == "ticket")
        .unwrap();
    assert!(feed.is_stale(&ticket.as_item()));
    assert_eq!(
        rows.iter()
            .filter(|matter| matter.key.as_str() == "ticket")
            .count(),
        1
    );

    // A weakly placed site report stays its own subject even though its flat
    // rendering also carries a repository and number (§FS-008-attribution.3).
    let mut weak = report("weak");
    weak.raw = serde_json::json!({"repo": "G", "number": "7"});
    weak.placement =
        crate::matter::Placement::claimed("widget", crate::attribution::Strength::Fallback);
    let whole = commit_whole(
        None,
        at(0),
        Ok(vec![(None, vec![vec![weak.clone()]])]),
        &[None],
    );
    feed.providers.insert(
        "site".to_string(),
        commit_whole(Some(&whole), at(60), Err(FAILED), &[None]),
    );
    assert!(feed.is_stale(&weak.as_item()));
}

/// An attempted question following a failed probe is not a new successful
/// declaration (§FS-001-forge-interface.6.1).
#[test]
fn a_failed_probe_cannot_declare_the_attempted_capability() {
    let previous = commit_whole(
        None,
        at(0),
        Ok(vec![(Some("issues"), vec![vec![]])]),
        &[Some("issues")],
    );
    let failed = commit_one(
        Some(&previous),
        at(60),
        "pull-requests",
        &[Some("issues")],
        Err(FAILED),
    );
    assert!(!failed.history(Some("pull-requests")).unwrap().required());
    assert!(!failed.history(Some("pull-requests")).unwrap().answered());
    assert_eq!(failed.required_histories().count(), 1);
}
