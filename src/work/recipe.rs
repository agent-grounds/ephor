//! Recipes: which items deserve work, and what the ticket asks for
//! (§FS-005-dispatch.1).
//!
//! A recipe is a selector and a brief. The selector is ephor's own vocabulary
//! — kind, role, gate, whether a response is owed — so one recipe matches the
//! same items whichever forge reported them
//! (§FS-001-forge-interface.3). The brief is the reader's, rendered with the
//! item's fields where it names them.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::feed::gate::Gate;
use crate::feed::model::{Item, ItemKind, ItemRole};

/// The work half of the feed configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkConfig {
    /// Where an item's plan is written, as a template over the whole
    /// vocabulary the ticket itself is rendered from
    /// ([`crate::work::dossier::Subject::placeholders`]) rather than the three
    /// names this said for a long time: `{workspace}` is the checkout the item
    /// resolves to, `{root}` the project root, `{project}` its id, and
    /// `{title}`, `{branch}`, `{ticket}`, `{number}`, `{org}`, `{org_root}`
    /// and the rest are nameable here too (§FS-005-dispatch.6.1). The site's
    /// answer; an organization's displaces it, and a project's displaces both.
    ///
    /// Enumeration resolves this without an item in hand, so a template naming
    /// a field only an item can fill is written to at dispatch and skipped by
    /// the board's walk (§FS-005-dispatch.15.1).
    #[serde(default = "default_root")]
    pub root: String,
    /// A states YAML to install into a work root that has none, instead of the
    /// one ephor ships.
    #[serde(default)]
    pub states: Option<String>,
    /// What runs a plan. The runtime is a binding with one shipped wired and
    /// ready (§FS-005-dispatch lead, §DA-001-runtime-bound-default); naming
    /// another here is how a person who works differently points work at it.
    #[serde(default)]
    pub runner: Option<String>,
    /// Recipes, appended to the shipped ones; one reusing a shipped id
    /// replaces it (§FS-005-dispatch.1).
    #[serde(default)]
    pub recipes: Vec<Recipe>,
    /// Who does which action, by the action's own id, with [`DEFAULT_HAND`]
    /// answering for every id the table does not name
    /// (§FS-006-project-interface.9). The site's answer, which a project's
    /// table displaces. An entry is one hand or an ordered list of them.
    #[serde(default)]
    pub hands: BTreeMap<String, HandList>,
    /// What reports each pool's headroom: a pool id, and the command that says
    /// what that provider has left (§FS-005-dispatch.29). Unbound by default —
    /// a pool nothing here names is *unknown*, which demotes nobody, and the
    /// ledger's own record of a refusal keeps working with this table empty.
    /// How any one vendor is asked stays outside ephor: what ships beside it is
    /// a worked example per vendor (§REQ-001-boundary.5).
    #[serde(default)]
    pub headroom: BTreeMap<String, String>,
    /// The fraction of a window at or below which a pool counts as spent
    /// (§FS-005-dispatch.29). `0` unless the site names another, which is what
    /// makes the rule a veto over what a provider actually refused rather than
    /// a preference about what is comfortable.
    #[serde(default)]
    pub headroom_floor: Option<f64>,
    /// A file naming which items `ephor work dispatch` opens first: one item
    /// id per line, most important first. `--ranking <path>` displaces it for
    /// one run (§FS-005-dispatch.26).
    #[serde(default)]
    pub ranking: Option<String>,
    /// The most autorun roots that may be live across the site. Omitted is
    /// unlimited; zero pauses new autorun starts (§FS-005-dispatch.24).
    #[serde(default)]
    pub max_concurrent: Option<usize>,
    /// The most autorun roots that may be *working* across the site — the
    /// live ones, less those parked on a person's answer
    /// (§FS-005-dispatch.24). Read exactly as [`WorkConfig::max_concurrent`]
    /// is: omitted is unlimited, zero pauses new autorun starts. Omitting it
    /// is the default, so a site that names only the other key is bounded
    /// exactly as it was.
    #[serde(default)]
    pub max_active: Option<usize>,
    /// The most money autorun may spend across the site in a trailing window,
    /// measured against the reading `burn` publishes
    /// (§FS-015-spend-ceiling.1). Read exactly as the two ceilings above are:
    /// omitted is unlimited, `0` admits no new autorun starts.
    #[serde(default)]
    pub max_spend: Option<crate::work::spend::SpendBudget>,
    /// The same ceiling on tokens, which are always measured where a dollar
    /// figure is only ever the one a log carried (§FS-015-spend-ceiling.2). A
    /// site may write both, and the first of them to be full is what refuses.
    #[serde(default)]
    pub max_tokens: Option<crate::work::spend::TokenBudget>,
}

impl Default for WorkConfig {
    fn default() -> Self {
        WorkConfig {
            root: default_root(),
            states: None,
            runner: None,
            recipes: Vec::new(),
            hands: BTreeMap::new(),
            headroom: BTreeMap::new(),
            headroom_floor: None,
            ranking: None,
            max_concurrent: None,
            max_active: None,
            max_spend: None,
            max_tokens: None,
        }
    }
}

fn default_root() -> String {
    format!("{{workspace}}/{}", crate::work::runtime::plan::PROJECT_DIR)
}

/// Work per project: extra recipes, and a work root of its own.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectWorkConfig {
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub states: Option<String>,
    #[serde(default)]
    pub recipes: Vec<Recipe>,
    /// Who does which action on this project — read before the site's table,
    /// this action's id before [`DEFAULT_HAND`] (§FS-006-project-interface.9).
    /// An entry is one hand or an ordered list of them.
    #[serde(default)]
    pub hands: BTreeMap<String, HandList>,
    /// The hands that may be used on this project at all. Empty asks nothing;
    /// a non-empty list refuses everything outside it with that reason,
    /// wherever it was named (§FS-006-project-interface.9) — which is what a
    /// repository under a policy about which models may see its code needs.
    #[serde(default)]
    pub permitted_hands: Vec<String>,
    /// This project's ceiling, inside its organization's and inside the
    /// site's aggregate autorun ceiling. Omitted leaves the ceilings above it
    /// in force and adds none of its own (§FS-005-dispatch.24).
    #[serde(default)]
    pub max_concurrent: Option<usize>,
    /// This project's ceiling on working roots, inside the site's aggregate
    /// one. The same relation to [`WorkConfig::max_active`] that
    /// [`ProjectWorkConfig::max_concurrent`] has to its own counterpart
    /// (§FS-005-dispatch.24).
    #[serde(default)]
    pub max_active: Option<usize>,
    /// This project's spend ceiling, inside its organization's and inside the
    /// site's (§FS-015-spend-ceiling.1). An organization's measured total
    /// already contains this project's, so a number above the one outside it
    /// is a project the outer ceiling stops first rather than a contradiction
    /// (§FS-015-spend-ceiling.5).
    #[serde(default)]
    pub max_spend: Option<crate::work::spend::SpendBudget>,
    /// The same, on tokens (§FS-015-spend-ceiling.2).
    #[serde(default)]
    pub max_tokens: Option<crate::work::spend::TokenBudget>,
}

/// Work for every project of one organization: the ceiling they share, inside
/// the site's aggregate one and outside each project's own
/// (§FS-005-dispatch.24), the work root they share, outside the site's and
/// inside each project's own (§FS-005-dispatch.6.1), and the recipes they
/// share, accumulated between the site's and each project's own
/// (§FS-005-dispatch.1). Which projects that is comes from the registry's
/// `organization` field, never from here — every key here is a binding, the
/// membership is identity (§REQ-001-boundary.2).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizationWorkConfig {
    /// The most autorun roots that may be live across this organization's
    /// projects. Omitted is unlimited; zero pauses new autorun starts under
    /// it (§FS-005-dispatch.24).
    #[serde(default)]
    pub max_concurrent: Option<usize>,
    /// Where the work of every project in this organization is written, unless
    /// the project names its own: the tier between the site's
    /// [`WorkConfig::root`] and [`ProjectWorkConfig::root`]
    /// (§FS-005-dispatch.6.1). This is the tier `{org_root}` is written at —
    /// one work root for a whole organization, for work that belongs to no
    /// single repository. Omitted leaves the site's answer in force.
    #[serde(default)]
    pub root: Option<String>,
    /// The most money autorun may spend across this organization's projects,
    /// inside the site's ceiling and outside each project's own
    /// (§FS-015-spend-ceiling.1). Which projects those are is the registry's
    /// `organization` field and nothing here.
    #[serde(default)]
    pub max_spend: Option<crate::work::spend::SpendBudget>,
    /// The same, on tokens (§FS-015-spend-ceiling.2).
    #[serde(default)]
    pub max_tokens: Option<crate::work::spend::TokenBudget>,
    /// The recipes offered on every project of this organization, read between
    /// the site's and the project's own: how the projects a person has said are
    /// one organization are worked, written once (§FS-005-dispatch.1). Unlike
    /// the root beside it this is not one answer but an ordered menu, so every
    /// scope is read outward in, and one written here reusing a site recipe's
    /// id replaces it where it already stands (§FS-005-dispatch.24).
    #[serde(default)]
    pub recipes: Vec<Recipe>,
}

/// The key a hands table answers every unnamed action with
/// (§FS-006-project-interface.9).
pub const DEFAULT_HAND: &str = "default";

/// Who a piece of work is meant for, as configuration names them
/// (§FS-006-project-interface.9): a hand the roster knows, or — for a pair the
/// runtime's registry never enumerated — the agent and the model it carries,
/// spelled out. Nothing here is the runtime's own grammar: a hand is an id and
/// an effort, and rendering one into whatever the binding executes happens in
/// the runtime adapter alone (§REQ-001-boundary.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandPin {
    /// `<hand-id>[:<effort>]`.
    Named { id: String, effort: Option<String> },
    /// `{ "agent": …, "model": …, "effort": … }` — both halves of the pair
    /// named, because half of one is a hand the roster already has an id for.
    Spelled {
        agent: String,
        model: String,
        effort: Option<String>,
    },
}

impl HandPin {
    /// The effort asked for, where one was.
    pub fn effort(&self) -> Option<&str> {
        match self {
            HandPin::Named { effort, .. } | HandPin::Spelled { effort, .. } => effort.as_deref(),
        }
    }

    /// How a message names this pin back to the person who wrote it.
    pub fn describe(&self) -> String {
        let effort = match self.effort() {
            Some(effort) => format!(" at effort '{effort}'"),
            None => String::new(),
        };
        match self {
            HandPin::Named { id, .. } => format!("'{id}'{effort}"),
            HandPin::Spelled { agent, model, .. } => format!("{agent} carrying {model}{effort}"),
        }
    }

    /// `<hand-id>[:<effort>]`, refusing what cannot be one. A selector in the
    /// runtime's own words is not a hand: it is spelled out in full instead,
    /// so that what configuration names is checkable against the roster.
    /// Public because the same words are typed at a command line as written in
    /// configuration, and one grammar cannot be read two ways.
    pub fn parse(text: &str) -> std::result::Result<HandPin, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("a hand is named '<hand>' or '<hand>:<effort>'; this one is empty".into());
        }
        let mut parts = text.splitn(2, ':');
        let id = parts.next().unwrap_or_default().trim();
        let effort = parts.next().map(str::trim);
        if id.is_empty() {
            return Err(format!("hand '{text}' names no hand before the ':'"));
        }
        match effort {
            Some(effort) if effort.is_empty() || effort.contains(':') => Err(format!(
                "hand '{text}' is not '<hand>:<effort>'; to name an agent and a model that the \
                 runtime's registry does not list, spell them out as \
                 {{ \"agent\": …, \"model\": … }}"
            )),
            effort => Ok(HandPin::Named {
                id: id.to_string(),
                effort: effort.map(str::to_string),
            }),
        }
    }
}

/// The long form, as a map spells it: both halves of a pair the registry never
/// enumerated. Read from a map and from nothing else — see [`PinVisitor`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Long {
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    effort: Option<String>,
}

impl Long {
    fn into_pin<E: serde::de::Error>(self) -> std::result::Result<HandPin, E> {
        match (self.agent, self.model) {
            (Some(agent), Some(model)) => Ok(HandPin::Spelled {
                agent,
                model,
                effort: self.effort,
            }),
            // Half a pair is a hand the roster names on its own, and naming it
            // by id is what lets ephor check it.
            _ => Err(serde::de::Error::custom(
                "a hand spelled out in full names both 'agent' and 'model'; \
                 to name one of them alone, use the roster's id for it",
            )),
        }
    }
}

/// One pin, read from a string or from a map and refused from a sequence.
///
/// The refusal is the point, and it is written by hand rather than derived: an
/// untagged enum over `String | Long` accepts a *sequence* too, because serde
/// will fill a struct's fields positionally from one — so `["a", "b"]` was read
/// as agent `a` carrying model `b`, silently, which is exactly the shape a list
/// of two alternates arrives in (§FS-006-project-interface.9). A visitor with
/// no `visit_seq` of its own would refuse it with serde's own wording; this one
/// says where the list belongs instead.
struct PinVisitor;

impl<'de> serde::de::Visitor<'de> for PinVisitor {
    type Value = HandPin;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a hand named '<hand>[:<effort>]', or spelled out as { \"agent\", \"model\" }")
    }

    fn visit_str<E: serde::de::Error>(self, text: &str) -> std::result::Result<HandPin, E> {
        HandPin::parse(text).map_err(serde::de::Error::custom)
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        map: A,
    ) -> std::result::Result<HandPin, A::Error> {
        Long::deserialize(serde::de::value::MapAccessDeserializer::new(map))?.into_pin()
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        _: A,
    ) -> std::result::Result<HandPin, A::Error> {
        Err(serde::de::Error::custom(
            "an array here is an ordered list of hands, and this place takes one hand — \
             a hand spelled out in full is the object { \"agent\": …, \"model\": … }, \
             never a pair written positionally",
        ))
    }
}

impl<'de> Deserialize<'de> for HandPin {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<HandPin, D::Error> {
        deserializer.deserialize_any(PinVisitor)
    }
}

impl Serialize for HandPin {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            HandPin::Named { id, effort } => match effort {
                Some(effort) => serializer.serialize_str(&format!("{id}:{effort}")),
                None => serializer.serialize_str(id),
            },
            HandPin::Spelled {
                agent,
                model,
                effort,
            } => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("agent", agent)?;
                map.serialize_entry("model", model)?;
                if let Some(effort) = effort {
                    map.serialize_entry("effort", effort)?;
                }
                map.end()
            }
        }
    }
}

/// What a pin names: an ordered list of hands, best first
/// (§FS-005-dispatch.14). One name *is* this list with a single member, so
/// nothing written before alternates existed means anything different, and the
/// seven steps still answer exactly once — with the list the answering step
/// carried rather than with a name.
///
/// The order is the author's and it ranks by fitness rather than by equality:
/// the first name is the right hand for this work and each name after it is
/// what to do when the one before it cannot be had. Which of them finally gets
/// the work is decided from evidence, later and elsewhere
/// (§FS-005-dispatch.29) — nothing here reorders anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandList(Vec<HandPin>);

impl HandList {
    /// The list one pin makes.
    pub fn one(pin: HandPin) -> HandList {
        HandList(vec![pin])
    }

    pub fn members(&self) -> &[HandPin] {
        &self.0
    }

    /// The first member — the hand the author would have written alone.
    pub fn first(&self) -> &HandPin {
        &self.0[0]
    }

    /// Whether anything was actually chosen among: a list of one is the bare
    /// name it was written as, and the choosing has nothing to say about it.
    pub fn is_alternates(&self) -> bool {
        self.0.len() > 1
    }

    /// How a message names this list back to the person who wrote it.
    pub fn describe(&self) -> String {
        self.0
            .iter()
            .map(HandPin::describe)
            .collect::<Vec<_>>()
            .join(", then ")
    }

    /// The command line's spelling: the names separated by commas
    /// (§FS-006-project-interface.9). Commas rather than a flag given twice,
    /// because a repeated flag would make the order depend on how the shell
    /// was typed — and an empty member is refused with what was written, since
    /// a trailing comma is a typo far more often than it is a name.
    ///
    /// Public because a hand is typed at a command line exactly as it is
    /// written in configuration, and one grammar cannot be read two ways.
    pub fn parse(text: &str) -> std::result::Result<HandList, String> {
        let empty = || {
            format!(
                "'{text}' names an empty hand between its commas — a list of hands is \
                 '<hand>[:<effort>]' separated by commas, best first"
            )
        };
        if text.trim().is_empty() {
            return Err(empty());
        }
        let mut members = Vec::new();
        for member in text.split(',') {
            if member.trim().is_empty() {
                return Err(empty());
            }
            members.push(HandPin::parse(member)?);
        }
        Ok(HandList(members))
    }
}

impl<'de> Deserialize<'de> for HandList {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<HandList, D::Error> {
        struct ListVisitor;
        impl<'de> serde::de::Visitor<'de> for ListVisitor {
            type Value = HandList;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("one hand, or an ordered list of them")
            }

            fn visit_str<E: serde::de::Error>(
                self,
                text: &str,
            ) -> std::result::Result<HandList, E> {
                PinVisitor.visit_str(text).map(HandList::one)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> std::result::Result<HandList, A::Error> {
                PinVisitor.visit_map(map).map(HandList::one)
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<HandList, A::Error> {
                let mut members = Vec::new();
                while let Some(pin) = seq.next_element::<HandPin>()? {
                    members.push(pin);
                }
                if members.is_empty() {
                    return Err(serde::de::Error::custom(
                        "an empty list names nobody — write the hand, or leave the entry out",
                    ));
                }
                Ok(HandList(members))
            }
        }
        deserializer.deserialize_any(ListVisitor)
    }
}

impl Serialize for HandList {
    /// A list of one serializes as the bare name it was written as, so nothing
    /// already on disk changes shape when it is read back and rewritten.
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self.0.as_slice() {
            [only] => only.serialize(serializer),
            several => serializer.collect_seq(several),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    pub description: String,
    /// The state a fresh ticket starts in, from the work root's machine.
    #[serde(default = "default_state")]
    pub state: String,
    #[serde(default)]
    pub when: Selector,
    /// The work needs the item's own branch on disk. Where it does and the
    /// branch is not checked out, dispatch refuses rather than writing a
    /// ticket about code that is not on the machine (§FS-005-dispatch.3).
    /// Work that reads a change rather than editing it — a review, a reply —
    /// says `false` and runs in the project's own checkout.
    #[serde(default = "yes")]
    pub needs_checkout: bool,
    /// Which branch this work belongs on, where the matter has none of its own
    /// (§FS-005-dispatch.25). A template rendered from the matter's fields
    /// exactly as [`Recipe::brief`] is — `fix/issue-{number}` — which dispatch
    /// resolves and makes the workspace of. Saying it means the work needs the
    /// checkout; a recipe that says nothing is placed as it always was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The whole work-root template for work handed over through this recipe.
    /// It wins the project, organization and site answers and is rendered
    /// after branch placement, from the same subject values (§FS-005-dispatch.1,
    /// §FS-005-dispatch.6.1, §FS-005-dispatch.25).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// This work needs nobody to start it: a ticket written from this recipe
    /// gets its run without anyone pressing a key (§FS-005-dispatch.24). The
    /// reader's deliberate act is adopting the recipe, made once, rather than
    /// starting each of its tickets. Silence means the key, as it always did —
    /// per recipe and nowhere else, because trusting one kind of work to start
    /// itself says nothing about the rest.
    #[serde(default)]
    pub autorun: bool,
    /// This recipe's own sweep needs nobody either, and goes at this rhythm
    /// (§FS-005-dispatch.32). One step earlier than [`Recipe::autorun`] and
    /// the same shape: its presence is the opt-in, and silence leaves the
    /// sweep the reader's to type. The two settings answer two different
    /// questions — *find them yourself* and *do not wait for me to start it* —
    /// and neither implies the other.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<Sweep>,
    /// What the ticket asks for, in the reader's words. `{...}` placeholders
    /// are filled from the item (see [`super::dossier::Subject::placeholders`]),
    /// plus `{reply}` — where a proposed answer for this matter belongs, which
    /// ephor reads back and offers beside the conversation
    /// (§FS-005-dispatch.13).
    ///
    /// Optional because a recipe may keep its words in the file that owns them
    /// instead ([`Recipe::brief_file`], §FS-005-dispatch.34.1). Writing both is
    /// not an error and they are not two spellings of one fact: the file says
    /// how work is done here, and this still says what to do with this matter.
    /// Writing neither is, and it is refused where the configuration loads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// The file the brief is kept in, as a path template rendered from the
    /// vocabulary a work root is rendered from (§FS-005-dispatch.34). It is
    /// read when the ticket is written and its text is the brief, so the plan
    /// carries the instruction rather than a path to it
    /// (§FS-005-dispatch.2) — and a standing instruction stays in the document
    /// that owns it rather than being pasted into site configuration.
    ///
    /// `{reply}` is not among the names a path may take: it is a place ephor
    /// writes to rather than a fact about the matter. Nothing inside the file
    /// is substituted — a version-controlled document is not a template, and
    /// its braces are the characters they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief_file: Option<String>,
    /// The configuration file this recipe was written in, stamped after the
    /// parse rather than written by anybody: a relative
    /// [`Recipe::brief_file`] resolves against the directory holding it and
    /// never against the working directory, because a recipe that sweeps on a
    /// timer (§FS-005-dispatch.32) runs from wherever the unit that called
    /// ephor happened to stand (§FS-005-dispatch.34).
    #[serde(skip)]
    pub based_in: Option<std::path::PathBuf>,
    /// A deterministic opening move ephor makes itself, before the ticket
    /// costs a model (§FS-005-dispatch.12). Where the move finishes, nothing
    /// is dispatched at all; where it stops, what it reached is written into
    /// the brief and that is the ticket. `rebase` is the one ephor knows —
    /// the same operation the reader presses a key for
    /// (§FS-004-quick-actions.6), so two of them cannot disagree about what a
    /// clean rebase is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opens_with: Option<String>,
    /// Whose work this is, when it is not whoever the tables would default to
    /// — the second of the seven steps, and the portable spelling: a hand id
    /// the roster knows, checked against it (§FS-006-project-interface.9). One
    /// name, or an ordered list of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand: Option<HandList>,
    /// Pin the runtime's execution identity for this ticket. The runtime's own
    /// words rather than a hand, so nothing checks it — it pins this recipe
    /// the same way `hand` does, and a project's tables do not displace it. A
    /// recipe carrying both this and `hand` is refused at dispatch: one of
    /// them would silently lose (§FS-006-project-interface.9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// A recipe's own sweep: how often it looks for its matters, and how much one
/// look may open (§FS-005-dispatch.32).
///
/// Written as the interval alone — `"6h"` — which is the spelling to prefer,
/// or as `{ "every": "6h", "limit": 3 }` where the recipe wants the bound. One
/// field says both *this sweep needs nobody* and *at this rhythm*, because an
/// interval already says the first: a boolean beside it would restate it, and
/// `false` written beside an interval is a state nothing could mean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sweep {
    /// At most this often. Zero has elapsed by the time anything reads it, so
    /// `"0h"` falls out of the same comparison every other value goes through
    /// rather than being a mode beside it — *whenever you ask me*, bounded by
    /// whatever rate the caller runs at, because ephor has no daemon.
    every: chrono::TimeDelta,
    /// The most matters this recipe may open in one sweep of its own. The
    /// reader is not there to type `--limit` (§FS-005-dispatch.26), and the
    /// verb hosting the sweep is not the verb that flag is on. Counted per
    /// recipe, so two self-sweeping recipes do not spend each other's
    /// allowance. Omitted leaves it bounded by the ceilings every start is
    /// bounded by and by nothing nearer.
    pub limit: Option<usize>,
}

impl Sweep {
    /// Whether this recipe is due, given when it last swept. A recipe nothing
    /// was recorded about is due now: the record is ephor's own and a missing
    /// one costs a sweep's worth of waiting rather than the sweep itself
    /// (§FS-005-dispatch.32).
    pub fn due(&self, last: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
        match last {
            // How long it has been, not when it would next be: `last + every`
            // is chrono's panicking add, and an interval this side of
            // `TimeDelta`'s ceiling can still land past the calendar's.
            Some(last) => now.signed_duration_since(last) >= self.every,
            None => true,
        }
    }

    /// How the interval is named back to whoever wrote it.
    pub fn describe(&self) -> String {
        let every = match self.every.num_minutes() {
            0 => "every sweep".to_string(),
            minutes if minutes % 1440 == 0 => format!("every {}d", minutes / 1440),
            minutes if minutes % 60 == 0 => format!("every {}h", minutes / 60),
            minutes => format!("every {minutes}m"),
        };
        match self.limit {
            Some(limit) => format!("{every}, at most {limit}"),
            None => every,
        }
    }

    /// The longest wait that could still come due: the whole span the calendar
    /// can represent. `TimeDelta` reaches some ten thousand times further than
    /// a `DateTime` does, so an interval it accepts is not yet one a rhythm can
    /// be written in — beyond this the recipe would simply never sweep.
    fn longest() -> chrono::TimeDelta {
        DateTime::<Utc>::MAX_UTC.signed_duration_since(DateTime::<Utc>::MIN_UTC)
    }

    /// `<n><unit>`, where the unit is `m`, `h` or `d`. Refused rather than
    /// defaulted: a rhythm nobody can read is a recipe that would sweep at a
    /// rate its author never chose, and this is the field that spends agents
    /// unattended.
    fn parse_every(text: &str) -> std::result::Result<chrono::TimeDelta, String> {
        let text = text.trim();
        let unsaid = || {
            format!(
                "'{text}' is not an interval; write it as '<number><unit>' with a unit of \
                 'm', 'h' or 'd' — '0h' is every sweep"
            )
        };
        // Stripped by unit rather than sliced at `len() - 1`: the unit is a
        // character and the length is bytes, so a tail nobody meant — '6µ',
        // a pasted smart quote — would split mid-character and panic where
        // this is meant to refuse. Nothing but 'm', 'h' and 'd' is read.
        let (count, per_minute) = if let Some(count) = text.strip_suffix('m') {
            (count, 1)
        } else if let Some(count) = text.strip_suffix('h') {
            (count, 60)
        } else if let Some(count) = text.strip_suffix('d') {
            (count, 60 * 24)
        } else {
            return Err(unsaid());
        };
        let count: i64 = count.parse().map_err(|_| unsaid())?;
        if count < 0 {
            return Err(format!(
                "interval '{text}' is negative; '0h' is every sweep"
            ));
        }
        let minutes = count.saturating_mul(per_minute);
        let every = chrono::TimeDelta::try_minutes(minutes)
            .filter(|every| *every <= Self::longest())
            .ok_or_else(|| format!("interval '{text}' is longer than anything can wait"))?;
        Ok(every)
    }
}

/// The long spelling, as a map writes it. Read from a map and from nothing
/// else — see [`SweepVisitor`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LongSweep {
    every: String,
    #[serde(default)]
    limit: Option<usize>,
}

/// One sweep, read from the interval alone or from the map that carries a
/// bound beside it.
struct SweepVisitor;

impl<'de> serde::de::Visitor<'de> for SweepVisitor {
    type Value = Sweep;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("an interval like '6h', or { \"every\": \"6h\", \"limit\": 3 }")
    }

    fn visit_str<E: serde::de::Error>(self, text: &str) -> std::result::Result<Sweep, E> {
        Ok(Sweep {
            every: Sweep::parse_every(text).map_err(serde::de::Error::custom)?,
            limit: None,
        })
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        map: A,
    ) -> std::result::Result<Sweep, A::Error> {
        let long = LongSweep::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
        Ok(Sweep {
            every: Sweep::parse_every(&long.every).map_err(serde::de::Error::custom)?,
            limit: long.limit,
        })
    }
}

impl<'de> Deserialize<'de> for Sweep {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Sweep, D::Error> {
        deserializer.deserialize_any(SweepVisitor)
    }
}

impl Serialize for Sweep {
    /// Back out in the spelling it was written in, so a recipe read and
    /// rewritten does not change shape.
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let every = match self.every.num_minutes() {
            minutes if minutes % 1440 == 0 && minutes != 0 => format!("{}d", minutes / 1440),
            minutes if minutes % 60 == 0 => format!("{}h", minutes / 60),
            minutes => format!("{minutes}m"),
        };
        match self.limit {
            None => serializer.serialize_str(&every),
            Some(limit) => {
                use serde::ser::SerializeMap;
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("every", &every)?;
                map.serialize_entry("limit", &limit)?;
                map.end()
            }
        }
    }
}

/// The deterministic moves ephor can make on its own behalf
/// (§FS-005-dispatch.12). The rebase is the first of these, not the shape of
/// the only one.
pub const OPENING_REBASE: &str = "rebase";

fn default_icon() -> String {
    "◆".to_string()
}

/// Where a ticket starts when nothing says otherwise: the shipped machine's
/// working state. Public because a menu entry that carries a brief instead of
/// a command is a recipe under another name (§FS-005-dispatch.1), and it
/// starts where the recipes do.
pub fn default_state() -> String {
    "fix".to_string()
}

fn yes() -> bool {
    true
}

/// Which items a recipe applies to. An empty field asks nothing; every field
/// that is set must hold.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    /// `pr`, `ci`, `issue`, `task`, `message`, `status`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<String>,
    /// `author`, `reviewer`. An item whose source reported no role — a
    /// project's own task among them (§FS-003-feed-categories.1) — matches
    /// only when this is empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    /// What the gate has to be doing. The two ways a gate is red are separate
    /// conditions because they ask for different work: `failing` — jobs
    /// failed, which is something a checkout can fix; `blocked` — the forge
    /// refuses the merge, which is often an approval nobody can give from
    /// here. Also `red` (either), `green` (neither), and `any` (a gate at
    /// all).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_response: Option<bool>,
    /// Provider names, for a recipe that only makes sense on one source.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
    /// The logins that may hold the matter (§FS-005-dispatch.31). A plain name
    /// is one the matter must be held by; `!name` is one it must not. A matter
    /// whose source reported no assignment answers neither form.
    #[serde(
        default,
        deserialize_with = "named",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub assignees: Vec<String>,
    /// The labels the matter must and must not carry (§FS-005-dispatch.31),
    /// asked exactly as `assignees` is: `["enhancement", "!GenAI"]` is
    /// labelled `enhancement` and not labelled `GenAI`.
    #[serde(
        default,
        deserialize_with = "named",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub labels: Vec<String>,
    /// The item's branch trails its main branch (`true`), or is level with it
    /// (`false`) — measured in the checkout, not asked of a forge
    /// (§FS-004-quick-actions.6). An item whose checkout cannot be measured —
    /// no branch, nothing on disk — matches neither, because a recipe that
    /// asks about the checkout and gets no answer is being offered blind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind: Option<bool>,
    /// The item's branch trails its own **published copy** (`true`), or is
    /// level with it (`false`) — the other distance, and a different question
    /// from the one above (§FS-004-quick-actions.8). A branch published
    /// nowhere matches neither, for the same reason an unmeasurable checkout
    /// does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind_upstream: Option<bool>,
}

/// What a selector asks about that the item does not carry: facts measured in
/// the reader's own checkout.
#[derive(Debug, Clone, Copy, Default)]
pub struct Facts {
    /// Commits the item's branch trails its main branch; None where it could
    /// not be measured.
    pub behind: Option<u64>,
    /// Commits it trails its own published copy; None where there is no copy
    /// to measure against (§FS-004-quick-actions.8).
    pub behind_upstream: Option<u64>,
}

impl Selector {
    /// Whether this selector holds for an item. Public because the same
    /// language selects menu offers, whoever wrote them
    /// (§FS-006-project-interface.9).
    pub fn matches(&self, item: &Item, facts: &Facts) -> bool {
        self.explain(item, facts).is_empty()
    }

    /// The explain-capable companion to [`Selector::matches`]: every field
    /// that refused, and what it found instead of what it asked for
    /// (§FS-005-dispatch.27). Empty exactly where `matches` would answer
    /// `true` — this decides nothing `matches` did not already decide, it
    /// only says why where `matches` only said no.
    pub fn explain(&self, item: &Item, facts: &Facts) -> Vec<Refusal> {
        let mut refusals = Vec::new();
        if let Some(want) = self.behind {
            match facts.behind {
                Some(behind) if (behind > 0) == want => {}
                Some(behind) => refusals.push(Refusal::new(
                    "behind",
                    format!(
                        "the branch is {} its main branch; the selector asks for {}",
                        if behind > 0 { "behind" } else { "level with" },
                        if want { "behind" } else { "level with it" }
                    ),
                )),
                None => refusals.push(Refusal::new(
                    "behind",
                    "the branch could not be measured against a main branch here",
                )),
            }
        }
        // The same shape for the other distance, and asked the same way: a
        // branch with no published copy answers neither `true` nor `false`
        // (§FS-004-quick-actions.8).
        if let Some(want) = self.behind_upstream {
            match facts.behind_upstream {
                Some(behind) if (behind > 0) == want => {}
                Some(behind) => refusals.push(Refusal::new(
                    "behind_upstream",
                    format!(
                        "the branch is {} its published copy; the selector asks for {}",
                        if behind > 0 { "behind" } else { "level with" },
                        if want { "behind" } else { "level with it" }
                    ),
                )),
                None => refusals.push(Refusal::new(
                    "behind_upstream",
                    "the branch has no published copy to measure against",
                )),
            }
        }
        if !self.kinds.is_empty() && !self.kinds.iter().any(|kind| kind_matches(item.kind, kind)) {
            refusals.push(Refusal::new(
                "kinds",
                format!(
                    "the matter's kind is `{}`; the selector asks for {}",
                    item.kind.label(),
                    join_quoted(&self.kinds)
                ),
            ));
        }
        if !self.roles.is_empty() && !self.roles.iter().any(|role| role_matches(item.role, role)) {
            refusals.push(Refusal::new(
                "roles",
                match item.role {
                    // The role-less case this exists for: a project's own
                    // task carries no role at all, so it is not merely a
                    // role the selector did not ask for
                    // (§FS-003-feed-categories.1).
                    None => format!(
                        "the matter carries no role; the selector asks for {}",
                        join_quoted(&self.roles)
                    ),
                    Some(role) => format!(
                        "the matter's role is `{}`; the selector asks for {}",
                        role_label(role),
                        join_quoted(&self.roles)
                    ),
                },
            ));
        }
        if !self.sources.is_empty() && !self.sources.contains(&item.source) {
            refusals.push(Refusal::new(
                "sources",
                format!(
                    "the matter's source is `{}`; the selector asks for {}",
                    item.source,
                    join_quoted(&self.sources)
                ),
            ));
        }
        if let Some(needs_response) = self.needs_response {
            if item.needs_response != needs_response {
                refusals.push(Refusal::new(
                    "needs_response",
                    format!(
                        "the matter {} an answer; the selector asks for one that {}",
                        if item.needs_response {
                            "needs"
                        } else {
                            "does not need"
                        },
                        if needs_response { "does" } else { "does not" }
                    ),
                ));
            }
        }
        if let Some(want) = self.gate.as_deref() {
            if !gate_matches(Gate::of(item), want) {
                refusals.push(Refusal::new(
                    "gate",
                    format!("the matter's gate does not match `{want}`"),
                ));
            }
        }
        if let Some(refusal) = held_or_labelled(
            "assignees",
            &self.assignees,
            reported(item, "assignees").as_deref(),
            "held by",
        ) {
            refusals.push(refusal);
        }
        if let Some(refusal) = held_or_labelled(
            "labels",
            &self.labels,
            reported(item, "labels").as_deref(),
            "labelled",
        ) {
            refusals.push(refusal);
        }
        refusals
    }
}

/// Every entry of an `assignees` or `labels` field, refused where one names
/// nothing (§FS-005-dispatch.31). `!` alone reads as a filter and excludes
/// nothing, which is what a templated recipe degrades into when the name it
/// interpolates is missing; a field that asks for nothing is written by
/// omitting the field. Read here rather than at either surface, because a
/// selector is written in a feed config, a project manifest and the runtime's
/// own recipes alike.
fn named<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    let entries = Vec::<String>::deserialize(deserializer)?;
    if let Some(unnamed) = entries
        .iter()
        .find(|entry| entry.trim_start_matches('!').is_empty())
    {
        return Err(serde::de::Error::custom(format!(
            "`{unnamed}` names nothing: write `name` or `!name`, or omit the field to filter on neither"
        )));
    }
    Ok(entries)
}

/// What the item's source said under `key`, or `None` where it said nothing
/// (§FS-005-dispatch.31). The absent key and the empty list are different
/// answers and stay different all the way to the refusal.
fn reported(item: &Item, key: &str) -> Option<Vec<String>> {
    let reported = item.raw.get(key)?.as_array()?;
    Some(
        reported
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
    )
}

/// One `assignees` or `labels` field against what the matter carries. `None`
/// where it held (§FS-005-dispatch.31): all negatives must hold, and the
/// positives, where any were written, must find one. A matter whose source
/// reported nothing refuses both forms rather than answering either, because a
/// fact nobody stated is not a fact.
fn held_or_labelled(
    field: &'static str,
    selector: &[String],
    carried: Option<&[String]>,
    verb: &str,
) -> Option<Refusal> {
    if selector.is_empty() {
        return None;
    }
    let Some(carried) = carried else {
        return Some(Refusal::new(
            field,
            format!("the matter's source reported nothing about what it is {verb}"),
        ));
    };
    let (wanted, unwanted): (Vec<&str>, Vec<&str>) = selector
        .iter()
        .map(String::as_str)
        .partition(|entry| !entry.starts_with('!'));
    let carries = |name: &str| carried.iter().any(|held| held == name);

    let forbidden: Vec<&str> = unwanted
        .iter()
        .map(|entry| &entry[1..])
        .filter(|name| carries(name))
        .collect();
    if !forbidden.is_empty() {
        return Some(Refusal::new(
            field,
            format!(
                "the matter is {verb} {}, which the selector excludes",
                join_quoted_str(&forbidden)
            ),
        ));
    }
    if !wanted.is_empty() && !wanted.iter().any(|name| carries(name)) {
        return Some(Refusal::new(
            field,
            format!(
                "the matter is {}; the selector asks for {}",
                if carried.is_empty() {
                    format!("{verb} nothing")
                } else {
                    format!("{verb} {}", join_quoted(carried))
                },
                join_quoted_str(&wanted)
            ),
        ));
    }
    None
}

/// Why a selector refused an item, one per field that asked for something the
/// item did not answer (§FS-005-dispatch.27). `field` is the selector's own
/// name for it, so a caller naming the refusal names the same word a person
/// would edit in the recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub field: &'static str,
    pub reason: String,
}

impl Refusal {
    fn new(field: &'static str, reason: impl Into<String>) -> Self {
        Refusal {
            field,
            reason: reason.into(),
        }
    }
}

fn join_quoted(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_quoted_str(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn role_label(role: ItemRole) -> &'static str {
    match role {
        ItemRole::Author => "author",
        ItemRole::Reviewer => "reviewer",
    }
}

fn kind_matches(kind: ItemKind, name: &str) -> bool {
    // The Message label is "msg"; accept the config-friendly spelling too.
    name == kind.label() || (kind == ItemKind::Message && name == "message")
}

fn role_matches(role: Option<ItemRole>, name: &str) -> bool {
    match (role, name) {
        (Some(ItemRole::Author), "author") => true,
        (Some(ItemRole::Reviewer), "reviewer") => true,
        _ => false,
    }
}

fn gate_matches(gate: Option<Gate>, want: &str) -> bool {
    let Some(gate) = gate else {
        return false;
    };
    match want {
        "failing" => gate.failed() > 0,
        "blocked" => gate.blocked,
        "red" => gate.is_red(),
        "green" => !gate.is_red(),
        _ => true,
    }
}

impl Recipe {
    /// Whether this recipe applies to the item. Finished work never does
    /// (§FS-005-dispatch.6): asking an agent to fix a merged pull request is
    /// asking it to invent something to do.
    pub fn matches(&self, item: &Item, facts: &Facts) -> bool {
        self.reserved().is_none() && !item.is_finished() && self.when.matches(item, facts)
    }

    /// Why this recipe belongs to something other than the feed, where it
    /// does. One recipe is spoken for: the rebase sweep's
    /// (§FS-004-quick-actions.6.1). Its subject is a *checkout*, and the feed
    /// has no row for a tree, so a selector cannot express "no item" — an
    /// empty one matches everything, which is how it came to be offered on
    /// matters it says nothing about.
    pub fn reserved(&self) -> Option<String> {
        (self.id == crate::sweep::RECIPE).then(|| {
            format!(
                "'{}' is the rebase sweep's own recipe: its subject is a checkout rather than \
                 a matter, so it is never offered on one",
                self.id
            )
        })
    }

    /// Why this recipe asks for nothing at all (§FS-005-dispatch.34.1). A
    /// recipe is a selector and a brief, and the brief may arrive by either
    /// door — but one of the doors has to be open, and a ticket carrying no
    /// words is work nobody can do.
    ///
    /// Answered here and asked where the configuration loads rather than at
    /// the dispatch that would have used it: a recipe can run from a timer
    /// with nobody watching (§FS-005-dispatch.24), so a dispatch-time refusal
    /// lands in a log while a load-time one stops the next reading of anything
    /// in front of the person who has just edited the file.
    pub fn without_a_brief(&self) -> Option<String> {
        (self.brief.is_none() && self.brief_file.is_none()).then(|| {
            format!(
                "recipe '{}' says neither 'brief' nor 'brief_file': a recipe is a selector and \
                 a brief, so write the words inline as 'brief', or name the file they are kept \
                 in as 'brief_file'",
                self.id
            )
        })
    }

    /// Record which configuration file wrote this recipe, so a relative
    /// `brief_file` has something to be relative *to* (§FS-005-dispatch.34).
    /// Stamped once, after the parse, by whoever read the file.
    pub fn written_in(&mut self, config: &std::path::Path) {
        self.based_in = Some(config.to_path_buf());
    }
}

/// The recipes ephor knows without being told (§FS-005-dispatch.1). They ask
/// for what is true on every forge — read the failures, answer the question,
/// read the change, do the issue — and stop at a local change
/// (§FS-005-dispatch.7).
pub fn shipped() -> Vec<Recipe> {
    let recipe = |id: &str,
                  icon: &str,
                  description: &str,
                  needs_checkout: bool,
                  when: Selector,
                  brief: &str| Recipe {
        id: id.to_string(),
        icon: icon.to_string(),
        description: description.to_string(),
        state: default_state(),
        when,
        needs_checkout,
        // Most shipped recipes either use the matter's own branch or do work
        // that does not edit a checkout. A recipe that creates code for a
        // branch-less matter opts into branch minting below
        // (§FS-005-dispatch.25).
        branch: None,
        root: None,
        // Silence means the key: what ships is started by the reader, and
        // saying otherwise is a thing configuration does (§FS-005-dispatch.24).
        autorun: false,
        // And nothing that ships sweeps for itself, for the same reason one
        // step earlier (§FS-005-dispatch.32).
        dispatch: None,
        brief: Some(brief.to_string()),
        // Nothing ephor ships names a file of its own: a project that gained a
        // voice in what is asked for merely by containing a well-known
        // filename would be an artifact required of it
        // (§REQ-001-boundary.3, §FS-005-dispatch.34).
        brief_file: None,
        based_in: None,
        opens_with: None,
        // The shipped recipes name nobody: who does them is the reader's
        // table to write, and unwritten is the runtime's to pick
        // (§FS-006-project-interface.9).
        hand: None,
        target: None,
        model: None,
    };
    let implement = {
        let mut recipe = recipe(
            "implement",
            "🧩",
            "do the work in this issue",
            // An issue's implementation work needs an isolated branch. The
            // template activates checkout creation for branch-less issues;
            // an existing matter branch still wins.
            false,
            Selector {
                kinds: vec!["issue".to_string()],
                roles: vec!["author".to_string()],
                ..Selector::default()
            },
            "{title} is an issue of mine.\n\n\
             The issue and its comments are above. Work out what is actually being\n\
             asked for — an issue is a description of a problem, not a\n\
             specification — and do the smallest thing that answers it, with a test\n\
             where the project tests that kind of change.\n\n\
             Where the issue is under-specified in a way that changes what the code\n\
             should do, do not guess: write the question in the report and stop.",
        );
        recipe.branch = Some("fix/issue-{number}".to_string());
        recipe
    };
    vec![
        recipe(
            "fix-gate",
            "🛠",
            "fix the red gate",
            // Fixing a gate is editing the change, so the change has to be here.
            true,
            Selector {
                kinds: vec!["pr".to_string(), "ci".to_string()],
                roles: vec!["author".to_string()],
                // Jobs that failed, not a gate that merely refuses: a change
                // waiting on an approval has nothing for a checkout to do, and
                // sending an agent at it spends a pass to be told so.
                gate: Some("failing".to_string()),
                ..Selector::default()
            },
            "The gate on {title} is red.\n\n\
             Find out what actually failed — the dossier above says what the watch\n\
             knew, and the forge itself says the rest — and fix the cause of it. A\n\
             job that failed for a reason unrelated to this change (a flake, an\n\
             infrastructure error, a broken upstream) is not something to fix here:\n\
             say so in the report and stop.\n\n\
             Where the gate is blocked rather than failing, the blockers above say\n\
             why. Some of them are nobody's to fix from a checkout — an approval, a\n\
             downstream repository — and those belong in the report, not in a\n\
             change.",
        ),
        recipe(
            "answer",
            "💬",
            "answer the conversation",
            // An answer is usually words, and the ones that need code can fetch
            // it: refusing every question asked about a branch that happens not
            // to be checked out would leave most of them unanswered
            // (§FS-005-dispatch.13).
            false,
            Selector {
                kinds: vec!["pr".to_string(), "issue".to_string(), "message".to_string()],
                needs_response: Some(true),
                ..Selector::default()
            },
            // The reply is asked for as a file of its own, because ephor reads
            // it back and offers it beside the conversation it answers
            // (§FS-005-dispatch.13). Prose about the reply belongs in the
            // report; that file is the reply and nothing else.
            "{title} is waiting on an answer from me.\n\n\
             The conversation is above, last message last. Work out what is being\n\
             asked. Where the answer is a change, make it. Where the answer is a\n\
             sentence, write the sentence.\n\n\
             Write the reply itself to {reply} — the whole message, in my voice,\n\
             exactly as it would be posted, and nothing else in that file: no\n\
             heading, no preamble, no notes about it. Say in the report what you\n\
             based it on and what you were unsure of.\n\n\
             Do not post it anywhere. Posting is mine to do.",
        ),
        recipe(
            "review",
            "👓",
            "review this change",
            // Someone else's branch is almost never checked out here, and a
            // review reads a change rather than editing it.
            false,
            Selector {
                kinds: vec!["pr".to_string()],
                roles: vec!["reviewer".to_string()],
                ..Selector::default()
            },
            "{title} is a change I am reviewing.\n\n\
             Read the change itself — fetch the branch if it is not here — and\n\
             review it as someone who has to live with it: correctness first, then\n\
             what it does to the code around it, then what it will cost to keep.\n\n\
             The report is the review: each point as the file and line it is about,\n\
             what is wrong, and what would be right. Say plainly which points would\n\
             block a merge and which are opinions. Do not post anything.",
        ),
        implement,
        Recipe {
            // The replay itself is ephor's, made before this ticket exists
            // (§FS-005-dispatch.12): a clean rebase is a done thing and never
            // reaches a model, and what is dispatched is the conflict the
            // algorithm stopped at.
            opens_with: Some(OPENING_REBASE.to_string()),
            ..recipe(
                "rebase",
                "⤴",
                "rebase onto the main branch, handing over what conflicts",
                // Replaying a branch happens where the branch is.
                true,
                Selector {
                    // Only where the branch has actually fallen behind: a
                    // recipe offered on a change that is already current is a
                    // ticket to do nothing (§FS-004-quick-actions.6). It is
                    // last of the shipped recipes because a red gate or an
                    // owed answer is the more urgent thing about the same
                    // pull request.
                    //
                    // And nothing else. This is the one recipe an entry of the
                    // menu dispatches by name — the replay hands its conflict
                    // to `rebase` (§FS-005-dispatch.12) — so what the entry is
                    // offered on and what the recipe applies to have to be the
                    // same set (§FS-005-dispatch.1). The entry asks about a
                    // branch on disk that trails its base and nothing else
                    // (§FS-004-quick-actions.6), so neither does this: the
                    // kind of row that mentions the branch, and whose change
                    // the forge says it is, are facts about how the reader
                    // arrived, not about the checkout being replayed — and a
                    // distance can only be measured where the branch is here.
                    behind: Some(true),
                    ..Selector::default()
                },
                "{title} is on {branch}, which has fallen behind its main branch, and the\n\
                 replay has already been run — the report above says where it stopped.\n\n\
                 A conflict is standing in the working tree of the repository it names.\n\
                 Resolve each conflict as the change itself would have been written against\n\
                 the new base — take neither side on principle, work out what the two commits\n\
                 were each trying to do, and where that is not decidable from the code, stop\n\
                 and say so rather than guessing. `git add` what you resolved and\n\
                 `git rebase --continue` until the replay finishes.\n\n\
                 Then check the result: build or test what the conflicting files belong to, so\n\
                 that \"it rebased\" is not the same claim as \"it still works\". Do not push.",
            )
        },
    ]
}

/// The recipes offered on a project: the shipped ones, then configuration's
/// three scopes accumulated outward in — the site's, the organization the
/// registry places the project in, then the project's own — with a later scope
/// reusing an earlier id replacing that recipe *where it already stands*
/// rather than moving it to the end, because position is the order dispatch
/// offers in (§FS-005-dispatch.1).
pub fn resolve(global: &[Recipe], organization: &[Recipe], project: &[Recipe]) -> Vec<Recipe> {
    let mut resolved = shipped();
    for recipe in global.iter().chain(organization).chain(project) {
        match resolved
            .iter()
            .position(|existing| existing.id == recipe.id)
        {
            Some(index) => resolved[index] = recipe.clone(),
            None => resolved.push(recipe.clone()),
        }
    }
    resolved
}

/// The recipes that apply to one item, in offer order.
pub fn applicable(recipes: &[Recipe], item: &Item, facts: &Facts) -> Vec<Recipe> {
    recipes
        .iter()
        .filter(|recipe| recipe.matches(item, facts))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feed::gate::RepoGate;
    use serde_json::json;

    fn item(kind: ItemKind, role: Option<ItemRole>) -> Item {
        Item {
            id: "github-prs:acme/widget#42".to_string(),
            project: "widget".to_string(),
            source: "github-prs".to_string(),
            kind,
            role,
            title: "Retry window".to_string(),
            url: None,
            state: Some("open".to_string()),
            needs_response: false,
            updated_at: chrono::Utc::now(),
            raw: json!({}),
        }
    }

    fn with_gate(mut item: Item, failed: u64, blocked: bool) -> Item {
        let gate = Gate {
            repos: vec![RepoGate {
                repo: "widget".to_string(),
                passed: 12,
                failed,
                running: 0,
            }],
            blocked,
            blockers: Vec::new(),
        };
        item.raw = json!({ "gate": gate.to_value() });
        item
    }

    /// Which recipes apply, with the checkout unmeasured — the answer for
    /// every item whose branch is not on this machine.
    fn ids(recipes: &[Recipe], item: &Item) -> Vec<String> {
        ids_with(recipes, item, Facts::default())
    }

    fn selector(json: Value) -> Selector {
        serde_json::from_value(json).expect("selector parses")
    }

    fn labelled(names: Value) -> Item {
        let mut item = item(ItemKind::Issue, Some(ItemRole::Author));
        item.raw = json!({ "labels": names });
        item
    }

    #[test]
    fn a_positive_label_asks_for_any_of_them() {
        let asks = selector(json!({ "labels": ["enhancement", "bug"] }));
        assert!(asks.matches(&labelled(json!(["enhancement"])), &Facts::default()));
        assert!(asks.matches(&labelled(json!(["bug", "priority"])), &Facts::default()));
        assert!(!asks.matches(&labelled(json!(["priority"])), &Facts::default()));
        assert!(!asks.matches(&labelled(json!([])), &Facts::default()));
    }

    #[test]
    fn a_negative_label_forbids_it_however_else_the_matter_qualifies() {
        let asks = selector(json!({ "labels": ["enhancement", "!GenAI"] }));
        assert!(asks.matches(&labelled(json!(["enhancement"])), &Facts::default()));
        assert!(!asks.matches(
            &labelled(json!(["enhancement", "GenAI"])),
            &Facts::default()
        ));
        // The negative alone still refuses the label, and asks nothing else.
        let only_negative = selector(json!({ "labels": ["!GenAI"] }));
        assert!(only_negative.matches(&labelled(json!(["priority"])), &Facts::default()));
        assert!(only_negative.matches(&labelled(json!([])), &Facts::default()));
        assert!(!only_negative.matches(&labelled(json!(["GenAI"])), &Facts::default()));
    }

    /// §FS-005-dispatch.31: `!` alone reads as a filter and excludes nothing,
    /// so it is refused where the recipe is read rather than matching every
    /// matter. Omitting the field is how a reader asks for neither.
    #[test]
    fn an_entry_that_names_nothing_is_refused() {
        for entry in ["!", "", "!!"] {
            let refused = serde_json::from_value::<Selector>(json!({ "labels": [entry] }))
                .expect_err("an entry naming nothing is refused");
            assert!(refused.to_string().contains("names nothing"), "{refused}");
            assert!(
                serde_json::from_value::<Selector>(json!({ "assignees": [entry] })).is_err(),
                "`{entry}` is refused on assignees as it is on labels"
            );
        }
        // A name behind the bang is the whole point, and still parses.
        assert_eq!(
            selector(json!({ "labels": ["!GenAI"] })).labels,
            vec!["!GenAI".to_string()]
        );
    }

    /// §FS-005-dispatch.31: a source that reported nothing has not said the
    /// matter is unlabelled, so both forms refuse rather than match.
    #[test]
    fn a_fact_nobody_reported_refuses_both_forms() {
        let unreported = item(ItemKind::Issue, Some(ItemRole::Author));
        assert!(
            !selector(json!({ "labels": ["enhancement"] })).matches(&unreported, &Facts::default())
        );
        assert!(!selector(json!({ "labels": ["!GenAI"] })).matches(&unreported, &Facts::default()));
        assert!(
            !selector(json!({ "assignees": ["kimeta"] })).matches(&unreported, &Facts::default())
        );

        let refusals =
            selector(json!({ "labels": ["!GenAI"] })).explain(&unreported, &Facts::default());
        assert_eq!(refusals.len(), 1);
        assert_eq!(refusals[0].field, "labels");
        assert!(refusals[0].reason.contains("reported nothing"));
    }

    #[test]
    fn assignees_ask_who_holds_the_matter() {
        let mut held = item(ItemKind::Issue, Some(ItemRole::Author));
        held.raw = json!({ "assignees": ["kimeta", "octocat"] });
        assert!(selector(json!({ "assignees": ["kimeta"] })).matches(&held, &Facts::default()));
        assert!(
            !selector(json!({ "assignees": ["someone-else"] })).matches(&held, &Facts::default())
        );
        assert!(!selector(json!({ "assignees": ["!octocat"] })).matches(&held, &Facts::default()));

        // Reported and empty is a matter nobody holds, which answers both forms.
        let mut unheld = item(ItemKind::Issue, Some(ItemRole::Author));
        unheld.raw = json!({ "assignees": [] });
        assert!(!selector(json!({ "assignees": ["kimeta"] })).matches(&unheld, &Facts::default()));
        assert!(selector(json!({ "assignees": ["!kimeta"] })).matches(&unheld, &Facts::default()));
    }

    /// The refusal names the field a reader would edit (§FS-005-dispatch.27).
    #[test]
    fn a_refused_label_says_what_the_matter_carried() {
        let refusals = selector(json!({ "labels": ["enhancement", "!GenAI"] })).explain(
            &labelled(json!(["enhancement", "GenAI"])),
            &Facts::default(),
        );
        assert_eq!(refusals.len(), 1);
        assert_eq!(refusals[0].field, "labels");
        assert!(
            refusals[0].reason.contains("`GenAI`"),
            "{}",
            refusals[0].reason
        );
    }

    fn ids_with(recipes: &[Recipe], item: &Item, facts: Facts) -> Vec<String> {
        applicable(recipes, item, &facts)
            .into_iter()
            .map(|recipe| recipe.id)
            .collect()
    }

    #[test]
    fn the_red_gate_recipe_is_offered_on_my_own_failing_change_only() {
        let recipes = shipped();
        let mine = with_gate(item(ItemKind::Pr, Some(ItemRole::Author)), 2, false);
        assert!(ids(&recipes, &mine).contains(&"fix-gate".to_string()));

        // Green: nothing to fix.
        let green = with_gate(item(ItemKind::Pr, Some(ItemRole::Author)), 0, false);
        assert!(!ids(&recipes, &green).contains(&"fix-gate".to_string()));

        // Someone else's change with a red gate is not mine to fix.
        let theirs = with_gate(item(ItemKind::Pr, Some(ItemRole::Reviewer)), 2, false);
        assert_eq!(ids(&recipes, &theirs), ["review"]);

        // A gate whose jobs all passed and which the forge refuses anyway is
        // waiting on a person, not on a fix.
        let blocked = with_gate(item(ItemKind::Pr, Some(ItemRole::Author)), 0, true);
        assert!(!ids(&recipes, &blocked).contains(&"fix-gate".to_string()));
        // Both at once is still work for a checkout.
        let both = with_gate(item(ItemKind::Pr, Some(ItemRole::Author)), 2, true);
        assert!(ids(&recipes, &both).contains(&"fix-gate".to_string()));
    }

    #[test]
    fn the_two_ways_a_gate_is_red_are_separate_conditions() {
        let selector = |gate: &str| Selector {
            gate: Some(gate.to_string()),
            ..Selector::default()
        };
        let failing = with_gate(item(ItemKind::Pr, None), 2, false);
        let refused = with_gate(item(ItemKind::Pr, None), 0, true);
        let clean = with_gate(item(ItemKind::Pr, None), 0, false);

        assert!(selector("failing").matches(&failing, &Facts::default()));
        assert!(!selector("failing").matches(&refused, &Facts::default()));
        assert!(selector("blocked").matches(&refused, &Facts::default()));
        assert!(!selector("blocked").matches(&failing, &Facts::default()));
        assert!(
            selector("red").matches(&failing, &Facts::default())
                && selector("red").matches(&refused, &Facts::default())
        );
        assert!(
            selector("green").matches(&clean, &Facts::default())
                && !selector("green").matches(&refused, &Facts::default())
        );
        assert!(selector("any").matches(&clean, &Facts::default()));
        // No gate at all answers no gate question.
        assert!(!selector("any").matches(&item(ItemKind::Pr, None), &Facts::default()));
    }

    /// A project's own task carries no role at all (§FS-003-feed-categories.1),
    /// and a `roles` selector matches a role-less item only when it is empty
    /// — that does not change here (§FS-005-dispatch.27). What changes is
    /// that the refusal now says so, naming `roles` and that the matter
    /// carries no role, instead of leaving the exclusion silent.
    #[test]
    fn a_role_less_item_explains_the_roles_refusal_by_name() {
        let wants_author = Selector {
            roles: vec!["author".to_string()],
            ..Selector::default()
        };
        let task = item(ItemKind::Task, None);

        assert!(!wants_author.matches(&task, &Facts::default()));
        let refused = wants_author.explain(&task, &Facts::default());
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].field, "roles");
        assert!(refused[0].reason.contains("carries no role"));
        assert!(refused[0].reason.contains("`author`"));

        // An empty `roles` still matches the same role-less item, and
        // explains nothing because nothing refused.
        let no_roles = Selector::default();
        assert!(no_roles.matches(&task, &Facts::default()));
        assert!(no_roles.explain(&task, &Facts::default()).is_empty());

        // A role that is merely the wrong one is named the same way, without
        // the role-less wording.
        let reviewer = item(ItemKind::Pr, Some(ItemRole::Reviewer));
        let wants_reviewer_role = wants_author.explain(&reviewer, &Facts::default());
        assert_eq!(wants_reviewer_role.len(), 1);
        assert_eq!(wants_reviewer_role[0].field, "roles");
        assert!(wants_reviewer_role[0].reason.contains("`reviewer`"));
        assert!(!wants_reviewer_role[0].reason.contains("carries no role"));
    }

    #[test]
    fn finished_work_is_never_dispatched() {
        let recipes = shipped();
        let mut merged = with_gate(item(ItemKind::Pr, Some(ItemRole::Author)), 2, false);
        merged.state = Some("merged".to_string());
        assert!(ids(&recipes, &merged).is_empty());
    }

    #[test]
    fn an_owed_answer_is_recognized_wherever_it_arrived() {
        let recipes = shipped();
        for kind in [ItemKind::Pr, ItemKind::Issue, ItemKind::Message] {
            let mut waiting = item(kind, None);
            waiting.needs_response = true;
            assert!(
                ids(&recipes, &waiting).contains(&"answer".to_string()),
                "{kind:?}"
            );
        }
        // A status line is not a conversation.
        let mut status = item(ItemKind::Status, None);
        status.needs_response = true;
        assert!(ids(&recipes, &status).is_empty());
    }

    /// The rebase is offered on what has actually fallen behind, and nothing
    /// else (§FS-004-quick-actions.6).
    #[test]
    fn the_rebase_recipe_is_offered_only_where_the_branch_trails_main() {
        let recipes = shipped();
        let rebase = "rebase".to_string();
        let mine = item(ItemKind::Pr, Some(ItemRole::Author));

        // Nothing measured — no branch, or nothing on disk — is not an
        // invitation to guess.
        assert!(!ids(&recipes, &mine).contains(&rebase));
        // Level with main: replaying it would be a ticket to do nothing.
        assert!(!ids_with(
            &recipes,
            &mine,
            Facts {
                behind: Some(0),
                ..Facts::default()
            }
        )
        .contains(&rebase));
        assert!(ids_with(
            &recipes,
            &mine,
            Facts {
                behind: Some(3),
                ..Facts::default()
            }
        )
        .contains(&rebase));

        // The kind of row that mentions the branch, and whose change the forge
        // says it is, do not enter it: this is the one recipe a menu entry
        // dispatches by name, and the entry asks about a branch on disk that
        // trails its base and nothing else (§FS-005-dispatch.1,
        // §FS-004-quick-actions.6). Gating the two differently would mean the
        // key handing over work its own recipe says does not apply here.
        for (kind, role) in [
            (ItemKind::Pr, Some(ItemRole::Reviewer)),
            (ItemKind::Issue, Some(ItemRole::Author)),
            (ItemKind::Message, None),
            (ItemKind::Status, None),
        ] {
            assert!(
                ids_with(
                    &recipes,
                    &item(kind, role),
                    Facts {
                        behind: Some(3),
                        ..Facts::default()
                    }
                )
                .contains(&rebase),
                "{kind:?} {role:?}"
            );
        }

        // A red gate on the same pull request is the more urgent thing about
        // it, and a sweep takes the first match.
        let failing = with_gate(item(ItemKind::Pr, Some(ItemRole::Author)), 2, false);
        assert_eq!(
            ids_with(
                &recipes,
                &failing,
                Facts {
                    behind: Some(3),
                    ..Facts::default()
                }
            )
            .first(),
            Some(&"fix-gate".to_string())
        );

        // Merged, and behind by a mile: there is nothing to replay onto.
        let mut merged = item(ItemKind::Pr, Some(ItemRole::Author));
        merged.state = Some("merged".to_string());
        assert!(ids_with(
            &recipes,
            &merged,
            Facts {
                behind: Some(9),
                ..Facts::default()
            }
        )
        .is_empty());
    }

    #[test]
    fn a_selector_can_ask_for_a_branch_that_is_level_with_main() {
        let level = Selector {
            behind: Some(false),
            ..Selector::default()
        };
        let pr = item(ItemKind::Pr, None);
        assert!(level.matches(
            &pr,
            &Facts {
                behind: Some(0),
                ..Facts::default()
            }
        ));
        assert!(!level.matches(
            &pr,
            &Facts {
                behind: Some(2),
                ..Facts::default()
            }
        ));
        // Unmeasurable answers neither question.
        assert!(!level.matches(&pr, &Facts::default()));
    }

    /// The other distance is asked about in the same words, and answered from
    /// the same fold (§FS-004-quick-actions.8). The two are separate
    /// questions: a branch level with main can be well behind its own copy.
    #[test]
    fn a_selector_can_ask_about_the_published_copy_instead_of_main() {
        let trails_copy = Selector {
            behind_upstream: Some(true),
            ..Selector::default()
        };
        let pr = item(ItemKind::Pr, None);
        assert!(trails_copy.matches(
            &pr,
            &Facts {
                behind: Some(0),
                behind_upstream: Some(2),
            }
        ));
        assert!(!trails_copy.matches(
            &pr,
            &Facts {
                behind: Some(9),
                behind_upstream: Some(0),
            }
        ));
        // Published nowhere answers neither, like an unmeasurable checkout.
        assert!(!trails_copy.matches(&pr, &Facts::default()));

        // And both at once ask for both.
        let both = Selector {
            behind: Some(true),
            behind_upstream: Some(true),
            ..Selector::default()
        };
        assert!(both.matches(
            &pr,
            &Facts {
                behind: Some(1),
                behind_upstream: Some(2),
            }
        ));
        assert!(!both.matches(
            &pr,
            &Facts {
                behind: Some(0),
                behind_upstream: Some(2),
            }
        ));
    }

    #[test]
    fn configuration_adds_recipes_and_replaces_a_shipped_one_by_id() {
        let configured: Vec<Recipe> = serde_json::from_value(json!([
            { "id": "fix-gate", "description": "our own gate fix", "brief": "do it our way" },
            { "id": "implement", "description": "our own implementation", "brief": "do it our way",
              "branch": "work/issue-{number}" },
            { "id": "bench", "description": "run the benchmarks", "brief": "bench {title}",
              "when": { "kinds": ["pr"] }, "state": "fix" }
        ]))
        .unwrap();
        let resolved = resolve(&configured, &[], &[]);
        let fix = resolved.iter().find(|r| r.id == "fix-gate").unwrap();
        assert_eq!(fix.description, "our own gate fix");
        let implement = resolved.iter().find(|r| r.id == "implement").unwrap();
        assert_eq!(implement.branch.as_deref(), Some("work/issue-{number}"));
        // Replacing keeps the position; the new one lands at the end.
        assert_eq!(resolved.len(), shipped().len() + 1);
        assert_eq!(resolved.last().unwrap().id, "bench");
        // The replacement's own selector applies — no gate condition now.
        let plain = item(ItemKind::Pr, None);
        assert!(ids(&resolved, &plain).contains(&"fix-gate".to_string()));

        let shipped = shipped();
        let shipped_implement = shipped.iter().find(|r| r.id == "implement").unwrap();
        assert_eq!(
            shipped_implement.branch.as_deref(),
            Some("fix/issue-{number}")
        );
        assert!(!shipped_implement.autorun);
        assert!(shipped
            .iter()
            .filter(|recipe| recipe.id != "implement")
            .all(|recipe| recipe.branch.is_none()));
    }

    /// §FS-005-dispatch.1: the block an organization's projects share takes
    /// the recipes they share, beside the work root and the ceilings that are
    /// already written there. The block is `deny_unknown_fields`, so until it
    /// does, a site configuration carrying the key is not partly read — it is
    /// refused whole, and with it every other thing that file says.
    #[test]
    fn issue_119_an_organization_block_takes_the_recipes_its_projects_share() {
        let block = serde_json::from_value::<OrganizationWorkConfig>(json!({
            "root": "{org_root}/panta",
            "recipes": [{
                "id": "fix-gate",
                "description": "Fix a failing gate",
                "brief": "A gate is red on a branch you authored. Make it green.",
                "when": { "kinds": ["ci"] }
            }]
        }));
        assert!(
            block.is_ok(),
            "the organization block refused the recipes its projects share: {:?}",
            block.err()
        );
    }

    /// A recipe may select a whole work-root template of its own; omission is
    /// the compatibility path through project, organization and site
    /// placement (§FS-005-dispatch.1, §FS-005-dispatch.6.1).
    #[test]
    fn issue_43_a_recipe_accepts_an_optional_root_template() {
        let placed = serde_json::from_value::<Recipe>(json!({
            "id": "fix", "description": "fix it", "brief": "Fix {title}.",
            "root": "{workspace}/panta"
        }));
        assert!(
            placed.is_ok(),
            "a recipe root is valid configuration: {placed:?}"
        );

        let omitted = serde_json::from_value::<Recipe>(json!({
            "id": "sweep", "description": "sweep it", "brief": "Sweep."
        }));
        assert!(
            omitted.is_ok(),
            "omitting root keeps existing recipes valid"
        );
    }

    #[test]
    fn autorun_concurrency_caps_parse_at_all_three_scopes() {
        let site: WorkConfig = serde_json::from_value(json!({ "max_concurrent": 3 })).unwrap();
        let organization: OrganizationWorkConfig =
            serde_json::from_value(json!({ "max_concurrent": 2 })).unwrap();
        let project: ProjectWorkConfig =
            serde_json::from_value(json!({ "max_concurrent": 0 })).unwrap();
        assert_eq!(site.max_concurrent, Some(3));
        assert_eq!(organization.max_concurrent, Some(2));
        assert_eq!(project.max_concurrent, Some(0));
        assert_eq!(WorkConfig::default().max_concurrent, None);
        assert_eq!(OrganizationWorkConfig::default().max_concurrent, None);
        assert_eq!(ProjectWorkConfig::default().max_concurrent, None);
        assert!(serde_json::from_value::<WorkConfig>(json!({ "max_concurrant": 1 })).is_err());
        assert!(
            serde_json::from_value::<OrganizationWorkConfig>(json!({ "max_concurrant": 1 }))
                .is_err()
        );
        assert!(
            serde_json::from_value::<ProjectWorkConfig>(json!({ "max_concurrant": 1 })).is_err()
        );

        // The agent ceiling is the same pair, read the same way, and is
        // omitted by default so that a site naming only the other key is
        // bounded exactly as it was (§FS-005-dispatch.24).
        let site: WorkConfig =
            serde_json::from_value(json!({ "max_concurrent": 4, "max_active": 2 })).unwrap();
        let project: ProjectWorkConfig =
            serde_json::from_value(json!({ "max_active": 0 })).unwrap();
        assert_eq!(site.max_active, Some(2));
        assert_eq!(project.max_active, Some(0));
        assert_eq!(project.max_concurrent, None);
        assert_eq!(WorkConfig::default().max_active, None);
        assert_eq!(ProjectWorkConfig::default().max_active, None);
        assert!(serde_json::from_value::<ProjectWorkConfig>(json!({ "max_activ": 1 })).is_err());
    }

    /// The spend ceilings are two more keys on the same three blocks, read the
    /// same way: omitted is unlimited, and a site that names none is bounded
    /// exactly as it was (§FS-015-spend-ceiling.1, §FS-015-spend-ceiling.4).
    #[test]
    fn spend_ceilings_parse_at_all_three_scopes() {
        let site: WorkConfig = serde_json::from_value(json!({
            "max_spend": { "amount": 50, "currency": "USD", "per": "24h" },
            "max_tokens": { "amount": 200000000, "per": "24h" }
        }))
        .unwrap();
        let organization: OrganizationWorkConfig = serde_json::from_value(json!({
            "max_spend": { "amount": 20, "currency": "USD", "per": "7d" }
        }))
        .unwrap();
        let project: ProjectWorkConfig = serde_json::from_value(json!({
            "max_tokens": { "amount": 0, "per": "1h" }
        }))
        .unwrap();
        assert_eq!(site.max_spend.map(|budget| budget.amount), Some(50.0));
        assert_eq!(site.max_tokens.map(|budget| budget.amount), Some(200000000));
        assert_eq!(
            organization.max_spend.map(|budget| budget.per),
            Some(crate::work::spend::Per::Week)
        );
        assert_eq!(project.max_tokens.map(|budget| budget.amount), Some(0));
        assert_eq!(project.max_spend, None);
        assert_eq!(WorkConfig::default().max_spend, None);
        assert_eq!(WorkConfig::default().max_tokens, None);
        assert_eq!(OrganizationWorkConfig::default().max_tokens, None);
        assert_eq!(ProjectWorkConfig::default().max_spend, None);
        assert!(serde_json::from_value::<WorkConfig>(json!({ "max_spent": 1 })).is_err());
    }

    /// The table a project writes to say who does what
    /// (§FS-006-project-interface.9): action id to hand, `default` for the
    /// rest, the short form for a hand the roster names and the long one for a
    /// pair it never enumerated.
    #[test]
    fn who_does_which_action_is_a_table_of_hands() {
        let work: ProjectWorkConfig = serde_json::from_value(json!({
            "hands": {
                "default": "sonnet",
                "rebase": "luna:high",
                "fix-gate": { "agent": "claude-code", "model": "our-proxy-model", "effort": "high" }
            },
            "permitted_hands": ["sonnet", "luna"]
        }))
        .unwrap();
        assert_eq!(
            work.hands["default"].members(),
            [HandPin::Named {
                id: "sonnet".to_string(),
                effort: None
            }]
        );
        assert_eq!(
            work.hands["rebase"].members(),
            [HandPin::Named {
                id: "luna".to_string(),
                effort: Some("high".to_string())
            }]
        );
        assert_eq!(
            work.hands["fix-gate"].members(),
            [HandPin::Spelled {
                agent: "claude-code".to_string(),
                model: "our-proxy-model".to_string(),
                effort: Some("high".to_string())
            }]
        );
        assert_eq!(work.permitted_hands, ["sonnet", "luna"]);
        assert_eq!(work.hands["rebase"].describe(), "'luna' at effort 'high'");

        // The same table at site level, and a recipe pinning its own hand.
        let site: WorkConfig = serde_json::from_value(json!({
            "hands": { "default": "sonnet" },
            "recipes": [{ "id": "bench", "description": "d", "brief": "b", "hand": "gpt-5:high" }]
        }))
        .unwrap();
        assert_eq!(site.hands.len(), 1);
        assert_eq!(
            site.recipes[0].hand.as_ref().unwrap().first().effort(),
            Some("high")
        );
        // And it survives the round trip a recipe makes through JSON.
        assert_eq!(
            serde_json::to_value(&site.recipes[0]).unwrap()["hand"],
            json!("gpt-5:high")
        );

        // Absence is the ordinary case: no table anywhere names nobody.
        assert!(WorkConfig::default().hands.is_empty());
        assert!(ProjectWorkConfig::default().permitted_hands.is_empty());
    }

    /// What a hand is not: the runtime's own selector, half a pair, or a
    /// spelling with nothing after the colon. Each fails at the parse, where
    /// the person can still see what they wrote.
    #[test]
    fn what_cannot_be_a_hand_fails_loudly() {
        for text in [
            "claude-code[yolo]:anthropic:sonnet",
            "sonnet:",
            ":high",
            "  ",
        ] {
            assert!(HandPin::parse(text).is_err(), "{text}");
        }
        assert!(serde_json::from_value::<HandPin>(json!({ "agent": "codex" })).is_err());
        assert!(serde_json::from_value::<HandPin>(json!({ "model": "m5" })).is_err());
        assert!(
            serde_json::from_value::<HandPin>(json!({ "agent": "codex", "modle": "m5" })).is_err()
        );
        assert!(serde_json::from_value::<ProjectWorkConfig>(json!({ "hand": "sonnet" })).is_err());
    }

    #[test]
    fn a_recipe_typo_fails_loudly() {
        assert!(serde_json::from_value::<Recipe>(
            json!({ "id": "x", "description": "d", "brief": "b", "kinds": ["pr"] })
        )
        .is_err());
    }

    // ---- a recipe's own sweep (§FS-005-dispatch.32) ----

    fn recipe_with(dispatch: serde_json::Value) -> serde_json::Value {
        json!({ "id": "implement", "description": "d", "brief": "b", "dispatch": dispatch })
    }

    fn sweep(dispatch: serde_json::Value) -> Sweep {
        serde_json::from_value::<Recipe>(recipe_with(dispatch))
            .unwrap()
            .dispatch
            .unwrap()
    }

    fn moment(minutes: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + minutes * 60, 0).unwrap()
    }

    /// Silence is how a recipe declines, one step earlier than `autorun`
    /// (§FS-005-dispatch.32).
    #[test]
    fn a_recipe_that_says_nothing_sweeps_for_nobody() {
        let quiet: Recipe =
            serde_json::from_value(json!({ "id": "q", "description": "d", "brief": "b" })).unwrap();
        assert!(quiet.dispatch.is_none());
    }

    #[test]
    fn the_interval_alone_is_the_spelling_to_prefer() {
        let every = sweep(json!("6h"));
        assert_eq!(every.limit, None);
        assert_eq!(every.describe(), "every 6h");
        // Not due until the interval has actually passed.
        assert!(!every.due(Some(moment(0)), moment(359)));
        assert!(every.due(Some(moment(0)), moment(360)));
    }

    /// Zero has elapsed by the time anything reads it, so it falls out of the
    /// same comparison rather than being a mode beside it
    /// (§FS-005-dispatch.32).
    #[test]
    fn zero_is_due_every_time_it_is_asked() {
        let always = sweep(json!("0h"));
        assert!(always.due(Some(moment(0)), moment(0)));
        assert!(always.due(Some(moment(10)), moment(10)));
        assert_eq!(always.describe(), "every sweep");
    }

    /// A record nobody wrote means due now: the cost is one early sweep, and
    /// the other direction is a queue that stops being looked at.
    #[test]
    fn a_recipe_nothing_was_recorded_about_is_due() {
        assert!(sweep(json!("7d")).due(None, moment(0)));
    }

    #[test]
    fn every_unit_is_read_and_a_day_is_a_day() {
        assert!(sweep(json!("30m")).due(Some(moment(0)), moment(30)));
        assert!(!sweep(json!("30m")).due(Some(moment(0)), moment(29)));
        assert!(sweep(json!("1d")).due(Some(moment(0)), moment(1440)));
        assert!(!sweep(json!("1d")).due(Some(moment(0)), moment(1439)));
    }

    /// `due` asks how long it has been rather than when it would next be, so
    /// even the longest rhythm the parser lets through answers instead of
    /// running the calendar off its end.
    #[test]
    fn the_longest_rhythm_still_answers() {
        // A fat-fingered interval: it reads, because the calendar can hold it,
        // and then it is simply never due. Asked the other way round — the
        // instant it would next be — this walked off the end of the calendar.
        let typo = sweep(json!("100000000d"));
        assert!(!typo.due(Some(moment(0)), moment(1)));

        let far = Sweep {
            every: Sweep::longest(),
            limit: None,
        };
        assert!(!far.due(Some(moment(0)), moment(1)));
        assert!(far.due(None, moment(1)));
    }

    /// The map is for the recipe that wants the bound; the string is sugar for
    /// it with no bound (§FS-005-dispatch.32.4).
    #[test]
    fn the_long_spelling_carries_a_limit() {
        let bounded = sweep(json!({ "every": "6h", "limit": 3 }));
        assert_eq!(bounded.limit, Some(3));
        assert_eq!(bounded.describe(), "every 6h, at most 3");
        assert!(bounded.due(Some(moment(0)), moment(360)));
        assert_eq!(sweep(json!({ "every": "6h" })).limit, None);
    }

    /// A rhythm nobody can read would sweep at a rate its author never chose,
    /// and this is the field that spends agents unattended — so it refuses
    /// rather than defaulting.
    #[test]
    fn an_interval_that_is_not_one_is_refused() {
        for bad in [
            json!("6"),         // no unit
            json!("h"),         // no number
            json!(""),          // nothing at all
            json!("6y"),        // a unit nothing here means
            json!("-1h"),       // backwards
            json!("6 h"),       // not one token
            json!("six hours"), // words
            json!(6),           // a bare number is not an interval
            json!(["6h"]),      // nor a list
            json!("6µ"),        // a unit whose last character is not one byte
            json!("6”"),        // nor is a quote a paste picked up
            json!("6д"),        // nor a letter from another alphabet
        ] {
            assert!(
                serde_json::from_value::<Recipe>(recipe_with(bad.clone())).is_err(),
                "{bad} should not read as an interval"
            );
        }
    }

    #[test]
    fn the_long_spelling_still_needs_an_interval_it_can_read() {
        assert!(serde_json::from_value::<Recipe>(recipe_with(json!({ "limit": 3 }))).is_err());
        assert!(serde_json::from_value::<Recipe>(recipe_with(json!({ "every": "6y" }))).is_err());
        // And refuses a key it does not know, as every other block here does.
        assert!(serde_json::from_value::<Recipe>(recipe_with(
            json!({ "every": "6h", "unattended": true })
        ))
        .is_err());
    }

    /// Read and written back in the spelling it arrived in, so a recipe that
    /// round-trips does not change shape on disk.
    #[test]
    fn a_sweep_serializes_as_it_was_written() {
        let round = |value: serde_json::Value| serde_json::to_value(sweep(value)).unwrap();
        assert_eq!(round(json!("6h")), json!("6h"));
        // Zero keeps the documented spelling rather than degrading to "0m".
        assert_eq!(round(json!("0h")), json!("0h"));
        assert_eq!(round(json!("90m")), json!("90m"));
        assert_eq!(round(json!("2d")), json!("2d"));
        assert_eq!(
            round(json!({ "every": "6h", "limit": 3 })),
            json!({ "every": "6h", "limit": 3 })
        );
    }
}
