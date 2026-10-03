//! A fence a text cannot close from inside (§FS-011-command-line.11.1.1).
//!
//! Text ephor quotes whole — what git printed, a forge message — may carry
//! fences of its own. The plan language closes a fence only on a bare run of
//! the same character at least as long as the one that opened it, so a longer
//! fence holds shorter ones (§FS-005-dispatch.3.2): one backtick longer than
//! the longest run the text holds, and never shorter than three.

/// The backtick run that fences `text` so nothing inside it closes the fence
/// (§FS-011-command-line.11.1.1, §FS-005-dispatch.3.2).
pub fn fence_for(text: &str) -> String {
    let longest = text.split(|ch| ch != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fence_is_longer_than_any_run_inside_and_never_shorter_than_three() {
        assert_eq!(fence_for("plain"), "```");
        assert_eq!(fence_for("a `tick` and ``two``"), "```");
        assert_eq!(fence_for("```\ncode\n```"), "````");
        assert_eq!(fence_for("````markdown\n```text\n```\n````"), "`````");
    }
}
