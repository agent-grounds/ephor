//! `ephor guide`: how work gets into the shapes ephor reads, asked of the
//! ephor that will read it (§FS-020-guide).
//!
//! A reading like any other (§AR-009-surfaces): it returns a view and changes
//! nothing. The `gate` topic is the runtime binding's text, because what a
//! gate's brief holds is the runtime's plan language (§FS-020-guide.2); the
//! `matter` topic is ephor's own (§FS-020-guide.3).

use crate::api::views;
use crate::error::{EphorError, Result};
use crate::work::recipe::WorkConfig;
use crate::work::runtime::{gate, guide};

/// Every topic, in the order `ephor guide` lists them, with the one line that
/// says what each explains (§FS-020-guide.1).
pub const TOPICS: [(&str, &str); 2] = [
    ("gate", "how a workflow prepares the question its gate asks"),
    (
        "matter",
        "what makes a matter new, and how a workflow becomes its proposal",
    ),
];

/// The `matter` topic, in ephor's own words, with no runtime named
/// (§FS-020-guide.3).
const MATTER: &str = "\
MATTER — what makes a matter new, and how a workflow becomes its proposal

A matter is anything the feed shows: an issue, a pull request, a CI run, a
task. Waiting lists it under New when nothing has been done about it:
  - the feed shows it and it is not finished;
  - it is not processed: the work ledger records no work about it, and no
    dismissal of it still stands;
  - it is not itself a ticket of the runtime's own task store; and
  - it is not held: no selector of the site excludes a label it carries,
    such as `\"labels\": [\"!human-discussion\"]`. A held matter is listed
    apart, with the label that holds it, and offered nothing.

Processed means one of two records, both in the ledger. Work laid about the
matter, by a key, `ephor work process`, `ephor work lay`, `ephor actions run`
or a recipe's own sweep; or a dismissal, `ephor work dismiss`, with a reason.
A dismissal is lifted the moment the matter moves: a new comment, commit or
state changes its fingerprint, and it is new again. Nothing is written to
lift it.

The proposal is the work ephor would lay. It is the first entry of the
matter's menu, in menu order, that hands work over: a recipe, or an action
that names a `workflow`, whose `when` selector in the site configuration
accepts the matter. An entry the menu refuses, or one with work already going
about the matter, is passed over. Where no entry accepts it, the proposal is
the runtime's `triage` workflow, or says that there is none. So a workflow
becomes the proposal for a kind of matter by an entry like this, placed before
any other entry that would accept the same matters:
  { \"id\": \"fix-task\", \"description\": \"fix this task end to end\",
    \"workflow\": \"supervised-ticket-fix\",
    \"when\": { \"kinds\": [\"issue\"], \"roles\": [\"author\"] },
    \"inputs\": { \"ticket\": \"{title}\" } }

Commands:
  ephor actions --item <id>                    the menu, the proposal first
  ephor waiting                                New, held, and what waits on you
  ephor work process --item <id> --dry-run     what processing would lay
  ephor work dismiss --item <id> --reason <text>   record that it needs none
";

/// The guide, or one topic of it (§FS-020-guide.1). With no topic, the list
/// and how to ask for one; a topic it does not know is refused, naming the
/// topics there are. For `gate` the runtime's own request and answer schemas
/// ride along, or the sentence why there are none (§FS-020-guide.4).
pub fn guide(config: &WorkConfig, topic: Option<&str>) -> Result<views::Guide> {
    let topics = TOPICS
        .iter()
        .map(|(name, summary)| views::GuideTopic {
            name: name.to_string(),
            summary: summary.to_string(),
        })
        .collect();
    let mut view = views::Guide {
        topic: topic.map(str::to_string),
        topics,
        text: String::new(),
        schemas: None,
        schemas_unavailable: None,
    };
    match topic {
        None => {
            let mut text = String::from("Topics:\n");
            for (name, summary) in TOPICS {
                text.push_str(&format!("  {name:<8}{summary}\n"));
            }
            text.push_str("\nAsk for one with `ephor guide <topic>`; add --json for the view.\n");
            view.text = text;
        }
        Some("gate") => {
            view.text = guide::gate(config);
            match gate::schemas(config) {
                Ok((request, answer)) => {
                    view.schemas = Some(views::GuideSchemas { request, answer });
                }
                Err(why) => view.schemas_unavailable = Some(why),
            }
        }
        Some("matter") => view.text = MATTER.to_string(),
        Some(other) => {
            let names: Vec<&str> = TOPICS.iter().map(|(name, _)| *name).collect();
            return Err(EphorError::Command(format!(
                "no guide topic '{other}'; the topics are {}",
                names.join(", ")
            )));
        }
    }
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No topic lists them all; an unknown one is refused, naming them
    /// (§FS-020-guide.1).
    #[test]
    fn the_topics_are_listed_and_an_unknown_one_is_refused_by_name() {
        let config = WorkConfig::default();
        let view = guide(&config, None).unwrap();
        assert_eq!(view.topics.len(), 2);
        assert!(view.text.contains("gate"), "{}", view.text);
        assert!(view.text.contains("ephor guide <topic>"), "{}", view.text);
        assert_eq!(view.schemas, None);
        let err = guide(&config, Some("nope")).unwrap_err().to_string();
        assert_eq!(err, "no guide topic 'nope'; the topics are gate, matter");
    }

    /// The matter topic covers new, held, processed and the proposal, and
    /// fits an 80-column screen (§FS-020-guide.3).
    #[test]
    fn the_matter_topic_says_new_processed_and_the_proposal() {
        let view = guide(&WorkConfig::default(), Some("matter")).unwrap();
        for part in [
            "new",
            "held",
            "Processed",
            "dismissal",
            "proposal",
            "triage",
            "ephor actions --item",
            "ephor work process --item <id> --dry-run",
            "ephor work dismiss",
        ] {
            assert!(view.text.contains(part), "{part} missing:\n{}", view.text);
        }
        assert!(view.text.lines().all(|line| line.chars().count() <= 80));
        // One screen, 80 columns by 48 lines (§FS-020-guide.1).
        assert!(view.text.trim_end().lines().count() <= 48, "{}", view.text);
        assert_eq!(view.schemas_unavailable, None);
    }
}
