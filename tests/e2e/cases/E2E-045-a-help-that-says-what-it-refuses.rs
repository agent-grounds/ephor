//! A person reads a verb's help to find out how to steer it, and the help says
//! which of the global flags the verb will refuse
//! (§FS-011-command-line.9.1, §FS-011-command-line.10.1).
//!
//! The reporter of this wanted a laid plan somewhere other than where `work
//! lay` had put it, saw `--workspace` in `ephor work lay --help` described as
//! the "project id or derived workspace id to operate on", and spent an
//! attempt on it: `work lay` refuses every scope selector, and `--act` too.
//! The flags stay global — the refusal and its exit code are the contract — so
//! what changes is that the help says what the refusal would have said.
//!
//! What each verb refuses is not written out here. It is read from ephor's own
//! classification, the one the refusal reads, by parsing a command line for
//! every page of help the binary has; the page itself comes from the binary,
//! run the way a person runs it. A verb moved from one side of the rule to the
//! other moves its expectation with it, and a verb added without being
//! classified does not compile.

#[path = "../support.rs"]
mod support;

use clap::{CommandFactory, Parser};
use ephor::cli::Cli;
use ephor::scope::{self, Honours, Scope, Sweeps};
use support::*;

/// One page of help: the subcommand path that prints it, and a command line
/// that parses to the verb the page is about.
struct Page {
    path: Vec<String>,
    argv: Vec<String>,
}

/// Every page of help below the root, with a command line for each that clap
/// accepts — required arguments filled with a value they take.
fn pages() -> Vec<Page> {
    fn walk(command: &clap::Command, path: &[String], into: &mut Vec<Page>) {
        for sub in command.get_subcommands() {
            if sub.get_name() == "help" {
                continue;
            }
            let mut path = path.to_vec();
            path.push(sub.get_name().to_string());
            let mut argv = vec!["ephor".to_string()];
            argv.extend(path.iter().cloned());
            for arg in sub.get_arguments() {
                if !arg.is_required_set() || arg.is_global_set() {
                    continue;
                }
                let value = arg
                    .get_possible_values()
                    .first()
                    .map(|value| value.get_name().to_string())
                    // A number reads as a name too, and some of these are counts.
                    .unwrap_or_else(|| "1".to_string());
                if arg.is_positional() {
                    argv.push(value);
                } else {
                    let long = arg.get_long().expect("a required flag has a long name");
                    argv.push(format!("--{long}"));
                    argv.push(value);
                }
            }
            into.push(Page {
                path: path.clone(),
                argv,
            });
            walk(sub, &path, into);
        }
    }
    let mut pages = Vec::new();
    walk(&Cli::command(), &[], &mut pages);
    pages
}

/// The forms that share a help with a form that honours the selectors
/// (§FS-011-command-line.9.1): the page, and the command line of the form.
fn shared_forms() -> Vec<Page> {
    let form = |path: &[&str], argv: &[&str]| Page {
        path: path.iter().map(|word| word.to_string()).collect(),
        argv: argv.iter().map(|word| word.to_string()).collect(),
    };
    vec![
        form(&["validate"], &["ephor", "validate", "--manifest", "x"]),
        form(&["validate"], &["ephor", "validate", "--schema-only"]),
        form(
            &["ensure-agents"],
            &["ephor", "ensure-agents", "--type", "x"],
        ),
        form(&["feed"], &["ephor", "feed", "--unattributed"]),
    ]
}

/// The help the binary prints for one page, with its line breaks folded, so a
/// sentence wrapped at a terminal's width is still the sentence.
fn help_of(world: &World, path: &[String]) -> String {
    let output = world
        .ephor_raw()
        .args(path)
        .arg("--help")
        .output()
        .expect("ran");
    assert_eq!(output.status.code(), Some(0), "{path:?} --help");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a page parses to: the verb as its refusal names it, and the two
/// classifications that decide what the refusal says.
fn classified(page: &Page) -> (String, Honours, Sweeps) {
    let cli = Cli::try_parse_from(&page.argv)
        .unwrap_or_else(|error| panic!("{:?} does not parse: {error}", page.argv));
    let (verb, honours) = scope::honoured(&cli.command);
    let sweeps = scope::sweeps(&cli.command, &Scope::default());
    (verb, honours, sweeps)
}

/// The reason a selector refusal gives, as the help must say it.
fn selector_note(verb: &str, honours: Honours) -> Option<String> {
    match honours {
        Honours::Nothing => Some(format!(
            "{verb} takes no scope selector: it is about what it is given, not about a set of \
             projects."
        )),
        Honours::NothingOverTheSite => Some(format!(
            "{verb} takes no scope selector: it reads every project the site is configured \
             with and answers for the site itself, not for a group the registry names."
        )),
        Honours::Watched | Honours::Registry => None,
    }
}

/// What the help of a verb that refuses `--act` must say, in its refusal's
/// terms (§FS-011-command-line.10.1).
fn act_notes(verb: &str, sweeps: Sweeps) -> Vec<String> {
    match sweeps {
        Sweeps::Nothing => vec![
            format!("{verb} does not take --act."),
            format!("{verb} sweeps no set of projects"),
        ],
        Sweeps::Ungated => vec![
            format!("{verb} does not take --act."),
            format!("{verb} is not held to that gate"),
        ],
        Sweeps::NothingWithoutASelector => {
            vec!["with no selector it is about the one checkout it was given".to_string()]
        }
        Sweeps::Gated => Vec::new(),
    }
}

/// Every page whose verb refuses the selectors says so, in the words of its
/// refusal; every page whose verb honours them says nothing of the kind
/// (§FS-011-command-line.9.1).
#[test]
fn a_verb_that_refuses_the_selectors_says_so_in_its_help() {
    let world = World::new();
    let mut refusing = std::collections::BTreeSet::new();
    let mut missing = Vec::new();
    for page in pages() {
        let (verb, honours, _) = classified(&page);
        let help = help_of(&world, &page.path);
        assert!(
            help.contains("--workspace"),
            "{:?}: the selectors stay global and listed: {help}",
            page.path
        );
        match selector_note(&verb, honours) {
            Some(note) => {
                refusing.insert(verb.clone());
                if !help.contains(&note) {
                    missing.push(format!("{} --help lacks: {note}", page.path.join(" ")));
                }
            }
            None => assert!(
                !help.contains(&format!("{verb} takes no scope selector")),
                "{verb} honours the selectors, and its help says it does not: {help}"
            ),
        }
    }
    assert!(missing.is_empty(), "\n{}", missing.join("\n"));
    // The walk found the verbs the report named, so a page cannot pass by not
    // being visited.
    for verb in [
        "check",
        "schema",
        "failures",
        "restart",
        "checkout",
        "job",
        "actions",
        "operations",
        "burn",
        "thread",
        "react",
        "tick",
        "reply",
        "capabilities",
        "doctor",
        "work offers",
        "work ask",
        "work cancel",
        "work workflows",
        "work lay",
        "work forget",
        "work states",
    ] {
        assert!(refusing.contains(verb), "{verb} was not found refusing");
    }
}

/// The four forms that share a help with a form that honours the selectors
/// are named in it, as their refusal names them (§FS-011-command-line.9.1).
#[test]
fn a_shared_help_names_the_form_that_refuses() {
    let world = World::new();
    let mut missing = Vec::new();
    for page in shared_forms() {
        let (verb, honours, _) = classified(&page);
        let note = selector_note(&verb, honours)
            .unwrap_or_else(|| panic!("{:?} is classified as honouring", page.argv));
        let help = help_of(&world, &page.path);
        if !help.contains(&note) {
            missing.push(format!("{} --help lacks: {note}", page.path.join(" ")));
        }
        let (whole, _, _) = classified(&Page {
            path: page.path.clone(),
            argv: page.argv[..2].to_vec(),
        });
        assert!(
            !help.contains(&format!("{whole} takes no scope selector")),
            "{whole} honours the selectors, and its help says it does not: {help}"
        );
    }
    assert!(missing.is_empty(), "\n{}", missing.join("\n"));
}

/// Every page whose verb refuses `--act` says so in its refusal's terms, and
/// the four the gate fires on say nothing of the kind
/// (§FS-011-command-line.10.1).
#[test]
fn a_verb_that_refuses_act_says_so_in_its_help() {
    let world = World::new();
    let mut gated = std::collections::BTreeSet::new();
    let mut missing = Vec::new();
    // A shared form is named only where its answer differs from its verb's:
    // `ensure-agents --type` sweeps nothing where `ensure-agents` is ungated.
    let forms = shared_forms().into_iter().filter(|page| {
        let whole = Page {
            path: page.path.clone(),
            argv: page.argv[..2].to_vec(),
        };
        classified(page).2 != classified(&whole).2
    });
    let forms: Vec<Page> = forms.collect();
    assert_eq!(forms.len(), 1, "only ensure-agents --type answers apart");
    for page in pages().into_iter().chain(forms) {
        let (verb, _, sweeps) = classified(&page);
        let help = help_of(&world, &page.path);
        if sweeps == Sweeps::Gated {
            gated.insert(verb.clone());
            assert!(
                !help.contains(&format!("{verb} does not take --act")),
                "the gate fires on {verb}, and its help says it refuses --act: {help}"
            );
            continue;
        }
        for note in act_notes(&verb, sweeps) {
            if !help.contains(&note) {
                missing.push(format!("{} --help lacks: {note}", page.path.join(" ")));
            }
        }
    }
    assert!(missing.is_empty(), "\n{}", missing.join("\n"));
    for verb in ["work dispatch", "work sync", "work run", "clean"] {
        assert!(gated.contains(verb), "{verb} was not found gated");
    }
}
