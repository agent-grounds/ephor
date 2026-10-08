//! Settling a conversation at its source, as a move (§FS-011-command-line.4):
//! one implementation the key `s` and `ephor settle` both call, so the same
//! row is refused with the same sentence from either (§REQ-002-parity).

use chrono::{DateTime, Utc};

use crate::error::Result;
use crate::feed::cache;
use crate::feed::model::Item;

use super::act::Sending;
use super::{views, Session};

impl Session {
    /// Ask the source that reported a conversation to put it away
    /// (§FS-011-command-line.4, §FS-001-forge-interface.1). A source that did
    /// not declare `settle` is refused in its own name whatever the row is,
    /// and a declaring source's row that is not a conversation is refused as
    /// one.
    ///
    /// [`Sending::Dry`] is the move stopped one call short: it asks the same
    /// `capabilities` and refuses wherever the move would, then says which
    /// source it would ask about which conversation. A row already read, or
    /// already settled, is asked again rather than refused — the source's
    /// answer to a repeat is success, and ephor never reads a settled state
    /// back (§FS-001-forge-interface.3).
    ///
    /// Once the source accepts, the row is marked done exactly as `m` marks it,
    /// so the next message brings it back saying why
    /// (§FS-003-feed-categories.4). Settled is not answered: whether a thread
    /// awaits the reader is still read off its messages.
    pub fn settle(&mut self, item: &Item, sending: Sending) -> views::Outcome {
        let sources = self.sources_for(&item.project);
        let prepared = match crate::feed::settle::prepare(item, &sources, &self.config.defaults) {
            Ok(prepared) => prepared,
            Err(err) => return views::Outcome::refused(err.to_string()),
        };
        if sending == Sending::Dry {
            return views::Outcome::ok(format!(
                "would ask {} to settle “{}”",
                prepared.source, item.title
            ));
        }
        if let Err(err) = prepared.send() {
            return views::Outcome::refused(err.to_string());
        }
        // The remote move is done, and asking again is harmless; a mark that
        // could not be written is said rather than swallowed, so a program
        // knows the row is still in front of the reader.
        match self.mark_done([(item.id.clone(), item.updated_at)]) {
            Ok(()) => views::Outcome::ok(format!("✓ settled at {}", prepared.source)),
            Err(err) => views::Outcome::refused(format!(
                "settled at {}, but the row could not be marked done: {err}",
                prepared.source
            )),
        }
    }

    /// Mark rows done, remembering what each matter looked like so the row
    /// can say what moved when it comes back (§FS-007-matters.5). The one
    /// write behind `m` and behind an accepted settle, so the two leave the
    /// same mark.
    pub fn mark_done(
        &mut self,
        marks: impl IntoIterator<Item = (String, DateTime<Utc>)>,
    ) -> Result<()> {
        for (id, updated_at) in marks {
            let mark = self
                .matter(&id)
                .map(|matter| cache::Mark::of(&matter))
                .unwrap_or_else(|| cache::Mark::at(updated_at));
            self.resurfacing.remove(&id);
            self.seen.insert(id, mark);
        }
        cache::store_seen(&self.seen)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use serde_json::{json, Value};

    use super::*;
    use crate::feed::cache::{ProjectFeed, ProviderSlot};
    use crate::feed::model::ItemKind;
    use crate::matter::Matter;

    const CARRIER: &str = r#"#!/bin/sh
here=$(dirname "$0")
cat > "$here/$1.json"
echo "$1" >> "$here/calls"
case "$1" in
  capabilities) cat "$here/caps.json" ;;
  settle)
    if [ -e "$here/refuse" ]; then echo "the network refused" >&2; exit 1; fi
    printf '{}' ;;
  *) printf '[]' ;;
esac
"#;

    /// A session holding one conversation of `mail`, an out-of-process
    /// carrier declaring `capabilities`, with the seen store kept beside it.
    struct World {
        tmp: tempfile::TempDir,
        session: Session,
        item: Item,
        matter: Matter,
        _seen: cache::TestSeen,
    }

    impl World {
        fn new(capabilities: Value) -> World {
            let tmp = tempfile::tempdir().unwrap();
            let _seen = cache::use_test_seen(tmp.path().join("seen.json"));
            let carrier = tmp.path().join("carrier");
            fs::write(&carrier, CARRIER).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&carrier, fs::Permissions::from_mode(0o755)).unwrap();
            }
            fs::write(tmp.path().join("caps.json"), capabilities.to_string()).unwrap();
            let item = conversation("mail", "mail:k-B");
            let matter = Matter::of_item(&item);
            let mut session = Session::default();
            session.provider_blocks.insert(
                "demo".into(),
                vec![json!({ "provider": "mail", "command": carrier, "user": "me" })],
            );
            session.feeds.push(ProjectFeed {
                project: "demo".into(),
                model: cache::MODEL,
                providers: BTreeMap::from([(
                    "mail".to_string(),
                    ProviderSlot {
                        ok: true,
                        matters: vec![matter.clone()],
                        ..ProviderSlot::default()
                    },
                )]),
                ..ProjectFeed::default()
            });
            World {
                tmp,
                session,
                item,
                matter,
                _seen,
            }
        }

        fn calls(&self) -> Vec<String> {
            fs::read_to_string(self.tmp.path().join("calls"))
                .unwrap_or_default()
                .lines()
                .map(String::from)
                .collect()
        }

        fn stored(&self) -> Option<Value> {
            stored(&self.tmp.path().join("seen.json"))
        }
    }

    fn stored(path: &Path) -> Option<Value> {
        let text = fs::read_to_string(path).ok()?;
        Some(serde_json::from_str(&text).unwrap())
    }

    fn conversation(source: &str, id: &str) -> Item {
        Item {
            id: id.to_string(),
            project: "demo".to_string(),
            source: source.to_string(),
            kind: ItemKind::Message,
            role: None,
            title: "Quote for the garden fence".to_string(),
            url: None,
            state: None,
            needs_response: true,
            updated_at: "2026-10-07T09:00:00Z".parse().unwrap(),
            raw: json!({ "conversation": true, "threads": [{ "messages": [
                { "author": "dana", "text": "Did the quote arrive?",
                  "when": "2026-10-07T09:00:00Z" }
            ] }] }),
        }
    }

    /// One of ephor's own providers is never reached through the forge
    /// interface, so it is refused in its own name before anything is called
    /// (§FS-011-command-line.4).
    #[test]
    fn a_built_in_source_is_refused_by_name_and_nothing_is_called() {
        let mut world = World::new(json!({ "messages": true, "settle": true }));
        let row = conversation("github-threads", "github-threads:acme/widget#42");
        for sending in [Sending::Now, Sending::Dry] {
            let outcome = world.session.settle(&row, sending);
            assert!(!outcome.ok);
            assert_eq!(
                outcome.says,
                "github-threads cannot settle a conversation at its source"
            );
        }
        assert!(world.calls().is_empty(), "{:?}", world.calls());
        assert!(world.session.seen.is_empty() && world.stored().is_none());
    }

    /// A source that declared something else — `archive` among them — is
    /// refused with the same sentence, and sent nothing that writes
    /// (§FS-001-forge-interface.1).
    #[test]
    fn an_undeclared_settle_is_refused_by_name_and_sends_nothing() {
        let mut world = World::new(json!({ "messages": true, "archive": true }));
        let item = world.item.clone();
        for sending in [Sending::Now, Sending::Dry] {
            let outcome = world.session.settle(&item, sending);
            assert!(!outcome.ok);
            assert_eq!(
                outcome.says,
                "mail cannot settle a conversation at its source"
            );
        }
        assert_eq!(world.calls(), ["capabilities", "capabilities"]);
        assert!(world.session.seen.is_empty() && world.stored().is_none());
    }

    /// A declaring source's row that is not one of its conversations is
    /// refused by name, and the source is sent nothing that writes.
    #[test]
    fn a_row_that_is_not_a_conversation_is_refused_by_name() {
        let mut world = World::new(json!({ "issues": true, "settle": true }));
        let mut issue = world.item.clone();
        issue.id = "mail:acme/widget#7".into();
        issue.kind = ItemKind::Issue;
        issue.raw = json!({});
        let outcome = world.session.settle(&issue, Sending::Now);
        assert!(!outcome.ok);
        assert!(
            outcome
                .says
                .contains("mail:acme/widget#7 is not a conversation"),
            "{}",
            outcome.says
        );
        assert!(!world.calls().iter().any(|call| call == "settle"));
        assert!(world.session.seen.is_empty() && world.stored().is_none());
    }

    /// A dry run asks what the move asks and stops before the send
    /// (§FS-011-command-line.4).
    #[test]
    fn a_dry_run_stops_before_the_send() {
        let mut world = World::new(json!({ "messages": true, "settle": true }));
        let item = world.item.clone();
        let outcome = world.session.settle(&item, Sending::Dry);
        assert!(outcome.ok, "{}", outcome.says);
        assert_eq!(
            outcome.says,
            "would ask mail to settle “Quote for the garden fence”"
        );
        assert_eq!(world.calls(), ["capabilities"]);
        assert!(world.session.seen.is_empty() && world.stored().is_none());
    }

    /// The row is marked done — the print `m` writes — only once the source
    /// accepts, and a repeat is asked again rather than refused
    /// (§FS-003-feed-categories.4, §FS-001-forge-interface.1).
    #[test]
    fn the_row_is_marked_done_only_when_the_source_accepts() {
        let mut world = World::new(json!({ "messages": true, "settle": true }));
        let item = world.item.clone();
        fs::write(world.tmp.path().join("refuse"), "").unwrap();
        let refused = world.session.settle(&item, Sending::Now);
        assert!(!refused.ok);
        assert!(
            refused.says.contains("the network refused"),
            "{}",
            refused.says
        );
        assert!(world.session.seen.is_empty() && world.stored().is_none());

        fs::remove_file(world.tmp.path().join("refuse")).unwrap();
        world
            .session
            .resurfacing
            .insert(item.id.clone(), "⟳ the conversation moved".into());
        let accepted = world.session.settle(&item, Sending::Now);
        assert!(accepted.ok, "{}", accepted.says);
        assert_eq!(accepted.says, "✓ settled at mail");
        let mark = serde_json::to_value(cache::Mark::of(&world.matter)).unwrap();
        assert_eq!(
            serde_json::to_value(&world.session.seen[&item.id]).unwrap(),
            mark
        );
        assert_eq!(world.stored(), Some(json!({ item.id.clone(): mark })));
        assert!(!world.session.resurfacing.contains_key(&item.id));

        let again = world.session.settle(&item, Sending::Now);
        assert!(again.ok, "{}", again.says);
        let sent = world
            .calls()
            .iter()
            .filter(|call| *call == "settle")
            .count();
        assert_eq!(sent, 3, "{:?}", world.calls());
        let request = stored(&world.tmp.path().join("settle.json")).unwrap();
        assert_eq!(request["target"], "k-B");
        assert_eq!(request["project"], "demo");
    }
}
