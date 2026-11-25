use super::*;

/// §FS-021-monitors.9.2: the digest that tells an unchanged file is the
/// content's, and nothing else's.
#[test]
fn a_digest_follows_the_content() {
    assert_eq!(digest(b"abc"), digest(b"abc"));
    assert_ne!(digest(b"abc"), digest(b"abd"));
    assert!(digest(b"").starts_with("fnv1a64:"));
}

/// §FS-021-monitors.5.6: the samples are the answer's `monitors`, and an
/// answer without them carries none.
#[test]
fn samples_are_the_answers_monitors() {
    let text = r#"{"v":1,"monitors":[{"id":"a","value":1}]}"#;
    assert_eq!(
        samples(text),
        vec![serde_json::json!({"id": "a", "value": 1})]
    );
    assert!(samples(r#"{"v":1}"#).is_empty());
}

/// §FS-021-monitors.5.3: with no place on disk nothing runs, and each read
/// says why.
#[test]
fn a_project_with_no_place_reads_nothing() {
    let job = Job {
        source: Source::Command {
            command: "true".to_string(),
            cwd: None,
        },
        reads: ["a".to_string()].into_iter().collect(),
        timeout: 30,
    };
    let got = read_all(&[job], None);
    assert!(matches!(&got[0].outcome, Outcome::Failed(why) if why.contains("no checkout")));
}
