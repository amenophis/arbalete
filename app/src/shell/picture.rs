//! Picture in Picture, like Arc's: a tab left during a call, like Google Meet's, goes on in the
//! page's own Picture in Picture window, and one left while its video plays with sound in
//! Chromium's floating video window; either closes as the tab shows again.

use super::*;

/// Puts a tab's page just hidden in Picture in Picture, once the app is done laying out: a page
/// only hidden while a split is laid out again shows again before.
pub(super) fn page_hidden(tab: NodeId) {
    post_task(ThreadId::UI, Some(&mut EnterPicture::new(tab)));
}

/// Keeps the handler a page gives the Media Session's `enterpictureinpicture` action, which
/// Chromium runs as a call's tab is left and Google Meet answers with its Picture in Picture
/// window. Like Chromium's, the window goes with its page.
const KEEP_ENTER_PICTURE: &str = r#"(() => {
    const session = navigator.mediaSession;
    if (!session || !self.documentPictureInPicture || "__arbaleteEnterPicture" in session) return;
    let enter = null;
    Object.defineProperty(session, "__arbaleteEnterPicture", { get: () => enter });
    const set = session.setActionHandler.bind(session);
    session.setActionHandler = (action, handler) => {
        if (action === "enterpictureinpicture") enter = handler;
        return set(action, handler);
    };
    addEventListener("pagehide", () => documentPictureInPicture.window?.close());
})()"#;

/// Starts keeping a tab's page's `enterpictureinpicture` handler, in its documents to come and
/// its current one.
pub(super) fn picture_page_created(browser: &Browser) {
    let Some(host) = browser.host() else { return };
    // New documents run the scripts added for them only while the Page domain is enabled.
    let params = json!({ "source": KEEP_ENTER_PICTURE, "runImmediately": true });
    for message in [
        json!({ "id": 0, "method": "Page.enable" }),
        json!({ "id": 0, "method": "Page.addScriptToEvaluateOnNewDocument", "params": params }),
    ] {
        host.send_dev_tools_message(Some(message.to_string().as_bytes()));
    }
}

/// During a call (`call`), runs the page's `enterpictureinpicture` handler, like Chromium's
/// automatic Picture in Picture; otherwise opens the video window on its video playing with
/// sound. Not when the page shows a window already, and the window it opens closes as the page
/// shows again. Chromium only opens either for a user's gesture: the request stands for the
/// user's, who left the tab.
const ENTER_PICTURE: &str = r#"(async (call) => {
    const pictures = self.documentPictureInPicture;
    if (document.pictureInPictureElement || pictures?.window) return;
    const enter = navigator.mediaSession?.__arbaleteEnterPicture;
    if (call && enter) {
        enter({ action: "enterpictureinpicture" });
        const shown = () => {
            if (document.hidden) return;
            document.removeEventListener("visibilitychange", shown);
            pictures.window?.close();
        };
        return document.addEventListener("visibilitychange", shown);
    }
    if (!document.pictureInPictureEnabled) return;
    const video = [...document.querySelectorAll("video")].find((video) =>
        !video.paused && !video.ended && !video.muted && video.volume > 0
            && video.readyState > 2 && !video.disablePictureInPicture);
    if (!video) return;
    await video.requestPictureInPicture();
    const shown = () => {
        if (document.hidden) return;
        document.removeEventListener("visibilitychange", shown);
        if (document.pictureInPictureElement === video) document.exitPictureInPicture();
    };
    document.addEventListener("visibilitychange", shown);
})(CALL).catch(() => {})"#;

/// Lets a site's videos go on in Picture in Picture as its tabs are left, or no longer, from its
/// site controls.
pub(super) fn allow_picture(site: String, on: bool) {
    SETTINGS.with_borrow_mut(|settings| {
        settings.no_picture_sites.retain(|blocked| *blocked != site);
        if !on {
            settings.no_picture_sites.push(site);
            settings.no_picture_sites.sort();
        }
    });
    save_settings();
    if let Some(url) = selected_url() {
        send_site_controls(&url);
    }
}

wrap_task! {
    struct EnterPicture {
        tab: NodeId,
    }

    impl Task {
        fn execute(&self) {
            let settings = app_settings();
            let blocked = tab_url(self.tab).is_some_and(|url| {
                settings.no_picture_sites.iter().any(|blocked| *blocked == site(&url))
            });
            if !settings.picture_in_picture || blocked {
                return;
            }
            let page = PAGES.with_borrow(|pages| {
                let live = pages.get(&self.tab).filter(|live| live.hidden_since.is_some())?;
                Some((live.browser.host()?, live.capturing))
            });
            let Some((host, call)) = page else { return };
            let expression = ENTER_PICTURE.replace("(CALL)", &format!("({call})"));
            let params = json!({ "expression": expression, "userGesture": true });
            let message = json!({ "id": 0, "method": "Runtime.evaluate", "params": params });
            host.send_dev_tools_message(Some(message.to_string().as_bytes()));
        }
    }
}

/// A page's Picture in Picture window, which the Document Picture-in-Picture API opens, like
/// Google Meet's: its page floats in a native window, which its bar takes back to the tab.
#[cfg(target_os = "macos")]
struct Picture {
    id: u32,
    /// The popup CEF makes it from, until made.
    popup: i32,
    /// The tab whose page opened it.
    tab: NodeId,
    browser: Option<Browser>,
}

#[cfg(target_os = "macos")]
thread_local! {
    static PICTURES: RefCell<Vec<Picture>> = const { RefCell::new(Vec::new()) };
    static NEXT_PICTURE: Cell<u32> = const { Cell::new(0) };
}

/// The size of a Picture in Picture window's page: CEF does not tell the one the page asks for,
/// and the window keeps the size the user gives it.
#[cfg(target_os = "macos")]
const PICTURE_SIZE: (i32, i32) = (400, 300);

/// Opens a Picture in Picture window for a tab's page, `opener`, and makes CEF's popup `popup`
/// its page; none for a page not a tab's.
#[cfg(target_os = "macos")]
pub(super) fn open_picture(
    opener: &Browser,
    popup: i32,
    window_info: &mut WindowInfo,
    client: &mut Option<Client>,
) -> bool {
    let Some(tab) = page_tab(opener) else {
        return false;
    };
    let id = NEXT_PICTURE.get();
    NEXT_PICTURE.set(id + 1);
    let (width, height) = PICTURE_SIZE;
    let Some(view) = crate::native::open_picture(id, width, height) else {
        return false;
    };
    *window_info = WindowInfo {
        parent_view: view,
        bounds: Rect {
            x: 0,
            y: 0,
            width,
            height,
        },
        runtime_style: RuntimeStyle::ALLOY,
        ..Default::default()
    };
    *client = Some(PictureClient::new(id));
    PICTURES.with_borrow_mut(|pictures| {
        pictures.push(Picture {
            id,
            popup,
            tab,
            browser: None,
        })
    });
    true
}

/// A popup CEF did not make after all leaves its window.
#[cfg(target_os = "macos")]
pub(super) fn picture_aborted(popup: i32) {
    let id = PICTURES.with_borrow_mut(|pictures| {
        let at = pictures
            .iter()
            .position(|p| p.browser.is_none() && p.popup == popup)?;
        Some(pictures.remove(at).id)
    });
    if let Some(id) = id {
        crate::native::picture_closed(id);
    }
}

/// Whether the Picture in Picture window `id` may close now: its page closes first, which then
/// closes the window.
#[cfg(target_os = "macos")]
pub fn picture_can_close(id: u32) -> bool {
    let Some(host) = picture_host(id) else {
        return true;
    };
    host.close_browser(0);
    false
}

/// Shows the tab of the Picture in Picture window `id`, in front, and closes the window, like
/// Chromium's back to tab button.
#[cfg(target_os = "macos")]
pub fn picture_back(id: u32) {
    let tab = PICTURES.with_borrow(|pictures| pictures.iter().find(|p| p.id == id).map(|p| p.tab));
    if let Some(tab) = tab.filter(|&tab| tab_url(tab).is_some()) {
        crate::mac::activate();
        select_tab(tab);
        focus_page(tab);
    }
    if let Some(host) = picture_host(id) {
        host.close_browser(0);
    }
}

/// Shows a tab in front and closes its Picture in Picture windows, like their back to tab button;
/// but the one showing `asking`'s page, whose request would go with it.
#[cfg(target_os = "macos")]
pub(super) fn back_to_tab(tab: NodeId, asking: &Browser) {
    crate::mac::activate();
    select_tab(tab);
    focus_page(tab);
    let hosts: Vec<_> = PICTURES.with_borrow(|pictures| {
        pictures
            .iter()
            .filter(|p| p.tab == tab)
            .filter_map(|p| p.browser.as_ref())
            .filter(|browser| browser.identifier() != asking.identifier())
            .filter_map(Browser::host)
            .collect()
    });
    for host in hosts {
        host.close_browser(0);
    }
}

#[cfg(target_os = "macos")]
fn picture_host(id: u32) -> Option<BrowserHost> {
    PICTURES.with_borrow(|pictures| {
        pictures
            .iter()
            .find(|p| p.id == id)?
            .browser
            .as_ref()?
            .host()
    })
}

// A Picture in Picture window's page, which its opener fills.
#[cfg(target_os = "macos")]
wrap_client! {
    struct PictureClient {
        id: u32,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(PictureLifeSpan::new(self.id))
        }

        /// Its page asks like its opener's, like Google Meet's button to share the screen.
        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(ContentPermissions::new())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(PictureDisplay::new())
        }
    }
}

#[cfg(target_os = "macos")]
wrap_display_handler! {
    struct PictureDisplay {}

    impl DisplayHandler {
        /// Its page sharing the screen shows the bar to end it, like its opener's.
        fn on_media_access_change(&self, browser: Option<&mut Browser>, _has_video_access: i32, _has_audio_access: i32) {
            if let Some(browser) = browser {
                media_changed(browser);
            }
        }
    }
}

/// The tab whose page opened the Picture in Picture window showing `browser`'s page, if it
/// shows in one.
#[cfg(target_os = "macos")]
pub(super) fn picture_opener(browser: &Browser) -> Option<NodeId> {
    let id = browser.identifier();
    PICTURES.with_borrow(|pictures| {
        pictures
            .iter()
            .find(|p| p.browser.as_ref().is_some_and(|b| b.identifier() == id))
            .map(|p| p.tab)
    })
}

/// A tab's page's, or, for a Picture in Picture window's page, its opener's tab.
pub(super) fn owning_tab(browser: &Browser) -> Option<NodeId> {
    #[cfg(target_os = "macos")]
    if let Some(tab) = picture_opener(browser) {
        return Some(tab);
    }
    page_tab(browser)
}

#[cfg(target_os = "macos")]
wrap_life_span_handler! {
    struct PictureLifeSpan {
        id: u32,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            OPEN_BROWSERS.set(OPEN_BROWSERS.get() + 1);
            PICTURES.with_borrow_mut(|pictures| {
                if let Some(picture) = pictures.iter_mut().find(|p| p.id == self.id) {
                    picture.browser = browser.map(|browser| browser.clone());
                }
            });
        }

        /// Its page closing, from its opener or the window's close button, closes its window,
        /// which CEF would otherwise close on its own.
        fn do_close(&self, _browser: Option<&mut Browser>) -> i32 {
            post_task(ThreadId::UI, Some(&mut RemovePicture::new(self.id)));
            1
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            browser_closed();
        }
    }
}

#[cfg(target_os = "macos")]
wrap_task! {
    struct RemovePicture {
        id: u32,
    }

    impl Task {
        fn execute(&self) {
            PICTURES.with_borrow_mut(|pictures| pictures.retain(|p| p.id != self.id));
            crate::native::picture_closed(self.id);
        }
    }
}
