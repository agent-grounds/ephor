//! The dossier: what ephor already knows about an item, written into its
//! ticket (§FS-005-dispatch.2).
//!
//! A ticket that says "look at pull request 42" has handed the whole job back.
//! Everything the work opens with — the state, the branch, the gate's counts
//! and the forge's reasons, the conversation — was fetched during a refresh
//! and is on disk. So it goes into the plan, and the work starts where a
//! person would have started.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::branches::{Checkout, Organization, WorkspaceState};
use crate::feed::gate::Gate;
use crate::feed::model::Item;
use crate::work::recipe::Recipe;

/// How much conversation a ticket quotes (§FS-005-dispatch.2). A transcript is
/// not evidence: what is dropped is said, and the url reaches the rest.
const MESSAGES_PER_THREAD: usize = 8;
const TOTAL_MESSAGES: usize = 24;
const MESSAGE_CHARS: usize = 1600;
/// What every thread keeps before any thread gets a second helping, so no
/// thread is dropped whole for having been last.
const RESERVED_PER_THREAD: usize = 2;

/// One item, with where its work belongs — everything both the dossier and a
/// recipe's brief are rendered from.
pub struct Subject<'a> {
    pub item: &'a Item,
    pub checkout: &'a Checkout,
    pub root: &'a Path,
    /// The organization the registry places this item's project in, where it
    /// places it in one (§FS-005-dispatch.6.1) — what `{org}` and `{org_root}`
    /// are rendered from, and what a work root reaching above the project is
    /// refused against.
    pub organization: Option<&'a Organization>,
}

/// The placeholders that reach above the project, to the organization the
/// registry places it in (§FS-005-dispatch.6.1). Named apart because a work
/// root that holds one of them cannot be rendered without an answer: a path is
/// refused where prose would have carried the gap.
pub const ORGANIZATION_PLACEHOLDERS: [&str; 2] = ["org", "org_root"];

impl Subject<'_> {
    /// The fields a brief may name as `{placeholder}`.
    pub fn placeholders(&self) -> BTreeMap<&'static str, String> {
        let item = self.item;
        let gate = Gate::of(item);
        BTreeMap::from([
            ("title", item.title.clone()),
            ("project", item.project.clone()),
            ("source", item.source.clone()),
            ("kind", item.kind.label().to_string()),
            ("id", item.id.clone()),
            ("url", item.url.clone().unwrap_or_default()),
            ("state", item.state.clone().unwrap_or_default()),
            ("repo", item.repo().unwrap_or_default()),
            ("number", item.number().unwrap_or_default()),
            ("branch", self.checkout.branch.clone().unwrap_or_default()),
            ("ticket", self.checkout.ticket.clone().unwrap_or_default()),
            (
                "workspace",
                self.checkout.workspace.to_string_lossy().into_owned(),
            ),
            ("root", self.root.to_string_lossy().into_owned()),
            ("gate", gate.as_ref().map(Gate::summary).unwrap_or_default()),
            (
                "org",
                self.organization
                    .map(|org| org.id.clone())
                    .unwrap_or_default(),
            ),
            (
                "org_root",
                self.organization
                    .and_then(|org| org.root.as_ref())
                    .map(|root| root.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
        ])
    }

    /// Where a work-root template puts this item's work
    /// (§FS-005-dispatch.6.1). Everything the dossier can name is nameable
    /// here, so this is [`render`] with one thing held to first: a template
    /// reaching above the project to an organization that cannot answer is
    /// refused by name, because rendering it would write a directory called
    /// `{org_root}` or a path with a segment missing.
    pub fn work_root(&self, template: &str) -> std::result::Result<std::path::PathBuf, String> {
        if let Some(why) = organization_gap(template, &self.item.project, self.organization) {
            return Err(why);
        }
        let values = self.placeholders();
        if let Some(name) = named(template)
            .into_iter()
            .find(|name| !values.contains_key(name.as_str()))
        {
            return Err(format!(
                "{}: the work root template '{template}' names {{{name}}}, which is not a \
                 placeholder this runtime knows",
                self.item.project
            ));
        }
        Ok(crate::paths::resolve_path(&render(template, &values)))
    }

    /// The item as data rather than as prose, for the programs in a state
    /// machine (§FS-005-dispatch.8). The same names a shell action gets in its
    /// environment, so one vocabulary covers both.
    pub fn metadata(&self) -> Vec<(&'static str, String)> {
        let values = self.placeholders();
        [
            "project",
            "source",
            "kind",
            "id",
            "url",
            "state",
            "repo",
            "number",
            "branch",
            "ticket",
            "workspace",
            "root",
            "title",
        ]
        .iter()
        .filter_map(|key| values.get(key).map(|value| (*key, value.clone())))
        .collect()
    }

    /// The whole dossier, as the markdown that opens the plan.
    pub fn dossier(&self) -> String {
        let mut out = String::from("## The item\n\n");
        out.push_str(&self.facts());
        if let Some(gate) = Gate::of(self.item) {
            out.push('\n');
            out.push_str(&render_gate(&gate));
        }
        let threads = threads_of(self.item);
        if threads.iter().any(|thread| !thread.messages.is_empty()) {
            out.push('\n');
            out.push_str(&render_threads(&threads, self.item.url.as_deref()));
        } else if expects_conversation(self.item.kind) {
            // An empty section reads as "there was nothing to say", and work
            // asked to answer a conversation that is merely unrecorded would
            // answer the silence (§FS-001-forge-interface.6). ephor does not
            // fetch an item's own description yet
            // (§RM-002-dossier-description), so for an issue this is most of
            // what is missing.
            out.push_str(
                "\n## The conversation\n\nThe watch recorded none of it — which is not the \
                 same as there being none.\n",
            );
            if let Some(url) = &self.item.url {
                out.push_str(&format!(
                    "Read {url} before anything else: the item's own description is there, and \
                     it is not here.\n"
                ));
            }
        }
        out
    }

    fn facts(&self) -> String {
        let item = self.item;
        let mut rows: Vec<(&str, String)> = vec![
            ("project", item.project.clone()),
            (
                "kind",
                match item.role {
                    Some(role) => format!("{} ({role:?})", item.kind.label()).to_lowercase(),
                    None => item.kind.label().to_string(),
                },
            ),
            ("reported by", item.source.clone()),
        ];
        let mut push = |label: &'static str, value: Option<String>| {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                rows.push((label, value));
            }
        };
        push("state", item.state.clone());
        push("blocked by", Some(item.open_blockers().join(", ")));
        push("url", item.url.clone());
        // Where the branch is not on disk, say so: the work is running in the
        // project's own checkout, and an agent told only the branch name would
        // believe it was standing on it.
        push(
            "branch",
            self.checkout
                .branch
                .clone()
                .map(|branch| match &self.checkout.state {
                    WorkspaceState::Missing(_) => {
                        format!("{branch} — not checked out here; fetch it if you need it")
                    }
                    _ => branch,
                }),
        );
        push("ticket", self.checkout.ticket.clone());
        push(
            "last activity",
            Some(item.updated_at.format("%Y-%m-%d %H:%M UTC").to_string()),
        );
        push(
            "checkout",
            Some(self.checkout.workspace.to_string_lossy().into_owned()),
        );
        if item.needs_response {
            rows.push(("waiting on", "an answer from me".to_string()));
        }

        let width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
        rows.iter()
            .map(|(label, value)| {
                format!("- **{label}**{} {value}\n", " ".repeat(width - label.len()))
            })
            .collect()
    }
}

/// Why a template reaching above the project cannot be rendered into a path
/// for it (§FS-005-dispatch.6.1), naming the placeholder that has no answer
/// and the organization that did not give one. None where the template names
/// neither organization placeholder, and none where the organization answers
/// what the template asked of it — `{org}` needs only the membership, and
/// `{org_root}` needs the root as well.
pub fn organization_gap(
    template: &str,
    project: &str,
    organization: Option<&Organization>,
) -> Option<String> {
    let names = named(template);
    let asked: Vec<&str> = ORGANIZATION_PLACEHOLDERS
        .into_iter()
        .filter(|name| names.iter().any(|held| held == name))
        .collect();
    if asked.is_empty() {
        return None;
    }
    let spelled = asked
        .iter()
        .map(|name| format!("{{{name}}}"))
        .collect::<Vec<_>>()
        .join(" and ");
    let Some(organization) = organization else {
        return Some(format!(
            "{project}: the work root names {spelled}, and no registry row places {project} \
             in an organization — give its row an 'organization', or write a work root that \
             stays inside the project."
        ));
    };
    if asked.contains(&"org_root") && organization.root.is_none() {
        return Some(format!(
            "{project}: the work root names {{org_root}}, and organization {} declares no \
             root — give '{}' a root in the registry, or write a work root that stays inside \
             the project.",
            organization.id, organization.id
        ));
    }
    None
}

/// Render `{placeholder}` occurrences; an unknown name is left as written so a
/// typo shows up in the ticket rather than becoming an empty line.
pub fn render(template: &str, values: &BTreeMap<&'static str, String>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match values.get(name) {
                    Some(value) => out.push_str(value),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Every `{placeholder}` a template names, in the order it names them. The
/// same grammar [`render`] reads, asked before the rendering rather than
/// after it: a template is answered for what it says, not for what its
/// rendering happened to look like.
pub fn named(template: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            return names;
        };
        names.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    names
}

/// The one name a path may never take (§FS-005-dispatch.34): `{reply}` is
/// where ephor writes a proposed answer for the matter
/// (§FS-005-dispatch.13), which is a place rather than a fact about it — and
/// on the sweep that has no matter at all there is no answer to place.
const NOT_IN_A_PATH: &str = "reply";

/// Where a ticket's words came from, where they came from a file
/// (§FS-005-dispatch.34.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    /// The rendered path, as it was read.
    pub path: PathBuf,
    /// A sha256 of the bytes as read, before anything was done to them, so
    /// `sha256sum` over the file on disk gives the same answer.
    pub sha256: String,
}

impl Instruction {
    /// What the ticket records about the ask rather than about the item
    /// (§FS-005-dispatch.34.2, §FS-005-dispatch.8). Per ticket and never in
    /// the dossier: the dossier is rewritten every time the matter reopens
    /// (§FS-005-dispatch.5), and a hash written there would name the latest
    /// dispatch's text while sitting above tickets that were given something
    /// else.
    pub fn metadata(&self) -> Vec<(&'static str, String)> {
        vec![
            ("instruction", self.path.to_string_lossy().into_owned()),
            ("instruction_sha256", self.sha256.clone()),
        ]
    }
}

/// What a recipe asks for, as the words that reach the ticket
/// (§FS-005-dispatch.34).
#[derive(Debug, Clone, Default)]
pub struct Brief {
    /// The whole of it: the instruction file's text first, the rendered
    /// `brief` after it (§FS-005-dispatch.34.1). Whatever a deterministic
    /// opening move reached goes after both, which is the dispatch's to append
    /// because it is what made the move (§FS-005-dispatch.12).
    pub text: String,
    /// Which words this was, where a file said them.
    pub instruction: Option<Instruction>,
}

/// Render a recipe's brief, reading the file it keeps its words in
/// (§FS-005-dispatch.34).
///
/// The whole behaviour is here — render the path, resolve it, read it, flatten
/// it, hash it — because every writer of a brief honours the key and a rule
/// each of them implemented would be a rule the sweep got wrong
/// (§FS-005-dispatch.34.3).
///
/// `values` is the vocabulary both halves are rendered from. The path takes
/// all of it but [`NOT_IN_A_PATH`], and a name it does not answer is refused
/// **by name** rather than written into a path with a segment missing
/// (§FS-005-dispatch.6.1) — which is what holds the unattended sweep, whose
/// values are a checkout's and never a matter's, to the names a checkout can
/// answer. The `brief` beside it is prose and carries the gap as prose always
/// has.
pub fn brief(
    recipe: &Recipe,
    values: &BTreeMap<&'static str, String>,
) -> std::result::Result<Brief, String> {
    let said = recipe
        .brief
        .as_deref()
        .map(|brief| render(brief, values))
        .unwrap_or_default();
    let Some(template) = recipe.brief_file.as_deref() else {
        return Ok(Brief {
            text: said,
            instruction: None,
        });
    };
    let (instruction, read) = read_instruction(recipe, template, values)?;
    // The file's text first and the rendered `brief` after it: the standing
    // instruction says how work is done here, and `brief` says what to do with
    // this matter (§FS-005-dispatch.34.1).
    //
    // Flattened after the hash, never before: the hash is of the bytes as
    // read, so whoever holds the file can say whether this ticket got these
    // words (§FS-005-dispatch.34.2). A heading inside a plan is a node the
    // runtime reads as a task (§FS-005-dispatch.3).
    let text = in_a_body(&String::from_utf8_lossy(&read));
    let text = match said.is_empty() {
        true => text.trim_end().to_string(),
        false => format!("{}\n\n{said}", text.trim_end()),
    };
    Ok(Brief {
        text,
        instruction: Some(instruction),
    })
}

/// The rendered path and the hash of what is behind it, refusing where there
/// is nothing readable behind it — naming the path, and before anything the
/// caller would have written (§FS-005-dispatch.34).
fn read_instruction(
    recipe: &Recipe,
    template: &str,
    values: &BTreeMap<&'static str, String>,
) -> std::result::Result<(Instruction, Vec<u8>), String> {
    let mut answerable = values.clone();
    answerable.remove(NOT_IN_A_PATH);
    if let Some(name) = named(template)
        .into_iter()
        .find(|name| !answerable.contains_key(name.as_str()))
    {
        return Err(format!(
            "recipe '{}': the brief_file template '{template}' names {{{name}}}, which is not \
             a placeholder this runtime can answer here",
            recipe.id
        ));
    }
    let rendered = crate::paths::expand_user_vars(&render(template, &answerable));
    // Relative to the directory holding the configuration file that wrote it,
    // never to the working directory: a recipe with its own sweep
    // (§FS-005-dispatch.32) runs from wherever the unit that called ephor
    // happened to stand, and a brief that depended on that would be a
    // different brief on a timer than under a person (§FS-005-dispatch.34).
    let path = match &recipe.based_in {
        Some(config) => crate::paths::resolve_registry_relative(config, &rendered),
        None => PathBuf::from(rendered),
    };
    let read = std::fs::read(&path).map_err(|err| {
        format!(
            "recipe '{}': cannot read the brief it keeps in {}: {err}",
            recipe.id,
            path.display()
        )
    })?;
    // A file with nothing in it is a path with no brief behind it: the ticket
    // would carry no words, which is the hole a recipe with neither key is
    // refused for (§FS-005-dispatch.34.1).
    if read.iter().all(u8::is_ascii_whitespace) {
        return Err(format!(
            "recipe '{}': the brief it keeps in {} is empty, so the ticket would ask for \
             nothing",
            recipe.id,
            path.display()
        ));
    }
    Ok((
        Instruction {
            sha256: sha256(&read),
            path,
        },
        read,
    ))
}

/// A document as a paragraph of somebody else's (§FS-005-dispatch.3).
///
/// A ticket's body is prose inside a plan, and a heading inside a plan is a
/// *node*: the runtime reads one as a task, fails to parse it, and refuses the
/// whole file — so the plan the writer meant to hand over is a plan nothing can
/// load. The headings become plain emphasis here rather than at each caller,
/// because everything that embeds a document in a body — the rebase report,
/// the hand-over, the sweep, an instruction file a recipe named — would
/// otherwise each have to remember. Fenced content is left exactly as it is:
/// what git said is what git said, an example in an instruction is the example
/// its author wrote, and the plan language skips a fence for the same reason.
pub fn in_a_body(text: &str) -> String {
    let mut out = String::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        let heading = match fenced {
            true => None,
            false => line
                .strip_prefix('#')
                .map(|rest| rest.trim_start_matches('#'))
                .and_then(|rest| rest.strip_prefix(' ')),
        };
        match heading {
            Some(text) => out.push_str(&format!("**{}**\n", text.trim_end())),
            None => out.push_str(&format!("{line}\n")),
        }
    }
    out
}

/// SHA-256 of some bytes, as lowercase hex (FIPS 180-4).
///
/// Written out rather than taken from a crate. The value is worth having only
/// because a reader holding the file can check it (§FS-005-dispatch.34.2), so
/// it has to be the hash `sha256sum` prints and nothing else — and this
/// repository has twice written a digest by hand rather than take a dependency
/// for one ([`crate::sweep`]'s branch fingerprint, the matter store's change
/// detection). Shelling out to `sha256sum` is not available: the tool is not on
/// every platform ephor runs on (§RM-004-windows).
fn sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut hash: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    // The padded message: the bytes, a single one bit, zeroes, and the length
    // in bits as a big-endian u64 — which is what makes two inputs that differ
    // only in trailing zeroes hash differently.
    let mut padded = bytes.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&(bytes.len() as u64 * 8).to_be_bytes());

    for block in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (index, word) in block.chunks_exact(4).enumerate() {
            w[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = hash;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let one = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let two = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(one);
            d = c;
            c = b;
            b = a;
            a = one.wrapping_add(two);
        }
        for (slot, value) in hash.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    hash.iter().map(|word| format!("{word:08x}")).collect()
}

fn render_gate(gate: &Gate) -> String {
    let mut out = String::from("## The gate\n\n");
    out.push_str(&format!("{}\n", gate.summary()));
    if gate.repos.len() > 1 {
        out.push_str(&format!("\n{}\n", gate.breakdown()));
    }
    if !gate.blockers.is_empty() {
        out.push_str("\nThe forge gives these reasons:\n\n");
        for blocker in &gate.blockers {
            out.push_str(&format!("- {}\n", one_line(blocker)));
        }
    }
    out
}

/// A message as a provider recorded it.
struct Message {
    author: String,
    when: String,
    text: String,
}

struct Thread {
    messages: Vec<Message>,
    total: usize,
}

fn threads_of(item: &Item) -> Vec<Thread> {
    let Some(threads) = item.raw.get("threads").and_then(Value::as_array) else {
        return Vec::new();
    };
    let lengths: Vec<usize> = threads
        .iter()
        .map(|thread| {
            thread
                .get("messages")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0)
        })
        .collect();

    // Two passes, so a total budget cannot be eaten by whatever came first.
    // A pull request opens with a bot posting the review policy and the
    // benchmark policy; spending the whole allowance there would drop the
    // human thread underneath it, which is the only one anyone was answering.
    //
    // The reservation is itself spent out of the total rather than added on
    // top of it: a matter with more threads than the budget has room for is
    // still bounded in total (§FS-005-dispatch.2), and the threads that fit
    // are the earlier ones rather than an unbounded tail of two-line
    // fragments.
    let mut budget = TOTAL_MESSAGES;
    let mut keep: Vec<usize> = Vec::with_capacity(lengths.len());
    for length in &lengths {
        let reserved = (*length).min(RESERVED_PER_THREAD).min(budget);
        budget -= reserved;
        keep.push(reserved);
    }
    for (index, length) in lengths.iter().enumerate() {
        let more = (length - keep[index])
            .min(MESSAGES_PER_THREAD - keep[index])
            .min(budget);
        keep[index] += more;
        budget -= more;
    }

    // A thread the budget could not reach at all is still carried, empty: it
    // is not rendered, but its messages count as dropped, so the ticket says
    // what it left out instead of quietly stopping at the budget
    // (§FS-005-dispatch.2, §FS-001-forge-interface.6).
    threads
        .iter()
        .zip(keep)
        .filter_map(|(thread, keep)| {
            let messages = thread.get("messages").and_then(Value::as_array)?;
            // The end of a thread is what is being answered, so a thread that
            // does not fit keeps its last messages rather than its first.
            let kept = messages[messages.len() - keep..]
                .iter()
                .map(|message| Message {
                    author: string_at(message, "author"),
                    when: string_at(message, "when"),
                    text: string_at(message, "text"),
                })
                .collect();
            Some(Thread {
                messages: kept,
                total: messages.len(),
            })
        })
        .collect()
}

/// Kinds whose whole point is what people said about them. A status line has
/// no conversation and saying so about it is noise; neither has the project's
/// own task, which is a heading in a file and not a thread
/// (§FS-006-project-interface.7).
fn expects_conversation(kind: crate::feed::model::ItemKind) -> bool {
    use crate::feed::model::ItemKind;
    matches!(kind, ItemKind::Pr | ItemKind::Issue | ItemKind::Message)
}

fn string_at(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn render_threads(threads: &[Thread], url: Option<&str>) -> String {
    let mut out = String::from("## The conversation\n\n");
    let dropped: usize = threads
        .iter()
        .map(|thread| thread.total - thread.messages.len())
        .sum();
    let shown = threads.iter().filter(|thread| !thread.messages.is_empty());
    let labelled = shown.clone().count() > 1;
    for (index, thread) in shown.enumerate() {
        if labelled {
            out.push_str(&format!("**Thread {}**\n\n", index + 1));
        }
        for message in &thread.messages {
            let when = message.when.split('T').next().unwrap_or("").to_string();
            out.push_str(&format!(
                "**{}**{}\n\n",
                if message.author.is_empty() {
                    "someone"
                } else {
                    &message.author
                },
                if when.is_empty() {
                    String::new()
                } else {
                    format!(" · {when}")
                }
            ));
            out.push_str(&fenced(&clamp(&message.text, MESSAGE_CHARS)));
            out.push('\n');
        }
    }
    if dropped > 0 {
        out.push_str(&format!(
            "_{dropped} earlier message{} not quoted{}._\n",
            if dropped == 1 { "" } else { "s" },
            match url {
                Some(url) => format!("; the whole conversation is at {url}"),
                None => String::new(),
            }
        ));
    }
    out
}

/// A message body, fenced so that nothing inside it can be read as structure.
/// A pull request discussing markdown will contain fences, headings, and lists
/// of its own; a fence one backtick longer than the longest run inside is what
/// keeps the plan a plan.
fn fenced(text: &str) -> String {
    let longest = text.split(|ch| ch != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}\n{}\n{fence}\n", text.trim_end())
}

fn clamp(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let kept: String = text.chars().take(limit).collect();
    format!("{kept}\n… (truncated)")
}

/// Collapse a value to one line: it goes into a bullet, and a forge that
/// writes paragraphs into a blocker would otherwise break the list.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branches::WorkspaceState;
    use crate::feed::gate::RepoGate;
    use crate::feed::model::{ItemKind, ItemRole};
    use serde_json::json;
    use std::path::PathBuf;

    fn checkout() -> Checkout {
        Checkout {
            workspace: PathBuf::from("/w/ABC-42"),
            branch: Some("you/ABC-42-retry".to_string()),
            ticket: Some("ABC-42".to_string()),
            state: WorkspaceState::Ready,
        }
    }

    fn item(raw: Value) -> Item {
        Item {
            id: "github-prs:acme/widget#42".to_string(),
            project: "widget".to_string(),
            source: "github-prs".to_string(),
            kind: ItemKind::Pr,
            role: Some(ItemRole::Author),
            title: "Retry window".to_string(),
            url: Some("https://forge.example/pr/42".to_string()),
            state: Some("open".to_string()),
            needs_response: true,
            updated_at: chrono::Utc::now(),
            raw,
        }
    }

    fn dossier(raw: Value) -> String {
        let item = item(raw);
        let checkout = checkout();
        Subject {
            item: &item,
            checkout: &checkout,
            root: Path::new("/w"),
            organization: None,
        }
        .dossier()
    }

    #[test]
    fn the_dossier_states_the_facts_the_work_opens_with() {
        let text = dossier(json!({}));
        assert!(text.contains("**project**"), "{text}");
        assert!(text.contains("widget"), "{text}");
        assert!(text.contains("you/ABC-42-retry"), "{text}");
        assert!(text.contains("ABC-42"), "{text}");
        assert!(text.contains("/w/ABC-42"), "{text}");
        assert!(text.contains("an answer from me"), "{text}");
        assert!(text.contains("https://forge.example/pr/42"), "{text}");
    }

    #[test]
    fn the_gate_arrives_with_the_forges_own_reasons() {
        let gate = Gate {
            repos: vec![
                RepoGate {
                    repo: "widget".to_string(),
                    passed: 40,
                    failed: 6,
                    running: 0,
                },
                RepoGate {
                    repo: "plugins".to_string(),
                    passed: 118,
                    failed: 0,
                    running: 0,
                },
            ],
            blocked: true,
            blockers: vec!["Requires approvals —\n   still one short.".to_string()],
        };
        let text = dossier(json!({ "gate": gate.to_value() }));
        assert!(text.contains("✗6"), "{text}");
        assert!(text.contains("widget ✓40 ✗6"), "{text}");
        assert!(text.contains("plugins ✓118"), "{text}");
        // Verbatim words, but on one line: it is a bullet.
        assert!(
            text.contains("- Requires approvals — still one short."),
            "{text}"
        );
    }

    #[test]
    fn a_conversation_is_quoted_bounded_and_says_what_it_dropped() {
        let message = |author: &str, text: &str| {
            json!({
                "author": author, "when": "2026-08-11T16:48:45+00:00", "text": text
            })
        };
        let many: Vec<Value> = (0..30)
            .map(|index| message("bot", &format!("message {index}")))
            .collect();
        let text = dossier(json!({
            "threads": [
                { "messages": [message("Ada", "Would it make sense to fix that instead?")] },
                { "messages": many },
            ]
        }));
        assert!(text.contains("**Ada** · 2026-08-11"), "{text}");
        assert!(text.contains("Would it make sense"), "{text}");
        // Bounded, keeping the end of the long thread, and saying so.
        assert!(text.contains("message 29"), "{text}");
        assert!(!text.contains("message 0\n"), "{text}");
        assert!(text.contains("earlier messages not quoted"), "{text}");
        assert!(text.contains("https://forge.example/pr/42"), "{text}");
    }

    /// The bot posts first and at length; the human posts last. A budget
    /// spent in order would quote the policy and drop the question.
    #[test]
    fn no_thread_is_dropped_whole_for_having_come_last() {
        let bot: Vec<Value> = (0..40)
            .map(|index| json!({ "author": "bot", "text": format!("policy {index}") }))
            .collect();
        let text = dossier(json!({
            "threads": [
                { "messages": bot.clone() },
                { "messages": bot },
                { "messages": [{ "author": "Ada", "text": "does this handle windows?" }] },
            ]
        }));
        assert!(text.contains("does this handle windows?"), "{text}");
        // Still bounded: nothing like eighty messages went in.
        assert!(text.matches("**bot**").count() <= 16, "{text}");
        assert!(text.contains("earlier messages not quoted"), "{text}");
    }

    /// The bound is a total, not a per-thread allowance repeated (
    /// §FS-005-dispatch.2). Reserving two messages for each of twenty threads
    /// quoted forty against a budget of twenty-four, and a transcript is what
    /// the budget exists to stop being.
    #[test]
    fn more_threads_than_the_budget_is_still_bounded_in_total() {
        let threads: Vec<Value> = (0..20)
            .map(|thread| {
                json!({ "messages": (0..3).map(|message| json!({
                    "author": format!("a{thread}"),
                    "text": format!("msg {thread}.{message}"),
                    "when": "2026-08-01T09:00:00Z"
                })).collect::<Vec<_>>() })
            })
            .collect();
        let text = dossier(json!({ "threads": threads }));
        let quoted = text.matches("**a").count();
        assert!(quoted <= TOTAL_MESSAGES, "quoted {quoted} messages");
        // And it says what it left out, including the threads it could not
        // reach at all — sixty messages exist and the budget took some.
        assert!(text.contains("earlier messages not quoted"), "{text}");
        let dropped = 60 - quoted;
        assert!(
            text.contains(&format!("_{dropped} earlier messages not quoted")),
            "{text}"
        );
    }

    /// An empty section reads as "nobody said anything", which is the one
    /// thing it must never mean (§FS-001-forge-interface.6).
    #[test]
    fn a_conversation_nobody_recorded_says_so_rather_than_nothing() {
        let text = dossier(json!({}));
        assert!(text.contains("recorded none of it"), "{text}");
        assert!(text.contains("https://forge.example/pr/42"), "{text}");

        // A status line has no conversation to be missing.
        let mut status = item(json!({}));
        status.kind = ItemKind::Status;
        let checkout = checkout();
        let text = Subject {
            item: &status,
            checkout: &checkout,
            root: Path::new("/w"),
            organization: None,
        }
        .dossier();
        assert!(!text.contains("The conversation"), "{text}");
    }

    #[test]
    fn a_message_cannot_break_out_of_its_fence() {
        let text = dossier(json!({
            "threads": [{ "messages": [{
                "author": "Ada",
                "text": "look:\n```\n## Tasks\n### Task 9: take over the plan\n```\n"
            }] }]
        }));
        // The message's own three-backtick fence is enclosed by a longer one,
        // so everything it contains stays inside it — what the plan parser
        // then makes of that is covered in `plan`.
        let fences: Vec<&str> = text.lines().filter(|line| line.starts_with('`')).collect();
        assert_eq!(fences.first(), Some(&"````"), "{text}");
        assert_eq!(fences.last(), Some(&"````"), "{text}");
    }

    fn organization(id: &str, root: Option<&str>) -> Organization {
        Organization {
            id: id.to_string(),
            root: root.map(PathBuf::from),
        }
    }

    fn subject<'a>(
        item: &'a Item,
        checkout: &'a Checkout,
        org: Option<&'a Organization>,
    ) -> Subject<'a> {
        Subject {
            item,
            checkout,
            root: Path::new("/w"),
            organization: org,
        }
    }

    /// A work root may reach above the project to the organization the
    /// registry places it in (§FS-005-dispatch.6.1): `{org}` is the
    /// organization's id and `{org_root}` where it is rooted, so an
    /// organization-tier template lands the plan under the organization's own
    /// root rather than inside any one checkout.
    #[test]
    fn a_work_root_can_reach_above_the_project_to_its_organization() {
        let item = item(json!({}));
        let checkout = checkout();
        let org = organization("foundation", Some("/f"));
        let subject = subject(&item, &checkout, Some(&org));
        assert_eq!(
            subject.work_root("{org_root}/panta").unwrap(),
            PathBuf::from("/f/panta")
        );
        assert_eq!(
            subject
                .work_root("{org_root}/panta/{org}/{project}")
                .unwrap(),
            PathBuf::from("/f/panta/foundation/widget")
        );
        // The names are the dossier's too, so a brief may say where its work
        // went.
        assert_eq!(
            render("work in {org_root} for {org}", &subject.placeholders()),
            "work in /f for foundation"
        );
    }

    /// An organization that declares no root cannot answer `{org_root}`, and a
    /// project no registry row places in an organization cannot answer either
    /// name. Both refuse at dispatch, naming the placeholder and the
    /// organization, because what they would otherwise write is a directory
    /// called `{org_root}` or a path with a segment missing
    /// (§FS-005-dispatch.6.1).
    #[test]
    fn a_work_root_above_a_project_that_cannot_answer_it_refuses_by_name() {
        let item = item(json!({}));
        let checkout = checkout();

        let rootless = organization("personal", None);
        let why = subject(&item, &checkout, Some(&rootless))
            .work_root("{org_root}/panta")
            .expect_err("an organization with no root cannot place work");
        assert!(
            why.contains("organization personal declares no root"),
            "{why}"
        );
        assert!(why.contains("{org_root}"), "{why}");

        let why = subject(&item, &checkout, None)
            .work_root("{org_root}/panta")
            .expect_err("a project in no organization cannot place work either");
        assert!(why.contains("widget"), "{why}");
        assert!(
            why.contains("no registry row places widget in an organization"),
            "{why}"
        );

        let why = subject(&item, &checkout, None)
            .work_root("{root}/work/{org}")
            .expect_err("the id alone has no answer either");
        assert!(why.contains("{org}"), "{why}");

        // Nothing that refuses may also have rendered: no literal
        // `{org_root}` directory, and no empty segment where the answer was.
        // The tiers a template can be written at are all the same render, so
        // a site or project template naming an organization is held to this
        // exactly as an organization-tier one is.
        for (template, org) in [
            ("{org_root}/panta", Some(&rootless)),
            ("{org_root}/panta", None),
            ("{root}/work/{org}", None),
            ("{org}/{org_root}/panta", Some(&rootless)),
        ] {
            match subject(&item, &checkout, org).work_root(template) {
                Ok(dir) => panic!(
                    "{template} rendered to {} instead of refusing",
                    dir.display()
                ),
                Err(why) => assert!(!why.is_empty()),
            }
        }
    }

    /// The refusal is about the *path*. The membership alone answers `{org}`,
    /// so an organization with no root still places work that does not ask for
    /// one — and prose carries an absent organization the way it carries any
    /// other field a matter has not got (§FS-005-dispatch.6.1).
    #[test]
    fn only_the_name_with_no_answer_refuses_and_prose_carries_the_gap() {
        let item = item(json!({}));
        let checkout = checkout();
        let rootless = organization("personal", None);
        assert_eq!(
            subject(&item, &checkout, Some(&rootless))
                .work_root("{root}/work/{org}")
                .unwrap(),
            PathBuf::from("/w/work/personal"),
            "the id is answered even where the root is not"
        );
        assert_eq!(
            subject(&item, &checkout, Some(&rootless))
                .work_root("{root}/panta")
                .unwrap(),
            PathBuf::from("/w/panta"),
            "a template that never reaches above the project is untouched"
        );
        let values = subject(&item, &checkout, None).placeholders();
        assert_eq!(render("for {org}{org_root}!", &values), "for !");
    }

    /// A recipe that keeps its brief in a file, with everything else at its
    /// default. Written here rather than reached for from `shipped()`: what
    /// these cases are about is the two brief keys and nothing else.
    fn keeping(brief: Option<&str>, brief_file: Option<&str>) -> Recipe {
        Recipe {
            id: "desires".to_string(),
            icon: "📜".to_string(),
            description: "work it under the standing instruction".to_string(),
            state: crate::work::recipe::default_state(),
            when: Default::default(),
            needs_checkout: false,
            branch: None,
            root: None,
            autorun: false,
            dispatch: None,
            brief: brief.map(str::to_string),
            brief_file: brief_file.map(str::to_string),
            based_in: None,
            opens_with: None,
            hand: None,
            target: None,
            model: None,
        }
    }

    /// The vocabulary a brief and its path are rendered from, with `{reply}`
    /// in it as a dispatch puts it there.
    fn values(root: &Path) -> BTreeMap<&'static str, String> {
        let item = item(json!({}));
        let checkout = checkout();
        let subject = Subject {
            item: &item,
            checkout: &checkout,
            root,
            organization: None,
        };
        let mut values = subject.placeholders();
        values.insert("reply", "/w/panta/answer.md".to_string());
        values
    }

    /// The published test vectors (FIPS 180-4 and RFC 6234): the hash goes
    /// into a ticket for a reader to check with `sha256sum`, so what it has to
    /// agree with is the standard and not this implementation
    /// (§FS-005-dispatch.34.2).
    #[test]
    fn the_hash_a_ticket_records_is_the_one_sha256sum_prints() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // Longer than one block and longer than a length that fits beside the
        // padding, which is where a hand-written round loop goes wrong.
        assert_eq!(
            sha256(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        // A file's own trailing newline changes the answer, which is the whole
        // reason the hash is of the bytes as read.
        assert_ne!(sha256(b"abc"), sha256(b"abc\n"));
    }

    /// The path is a template over the vocabulary a work root is rendered
    /// from, and the file's text is the brief (§FS-005-dispatch.34).
    #[test]
    fn the_path_renders_and_the_files_words_are_the_brief() {
        let tmp = tempfile::tempdir().expect("a scratch root");
        std::fs::create_dir_all(tmp.path().join("widget")).expect("the project's directory");
        std::fs::write(
            tmp.path().join("widget/DESIRES.md"),
            "Read the spec before the code.\n",
        )
        .expect("the instruction");
        let recipe = keeping(None, Some("{root}/{project}/DESIRES.md"));
        let asked = brief(&recipe, &values(tmp.path())).expect("the file is there");
        assert_eq!(asked.text, "Read the spec before the code.");
        assert_eq!(
            asked.instruction.expect("it read a file").path,
            tmp.path().join("widget/DESIRES.md")
        );
    }

    /// A name the vocabulary cannot answer is refused by name, because what it
    /// would otherwise write is a path with a segment missing
    /// (§FS-005-dispatch.34, §FS-005-dispatch.6.1). `{reply}` is one of them
    /// wherever it is written: it is where ephor puts an answer, not a fact
    /// about the matter — and the prose beside it carries the gap as prose
    /// always has.
    #[test]
    fn a_name_a_path_cannot_answer_is_refused_by_name() {
        let values = values(Path::new("/w"));
        for (template, named) in [
            ("{root}/{nope}/DESIRES.md", "{nope}"),
            ("{reply}.md", "{reply}"),
        ] {
            let why = brief(&keeping(None, Some(template)), &values)
                .expect_err("a path with no answer cannot be written");
            assert!(why.contains(named), "{why}");
            assert!(why.contains("desires"), "{why}");
        }
        // The same name in `brief` is prose, and prose keeps it as written.
        let asked = brief(&keeping(Some("answer in {reply}"), None), &values).expect("prose");
        assert_eq!(asked.text, "answer in /w/panta/answer.md");
    }

    /// `~` and `$VAR` are expanded, and a relative path resolves against the
    /// directory holding the configuration file that named it — never the
    /// working directory, which is wherever the unit that called ephor
    /// happened to stand (§FS-005-dispatch.34).
    #[test]
    fn a_relative_path_is_relative_to_the_configuration_that_named_it() {
        let tmp = tempfile::tempdir().expect("a scratch root");
        let config = tmp.path().join("ephor/status.json");
        std::fs::create_dir_all(config.parent().expect("its directory")).expect("the config dir");
        std::fs::write(config.parent().unwrap().join("DESIRES.md"), "Standing.\n")
            .expect("the instruction beside it");

        let mut recipe = keeping(None, Some("DESIRES.md"));
        recipe.written_in(&config);
        let asked = brief(&recipe, &values(Path::new("/w"))).expect("beside the configuration");
        assert_eq!(asked.text, "Standing.");
        assert_eq!(
            asked.instruction.expect("it read a file").path,
            config.parent().unwrap().join("DESIRES.md")
        );

        // An absolute path is untouched by that, and `$VAR` is expanded first
        // so what is resolved is a path rather than a name.
        // SAFETY: single-threaded test, and the variable is this test's own.
        unsafe { std::env::set_var("EPHOR_TEST_DESIRES", tmp.path().to_string_lossy().as_ref()) };
        std::fs::write(tmp.path().join("OTHER.md"), "Elsewhere.\n").expect("the other one");
        let mut recipe = keeping(None, Some("$EPHOR_TEST_DESIRES/OTHER.md"));
        recipe.written_in(&config);
        let asked = brief(&recipe, &values(Path::new("/w"))).expect("an absolute path");
        assert_eq!(asked.text, "Elsewhere.");
    }

    /// Nothing inside the file is substituted and its headings arrive as
    /// emphasis: a version-controlled document is not a template, and a
    /// heading inside a plan is a node the runtime reads as a task
    /// (§FS-005-dispatch.34, §FS-005-dispatch.3).
    #[test]
    fn the_file_is_a_document_and_not_a_template() {
        let tmp = tempfile::tempdir().expect("a scratch root");
        std::fs::write(
            tmp.path().join("DESIRES.md"),
            "# How we work\n\nAn example naming `{title}` stays as written.\n\n\
             ```text\n# not a heading\n```\n",
        )
        .expect("the instruction");
        let recipe = keeping(Some("Work {title}."), Some("{root}/DESIRES.md"));
        let asked = brief(&recipe, &values(tmp.path())).expect("the file is there");
        assert!(asked.text.contains("**How we work**"), "{}", asked.text);
        assert!(!asked.text.contains("\n# How we work"), "{}", asked.text);
        assert!(
            asked.text.contains("naming `{title}` stays as written"),
            "{}",
            asked.text
        );
        // Fenced content is what its author wrote, heading or not.
        assert!(asked.text.contains("\n# not a heading\n"), "{}", asked.text);
        // And the rendered `brief` after the file's words, with the matter's
        // own title in it (§FS-005-dispatch.34.1).
        assert!(asked.text.ends_with("Work Retry window."), "{}", asked.text);
        assert!(
            asked.text.find("**How we work**") < asked.text.find("Work Retry window."),
            "{}",
            asked.text
        );
    }

    /// A rendered path with nothing readable behind it refuses, naming the
    /// path — and an empty file is the same hole arriving later, because the
    /// ticket would ask for nothing (§FS-005-dispatch.34).
    #[test]
    fn a_path_with_nothing_behind_it_refuses_naming_it() {
        let tmp = tempfile::tempdir().expect("a scratch root");
        let missing = tmp.path().join("DESIRES.md");
        let why = brief(
            &keeping(Some("Work it."), Some("{root}/DESIRES.md")),
            &values(tmp.path()),
        )
        .expect_err("there is no file there");
        assert!(why.contains(&missing.display().to_string()), "{why}");

        std::fs::write(&missing, "   \n\n").expect("an empty instruction");
        let why = brief(
            &keeping(None, Some("{root}/DESIRES.md")),
            &values(tmp.path()),
        )
        .expect_err("a file with no words in it asks for nothing");
        assert!(why.contains(&missing.display().to_string()), "{why}");
        assert!(why.contains("empty"), "{why}");
    }

    /// A recipe with no file named is the brief it always was, and records
    /// nothing about an instruction it never read.
    #[test]
    fn a_recipe_with_no_file_is_the_brief_it_always_was() {
        let asked = brief(
            &keeping(Some("Work {title}."), None),
            &values(Path::new("/w")),
        )
        .expect("prose needs nothing on disk");
        assert_eq!(asked.text, "Work Retry window.");
        assert!(asked.instruction.is_none());
    }

    #[test]
    fn a_brief_is_rendered_with_the_items_own_words() {
        let item = item(json!({}));
        let checkout = checkout();
        let subject = Subject {
            item: &item,
            checkout: &checkout,
            root: Path::new("/w"),
            organization: None,
        };
        let values = subject.placeholders();
        assert_eq!(
            render("fix {title} on {branch} ({repo}#{number})", &values),
            "fix Retry window on you/ABC-42-retry (acme/widget#42)"
        );
        // An unknown name stays visible instead of silently emptying.
        assert_eq!(render("{nope} {title}", &values), "{nope} Retry window");
        assert_eq!(render("unterminated {tit", &values), "unterminated {tit");
    }
}
