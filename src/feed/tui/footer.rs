//! The footer's lines: what a screen names as its keys, said in full on a
//! screen too narrow for one line (§FS-021-monitors.20.1,
//! §FS-019-waiting.3).
//!
//! A footer is segments set apart by runs of spaces, `j/k move  enter open`,
//! so it breaks between them and never inside one, which keeps a key beside
//! what it does. A segment wider than the screen breaks at its words.

use unicode_width::UnicodeWidthStr;

/// The most lines a footer takes from the body, so a long one on a short
/// terminal still leaves the screen it describes.
pub(super) const MOST: usize = 3;

/// `footer` as lines `width` wide, each but the first one space in, as the
/// first is: one line where it fits, and never more than [`MOST`].
pub(super) fn lines(footer: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    if UnicodeWidthStr::width(footer) <= width {
        return vec![footer.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for segment in segments(footer) {
        let joined = match current.is_empty() {
            true => format!(" {segment}"),
            false => format!("{current}  {segment}"),
        };
        if UnicodeWidthStr::width(joined.as_str()) <= width {
            current = joined;
            continue;
        }
        if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        let mut broken = super::thread::wrap_line(segment, width.saturating_sub(1).max(1));
        current = broken
            .pop()
            .map(|last| format!(" {last}"))
            .unwrap_or_default();
        out.extend(broken.into_iter().map(|line| format!(" {line}")));
    }
    if !current.is_empty() {
        out.push(current);
    }
    out.truncate(MOST);
    out
}

/// The footer's segments: what lies between runs of two or more spaces, or
/// a `·` set off by spaces.
fn segments(footer: &str) -> Vec<&str> {
    footer
        .split("  ")
        .map(|segment| segment.trim().trim_start_matches('·').trim())
        .filter(|segment| !segment.is_empty())
        .collect()
}

#[cfg(test)]
#[path = "footer_tests.rs"]
mod tests;
