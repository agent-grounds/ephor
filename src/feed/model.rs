use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Status,
    Pr,
    Ci,
    Issue,
    /// The project's own task, read out of a store in its checkout
    /// (§FS-006-project-interface.7). Not an issue: an issue is what a forge
    /// files, and this is the project's own work
    /// (§FS-003-feed-categories.1).
    Task,
    Message,
}

impl ItemKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "status" => Some(ItemKind::Status),
            "pr" => Some(ItemKind::Pr),
            "ci" => Some(ItemKind::Ci),
            "issue" => Some(ItemKind::Issue),
            "task" => Some(ItemKind::Task),
            "message" => Some(ItemKind::Message),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ItemKind::Status => "status",
            ItemKind::Pr => "pr",
            ItemKind::Ci => "ci",
            ItemKind::Issue => "issue",
            ItemKind::Task => "task",
            ItemKind::Message => "msg",
        }
    }
}

/// The names forges give a state that means the work is over
/// (§FS-003-feed-categories.2). Matched as substrings because forges spell
/// them differently and compose them (`open:changes_requested`, `CLOSED`).
const TERMINAL_STATES: [&str; 5] = ["closed", "merged", "done", "resolved", "declined"];

/// Whether a state means the work is over (§FS-003-feed-categories.2). Free of
/// [`Item`] so the model can ask it of a matter without building a report to
/// ask it about — the two must answer the same way or a row lands in one
/// category and settles by another.
pub fn is_terminal(state: Option<&str>) -> bool {
    let state = state.unwrap_or("").to_lowercase();
    TERMINAL_STATES.iter().any(|needle| state.contains(needle))
}

/// Where a settled report keeps the answer it was owed
/// (§FS-003-feed-categories.2). Finishing clears `needs_response`, because a
/// finished item is news and not a task; this is the same fact kept as news,
/// which is what tells the finished work that still has a loose end from the
/// finished work that has none.
pub const UNANSWERED: &str = "unanswered";

/// The reserved `raw` key carrying why a waiting matter waits, where that is
/// more than its conversation (§FS-005-dispatch.31.2). Written only where a
/// reason other than the conversation contributes, because a matter that waits
/// and records no reason waits on its conversation.
pub const AWAITS: &str = "awaits";

/// The reserved key on one of `raw`'s threads that says that discussion awaits
/// the reader, by the calculus of §FS-003-feed-categories.4. Written by the
/// forge policy, which alone sees whose each message is, and read by the
/// row's file mark, which counts only the discussions that wait
/// (§FS-007-matters.3). Absent is a discussion that does not wait. It is
/// ephor's own annotation rather than a source's word (§AR-006-matters.1).
pub const THREAD_AWAITS: &str = "awaits_reader";

/// One reason a matter waits on the reader (§FS-005-dispatch.31.2): its
/// conversation, or that it is an issue nobody holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Awaiting {
    Conversation,
    Unclaimed,
}

impl Awaiting {
    pub fn label(self) -> &'static str {
        match self {
            Awaiting::Conversation => "conversation",
            Awaiting::Unclaimed => "unclaimed",
        }
    }
}

/// Why a report waits, read from its flag and its `raw`
/// (§FS-005-dispatch.31.2). Shared by the flat item and the fold, which adds
/// up the reasons of every report of one matter.
pub(crate) fn awaiting(needs_response: bool, raw: &Value) -> Vec<Awaiting> {
    if !needs_response {
        return Vec::new();
    }
    let mut reasons: Vec<Awaiting> = raw
        .get(AWAITS)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|reason| serde_json::from_value(reason.clone()).ok())
        .collect();
    reasons.sort();
    reasons.dedup();
    if reasons.is_empty() {
        reasons.push(Awaiting::Conversation);
    }
    reasons
}

/// A discussion's newest turn (§FS-007-matters.3): its last message together
/// with the messages its author sent just before it, as a photo is followed
/// by "is this the one?".
pub fn newest_turn(messages: &[Value]) -> &[Value] {
    let Some(last) = messages.last() else {
        return messages;
    };
    let author = last.get("author");
    let start = messages
        .iter()
        .rposition(|message| message.get("author") != author)
        .map_or(0, |earlier| earlier + 1);
    &messages[start..]
}

/// The original metadata overlay remains readable but is not evidence of
/// provenance: legacy JSON could already contain it (§FS-006-project-interface.4).
pub(crate) const SOURCE_METADATA: &str = "_ephor";
pub(crate) const CUSTOM_STATUS_ANSWER: &str = "custom_status_answer";

/// The custom-status adapter owns this record at its ingestion boundary. Any
/// input at this key is nested under `passthrough` before the adapter writes
/// `answer`; legacy JSON and summary-only answers never author that member.
/// Escaping rather than reserving an input key keeps every JSON value intact
/// without changing the public Item/Matter layouts or cache model
/// (§FS-006-project-interface.4).
const CUSTOM_STATUS_ORIGIN: &str = "_ephor_custom_status";

pub(crate) fn custom_status_raw(mut raw: Value, answer: Option<Value>) -> Value {
    if let Some(raw) = raw.as_object_mut() {
        let mut origin = serde_json::Map::new();
        if let Some(passthrough) = raw.remove(CUSTOM_STATUS_ORIGIN) {
            origin.insert("passthrough".to_string(), passthrough);
        }
        if let Some(answer) = answer {
            origin.insert("answer".to_string(), answer);
        }
        if !origin.is_empty() {
            raw.insert(CUSTOM_STATUS_ORIGIN.to_string(), Value::Object(origin));
        }
    }
    raw
}

/// Read only the adapter-authored record, never the legacy metadata overlay.
/// Other providers' free data cannot acquire custom-status semantics
/// (§FS-006-project-interface.4).
pub(crate) fn custom_status_answer<'a>(source: &str, raw: &'a Value) -> Option<&'a Value> {
    if source != "custom-status" {
        return None;
    }
    raw.get(CUSTOM_STATUS_ORIGIN)?.get("answer")
}

/// Finality explicitly stated by a structured source. Passthrough that merely
/// uses the same public field name has no authority
/// (§FS-003-feed-categories.2).
pub(crate) fn source_terminal(source: &str, raw: &Value) -> Option<bool> {
    custom_status_answer(source, raw)?
        .get("terminal")?
        .as_bool()
}

/// Record on a report's passthrough that an answer was missing when the
/// subject finished. Written wherever settling clears the response it owed, so
/// that clearing it does not also forget it (§FS-003-feed-categories.2).
pub fn note_unanswered(raw: &mut Value) {
    match raw {
        Value::Object(map) => {
            map.insert(UNANSWERED.to_string(), Value::Bool(true));
        }
        // A report that carried nothing else still carries this.
        Value::Null => *raw = serde_json::json!({ UNANSWERED: true }),
        _ => {}
    }
}

/// Why a finished item is still in front of the reader
/// (§FS-003-feed-categories.2). Finished work with none of these is over in
/// every sense the reader cares about and leaves the feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LooseEnd {
    /// An answer is missing — whatever would have made the subject await one
    /// while it was still open (§FS-003-feed-categories.4).
    Unanswered,
    /// The gate went the other way, after the merge or before it.
    RedGate,
    /// The runtime still holds a ticket about this matter
    /// (§FS-005-dispatch.23). Not a fact of any report: the ledger's, so a
    /// surface that reads the ledger adds it and the model never guesses it.
    Working,
}

/// Whether an item is the user's own work or something they are reviewing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemRole {
    Author,
    Reviewer,
}

/// One entry in a project's information stream. `id` must be stable across
/// fetches (it is the unread-tracking key), so providers derive it from
/// natural identifiers (repo#number, ticket key, message timestamp) — never
/// from array positions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub project: String,
    pub source: String,
    pub kind: ItemKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<ItemRole>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    pub needs_response: bool,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub raw: Value,
}

/// The reserved `raw` key carrying what one source said about one matter
/// (§AR-006-matters, §FS-005-dispatch.8). Named here, beside the model it
/// rides in, because the seam that fills it and the four surfaces that read it
/// must be spelling the same word.
pub const META: &str = "meta";

/// One `meta` value in the spelling every surface uses: a string as written, a
/// number or a boolean by its canonical form (§FS-005-dispatch.31.1). `None`
/// for anything else, which the bound already refused.
///
/// One spelling, here, because the selector compares it, a template renders
/// it, a summons hands it over and the bound measures it — and a value that
/// meant one thing to the bound and another to the selector would be a key
/// that stopped matching for no reason a reader could see.
pub fn spelled(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(_) | Value::Bool(_) => Some(value.to_string()),
        _ => None,
    }
}

/// The most a carried `meta` value may render to (§FS-005-dispatch.8).
/// Identifiers, not prose: this is the size a selector compares, a path
/// carries and a process environment holds.
pub const META_VALUE_CAP: usize = 1024;

/// `[A-Za-z_][A-Za-z0-9_-]*` — the `meta` keys §FS-005-dispatch.8 admits,
/// which is what keeps every one of them nameable in a template and in a
/// summons.
///
/// Here rather than in the reader that reports its drops, because the bound is
/// a property of the map and holds wherever the map is read: a source with no
/// channel to report a drop on is still bounded, silently.
pub fn is_nameable(key: &str) -> bool {
    let mut letters = key.chars();
    letters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && letters.all(|letter| letter.is_ascii_alphanumeric() || letter == '_' || letter == '-')
}

/// One `meta` entry held to the bound §FS-005-dispatch.8 states, in the one
/// spelling every surface reads: a key a shell will take, a scalar value on
/// one line, at most `META_VALUE_CAP` bytes rendered. `None` for an entry the
/// bound refuses.
pub fn bounded_entry(key: &str, value: &Value) -> Option<String> {
    if !is_nameable(key) {
        return None;
    }
    let rendered = spelled(value)?;
    if rendered.contains('\n') || rendered.len() > META_VALUE_CAP {
        return None;
    }
    Some(rendered)
}

impl Item {
    /// The work is over: the item belongs under Recent rather than in its own
    /// category (§FS-003-feed-categories.2).
    pub fn is_finished(&self) -> bool {
        source_terminal(&self.source, &self.raw)
            .unwrap_or_else(|| is_terminal(self.state.as_deref()))
    }

    /// The first-class issue dependencies that still block this item
    /// (§FS-003-feed-categories.4). Missing or unknown state is unfinished by
    /// the same degrade rule as every other free-form forge state; only an
    /// explicitly terminal state releases it.
    pub fn open_blockers(&self) -> Vec<String> {
        let mut blockers: Vec<String> = self
            .raw
            .get("blocked_by")
            .and_then(Value::as_array)
            .map(|dependencies| {
                dependencies
                    .iter()
                    .filter(|dependency| {
                        !is_terminal(dependency.get("status").and_then(Value::as_str))
                    })
                    .filter_map(|dependency| dependency.get("key").and_then(Value::as_str))
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        blockers.sort();
        blockers.dedup();
        blockers
    }

    /// Whether the forge says another unfinished issue must land first.
    pub fn is_blocked(&self) -> bool {
        !self.open_blockers().is_empty()
    }

    /// One sentence fragment for rows, dossiers, and work refusals.
    pub fn blocking_reason(&self) -> Option<String> {
        let blockers = self.open_blockers();
        (!blockers.is_empty()).then(|| format!("blocked by {}", blockers.join(", ")))
    }

    /// Whether a finished item is recent enough to still be shown, against a
    /// window in days. A window of zero shows nothing finished
    /// (§FS-003-feed-categories.3).
    pub fn within_recent_window(&self, now: DateTime<Utc>, recent_days: u64) -> bool {
        recent_days > 0 && (now - self.updated_at).num_days() < recent_days as i64
    }

    /// What this finished item still leaves its reader to do
    /// (§FS-003-feed-categories.2). None while it is unfinished — such an item
    /// is in the feed because of its category, not because of this — and None
    /// on finished work that asks nothing, which is most of it.
    ///
    /// Two of the three the spec lists are facts of the report and are read
    /// here. The third, work still open on the matter, belongs to the ledger
    /// and is added by the surfaces that read one.
    pub fn loose_end(&self) -> Option<LooseEnd> {
        if !self.is_finished() {
            return None;
        }
        if self
            .raw
            .get(UNANSWERED)
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Some(LooseEnd::Unanswered);
        }
        crate::feed::gate::Gate::of(self)
            .is_some_and(|gate| gate.is_red())
            .then_some(LooseEnd::RedGate)
    }

    /// In the feed at all: unfinished work always; finished work only while it
    /// still has a loose end (§FS-003-feed-categories.2) whose last activity is
    /// inside the recency window (§FS-003-feed-categories.3).
    ///
    /// Work the runtime still holds open keeps a matter here too, and is not
    /// bounded by the window — but it is the ledger's fact rather than the
    /// report's, so it is added where a ledger is in hand rather than guessed
    /// at from a row.
    pub fn is_visible(&self, now: DateTime<Utc>, recent_days: u64) -> bool {
        if !self.is_finished() {
            return true;
        }
        self.within_recent_window(now, recent_days) && self.loose_end().is_some()
    }

    /// The repository, best effort: `raw.repo`, or the `owner/name` between
    /// the source prefix and `#` in the id (`github-prs:acme/widget#42`).
    /// Never out of a conversation's id, which is the gateway's own word for
    /// it and only happens to be spelled that way (§AR-003-attribution.1).
    pub fn repo(&self) -> Option<String> {
        if let Some(repo) = self.raw.get("repo").and_then(Value::as_str) {
            return Some(repo.to_string());
        }
        if self.raw.get("conversation").and_then(Value::as_bool) == Some(true) {
            return None;
        }
        let tail = self.id.split_once(':')?.1;
        let (repo, _) = tail.rsplit_once('#')?;
        if repo.contains('/') {
            Some(repo.to_string())
        } else {
            None
        }
    }

    /// What this matter's own source said about *this matter*
    /// (§FS-005-dispatch.8): the third reserved `raw` key, after `assignees`
    /// and `labels`, and the one accessor every surface reads it through
    /// (§AR-006-matters).
    ///
    /// `None` where the source reported no such map at all — **absent rather
    /// than empty**, which is the distinction the selector's silence rule
    /// turns on (§FS-005-dispatch.31.1). Each value comes back in the one
    /// spelling a selector compares, a template renders and a summons hands
    /// over, so a number or a boolean the source did not quote answers by its
    /// canonical spelling.
    ///
    /// The bound is applied here and not only where a reader reports its
    /// drops, because it is a property of the map rather than of one reader
    /// (§FS-005-dispatch.8): a source whose free passthrough puts a whole
    /// paragraph or a key no shell will take under this name reaches the same
    /// four surfaces, and has no channel to be told on — so the entry is
    /// dropped silently rather than carried. A reader that has such a channel
    /// still reports what it drops, which is where the *once* is.
    pub fn meta(&self) -> Option<std::collections::BTreeMap<String, String>> {
        Some(
            self.raw
                .get(META)?
                .as_object()?
                .iter()
                .filter_map(|(key, value)| Some((key.clone(), bounded_entry(key, value)?)))
                .collect(),
        )
    }

    /// Why the matter waits on the reader (§FS-005-dispatch.31.2): empty where
    /// it does not wait, and the conversation where it waits and recorded no
    /// reason, which was the only reason a matter waited before the others were
    /// told apart.
    pub fn awaiting(&self) -> Vec<Awaiting> {
        awaiting(self.needs_response, &self.raw)
    }

    /// The one mark a row shows of the files on its conversation, `📎N`
    /// (§FS-007-matters.3): N is the number of files on the newest turn of
    /// each discussion that awaits the reader, summed. Nothing while the row
    /// does not wait, and nothing where those turns carry no file — a logo
    /// on a mail already answered is nothing to do (§GOAL-002-glance).
    pub fn files_mark(&self) -> Option<String> {
        if !self.needs_response {
            return None;
        }
        let files: usize = self
            .raw
            .get("threads")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|thread| thread.get(THREAD_AWAITS).and_then(Value::as_bool) == Some(true))
            .filter_map(|thread| thread.get("messages").and_then(Value::as_array))
            .flat_map(|messages| newest_turn(messages))
            .map(|message| {
                message
                    .get("attachments")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len)
            })
            .sum();
        (files > 0).then(|| format!("📎{files}"))
    }

    /// The pull request or issue number, best effort: the digits after the
    /// last `#` (`github-prs:acme/widget#42`) or the last `/`
    /// (`forge-prs:repo/123`) of the id.
    pub fn number(&self) -> Option<String> {
        for separator in ['#', '/'] {
            if let Some((_, tail)) = self.id.rsplit_once(separator) {
                if !tail.is_empty() && tail.bytes().all(|byte| byte.is_ascii_digit()) {
                    return Some(tail.to_string());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(state: Option<&str>, updated_at: DateTime<Utc>) -> Item {
        Item {
            id: "x".to_string(),
            project: "p".to_string(),
            source: "s".to_string(),
            kind: ItemKind::Issue,
            role: None,
            title: "t".to_string(),
            url: None,
            state: state.map(String::from),
            needs_response: false,
            updated_at,
            raw: Value::Null,
        }
    }

    /// The bound holds of the map wherever it is read, not only where a reader
    /// has a channel to report a drop on (§FS-005-dispatch.8). A source whose
    /// free passthrough puts a paragraph, an over-cap value or a key no shell
    /// will take under `meta` reaches the same four surfaces as any other, and
    /// a multi-line value in a process environment is exactly what the bound
    /// exists to keep out — so the accessor drops them, silently.
    #[test]
    fn the_accessor_holds_the_bound_whatever_the_source_put_under_meta() {
        let mut matter = item(Some("open"), Utc::now());
        matter.raw = serde_json::json!({
            META: {
                "context": "acme-labs",
                "bad key": "no shell will take this name",
                "prose": "line one\nline two",
                "essay": "x".repeat(META_VALUE_CAP + 1),
                "owners": ["a", "b"],
                "tier": 1,
            }
        });
        let said = matter.meta().expect("the source reported a map");
        assert_eq!(
            said.keys().collect::<Vec<_>>(),
            vec!["context", "tier"],
            "an unbounded entry reached a surface: {said:?}"
        );
        assert_eq!(said["context"], "acme-labs");
        assert_eq!(said["tier"], "1");
    }

    /// Absent rather than empty is the distinction the selector's silence rule
    /// turns on (§FS-005-dispatch.31.1), and the bound does not blur it: a map
    /// every one of whose keys the bound refused is still a map the source
    /// reported.
    #[test]
    fn a_source_that_reported_no_map_is_absent_and_one_wholly_refused_is_empty() {
        let mut matter = item(Some("open"), Utc::now());
        assert!(matter.meta().is_none());
        matter.raw = serde_json::json!({ META: { "bad key": "v" } });
        assert_eq!(matter.meta().map(|said| said.len()), Some(0));
    }

    #[test]
    fn terminal_states_are_recognized_however_the_forge_spells_them() {
        for state in ["closed", "CLOSED", "open:merged", "Done", "declined"] {
            assert!(item(Some(state), Utc::now()).is_finished(), "{state}");
        }
        for state in ["open", "open:changes_requested", "in progress"] {
            assert!(!item(Some(state), Utc::now()).is_finished(), "{state}");
        }
        assert!(!item(None, Utc::now()).is_finished());
    }

    #[test]
    fn the_recency_window_is_a_span_of_days_and_zero_closes_it() {
        let now = Utc::now();
        let two_days_ago = item(Some("closed"), now - chrono::Duration::days(2));
        assert!(two_days_ago.within_recent_window(now, 7));
        assert!(!two_days_ago.within_recent_window(now, 1));
        assert!(!two_days_ago.within_recent_window(now, 0));
    }

    /// Finished work is in the feed only while it still leaves something to do
    /// (§FS-003-feed-categories.2). The merge that went as asked is over: it is
    /// not news anybody has to clear, and Recent is not a list of it.
    #[test]
    fn a_finished_item_with_nothing_left_to_do_leaves_the_feed_at_once() {
        let now = Utc::now();
        let merged = item(Some("merged"), now - chrono::Duration::hours(2));
        assert_eq!(merged.loose_end(), None);
        assert!(
            !merged.is_visible(now, 7),
            "inside the window and still over"
        );

        // Unfinished work is in the feed because of its category, and this
        // question is never asked of it.
        let open = item(Some("open"), now - chrono::Duration::days(400));
        assert_eq!(open.loose_end(), None);
        assert!(open.is_visible(now, 7));
    }

    /// The two loose ends a report knows: an answer that was missing when the
    /// subject finished, and a gate that went the other way
    /// (§FS-003-feed-categories.2).
    #[test]
    fn a_finished_item_stays_while_an_answer_is_missing_or_the_gate_is_red() {
        let now = Utc::now();

        let mut commented = item(Some("merged"), now - chrono::Duration::hours(2));
        note_unanswered(&mut commented.raw);
        assert_eq!(commented.loose_end(), Some(LooseEnd::Unanswered));
        assert!(commented.is_visible(now, 7));

        let mut red = item(Some("merged"), now - chrono::Duration::hours(2));
        red.raw = serde_json::json!({ "gate": { "repos": [
            { "repo": "acme/widget", "passed": 3, "failed": 1 }
        ] } });
        assert_eq!(red.loose_end(), Some(LooseEnd::RedGate));
        assert!(red.is_visible(now, 7));

        // A gate that went green is not a loose end, whatever else it says.
        let mut green = item(Some("merged"), now - chrono::Duration::hours(2));
        green.raw = serde_json::json!({ "gate": { "repos": [
            { "repo": "acme/widget", "passed": 4 }
        ] } });
        assert_eq!(green.loose_end(), None);
        assert!(!green.is_visible(now, 7));
    }

    /// The window still bounds a loose end the report knows: a conversation
    /// nobody answered a year ago is not this week's work
    /// (§FS-003-feed-categories.3).
    #[test]
    fn a_loose_end_outside_the_window_leaves_with_everything_else() {
        let now = Utc::now();
        let mut old = item(Some("closed"), now - chrono::Duration::days(30));
        note_unanswered(&mut old.raw);
        assert_eq!(old.loose_end(), Some(LooseEnd::Unanswered));
        assert!(!old.is_visible(now, 7));
        assert!(old.is_visible(now, 90));
    }

    /// The mark goes on a report that carried nothing else as readily as on one
    /// that carried a conversation: a notice is the thinnest report there is,
    /// and it is exactly the report that says somebody is waiting.
    #[test]
    fn the_unanswered_mark_lands_on_a_report_that_carried_nothing() {
        let mut raw = Value::Null;
        note_unanswered(&mut raw);
        assert_eq!(raw[UNANSWERED], serde_json::json!(true));

        let mut carrying = serde_json::json!({ "repo": "acme/widget" });
        note_unanswered(&mut carrying);
        assert_eq!(carrying["repo"], serde_json::json!("acme/widget"));
        assert_eq!(carrying[UNANSWERED], serde_json::json!(true));
    }

    /// Issue relationships, not labels or prose, decide whether the item is
    /// blocked (§FS-003-feed-categories.4). Only a terminal prerequisite
    /// releases it; duplicates cannot make the same ticket appear twice.
    #[test]
    fn open_issue_dependencies_are_the_blockers() {
        let mut dependent = item(Some("open"), Utc::now());
        dependent.raw = serde_json::json!({
            "blocked_by": [
                { "key": "acme/widget#9", "status": "open" },
                { "key": "acme/widget#8", "status": "closed" },
                { "key": "acme/widget#9" }
            ],
            "labels": ["state:blocked"]
        });
        assert_eq!(dependent.open_blockers(), ["acme/widget#9"]);
        assert!(dependent.is_blocked());
        assert_eq!(
            dependent.blocking_reason().as_deref(),
            Some("blocked by acme/widget#9")
        );

        dependent.raw = serde_json::json!({ "labels": ["state:blocked"] });
        assert!(!dependent.is_blocked(), "a label is not a dependency");
    }

    #[test]
    fn issue_is_a_kind_of_its_own() {
        assert_eq!(ItemKind::parse("issue"), Some(ItemKind::Issue));
        assert_eq!(ItemKind::Issue.label(), "issue");
    }

    /// The project's own task is its own kind, and not an issue: an issue is
    /// what a forge files, and a task is the project's own work in its own
    /// checkout (§FS-003-feed-categories.1). The label is what a serialized
    /// matter carries and what a recipe's `kinds` matches, so it round-trips.
    #[test]
    fn task_is_a_kind_of_its_own() {
        assert_eq!(ItemKind::parse("task"), Some(ItemKind::Task));
        assert_eq!(ItemKind::Task.label(), "task");
        assert_ne!(ItemKind::Task, ItemKind::Issue);
        assert_eq!(
            serde_json::to_value(ItemKind::Task).unwrap(),
            serde_json::json!("task")
        );
    }

    /// The row's file mark counts the newest turn of each discussion that
    /// awaits the reader, as one token whatever the number, and says nothing
    /// once the row waits on nobody (§FS-007-matters.3).
    #[test]
    fn the_row_marks_the_files_on_the_newest_turn_of_each_waiting_discussion() {
        use serde_json::json;
        let files = |count: usize| {
            json!((0..count)
                .map(|n| json!({ "name": format!("f{n}") }))
                .collect::<Vec<_>>())
        };
        let mut row = item(Some("open"), Utc::now());
        row.needs_response = true;
        row.raw = json!({ "threads": [
            // Waiting: the newest turn is Dana's photo and her caption after
            // it; her earlier file, before the reader's word, is not in it.
            { THREAD_AWAITS: true, "messages": [
                { "author": "dana", "text": "first", "attachments": files(5) },
                { "author": "me", "text": "ok", "attachments": [] },
                { "author": "dana", "text": "", "attachments": files(10) },
                { "author": "dana", "text": "Is this the one?", "attachments": files(1) },
                { "author": "dana", "text": "or this?" }
            ] },
            // Waiting, with a file on its one-message turn.
            { THREAD_AWAITS: true, "messages": [
                { "author": "eli", "text": "see attached", "attachments": files(1) }
            ] },
            // Not waiting: the reader's own file on their own last word.
            { "messages": [
                { "author": "fay", "text": "?" },
                { "author": "me", "text": "here", "attachments": files(3) }
            ] }
        ] });
        assert_eq!(row.files_mark().as_deref(), Some("📎12"));

        let mut answered = row.clone();
        answered.needs_response = false;
        assert_eq!(answered.files_mark(), None);

        let mut wordy = row.clone();
        wordy.raw["threads"][0]["messages"][3]["attachments"] = json!([]);
        wordy.raw["threads"][0]["messages"][2]
            .as_object_mut()
            .unwrap()
            .remove("attachments");
        wordy.raw["threads"][1]["messages"][0]
            .as_object_mut()
            .unwrap()
            .remove("attachments");
        assert_eq!(wordy.files_mark(), None, "no file on a waiting turn");
    }

    #[test]
    fn a_turn_is_the_last_message_and_what_its_author_sent_just_before() {
        use serde_json::json;
        let said = |author: &str| json!({ "author": author, "text": "…" });
        assert!(newest_turn(&[]).is_empty());
        let one = [said("dana")];
        assert_eq!(newest_turn(&one).len(), 1);
        let messages = [said("dana"), said("me"), said("dana"), said("dana")];
        assert_eq!(newest_turn(&messages), &messages[2..]);
        let all = [said("dana"), said("dana")];
        assert_eq!(newest_turn(&all).len(), 2);
    }
}
