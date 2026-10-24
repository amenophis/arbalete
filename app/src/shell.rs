//! The main window and the UI it shows.
//!
//! The `cef` crate's default callbacks return 0, which means "no" or "cancel": every callback the app
//! relies on is implemented explicitly.

use crate::command::Command;
use arbalette_core::sidebar::{NodeId, NodeKind, Sidebar, Tab};
use arbalette_core::store;
use cef::wrapper::stream_resource_handler::StreamResourceHandler;
use cef::*;
use serde::Serialize;
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::thread::LocalKey;

/// The UI is served by the app itself under this origin.
const UI_ORIGIN: &str = "https://arbalette.ui/";

const SIDEBAR_WIDTH: i32 = 260;

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
    static CONTENT: RefCell<Option<BrowserView>> = const { RefCell::new(None) };
    /// Shown above the page, to open tabs.
    static OVERLAY_VIEW: RefCell<Option<BrowserView>> = const { RefCell::new(None) };
    static OVERLAY: RefCell<Option<OverlayController>> = const { RefCell::new(None) };
    static STATE: RefCell<UiState> = RefCell::default();
    /// Where the sidebar is saved, once the previous session is restored.
    static SESSION_FILE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    /// Browsers not closed yet: the app quits when the last one closes.
    static OPEN_BROWSERS: Cell<usize> = const { Cell::new(0) };
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
    /// Sent whole to the UI on every change. Only the first Space is shown for now.
    fn to_json(&self) -> serde_json::Value {
        let home = &self.sidebar.spaces()[0];
        json!({
            "page": self.page,
            "pinned": self.items(home.pinned),
            "today": self.items(home.today),
            "active": home.selected,
        })
    }

    /// The tabs and folders inside a container or a folder, folders with their own content.
    fn items(&self, parent: NodeId) -> Vec<serde_json::Value> {
        let node = self.sidebar.node(parent).expect("a known container");
        node.children
            .iter()
            .filter_map(|&id| match &self.sidebar.node(id)?.kind {
                NodeKind::Tab(tab) => Some(json!({
                    "id": id,
                    "url": tab.url,
                    "title": tab.title,
                    "favicon": tab.favicon,
                })),
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
    dispatch(
        &SIDEBAR,
        &format!(r#"new CustomEvent("arbalette:state", {{ detail: {json} }})"#),
    );
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

/// Loads the sidebar saved by the previous session; a file that cannot be read is replaced on the
/// next save.
fn restore(path: PathBuf) {
    match store::load(&path) {
        Ok(Some(sidebar)) => STATE.with_borrow_mut(|state| state.sidebar = sidebar),
        Ok(None) => {}
        Err(error) => eprintln!("cannot restore the session: {error:?}"),
    }
    SESSION_FILE.set(Some(path));
    if STATE.with_borrow_mut(|state| active_tab(&mut state.sidebar).is_none()) {
        open_tab(START_URL);
    }
}

fn active_tab(sidebar: &mut Sidebar) -> Option<&mut Tab> {
    let id = sidebar.spaces()[0].selected?;
    sidebar.tab_mut(id)
}

/// Opens a tab in the first Space and shows it.
fn open_tab(url: &str) {
    update_sidebar(|sidebar| {
        let home = sidebar.spaces()[0].id;
        let tab = sidebar.open_tab(home, url).expect("the first Space");
        sidebar.select(home, tab).expect("a new tab");
    });
    load(url);
}

fn select_tab(id: NodeId) {
    let url = update_sidebar(|sidebar| {
        let home = sidebar.spaces()[0].id;
        sidebar.select(home, id).ok()?;
        Some(sidebar.tab_mut(id)?.url.clone())
    });
    if let Some(url) = url {
        load(&url);
    }
}

/// Closes a tab; closing the selected one shows the next tab, or the overlay when none is left.
fn close_tab(tab: Option<NodeId>) {
    let (selected, next) = STATE.with_borrow_mut(|state| {
        let selected = state.sidebar.spaces()[0].selected;
        let tab = tab
            .or(selected)
            .filter(|&id| state.sidebar.tab_mut(id).is_some());
        (
            selected,
            tab.map(|tab| (tab, state.sidebar.next_after_close(tab))),
        )
    });
    let Some((tab, next)) = next else { return };
    STATE.with_borrow_mut(|state| {
        let closed = state.sidebar.tab_mut(tab).expect("an open tab").clone();
        state.closed.push(closed);
    });
    update_sidebar(|sidebar| sidebar.remove(tab)).expect("an open tab");
    if selected == Some(tab) {
        match next {
            Some(next) => select_tab(next),
            None => {
                load("about:blank");
                show_overlay(true);
            }
        }
    }
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

fn load(url: &str) {
    if let Some(frame) = CONTENT.with_borrow(|view| view.as_ref()?.browser()?.main_frame()) {
        frame.load_url(Some(&CefString::from(url)));
    }
}

/// Runs a command from the UI or a menu, on the UI thread.
pub fn run(command: Command) {
    let browser = CONTENT.with_borrow(|view| view.as_ref()?.browser());
    match (command, browser) {
        (Command::Ready, _) => send_state(),
        (Command::Select(id), _) => select_tab(id),
        (Command::ShowOverlay, _) => show_overlay(true),
        (Command::HideOverlay, _) => show_overlay(false),
        (Command::Open(url), _) => {
            show_overlay(false);
            open_tab(&url);
        }
        (Command::Close(tab), _) => close_tab(tab),
        (Command::Reopen, _) => reopen_tab(),
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
        session_file: PathBuf,
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
            Some(ArbaletteBrowserProcessHandler::new(self.session_file.clone()))
        }
    }
}

wrap_browser_process_handler! {
    struct ArbaletteBrowserProcessHandler {
        session_file: PathBuf,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            restore(self.session_file.clone());
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
            let sidebar = browser_view(UiClient::new(), &format!("{UI_ORIGIN}index.html"), SIDEBAR_WIDTH);
            // Only the active tab loads: the others wait until they are selected.
            let url = STATE.with_borrow_mut(|state| Some(active_tab(&mut state.sidebar)?.url.clone()));
            let content = browser_view(ContentClient::new(), &url.expect("restore selects a tab"), 0);
            SIDEBAR.set(Some(sidebar.clone()));
            CONTENT.set(Some(content.clone()));
            for (view, flex) in [(sidebar, 0), (content, 1)] {
                let mut view = View::from(&view);
                window.add_child_view(Some(&mut view));
                layout.set_flex_for_view(Some(&mut view), flex);
            }
            let overlay = browser_view(UiClient::new(), &format!("{UI_ORIGIN}index.html#overlay"), 0);
            let controller = window
                .add_overlay_view(Some(&mut View::from(&overlay)), DockingMode::CUSTOM, 1)
                .expect("overlay");
            controller.set_visible(0);
            OVERLAY_VIEW.set(Some(overlay));
            OVERLAY.set(Some(controller));
            window.show();
            WINDOW.set(Some(window));
        }

        /// Asks every browser to close; the window closes once they all have.
        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            let mut closed = 1;
            for view in [&SIDEBAR, &CONTENT, &OVERLAY_VIEW] {
                if let Some(host) = view.with_borrow(|view| view.as_ref()?.browser()?.host()) {
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

/// An Alloy browser view. A width of 0 means "no preferred width": the layout decides.
fn browser_view(mut client: Client, url: &str, width: i32) -> BrowserView {
    browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        None,
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
            Some(BrowserLifeSpan::new())
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
        // The sidebar sits at the window's origin: its coordinates are the window's.
        fn on_draggable_regions_changed(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>, regions: Option<&[DraggableRegion]>) {
            if let Some(window) = WINDOW.with_borrow(Clone::clone) {
                window.set_draggable_regions(regions);
            }
        }
    }
}

// Shows web pages.
wrap_client! {
    struct ContentClient {}

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(BrowserLifeSpan::new())
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
        fn on_address_change(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if frame.is_some_and(|frame| frame.is_main() == 1) {
                let url = url.map(CefString::to_string).unwrap_or_default();
                update_sidebar(|sidebar| {
                    if let Some(tab) = active_tab(sidebar) {
                        tab.url = url;
                    }
                });
            }
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            let title = title.map(CefString::to_string).unwrap_or_default();
            update_sidebar(|sidebar| {
                if let Some(tab) = active_tab(sidebar) {
                    tab.title = title;
                }
            });
        }

        fn on_favicon_urlchange(&self, _browser: Option<&mut Browser>, icon_urls: Option<&mut CefStringList>) {
            let favicon = icon_urls.and_then(|urls| urls.clone().into_iter().next());
            update_sidebar(|sidebar| {
                if let Some(tab) = active_tab(sidebar) {
                    tab.favicon = favicon;
                }
            });
        }

        fn on_loading_progress_change(&self, _browser: Option<&mut Browser>, progress: f64) {
            update_page(|page| page.progress = progress);
        }
    }
}

wrap_load_handler! {
    struct ContentLoadHandler {}

    impl LoadHandler {
        fn on_loading_state_change(&self, _browser: Option<&mut Browser>, is_loading: i32, can_go_back: i32, can_go_forward: i32) {
            update_page(|page| {
                page.loading = is_loading == 1;
                page.can_go_back = can_go_back == 1;
                page.can_go_forward = can_go_forward == 1;
            });
        }
    }
}

// Quits the app when its last browser closes, and opens popups as tabs.
wrap_life_span_handler! {
    struct BrowserLifeSpan {}

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

        fn on_after_created(&self, _browser: Option<&mut Browser>) {
            OPEN_BROWSERS.set(OPEN_BROWSERS.get() + 1);
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
