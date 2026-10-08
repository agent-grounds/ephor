//! What moved in a bound thread since hand-off: the draft's review
//! (§FS-005-dispatch.13.2). It explains a refusal and decides none —
//! [`Binding::freshness`] alone does.

use serde::Serialize;
use serde_json::{json, Value};

use super::binding::{fingerprints, threads, Binding};
use super::Record;
use crate::feed::model::Item;

/// One movement in the bound thread, named by the position the thread reading
/// prints for its message wherever that message is still shown
/// (§FS-005-dispatch.13.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Since {
    pub change: Change,
    pub message: Option<usize>,
    pub author: Option<String>,
    /// The time the source recorded for the message, as it recorded it.
    pub at: Option<String>,
    pub text: String,
    /// An edited message's words as the draft saw them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
}

/// The four kinds of movement (§FS-005-dispatch.13.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Change {
    New,
    Yours,
    Edited,
    NoLongerShown,
}

impl Since {
    fn of(change: Change, message: Option<usize>, saved: &Value) -> Self {
        Self {
            change,
            message,
            author: saved["author"].as_str().map(String::from),
            at: saved["when"].as_str().map(String::from),
            text: saved["text"].as_str().unwrap_or("").to_string(),
            before: None,
        }
    }

    fn author(&self) -> &str {
        self.author.as_deref().unwrap_or("unknown author")
    }

    fn position(&self) -> String {
        self.message.map(|n| format!("[{n}]")).unwrap_or_default()
    }

    /// The entry without anybody's words: what a ticket about the matter may
    /// say outside the dossier's fences (§FS-005-dispatch.5).
    pub fn named(&self) -> String {
        match (self.change, self.message) {
            (Change::Edited, _) => format!("{} edited {}", self.author(), self.position()),
            (Change::New, _) => format!("{} wrote {}", self.author(), self.position()),
            (Change::Yours, Some(_)) => format!("you wrote {}", self.position()),
            (Change::Yours, None) => "you sent a reply no refresh has shown yet".to_string(),
            (Change::NoLongerShown, _) => {
                format!("a message from {} is no longer shown", self.author())
            }
        }
    }

    /// The entry as the reading and the screen print it, one message's words
    /// to a line. A message no longer shown is called neither edited nor
    /// deleted, because ephor cannot tell which (§FS-005-dispatch.13.2).
    pub fn lines(&self) -> Vec<String> {
        let text = flat(&self.text);
        match (self.change, self.message) {
            (Change::Edited, _) => vec![
                self.named(),
                format!("  - {}", flat(self.before.as_deref().unwrap_or(""))),
                format!("  + {text}"),
            ],
            (Change::Yours, None) => vec![format!("you sent, not yet shown by a refresh: {text}")],
            (Change::NoLongerShown, _) => {
                vec![format!("no longer shown: {}: {text}", self.author())]
            }
            _ => vec![format!("{}: {text}", self.named())],
        }
    }

    fn label(&self) -> String {
        Binding::label(&json!({"author": self.author, "when": self.at, "text": self.text}))
    }
}

/// Words on one line, so a blank line inside a message never ends the list it
/// is printed in.
fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The review of `binding` against the conversation as last recorded
/// (§FS-005-dispatch.13.2). Ephor guesses nothing: a thread whose own
/// identity is missing or ambiguous, or whose messages cannot be lined up one
/// to one, lists nothing.
pub fn review(binding: &Binding, item: &Item, record: &Record) -> Vec<Since> {
    if binding.row != item.id || binding.source != item.source || binding.messages.is_empty() {
        return Vec::new();
    }
    let all: Vec<Vec<Value>> = threads(item).iter().map(fingerprints).collect();
    let Some(current) = all.get(binding.thread) else {
        return Vec::new();
    };
    let begins = current.starts_with(&binding.messages);
    let prefixed = all
        .iter()
        .filter(|messages| messages.starts_with(&binding.messages))
        .count();
    if prefixed > 1 || (prefixed == 1 && !begins) {
        return Vec::new();
    }
    // The position the reading prints: every message of the threads before
    // this one comes first.
    let offset: usize = all[..binding.thread].iter().map(Vec::len).sum();
    let mut since = match begins {
        true => arrivals(current, binding.messages.len(), offset),
        false => match aligned(&binding.messages, current, offset) {
            Some(since) => since,
            None => return Vec::new(),
        },
    };
    // A send accepted here is the person's own, listed once: as the message a
    // refresh made of it, or as itself until one does.
    if record.generation(binding) != binding.generation {
        if let Some(accepted) = record.accepted.get(&binding.key()) {
            let shown = since.iter().any(|entry| {
                entry.change == Change::Yours && entry.text.trim() == accepted.text.trim()
            });
            if !shown {
                since.push(Since {
                    change: Change::Yours,
                    message: None,
                    author: binding.user.clone(),
                    at: None,
                    text: accepted.text.clone(),
                    before: None,
                });
            }
        }
    }
    since
}

/// Everything from `from` on has arrived; the configured user's are theirs.
fn arrivals(current: &[Value], from: usize, offset: usize) -> Vec<Since> {
    current
        .iter()
        .enumerate()
        .skip(from)
        .map(|(index, message)| {
            let change = match message["mine"].as_bool() == Some(true) {
                true => Change::Yours,
                false => Change::New,
            };
            Since::of(change, Some(offset + index), message)
        })
        .collect()
}

/// Line the messages up by author and time, in order. `None` where that
/// cannot be done one to one: a time is missing, two messages share an author
/// and a time, the order changed, nothing lines up at all, or a message the
/// draft did not see sits among the ones it did.
fn aligned(saved: &[Value], current: &[Value], offset: usize) -> Option<Vec<Since>> {
    fn keys(messages: &[Value]) -> Option<Vec<(Value, String)>> {
        let keys: Vec<(Value, String)> = messages
            .iter()
            .map(|message| {
                Some((
                    message["author"].clone(),
                    message["when"].as_str()?.to_string(),
                ))
            })
            .collect::<Option<_>>()?;
        let unique = keys
            .iter()
            .enumerate()
            .all(|(index, key)| !keys[..index].contains(key));
        unique.then_some(keys)
    }
    let (was, now) = (keys(saved)?, keys(current)?);
    let partners: Vec<Option<usize>> = was
        .iter()
        .map(|key| now.iter().position(|other| other == key))
        .collect();
    let paired: Vec<usize> = partners.iter().flatten().copied().collect();
    let last = *paired.last()?;
    if paired.windows(2).any(|pair| pair[0] >= pair[1]) || paired.len() != last + 1 {
        return None;
    }
    let mut since = Vec::new();
    for (message, partner) in saved.iter().zip(&partners) {
        match partner {
            None => since.push(Since::of(Change::NoLongerShown, None, message)),
            Some(index) if current[*index]["text"] != message["text"] => since.push(Since {
                before: Some(message["text"].as_str().unwrap_or("").to_string()),
                ..Since::of(Change::Edited, Some(offset + index), &current[*index])
            }),
            Some(_) => {}
        }
    }
    since.extend(arrivals(current, last + 1, offset));
    Some(since)
}

/// The one-line stale reason, naming the review's most telling entry
/// (§FS-005-dispatch.13.2): an edit, else the accepted send or the first
/// arrival as before, else the first message no longer shown. Where the review
/// lists nothing the reason freshness gave stands as it was.
pub fn reason(review: &[Since], advanced: bool, decided: String) -> String {
    if review.is_empty() {
        return decided;
    }
    let edit = review.iter().find(|entry| entry.change == Change::Edited);
    let arrival = review.iter().find(|entry| {
        matches!(entry.change, Change::New | Change::Yours) && entry.message.is_some()
    });
    let gone = review
        .iter()
        .find(|entry| entry.change == Change::NoLongerShown);
    let named = match (edit, arrival, gone) {
        (Some(edit), _, _) => format!(
            "Draft is stale: {} edited {} after it was drafted",
            edit.author(),
            edit.position()
        ),
        _ if advanced => decided,
        (_, Some(arrival), _) => format!("Draft is stale: {}", arrival.label()),
        (_, _, Some(gone)) => format!("Draft is stale: no longer shown: {}", gone.label()),
        _ => return decided,
    };
    match review.len() {
        1 => named,
        more => format!("{named} (and {} more)", more - 1),
    }
}
