//! E2E-027-supported-remote-reader: a reader on the other end of SSH gets the
//! action on their terminal, never on an unseen screen.
//!
//! Browser opening is either an explicit binding, an eligible local graphical
//! default, or a terminal notice that keeps the complete URL (§FS-016-browser-opening).
//! Automatic windows use the same meaning of SSH: remote tmux remains useful,
//! inherited GUI-terminal markers do not (§FS-005-dispatch.22).

#![cfg(unix)]

#[path = "../support.rs"]
mod support;

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use ephor::feed::config::StatusConfig;
use serde_json::{json, Value};
use wait_timeout::ChildExt;

use support::*;

const ITEM: &str = "remote-browser";
const URL: &str = "https://reader.example/needs-me";
const SIGNALS: &[&str] = &[
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "SSH_CONNECTION",
    "SSH_CLIENT",
    "SSH_TTY",
    "TMUX",
    "WEZTERM_PANE",
    "KITTY_WINDOW_ID",
];

fn remote_world(url: &str, browser: Option<Value>, window: Option<Value>) -> World {
    let world = World::new();
    world.stub(
        "status-reporter",
        &answering(
            json!({
                "v": 1,
                "matters": [{
                    "key": ITEM,
                    "kind": "status",
                    "title": "Remote browser",
                    "url": url,
                    "state": "waiting",
                    "terminal": false,
                    "time": "2026-09-17T00:00:00Z"
                }]
            }),
            0,
        ),
    );
    let mut defaults = json!({ "ttl_seconds": 600, "provider_timeout_seconds": 2 });
    if let Some(browser) = browser {
        defaults["browser"] = browser;
    }
    if let Some(window) = window {
        defaults["window"] = window;
    }
    world.configure(json!({
        "defaults": defaults,
        "actions": [
            {
                "id": "windowed",
                "icon": "W",
                "description": "windowed control",
                "command": "printf window-action-ran\\n",
                "window": true,
                "when": { "kinds": ["status"] }
            },
            {
                "id": "attached",
                "icon": "A",
                "description": "attached control",
                "command": "printf attached-terminal-ok\\n",
                "when": { "kinds": ["status"] }
            }
        ],
        "projects": { PROJECT: { "providers": [{
            "provider": "custom-status",
            "command": "status-reporter",
            "format": "answer"
        }] } }
    }));
    symlink("/bin/sh", world.path().join("fakebin/sh"));
    clean(world.ephor())
        .args(["refresh", PROJECT])
        .assert()
        .success();
    world
}

fn symlink(target: impl AsRef<Path>, link: impl AsRef<Path>) {
    std::os::unix::fs::symlink(target, link).expect("install the isolated shell");
}

fn clean(mut command: assert_cmd::Command) -> assert_cmd::Command {
    for signal in SIGNALS {
        command.env_remove(signal);
    }
    command
}

fn isolated(command: &mut Command, world: &World, environment: &[(&str, &str)]) {
    command
        .env_clear()
        .env("HOME", world.path())
        .env("XDG_STATE_HOME", world.path().join("state"))
        .env("EPHOR_REGISTRY", world.registry_path())
        .env("EPHOR_STATUS_CONFIG", world.config_path())
        .env("PATH", world.path().join("fakebin"))
        .env("TERM", "xterm-256color");
    for (name, value) in environment {
        command.env(name, value);
    }
}

struct TuiRun {
    output: Output,
    compact: String,
    waited_for_enter: bool,
    opener_alive_before_enter: Option<bool>,
    outcome_after: Duration,
}

/// Wait for evidence from the PTY, retaining the raw transcript on a timeout
/// rather than sending an action to a screen that is not ready
/// (§FS-016-browser-opening.1, §FS-016-browser-opening.2).
fn await_terminal(
    child: &mut Child,
    receiver: &Receiver<Vec<u8>>,
    transcript: &mut Vec<u8>,
    phase: &str,
    timeout: Duration,
    ready: impl Fn(&[u8]) -> bool,
) {
    let deadline = Instant::now() + timeout;
    loop {
        if ready(transcript) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match receiver.recv_timeout(remaining) {
            Ok(bytes) => transcript.extend(bytes),
            Err(_) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!(
        "PTY never reached {phase} within {timeout:?}; raw transcript: {:?}",
        String::from_utf8_lossy(transcript)
    );
}

fn tui(
    world: &World,
    keys: &[u8],
    environment: &[(&str, &str)],
    notice_timeout: Option<Duration>,
) -> TuiRun {
    let binary = assert_cmd::cargo::cargo_bin("ephor");
    let script_command = format!(
        "/bin/stty rows 16 cols 48; {} tui",
        ephor::seams::summons::quote(&binary.to_string_lossy())
    );
    let mut command = Command::new("/usr/bin/script");
    command
        .args(["-qefc", &script_command, "/dev/null"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    isolated(&mut command, world, environment);
    let mut child = command.spawn().expect("start the TUI under a pty");
    let mut input = child.stdin.take().expect("the pty input");
    let mut stdout = child.stdout.take().expect("the pty output");
    let mut stderr = child.stderr.take().expect("script errors");
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut bytes = [0; 4096];
        while let Ok(count) = stdout.read(&mut bytes) {
            if count == 0 || sender.send(bytes[..count].to_vec()).is_err() {
                break;
            }
        }
    });
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let mut transcript = Vec::new();
    let readiness_timeout = Duration::from_secs(10);
    await_terminal(
        &mut child,
        &receiver,
        &mut transcript,
        "loaded row and footer",
        readiness_timeout,
        |bytes| {
            let text = compact_terminal(bytes);
            text.contains("Remotebrowser[waiting]") && text.contains("j/kmoveenterthread")
        },
    );
    assert_eq!(
        keys.first(),
        Some(&b'j'),
        "select the fixture's sole matter"
    );
    let selection_at = transcript.len();
    input.write_all(b"j").expect("select the matter");
    await_terminal(
        &mut child,
        &receiver,
        &mut transcript,
        "selected matter",
        readiness_timeout,
        |bytes| {
            let text = compact_terminal(&bytes[selection_at..]);
            text.contains('▸') && text.contains("Remotebrowser[waiting]")
        },
    );
    let action_at = transcript.len();
    let action_started = Instant::now();
    input
        .write_all(&keys[1..])
        .expect("act on the selected matter");

    let mut waited_for_enter = false;
    let mut opener_alive_before_enter = None;
    match notice_timeout {
        Some(timeout) => {
            await_terminal(
                &mut child,
                &receiver,
                &mut transcript,
                "Enter handoff",
                timeout.max(Duration::from_secs(2)),
                |bytes| {
                    compact_terminal(&bytes[action_at..]).contains("PressEntertoreturntoephor…")
                },
            );
            let pid = read_or_empty(world, "opener.pid").trim().to_string();
            if !pid.is_empty() {
                opener_alive_before_enter = Some(process_alive(&pid));
            }
            // A fallback owns the terminal until Enter. If `q` exits here, it
            // was only a status-line message and the copyable floor was never
            // shown.
            input.write_all(b"q").expect("probe the Enter handoff");
            thread::sleep(Duration::from_millis(250));
            waited_for_enter = child.try_wait().expect("probe the TUI").is_none();
            for bytes in receiver.try_iter() {
                transcript.extend(bytes);
            }
            assert!(
                !transcript[action_at..]
                    .windows(8)
                    .any(|bytes| bytes == b"\x1b[?1049h"),
                "redrew before Enter: {:?}",
                String::from_utf8_lossy(&transcript)
            );
            let resumed_at = transcript.len();
            input.write_all(b"\n").expect("acknowledge the notice");
            await_terminal(
                &mut child,
                &receiver,
                &mut transcript,
                "redraw after Enter",
                readiness_timeout,
                |bytes| compact_terminal(&bytes[resumed_at..]).contains("j/kmoveenterthread"),
            );
        }
        None => {
            await_terminal(
                &mut child,
                &receiver,
                &mut transcript,
                "successful opener outcome",
                Duration::from_secs(7),
                |bytes| {
                    compact_terminal(&bytes[action_at..])
                        .contains("Browseropenerexitedsuccessfully")
                },
            );
        }
    }
    let outcome_after = action_started.elapsed();
    input.write_all(b"q").expect("quit after the action");
    drop(input);
    let status = match child
        .wait_timeout(Duration::from_secs(3))
        .expect("wait for the TUI")
    {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "TUI did not quit: {:?}",
                String::from_utf8_lossy(&transcript)
            );
        }
    };
    reader.join().expect("finish capture");
    for bytes in receiver.try_iter() {
        transcript.extend(bytes);
    }
    let output = Output {
        status,
        stdout: transcript,
        stderr: errors.join().expect("finish stderr"),
    };
    let compact = compact_terminal(&output.stdout);
    TuiRun {
        output,
        compact,
        waited_for_enter,
        opener_alive_before_enter,
        outcome_after,
    }
}

fn compact_terminal(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut plain = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for escaped in chars.by_ref() {
                if ('@'..='~').contains(&escaped) {
                    break;
                }
            }
        } else if !ch.is_control() {
            plain.push(ch);
        }
    }
    plain.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn contains(run: &TuiRun, expected: &str) -> bool {
    run.compact.contains(
        &expected
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>(),
    )
}

fn opener(world: &World, name: &str, body: &str) {
    world.stub(name, body);
}

fn read_or_empty(world: &World, name: &str) -> String {
    fs::read_to_string(world.path().join(name)).unwrap_or_default()
}

fn process_alive(pid: &str) -> bool {
    // An orphan may await init's reaping after SIGKILL. A zombie has stopped
    // executing and owns no capture pipes (§FS-016-browser-opening.2).
    #[cfg(target_os = "linux")]
    if fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(") ")
                .map(|(_, tail)| tail.starts_with("Z "))
        })
        == Some(true)
    {
        return false;
    }
    Command::new("/bin/kill")
        .args(["-0", pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The configuration forms are deliberately parsed through today's public
/// configuration type, so this fails on the missing contract rather than on a
/// future adapter symbol (§FS-016-browser-opening.1).
#[test]
fn browser_configuration_is_strict() {
    let valid = [
        json!("xdg-open"),
        json!({ "open": "my-opener {url}" }),
        json!(false),
    ];
    let mut failures = Vec::new();
    for value in valid {
        let document = json!({ "defaults": { "browser": value } });
        if let Err(error) = serde_json::from_value::<StatusConfig>(document) {
            failures.push(format!("valid browser binding was refused: {error}"));
        }
    }
    assert!(
        serde_json::from_value::<StatusConfig>(json!({ "defaults": {} })).is_ok(),
        "absence remains automatic"
    );

    for (value, clue) in [
        (json!("other-opener"), "other-opener"),
        (json!({ "open": "my-opener" }), "{url}"),
        (json!({ "open": "my-opener {url}", "extra": true }), "extra"),
        (json!(17), "browser"),
    ] {
        let error =
            serde_json::from_value::<StatusConfig>(json!({ "defaults": { "browser": value } }))
                .expect_err("an invalid browser binding is a configuration error")
                .to_string();
        if error.contains("unknown field `browser`") || !error.contains(clue) {
            failures.push(format!(
                "invalid binding was refused for the wrong reason: {error}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Automatic selection uses trimmed display and SSH signals. Inherited display
/// never makes the remote machine the reader's screen (§FS-016-browser-opening.2).
#[test]
fn automatic_browser_selection_obeys_trimmed_ssh_and_display_signals() {
    let cases = [
        ("local", vec![("DISPLAY", " :1 ")], true, None),
        (
            "remote",
            vec![("DISPLAY", ":1"), ("SSH_CONNECTION", " reader ")],
            false,
            Some("Browser opener bypassed: SSH session"),
        ),
        (
            "blank-display",
            vec![("DISPLAY", "   ")],
            false,
            Some("Browser opener bypassed: no graphical display"),
        ),
        (
            "blank-ssh",
            vec![("DISPLAY", ":1"), ("SSH_TTY", "  ")],
            true,
            None,
        ),
    ];
    let mut failures = Vec::new();
    for (name, environment, invoked, reason) in cases {
        let world = remote_world(URL, None, None);
        opener(
            &world,
            "xdg-open",
            "#!/bin/sh\nprintf '%s\\n' \"$1\" > \"$HOME/browser.log\"\n",
        );
        let run = tui(
            &world,
            b"jo",
            &environment,
            reason.map(|_| Duration::from_millis(800)),
        );
        let called = read_or_empty(&world, "browser.log");
        if called.is_empty() == invoked {
            failures.push(format!("{name}: invocation was {called:?}"));
        }
        if invoked && !contains(&run, "Browser opener exited successfully") {
            failures.push(format!("{name}: no truthful success in {}", run.compact));
        }
        if let Some(reason) = reason {
            if !contains(&run, reason) || !contains(&run, URL) || !run.waited_for_enter {
                failures.push(format!("{name}: fallback was {}", run.compact));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Both routes to a matter URL use the floor. A narrow terminal may wrap the
/// line, but its transcript retains every byte and the TUI resumes only after
/// Enter (§FS-016-browser-opening.1, §FS-016-browser-opening.2).
#[test]
fn both_browser_entry_paths_keep_a_long_remote_url_and_wait_for_enter() {
    let long_url = format!(
        "https://reader.example/{}?token=complete",
        "segment-".repeat(18)
    );
    for keys in [b"jo".as_slice(), b"j\n".as_slice()] {
        let world = remote_world(&long_url, None, None);
        let run = tui(
            &world,
            keys,
            &[("SSH_CLIENT", "reader"), ("WAYLAND_DISPLAY", "wayland-0")],
            Some(Duration::from_millis(800)),
        );
        assert!(run.output.status.success(), "{:?}", run.output);
        assert!(
            contains(&run, "Browser opener bypassed: SSH session"),
            "{}",
            run.compact
        );
        assert!(
            contains(&run, &long_url),
            "complete URL missing from {}",
            run.compact
        );
        let raw = String::from_utf8_lossy(&run.output.stdout);
        assert!(
            raw.contains(&format!("\r\n{long_url}\r\n")),
            "the notice must retain the exact URL on its own line: {raw:?}"
        );
        assert!(run.waited_for_enter, "the fallback did not wait for Enter");
        assert!(
            !contains(&run, "Opened"),
            "false success in {}",
            run.compact
        );
    }
}

/// Start failure, child failure, exit zero, and timeout are four different
/// observed outcomes. Process creation alone proves none of them
/// (§FS-016-browser-opening.2).
#[test]
fn opener_outcomes_are_truthful_and_timeout_is_cleaned_up() {
    let mut failures = Vec::new();

    let missing = remote_world(URL, None, None);
    let run = tui(
        &missing,
        b"jo",
        &[("DISPLAY", ":1")],
        Some(Duration::from_millis(800)),
    );
    if !contains(&run, "Browser opener could not start")
        || !contains(&run, URL)
        || !run.waited_for_enter
    {
        failures.push(format!("missing opener: {}", run.compact));
    }

    let failed = remote_world(URL, None, None);
    opener(
        &failed,
        "xdg-open",
        "#!/bin/sh\nprintf 'invoked=%s\\n' \"$1\" > \"$HOME/browser.log\"\nexit 42\n",
    );
    let run = tui(
        &failed,
        b"jo",
        &[("DISPLAY", ":1")],
        Some(Duration::from_millis(800)),
    );
    if !read_or_empty(&failed, "browser.log").contains(URL)
        || !contains(&run, "Browser opener failed (42)")
        || !contains(&run, URL)
        || contains(&run, "Browser opener exited successfully")
        || contains(&run, "Opened")
        || !run.waited_for_enter
    {
        failures.push(format!("nonzero opener: {}", run.compact));
    }

    let succeeded = remote_world(URL, None, None);
    opener(&succeeded, "xdg-open", "#!/bin/sh\nexit 0\n");
    let run = tui(&succeeded, b"jo", &[("DISPLAY", ":1")], None);
    if !contains(&run, "Browser opener exited successfully") || contains(&run, "Opened") {
        failures.push(format!("zero opener: {}", run.compact));
    }

    let timed = remote_world(URL, None, None);
    opener(
        &timed,
        "xdg-open",
        "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"$HOME/opener.pid\"\n/bin/sleep 30\n",
    );
    let run = tui(
        &timed,
        b"jo",
        &[("DISPLAY", ":1")],
        Some(Duration::from_millis(5_600)),
    );
    let pid = read_or_empty(&timed, "opener.pid").trim().to_string();
    let alive = !pid.is_empty() && process_alive(&pid);
    if !contains(&run, "Browser opener did not finish within 5 seconds")
        || !contains(&run, URL)
        || run.opener_alive_before_enter != Some(false)
        || alive
        || !run.waited_for_enter
    {
        failures.push(format!(
            "timed opener (alive before Enter={:?}, alive after={alive}): {}",
            run.opener_alive_before_enter, run.compact
        ));
    }
    if alive {
        let _ = Command::new("/bin/kill").args(["-TERM", &pid]).status();
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A shell's successful exit is not completion when its descendant still
/// owns the output pipes. Cleanup precedes the Enter handoff
/// (§FS-016-browser-opening.2).
#[test]
fn exited_shell_with_inherited_pipes_times_out_before_enter() {
    let world = remote_world(
        URL,
        Some(json!({ "open":
            "printf '%s\\n' \"$$\" > \"$HOME/shell.pid\"; /bin/sleep 30 & printf '%s\\n' \"$!\" > \"$HOME/opener.pid\"; exit 0 # {url}"
        })),
        None,
    );
    let run = tui(
        &world,
        b"jo",
        &[("SSH_CLIENT", "reader")],
        Some(Duration::from_secs(7)),
    );
    let pid = read_or_empty(&world, "opener.pid").trim().to_string();
    let shell = read_or_empty(&world, "shell.pid").trim().to_string();
    let alive = !pid.is_empty() && process_alive(&pid);
    if alive {
        let _ = Command::new("/bin/kill").args(["-KILL", &pid]).status();
    }
    assert!(run.output.status.success(), "{:?}", run.output);
    assert!(
        !shell.is_empty() && !process_alive(&shell),
        "direct shell did not exit"
    );
    assert!(!pid.is_empty(), "the descendant never started");
    assert!(
        contains(&run, "Browser opener did not finish within 5 seconds"),
        "{}",
        run.compact
    );
    assert!(contains(&run, URL), "{}", run.compact);
    assert_eq!(
        run.opener_alive_before_enter,
        Some(false),
        "descendant still ran before Enter"
    );
    assert!(!alive, "descendant still ran after Enter");
    assert!(run.waited_for_enter);
    assert!(
        run.outcome_after < Duration::from_secs(7),
        "unbounded capture: {:?}",
        run.outcome_after
    );
}

/// Codes reserved by the shell are also codes a started opener can return;
/// exec failure is separate evidence (§FS-016-browser-opening.2).
#[test]
fn started_openers_keep_exit_126_and_127() {
    let mut failures = Vec::new();
    for code in [126, 127] {
        for custom in [false, true] {
            let browser = custom.then(|| {
                json!({ "open": format!(
                "sh -c 'printf started > \"$HOME/browser.log\"; exit {code}' ignored {{url}}"
            ) })
            });
            let world = remote_world(URL, browser, None);
            opener(
                &world,
                "xdg-open",
                &format!("#!/bin/sh\nprintf started > \"$HOME/browser.log\"\nexit {code}\n"),
            );
            let run = tui(
                &world,
                b"jo",
                &[("DISPLAY", ":1")],
                Some(Duration::from_secs(2)),
            );
            assert!(run.output.status.success(), "{:?}", run.output);
            assert_eq!(read_or_empty(&world, "browser.log"), "started");
            if !contains(&run, &format!("Browser opener failed ({code})"))
                || contains(&run, "Browser opener could not start")
            {
                failures.push(format!("exit={code}, custom={custom}: {}", run.compact));
            }
            assert!(contains(&run, URL));
            assert!(run.waited_for_enter);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A configured binding is deliberate even over SSH, and `{url}` is one
/// shell-quoted argument rather than source code (§FS-016-browser-opening.1).
#[test]
fn explicit_browser_under_ssh_quotes_the_url_as_one_argument() {
    let marker_name = "url-was-executed";
    let url = format!(r#"https://reader.example/path?value='; touch "$HOME/{marker_name}"; #"#);
    let world = remote_world(&url, Some(json!({ "open": "custom-open {url}" })), None);
    let marker = world.path().join(marker_name);
    opener(
        &world,
        "custom-open",
        "#!/bin/sh\nprintf 'argc=%s\\narg=%s\\n' \"$#\" \"$1\" > \"$HOME/custom.log\"\n",
    );
    let run = tui(
        &world,
        b"jo",
        &[("SSH_CONNECTION", "reader"), ("DISPLAY", ":1")],
        None,
    );
    let log = read_or_empty(&world, "custom.log");
    assert!(run.output.status.success(), "{:?}", run.output);
    assert!(log.contains("argc=1"), "{log}");
    assert!(log.contains(&format!("arg={url}")), "{log}");
    assert!(!marker.exists(), "URL contents executed as shell source");
    assert!(
        contains(&run, "Browser opener exited successfully"),
        "{}",
        run.compact
    );
}

/// `false` is an explicit binding to the same copyable floor, independent of
/// an otherwise eligible display (§FS-016-browser-opening.1).
#[test]
fn browser_false_selects_the_copyable_floor() {
    let world = remote_world(URL, Some(json!(false)), None);
    opener(
        &world,
        "xdg-open",
        "#!/bin/sh\nprintf called > \"$HOME/browser.log\"\n",
    );
    let run = tui(
        &world,
        b"jo",
        &[("DISPLAY", ":1")],
        Some(Duration::from_millis(800)),
    );
    assert!(read_or_empty(&world, "browser.log").is_empty());
    assert!(
        contains(&run, "Browser opener disabled by configuration"),
        "{}",
        run.compact
    );
    assert!(contains(&run, URL), "{}", run.compact);
    assert!(run.waited_for_enter);
}

fn terminal_stub(world: &World, name: &str, handle: &str) {
    let body = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$HOME/{name}.log\"\nprintf '%s\\n' '{handle}'\n"
    );
    opener(world, name, &body);
}

fn action(world: &World, id: &str, environment: &[(&str, &str)]) -> Output {
    let binary = assert_cmd::cargo::cargo_bin("ephor");
    let mut command = Command::new(binary);
    command.args(["actions", "run", "--item", ITEM, id, "--json"]);
    isolated(&mut command, world, environment);
    command.output().expect("run the action")
}

/// SSH suppresses automatic GUI windows with a reason, but not tmux. Local
/// marker order and trimmed-empty markers remain controls (§FS-005-dispatch.22).
#[test]
fn automatic_window_selection_is_ssh_aware_and_tmux_remains_remote() {
    let mut failures = Vec::new();
    for (name, marker) in [("wezterm", "WEZTERM_PANE"), ("kitty", "KITTY_WINDOW_ID")] {
        let world = remote_world(URL, None, None);
        terminal_stub(&world, name, "@gui");
        let output = action(
            &world,
            "windowed",
            &[
                ("SSH_TTY", "/dev/pts/7"),
                ("DISPLAY", ":1"),
                (marker, "inherited"),
            ],
        );
        let text = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);
        if !read_or_empty(&world, &format!("{name}.log")).is_empty()
            || !text.contains("SSH permits automatic tmux but not an automatic GUI window")
            || !text.contains("window-action-ran")
        {
            failures.push(format!("remote {name}: {text}"));
        }
    }

    let tmux = remote_world(URL, None, None);
    terminal_stub(&tmux, "tmux", "@remote");
    let output = action(
        &tmux,
        "windowed",
        &[("SSH_CLIENT", "reader"), ("TMUX", "session")],
    );
    if !output.status.success() || !read_or_empty(&tmux, "tmux.log").contains("new-window") {
        failures.push(format!("remote tmux: {:?}", output));
    }

    let local = remote_world(URL, None, None);
    for (name, handle) in [("tmux", "@tmux"), ("wezterm", "@wez"), ("kitty", "@kitty")] {
        terminal_stub(&local, name, handle);
    }
    let _ = action(
        &local,
        "windowed",
        &[
            ("DISPLAY", ":1"),
            ("TMUX", "t"),
            ("WEZTERM_PANE", "w"),
            ("KITTY_WINDOW_ID", "k"),
        ],
    );
    if read_or_empty(&local, "tmux.log").is_empty()
        || !read_or_empty(&local, "wezterm.log").is_empty()
        || !read_or_empty(&local, "kitty.log").is_empty()
    {
        failures.push("local marker order did not prefer tmux".to_string());
    }

    let blank = remote_world(URL, None, None);
    terminal_stub(&blank, "wezterm", "@wez");
    let output = action(
        &blank,
        "windowed",
        &[("WEZTERM_PANE", "   "), ("DISPLAY", ":1")],
    );
    let text = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    if !read_or_empty(&blank, "wezterm.log").is_empty() || !text.contains("window-action-ran") {
        failures.push(format!("blank marker/display-only control: {text}"));
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Explicit GUI configuration remains authoritative under SSH, while an
/// attached action still inherits the SSH pty (§FS-005-dispatch.22).
#[test]
fn explicit_gui_window_and_attached_terminal_controls_remain_available() {
    let world = remote_world(URL, None, Some(json!("wezterm")));
    terminal_stub(&world, "wezterm", "@explicit");
    let output = action(&world, "windowed", &[("SSH_CONNECTION", "reader")]);
    assert!(output.status.success(), "{:?}", output);
    assert!(read_or_empty(&world, "wezterm.log").contains("spawn"));

    let binary = assert_cmd::cargo::cargo_bin("ephor");
    let script_command = format!(
        "{} actions run --item {} attached",
        ephor::seams::summons::quote(&binary.to_string_lossy()),
        ITEM
    );
    let mut command = Command::new("/usr/bin/script");
    command.args(["-qefc", &script_command, "/dev/null"]);
    isolated(&mut command, &world, &[("SSH_TTY", "/dev/pts/7")]);
    let output = command.output().expect("run attached action in a pty");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("attached-terminal-ok"), "{text}");
}
