//! Resolving `--source` and `--only` over the selected projects, before any
//! data call (§FS-011-command-line.9.2).

use serde_json::json;

use super::*;

fn config() -> StatusConfig {
    serde_json::from_value(json!({
        "sources": [{ "provider": "gmail" }, { "provider": "gdev", "block": "site" }],
        "projects": {
            "graal": { "providers": [
                { "provider": "gdev", "block": "a" },
                { "provider": "github-prs", "repos": ["acme/app"] }
            ] },
            "mail": { "providers": [{ "provider": "gmail" }] },
            "bare": { "providers": [] }
        }
    }))
    .unwrap()
}

fn resolved(over: &[&str], source: &str, only: Option<&'static str>) -> Result<Narrowing> {
    let config = config();
    let names: Vec<String> = over.iter().map(|project| project.to_string()).collect();
    let selected: Vec<(&String, &ProjectFeedConfig)> = names
        .iter()
        .map(|project| (project, &config.projects[project.as_str()]))
        .collect();
    resolve(&config, &selected, source, only)
}

fn bindings(narrowing: &Narrowing) -> Vec<(&str, Binding)> {
    narrowing
        .projects
        .iter()
        .map(|(project, binding)| (project.as_str(), *binding))
        .collect()
}

/// Each selected project uses its own binding of the name where it has one,
/// and the site's otherwise; one that neither reaches is skipped and named,
/// not refused (§FS-011-command-line.9.2, §FS-001-forge-interface.9).
#[test]
fn each_project_asks_its_own_binding_then_the_sites_or_is_skipped() {
    let gdev = resolved(&["graal", "bare"], "gdev", None).unwrap();
    assert_eq!(
        bindings(&gdev),
        [("graal", Binding::Own), ("bare", Binding::Site)]
    );
    assert!(gdev.asks_the_site());
    assert!(matches!(gdev.ask("graal"), Some(Ask::Source("gdev"))));
    assert!(
        gdev.ask("bare").is_none(),
        "the site's binding is asked once, apart"
    );
    assert!(gdev.skipped().is_empty());

    let prs = resolved(&["graal", "mail", "bare"], "github-prs", None).unwrap();
    assert_eq!(
        bindings(&prs),
        [
            ("graal", Binding::Own),
            ("mail", Binding::Unbound),
            ("bare", Binding::Unbound)
        ]
    );
    assert!(!prs.asks_the_site());
    assert_eq!(prs.skipped(), ["mail", "bare"]);

    // A project's own binding of a name the site also binds is the one meant.
    let gmail = resolved(&["mail", "bare"], "gmail", None).unwrap();
    assert_eq!(
        bindings(&gmail),
        [("mail", Binding::Own), ("bare", Binding::Site)]
    );
}

/// The refusals that need no probe, each decided from the configuration
/// alone (§FS-011-command-line.9.2 refusals 3, 4 and 6).
#[test]
fn what_nothing_finer_or_nothing_at_all_answers_is_refused() {
    let refused = |over: &[&str], source: &str, only: Option<&'static str>| {
        resolved(over, source, only)
            .err()
            .unwrap_or_else(|| panic!("--source {source} --only {only:?} over {over:?}"))
            .to_string()
    };
    // 3: a native provider and a task store answer whole.
    assert!(refused(&["graal"], "github-prs", Some("issues")).contains("a native provider"));
    assert!(refused(&["graal"], "rhei", Some("issues")).contains("a task store"));
    // 4: one project in scope reaches the source through the site.
    let site = refused(&["graal", "bare"], "gdev", Some("issues"));
    assert!(site.contains("'bare'") && site.contains("site"), "{site}");
    // 6: nothing in scope binds the name, and a task store says why.
    assert!(refused(&["bare"], "nosuch", None).contains("'nosuch'"));
    assert!(refused(&["bare"], "rhei", None).contains("task store"));
}
