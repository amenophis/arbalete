//! The main window and the UI it shows.
//!
//! The `cef` crate's default callbacks return 0, which means "no" or "cancel": every callback the app
//! relies on is implemented explicitly.

use cef::wrapper::stream_resource_handler::StreamResourceHandler;
use cef::*;
use std::cell::RefCell;

/// The UI is served by the app itself under this origin.
const UI_ORIGIN: &str = "https://arbalette.ui/";

thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
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
    pub struct ArbaletteApp {}

    impl App {
        // Development builds are signed ad hoc, so each build would ask for keychain access.
        fn on_before_command_line_processing(&self, _process_type: Option<&CefString>, command_line: Option<&mut CommandLine>) {
            if cfg!(debug_assertions) && let Some(command_line) = command_line {
                command_line.append_switch(Some(&CefString::from("use-mock-keychain")));
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(ArbaletteBrowserProcessHandler::new())
        }
    }
}

wrap_browser_process_handler! {
    struct ArbaletteBrowserProcessHandler {}

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
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
            window.set_to_fill_layout();
            let ui = browser_view_create(
                Some(&mut UiClient::new()),
                Some(&CefString::from(format!("{UI_ORIGIN}index.html").as_str())),
                Some(&BrowserSettings::default()),
                None,
                None,
                Some(&mut UiViewDelegate::new()),
            )
            .expect("ui browser view");
            window.add_child_view(Some(&mut View::from(&ui)));
            window.show();
            WINDOW.set(Some(window));
        }

        fn can_close(&self, window: Option<&mut Window>) -> i32 {
            let browser = window
                .and_then(|w| w.child_view_at(0))
                .and_then(|view| view.as_browser_view())
                .and_then(|view| view.browser());
            match browser.and_then(|b| b.host()) {
                Some(host) => host.try_close_browser(),
                None => 1,
            }
        }

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

wrap_browser_view_delegate! {
    struct UiViewDelegate {}

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_client! {
    struct UiClient {}

    impl Client {
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(UiRequestHandler::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(UiLifeSpanHandler::new())
        }
    }
}

wrap_life_span_handler! {
    struct UiLifeSpanHandler {}

    impl LifeSpanHandler {
        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            quit_message_loop();
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
            let (mime, body) = ui_file(&url[UI_ORIGIN.len()..])?;
            let stream = stream_reader_create_for_data(body.as_ptr() as *mut u8, body.len());
            Some(StreamResourceHandler::new(200, "OK".to_string(), mime.to_string(), None, stream))
        }
    }
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
    use super::ui_file;

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
}
