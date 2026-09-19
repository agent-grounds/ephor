//! The reader's browser: one bound opener or a terminal-visible address
//! (§FS-016-browser-opening).
//!
//! Browser opening is presentation over an address the feed already carries.
//! This module owns the external product, configured command, automatic
//! reader-environment decision, bounded invocation, and truthful outcomes.
//! The TUI owns only the terminal it must release to show the floor.

use std::time::Duration;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::seams::summons::{self, ExecutionError, Site, Summons};

const PRODUCT: &str = "xdg-open";
const URL: &str = "{url}";
const OPEN_VERB: &str = "browser.open";
const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

const SSH_MARKERS: &[&str] = &["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"];
const DISPLAY_MARKERS: &[&str] = &["DISPLAY", "WAYLAND_DISPLAY"];

/// Which browser binding fills the seam: ephor's shipped binding, one command
/// of the reader's own, or the explicit terminal floor
/// (§FS-016-browser-opening.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    Named,
    Command { open: String },
    Disabled,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandBinding {
    open: String,
}

/// Parse the three browser forms strictly where site configuration is read;
/// invalid configuration never becomes an automatic guess
/// (§FS-016-browser-opening.1).
impl<'de> Deserialize<'de> for Binding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(name) if name == PRODUCT => Ok(Binding::Named),
            serde_json::Value::String(name) => Err(D::Error::custom(format!(
                "unknown browser binding '{name}'; the shipped binding is '{PRODUCT}'"
            ))),
            serde_json::Value::Object(_) => {
                let command: CommandBinding =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                if !command.open.contains(URL) {
                    return Err(D::Error::custom(format!(
                        "the configured browser's 'open' has no {URL} in it"
                    )));
                }
                Ok(Binding::Command { open: command.open })
            }
            serde_json::Value::Bool(false) => Ok(Binding::Disabled),
            _ => Err(D::Error::custom(
                "defaults.browser must be 'xdg-open', an object containing only 'open', or false",
            )),
        }
    }
}

/// The two reader-environment facts shared by automatic browser and window
/// resolution (§FS-016-browser-opening.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReaderEnvironment {
    pub(crate) ssh: bool,
    pub(crate) graphical: bool,
}

impl ReaderEnvironment {
    pub(crate) fn current() -> Self {
        Self::from_values(|name| std::env::var(name).ok())
    }

    fn from_values(mut value: impl FnMut(&str) -> Option<String>) -> Self {
        let present = |value: Option<String>| value.is_some_and(|value| !value.trim().is_empty());
        Self {
            ssh: SSH_MARKERS.iter().any(|name| present(value(name))),
            graphical: DISPLAY_MARKERS.iter().any(|name| present(value(name))),
        }
    }
}

/// Why browser opening deliberately stays in the reader's terminal
/// (§FS-016-browser-opening.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Floor {
    Ssh,
    Headless,
    Disabled,
}

impl Floor {
    pub fn reason(self) -> &'static str {
        match self {
            Floor::Ssh => "Browser opener bypassed: SSH session",
            Floor::Headless => "Browser opener bypassed: no graphical display",
            Floor::Disabled => "Browser opener disabled by configuration",
        }
    }
}

/// The result of browser binding resolution (§FS-016-browser-opening.1,
/// §FS-016-browser-opening.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Open(Opener),
    Floor(Floor),
}

/// One resolved browser command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opener {
    open: String,
}

/// Resolve the configured choice first, then the automatic reader environment;
/// an explicit choice is authoritative even under SSH or without a display
/// (§FS-016-browser-opening.1).
pub fn bound(configured: Option<&Binding>) -> Resolution {
    bound_in(configured, ReaderEnvironment::current())
}

fn bound_in(configured: Option<&Binding>, environment: ReaderEnvironment) -> Resolution {
    match configured {
        Some(Binding::Named) => Resolution::Open(shipped()),
        Some(Binding::Command { open }) => Resolution::Open(Opener { open: open.clone() }),
        Some(Binding::Disabled) => Resolution::Floor(Floor::Disabled),
        None if environment.ssh => Resolution::Floor(Floor::Ssh),
        None if !environment.graphical => Resolution::Floor(Floor::Headless),
        None => Resolution::Open(shipped()),
    }
}

fn shipped() -> Opener {
    Opener {
        open: format!("{PRODUCT} {URL}"),
    }
}

/// What the bounded opener actually established (§FS-016-browser-opening.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    ExitedSuccessfully,
    CouldNotStart,
    Failed(i32),
    TimedOut,
    ExecutionFailed(String),
}

impl Outcome {
    pub fn message(&self) -> String {
        match self {
            Outcome::ExitedSuccessfully => "Browser opener exited successfully".to_string(),
            Outcome::CouldNotStart => "Browser opener could not start".to_string(),
            Outcome::Failed(code) => format!("Browser opener failed ({code})"),
            Outcome::TimedOut => "Browser opener did not finish within 5 seconds".to_string(),
            Outcome::ExecutionFailed(detail) => {
                format!("Browser opener execution failed: {detail}")
            }
        }
    }

    pub fn succeeded(&self) -> bool {
        *self == Outcome::ExitedSuccessfully
    }
}

impl Opener {
    /// Substitute the complete address as one shell word
    /// (§FS-016-browser-opening.1).
    pub fn command(&self, url: &str) -> String {
        self.open.replace(URL, &summons::quote(url))
    }

    /// Run the opener through the shared executor, stop it at five seconds,
    /// and report only what its exit established
    /// (§FS-016-browser-opening.2, §AR-002-summons.2).
    pub fn open(&self, url: &str, site: &Site) -> Outcome {
        let result = summons::run_captured_shell(
            &Summons::new(OPEN_VERB, self.command(url)),
            site,
            OPEN_TIMEOUT,
        );
        match result {
            Ok(answer) if answer.is_done() => Outcome::ExitedSuccessfully,
            Ok(answer) => outcome_from_exit(answer.exit_code),
            Err(ExecutionError::TimedOut(_)) => Outcome::TimedOut,
            Err(ExecutionError::BeforeSpawn(_)) => Outcome::CouldNotStart,
            Err(ExecutionError::AfterSpawn(error)) => Outcome::ExecutionFailed(error.to_string()),
        }
    }
}

fn outcome_from_exit(code: Option<i32>) -> Outcome {
    match code {
        // These are the shell invocation's exits, with no downstream start
        // inference from reserved codes (§FS-016-browser-opening.2).
        Some(code) => Outcome::Failed(code),
        None => Outcome::ExecutionFailed("invocation ended without an exit code".to_string()),
    }
}

/// The terminal notice, kept as plain unmodified text so a long address stays
/// in scrollback on a line of its own (§FS-016-browser-opening.2).
pub fn notice(reason: &str, url: &str) -> String {
    format!("{reason}\n{url}\n")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn environment(values: &[(&str, &str)]) -> ReaderEnvironment {
        let values: BTreeMap<&str, &str> = values.iter().copied().collect();
        ReaderEnvironment::from_values(|name| values.get(name).map(|value| (*value).to_string()))
    }

    /// Every SSH and display marker independently obeys the same trimmed
    /// predicate, and SSH always wins (§FS-016-browser-opening.2).
    #[test]
    fn reader_environment_has_one_complete_truth_table() {
        assert_eq!(
            environment(&[]),
            ReaderEnvironment {
                ssh: false,
                graphical: false
            }
        );
        for marker in SSH_MARKERS {
            assert!(environment(&[(marker, " reader ")]).ssh, "{marker}");
            assert!(!environment(&[(marker, "")]).ssh, "empty {marker}");
            assert!(!environment(&[(marker, "   ")]).ssh, "blank {marker}");
        }
        for marker in DISPLAY_MARKERS {
            assert!(environment(&[(marker, " display ")]).graphical, "{marker}");
            assert!(!environment(&[(marker, "")]).graphical, "empty {marker}");
            assert!(!environment(&[(marker, "   ")]).graphical, "blank {marker}");
        }
        let remote_display = environment(&[("SSH_CLIENT", "reader"), ("DISPLAY", ":1")]);
        assert_eq!(
            bound_in(None, remote_display),
            Resolution::Floor(Floor::Ssh)
        );
    }

    /// The three accepted forms are exact, and invalid forms are refused at
    /// deserialization rather than weakened into automation
    /// (§FS-016-browser-opening.1).
    #[test]
    fn browser_binding_parsing_is_strict() {
        for valid in [
            serde_json::json!("xdg-open"),
            serde_json::json!({ "open": "mine {url}" }),
            serde_json::json!(false),
        ] {
            assert!(serde_json::from_value::<Binding>(valid).is_ok());
        }
        for invalid in [
            serde_json::json!("mine"),
            serde_json::json!({ "open": "mine" }),
            serde_json::json!({ "open": "mine {url}", "extra": true }),
            serde_json::json!(true),
            serde_json::json!(17),
            serde_json::Value::Null,
        ] {
            assert!(serde_json::from_value::<Binding>(invalid).is_err());
        }
    }

    /// Explicit choices outrank SSH and display signals, while absence uses
    /// those signals and `false` always selects the floor
    /// (§FS-016-browser-opening.1, §FS-016-browser-opening.2).
    #[test]
    fn explicit_browser_choices_have_precedence() {
        let remote = environment(&[("SSH_TTY", "/dev/pts/1")]);
        let headless = environment(&[]);
        let custom = Binding::Command {
            open: "mine {url}".to_string(),
        };
        for environment in [remote, headless] {
            assert!(matches!(
                bound_in(Some(&Binding::Named), environment),
                Resolution::Open(_)
            ));
            assert!(matches!(
                bound_in(Some(&custom), environment),
                Resolution::Open(_)
            ));
        }
        assert_eq!(
            bound_in(Some(&Binding::Disabled), environment(&[("DISPLAY", ":1")])),
            Resolution::Floor(Floor::Disabled)
        );
        assert_eq!(bound_in(None, remote), Resolution::Floor(Floor::Ssh));
        assert_eq!(bound_in(None, headless), Resolution::Floor(Floor::Headless));
    }

    /// Substitution is one quoted shell argument, and observed outcomes keep
    /// distinct reader-facing words (§FS-016-browser-opening.1, §FS-016-browser-opening.2).
    #[test]
    fn substitution_and_outcome_mapping_are_exact() {
        let opener = Opener {
            open: "mine --url {url}".to_string(),
        };
        assert_eq!(
            opener.command("https://example.test/a b?x='; echo no"),
            "mine --url 'https://example.test/a b?x='\\''; echo no'"
        );
        assert_eq!(outcome_from_exit(Some(42)), Outcome::Failed(42));
        assert_eq!(outcome_from_exit(Some(126)), Outcome::Failed(126));
        assert_eq!(outcome_from_exit(Some(127)), Outcome::Failed(127));
        assert_eq!(
            Outcome::ExitedSuccessfully.message(),
            "Browser opener exited successfully"
        );
        assert_eq!(
            Outcome::TimedOut.message(),
            "Browser opener did not finish within 5 seconds"
        );
        assert_eq!(
            Outcome::CouldNotStart.message(),
            "Browser opener could not start"
        );
        assert_eq!(Outcome::Failed(42).message(), "Browser opener failed (42)");
    }

    /// The shell's observed result is stable for simple, compound and waited
    /// background commands, including codes also used for downstream exec
    /// failure (§FS-016-browser-opening.2, §AR-002-summons.2).
    #[cfg(unix)]
    #[test]
    fn shell_forms_preserve_exit_codes_and_invocation_evidence() {
        let tmp = tempfile::tempdir().unwrap();
        let site = Site::root(tmp.path());
        for code in [0, 42, 126, 127] {
            for form in [0, 1, 2] {
                let command =
                    format!("sh -c 'printf started > marker; exit {code}' ignored {{url}}");
                let open = match form {
                    0 => command,
                    1 => format!("{{ {command}; }}"),
                    _ => format!("{command} & wait $!"),
                };
                let marker = tmp.path().join("marker");
                let _ = std::fs::remove_file(&marker);
                let outcome = Opener { open }.open("https://example.test/a b?quote='", &site);
                let expected = if code == 0 {
                    Outcome::ExitedSuccessfully
                } else {
                    Outcome::Failed(code)
                };
                assert_eq!(outcome, expected, "code={code}, form={form}");
                assert_eq!(std::fs::read_to_string(&marker).unwrap(), "started");
            }
        }
        // Even an absolute missing first word belongs to the shell, and a
        // compound may deliberately recover from it (§FS-016-browser-opening.2).
        let missing = summons::quote(&tmp.path().join("missing").to_string_lossy());
        assert_eq!(
            Opener {
                open: format!("{missing} {{url}}")
            }
            .open("url", &site),
            Outcome::Failed(127)
        );
        assert_eq!(
            Opener {
                open: format!("{missing} {{url}}; exit 0")
            }
            .open("url", &site),
            Outcome::ExitedSuccessfully
        );
    }

    /// Preparation and OS spawn failures are positively known before execution;
    /// signal, capture and answer errors after spawn must not claim otherwise
    /// (§FS-016-browser-opening.2).
    #[cfg(unix)]
    #[test]
    fn invocation_errors_keep_their_actual_phase() {
        let tmp = tempfile::tempdir().unwrap();
        let site = Site::root(tmp.path());
        let ordinary = Opener {
            open: "printf started > marker # {url}".into(),
        };
        assert_eq!(
            ordinary.open("url", &Site::root(tmp.path().join("absent"))),
            Outcome::CouldNotStart
        );
        assert!(!tmp.path().join("marker").exists());
        // NUL in an OS argument rejects the invocation at Command::spawn.
        let invalid = Opener {
            open: "printf started > marker; \0 # {url}".into(),
        };
        assert_eq!(invalid.open("url", &site), Outcome::CouldNotStart);
        assert!(!tmp.path().join("marker").exists());
        for (script, diagnostic) in [
            ("kill -TERM $$", "without an exit code"),
            ("printf '\\377'", "capture failed"),
            ("printf '{' > \"$EPHOR_ANSWER\"", "answer"),
        ] {
            let marker = tmp.path().join("marker");
            let _ = std::fs::remove_file(&marker);
            let opener = Opener {
                open: format!("printf started > marker; {script} # {{url}}"),
            };
            let outcome = opener.open("url", &site);
            assert_eq!(std::fs::read_to_string(&marker).unwrap(), "started");
            assert!(
                matches!(&outcome, Outcome::ExecutionFailed(detail) if detail.contains(diagnostic)),
                "{outcome:?}"
            );
            assert!(outcome
                .message()
                .starts_with("Browser opener execution failed:"));
        }
    }

    /// The notice preserves its address byte for byte and gives it a line of
    /// its own, including spaces and shell punctuation
    /// (§FS-016-browser-opening.2).
    #[test]
    fn notice_preserves_the_complete_address_on_its_own_line() {
        let url = "https://example.test/a b?quote='&value=$HOME";
        assert_eq!(
            notice("Browser opener failed (42)", url),
            format!("Browser opener failed (42)\n{url}\n")
        );
    }
}
