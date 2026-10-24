//! The main window and the UI it shows.
//!
//! The `cef` crate's default callbacks return 0, which means "no" or "cancel": every callback the app
//! relies on is implemented explicitly.

use crate::command::Command;
use arbalette_core::sidebar::{NodeId, NodeKind, ProfileId, Sidebar, SpaceId, Tab};
use arbalette_core::{import, store};
use cef::wrapper::stream_resource_handler::StreamResourceHandler;
use cef::*;
use serde::Serialize;
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread::LocalKey;

/// The UI is served by the app itself under this origin.
const UI_ORIGIN: &str = "https://arbalette.ui/";

const SIDEBAR_WIDTH: i32 = 260;

/// The strip along the window's left edge that reveals the hidden sidebar when the pointer
/// reaches it. Views cannot be transparent, so it shows the sidebar's background.
const EDGE_WIDTH: i32 = 4;

/// The overlay's size: one text field for now (ui/src/Overlay.tsx).
const OVERLAY_SIZE: Size = Size {
    width: 600,
    height: 56,
};

/// The height of the sidebar's toolbar, which holds the window buttons on macOS (ui/src/index.css).
const TOOLBAR_HEIGHT: f32 = 40.0;

/// The first tab of a new session.
const START_URL: &str = "https://www.wikipedia.org/";

thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
    static SIDEBAR: RefCell<Option<BrowserView>> = const { RefCell::new(None) };
    /// The content view of each profile, `None` until its browser is created. Tabs of the same
    /// profile share one view: switching between them only navigates.
    static CONTENTS: RefCell<HashMap<ProfileId, Option<BrowserView>>> = RefCell::default();
    /// The profile whose content view the window shows.
    static SHOWN_PROFILE: Cell<Option<ProfileId>> = const { Cell::new(None) };
    /// Shown above the page, to open tabs.
    static OVERLAY_VIEW: RefCell<Option<BrowserView>> = const { RefCell::new(None) };
    static OVERLAY: RefCell<Option<OverlayController>> = const { RefCell::new(None) };
    /// A second sidebar, shown above the page while the sidebar is hidden.
    static REVEAL_VIEW: RefCell<Option<BrowserView>> = const { RefCell::new(None) };
    static REVEAL: RefCell<Option<OverlayController>> = const { RefCell::new(None) };
    static SIDEBAR_MODE: Cell<SidebarMode> = const { Cell::new(SidebarMode::Shown) };
    /// The sidebar's draggable regions, which move the window while the sidebar is shown.
    static DRAG_REGIONS: RefCell<Vec<DraggableRegion>> = const { RefCell::new(Vec::new()) };
    static STATE: RefCell<UiState> = RefCell::default();
    /// Where the sidebar is saved, once the previous session is restored.
    static SESSION_FILE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    /// CEF's root cache path, which holds every profile's web data.
    static DATA_DIR: RefCell<PathBuf> = RefCell::default();
    /// Browsers not closed yet: the app quits when the last one closes.
    static OPEN_BROWSERS: Cell<usize> = const { Cell::new(0) };
}

/// The sidebar sits beside the page, or is hidden: then a strip at the window's left edge
/// reveals it above the page while the pointer is over it. For this session only.
#[derive(Clone, Copy, PartialEq)]
enum SidebarMode {
    Shown,
    Hidden,
    Revealed,
}

/// What the UI shows.
#[derive(Default)]
struct UiState {
    page: Page,
    sidebar: Sidebar,
    /// Closed tabs, the last one at the end, until the app quits.
    closed: Vec<Tab>,
}

impl UiState {
    /// Sent whole to the UI on every change: the displayed Space's tabs and its profile's
    /// favorites. `roots` names the sections' containers, which the UI moves nodes into.
    fn to_json(&self) -> serde_json::Value {
        let space = self.sidebar.displayed();
        let favorites = self
            .sidebar
            .favorites_of(space.id)
            .expect("a known profile");
        json!({
            "page": self.page,
            "roots": { "favorites": favorites, "pinned": space.pinned, "today": space.today },
            "favorites": self.tabs(favorites),
            "pinned": self.items(space.pinned),
            "today": self.items(space.today),
            "active": space.selected,
            "space": space.id,
            "spaces": self.sidebar.spaces().iter().map(|s| json!({
                "id": s.id,
                "title": s.title,
                "emoji": s.emoji,
                "color": s.color,
            })).collect::<Vec<_>>(),
            "profiles": self.sidebar.profiles().iter().map(|p| json!({
                "id": p.id,
                "name": p.name,
                "color": p.color,
            })).collect::<Vec<_>>(),
        })
    }

    /// The tabs directly inside a container, like the favorites, which hold no folders.
    fn tabs(&self, parent: NodeId) -> Vec<serde_json::Value> {
        let node = self.sidebar.node(parent).expect("a known container");
        node.children
            .iter()
            .filter_map(|&id| match &self.sidebar.node(id)?.kind {
                NodeKind::Tab(tab) => Some(tab_json(id, tab)),
                _ => None,
            })
            .collect()
    }

    /// The tabs and folders inside a container or a folder, folders with their own content.
    fn items(&self, parent: NodeId) -> Vec<serde_json::Value> {
        let node = self.sidebar.node(parent).expect("a known container");
        node.children
            .iter()
            .filter_map(|&id| match &self.sidebar.node(id)?.kind {
                NodeKind::Tab(tab) => Some(tab_json(id, tab)),
                NodeKind::Folder { title, expanded } => Some(json!({
                    "id": id,
                    "title": title,
                    "expanded": expanded,
                    "children": self.items(id),
                })),
                NodeKind::Container => None,
            })
            .collect()
    }
}

fn tab_json(id: NodeId, tab: &Tab) -> serde_json::Value {
    json!({
        "id": id,
        "url": tab.url,
        "title": tab.title,
        "favicon": tab.favicon,
    })
}

/// The loading state of the content view; its title and url belong to the active tab.
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    /// From 0 to 1.
    progress: f64,
    loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
}

/// Changes the page and sends the state to the UI.
fn update_page(change: impl FnOnce(&mut Page)) {
    STATE.with_borrow_mut(|state| change(&mut state.page));
    send_state();
}

/// Changes the sidebar, saves it and sends the state to the UI.
fn update_sidebar<R>(change: impl FnOnce(&mut Sidebar) -> R) -> R {
    let result = STATE.with_borrow_mut(|state| change(&mut state.sidebar));
    if let Some(path) = SESSION_FILE.with_borrow(Clone::clone)
        && let Err(error) = STATE.with_borrow(|state| store::save(&state.sidebar, &path))
    {
        eprintln!("cannot save the session: {error}");
    }
    send_state();
    result
}

fn send_state() {
    let json = STATE.with_borrow(UiState::to_json);
    let event = format!(r#"new CustomEvent("arbalette:state", {{ detail: {json} }})"#);
    dispatch(&SIDEBAR, &event);
    dispatch(&REVEAL_VIEW, &event);
}

/// Dispatches a DOM event, given as JavaScript, in one of the UI's views.
fn dispatch(view: &'static LocalKey<RefCell<Option<BrowserView>>>, event: &str) {
    if let Some(frame) = view.with_borrow(|view| view.as_ref()?.browser()?.main_frame()) {
        let code = format!("dispatchEvent({event})");
        frame.execute_java_script(Some(&CefString::from(code.as_str())), None, 0);
    }
}

fn show_overlay(visible: bool) {
    let Some(overlay) = OVERLAY.with_borrow(Clone::clone) else {
        return;
    };
    if visible {
        let window = WINDOW.with_borrow(|window| window.as_ref().map(|w| w.size()));
        let width = window.map_or(0, |size| size.width);
        overlay.set_bounds(Some(&Rect {
            x: (width - OVERLAY_SIZE.width) / 2,
            y: 96,
            width: OVERLAY_SIZE.width,
            height: OVERLAY_SIZE.height,
        }));
        dispatch(&OVERLAY_VIEW, r#"new Event("arbalette:show")"#);
    }
    overlay.set_visible(visible.into());
    if visible && let Some(view) = overlay.contents_view() {
        view.request_focus();
    }
}

fn set_sidebar(mode: SidebarMode) {
    SIDEBAR_MODE.set(mode);
    let shown = mode == SidebarMode::Shown;
    if let Some(sidebar) = SIDEBAR.with_borrow(Clone::clone) {
        sidebar.set_visible(shown.into());
    }
    place_reveal();
    let Some(window) = WINDOW.with_borrow(Clone::clone) else {
        return;
    };
    let regions = DRAG_REGIONS.with_borrow(|regions| if shown { regions.clone() } else { vec![] });
    window.set_draggable_regions(Some(&regions));
    #[cfg(target_os = "macos")]
    crate::mac::sidebar_shown(window.window_handle(), shown);
}

/// Places the revealed sidebar, or the strip that reveals it, along the window's left edge.
fn place_reveal() {
    let Some(reveal) = REVEAL.with_borrow(Clone::clone) else {
        return;
    };
    let mode = SIDEBAR_MODE.get();
    let height = WINDOW.with_borrow(|window| window.as_ref().map_or(0, |w| w.size().height));
    let width = if mode == SidebarMode::Revealed {
        SIDEBAR_WIDTH
    } else {
        EDGE_WIDTH
    };
    reveal.set_bounds(Some(&Rect {
        x: 0,
        y: 0,
        width,
        height,
    }));
    reveal.set_visible((mode != SidebarMode::Shown).into());
}

/// Loads the sidebar saved by the previous session, or on the first launch imports Arc's; a file
/// that cannot be read is replaced on the next save.
fn restore(path: PathBuf) {
    match store::load(&path) {
        Ok(Some(sidebar)) => STATE.with_borrow_mut(|state| state.sidebar = sidebar),
        Ok(None) => {
            if let Some(sidebar) = import_arc() {
                // Saved at once, so that the next launch restores it instead of importing again.
                if let Err(error) = store::save(&sidebar, &path) {
                    eprintln!("cannot save the session: {error}");
                }
                STATE.with_borrow_mut(|state| state.sidebar = sidebar);
            }
        }
        Err(error) => eprintln!("cannot restore the session: {error:?}"),
    }
    SESSION_FILE.set(Some(path));
    if STATE.with_borrow_mut(|state| active_tab(&mut state.sidebar).is_none()) {
        open_tab(START_URL);
    }
}

/// Arc's sidebar, when Arc was used on this machine; an import that fails leaves the default one.
fn import_arc() -> Option<Sidebar> {
    let dir = crate::arc_dir()?;
    import::arc::read(&dir).unwrap_or_else(|error| {
        eprintln!("cannot import Arc: {error:?}");
        None
    })
}

fn active_tab(sidebar: &mut Sidebar) -> Option<&mut Tab> {
    let id = sidebar.displayed().selected?;
    sidebar.tab_mut(id)
}

/// Opens a tab in the displayed Space and shows it.
fn open_tab(url: &str) {
    update_sidebar(|sidebar| {
        let space = sidebar.displayed().id;
        let tab = sidebar.open_tab(space, url).expect("the displayed Space");
        sidebar.select(space, tab).expect("a new tab");
    });
    show_selected();
}

/// Shows a tab, with its Space; a favorite is shown in the displayed Space.
fn select_tab(id: NodeId) {
    let selected = update_sidebar(|sidebar| {
        let space = sidebar.space_of(id).unwrap_or(sidebar.displayed().id);
        sidebar
            .select(space, id)
            .and_then(|()| sidebar.display(space))
    });
    if selected.is_ok() {
        show_selected();
    }
}

/// Shows the tab `find` picks, if any.
fn select_found(find: impl FnOnce(&Sidebar) -> Option<NodeId>) {
    if let Some(tab) = STATE.with_borrow(|state| find(&state.sidebar)) {
        select_tab(tab);
    }
}

/// Shows a Space in the sidebar and its selected tab; like a selection, an unknown Space from the
/// UI is ignored.
fn show_space(id: SpaceId) {
    if update_sidebar(|sidebar| sidebar.display(id)).is_ok() {
        show_selected();
    }
}

/// Shows the Space `find` picks, if any.
fn show_found_space(find: impl FnOnce(&Sidebar) -> Option<SpaceId>) {
    if let Some(space) = STATE.with_borrow(|state| find(&state.sidebar)) {
        show_space(space);
    }
}

/// Closes a tab; closing the selected one shows the next tab, or the overlay when none is left.
/// Favorites and pinned tabs stay where they are: closing one deselects it and takes it back to
/// its saved url, and it cannot be reopened since it was never removed.
fn close_tab(tab: Option<NodeId>) {
    let (selected, tab) = STATE.with_borrow_mut(|state| {
        let selected = state.sidebar.displayed().selected;
        let tab = tab
            .or(selected)
            .filter(|&id| state.sidebar.tab_mut(id).is_some());
        (selected, tab)
    });
    let Some(tab) = tab else { return };
    let stays =
        STATE.with_borrow(|state| state.sidebar.is_favorite(tab) || state.sidebar.is_pinned(tab));
    let next = if stays {
        update_sidebar(|sidebar| {
            let space = sidebar.displayed().id;
            sidebar
                .close_in_place(space, tab)
                .expect("the displayed Space")
        })
    } else {
        let next = STATE.with_borrow_mut(|state| {
            let closed = state.sidebar.tab_mut(tab).expect("an open tab").clone();
            state.closed.push(closed);
            state.sidebar.next_after_close(tab)
        });
        update_sidebar(|sidebar| sidebar.remove(tab)).expect("an open tab");
        next
    };
    if selected != Some(tab) {
        return;
    }
    match next {
        Some(next) => select_tab(next),
        None => {
            load("about:blank");
            show_overlay(true);
        }
    }
}

/// Pins or unpins a tab, or the selected one.
fn toggle_pin(tab: Option<NodeId>) {
    update_sidebar(|sidebar| {
        let space = sidebar.displayed();
        let (space, tab) = (space.id, tab.or(space.selected));
        if let Some(tab) = tab {
            // Like a selection, an unknown tab from the UI is ignored.
            let _ = sidebar.toggle_pin(space, tab);
        }
    });
}

/// Creates a folder at the end of the pinned tabs and asks the sidebar to rename it, giving it the
/// keyboard focus even when the menu was used from the page.
fn new_folder() {
    let folder = update_sidebar(|sidebar| {
        let pinned = sidebar.displayed().pinned;
        sidebar
            .add_folder(pinned, "New Folder")
            .expect("the pinned tabs")
    });
    if let Some(sidebar) = SIDEBAR.with_borrow(Clone::clone) {
        sidebar.request_focus();
    }
    let folder = serde_json::to_string(&folder).expect("an id");
    let event = format!(r#"new CustomEvent("arbalette:rename", {{ detail: {folder} }})"#);
    dispatch(&SIDEBAR, &event);
    dispatch(&REVEAL_VIEW, &event);
}

/// Creates a profile and asks the sidebar to rename it, like a new folder.
fn new_profile() {
    let profile = update_sidebar(|sidebar| {
        sidebar
            .create_profile("New Profile")
            .expect("a name that is not blank")
    });
    if let Some(sidebar) = SIDEBAR.with_borrow(Clone::clone) {
        sidebar.request_focus();
    }
    let profile = serde_json::to_string(&profile).expect("an id");
    let event = format!(r#"new CustomEvent("arbalette:rename-profile", {{ detail: {profile} }})"#);
    dispatch(&SIDEBAR, &event);
    dispatch(&REVEAL_VIEW, &event);
}

/// Creates a Space in the displayed Space's profile, shows it and asks the sidebar to rename it,
/// like a new profile.
fn new_space() {
    let space = update_sidebar(|sidebar| {
        let profile = sidebar.displayed().profile;
        sidebar
            .create_space("New Space", profile)
            .expect("the displayed Space's profile")
    });
    show_selected();
    if let Some(sidebar) = SIDEBAR.with_borrow(Clone::clone) {
        sidebar.request_focus();
    }
    let space = serde_json::to_string(&space).expect("an id");
    let event = format!(r#"new CustomEvent("arbalette:rename-space", {{ detail: {space} }})"#);
    dispatch(&SIDEBAR, &event);
    dispatch(&REVEAL_VIEW, &event);
}

fn reopen_tab() {
    let Some(closed) = STATE.with_borrow_mut(|state| state.closed.pop()) else {
        return;
    };
    open_tab(&closed.url);
    update_sidebar(|sidebar| {
        if let Some(tab) = active_tab(sidebar) {
            tab.title = closed.title;
            tab.favicon = closed.favicon;
        }
    });
}

/// Loads a url in the shown content view.
fn load(url: &str) {
    if let Some(frame) = content().and_then(|view| view.browser()?.main_frame()) {
        frame.load_url(Some(&CefString::from(url)));
    }
}

/// The content view the window shows.
fn content() -> Option<BrowserView> {
    let profile = SHOWN_PROFILE.get()?;
    CONTENTS.with_borrow(|contents| contents.get(&profile)?.clone())
}

/// Whether a browser is the shown content view's: the other views' pages are blank.
fn is_shown(browser: Option<&mut Browser>) -> bool {
    let (Some(browser), Some(mut shown)) = (browser, content().and_then(|view| view.browser()))
    else {
        return false;
    };
    browser.is_same(Some(&mut shown)) == 1
}

/// The selected tab's profile and url.
fn selected() -> Option<(ProfileId, String)> {
    STATE.with_borrow_mut(|state| {
        let tab = state.sidebar.displayed().selected?;
        let profile = state.sidebar.profile_of(tab)?;
        Some((profile, state.sidebar.tab_mut(tab)?.url.clone()))
    })
}

/// Shows the selected tab in its profile's content view, which is created the first time, or a
/// blank page when the displayed Space has none.
fn show_selected() {
    let Some((profile, url)) = selected() else {
        load("about:blank");
        return;
    };
    match CONTENTS.with_borrow(|contents| contents.get(&profile).cloned()) {
        Some(Some(view)) => {
            show_content(profile, &view);
            load(&url);
        }
        // The view shows the selected tab once its browser is created.
        Some(None) => {}
        None => create_profile_context(profile),
    }
}

/// Makes a profile's content view usable once its browser exists, and shows the selected tab in it
/// if the tab belongs to the profile.
fn content_created(profile: ProfileId, browser: Option<&mut Browser>) {
    let view = browser_view_get_for_browser(browser);
    CONTENTS.with_borrow_mut(|contents| contents.insert(profile, view));
    if selected().is_some_and(|(selected, _)| selected == profile) {
        show_selected();
    }
}

/// Puts a profile's content view in the window instead of the shown one, which is left on a
/// blank page: only the selected tab runs.
fn show_content(profile: ProfileId, view: &BrowserView) {
    if SHOWN_PROFILE.get() == Some(profile) {
        return;
    }
    if let Some(shown) = content() {
        load("about:blank");
        shown.set_visible(0);
    }
    SHOWN_PROFILE.set(Some(profile));
    view.set_visible(1);
    if let Some(window) = WINDOW.with_borrow(Clone::clone) {
        window.layout();
    }
}

/// Opens a profile's web data in its own directory, `Profile <id>`: Chromium only accepts profile
/// directories directly in the root cache path. CEF needs it initialized before a browser can
/// use it (ProfileContextHandler).
fn create_profile_context(profile: ProfileId) {
    CONTENTS.with_borrow_mut(|contents| contents.insert(profile, None));
    let id = serde_json::to_string(&profile).expect("an id");
    let dir = DATA_DIR.with_borrow(|dir| dir.join(format!("Profile {id}")));
    let settings = RequestContextSettings {
        cache_path: CefString::from(dir.to_str().expect("a UTF-8 path")),
        ..Default::default()
    };
    request_context_create_context(
        Some(&settings),
        Some(&mut ProfileContextHandler::new(profile)),
    );
}

wrap_request_context_handler! {
    struct ProfileContextHandler {
        profile: ProfileId,
    }

    impl RequestContextHandler {
        /// Adds the profile's content view to the window, hidden. Its browser is created after it
        /// (content_created).
        fn on_request_context_initialized(&self, context: Option<&mut RequestContext>) {
            let Some(window) = WINDOW.with_borrow(Clone::clone) else { return };
            let view = browser_view(ContentClient::new(self.profile), "about:blank", 0, context);
            view.set_visible(0);
            let mut child = View::from(&view);
            window.add_child_view(Some(&mut child));
            if let Some(layout) = window.get_layout().and_then(|layout| layout.as_box_layout()) {
                layout.set_flex_for_view(Some(&mut child), 1);
            }
        }
    }
}

/// Runs a command from the UI or a menu, on the UI thread.
pub fn run(command: Command) {
    let browser = content().and_then(|view| view.browser());
    match (command, browser) {
        (Command::Ready, _) => send_state(),
        (Command::Select(id), _) => select_tab(id),
        (Command::NextTab, _) => select_found(|s| s.next_tab(s.displayed().id, true)),
        (Command::PreviousTab, _) => select_found(|s| s.next_tab(s.displayed().id, false)),
        (Command::Favorite(number), _) => select_found(|s| s.favorite(s.displayed().id, number)),
        (Command::ShowSpace(id), _) => show_space(id),
        (Command::SpaceNumber(number), _) => show_found_space(|s| s.space_at(number)),
        (Command::NextSpace, _) => show_found_space(|s| s.next_space(true)),
        (Command::PreviousSpace, _) => show_found_space(|s| s.next_space(false)),
        (Command::ShowOverlay, _) => show_overlay(true),
        (Command::HideOverlay, _) => show_overlay(false),
        (Command::ToggleSidebar, _) => set_sidebar(match SIDEBAR_MODE.get() {
            SidebarMode::Shown => SidebarMode::Hidden,
            _ => SidebarMode::Shown,
        }),
        (Command::RevealSidebar, _) if SIDEBAR_MODE.get() == SidebarMode::Hidden => {
            set_sidebar(SidebarMode::Revealed);
        }
        (Command::ConcealSidebar, _) if SIDEBAR_MODE.get() == SidebarMode::Revealed => {
            set_sidebar(SidebarMode::Hidden);
        }
        (Command::RevealSidebar | Command::ConcealSidebar, _) => {}
        (Command::Open(url), _) => {
            show_overlay(false);
            open_tab(&url);
        }
        (Command::Close(tab), _) => close_tab(tab),
        (Command::Reopen, _) => reopen_tab(),
        (Command::Pin(tab), _) => toggle_pin(tab),
        (Command::NewFolder, _) => new_folder(),
        // Like a selection, an unknown folder or a blank title from the UI is ignored.
        (Command::ToggleFolder(id), _) => {
            let _ = update_sidebar(|sidebar| sidebar.toggle_folder(id));
        }
        (Command::RenameFolder(id, title), _) => {
            let _ = update_sidebar(|sidebar| sidebar.rename_folder(id, &title));
        }
        (Command::Move(id, parent, index), _) => {
            let _ = update_sidebar(|sidebar| sidebar.move_node(id, parent, index));
        }
        (Command::NewProfile, _) => new_profile(),
        // Like a folder, an unknown profile or a blank name from the UI is ignored.
        (Command::RenameProfile(id, name), _) => {
            let _ = update_sidebar(|sidebar| sidebar.rename_profile(id, &name));
        }
        (Command::ColorProfile(id, color), _) => {
            let _ = update_sidebar(|sidebar| sidebar.set_profile_color(id, color));
        }
        (Command::NewSpace, _) => new_space(),
        // Like a profile, an unknown Space or a blank title from the UI is ignored.
        (Command::RenameSpace(id, title), _) => {
            let _ = update_sidebar(|sidebar| sidebar.rename_space(id, &title));
        }
        (Command::SetSpaceEmoji(id, emoji), _) => {
            let _ = update_sidebar(|sidebar| sidebar.set_space_emoji(id, &emoji));
        }
        (Command::ColorSpace(id, color), _) => {
            let _ = update_sidebar(|sidebar| sidebar.set_space_color(id, color));
        }
        (Command::Back, Some(browser)) => browser.go_back(),
        (Command::Forward, Some(browser)) => browser.go_forward(),
        (Command::Reload, Some(browser)) => browser.reload(),
        (_, None) => {}
    }
}

/// Closes the main window, which quits the app once its browser is closed. Called by the macOS
/// app menu; the window's own close button does the same on every OS.
#[cfg(target_os = "macos")]
pub fn close() {
    if let Some(window) = WINDOW.with_borrow(Clone::clone) {
        window.close();
    }
}

wrap_app! {
    pub struct ArbaletteApp {
        data_dir: PathBuf,
    }

    impl App {
        fn on_before_command_line_processing(&self, _process_type: Option<&CefString>, command_line: Option<&mut CommandLine>) {
            let Some(command_line) = command_line else { return };
            // Development builds are signed ad hoc, so each build would ask for keychain access.
            if cfg!(debug_assertions) {
                command_line.append_switch(Some(&CefString::from("use-mock-keychain")));
            }
            // Lets the end-to-end check and the benchmark (e2e/devtools.mjs) drive the UI through DevTools.
            if let Ok(port) = std::env::var("ARBALETTE_DEBUG_PORT") {
                command_line.append_switch_with_value(
                    Some(&CefString::from("remote-debugging-port")),
                    Some(&CefString::from(port.as_str())),
                );
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(ArbaletteBrowserProcessHandler::new(self.data_dir.clone()))
        }
    }
}

wrap_browser_process_handler! {
    struct ArbaletteBrowserProcessHandler {
        data_dir: PathBuf,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            DATA_DIR.set(self.data_dir.clone());
            restore(self.data_dir.join(store::FILE_NAME));
            window_create_top_level(Some(&mut MainWindowDelegate::new()));
        }
    }
}

wrap_window_delegate! {
    struct MainWindowDelegate {}

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 1280, height: 820 }
        }

        // The revealed sidebar is as tall as the window.
        fn on_layout_changed(&self, _view: Option<&mut View>, _new_bounds: Option<&Rect>) {
            place_reveal();
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let window = window.expect("window").clone();
            window.set_title(Some(&CefString::from("Arbalette")));
            let layout = window
                .set_to_box_layout(Some(&BoxLayoutSettings {
                    horizontal: 1,
                    cross_axis_alignment: AxisAlignment::STRETCH,
                    ..Default::default()
                }))
                .expect("box layout");
            let sidebar = browser_view(UiClient::new(), &format!("{UI_ORIGIN}index.html"), SIDEBAR_WIDTH, None);
            SIDEBAR.set(Some(sidebar.clone()));
            let mut view = View::from(&sidebar);
            window.add_child_view(Some(&mut view));
            layout.set_flex_for_view(Some(&mut view), 0);
            // Below the overlay, which is added after it.
            let reveal = browser_view(UiClient::new(), &format!("{UI_ORIGIN}index.html#reveal"), 0, None);
            let controller = window
                .add_overlay_view(Some(&mut View::from(&reveal)), DockingMode::CUSTOM, 1)
                .expect("reveal");
            controller.set_visible(0);
            REVEAL_VIEW.set(Some(reveal));
            REVEAL.set(Some(controller));
            let overlay = browser_view(UiClient::new(), &format!("{UI_ORIGIN}index.html#overlay"), 0, None);
            let controller = window
                .add_overlay_view(Some(&mut View::from(&overlay)), DockingMode::CUSTOM, 1)
                .expect("overlay");
            controller.set_visible(0);
            OVERLAY_VIEW.set(Some(overlay));
            OVERLAY.set(Some(controller));
            window.show();
            WINDOW.set(Some(window));
            // Only the selected tab loads: the others wait until they are selected.
            show_selected();
        }

        /// Asks every browser to close; the window closes once they all have.
        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            let mut views: Vec<_> = [&SIDEBAR, &REVEAL_VIEW, &OVERLAY_VIEW]
                .into_iter()
                .filter_map(|view| view.with_borrow(Clone::clone))
                .collect();
            views.extend(CONTENTS.with_borrow(|contents| contents.values().flatten().cloned().collect::<Vec<_>>()));
            let mut closed = 1;
            for view in views {
                if let Some(host) = view.browser().and_then(|browser| browser.host()) {
                    closed &= host.try_close_browser();
                }
            }
            closed
        }

        // No title bar, like Arc: the window buttons sit in the sidebar's toolbar. Only macOS
        // keeps standard window buttons in a frameless window.
        fn is_frameless(&self, _window: Option<&mut Window>) -> i32 {
            cfg!(target_os = "macos").into()
        }

        // The window buttons are centered vertically in this height.
        fn titlebar_height(&self, _window: Option<&mut Window>, height: Option<&mut f32>) -> i32 {
            if let Some(height) = height {
                *height = TOOLBAR_HEIGHT;
            }
            1
        }

        fn with_standard_window_buttons(&self, _window: Option<&mut Window>) -> i32 { 1 }
        fn can_resize(&self, _window: Option<&mut Window>) -> i32 { 1 }
        fn can_maximize(&self, _window: Option<&mut Window>) -> i32 { 1 }
        fn can_minimize(&self, _window: Option<&mut Window>) -> i32 { 1 }

        fn accepts_first_mouse(&self, _window: Option<&mut Window>) -> State {
            State::ENABLED
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

/// An Alloy browser view. A width of 0 means "no preferred width": the layout decides. Without a
/// request context, it uses the global one, which only the app's own UI uses.
fn browser_view(
    mut client: Client,
    url: &str,
    width: i32,
    context: Option<&mut RequestContext>,
) -> BrowserView {
    browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        context,
        Some(&mut AlloyViewDelegate::new(width)),
    )
    .expect("browser view")
}

wrap_browser_view_delegate! {
    struct AlloyViewDelegate {
        width: i32,
    }

    impl ViewDelegate {
        // A zero height would make the size "empty" and CEF would ignore it.
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: self.width, height: 1 }
        }
    }

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

// Shows the app's own UI.
wrap_client! {
    struct UiClient {}

    impl Client {
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(UiRequestHandler::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(BrowserLifeSpan::new(None))
        }

        fn drag_handler(&self) -> Option<DragHandler> {
            Some(UiDragHandler::new())
        }
    }
}

// Lets the UI's `-webkit-app-region: drag` areas move the window.
wrap_drag_handler! {
    struct UiDragHandler {}

    impl DragHandler {
        // The sidebar sits at the window's origin: its coordinates are the window's. Only the
        // sidebar beside the page moves the window: the views above the page are separate
        // windows on macOS, which the window's regions do not reach.
        fn on_draggable_regions_changed(&self, browser: Option<&mut Browser>, _frame: Option<&mut Frame>, regions: Option<&[DraggableRegion]>) {
            let Some(mut sidebar) = SIDEBAR.with_borrow(|view| view.as_ref()?.browser()) else { return };
            if browser.is_none_or(|browser| browser.is_same(Some(&mut sidebar)) == 0) {
                return;
            }
            DRAG_REGIONS.set(regions.unwrap_or_default().to_vec());
            if SIDEBAR_MODE.get() == SidebarMode::Shown
                && let Some(window) = WINDOW.with_borrow(Clone::clone)
            {
                window.set_draggable_regions(regions);
            }
        }
    }
}

// Shows a profile's web pages.
wrap_client! {
    struct ContentClient {
        profile: ProfileId,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(BrowserLifeSpan::new(Some(self.profile)))
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(ContentDisplayHandler::new())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(ContentLoadHandler::new())
        }
    }
}

wrap_display_handler! {
    struct ContentDisplayHandler {}

    impl DisplayHandler {
        fn on_address_change(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if is_shown(browser) && frame.is_some_and(|frame| frame.is_main() == 1) {
                let url = url.map(CefString::to_string).unwrap_or_default();
                update_sidebar(|sidebar| {
                    if let Some(tab) = active_tab(sidebar) {
                        tab.url = url;
                    }
                });
            }
        }

        fn on_title_change(&self, browser: Option<&mut Browser>, title: Option<&CefString>) {
            if !is_shown(browser) {
                return;
            }
            let title = title.map(CefString::to_string).unwrap_or_default();
            update_sidebar(|sidebar| {
                if let Some(tab) = active_tab(sidebar) {
                    tab.title = title;
                }
            });
        }

        fn on_favicon_urlchange(&self, browser: Option<&mut Browser>, icon_urls: Option<&mut CefStringList>) {
            if !is_shown(browser) {
                return;
            }
            let favicon = icon_urls.and_then(|urls| urls.clone().into_iter().next());
            update_sidebar(|sidebar| {
                if let Some(tab) = active_tab(sidebar) {
                    tab.favicon = favicon;
                }
            });
        }

        fn on_loading_progress_change(&self, browser: Option<&mut Browser>, progress: f64) {
            if is_shown(browser) {
                update_page(|page| page.progress = progress);
            }
        }
    }
}

wrap_load_handler! {
    struct ContentLoadHandler {}

    impl LoadHandler {
        fn on_loading_state_change(&self, browser: Option<&mut Browser>, is_loading: i32, can_go_back: i32, can_go_forward: i32) {
            if !is_shown(browser) {
                return;
            }
            update_page(|page| {
                page.loading = is_loading == 1;
                page.can_go_back = can_go_back == 1;
                page.can_go_forward = can_go_forward == 1;
            });
        }
    }
}

// Quits the app when its last browser closes, and opens popups as tabs. `profile` is the profile
// of a content view's browser, `None` for the app's own UI.
wrap_life_span_handler! {
    struct BrowserLifeSpan {
        profile: Option<ProfileId>,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: i32,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: i32,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut i32>,
        ) -> i32 {
            if let Some(url) = target_url {
                open_tab(&url.to_string());
            }
            1
        }

        fn on_after_created(&self, browser: Option<&mut Browser>) {
            OPEN_BROWSERS.set(OPEN_BROWSERS.get() + 1);
            if let Some(profile) = self.profile {
                content_created(profile, browser);
            }
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            OPEN_BROWSERS.set(OPEN_BROWSERS.get() - 1);
            if OPEN_BROWSERS.get() == 0 {
                quit_message_loop();
            }
        }
    }
}

wrap_request_handler! {
    struct UiRequestHandler {}

    impl RequestHandler {
        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _is_navigation: i32,
            _is_download: i32,
            _request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut i32>,
        ) -> Option<ResourceRequestHandler> {
            let url = CefString::from(&request?.url()).to_string();
            url.starts_with(UI_ORIGIN).then(UiResourceRequestHandler::new)
        }
    }
}

wrap_resource_request_handler! {
    struct UiResourceRequestHandler {}

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            ReturnValue::CONTINUE
        }

        fn resource_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let url = CefString::from(&request?.url()).to_string();
            let path = ui_path(&url);
            let (mime, body) = match path.strip_prefix("command/").and_then(Command::parse) {
                Some(command) => {
                    post_task(ThreadId::UI, Some(&mut RunCommand::new(command)));
                    ("application/json", b"{}".as_slice())
                }
                None => ui_file(path)?,
            };
            let stream = stream_reader_create_for_data(body.as_ptr() as *mut u8, body.len());
            Some(StreamResourceHandler::new(200, "OK".to_string(), mime.to_string(), None, stream))
        }
    }
}

// Requests arrive on the IO thread; browsers are driven from the UI thread.
wrap_task! {
    struct RunCommand {
        command: Command,
    }

    impl Task {
        fn execute(&self) {
            run(self.command.clone());
        }    }
}

/// The path of a UI url. Request urls keep their fragment, like `index.html#overlay`.
fn ui_path(url: &str) -> &str {
    let path = &url[UI_ORIGIN.len()..];
    path.split('#').next().unwrap_or(path)
}

/// The built UI (`mise run ui`), embedded in the app.
fn ui_file(path: &str) -> Option<(&'static str, &'static [u8])> {
    match path {
        "index.html" => Some(("text/html", include_bytes!("../../ui/dist/index.html"))),
        "assets/index.js" => Some((
            "text/javascript",
            include_bytes!("../../ui/dist/assets/index.js"),
        )),
        "assets/index.css" => Some(("text/css", include_bytes!("../../ui/dist/assets/index.css"))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{UiState, ui_file, ui_path};

    #[test]
    fn sends_the_page_and_the_tabs() {
        let mut state = UiState::default();
        let home = state.sidebar.spaces()[0].id;
        let tab = state
            .sidebar
            .open_tab(home, "https://example.com/")
            .unwrap();
        state.sidebar.select(home, tab).unwrap();
        let json = state.to_json();
        assert_eq!(json["page"]["canGoBack"], false);
        assert_eq!(json["today"][0]["url"], "https://example.com/");
        assert_eq!(json["today"][0]["id"], json["active"]);
        let roots = serde_json::json!({
            "favorites": state.sidebar.profiles()[0].favorites,
            "pinned": state.sidebar.spaces()[0].pinned,
            "today": state.sidebar.spaces()[0].today,
        });
        assert_eq!(json["roots"], roots);
    }

    #[test]
    fn sends_pinned_folders_with_their_tabs() {
        let mut state = UiState::default();
        let home = state.sidebar.spaces()[0].clone();
        let folder = state.sidebar.add_folder(home.pinned, "Docs").unwrap();
        let tab = state
            .sidebar
            .open_tab(home.id, "https://example.com/")
            .unwrap();
        state.sidebar.move_node(tab, folder, 0).unwrap();
        let json = state.to_json();
        assert_eq!(json["pinned"][0]["title"], "Docs");
        assert_eq!(json["pinned"][0]["expanded"], true);
        assert_eq!(
            json["pinned"][0]["children"][0]["url"],
            "https://example.com/"
        );
        assert_eq!(json["today"], serde_json::json!([]));
    }

    #[test]
    fn sends_the_favorites_of_the_displayed_space_s_profile() {
        let mut state = UiState::default();
        let home = state.sidebar.spaces()[0].id;
        let favorites = state.sidebar.profiles()[0].favorites;
        let tab = state
            .sidebar
            .open_tab(home, "https://example.com/")
            .unwrap();
        state.sidebar.move_node(tab, favorites, 0).unwrap();
        let json = state.to_json();
        assert_eq!(json["favorites"][0]["url"], "https://example.com/");
        assert_eq!(json["favorites"][0]["id"], serde_json::json!(tab));
        assert_eq!(json["today"], serde_json::json!([]));
        state.sidebar.create_profile("Work").unwrap();
        let office = state.sidebar.spaces()[1].id;
        state.sidebar.display(office).unwrap();
        let json = state.to_json();
        assert_eq!(json["favorites"], serde_json::json!([]));
        let work_favorites = state.sidebar.profiles()[1].favorites;
        assert_eq!(
            json["roots"]["favorites"],
            serde_json::json!(work_favorites)
        );
    }

    #[test]
    fn sends_the_displayed_space_and_every_space() {
        let mut state = UiState::default();
        let home = state.sidebar.spaces()[0].id;
        let profile = state.sidebar.profiles()[0].id;
        let work = state.sidebar.add_space("Work", profile).unwrap();
        state
            .sidebar
            .open_tab(home, "https://example.com/")
            .unwrap();
        let tab = state
            .sidebar
            .open_tab(work, "https://example.org/")
            .unwrap();
        state.sidebar.select(work, tab).unwrap();
        state.sidebar.display(work).unwrap();
        state.sidebar.set_space_emoji(work, "💼").unwrap();
        state
            .sidebar
            .set_space_color(work, arbalette_core::sidebar::Color::Blue)
            .unwrap();
        let json = state.to_json();
        assert_eq!(json["space"], serde_json::json!(work));
        let spaces = serde_json::json!([
            { "id": home, "title": "Home", "emoji": null, "color": "purple" },
            { "id": work, "title": "Work", "emoji": "💼", "color": "blue" },
        ]);
        assert_eq!(json["spaces"], spaces);
        assert_eq!(json["today"][0]["url"], "https://example.org/");
        assert_eq!(json["today"].as_array().unwrap().len(), 1);
        assert_eq!(json["active"], serde_json::json!(tab));
        assert_eq!(
            json["roots"]["today"],
            serde_json::json!(state.sidebar.spaces()[1].today)
        );
    }

    #[test]
    fn sends_every_profile_with_its_name_and_color() {
        let mut state = UiState::default();
        let work = state.sidebar.create_profile("Work").unwrap();
        state
            .sidebar
            .set_profile_color(work, arbalette_core::sidebar::Color::Blue)
            .unwrap();
        let json = state.to_json();
        let default = state.sidebar.profiles()[0].id;
        let profiles = serde_json::json!([
            { "id": default, "name": "Default", "color": "purple" },
            { "id": work, "name": "Work", "color": "blue" },
        ]);
        assert_eq!(json["profiles"], profiles);
    }

    #[test]
    fn serves_the_built_ui() {
        let (mime, body) = ui_file("index.html").unwrap();
        assert_eq!(mime, "text/html");
        assert!(String::from_utf8_lossy(body).contains("/assets/index.js"));
        assert!(ui_file("assets/index.js").is_some());
        assert!(ui_file("assets/index.css").is_some());
    }

    #[test]
    fn serves_nothing_else() {
        assert!(ui_file("../Cargo.toml").is_none());
    }

    #[test]
    fn ignores_the_fragment() {
        assert_eq!(
            ui_path("https://arbalette.ui/index.html#overlay"),
            "index.html"
        );
        assert_eq!(
            ui_path("https://arbalette.ui/command/open/a%23b"),
            "command/open/a%23b"
        );
    }
}
