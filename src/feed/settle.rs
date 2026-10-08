//! Settling a conversation at its source (§FS-001-forge-interface.1): the one
//! write that acts on a whole conversation rather than on a message in it, sent
//! back to the source that reported it wherever that source is bound
//! (§FS-001-forge-interface.9).
//!
//! What settling means is the source's — a mail thread archived, a chat marked
//! done. Ephor sends the conversation's own `id`, exactly as the source
//! reported it, and reads nothing back but whether the source accepted.

use serde_json::Value;

use crate::error::{EphorError, Result};
use crate::feed::config::Defaults;
use crate::feed::model::{Item, ItemKind};
use crate::feed::providers::{built_in, forge_call, Sources};
use crate::forge::{Forge, Request};

/// A settle every check has passed, one call short of the source. A dry run
/// is the move stopped here, so it refuses wherever the move would
/// (§FS-011-command-line.4).
pub struct Prepared {
    /// The source that will be asked.
    pub source: String,
    /// The conversation's own id, as that source reported it.
    pub id: String,
    forge: Box<dyn Forge>,
    request: Request,
}

impl Prepared {
    /// Ask the source to settle the conversation. A conversation already
    /// settled there is settled again, and that is success.
    pub fn send(&self) -> Result<()> {
        self.forge
            .settle(&self.request, &self.id)
            .map_err(|err| EphorError::Command(err.to_string()))
    }
}

/// The sentence every surface refuses a source that cannot settle with,
/// whatever the row is (§FS-011-command-line.4).
pub fn cannot(source: &str) -> String {
    format!("{source} cannot settle a conversation at its source")
}

/// Everything the move checks before it sends, in the order the refusals
/// rank (§FS-011-command-line.4): a source that did not declare `settle` is
/// refused in its own name whatever the row is, and only a declaring source's
/// row that is not one of its conversations is refused as such.
///
/// One of ephor's own providers, and a project's own task store, are never
/// reached through the forge interface, so they are refused without a call.
/// Any other source is asked `capabilities`, the read every capability check
/// makes (§FS-001-forge-interface.1); nothing that writes is sent.
pub fn prepare(item: &Item, sources: &Sources, defaults: &Defaults) -> Result<Prepared> {
    if built_in(&item.source) || item.kind == ItemKind::Task {
        return Err(EphorError::Command(cannot(&item.source)));
    }
    let (forge, request) = forge_call(sources, &item.source, defaults)
        .map_err(|err| EphorError::Command(err.to_string()))?;
    let declared = forge
        .capabilities()
        .map_err(|err| EphorError::Command(err.to_string()))?;
    if !declared.settle {
        return Err(EphorError::Command(cannot(&item.source)));
    }
    let id = conversation_id(item).ok_or_else(|| {
        EphorError::Command(format!(
            "{} is not a conversation; {} settles only the conversations it reported \
             under Messages by reason",
            item.id, item.source
        ))
    })?;
    Ok(Prepared {
        source: item.source.clone(),
        id,
        forge,
        request,
    })
}

/// The id a source gave a conversation it reported under Messages by reason:
/// the row's key without the source's prefix (§FS-001-forge-interface.1).
/// Anything else that source reported — a pull request, an issue, a notice —
/// has no such id.
fn conversation_id(item: &Item) -> Option<String> {
    if item.kind != ItemKind::Message
        || item.raw.get("conversation").and_then(Value::as_bool) != Some(true)
    {
        return None;
    }
    item.id
        .strip_prefix(&format!("{}:", item.source))
        .filter(|id| !id.is_empty())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(kind: ItemKind, id: &str, raw: Value) -> Item {
        Item {
            id: id.to_string(),
            project: "demo".to_string(),
            source: "mail-me".to_string(),
            kind,
            role: None,
            title: "Quote for the garden fence".to_string(),
            url: None,
            state: None,
            needs_response: false,
            updated_at: "2026-10-07T09:00:00Z".parse().unwrap(),
            raw,
        }
    }

    /// The id goes back exactly as the source reported it, prefix and all
    /// removed, and only a conversation has one.
    #[test]
    fn only_a_conversation_has_an_id_to_settle() {
        let conversation = row(
            ItemKind::Message,
            "mail-me:k-B",
            json!({ "conversation": true }),
        );
        assert_eq!(conversation_id(&conversation).as_deref(), Some("k-B"));

        let notice = row(ItemKind::Message, "mail-me:n-1", json!({}));
        assert_eq!(conversation_id(&notice), None);
        let issue = row(
            ItemKind::Issue,
            "mail-me:k-B",
            json!({ "conversation": true }),
        );
        assert_eq!(conversation_id(&issue), None);
        let foreign = row(
            ItemKind::Message,
            "other:k-B",
            json!({ "conversation": true }),
        );
        assert_eq!(conversation_id(&foreign), None);
    }
}
