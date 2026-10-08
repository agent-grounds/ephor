//! The draft card under a conversation: the reply a run drafted, what moved
//! since it was drafted where it is stale, and the keys that go on from it
//! (§FS-005-dispatch.13, §FS-005-dispatch.13.2).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::{wrap_line, Draft};

/// The draft as a card of its own: marked as unsent, with what the reader can
/// do about it here — post it, or copy it from where it sits
/// (§FS-005-dispatch.13, §REQ-001-boundary.1). A stale draft shows its review
/// and its two ways on instead, `types` saying whether there is a thread the
/// person's own words can go to (§FS-005-dispatch.13.2).
pub(super) fn draft_lines(
    draft: &Draft,
    types: bool,
    wrap_width: usize,
    lines: &mut Vec<Line<'static>>,
) {
    let color = if draft.posted {
        Color::Green
    } else {
        Color::Magenta
    };
    let gutter = || Span::styled("▍ ", Style::default().fg(color));
    let banner = match (draft.posted, draft.can_post, draft.stale) {
        (true, _, _) => "posted".to_string(),
        (false, true, _) => "proposed reply — not posted".to_string(),
        (false, false, true) => "proposed reply — stale, not posted".to_string(),
        // A channel that declared no reply gets the honest half-offer: the
        // words are here, sending them is somewhere else.
        (false, false, false) => "proposed reply — copy or edit; posting unavailable".to_string(),
    };
    lines.push(Line::from(vec![
        gutter(),
        Span::styled(
            banner,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]));
    for text_line in draft.text.lines() {
        for wrapped in wrap_line(text_line, wrap_width) {
            lines.push(Line::from(vec![gutter(), Span::raw(wrapped)]));
        }
    }
    if let Some(target) = &draft.bound_target {
        lines.push(Line::from(format!(
            "Bound thread {} target: {target}",
            draft.thread
        )));
    }
    if let Some(reason) = &draft.stale_reason {
        for line in wrap_line(reason, wrap_width) {
            lines.push(Line::from(line));
        }
    }
    if !draft.since.is_empty() {
        review_lines(draft, wrap_width, lines);
    }
    if !draft.posted {
        let hint = |text: String| {
            Line::from(vec![
                gutter(),
                Span::styled(text, Style::default().fg(Color::DarkGray)),
            ])
        };
        match (draft.stale, draft.can_post) {
            (true, _) => {
                if let Some(redraft) = &draft.redraft {
                    lines.push(hint(format!("n drafts it again · {}", redraft.command)));
                }
                if let Some(why) = &draft.redraft_refused {
                    for line in wrap_line(why, wrap_width) {
                        lines.push(hint(line));
                    }
                }
                lines.push(hint(match types {
                    true => format!(
                        "r types your own · e starts from these words · they stay at {}",
                        draft.path.display()
                    ),
                    false => format!("copy it from {}", draft.path.display()),
                }));
            }
            (false, true) => lines.push(hint(format!(
                "p posts it · e edits it first · {}",
                draft.path.display()
            ))),
            (false, false) => lines.push(hint(format!(
                "e edits it · copy it from {}",
                draft.path.display()
            ))),
        }
    }
    lines.push(Line::default());
}

/// What moved in the bound thread since the draft, one line per change, an
/// edit with its words before and after (§FS-005-dispatch.13.2). The lines are
/// the ones `ephor thread` prints under the same heading.
fn review_lines(draft: &Draft, wrap_width: usize, lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(Span::styled(
        "Since the draft:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for entry in &draft.since {
        for (index, text) in entry.lines().into_iter().enumerate() {
            let style = match (index, text.trim_start().chars().next()) {
                (0, _) => Style::default(),
                (_, Some('-')) => Style::default().fg(Color::Red),
                (_, Some('+')) => Style::default().fg(Color::Green),
                _ => Style::default(),
            };
            for line in wrap_line(&format!("  {text}"), wrap_width) {
                lines.push(Line::from(Span::styled(line, style)));
            }
        }
    }
}
