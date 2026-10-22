//! The suite held to its own law from the integration tree: every name `src/`
//! reads as a flag's environment spelling is one the `ephor` the suite spawns
//! is built without (§REQ-003-test-isolation.4.1).
//!
//! The re-run cases (`holds_with_the_callers_flag_spellings`) show what an
//! inherited spelling does to a case today. They cannot show the next
//! spelling: a name `given` starts reading tomorrow is in no caller's
//! environment those cases set. This reads the names off the tree instead.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use common::ephor_command;

/// §REQ-003-test-isolation.4.1 and §REQ-003-test-isolation.4.4: the removal is
/// on the command `ephor_command()` builds, for every spelling `src/` reads,
/// whether this process happens to hold it or not — CI runs this from an
/// environment that holds none of them.
#[test]
#[ignore = "red until #237: ephor_command() removes none of the flag spellings src/ reads; implement removes this"]
fn every_flag_spelling_src_reads_is_one_ephor_command_removes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = spellings_read_under(root, &root.join("src"));
    // A scan that found nothing would hold nothing: one name of each reader.
    for known in ["REPORT", "BRANCH"] {
        assert!(
            read.contains_key(known),
            "the scan of src/ found no read of {known}, so it is not reading what ephor reads: \
             {read:?}"
        );
    }

    let command = ephor_command();
    let removed: BTreeSet<String> = command
        .get_envs()
        .filter(|(_, value)| value.is_none())
        .map(|(key, _)| key.to_string_lossy().into_owned())
        .collect();
    let missing: Vec<String> = read
        .iter()
        .filter(|(name, _)| !removed.contains(*name))
        .map(|(name, sites)| {
            let sites: Vec<&str> = sites.iter().map(String::as_str).collect();
            format!("{name} (read at {})", sites.join(", "))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "ephor reads these as a flag's environment spelling, and the `ephor` the suite spawns \
         is not built without them, so each reaches it from whoever ran the suite \
         (§REQ-003-test-isolation.4.1):\n  {}",
        missing.join("\n  ")
    );
}

/// Every name ephor reads as a flag's environment spelling, each with the
/// places it is read. Three shapes, the ones `src/` reads in:
///
/// - the name a `given::value`, `given::branch` or `given::input` call is
///   handed as a literal;
/// - the name paired with the flag it spells, `(&args.project, "PROJECT")`,
///   where a table of them is handed to `given` one at a time;
/// - the name read around `given`, `std::env::var("UPSTREAM")` on the line
///   that reads `args.upstream`.
///
/// `src/given.rs` itself reads a name it is handed, and is the home of the
/// first shape rather than a site of it.
fn spellings_read_under(root: &Path, src: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let given = src.join("given.rs");
    let mut read: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in rust_sources_under(src) {
        if path == given {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a source is readable");
        let shown = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let mut note = |name: String, at: usize| {
            let line = text[..at].matches('\n').count() + 1;
            read.entry(name)
                .or_default()
                .insert(format!("{shown}:{line}"));
        };
        for call in ["given::value(", "given::branch(", "given::input("] {
            for (at, _) in text.match_indices(call) {
                let arguments = &text[at + call.len()..];
                let arguments = &arguments[..arguments.find(')').unwrap_or(arguments.len())];
                if let Some(name) = literals(arguments).pop() {
                    note(name, at);
                }
            }
        }
        for (at, _) in text.match_indices("(&args.") {
            let rest = &text[at + "(&args.".len()..];
            let field: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let pair = rest[field.len()..].trim_start();
            if let Some(name) = pair
                .strip_prefix(',')
                .map(str::trim_start)
                .and_then(|pair| literals(&pair[..pair.find(')').unwrap_or(pair.len())]).pop())
            {
                if name == field.to_ascii_uppercase() {
                    note(name, at);
                }
            }
        }
        for read_call in ["env::var(", "env::var_os("] {
            for (at, _) in text.match_indices(read_call) {
                let rest = &text[at + read_call.len()..];
                let Some(name) = literals(&rest[..rest.find(')').unwrap_or(rest.len())]).pop()
                else {
                    continue;
                };
                let start = text[..at].rfind('\n').map_or(0, |newline| newline + 1);
                let end = text[at..]
                    .find('\n')
                    .map_or(text.len(), |newline| at + newline);
                let flag = format!("args.{}", name.to_ascii_lowercase());
                if text[start..end].contains(&flag) {
                    note(name, at);
                }
            }
        }
    }
    read
}

/// The upper-case string literals in `text`, in order: the shape of an
/// environment name.
fn literals(text: &str) -> Vec<String> {
    text.split('"')
        .skip(1)
        .step_by(2)
        .filter(|word| {
            word.starts_with(|c: char| c.is_ascii_uppercase())
                && word
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        })
        .map(str::to_string)
        .collect()
}

/// Every `.rs` file under `dir`, so the scan reads the tree rather than a list
/// someone has to keep up to date.
fn rust_sources_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}
