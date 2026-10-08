//! Whose a matter is (§FS-018-private-sources): the sources the site lists as
//! the person's own, the root their work goes in, and whose one ticket is.
//!
//! Every reading that renders a work root with the matter in hand asks
//! [`root`] first — [`super::Dispatcher::site_for`] for every write, and both
//! previews of it — so the private rung is one rung, refused in one sentence
//! wherever it is met (§AR-009-surfaces.1). The sweep asks [`of_ticket`], and
//! reads the answer off the ticket rather than off the ledger
//! (§FS-005-dispatch.8, §FS-005-dispatch.4).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::recipe::WorkConfig;
use super::runtime::plan::{self, Plan};
use super::runtime::watch::PlanRef;

/// The key a refusal names, spelled once.
const ROOT_KEY: &str = "work.private.root";

/// The source as the site lists it in `work.private.sources`, where it lists
/// it (§FS-018-private-sources.1). Provider names, the same names a matter id
/// is keyed by.
pub fn listed<'a>(global: &'a WorkConfig, source: &str) -> Option<&'a str> {
    global
        .private
        .as_ref()?
        .sources
        .iter()
        .find(|listed| listed.as_str() == source)
        .map(String::as_str)
}

/// The work root of a matter `source` reported, where the site lists that
/// source as private: the first rung and the only one, ahead of every entry,
/// recipe, project, organization and site root (§FS-018-private-sources.2).
/// `render` is the subject's own rendering of a template, with `work.root`'s
/// vocabulary and refusals; `project` is the registry root of the project the
/// matter is about. None for a source the site does not list, whose work climbs
/// the ladder it always did.
///
/// Two refusals are this rung's own, and neither falls back to the ladder,
/// because the ladder is the leak: a listed source with no root declared, and
/// a root that renders inside the project's registry root, which holds its
/// checkouts and its work root.
pub fn root(
    global: &WorkConfig,
    source: &str,
    project: &Path,
    render: impl FnOnce(&str) -> Result<PathBuf, String>,
) -> Option<Result<PathBuf, String>> {
    let source = listed(global, source)?;
    let template = global
        .private
        .as_ref()
        .and_then(|private| private.root.as_deref())
        .filter(|template| !template.trim().is_empty());
    let Some(template) = template else {
        return Some(Err(format!(
            "{source} is listed in work.private.sources, and {ROOT_KEY} declares no root for \
             its work — a private matter's work goes in a root of the person's own and \
             nowhere else. Declare one outside every project, e.g.\n  \"root\": \
             \"~/private/{{org}}/{{project}}\""
        )));
    };
    Some(
        render(template)
            .map_err(|why| format!("{ROOT_KEY}: {why}"))
            .and_then(|rendered| outside(&rendered, project).map(|()| rendered)),
    )
}

/// Refuses a private root that lands inside the project's registry root,
/// naming the key, what it rendered and the root that holds it, as written
/// (§FS-018-private-sources.2). Compared through every symlink, so a temporary
/// directory behind one is still inside.
fn outside(rendered: &Path, project: &Path) -> Result<(), String> {
    if !real(rendered).starts_with(real(project)) {
        return Ok(());
    }
    Err(format!(
        "{ROOT_KEY} renders {} for this matter, inside {}, the project's registry root — \
         which holds its checkouts and its work root, so a private root there is the \
         organization's under another name. Move {ROOT_KEY} outside every project's root.",
        rendered.display(),
        project.display()
    ))
}

/// A path with every symlink above it resolved, as far as it exists: the
/// deepest ancestor on disk canonicalized, and the rest joined back on. A root
/// is usually not made yet when it is judged.
fn real(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut rest = Vec::new();
    loop {
        if let Ok(resolved) = fs::canonicalize(existing) {
            return rest
                .iter()
                .rev()
                .fold(resolved, |resolved, part| resolved.join(part));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                existing = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// The listed source a ticket is about, where it is about one: whose the
/// ticket is, read off what it records about its matter
/// (§FS-018-private-sources.3, §FS-005-dispatch.8). So a ticket laid before
/// its source was listed answers too, wherever it was written.
///
/// A ticket ephor wrote records its source in its plan's metadata block. A
/// plan a workflow laid has no ticket of ephor's in it, so it answers with the
/// item file ephor carried beside it (§FS-005-dispatch.19) — `laid` says which.
pub fn of_ticket<'a>(
    global: &'a WorkConfig,
    plan_ref: &PlanRef,
    laid: bool,
    plan: &Plan,
    ticket: &str,
) -> Option<&'a str> {
    if global
        .private
        .as_ref()
        .is_none_or(|private| private.sources.is_empty())
    {
        return None;
    }
    let source = plan
        .source_of(&plan_ref.plan_id, ticket)
        .or_else(|| laid.then(|| carried_source(&plan_ref.path)).flatten())?;
    listed(global, &source)
}

/// The `source` in the item file carried beside a laid plan: the directory
/// the plan was laid into names it in the work root's carried corner.
fn carried_source(plan: &Path) -> Option<String> {
    let output = plan.parent()?;
    let item = output
        .parent()?
        .join(plan::CARRIED)
        .join(output.file_name()?)
        .join("item.json");
    let said: Value = serde_json::from_str(&fs::read_to_string(item).ok()?).ok()?;
    said.get("source")?
        .as_str()
        .filter(|source| !source.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::recipe::PrivateSources;

    fn site(root: Option<&str>) -> WorkConfig {
        WorkConfig {
            private: Some(PrivateSources {
                sources: vec!["chatgw".to_string()],
                root: root.map(str::to_string),
            }),
            ..WorkConfig::default()
        }
    }

    fn rendered(template: &str) -> Result<PathBuf, String> {
        Ok(PathBuf::from(template))
    }

    /// An unlisted source has no private rung; a listed one with no root is
    /// refused by name, never sent down the ladder (§FS-018-private-sources.2).
    #[test]
    fn issue_191_the_private_rung_answers_listed_sources_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("rhei");
        let mine = tmp.path().join("me/private/rhei");
        let site = site(Some(&mine.to_string_lossy()));
        assert!(root(&site, "github", &project, rendered).is_none());
        assert_eq!(
            root(&site, "chatgw", &project, rendered),
            Some(Ok(mine.clone()))
        );
        assert!(root(&WorkConfig::default(), "chatgw", &project, rendered).is_none());

        let why = root(&self::site(None), "chatgw", &project, rendered)
            .unwrap()
            .unwrap_err();
        assert!(
            why.contains("work.private.root") && why.contains("chatgw"),
            "{why}"
        );
    }

    /// A private root inside the project's registry root is refused, naming
    /// the key, the rendered path and the root, as written — and judged
    /// through a symlink above both (§FS-018-private-sources.2).
    #[test]
    fn issue_191_a_private_root_inside_the_project_is_refused_through_a_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let real_dir = tmp.path().join("real");
        fs::create_dir_all(real_dir.join("rhei")).unwrap();
        let link = tmp.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_dir, &link).unwrap();
        #[cfg(not(unix))]
        fs::create_dir_all(link.join("rhei")).unwrap();
        let project = link.join("rhei");
        let inside = real_dir.join("rhei/private/answers");
        let site = site(Some(&inside.to_string_lossy()));
        let why = root(&site, "chatgw", &project, rendered)
            .unwrap()
            .unwrap_err();
        for word in [
            "work.private.root",
            &*inside.to_string_lossy(),
            &*project.to_string_lossy(),
        ] {
            assert!(why.contains(word), "names {word}: {why}");
        }
        let beside = link.join("me/rhei");
        let site = self::site(Some(&beside.to_string_lossy()));
        assert_eq!(root(&site, "chatgw", &project, rendered), Some(Ok(beside)));
    }

    /// A rendering refusal is the template's own, said under the key that
    /// carried it (§FS-018-private-sources.2).
    #[test]
    fn issue_191_a_private_template_that_will_not_render_names_its_key() {
        let site = site(Some("{nope}"));
        let why = root(&site, "chatgw", Path::new("/p"), |_| {
            Err("rhei: names {nope}".to_string())
        })
        .unwrap()
        .unwrap_err();
        assert!(why.starts_with("work.private.root: rhei:"), "{why}");
    }

    /// A laid plan answers with the item file carried beside it
    /// (§FS-018-private-sources.3, §FS-005-dispatch.19).
    #[test]
    fn issue_191_a_laid_plan_is_whose_its_carried_item_says() {
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join("chatgw-dana-draft-reply");
        fs::create_dir_all(&output).unwrap();
        let index = output.join("index.rhei.md");
        fs::write(
            &index,
            "# Rhei: t\n**States:** m\n\n## Tasks\n\n### Task draft: d\n**State:** fix\n\nw\n",
        )
        .unwrap();
        let carried = tmp
            .path()
            .join(plan::CARRIED)
            .join("chatgw-dana-draft-reply");
        fs::create_dir_all(&carried).unwrap();
        fs::write(carried.join("item.json"), r#"{"source": "chatgw"}"#).unwrap();
        let plan_ref = PlanRef {
            project: "rhei".to_string(),
            plan_id: "chatgw-dana-draft-reply".to_string(),
            path: index.clone(),
            item: None,
            title: String::new(),
        };
        let plan = Plan::read(&index).unwrap().unwrap();
        let site = site(Some("/elsewhere"));
        assert_eq!(
            of_ticket(&site, &plan_ref, true, &plan, "draft"),
            Some("chatgw")
        );
        assert_eq!(of_ticket(&site, &plan_ref, false, &plan, "draft"), None);
        assert_eq!(
            of_ticket(&WorkConfig::default(), &plan_ref, true, &plan, "draft"),
            None
        );
    }
}
