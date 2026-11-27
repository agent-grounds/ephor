//! The interface before its session has loaded (§FS-019-waiting.5.1).
//!
//! `ephor tui` enters the terminal first and then opens the session where it
//! always opened it, on the main thread, while a thread of its own draws this
//! frame. The load says each step as it begins over a channel, the frame says
//! the latest one, and the keys are still read, so `q` and `ctrl-c` quit
//! before anything has landed. What the load returns is handed back whole,
//! and the shell carries on with it exactly as it did when the load ran
//! before the terminal was entered (§AR-009-surfaces.2).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::ThreadId;
use std::time::Duration;

use ratatui::backend::Backend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Padding, Paragraph, Wrap};
use ratatui::Terminal;

use crate::api::opening::{Opening, Progress};
use crate::error::{EphorError, Result};

/// How long the frame waits on the load before it reads the keys again. A
/// step, and the end of the load, are taken the moment they are said; only a
/// key waits, and never longer than this.
const TICK: Duration = Duration::from_millis(50);

/// What the frame says before the load has said its first step.
pub(super) const STARTING: &str = "Starting…";

/// How the wait for the load ended.
pub(super) enum Waited<T> {
    Loaded(T),
    /// The reader quit before the load landed. [`wait_for`] is handed what
    /// quitting does; when that returns, as it does only under test, what the
    /// load returns is dropped once it lands.
    Quit,
}

/// How the frame stopped being drawn.
enum Shown {
    /// The load finished, whatever it returned.
    Landed,
    /// A thread panicked, and the load is left to carry the panic
    /// ([`Halt`]).
    Halted,
    Quit,
}

/// Why the frame hands the terminal back before the load has landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Left {
    /// The reader quit (§FS-019-waiting.5.1).
    Quit,
    /// The terminal stopped answering: the frame can neither be drawn nor
    /// read the keys. The load goes on to its end, with the terminal as it
    /// was found, so `ctrl-c` there is an interrupt again (§AR-009-surfaces.2).
    Broken,
}

/// The frame: the step being read now, none before the first.
#[derive(Debug, Default)]
pub(super) struct LoadingScreen {
    step: Option<Opening>,
}

impl LoadingScreen {
    /// The step the load says it has begun.
    pub fn say(&mut self, step: Opening) {
        self.step = Some(step);
    }

    /// The one line the frame says.
    pub fn line(&self) -> String {
        match self.step {
            Some(step) => format!("{}…", sentence(step)),
            None => STARTING.to_string(),
        }
    }

    /// The header, the line, and the one key there is (§FS-019-waiting.5.1):
    /// no section, count or message, because nothing they come from has been
    /// read yet. A line wider than the screen goes on to the next one, so the
    /// count it ends with is never what is cut.
    pub fn draw(&self, frame: &mut ratatui::Frame) {
        let [header, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        let dim = Style::default().fg(Color::DarkGray);
        frame.render_widget(
            Paragraph::new(" ephor").style(Style::default().add_modifier(Modifier::BOLD)),
            header,
        );
        frame.render_widget(
            Paragraph::new(self.line())
                .style(dim)
                .block(Block::new().padding(Padding::left(2)))
                .wrap(Wrap { trim: true }),
            body,
        );
        frame.render_widget(Paragraph::new(" q quit").style(dim), footer);
    }
}

/// A step in ephor's words, with the count it has (§FS-019-waiting.5.1).
pub(super) fn sentence(step: Opening) -> String {
    match step {
        Opening::Registry => "Reading the registry".to_string(),
        Opening::Feeds { projects } => format!(
            "Reading the cached feeds of {projects} {}",
            plural(projects, "project")
        ),
        Opening::Branches { checkouts, done } => format!(
            "Reading the branches of {checkouts} {} — {done} done",
            plural(checkouts, "checkout")
        ),
        Opening::Capabilities { projects } => format!(
            "Reading what {projects} {} can do",
            plural(projects, "project")
        ),
        Opening::Work => "Reading the work handed over".to_string(),
        Opening::Jobs => "Reading the jobs ephor started".to_string(),
        Opening::Roots => "Reading the work roots".to_string(),
    }
}

fn plural(count: usize, noun: &str) -> String {
    match count {
        1 => noun.to_string(),
        _ => format!("{noun}s"),
    }
}

/// Whether a key quits while the load runs: `q` and `ctrl-c`, the two that
/// quit the screens it opens (§FS-019-waiting.5.1). Nothing else is on the
/// screen to act on.
fn quits(key: &KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    // Read as the event loop reads them: `ctrl-c` first, then the key alone.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }
    matches!(key.code, KeyCode::Char('q'))
}

/// The terminal's next key, waiting at most `wait` for one. Anything else the
/// terminal reports is passed over: the next frame is drawn at the size it
/// has then.
pub(super) fn terminal_key(wait: Duration) -> Result<Option<KeyEvent>> {
    if !event::poll(wait).map_err(|err| EphorError::Command(format!("event poll failed: {err}")))? {
        return Ok(None);
    }
    match event::read().map_err(|err| EphorError::Command(format!("event read failed: {err}")))? {
        Event::Key(key) => Ok(Some(key)),
        _ => Ok(None),
    }
}

/// What stops the frame when a thread panics while it is up
/// (§AR-009-surfaces.2). The hook `ratatui::init` sets gives the terminal
/// back and prints the panic; without this the frame would go on drawing over
/// that message, because a standing worker that panics leaves the others
/// counting, and the count reaches the frame, until the load comes to the
/// panic and carries it out.
#[derive(Debug, Default)]
pub(super) struct Halt {
    halted: AtomicBool,
    /// Held for each draw, so a stop waits out the one under way.
    drawing: Mutex<()>,
    /// The frame's own thread, which never waits on its own draw.
    frame: OnceLock<ThreadId>,
}

impl Halt {
    /// A halt the process's panic hook stops before anything else it does.
    /// It wraps whatever hook there is, once, before the load; it stays, and
    /// once the frame is gone it only sets a flag nobody reads.
    pub fn on_panic() -> Arc<Halt> {
        let halt = Arc::new(Halt::default());
        let stops = Arc::clone(&halt);
        let then = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            stops.stop();
            then(info);
        }));
        halt
    }

    /// Once this returns no frame is being drawn and none will be.
    pub fn stop(&self) {
        self.halted.store(true, Ordering::SeqCst);
        // A panic in the frame's own draw holds the lock on this thread.
        if self.frame.get() != Some(&std::thread::current().id()) {
            drop(self.drawing.lock().unwrap_or_else(PoisonError::into_inner));
        }
    }

    /// Draw `screen` unless stopped, and say whether it was drawn.
    fn draw<B: Backend>(&self, terminal: &mut Terminal<B>, screen: &LoadingScreen) -> Result<bool> {
        let _drawing = self.drawing.lock().unwrap_or_else(PoisonError::into_inner);
        if self.halted.load(Ordering::SeqCst) {
            return Ok(false);
        }
        terminal
            .draw(|frame| screen.draw(frame))
            .map_err(|err| EphorError::Command(format!("draw failed: {err}")))?;
        Ok(true)
    }
}

/// Run `load` here while the frame is drawn on a thread of its own, until
/// the load lands or the reader quits (§FS-019-waiting.5.1). The load keeps
/// the thread it always ran on, so its reading and what it costs are what
/// they were. A load that fails is returned as it failed, so the shell
/// reports it as it always has, even where the frame failed too; one that
/// panics panics here, once the frame has stopped. A quit, or a terminal that
/// stops answering, runs `leave` on the frame's thread while the load is
/// still reading, with the cursor every frame hid shown again.
pub(super) fn wait_for<B, T, L, K, Q>(
    terminal: &mut Terminal<B>,
    halt: &Halt,
    load: L,
    key: K,
    leave: Q,
) -> Result<Waited<T>>
where
    B: Backend + Send,
    L: FnOnce(Progress) -> Result<T>,
    K: FnMut(Duration) -> Result<Option<KeyEvent>> + Send,
    Q: FnOnce(Left) + Send,
{
    let (said, hears) = mpsc::channel();
    std::thread::scope(|scope| {
        let frame = std::thread::Builder::new()
            .name("ephor-loading".to_string())
            .spawn_scoped(scope, move || show(terminal, halt, hears, key, leave))
            .map_err(|err| EphorError::Command(format!("cannot draw while reading: {err}")))?;
        let loaded = {
            // Dropped when the load returns or unwinds, which is how the frame
            // hears that it has landed.
            let say = move |step: Opening| {
                let _ = said.send(step);
            };
            load(&say)
        };
        match frame.join() {
            Ok(Ok(Shown::Landed | Shown::Halted)) => loaded.map(Waited::Loaded),
            Ok(Ok(Shown::Quit)) => Ok(Waited::Quit),
            // The load's own failure is the one to say: the frame's says only
            // that the terminal stopped answering (§AR-009-surfaces.2).
            Ok(Err(err)) => loaded.and(Err(err)),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

/// The frame's thread: draw until the load lands, then hand the terminal back
/// through `leave` if the reader quit or the terminal broke.
fn show<B, K, Q>(
    terminal: &mut Terminal<B>,
    halt: &Halt,
    hears: Receiver<Opening>,
    mut key: K,
    leave: Q,
) -> Result<Shown>
where
    B: Backend,
    K: FnMut(Duration) -> Result<Option<KeyEvent>>,
    Q: FnOnce(Left),
{
    let _ = halt.frame.set(std::thread::current().id());
    let shown = frames(terminal, halt, &hears, &mut key);
    let left = match &shown {
        Ok(Shown::Quit) => Left::Quit,
        Err(_) => Left::Broken,
        Ok(Shown::Landed | Shown::Halted) => return shown,
    };
    // Every frame hid the cursor; it is shown again before the terminal is
    // handed back, as dropping the terminal shows it after any other screen.
    let _ = terminal.show_cursor();
    leave(left);
    shown
}

/// Draw what was said last, take what was said since, read the keys, until
/// the load has landed, the reader quits, or a panic halts the frame.
fn frames<B, K>(
    terminal: &mut Terminal<B>,
    halt: &Halt,
    hears: &Receiver<Opening>,
    key: &mut K,
) -> Result<Shown>
where
    B: Backend,
    K: FnMut(Duration) -> Result<Option<KeyEvent>>,
{
    let mut screen = LoadingScreen::default();
    loop {
        if !halt.draw(terminal, &screen)? {
            return Ok(Shown::Halted);
        }
        match hears.recv_timeout(TICK) {
            Ok(step) => screen.say(step),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(Shown::Landed),
        }
        // Everything said since the last frame goes into the next one.
        loop {
            match hears.try_recv() {
                Ok(step) => screen.say(step),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(Shown::Landed),
            }
        }
        while let Some(pressed) = key(Duration::ZERO)? {
            if quits(&pressed) {
                return Ok(Shown::Quit);
            }
        }
    }
}

/// Held by the one write the load makes, the sweep of jobs long over
/// (§FS-005-dispatch.17), for as long as it writes.
static WRITING: Mutex<()> = Mutex::new(());

/// Hold off a quit while the load writes: a quit that comes then waits for
/// the write to finish (§AR-009-surfaces.2).
pub(super) fn writing() -> MutexGuard<'static, ()> {
    WRITING.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What handing the terminal back does while the load reads: leave it as it
/// was found, and on a quit end the process with success, as `q` does on any
/// screen (§FS-019-waiting.5.1). The load cannot be stopped where it is, so
/// it ends with the process, once no write of its own is under way.
pub(super) fn leave(left: Left) {
    ratatui::restore();
    if left == Left::Quit {
        ending(&WRITING, || std::process::exit(0));
    }
}

/// Run `end` once nothing holds `writes`.
fn ending(writes: &Mutex<()>, end: impl FnOnce()) {
    let _unbroken = writes.lock().unwrap_or_else(PoisonError::into_inner);
    end();
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;
