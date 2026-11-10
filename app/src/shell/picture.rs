//! Picture in Picture, like Arc's: a tab left while its video plays with sound goes on playing in
//! Chromium's floating video window, which closes as the tab shows again.

use super::*;

/// Puts a tab's page just hidden in Picture in Picture, once the app is done laying out: a page
/// only hidden while a split is laid out again shows again before.
pub(super) fn page_hidden(tab: NodeId) {
    post_task(ThreadId::UI, Some(&mut EnterPicture::new(tab)));
}

/// Opens the window on the page's video playing with sound, unless the page shows one already,
/// and closes it as the page shows again. Chromium only opens it for a user's gesture: the request
/// stands for the user's, who left the tab.
const ENTER_PICTURE: &str = r#"(async () => {
    if (document.pictureInPictureElement || !document.pictureInPictureEnabled) return;
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
})().catch(() => {})"#;

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
            let host = PAGES.with_borrow(|pages| {
                let live = pages.get(&self.tab).filter(|live| live.hidden_since.is_some())?;
                live.browser.host()
            });
            let Some(host) = host else { return };
            let params = json!({ "expression": ENTER_PICTURE, "userGesture": true });
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
    }
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
