//! The footer broken between its segments (§FS-004-quick-actions.2).

use super::*;

#[test]
fn a_footer_that_fits_is_left_as_it_is() {
    assert_eq!(
        lines(" j/k move  enter open", 80),
        [" j/k move  enter open"]
    );
}

#[test]
fn a_long_footer_breaks_between_its_segments_and_keeps_each_whole() {
    let said = lines(" j/k move  enter open  w window  r refresh project", 30);
    assert_eq!(
        said,
        [" j/k move  enter open", " w window  r refresh project"]
    );
    for line in &said {
        assert!(line.chars().count() <= 30, "{line}");
    }
    let dotted = lines(
        " enter lay and start  ·  a other workflow  ·  esc waiting",
        24,
    );
    assert_eq!(
        dotted,
        [" enter lay and start", " a other workflow", " esc waiting"]
    );
}

#[test]
fn a_segment_wider_than_the_screen_breaks_at_its_words_and_the_lines_stop_at_most() {
    let said = lines(
        " overview: 3 matters · 1 changed since you looked   q quit",
        20,
    );
    assert!(
        said.iter().all(|line| line.chars().count() <= 20),
        "{said:?}"
    );
    assert_eq!(said[0], " overview: 3 matters");
    assert!(said.len() <= MOST);
}
