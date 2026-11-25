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
    let got = read_all(&[job], None, &Clock::starting(Utc::now()));
    assert!(matches!(&got[0].outcome, Outcome::Failed(why) if why.contains("no checkout")));
}

fn place(checkout: &Path) -> Where<'_> {
    Where {
        project: "alpha",
        root: checkout,
        checkout,
        organization: None,
        forest: None,
    }
}

fn job(source: Source, timeout: u64) -> Job {
    Job {
        source,
        reads: ["a".to_string()].into_iter().collect(),
        timeout,
    }
}

fn file(path: &str) -> Source {
    Source::File {
        file: path.to_string(),
        cwd: None,
    }
}

/// §FS-021-monitors.7.1: an answer is timed when it is taken, after its
/// command ran, not when the refresh began.
#[test]
fn an_answer_is_timed_when_it_is_taken() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("kpis.sh"),
        "sleep 1\nprintf '{\"v\":1}' > \"$EPHOR_ANSWER\"\n",
    )
    .unwrap();
    let began = Utc::now();
    let command = Source::Command {
        command: "sh kpis.sh".to_string(),
        cwd: None,
    };
    let got = read_all(
        &[job(command, 30)],
        Some(&place(dir.path())),
        &Clock::starting(began),
    );
    assert!(matches!(got[0].outcome, Outcome::Answered(_)), "{got:?}");
    assert!(
        got[0].read_at >= began + chrono::TimeDelta::seconds(1),
        "{} is not a second after {began}",
        got[0].read_at
    );
}

/// §FS-021-monitors.12.1: a file that is not there is said missing, in the
/// words an unknown matter's title says it.
#[test]
fn a_missing_file_is_said_missing() {
    let dir = tempfile::tempdir().unwrap();
    let got = read_all(
        &[job(file("target/kpi/coverage.json"), 30)],
        Some(&place(dir.path())),
        &Clock::starting(Utc::now()),
    );
    assert_eq!(
        got[0].outcome,
        Outcome::Failed("target/kpi/coverage.json is missing".to_string())
    );
}

/// §FS-021-monitors.5.2, §FS-021-monitors.17.2: a pipe is not read, so a
/// file monitor never holds the refresh waiting on a writer.
#[cfg(unix)]
#[test]
fn a_pipe_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("f.json");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap();
    assert!(made.success());
    let (sent, done) = std::sync::mpsc::channel();
    let checkout = dir.path().to_path_buf();
    std::thread::spawn(move || {
        let got = read_all(
            &[job(file("f.json"), 1)],
            Some(&place(&checkout)),
            &Clock::starting(Utc::now()),
        );
        let _ = sent.send(got);
    });
    let got = done
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("reading a pipe held the refresh");
    assert_eq!(
        got[0].outcome,
        Outcome::Failed("f.json is not a regular file".to_string())
    );
}

/// §FS-021-monitors.5.2: a file larger than an answer is is not read whole.
#[test]
fn a_file_past_the_limit_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let big = std::fs::File::create(dir.path().join("big.json")).unwrap();
    big.set_len(FILE_LIMIT_BYTES + 1).unwrap();
    let got = read_all(
        &[job(file("big.json"), 30)],
        Some(&place(dir.path())),
        &Clock::starting(Utc::now()),
    );
    assert!(
        matches!(&got[0].outcome, Outcome::Failed(why) if why.starts_with("big.json is larger than")),
        "{:?}",
        got[0].outcome
    );
}
