//! The frame `ephor tui` draws before its session has loaded, and the wait
//! behind it (§FS-019-waiting.5.1). Attached by `#[path]` from `loading.rs`,
//! so this is `feed::tui::loading::tests`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use serde_json::json;

use super::*;
use crate::api::opening::Opening;
use crate::feed::config::StatusConfig;
use crate::feed::tui::App;

const WIDTH: u16 = 80;
const HEIGHT: u16 = 6;

fn terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap()
}

/// What the last frame drew, a row per line with its trailing blanks cut.
fn rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    (0..HEIGHT)
        .map(|y| {
            (0..WIDTH)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

fn drawn(step: Option<Opening>) -> Vec<String> {
    let mut screen = LoadingScreen::default();
    if let Some(step) = step {
        screen.say(step);
    }
    let mut terminal = terminal();
    terminal.draw(|frame| screen.draw(frame)).unwrap();
    rows(&terminal)
}

fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press)
}

/// Every step is one line in ephor's words, with the count it has, under the
/// header and over the one key there is — and nothing a reading would bring:
/// no section, no count of matters, no `No cached data` (§FS-019-waiting.5.1).
#[test]
fn each_step_is_one_line_in_ephors_words() {
    let said = [
        (None, "Starting…"),
        (Some(Opening::Registry), "Reading the registry…"),
        (
            Some(Opening::Feeds { projects: 15 }),
            "Reading the cached feeds of 15 projects…",
        ),
        (
            Some(Opening::Feeds { projects: 1 }),
            "Reading the cached feeds of 1 project…",
        ),
        (
            Some(Opening::Branches {
                checkouts: 29,
                done: 12,
            }),
            "Reading the branches of 29 checkouts — 12 done…",
        ),
        (
            Some(Opening::Branches {
                checkouts: 1,
                done: 0,
            }),
            "Reading the branches of 1 checkout — 0 done…",
        ),
        (Some(Opening::Work), "Reading the work handed over…"),
        (Some(Opening::Jobs), "Reading the jobs ephor started…"),
        (Some(Opening::Roots), "Reading the work roots…"),
    ];
    for (step, sentence) in said {
        let rows = drawn(step);
        assert_eq!(rows[0], " ephor", "{rows:#?}");
        assert_eq!(rows[1], format!("  {sentence}"), "{rows:#?}");
        assert!(
            rows[2..rows.len() - 1].iter().all(String::is_empty),
            "nothing under the line: {rows:#?}"
        );
        assert_eq!(rows[rows.len() - 1], " q quit", "{rows:#?}");
    }
}

/// What a quit does under test, where it must not end the process: nothing.
fn stays() {}

/// A quit where none may come.
fn never() {
    panic!("nothing quit");
}

/// A load that lands is handed back whole, and a key other than a quit
/// pressed while it reads does nothing (§FS-019-waiting.5.1).
#[test]
fn a_load_that_lands_is_handed_back_and_other_keys_wait_for_it() {
    let (release, gate) = mpsc::channel::<()>();
    let mut pressed = vec![
        press(KeyCode::Char('x'), KeyModifiers::NONE),
        press(KeyCode::Esc, KeyModifiers::NONE),
        press(KeyCode::Enter, KeyModifiers::NONE),
        // A `q` let go of is not a `q` pressed.
        KeyEvent::new_with_kind(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
    ];
    let mut terminal = terminal();
    let waited = wait_for(
        &mut terminal,
        move |say| {
            say(Opening::Registry);
            gate.recv().unwrap();
            Ok(7)
        },
        move |_| {
            let next = pressed.pop();
            if pressed.is_empty() {
                let _ = release.send(());
            }
            Ok(next)
        },
        never,
    )
    .unwrap();
    assert!(matches!(waited, Waited::Loaded(7)));
}

/// The load runs on the thread that waits for it, as it did before there was
/// a frame to draw: its stack and its heap are the ones it always had.
#[test]
fn the_load_runs_on_the_thread_that_waits_for_it() {
    let waiting = std::thread::current().id();
    let mut terminal = terminal();
    let waited = wait_for(
        &mut terminal,
        |_| Ok(std::thread::current().id()),
        |_| Ok(None),
        never,
    )
    .unwrap();
    assert!(matches!(waited, Waited::Loaded(ran) if ran == waiting));
}

/// What a load returns when the reader has quit before it landed: it says
/// when it is dropped.
struct Loaded(Arc<AtomicBool>);

impl Drop for Loaded {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// `q` quits while the load is still reading, on the frame that says what it
/// reads, and what quitting does runs then rather than once the load lands.
/// Under test quitting returns, so the load goes on to land, and what it
/// returns is dropped (§FS-019-waiting.5.1).
#[test]
fn q_quits_while_the_load_reads_and_its_result_is_dropped() {
    let (release, gate) = mpsc::channel::<()>();
    let (said_tx, said) = mpsc::channel::<()>();
    let dropped = Arc::new(AtomicBool::new(false));
    let loaded = Arc::clone(&dropped);
    let reading = Arc::new(AtomicBool::new(false));
    let read = Arc::clone(&reading);
    let quit_reading = Arc::new(AtomicBool::new(false));
    let quit = Arc::clone(&quit_reading);
    let mut terminal = terminal();
    let mut after = None;
    let waited = wait_for(
        &mut terminal,
        move |say| {
            say(Opening::Registry);
            say(Opening::Branches {
                checkouts: 3,
                done: 1,
            });
            read.store(true, Ordering::SeqCst);
            said_tx.send(()).unwrap();
            gate.recv().unwrap();
            read.store(false, Ordering::SeqCst);
            Ok(Loaded(loaded))
        },
        // Two frames after the load said its steps, the second has them: one
        // to take what was sent while the last was drawn, one to draw it.
        move |_| {
            if after.is_none() && said.try_recv().is_ok() {
                after = Some(0);
            }
            match after.as_mut() {
                Some(frames) if *frames >= 2 => {
                    Ok(Some(press(KeyCode::Char('q'), KeyModifiers::NONE)))
                }
                Some(frames) => {
                    *frames += 1;
                    Ok(None)
                }
                None => Ok(None),
            }
        },
        move || {
            quit.store(reading.load(Ordering::SeqCst), Ordering::SeqCst);
            release.send(()).unwrap();
        },
    )
    .unwrap();
    assert!(matches!(waited, Waited::Quit));
    assert!(
        quit_reading.load(Ordering::SeqCst),
        "the quit came while the load was reading"
    );
    assert_eq!(
        rows(&terminal)[1],
        "  Reading the branches of 3 checkouts — 1 done…"
    );
    assert!(
        dropped.load(Ordering::SeqCst),
        "what the load returned was kept"
    );
}

/// `ctrl-c` quits as `q` does (§FS-019-waiting.5.1).
#[test]
fn ctrl_c_quits_while_the_load_reads() {
    let (release, gate) = mpsc::channel::<()>();
    let mut terminal = terminal();
    let waited = wait_for(
        &mut terminal,
        move |_| {
            gate.recv().unwrap();
            Ok(())
        },
        |_| Ok(Some(press(KeyCode::Char('c'), KeyModifiers::CONTROL))),
        move || release.send(()).unwrap(),
    )
    .unwrap();
    assert!(matches!(waited, Waited::Quit));
}

/// A load that fails is returned exactly as it failed, so the shell reports
/// it as it did when the load ran before the terminal was entered: the same
/// sentence, and the kind of error that picks the exit code
/// (§FS-019-waiting.5.1).
#[test]
fn a_load_that_fails_is_reported_as_it_failed() {
    for (failed, code) in [
        (
            EphorError::Registry("the registry cannot be read".into()),
            2,
        ),
        (EphorError::Command("Corrupt seen file".into()), 1),
    ] {
        let expected = failed.to_string();
        let mut terminal = terminal();
        let waited = wait_for(
            &mut terminal,
            move |say| -> Result<()> {
                say(Opening::Registry);
                Err(failed)
            },
            |_| Ok(None),
            never,
        );
        let err = match waited {
            Err(err) => err,
            Ok(_) => panic!("the failure was lost"),
        };
        assert_eq!(err.to_string(), expected);
        let exit = match err {
            EphorError::Registry(_) => 2,
            EphorError::Command(_) => 1,
        };
        assert_eq!(exit, code, "the kind that picks the exit code is kept");
    }
}

/// A load that panics panics in the caller, with what it panicked with, once
/// the frame has stopped drawing.
#[test]
fn a_load_that_panics_panics_in_the_caller() {
    let mut terminal = terminal();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = wait_for(
            &mut terminal,
            |_| -> Result<()> { panic!("the load broke") },
            |_| Ok(None),
            stays,
        );
    }));
    let payload = caught.expect_err("the panic reached the caller");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"the load broke"));
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = crate::git::command()
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} in {}", dir.display());
}

/// The real load says its steps in the order it reads them, the branch
/// measurements counted one checkout at a time up to every checkout on disk,
/// and then the screen's own (§FS-019-waiting.5.1). Run in a home of its own.
#[test]
fn the_load_says_its_steps_in_order_with_the_checkouts_it_measures() {
    if crate::feed::tui::cache_freshness_tests::isolated(
        "feed::tui::loading::tests::the_load_says_its_steps_in_order_with_the_checkouts_it_measures",
    ) {
        return;
    }
    let home = std::env::current_dir().unwrap();
    let root = home.join("demo");
    let branches = ["feature-1", "feature-2", "feature-3"];
    for branch in branches {
        let repo = root.join(branch);
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "base"]);
    }
    std::fs::write(home.join("agents.tmpl"), "# AGENTS.md\n\n{summary}\n").unwrap();
    let registry = json!({
        "project_types": [{
            "id": "monorepo",
            "layout": "monorepo",
            "repos": [{
                "id": "repo", "path": ".", "role": "Repository root", "required": true,
                "update_mode": "branch", "default_branch": "{branch}"
            }],
            "agents": {
                "template": home.join("agents.tmpl").to_string_lossy(),
                "structure_intro": "This project uses a single repository root:",
                "summary_template": "This project root tracks `{display_name}` on branch `{branch}`."
            }
        }],
        "hook_sets": [],
        "projects": [{
            "id": "demo", "type": "monorepo", "display_name": "Demo",
            "root": root.to_string_lossy(), "main_branch": "main",
            "branch_root_template": "{project_root}/{branch}",
            "branches": branches.iter().map(|branch| json!({
                "id": branch, "branch": branch, "active": true
            })).collect::<Vec<_>>()
        }]
    });
    std::fs::write(home.join("registry.json"), registry.to_string()).unwrap();
    let config: StatusConfig =
        serde_json::from_value(json!({"projects": {"demo": {"providers": []}}})).unwrap();

    let steps = Mutex::new(Vec::new());
    let app = App::load_saying(&config, &crate::scope::Projects::every(), &|step| {
        steps.lock().unwrap().push(step)
    })
    .unwrap();
    let branches = |done| Opening::Branches { checkouts: 3, done };
    assert_eq!(
        steps.into_inner().unwrap(),
        [
            Opening::Registry,
            Opening::Feeds { projects: 1 },
            branches(0),
            branches(1),
            branches(2),
            branches(3),
            Opening::Work,
            Opening::Jobs,
            Opening::Roots,
        ]
    );
    assert_eq!(
        app.ctx.behind.len(),
        3,
        "every checkout was measured once the steps were said"
    );
}
