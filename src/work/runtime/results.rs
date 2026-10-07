//! What a run left behind, read back out of the work root
//! (§AR-007-runtime.1).
//!
//! Two things are read: the verdict line a finished ticket wrote, and the
//! **proposed answer** a ticket about a conversation wrote instead of posting
//! one (§FS-005-dispatch.13). Both are files the runtime's own states produce,
//! found by where they sit and read for what they say — nothing here writes
//! work state, which stays the runtime's (§FS-005-dispatch.4).

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{EphorError, Result};

/// Where the runtime keeps everything a run left behind, relative to the work
/// root. The directories below it are the runtime's to name and can be added
/// to, so a carry-over sweeps them rather than a list of the ones this module
/// reads itself (§FS-005-dispatch.3.1).
const RUNTIME: &str = "runtime";

/// Where the shipped states put what a run wrote, relative to the work root.
const ARTIFACTS: &str = "runtime/ephor";

/// Where the runtime itself keeps a ticket's result — the message a terminal
/// move carried, appended as one `## Result` entry per move — relative to the
/// work root, keyed by the ticket's project-qualified id. The runtime's own
/// ledger, read and never written here.
const RESULTS: &str = "runtime/results";

/// The result the runtime recorded for a ticket, where it recorded one: the
/// last entry's first line — which for a ticket the reader took back is the
/// reason they gave (§FS-005-dispatch.16). None where the runtime wrote
/// nothing, or wrote an empty file to satisfy its own link.
pub fn result(root: &Path, plan_id: &str, ticket: &str) -> Option<String> {
    let path = root.join(RESULTS).join(format!("{plan_id}.{ticket}.md"));
    let text = fs::read_to_string(path).ok()?;
    let last_entry = text.rsplit("## Result").next().unwrap_or(&text);
    last_entry
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
}

/// The verdict a finished ticket left behind, as the state machine ephor ships
/// asks for it. Found by what it says rather than by where it sits: an agent
/// asked for a document writes a document, and its first line is a heading.
/// Absent while the work has not reached that state, which is not a failure.
pub fn verdict(root: &Path, plan_id: &str, ticket: &str) -> Option<String> {
    let path = root
        .join(ARTIFACTS)
        .join(format!("{plan_id}.{ticket}.verdict.md"));
    let text = fs::read_to_string(path).ok()?;
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("VERDICT:"))
        .map(|line| line.trim_start_matches("VERDICT:").trim())
        .or_else(|| text.lines().map(str::trim).find(|line| !line.is_empty()))?;
    Some(line.to_string())
}

/// A reply a run drafted and did not send (§FS-005-dispatch.13). It is a file
/// and stays one until a person posts it: the proposal is materials, never an
/// act (§REQ-001-boundary.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Absent on legacy drafts, which remain copyable (§FS-005-dispatch.13).
    pub binding: Option<crate::replies::Binding>,
    /// The reply as it would be posted, exactly as the run wrote it.
    pub text: String,
    /// Where it sits — what the reader copies from where nothing can post it,
    /// and what they edit before posting where something can.
    pub path: PathBuf,
}

/// Where a plan's proposed answer belongs. One per matter rather than one per
/// ticket: what a reader posts is the answer to the conversation, and a second
/// pass over the same conversation supersedes the first rather than adding to
/// it.
pub fn reply_path(root: &Path, plan_id: &str) -> PathBuf {
    root.join(ARTIFACTS).join(format!("{plan_id}.reply.md"))
}

/// Every request writes its own answer; late older output never replaces the
/// latest request (§FS-005-dispatch.13, §AR-007-runtime.1).
pub fn request_reply_path(root: &Path, plan_id: &str, ticket: &str) -> PathBuf {
    root.join(ARTIFACTS)
        .join(format!("{plan_id}.{ticket}.reply.md"))
}

/// Read only the chosen request's file, including absent/withdrawn output;
/// never fall back to an earlier proposal (§FS-005-dispatch.13).
pub fn proposal_at(path: PathBuf, binding: Option<crate::replies::Binding>) -> Option<Proposal> {
    let text = fs::read_to_string(&path).ok()?;
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(Proposal {
        text: text.to_string(),
        path,
        binding,
    })
}

/// Retirement follows durable confirmation, preserving the established suffix
/// for each request's output (§FS-005-dispatch.13).
pub fn mark_path_posted(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let posted = path.with_file_name(format!(
        "{}.posted.md",
        name.strip_suffix(".md").unwrap_or(&name)
    ));
    fs::rename(path, posted)
        .map_err(|err| EphorError::Command(format!("Cannot move {}: {err}", path.display())))
}

/// Where a posted proposal is moved to. Posting is the one deliberate move
/// (§FS-005-dispatch.13), and a proposal that stayed offered after it was sent
/// would invite sending it twice.
fn posted_path(root: &Path, plan_id: &str) -> PathBuf {
    root.join(ARTIFACTS)
        .join(format!("{plan_id}.reply.posted.md"))
}

/// The proposed answer a run left for this plan, or None where it left none —
/// which is every ticket that was not about a conversation, and every answer
/// ticket that has not finished.
pub fn proposal(root: &Path, plan_id: &str) -> Option<Proposal> {
    let path = reply_path(root, plan_id);
    let text = fs::read_to_string(&path).ok()?;
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(Proposal {
        binding: None,
        text: text.to_string(),
        path,
    })
}

/// Every entry the runtime keyed by the plan stem `from` under this root,
/// paired with where the same entry belongs under the stem `to`
/// (§FS-005-dispatch.3.1).
///
/// Swept out of **every** directory the runtime keeps rather than out of a list
/// of the two this module reads itself: a result, an artifact ephor's own states
/// wrote, an export one ticket handed another — this module reads what a run
/// left behind and cannot know every file a machine put there, and a file left
/// at a name nothing names any more is exactly what the carry-over is for. A
/// directory named after the stem moves whole, which is how an export's
/// contents come with it.
///
/// The stem is matched with its separator, because the name the digest renders
/// begins with the name it replaces — without the `.` a carried-over file would
/// be carried over a second time. That same separator is what leaves a past
/// run's transcript alone: it is named after the invocation, so the stem is
/// inside its name rather than at the start of it.
pub fn carried_over(root: &Path, from: &str, to: &str) -> Vec<(PathBuf, PathBuf)> {
    let mut moves = Vec::new();
    let Ok(kept) = fs::read_dir(root.join(RUNTIME)) else {
        return moves;
    };
    for dir in kept.flatten() {
        let dir = dir.path();
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(rest) = name.strip_prefix(&format!("{from}.")) else {
                continue;
            };
            moves.push((dir.join(&name), dir.join(format!("{to}.{rest}"))));
        }
    }
    moves.sort();
    moves
}

/// Record that a proposal was posted, by moving it aside. The file is kept
/// rather than deleted: it is what was said in the reader's name.
pub fn mark_posted(root: &Path, plan_id: &str) -> Result<()> {
    let from = reply_path(root, plan_id);
    if !from.is_file() {
        return Ok(());
    }
    fs::rename(&from, posted_path(root, plan_id))
        .map_err(|err| EphorError::Command(format!("Cannot move {}: {err}", from.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifacts(root: &Path) -> PathBuf {
        let dir = root.join(ARTIFACTS);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The runtime's own result entry comes back as the ticket's line: the
    /// last entry, its first line — the reason a cancel carried
    /// (§FS-005-dispatch.16) — and nothing where the file is empty.
    #[test]
    fn the_runtimes_result_entry_is_read_back_last_entry_first_line() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(RESULTS);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("widget-42.fix-gate-2.md"),
            "## Result\n\nfirst move\n\n## Result\n\nasked twice by mistake\nmore words\n",
        )
        .unwrap();
        assert_eq!(
            result(tmp.path(), "widget-42", "fix-gate-2").as_deref(),
            Some("asked twice by mistake")
        );
        fs::write(dir.join("widget-42.fix-gate-3.md"), "").unwrap();
        assert_eq!(result(tmp.path(), "widget-42", "fix-gate-3"), None);
        assert_eq!(result(tmp.path(), "widget-42", "nothing-1"), None);
    }

    #[test]
    fn a_verdict_is_read_back_without_its_label() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = artifacts(tmp.path());
        // As an agent asked for a document actually writes one: a heading
        // first, and the verdict in the body.
        fs::write(
            dir.join("widget-42.fix-gate-1.verdict.md"),
            "# widget-42.fix-gate-1 — review verdict\n\n\
             VERDICT: blocked — the failing job needs a credential\n\n## What was done\n",
        )
        .unwrap();
        assert_eq!(
            verdict(tmp.path(), "widget-42", "fix-gate-1").as_deref(),
            Some("blocked — the failing job needs a credential")
        );
        assert!(verdict(tmp.path(), "widget-42", "nothing-1").is_none());
    }

    /// The proposal is read whole: it is a reply, and a reply summarized is a
    /// different reply (§FS-005-dispatch.13).
    #[test]
    fn a_proposed_reply_is_read_whole_and_says_where_it_sits() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = artifacts(tmp.path());
        assert_eq!(proposal(tmp.path(), "widget-42"), None);

        fs::write(
            dir.join("widget-42.reply.md"),
            "\nYes — the retry window is per attempt.\n\nThe test covers it.\n\n",
        )
        .unwrap();
        let found = proposal(tmp.path(), "widget-42").expect("a reply was drafted");
        assert_eq!(
            found.text,
            "Yes — the retry window is per attempt.\n\nThe test covers it."
        );
        assert_eq!(found.path, reply_path(tmp.path(), "widget-42"));

        // A file the run created and wrote nothing into is not a proposal.
        fs::write(dir.join("widget-43.reply.md"), "   \n\n").unwrap();
        assert_eq!(proposal(tmp.path(), "widget-43"), None);
    }

    /// Posted once: what was sent is kept, and what is left is not offered
    /// again (§FS-005-dispatch.13).
    #[test]
    fn posting_a_proposal_moves_it_aside_and_keeps_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = artifacts(tmp.path());
        fs::write(dir.join("widget-42.reply.md"), "posted words").unwrap();

        mark_posted(tmp.path(), "widget-42").unwrap();
        assert_eq!(proposal(tmp.path(), "widget-42"), None);
        assert_eq!(
            fs::read_to_string(posted_path(tmp.path(), "widget-42")).unwrap(),
            "posted words"
        );
        // Nothing to move is not an error: the reader may have posted from
        // another surface, or the run may never have written one.
        mark_posted(tmp.path(), "widget-42").unwrap();
    }
}
