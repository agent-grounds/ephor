use super::*;

/// §FS-021-monitors.18.8: six windows, `7d` by default, and `w` comes back
/// round to where it began.
#[test]
fn the_window_defaults_to_seven_days_and_cycles_through_six() {
    assert_eq!(Window::default(), Window::Week);
    assert_eq!(Window::default().label(), "7d");
    let mut window = Window::SixHours;
    let mut seen = Vec::new();
    for _ in 0..6 {
        seen.push(window.label());
        window = window.next();
    }
    assert_eq!(seen, ["6h", "1d", "7d", "30d", "90d", "1y"]);
    assert_eq!(window, Window::SixHours);
    assert_eq!(Window::Year.span(), TimeDelta::days(365));
    assert_eq!(Window::SixHours.span(), TimeDelta::hours(6));
}
