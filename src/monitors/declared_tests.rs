use super::*;

use serde_json::json;

/// §FS-021-monitors.4.6: a refused declaration is still shown — its id is
/// declared — and every refresh notes it, as it notes an unknown field.
#[test]
fn a_refused_declaration_is_declared_and_noted() {
    let declarations = read(
        None,
        &[
            json!({"id": "a", "heathy": {"min": 1}}),
            json!({"id": "b"}),
            json!({"id": "b"}),
        ],
    );
    assert_eq!(
        declarations.ids(),
        ["a", "b"].map(String::from).into_iter().collect()
    );
    assert!(declarations.any());
    assert!(declarations.site_declares());
    let notes = declarations.notes();
    assert!(
        notes
            .iter()
            .any(|note| note == "site monitor a: unknown field heathy"),
        "{notes:?}"
    );
    // Said as a rule the homes break together is, and once for the two
    // declarations that say it.
    let twice: Vec<&String> = notes
        .iter()
        .filter(|note| note.starts_with("site monitor b "))
        .collect();
    assert_eq!(
        twice,
        ["site monitor b is not read: b is declared more than once in the site configuration"],
        "{notes:?}"
    );
    // The id declared twice is not read until one of its declarations goes.
    assert!(!declarations.resolve("b", None).is_readable());
    assert!(declarations.resolve("a", None).faults.is_empty());
}

#[test]
fn a_refusal_without_an_id_is_named_by_its_place() {
    let declared = shape::read_declarations(&[json!({"kind": "number"})], Home::Manifest);
    assert_eq!(
        say_refused(&declared.refused[0]),
        "monitors[0]: a monitor's declaration needs an id"
    );
}

/// §FS-021-monitors.4.1: no placement, no manifest; the site's declarations
/// alone.
#[test]
fn a_project_with_no_place_reads_the_site_alone() {
    let declarations = read(None, &[]);
    assert!(!declarations.any());
    assert!(declarations.manifest_error.is_none());
}
