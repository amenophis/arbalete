//! The prompt asking the user before a site uses the camera, the microphone or the like, or opens
//! another app (shell::app_links).

use super::*;

/// A site's request for permissions, waiting for the user's answer in the permission prompt.
pub(super) struct PermissionRequest {
    /// What the prompt's answer names it by.
    id: u64,
    browser: Browser,
    /// The window in front as it came, which shows its prompt unless the page is a tab's.
    window: u32,
    /// The origin asking, like `https://meet.google.com/`, and its site, `meet.google.com`.
    origin: String,
    site: String,
    asks: Vec<&'static permissions::Capability>,
    callback: PermissionCallback,
}

/// What learns the user's answer to a permission request.
enum PermissionCallback {
    /// For the camera or the microphone: the media types asked for.
    Media(MediaAccessCallback, u32),
    /// For sharing the screen, a window or a tab, which the picker asks (screen_share): the media
    /// types asked for, the screen's sound among them.
    Screen(MediaAccessCallback, u32),
    /// For anything else, Chromium's prompt by its id, and the permissions it asks for.
    Prompt(u64, u32, PermissionPromptCallback),
    /// For another app's link: the link, the app's name, and whether its page opened only for it.
    AppLink {
        url: String,
        app: String,
        blank: bool,
    },
}

thread_local! {
    /// The origin and the permissions of the prompt being answered, while Chromium learns the
    /// answer.
    static ANSWERING: RefCell<Option<(String, u32)>> = const { RefCell::new(None) };
}

// Asks the user before a site uses the camera, the microphone, the screen or anything else
// Chromium guards, like Arc: CEF denies them all otherwise.
wrap_permission_handler! {
    pub(super) struct ContentPermissions {}

    impl PermissionHandler {
        fn on_request_media_access_permission(
            &self,
            browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut MediaAccessCallback>,
        ) -> i32 {
            let (Some(browser), Some(callback)) = (browser, callback) else { return 0 };
            let origin = requesting_origin.map(CefString::to_string).unwrap_or_default();
            let asks = permissions::media(requested_permissions);
            if asks.is_empty() {
                return 0;
            }
            // Sharing the screen asks what to share, each time.
            if requested_permissions & MediaAccessPermissionTypes::DESKTOP_VIDEO_CAPTURE.get_raw() != 0 {
                let callback = PermissionCallback::Screen(callback.clone(), requested_permissions);
                send_share_snapshots(browser);
                ask_permission(browser, &origin, asks, callback);
                return 1;
            }
            // Chromium keeps no decision for media requests: the profile's content settings do,
            // where the page's permission queries read it too.
            if let Some(allow) = kept_decision(browser, &origin, &asks) {
                callback.cont(if allow { requested_permissions } else { 0 });
                return 1;
            }
            let callback = PermissionCallback::Media(callback.clone(), requested_permissions);
            ask_permission(browser, &origin, asks, callback);
            1
        }

        fn on_show_permission_prompt(
            &self,
            browser: Option<&mut Browser>,
            prompt_id: u64,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> i32 {
            let (Some(browser), Some(callback)) = (browser, callback) else { return 0 };
            let origin = requesting_origin.map(CefString::to_string).unwrap_or_default();
            let asks = permissions::prompted(requested_permissions);
            if asks.is_empty() {
                return 0;
            }
            // Once allowed, a page's permission button, like Meet's for the camera and the
            // microphone, shows Chromium's next screen for the same request, like the one waiting
            // for macOS's own prompt: the request is decided, and accepting it again would crash.
            let follows = ANSWERING.with_borrow(|answering| {
                answering.as_ref().is_some_and(|(answered, permissions)| {
                    *answered == origin && requested_permissions & !permissions == 0
                })
            });
            if follows {
                callback.cont(PermissionRequestResult::IGNORE);
                return 1;
            }
            let callback = PermissionCallback::Prompt(prompt_id, requested_permissions, callback.clone());
            ask_permission(browser, &origin, asks, callback);
            1
        }

        /// Chromium took the prompt back, as its page navigated or closed.
        fn on_dismiss_permission_prompt(&self, _browser: Option<&mut Browser>, prompt_id: u64, _result: PermissionRequestResult) {
            PERMISSION_REQUESTS.with_borrow_mut(|requests| {
                requests.retain(|request| {
                    !matches!(request.callback, PermissionCallback::Prompt(id, ..) if id == prompt_id)
                });
            });
            surfaces_changed();
        }
    }
}

/// Keeps a site's request until the user answers it in the permission prompt.
fn ask_permission(
    browser: &Browser,
    origin: &str,
    asks: Vec<&'static permissions::Capability>,
    callback: PermissionCallback,
) {
    let id = NEXT_PERMISSION_REQUEST.get();
    NEXT_PERMISSION_REQUEST.set(id + 1);
    let request = PermissionRequest {
        id,
        browser: browser.clone(),
        window: FOCUSED.get(),
        origin: origin.to_string(),
        site: site(origin).to_string(),
        asks,
        callback,
    };
    PERMISSION_REQUESTS.with_borrow_mut(|requests| requests.push(request));
    surfaces_changed();
}

/// Asks the user before `browser`'s page, from `origin`, opens `url` in `app`: once at a time per
/// page, which a page asking again and again cannot pile up.
pub(super) fn ask_app_link(browser: &Browser, origin: &str, url: String, app: String, blank: bool) {
    let asking = PERMISSION_REQUESTS.with_borrow(|requests| {
        requests
            .iter()
            .any(|request| request.browser.identifier() == browser.identifier())
    });
    if asking {
        return;
    }
    let callback = PermissionCallback::AppLink { url, app, blank };
    ask_permission(browser, origin, Vec::new(), callback);
}

/// Gives the site the user's answer to its request `id`, from the permission prompt.
pub(super) fn answer_permission(id: u64, allow: bool) {
    let request = PERMISSION_REQUESTS.with_borrow_mut(|requests| {
        let index = requests.iter().position(|request| request.id == id)?;
        Some(requests.remove(index))
    });
    let Some(request) = request else { return };
    match request.callback {
        PermissionCallback::Media(callback, requested) => {
            keep_decision(&request.browser, &request.origin, &request.asks, allow);
            callback.cont(if allow { requested } else { 0 });
        }
        // Shared only once picked (share_screen).
        PermissionCallback::Screen(callback, _) => callback.cont(0),
        PermissionCallback::Prompt(_, requested, callback) => {
            ANSWERING.set(Some((request.origin, requested)));
            callback.cont(if allow {
                PermissionRequestResult::ACCEPT
            } else {
                PermissionRequestResult::DENY
            });
            ANSWERING.set(None);
        }
        PermissionCallback::AppLink { url, blank, .. } => {
            #[cfg(target_os = "macos")]
            if allow {
                crate::mac::open_in_app(&url);
            }
            if blank {
                close_blank_page(&request.browser);
            }
        }
    }
    surfaces_changed();
}

/// Gives each window's picker the pictures of the tabs it offers to share, once as it opens: the
/// prompt's state, sent again and again, leaves them out.
fn send_share_snapshots(asking: &Browser) {
    let ids: Vec<NodeId> = screen_share::shareable_tabs(asking)
        .iter()
        .filter_map(|tab| serde_json::from_value(tab["id"].clone()).ok())
        .collect();
    let snapshots: serde_json::Map<_, _> = SNAPSHOTS.with_borrow(|snapshots| {
        ids.iter()
            .filter_map(|id| {
                let jpeg = snapshots.get(id)?;
                Some((serde_json::to_string(id).ok()?, json!(jpeg)))
            })
            .collect()
    });
    let detail = serde_json::Value::Object(snapshots).to_string();
    in_each_window(|| send_surface("permission", "arbalete:share-snapshots", &detail));
}

/// Gives the site the source the user picked to share for its request `id`, from the picker, and
/// whether with its sound; or turns the request down, without a source.
pub(super) fn share_screen(id: u64, source: Option<String>, sound: bool) {
    let request = PERMISSION_REQUESTS.with_borrow_mut(|requests| {
        let index = requests.iter().position(|request| {
            request.id == id && matches!(request.callback, PermissionCallback::Screen(..))
        })?;
        Some(requests.remove(index))
    });
    let Some(PermissionRequest {
        browser,
        callback: PermissionCallback::Screen(callback, requested),
        ..
    }) = request
    else {
        return;
    };
    let chosen = source.is_some_and(|source| screen_share::choose(&browser, &source));
    let sound_off = if sound {
        0
    } else {
        MediaAccessPermissionTypes::DESKTOP_AUDIO_CAPTURE.get_raw()
    };
    callback.cont(if chosen { requested & !sound_off } else { 0 });
    surfaces_changed();
}

/// The decision the profile keeps on `origin` using all of `asks`: allowed once each is, blocked
/// once any is; none while one is undecided, or asked each time, like sharing the screen.
fn kept_decision(
    browser: &Browser,
    origin: &str,
    asks: &[&permissions::Capability],
) -> Option<bool> {
    let context = browser.host()?.request_context()?;
    let origin = CefString::from(origin);
    let mut allowed = true;
    for ask in asks {
        let value = context.content_setting(Some(&origin), Some(&origin), ask.setting?);
        if value == ContentSettingValues::BLOCK {
            return Some(false);
        }
        allowed &= value == ContentSettingValues::ALLOW;
    }
    allowed.then_some(true)
}

/// Keeps the user's decision on `origin` using `asks`, where the profile keeps one.
fn keep_decision(browser: &Browser, origin: &str, asks: &[&permissions::Capability], allow: bool) {
    let Some(context) = browser.host().and_then(|host| host.request_context()) else {
        return;
    };
    let origin = CefString::from(origin);
    let value = if allow {
        ContentSettingValues::ALLOW
    } else {
        ContentSettingValues::BLOCK
    };
    for setting in asks.iter().filter_map(|ask| ask.setting) {
        context.set_content_setting(Some(&origin), Some(&origin), setting, value);
    }
}

/// Whether the window shows a request's page: its tab's, shown alone or in a split; else, a page
/// that is no tab's, like Peek's, in the window in front as the request came.
fn shows_request(request: &PermissionRequest) -> bool {
    shows_page(&request.browser, request.window)
}

/// Whether the window shows `browser`'s page: its tab's, shown alone or in a split; else, a page
/// that is no tab's, like Peek's, in the window `window`.
pub(super) fn shows_page(browser: &Browser, window: u32) -> bool {
    match page_tab(browser) {
        Some(tab) => win().shown.get() == Some(tab) || win().split.borrow().0.contains(&tab),
        None => win().id == window,
    }
}

/// Shows the window the first request of a page it shows in the permission prompt, at the top
/// left of the card, over the page; or hides it. The requests of closed pages go.
#[cfg(target_os = "macos")]
pub(super) fn sync_permission_prompt() {
    PERMISSION_REQUESTS.with_borrow_mut(|requests| {
        requests.retain(|request| request.browser.is_valid() == 1);
    });
    let shown = PERMISSION_REQUESTS.with_borrow(|requests| {
        let request = requests.iter().find(|request| shows_request(request))?;
        if let PermissionCallback::AppLink { app, .. } = &request.callback {
            let asks = [json!({ "text": format!("Open {app}"), "symbol": "arrow.up.forward.app" })];
            let site = Some(request.site.as_str()).filter(|site| !site.is_empty());
            let site = site.unwrap_or("A page");
            return Some(json!({ "id": request.id, "site": site, "asks": asks, "app": true }));
        }
        if let PermissionCallback::Screen(_, requested) = &request.callback {
            let sound =
                requested & MediaAccessPermissionTypes::DESKTOP_AUDIO_CAPTURE.get_raw() != 0;
            let tabs = screen_share::shareable_tabs(&request.browser);
            let screen = json!({ "tabs": tabs, "sound": sound });
            return Some(
                json!({ "id": request.id, "site": request.site, "asks": [], "screen": screen }),
            );
        }
        let asks: Vec<_> = request
            .asks
            .iter()
            .map(|ask| json!({ "text": ask.asks, "symbol": ask.symbol }))
            .collect();
        Some(json!({ "id": request.id, "site": request.site, "asks": asks }))
    });
    let card = card_rect().unwrap_or_default();
    // The picker of what to share, wider, shows in the middle.
    let picker = shown
        .as_ref()
        .is_some_and(|shown| shown.get("screen").is_some());
    let width = if picker {
        SCREEN_PICKER_WIDTH.min(card.width - 2 * PERMISSION_INSET)
    } else {
        PERMISSION_WIDTH
    };
    let frame = Rect {
        x: if picker {
            card.x + (card.width - width) / 2
        } else {
            card.x + PERMISSION_INSET
        },
        y: card.y + PERMISSION_INSET,
        width,
        height: win().permission_height.get(),
    };
    let detail = shown.clone().unwrap_or_default();
    crate::native::event(
        win().id,
        "permission",
        "arbalete:permission",
        &detail.to_string(),
    );
    crate::native::surface(win().id, "permission", shown.is_some(), &frame);
}

/// The permission prompt's width, and how far inside the card's top left corner it shows.
pub(super) const PERMISSION_WIDTH: i32 = 300;
pub(super) const PERMISSION_INSET: i32 = 10;
/// The width of the picker of what to share.
const SCREEN_PICKER_WIDTH: i32 = 640;
