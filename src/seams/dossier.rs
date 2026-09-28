//! The `EPHOR_*` vocabulary: what a summoned command is told about the matter
//! it is being run on (§FS-005-dispatch.8).
//!
//! One vocabulary, whether the thing reading it is a shell command in a menu, a
//! check verb during a refresh, or a program in a state machine — and one place
//! that writes it, so a name cannot mean two things depending on who spawned
//! the command (§AR-002-summons.2). Identifiers only: the prose belongs in the
//! dossier a reader is looking at, and this is what a script can use.

use std::path::Path;

use crate::branches::BranchInfo;
use crate::feed::model::Item;
use crate::forest::Forest;

/// What every summons is told, whichever matter it is about.
fn place(project: &str, root: &Path, workspace: &Path) -> Vec<(String, String)> {
    vec![
        ("EPHOR_PROJECT".to_string(), project.to_string()),
        // Spelled for the shell that will parse them, not for the platform
        // (§FS-006-project-interface.3).
        ("EPHOR_ROOT".to_string(), crate::paths::for_shell(root)),
        (
            "EPHOR_WORKSPACE".to_string(),
            crate::paths::for_shell(workspace),
        ),
    ]
}

/// The repositories the command may fold over, one per line, in forest order
/// (§AR-004-forest.1). A script that pushes or checks per repository reads
/// this instead of probing the directory itself, so it and ephor cannot
/// disagree about what the workspace holds.
pub const REPOS: &str = "EPHOR_REPOS";

/// The check verbs a project fills, one per line, in the order a verify step
/// should run them (§FS-006-project-interface.5). Which verbs run and in what
/// order is policy above the interface, so it is handed over rather than
/// decided inside ephor.
pub const CHECKS: &str = "EPHOR_CHECKS";

/// What this matter's own source said about it, one variable per key
/// (§FS-006-project-interface.3): the key upper-cased in ASCII with `-`
/// written `_`, behind this prefix.
const META_PREFIX: &str = "EPHOR_META_";

/// Which of those names are this matter's — the carried keys, one per line
/// (§FS-006-project-interface.3).
///
/// Fixed, and always set. A summons does not start from a cleared environment
/// (§AR-002-summons.1), so every name a matter can answer is also set empty on
/// a summons about something that is not one; an open namespace cannot be
/// emptied that way, because ephor cannot blank an `EPHOR_META_CONTEXT` it has
/// never heard of. This is the one fixed name that makes it enumerable, and
/// the contract is that an `EPHOR_META_*` variable is this matter's only if
/// its key is listed here.
pub const META_KEYS: &str = "EPHOR_META_KEYS";

/// The variable one `meta` key is read under (§FS-006-project-interface.3).
/// One direction only: the variable is derived from the key, never the key
/// from the variable.
pub(crate) fn meta_variable(key: &str) -> String {
    format!(
        "{META_PREFIX}{}",
        key.to_ascii_uppercase().replace('-', "_")
    )
}

/// What a matter's source said about it, as the summons carries it
/// (§FS-006-project-interface.3): one variable per key, and the enumeration.
///
/// Two keys that fold to one variable name set no variable at all — a variable
/// that could be either key's is a fact about neither — while both keys stay
/// in `meta`, where a selector and a template name them unambiguously.
fn said_about(item: &Item) -> Vec<(String, String)> {
    let said = item.meta().unwrap_or_default();
    let mut pairs: Vec<(String, String)> = said
        .iter()
        .map(|(key, value)| (meta_variable(key), value.clone()))
        .filter(|(name, _)| {
            said.keys()
                .filter(|key| &meta_variable(key) == name)
                .count()
                == 1
        })
        .collect();
    pairs.push((
        META_KEYS.to_string(),
        said.keys().cloned().collect::<Vec<_>>().join("\n"),
    ));
    pairs
}

/// Add the bound check verbs to a dossier, where the project fills any.
pub fn with_checks(mut pairs: Vec<(String, String)>, checks: &[String]) -> Vec<(String, String)> {
    if !checks.is_empty() {
        pairs.push((CHECKS.to_string(), checks.join("\n")));
    }
    pairs
}

fn with_forest(mut pairs: Vec<(String, String)>, forest: Option<&Forest>) -> Vec<(String, String)> {
    if let Some(forest) = forest.filter(|forest| !forest.is_empty()) {
        pairs.push((REPOS.to_string(), forest.names().join("\n")));
    }
    pairs
}

/// A summons about a project rather than one of its matters — a status
/// command, a check verb asked of the whole forest.
pub fn of_project(
    project: &str,
    root: &Path,
    workspace: &Path,
    forest: Option<&Forest>,
) -> Vec<(String, String)> {
    with_forest(place(project, root, workspace), forest)
}

/// A summons about a branch rather than a matter — what a branch row's menu
/// tells the command it runs (§FS-004-quick-actions.6).
///
/// Every name [`of_item`] sets is set here too, empty where a branch has no
/// answer for it. A summons does not start from a cleared environment
/// (§AR-002-summons.1), so a name left unset is not absent — it is inherited
/// from whatever launched ephor, and a command on a branch row would read some
/// other matter's title, url or number as if they were this row's. The empties
/// are the point; `assert_eq!` on the two vocabularies keeps them that way.
pub fn of_branch(
    project: &str,
    root: &Path,
    workspace: &Path,
    branch: Option<&BranchInfo>,
    forest: Option<&Forest>,
) -> Vec<(String, String)> {
    let mut pairs = place(project, root, workspace);
    pairs.extend([
        ("EPHOR_ITEM_ID".to_string(), String::new()),
        ("EPHOR_SOURCE".to_string(), String::new()),
        ("EPHOR_KIND".to_string(), String::new()),
        ("EPHOR_TITLE".to_string(), String::new()),
        ("EPHOR_URL".to_string(), String::new()),
        ("EPHOR_STATE".to_string(), String::new()),
        (
            "EPHOR_BRANCH".to_string(),
            branch
                .map(|branch| branch.branch.clone())
                .unwrap_or_default(),
        ),
        (
            "EPHOR_TICKET".to_string(),
            branch
                .and_then(|branch| branch.ticket.clone())
                .unwrap_or_default(),
        ),
        ("EPHOR_REPO".to_string(), String::new()),
        ("EPHOR_NUMBER".to_string(), String::new()),
        ("EPHOR_RAW".to_string(), String::new()),
        // The open namespace's one fixed name, empty like the rest: a branch
        // carries no matter, so it carries no keys, and a command reading an
        // `EPHOR_META_*` it inherited is told so by an enumeration that does
        // not list it (§FS-006-project-interface.3).
        (META_KEYS.to_string(), String::new()),
    ]);
    with_forest(pairs, forest)
}

/// A summons about one matter. `branch` is the registry branch the item was
/// matched to (org → project → branch, the same grouping the tree shows);
/// `workspace` is the resolved checkout the command runs in.
pub fn of_item(
    item: &Item,
    root: &Path,
    workspace: &Path,
    branch: Option<&BranchInfo>,
    forest: Option<&Forest>,
) -> Vec<(String, String)> {
    let string = |value: &Option<String>| value.clone().unwrap_or_default();
    // The provider-recorded branch is ground truth; the matched registry
    // branch fills in for providers that don't record one.
    let branch_name = item
        .raw
        .get("branch")
        .and_then(serde_json::Value::as_str)
        .map(String::from)
        .or_else(|| branch.map(|branch| branch.branch.clone()))
        .unwrap_or_default();
    let mut pairs = place(&item.project, root, workspace);
    pairs.extend([
        ("EPHOR_ITEM_ID".to_string(), item.id.clone()),
        ("EPHOR_SOURCE".to_string(), item.source.clone()),
        ("EPHOR_KIND".to_string(), item.kind.label().to_string()),
        ("EPHOR_TITLE".to_string(), item.title.clone()),
        ("EPHOR_URL".to_string(), string(&item.url)),
        ("EPHOR_STATE".to_string(), string(&item.state)),
        ("EPHOR_BRANCH".to_string(), branch_name),
        (
            "EPHOR_TICKET".to_string(),
            branch
                .and_then(|branch| branch.ticket.clone())
                .unwrap_or_default(),
        ),
        ("EPHOR_REPO".to_string(), item.repo().unwrap_or_default()),
        (
            "EPHOR_NUMBER".to_string(),
            item.number().unwrap_or_default(),
        ),
        // What the source knew beyond the model, passed through whole
        // (§AR-006-matters.1).
        ("EPHOR_RAW".to_string(), item.raw.to_string()),
    ]);
    // And what it said about *this matter*, by name, so a program does not
    // re-parse the passthrough to reach one word (§FS-006-project-interface.3).
    pairs.extend(said_about(item));
    with_forest(pairs, forest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feed::model::ItemKind;
    use serde_json::json;

    /// A branch row answers every name a matter does, so nothing an item would
    /// have named is left to be inherited from the process that launched ephor.
    /// This is the whole guard: add a name to [`of_item`] and this fails until
    /// [`of_branch`] answers it too.
    #[test]
    fn a_branch_answers_every_name_a_matter_does() {
        let root = Path::new("/w");
        let names = |pairs: Vec<(String, String)>| {
            pairs
                .into_iter()
                .map(|(name, _)| name)
                .collect::<std::collections::BTreeSet<_>>()
        };
        let matter = names(of_item(
            &item(ItemKind::Pr, "test:1", json!({})),
            root,
            root,
            None,
            None,
        ));
        let branch = names(of_branch("widget", root, root, None, None));
        assert_eq!(matter, branch);
    }

    fn item(kind: ItemKind, id: &str, raw: serde_json::Value) -> Item {
        Item {
            id: id.to_string(),
            project: "widget".to_string(),
            source: "test".to_string(),
            kind,
            role: None,
            title: "title".to_string(),
            url: None,
            state: None,
            needs_response: false,
            updated_at: chrono::Utc::now(),
            raw,
        }
    }

    fn value(pairs: &[(String, String)], key: &str) -> String {
        pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| {
                let names: Vec<&str> = pairs.iter().map(|(name, _)| name.as_str()).collect();
                panic!("no {key} in the summons; it sets {names:?}")
            })
    }

    /// What the matter's own source said about it reaches the command by name
    /// (§FS-006-project-interface.3): the key upper-cased in ASCII, `-` written
    /// `_`, prefixed `EPHOR_META_`.
    #[test]
    fn what_the_source_said_about_the_matter_reaches_the_command_by_name() {
        let root = Path::new("/w");
        let pairs = of_item(
            &item(
                ItemKind::Task,
                "rhei:slices.1",
                json!({ "meta": { "context": "acme-labs", "roll-out": "second", "tier": 1 } }),
            ),
            root,
            root,
            None,
            None,
        );
        assert_eq!(value(&pairs, "EPHOR_META_CONTEXT"), "acme-labs");
        assert_eq!(value(&pairs, "EPHOR_META_ROLL_OUT"), "second");
        // A number the store did not quote arrives in the spelling the selector
        // and the template both use.
        assert_eq!(value(&pairs, "EPHOR_META_TIER"), "1");
        // And the enumeration, so a command can tell this matter's variable
        // from one it inherited from whatever launched ephor.
        let keys = value(&pairs, "EPHOR_META_KEYS");
        let mut listed: Vec<&str> = keys.lines().collect();
        listed.sort_unstable();
        assert_eq!(listed, vec!["context", "roll-out", "tier"]);
    }

    /// Two keys that fold to one variable name set no variable at all, because a
    /// variable that could be either key's is a fact about neither. Both keys
    /// stay in `meta`, where a selector and a template name them unambiguously
    /// (§FS-006-project-interface.3).
    #[test]
    fn two_keys_that_fold_to_one_name_set_neither() {
        let root = Path::new("/w");
        let pairs = of_item(
            &item(
                ItemKind::Task,
                "rhei:slices.1",
                json!({ "meta": { "roll-out": "second", "roll_out": "third", "tier": 1 } }),
            ),
            root,
            root,
            None,
            None,
        );
        assert!(
            pairs.iter().all(|(name, _)| name != "EPHOR_META_ROLL_OUT"),
            "a colliding pair set a variable that is neither key's: {pairs:#?}"
        );
        // The rest of the map is unaffected, and the enumeration still names
        // both keys: they are readable where they are unambiguous.
        assert_eq!(value(&pairs, "EPHOR_META_TIER"), "1");
        let keys = value(&pairs, "EPHOR_META_KEYS");
        let mut listed: Vec<&str> = keys.lines().collect();
        listed.sort_unstable();
        assert_eq!(listed, vec!["roll-out", "roll_out", "tier"]);
    }

    /// A matter whose source said nothing still answers the fixed name, empty —
    /// and so does a summons about a branch. That is what keeps an open
    /// namespace enumerable, and it is why the two-vocabulary assertion above
    /// can go on holding (§FS-006-project-interface.3).
    #[test]
    fn the_enumeration_is_always_answered_even_where_there_is_nothing_to_enumerate() {
        let root = Path::new("/w");
        let silent = of_item(
            &item(ItemKind::Pr, "test:1", json!({})),
            root,
            root,
            None,
            None,
        );
        assert_eq!(value(&silent, "EPHOR_META_KEYS"), "");
        let branch = of_branch("widget", root, root, None, None);
        assert_eq!(value(&branch, "EPHOR_META_KEYS"), "");
    }

    #[test]
    fn a_project_summons_is_told_where_it_is() {
        let pairs = of_project(
            "widget",
            Path::new("/tmp/widget"),
            Path::new("/tmp/widget"),
            None,
        );
        assert_eq!(value(&pairs, "EPHOR_PROJECT"), "widget");
        assert_eq!(value(&pairs, "EPHOR_ROOT"), "/tmp/widget");
        assert_eq!(value(&pairs, "EPHOR_WORKSPACE"), "/tmp/widget");
    }

    #[test]
    fn the_forest_reaches_the_command_so_it_folds_over_what_ephor_folds_over() {
        let tmp = tempfile::tempdir().unwrap();
        for name in ["ce", "ee"] {
            let path = tmp.path().join(name);
            std::fs::create_dir_all(&path).unwrap();
            assert!(std::process::Command::new("git")
                .args(["init", "--quiet"])
                .current_dir(&path)
                .status()
                .unwrap()
                .success());
        }
        let forest = Forest::resolve(
            tmp.path(),
            None,
            &[
                crate::forest::Declaration::at("ce"),
                crate::forest::Declaration::at("ee"),
            ],
        );
        let pairs = of_project("widget", tmp.path(), tmp.path(), Some(&forest));
        assert_eq!(value(&pairs, REPOS), "ce\nee");

        // A forest of nothing says nothing rather than saying "none".
        let empty = Forest::resolve(tmp.path(), None, &[crate::forest::Declaration::at("gone")]);
        let pairs = of_project("widget", tmp.path(), tmp.path(), Some(&empty));
        assert!(pairs.iter().all(|(name, _)| name != REPOS));
    }

    #[test]
    fn the_check_verbs_are_handed_over_so_composition_stays_configuration() {
        let pairs = with_checks(
            of_project("widget", Path::new("/w"), Path::new("/w"), None),
            &["./check.sh".to_string(), "mx gate".to_string()],
        );
        assert_eq!(value(&pairs, CHECKS), "./check.sh\nmx gate");

        // A project that fills none says nothing, rather than saying "none".
        let bare = with_checks(
            of_project("widget", Path::new("/w"), Path::new("/w"), None),
            &[],
        );
        assert!(bare.iter().all(|(name, _)| name != CHECKS));
    }

    #[test]
    fn env_extracts_github_number_and_repo() {
        let pr = item(ItemKind::Pr, "github-prs:acme/widget#42", json!({}));
        let pairs = of_item(
            &pr,
            Path::new("/tmp/widget"),
            Path::new("/tmp/widget/master"),
            None,
            None,
        );
        assert_eq!(value(&pairs, "EPHOR_NUMBER"), "42");
        assert_eq!(value(&pairs, "EPHOR_REPO"), "acme/widget");
        assert_eq!(value(&pairs, "EPHOR_ROOT"), "/tmp/widget");
        assert_eq!(value(&pairs, "EPHOR_WORKSPACE"), "/tmp/widget/master");
        assert_eq!(value(&pairs, "EPHOR_KIND"), "pr");
    }

    #[test]
    fn env_extracts_bitbucket_number_and_repo() {
        let pr = item(
            ItemKind::Pr,
            "bitbucket-prs:plugins/123",
            json!({ "repo": "plugins", "branch": "you/ABC-7-fix" }),
        );
        let pairs = of_item(
            &pr,
            Path::new("/tmp/widget"),
            Path::new("/tmp/widget"),
            None,
            None,
        );
        assert_eq!(value(&pairs, "EPHOR_NUMBER"), "123");
        assert_eq!(value(&pairs, "EPHOR_REPO"), "plugins");
        assert_eq!(value(&pairs, "EPHOR_BRANCH"), "you/ABC-7-fix");
    }

    #[test]
    fn env_fills_branch_and_ticket_from_matched_registry_branch() {
        let branch = BranchInfo {
            branch: "you/ABC-42-retry-window".to_string(),
            ticket: Some("ABC-42".to_string()),
            active: true,
            is_release: false,
            declared: true,
        };
        // A github item records no branch: the registry match fills in.
        let pr = item(ItemKind::Pr, "github-prs:acme/widget#42", json!({}));
        let pairs = of_item(&pr, Path::new("/r"), Path::new("/r/b"), Some(&branch), None);
        assert_eq!(value(&pairs, "EPHOR_BRANCH"), "you/ABC-42-retry-window");
        assert_eq!(value(&pairs, "EPHOR_TICKET"), "ABC-42");

        // A provider-recorded branch wins over the registry match.
        let pr = item(
            ItemKind::Pr,
            "bitbucket-prs:app/123",
            json!({ "branch": "other" }),
        );
        let pairs = of_item(&pr, Path::new("/r"), Path::new("/r/b"), Some(&branch), None);
        assert_eq!(value(&pairs, "EPHOR_BRANCH"), "other");
    }
}
