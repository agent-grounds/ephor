//! E2E-044-git-words-quoted-whole: a refusal whose words carry fences of
//! their own reaches the report still inside the report's fence.
//!
//! The scenario is §FS-011-command-line.11.1.1 end to end. When git refuses a
//! replay or a checkout, the report quotes what git said, and what git said is
//! whatever its hooks printed. A hook guarding a plan prints a piece of that
//! plan back — a four-backtick block holding a three-backtick pair and a
//! heading after it. Fenced in a fixed run of three, the message's own bare
//! three-backtick line closed the report's fence: the heading stood outside it
//! as one of the report's own sections, and the four-backtick line opened a
//! fence the report's closing line was too short to close.
//!
//! What this case holds ephor to, for both reports that quote git — the
//! replay's (`ephor rebase`) and the checkout's (`ephor checkout`): read by the
//! plan language's own fence rule (§FS-005-dispatch.3.2), every line of git's
//! message lies inside the report's fence, and the report ends with no fence
//! open — in the `--report` file and in the `report` field of `--json` alike.

#[path = "../support.rs"]
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;

use support::*;

/// What the hook prints and exits 1 on: its words are git's message for the
/// refusal. The shape is the one agent-grounds/ephor#164 reports — a longer
/// fence holding a shorter pair, and a heading after the pair.
const HOOK_SAYS: &str = "\
hook: this branch must keep the plan skeleton below unchanged:
````markdown
# Rhei: the retry window
```text
an example the skeleton carries
```
### Task 1: after the inner pair
````
hook: restore it, then try again.
";

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

fn commit(dir: &Path, file: &str, contents: &str, message: &str) {
    std::fs::write(dir.join(file), contents).expect("write the file");
    git(dir, &["add", file]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// An origin on `main` with one commit, which every checkout here grows from.
fn origin(world: &World) -> PathBuf {
    let origin = world.path().join("origin");
    std::fs::create_dir_all(&origin).expect("the remote");
    git(&origin, &["init", "-q", "--initial-branch=main"]);
    commit(&origin, "shared.txt", "one\n", "one");
    origin
}

fn clone(origin: &Path, into: &Path) {
    std::fs::create_dir_all(into.parent().expect("a parent")).expect("the parent directory");
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(origin)
        .arg(into)
        .stderr(Stdio::null())
        .status()
        .expect("git clones");
    assert!(status.success());
    git(into, &["config", "user.email", "t@example.com"]);
    git(into, &["config", "user.name", "t"]);
}

/// A hook in `repo` that prints [`HOOK_SAYS`] and refuses.
fn hook(world: &World, repo: &Path, name: &str) {
    // Beside the forest rather than in it: an untracked file in the checkout
    // is not what this case is about.
    let says = world.path().join("hook-says.md");
    std::fs::write(&says, HOOK_SAYS).expect("the hook's words");
    let hooks = repo.join(".git/hooks");
    std::fs::create_dir_all(&hooks).expect("the hooks directory");
    let path = hooks.join(name);
    std::fs::write(
        &path,
        format!(
            "#!/usr/bin/env bash\ncat '{}' >&2\nexit 1\n",
            says.display()
        ),
    )
    .expect("the hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("the hook runs");
    }
}

/// One line's place under the plan language's fence rule: whether it is the
/// content of a fence, and the fence left open once every line is read.
struct Walk {
    inside: Vec<bool>,
    open: Option<(char, usize)>,
}

/// Read `text` by the rule rhei's plan language defines for itself
/// (§FS-005-dispatch.3.2): a fence opens on a line-initial run of three or
/// more backticks or tildes, and closes only on a bare run of the same
/// character at least as long. A line is inside when a fence was open before
/// it and it did not close that fence.
fn walk(text: &str) -> Walk {
    let mut inside = Vec::new();
    let mut open: Option<(char, usize)> = None;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let run = trimmed
            .chars()
            .next()
            .filter(|c| *c == '`' || *c == '~')
            .map(|c| {
                let n = trimmed.chars().take_while(|ch| *ch == c).count();
                (c, n, trimmed[n..].trim().is_empty())
            });
        let run = run.filter(|(_, n, _)| *n >= 3);
        match (open, run) {
            (None, Some((c, n, _))) => {
                open = Some((c, n));
                inside.push(false);
            }
            (Some((c, len)), Some((d, n, bare))) if d == c && n >= len && bare => {
                open = None;
                inside.push(false);
            }
            (Some(_), _) => inside.push(true),
            (None, None) => inside.push(false),
        }
    }
    Walk { inside, open }
}

/// What §FS-011-command-line.11.1.1 asks of a report quoting git: every line
/// of the hook's words is the content of a fence, and nothing is left open.
fn quotes_git_whole(report: &str, what: &str) {
    let lines: Vec<&str> = report.lines().collect();
    let said: Vec<&str> = HOOK_SAYS.lines().collect();
    let start = lines
        .iter()
        .position(|line| *line == said[0])
        .unwrap_or_else(|| panic!("{what} does not carry git's message at all:\n{report}"));
    assert_eq!(
        &lines[start..start + said.len()],
        &said[..],
        "{what} did not carry git's message word for word:\n{report}"
    );
    let walked = walk(report);
    for (offset, line) in said.iter().enumerate() {
        assert!(
            walked.inside[start + offset],
            "{what}: line {}, from git's message, stands outside the report's fence: {line:?}\n{report}",
            start + offset + 1
        );
    }
    if let Some((c, len)) = walked.open {
        panic!("{what} ends inside a fence of {len} {c:?} that nothing closes:\n{report}");
    }
}

/// A clone on its own branch, trailing `main` by one commit it has seen,
/// whose `pre-rebase` hook refuses the replay with [`HOOK_SAYS`].
fn trailing_with_a_refusing_hook(world: &World) -> PathBuf {
    let origin = origin(world);
    let checkout = world.forest();
    std::fs::remove_dir_all(&checkout).expect("the forest is the clone");
    clone(&origin, &checkout);
    git(&checkout, &["checkout", "-q", "-b", "you/ABC-42-work"]);
    commit(&checkout, "mine.txt", "mine\n", "mine");
    commit(&origin, "theirs.txt", "theirs\n", "main moves");
    git(&checkout, &["fetch", "-q", "origin"]);
    hook(world, &checkout, "pre-rebase");
    checkout
}

#[test]
fn a_replay_report_keeps_every_line_git_said_inside_its_fence() {
    let world = World::new();
    let checkout = trailing_with_a_refusing_hook(&world);
    let report = world.path().join("rebase-report.md");

    let out = world
        .ephor()
        .args(["rebase", "--checkout"])
        .arg(&checkout)
        .args(["--onto", "main", "--report"])
        .arg(&report)
        .output()
        .expect("the rebase runs");
    let written = std::fs::read_to_string(&report).unwrap_or_else(|_| {
        panic!(
            "ephor rebase wrote no report:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    // The scenario only means something while the hook is the one refusing.
    assert!(
        written.contains("pre-rebase hook refused"),
        "the replay was not refused by the hook, so there is nothing to judge:\n{written}"
    );
    quotes_git_whole(&written, "the `--report` file of `ephor rebase`");

    // The same report as the `--json` reading carries it (§FS-011-command-line.7).
    let out = world
        .ephor()
        .args(["rebase", "--checkout"])
        .arg(&checkout)
        .args(["--onto", "main", "--json"])
        .output()
        .expect("the rebase runs");
    let reading = json_of(&out);
    let carried = reading["report"]
        .as_str()
        .unwrap_or_else(|| panic!("`ephor rebase --json` carries no report field:\n{reading}"));
    quotes_git_whole(carried, "the `report` field of `ephor rebase --json`");
}

#[test]
fn a_checkout_report_keeps_every_line_git_said_inside_its_fence() {
    let world = World::new();
    let origin = origin(&world);
    let main = world.forest().join("main");
    clone(&origin, &main);
    // `git worktree add` returns the hook's status, so the checkout is refused
    // with the hook's words as git's message.
    hook(&world, &main, "post-checkout");
    world.register(json!({
        "clone_mode": "worktree",
        "branch_root_template": "{project_root}/{branch}"
    }));
    let report = world.path().join("checkout-report.md");

    let out = world
        .ephor()
        .args([
            "checkout",
            "--project",
            PROJECT,
            "--branch",
            "feat/x",
            "--report",
        ])
        .arg(&report)
        .output()
        .expect("the checkout runs");
    let written = std::fs::read_to_string(&report).unwrap_or_else(|_| {
        panic!(
            "ephor checkout wrote no report:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    assert!(
        written.contains("hook: restore it, then try again."),
        "the checkout was not refused with the hook's words, so there is nothing to judge:\n{written}"
    );
    quotes_git_whole(&written, "the `--report` file of `ephor checkout`");
}
