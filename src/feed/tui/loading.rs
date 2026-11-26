//! The interface before its session has loaded (§FS-019-waiting.5.1).
//!
//! `ephor tui` enters the terminal first and then opens the session where it
//! always opened it, on the main thread, while a thread of its own draws this
//! frame. The load says each step as it begins over a channel, the frame says
//! the latest one, and the keys are still read, so `q` and `ctrl-c` quit
//! before anything has landed. What the load returns is handed back whole,
//! and the shell carries on with it exactly as it did when the load ran
//! before the terminal was entered (§AR-009-surfaces.2).

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

use ratatui::backend::Backend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Paragraph;
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
    Quit,
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
    /// read yet.
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
            Paragraph::new(format!("  {}", self.line())).style(dim),
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

/// Draw the frame on a thread of its own while `load` runs here, until it
/// lands or the reader quits (§FS-019-waiting.5.1). The load keeps the thread
/// it always ran on, so its reading and what it costs are what they were. A
/// load that fails is returned as it failed, so the shell reports it as it
/// always has, and one that panics panics here, once the frame has stopped.
/// A quit runs `quit` on the frame's thread, while the load is still reading.
pub(super) fn wait_for<B, T, L, K, Q>(
    terminal: &mut Terminal<B>,
    load: L,
    key: K,
    quit: Q,
) -> Result<Waited<T>>
where
    B: Backend + Send,
    L: FnOnce(Progress) -> Result<T>,
    K: FnMut(Duration) -> Result<Option<KeyEvent>> + Send,
    Q: FnOnce() + Send,
{
    let (said, hears) = mpsc::channel();
    std::thread::scope(|scope| {
        let frame = std::thread::Builder::new()
            .name("ephor-loading".to_string())
            .spawn_scoped(scope, move || show(terminal, hears, key, quit))
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
            Ok(Ok(Shown::Landed)) => loaded.map(Waited::Loaded),
            Ok(Ok(Shown::Quit)) => Ok(Waited::Quit),
            Ok(Err(err)) => Err(err),
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

/// The frame's own loop: draw what was said last, take what was said since,
/// read the keys, until the load has landed or the reader quits.
fn show<B, K, Q>(
    terminal: &mut Terminal<B>,
    hears: Receiver<Opening>,
    mut key: K,
    quit: Q,
) -> Result<Shown>
where
    B: Backend,
    K: FnMut(Duration) -> Result<Option<KeyEvent>>,
    Q: FnOnce(),
{
    let mut screen = LoadingScreen::default();
    loop {
        terminal
            .draw(|frame| screen.draw(frame))
            .map_err(|err| EphorError::Command(format!("draw failed: {err}")))?;
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
                quit();
                return Ok(Shown::Quit);
            }
        }
    }
}

/// What quitting while the load reads does: leave the terminal as it was
/// found and end the process with success, as `q` does on any screen
/// (§FS-019-waiting.5.1). The load cannot be stopped where it is, so it ends
/// with the process, as an interrupt during the load always ended it.
pub(super) fn leave() {
    ratatui::restore();
    std::process::exit(0);
}

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;
