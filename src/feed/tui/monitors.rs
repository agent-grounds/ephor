//! The Monitors page and one monitor's detail view (§FS-021-monitors.18,
//! §FS-021-monitors.19).
//!
//! The reading is the API's, the same one `ephor monitors --json` and
//! `ephor monitors show --json` print, so the page `M` opens and the command
//! cannot come apart about what a monitor shows (§AR-009-surfaces.1). The
//! screen holds the cursor, the window it asks for and whether the detail
//! is open; every reading is taken by the shell, off the draw path, and
//! nothing here reads a store, runs a monitor or writes
//! (§FS-021-monitors.18.1).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use ratatui::crossterm::event::KeyCode;

use crate::api::monitor_views::{self as views, MonitorRow};
use crate::api::monitors::NEW;
use crate::feed::model::Item;
use crate::monitors::window::Window;

use super::{Action, Screen};

/// What `o` says on a row whose last answer named no address.
const NO_URL: &str = "o: the last answer carried no url";

pub(crate) struct MonitorsScreen {
    page: views::Monitors,
    /// The open monitor, as the shell last read it.
    detail: Option<views::MonitorDetail>,
    /// Whether the detail view is open: set by `Enter`, before the shell
    /// reads the monitor it asks for.
    open: bool,
    /// What the next reading asks for (§FS-021-monitors.18.8).
    pub window: Window,
    /// The row the cursor is on, in the page's order
    /// (§FS-021-monitors.18.3).
    selected: usize,
    /// The matters `p` processes, resolved with the reading by their key
    /// rather than on a key (§FS-021-monitors.11.3).
    items: BTreeMap<String, Item>,
    /// When the reading was taken: every age and moment on the page is said
    /// as of then, as its health column is.
    now: DateTime<Utc>,
    pub(super) scroll: u16,
    pub(super) viewport: u16,
    pub(super) detail_scroll: u16,
    pub(super) detail_viewport: u16,
    /// The chart point the cursor stands on (§FS-021-monitors.19).
    pub(super) cursor: usize,
}

impl MonitorsScreen {
    pub fn new(
        page: views::Monitors,
        items: BTreeMap<String, Item>,
        window: Window,
        now: DateTime<Utc>,
    ) -> Self {
        MonitorsScreen {
            page,
            detail: None,
            open: false,
            window,
            selected: 0,
            items,
            now,
            scroll: 0,
            viewport: 0,
            detail_scroll: 0,
            detail_viewport: 0,
            cursor: 0,
        }
    }

    pub(super) fn page(&self) -> &views::Monitors {
        &self.page
    }

    pub(super) fn now(&self) -> DateTime<Utc> {
        self.now
    }

    pub(super) fn selected(&self) -> usize {
        self.selected
    }

    /// The detail view, where it is open and read.
    pub(super) fn detail(&self) -> Option<&views::MonitorDetail> {
        self.detail.as_ref().filter(|_| self.open)
    }

    fn row(&self) -> Option<&MonitorRow> {
        self.page.rows().nth(self.selected)
    }

    /// The monitor the open detail view asks the shell for.
    pub(super) fn wanted(&self) -> Option<(String, String)> {
        self.open
            .then(|| self.row())
            .flatten()
            .map(|row| (row.project.clone(), row.id.clone()))
    }

    /// A fresh reading, with the cursor kept on the monitor it was on: a
    /// refresh landing re-reads the page, and a row that moved up or down
    /// would otherwise change what the next key acts on.
    pub fn replace(
        &mut self,
        page: views::Monitors,
        items: BTreeMap<String, Item>,
        now: DateTime<Utc>,
    ) {
        let on = self.row().map(|row| (row.project.clone(), row.id.clone()));
        self.page = page;
        self.items = items;
        self.now = now;
        let count = self.page.rows().count();
        self.selected = on
            .and_then(|(project, id)| {
                self.page
                    .rows()
                    .position(|row| row.project == project && row.id == id)
            })
            .unwrap_or(self.selected)
            .min(count.saturating_sub(1));
        if count == 0 {
            self.close_detail();
        }
    }

    /// The monitor the detail view asked for. The cursor stays on the point
    /// it was on where the same monitor is read again, and otherwise starts
    /// at the newest.
    pub fn show(&mut self, detail: views::MonitorDetail) {
        let same = self.detail.as_ref().is_some_and(|shown| {
            shown.row.project == detail.row.project && shown.row.id == detail.row.id
        });
        let last = detail.chart.len().saturating_sub(1);
        self.cursor = match same {
            true => self.cursor.min(last),
            false => {
                self.detail_scroll = 0;
                last
            }
        };
        self.detail = Some(detail);
        self.open = true;
    }

    pub fn close_detail(&mut self) {
        self.open = false;
        self.detail = None;
    }

    pub fn title(&self) -> String {
        match self.detail() {
            Some(detail) => format!(
                " {}   {} · {}",
                detail.title(),
                detail.window_says(),
                views::refreshed(detail.refreshed_at, self.now)
            ),
            None => format!(" {}", self.page.header(self.now)),
        }
    }

    /// The keys, said on the page (§FS-021-monitors.18.10,
    /// §FS-021-monitors.19).
    pub fn footer(&self) -> &'static str {
        match self.detail() {
            Some(_) => {
                " ←/→ cursor  [/] previous/next  w window  r refresh project  o open url  \
                 p process matter  j/k scroll  esc page  M close"
            }
            None => {
                " j/k move  enter open  w window  r refresh project  o open url  \
                 p process matter  esc/M back"
            }
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> Action {
        match self.detail.is_some() && self.open {
            true => self.detail_key(code),
            false => self.page_key(code),
        }
    }

    /// The page's keys (§FS-021-monitors.18.10).
    fn page_key(&mut self, code: KeyCode) -> Action {
        let last = self.page.rows().count().saturating_sub(1);
        let page = usize::from(self.viewport.max(1));
        match code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace => {
                Action::Back
            }
            KeyCode::Char('j') | KeyCode::Down => self.go(self.selected.saturating_add(1), last),
            KeyCode::Char('k') | KeyCode::Up => self.go(self.selected.saturating_sub(1), last),
            KeyCode::Char('g') | KeyCode::Home => self.go(0, last),
            KeyCode::Char('G') | KeyCode::End => self.go(last, last),
            KeyCode::Char('f') | KeyCode::PageDown => {
                self.go(self.selected.saturating_add(page), last)
            }
            KeyCode::Char('b') | KeyCode::PageUp => {
                self.go(self.selected.saturating_sub(page), last)
            }
            // The detail view is read by the shell, as `ephor monitors show`
            // reads it (§FS-021-monitors.19).
            KeyCode::Enter => match self.row() {
                Some(_) => {
                    self.open = true;
                    Action::ReadMonitor
                }
                None => Action::None,
            },
            // §FS-021-monitors.18.8: the window, which `--window` spells.
            KeyCode::Char('w') => {
                self.window = self.window.next();
                Action::ReadMonitors
            }
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Char('o') => self.open_url(),
            KeyCode::Char('p') => self.process(None),
            _ => Action::None,
        }
    }

    /// The detail view's keys (§FS-021-monitors.19): `←` and `→` walk the
    /// chart, `[` and `]` step through the page's order, and `w`, `r`, `o`
    /// and `p` work as on the page.
    fn detail_key(&mut self, code: KeyCode) -> Action {
        let last = self.page.rows().count().saturating_sub(1);
        let points = self.detail.as_ref().map_or(0, |detail| detail.chart.len());
        match code {
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('h') | KeyCode::Backspace => {
                self.close_detail();
                Action::None
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                Action::None
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(points.saturating_sub(1));
                Action::None
            }
            KeyCode::Char('[') => self.step(self.selected.saturating_sub(1), last),
            KeyCode::Char(']') => self.step(self.selected.saturating_add(1), last),
            KeyCode::Char('j') | KeyCode::Down => {
                self.detail_scroll = self.detail_scroll.saturating_add(1);
                Action::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.detail_scroll = self.detail_scroll.saturating_sub(1);
                Action::None
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.detail_scroll = 0;
                Action::None
            }
            KeyCode::Char('f') | KeyCode::PageDown => {
                self.detail_scroll = self
                    .detail_scroll
                    .saturating_add(self.detail_viewport.max(1));
                Action::None
            }
            KeyCode::Char('b') | KeyCode::PageUp => {
                self.detail_scroll = self
                    .detail_scroll
                    .saturating_sub(self.detail_viewport.max(1));
                Action::None
            }
            // §FS-021-monitors.19: a series drawn by `x` labels has no time
            // to set a window on.
            KeyCode::Char('w') if self.detail.as_ref().is_some_and(|detail| !detail.windowed) => {
                Action::SetMessage(
                    "w: a series shows the points the repository supplied, so the window does \
                     not apply"
                        .to_string(),
                )
            }
            KeyCode::Char('w') => {
                self.window = self.window.next();
                Action::ReadMonitors
            }
            KeyCode::Char('r') => self.refresh(),
            KeyCode::Char('o') => self.open_url(),
            KeyCode::Char('p') => {
                let no_matter = self
                    .detail
                    .as_ref()
                    .and_then(|detail| detail.no_matter.clone());
                self.process(no_matter)
            }
            _ => Action::None,
        }
    }

    fn go(&mut self, to: usize, last: usize) -> Action {
        self.selected = to.min(last);
        Action::None
    }

    /// `[` and `]`: the previous or the next monitor, read as `Enter` reads
    /// one.
    fn step(&mut self, to: usize, last: usize) -> Action {
        let to = to.min(last);
        if to == self.selected {
            return Action::None;
        }
        self.selected = to;
        Action::ReadMonitor
    }

    /// The row the keys act on: the open monitor's, which is the selected
    /// one.
    fn acted_on(&self) -> Option<&MonitorRow> {
        self.detail()
            .map(|detail| &detail.row)
            .or_else(|| self.row())
    }

    /// `r`: the row's project, every monitor of it now, beneath the screen
    /// (§FS-021-monitors.18.10, §FS-001-forge-interface.7).
    fn refresh(&self) -> Action {
        match self.acted_on() {
            Some(row) => Action::RefreshMonitors(row.project.clone()),
            None => Action::SetMessage("r: no monitor to refresh here".to_string()),
        }
    }

    /// `o`: the last sample's `url` (§FS-021-monitors.18.10).
    fn open_url(&self) -> Action {
        match self.acted_on().and_then(|row| row.url.clone()) {
            Some(url) => Action::OpenUrl(Some(url)),
            None => Action::SetMessage(NO_URL.to_string()),
        }
    }

    /// `p`: an open matter nothing has been done about is processed in
    /// place, through the move `Enter` makes on Waiting
    /// (§FS-021-monitors.18.10, §FS-019-waiting.7.2). Anything else says
    /// why there is nothing to process: `no_matter` is the detail view's own
    /// sentence where it has one (§FS-021-monitors.19).
    fn process(&self, no_matter: Option<String>) -> Action {
        let Some(row) = self.acted_on() else {
            return Action::None;
        };
        let Some(matter) = &row.matter else {
            return Action::SetMessage(match no_matter {
                Some(why) => format!("p: {why}"),
                None if !row.shows.status => "p: a KPI has no matter".to_string(),
                None if row.shows.overdue => {
                    format!("p: overdue opens no matter; r refreshes {}", row.project)
                }
                None => "p: no matter is open for this monitor".to_string(),
            });
        };
        match matter.state {
            NEW => match self.items.get(&matter.key) {
                Some(item) => Action::ProcessMatter {
                    item: item.clone(),
                    entry: None,
                    choosing: false,
                },
                None => Action::SetMessage(format!(
                    "p: {} is not in the feed yet; r refreshes {}",
                    matter.key, row.project
                )),
            },
            "running" => Action::SetMessage(
                "p: work is already running on this matter; Waiting shows the run".to_string(),
            ),
            state => Action::SetMessage(format!(
                "p: the matter is {state}; the next drop opens a new one"
            )),
        }
    }
}

/// What the shell does about this page (§FS-021-monitors.18). Off the draw
/// path, like the burn page's: opening, re-reading after a key or a refresh,
/// and the scoped refresh `r` starts.
impl super::App {
    /// Open the Monitors page over whatever is on screen, or close it back
    /// to where the reader was (§FS-021-monitors.18.1). The page joins the
    /// operations board and the burn page in one modal layer: opening any
    /// of them over another replaces it and keeps the screen beneath.
    pub(super) fn toggle_monitors(&mut self) {
        if matches!(self.screen, Screen::Monitors(_)) {
            self.screen = self.saved.take().unwrap_or_else(|| self.home_screen());
            return;
        }
        let window = Window::default();
        let page = self.ctx.monitors(window, &[]);
        let items = self.monitor_items(&page);
        let screen = Screen::Monitors(MonitorsScreen::new(page, items, window, Utc::now()));
        let previous = std::mem::replace(&mut self.screen, screen);
        if !matches!(previous, Screen::Operations(_) | Screen::Burn(_)) {
            self.saved = Some(previous);
        }
    }

    /// Take the page again under the window it now asks for, and the open
    /// monitor with it. A closed page costs nothing; nothing re-reads it on
    /// a tick (§FS-021-monitors.22).
    pub(super) fn reload_monitors(&mut self) -> bool {
        let Screen::Monitors(screen) = &self.screen else {
            return false;
        };
        let window = screen.window;
        let page = self.ctx.monitors(window, &[]);
        let items = self.monitor_items(&page);
        if let Screen::Monitors(screen) = &mut self.screen {
            screen.replace(page, items, Utc::now());
        }
        self.read_monitor();
        true
    }

    /// Read the monitor the detail view asks for, as `ephor monitors show`
    /// reads it (§FS-021-monitors.19, §FS-021-monitors.21.3).
    pub(super) fn read_monitor(&mut self) {
        let Screen::Monitors(screen) = &self.screen else {
            return;
        };
        let Some((project, id)) = screen.wanted() else {
            return;
        };
        let window = screen.window;
        let read = self.ctx.monitor(&project, &id, window);
        let Screen::Monitors(screen) = &mut self.screen else {
            return;
        };
        match read {
            Ok(detail) => screen.show(detail),
            Err(refusal) => {
                screen.close_detail();
                self.message = refusal;
            }
        }
    }

    /// `r`: refresh one project and read every monitor of it now, beneath
    /// the screen, as `ephor refresh <project> --monitors-now` does
    /// (§FS-021-monitors.18.10, §FS-021-monitors.6.7).
    pub(super) fn refresh_monitors(
        &mut self,
        config: &crate::feed::config::StatusConfig,
        project: &str,
    ) {
        if self.refresh.is_some() {
            self.message = "Already refreshing".to_string();
            return;
        }
        let over = [project.to_string()];
        match crate::feed::refresh::BackgroundRefresh::start_when(
            config,
            &over,
            None,
            crate::monitors::watch::When::Now,
        ) {
            Ok(refresh) => {
                self.message.clear();
                self.refresh = Some(refresh);
            }
            Err(err) => self.message = err.to_string(),
        }
    }

    /// The matters the page's rows lead to, resolved once with the reading,
    /// in one walk of the feeds.
    fn monitor_items(&self, page: &views::Monitors) -> BTreeMap<String, Item> {
        let wanted: std::collections::BTreeSet<&str> = page
            .rows()
            .filter_map(|row| row.matter.as_ref())
            .filter(|matter| matter.state == NEW)
            .map(|matter| matter.key.as_str())
            .collect();
        if wanted.is_empty() {
            return BTreeMap::new();
        }
        self.ctx
            .feeds
            .iter()
            .flat_map(|feed| feed.items())
            .filter(|item| wanted.contains(item.id.as_str()))
            .map(|item| (item.id.clone(), item))
            .collect()
    }

    /// The screen processing replaced. Processing opened from the Monitors
    /// page is processing in place, so `esc` and a confirmed move come back
    /// to the page rather than to Waiting (§FS-021-monitors.18.10); one
    /// opened from anywhere else forgets any page kept from before.
    pub(super) fn processing_over(&mut self, previous: Screen) {
        match previous {
            Screen::Monitors(page) => self.monitors_beneath = Some(page),
            Screen::Process(_) => {}
            _ => self.monitors_beneath = None,
        }
    }

    /// Back to the page processing was opened from, where it was; true
    /// where there was one.
    pub(super) fn back_to_monitors(&mut self) -> bool {
        if !matches!(self.screen, Screen::Process(_)) {
            return false;
        }
        let Some(page) = self.monitors_beneath.take() else {
            return false;
        };
        self.screen = Screen::Monitors(page);
        true
    }
}

#[cfg(test)]
#[path = "monitors_tests.rs"]
mod tests;
