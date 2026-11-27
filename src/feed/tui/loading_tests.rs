//! The frame `ephor tui` draws before its session has loaded, and the wait
//! behind it (§FS-019-waiting.5.1). Attached by `#[path]` from `loading.rs`,
//! so this is `feed::tui::loading::tests`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use ratatui::backend::{Backend, ClearType, TestBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Position, Size};
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
fn rows_of(backend: &TestBackend) -> Vec<String> {
    let buffer = backend.buffer();
    let area = buffer.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

fn rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    rows_of(terminal.backend())
}

fn drawn_at(width: u16, step: Option<Opening>) -> Vec<String> {
    let mut screen = LoadingScreen::default();
    if let Some(step) = step {
        screen.say(step);
    }
    let mut terminal = Terminal::new(TestBackend::new(width, HEIGHT)).unwrap();
    terminal.draw(|frame| screen.draw(frame)).unwrap();
    rows(&terminal)
}

fn drawn(step: Option<Opening>) -> Vec<String> {
    drawn_at(WIDTH, step)
}

fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Press)
}

/// What a [`Recording`] terminal was asked to do, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Drew,
    /// A frame drawn once the test said a panic had been printed.
    DrewLate,
    Hid,
    Showed,
    Left(Left),
}

/// A test terminal that writes down what it is asked to do, and can be made
/// to stop answering.
struct Recording {
    inner: TestBackend,
    calls: Arc<Mutex<Vec<Call>>>,
    late: Arc<AtomicBool>,
    broken: bool,
}

impl Recording {
    fn new(calls: &Arc<Mutex<Vec<Call>>>) -> Self {
        Recording {
            inner: TestBackend::new(WIDTH, HEIGHT),
            calls: Arc::clone(calls),
            late: Arc::new(AtomicBool::new(false)),
            broken: false,
        }
    }

    fn call(&self, call: Call) {
        self.calls.lock().unwrap().push(call);
    }
}

impl Backend for Recording {
    fn draw<'a, I>(&mut self, content: I) -> std::io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        if self.broken {
            return Err(std::io::Error::other("the terminal is gone"));
        }
        self.call(match self.late.load(Ordering::SeqCst) {
            true => Call::DrewLate,
            false => Call::Drew,
        });
        self.inner.draw(content)
    }
    fn hide_cursor(&mut self) -> std::io::Result<()> {
        self.call(Call::Hid);
        self.inner.hide_cursor()
    }
    fn show_cursor(&mut self) -> std::io::Result<()> {
        self.call(Call::Showed);
        self.inner.show_cursor()
    }
    fn get_cursor_position(&mut self) -> std::io::Result<Position> {
        self.inner.get_cursor_position()
    }
    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> std::io::Result<()> {
        self.inner.set_cursor_position(position)
    }
    fn clear(&mut self) -> std::io::Result<()> {
        self.inner.clear()
    }
    fn clear_region(&mut self, clear_type: ClearType) -> std::io::Result<()> {
        self.inner.clear_region(clear_type)
    }
    fn size(&self) -> std::io::Result<Size> {
        self.inner.size()
    }
    fn window_size(&mut self) -> std::io::Result<WindowSize> {
        self.inner.window_size()
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
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
        (
            Some(Opening::Capabilities { projects: 15 }),
            "Reading what 15 projects can do…",
        ),
        (
            Some(Opening::Capabilities { projects: 1 }),
            "Reading what 1 project can do…",
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

/// A line wider than the screen goes on to the next line, indented as the
/// first, so the count it ends with is still on the screen
/// (§FS-019-waiting.5.1).
#[test]
fn a_line_wider_than_the_screen_goes_on_to_the_next() {
    let rows = drawn_at(
        40,
        Some(Opening::Branches {
            checkouts: 749,
            done: 12,
        }),
    );
    assert_eq!(
        rows[1], "  Reading the branches of 749 checkouts",
        "{rows:#?}"
    );
    assert_eq!(rows[2], "  — 12 done…", "{rows:#?}");
    assert_eq!(rows[rows.len() - 1], " q quit", "{rows:#?}");
}

/// What leaving does under test, where it must not end the process: it is
/// written down.
fn leaves(calls: &Arc<Mutex<Vec<Call>>>) -> impl FnOnce(Left) + Send {
    let calls = Arc::clone(calls);
    move |left| calls.lock().unwrap().push(Call::Left(left))
}

/// What leaving does where the frame should not leave at all.
fn never(left: Left) {
    panic!("the frame left: {left:?}");
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
        &Halt::default(),
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
        &Halt::default(),
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
/// reads, and what quitting does runs then rather than once the load lands,
/// with the cursor every frame hid shown first. Under test quitting returns,
/// so the load goes on to land, and what it returns is dropped
/// (§FS-019-waiting.5.1).
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
    let calls = Arc::new(Mutex::new(Vec::new()));
    let left = leaves(&calls);
    let mut terminal = Terminal::new(Recording::new(&calls)).unwrap();
    let mut after = None;
    let waited = wait_for(
        &mut terminal,
        &Halt::default(),
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
        move |how| {
            quit.store(reading.load(Ordering::SeqCst), Ordering::SeqCst);
            left(how);
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
        rows_of(&terminal.backend().inner)[1],
        "  Reading the branches of 3 checkouts — 1 done…"
    );
    let calls = calls.lock().unwrap().clone();
    assert_eq!(
        calls[calls.len() - 2..],
        [Call::Showed, Call::Left(Left::Quit)],
        "the cursor is shown before the terminal is handed back: {calls:?}"
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
        &Halt::default(),
        move |_| {
            gate.recv().unwrap();
            Ok(())
        },
        |_| Ok(Some(press(KeyCode::Char('c'), KeyModifiers::CONTROL))),
        move |left| {
            assert_eq!(left, Left::Quit);
            release.send(()).unwrap();
        },
    )
    .unwrap();
    assert!(matches!(waited, Waited::Quit));
}

/// A quit that comes while the load writes waits for the write to finish
/// before the process ends (§AR-009-surfaces.2).
#[test]
fn a_quit_waits_for_a_write_under_way() {
    let writes = Mutex::new(());
    let ended = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let held = writes.lock().unwrap();
        let quitting = scope.spawn(|| ending(&writes, || ended.store(true, Ordering::SeqCst)));
        std::thread::sleep(Duration::from_millis(100));
        assert!(!ended.load(Ordering::SeqCst), "the quit cut the write");
        drop(held);
        quitting.join().unwrap();
    });
    assert!(ended.load(Ordering::SeqCst), "the quit never came");
}

/// A load that fails is returned exactly as it failed, so the shell reports
/// it as it did when the load ran before the terminal was entered: the same
/// sentence, and the same kind of error, which is what picks the exit code
/// (§FS-019-waiting.5.1). The exit code itself is E2E-088's to prove.
#[test]
fn a_load_that_fails_is_reported_as_it_failed() {
    for registry in [true, false] {
        let said = "the registry cannot be read";
        let failed = match registry {
            true => EphorError::Registry(said.into()),
            false => EphorError::Command(said.into()),
        };
        let mut terminal = terminal();
        let waited = wait_for(
            &mut terminal,
            &Halt::default(),
            move |say| -> Result<()> {
                say(Opening::Registry);
                Err(failed)
            },
            |_| Ok(None),
            never,
        );
        match waited {
            Err(EphorError::Registry(got)) if registry => assert_eq!(got, said),
            Err(EphorError::Command(got)) if !registry => assert_eq!(got, said),
            Err(other) => panic!("the kind of the failure changed: {other:?}"),
            Ok(_) => panic!("the failure was lost"),
        }
    }
}

/// A terminal that stops answering hands itself back at once, cursor shown,
/// while the load reads on; and where the load then fails too, its failure is
/// the one reported, not the frame's (§AR-009-surfaces.2).
#[test]
fn a_load_that_fails_wins_over_a_frame_that_failed() {
    let (broke, frame_broke) = mpsc::channel::<()>();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let left = leaves(&calls);
    let mut terminal = Terminal::new(Recording::new(&calls)).unwrap();
    let waited = wait_for(
        &mut terminal,
        &Halt::default(),
        move |_| -> Result<()> {
            frame_broke.recv().unwrap();
            Err(EphorError::Registry("the registry cannot be read".into()))
        },
        |_| Err(EphorError::Command("event poll failed: gone".into())),
        move |how| {
            left(how);
            broke.send(()).unwrap();
        },
    );
    match waited {
        Err(EphorError::Registry(said)) => assert_eq!(said, "the registry cannot be read"),
        Err(other) => panic!("the frame's failure won: {other:?}"),
        Ok(_) => panic!("the failure was lost"),
    }
    let calls = calls.lock().unwrap().clone();
    assert_eq!(
        calls[calls.len() - 2..],
        [Call::Showed, Call::Left(Left::Broken)],
        "{calls:?}"
    );
}

/// The frame's own failure is reported where the load landed.
#[test]
fn a_frame_that_failed_is_reported_where_the_load_landed() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut backend = Recording::new(&calls);
    backend.broken = true;
    let mut terminal = Terminal::new(backend).unwrap();
    let (release, gate) = mpsc::channel::<()>();
    let waited = wait_for(
        &mut terminal,
        &Halt::default(),
        move |_| {
            gate.recv().unwrap();
            Ok(7)
        },
        |_| Ok(None),
        move |left| {
            assert_eq!(left, Left::Broken);
            release.send(()).unwrap();
        },
    );
    match waited {
        Err(EphorError::Command(said)) => assert!(said.starts_with("draw failed"), "{said}"),
        Err(other) => panic!("{other:?}"),
        Ok(_) => panic!("the frame's failure was lost"),
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
            &Halt::default(),
            |_| -> Result<()> { panic!("the load broke") },
            |_| Ok(None),
            never,
        );
    }));
    let payload = caught.expect_err("the panic reached the caller");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"the load broke"));
}

/// A worker that panics while the others go on counting stops the frame
/// through the process's panic hook, before that hook goes on to print the
/// panic: nothing is drawn after it, however many steps are still said
/// (§AR-009-surfaces.2). The hook is the process's, so this installs it and
/// puts the default back.
#[test]
fn a_panic_on_any_thread_stops_the_frame_before_it_is_printed() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let backend = Recording::new(&calls);
    let late = Arc::clone(&backend.late);
    let mut terminal = Terminal::new(backend).unwrap();
    let halt = Halt::on_panic();
    let waited = wait_for(
        &mut terminal,
        &halt,
        |say| {
            say(Opening::Branches {
                checkouts: 9,
                done: 0,
            });
            std::thread::sleep(TICK * 2);
            let worker = std::thread::scope(|scope| {
                scope.spawn(|| panic!("a standing worker panicked")).join()
            });
            assert!(worker.is_err());
            // The hook ran on the worker before it ended: whatever is drawn
            // from here is drawn over the message.
            late.store(true, Ordering::SeqCst);
            for done in 1..9 {
                say(Opening::Branches { checkouts: 9, done });
                std::thread::sleep(TICK / 2);
            }
            Ok(())
        },
        |_| Ok(None),
        never,
    );
    drop(std::panic::take_hook());
    assert!(matches!(waited, Ok(Waited::Loaded(()))));
    let calls = calls.lock().unwrap().clone();
    assert!(!calls.contains(&Call::DrewLate), "{calls:?}");
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
            Opening::Capabilities { projects: 1 },
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
