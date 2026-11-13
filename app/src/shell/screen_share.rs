//! Sharing the screen, like Arc's: a site asking to share it shows the picker of the screens, the
//! windows and the tabs it can share (permission_prompt); a tab shared gives way to another from
//! that tab's row in the sidebar or the bar over the page, which also ends the sharing.
//! amenophis/cef-codecs' CEF shares the source picked and runs these controls from build 9
//! (patches/alloy-display-capture.patch in amenophis/cef-builds).

use super::*;

/// The profile's preference CEF takes the source picked from, once: "screen:<id>:0",
/// "window:<id>:0" or "arbalete-tab:<browser id>".
const SOURCE: &str = "arbalete.display_media_source";
/// The commands that end a page's screen sharing, and move it to the tab in `SOURCE`; whether
/// they can run tells whether it shares, and may move.
const STOP: i32 = 0x4152_4201;
const SWITCH: i32 = 0x4152_4202;

/// What a page shares.
#[derive(Clone, Copy, PartialEq)]
enum Shared {
    Screen,
    Window,
    Tab(NodeId),
}

/// The screen sharing going on, the last one started.
struct Sharing {
    /// The page sharing, like a call's.
    page: Browser,
    /// Its site, like `meet.google.com`, and its profile, whose tabs it may share.
    site: String,
    profile: ProfileId,
    shared: Shared,
    /// Chromium shares it: until then, and while it moves to another tab, it only was asked for.
    started: bool,
    moving: bool,
    /// The page lets it move to another tab (getDisplayMedia's surfaceSwitching).
    movable: bool,
    /// The user hid the bar over the page.
    hidden: bool,
}

thread_local! {
    static SHARING: RefCell<Option<Sharing>> = const { RefCell::new(None) };
}

/// Lets the app set the source picked in each profile's preferences.
pub(super) fn register_source(registrar: &mut PreferenceRegistrar) {
    if let Some(mut empty) = value_create() {
        empty.set_string(Some(&CefString::from("")));
        registrar.add_preference(Some(&CefString::from(SOURCE)), Some(&mut empty));
    }
}

/// Has `page`, of `site`, share `source`, from the picker: "screen:<id>:0", "window:<id>:0" or
/// "tab:<tab id>". Whether it is one the page can share.
pub(super) fn choose(page: &Browser, site: &str, source: &str) -> bool {
    let shared = if source.starts_with("screen:") {
        Shared::Screen
    } else if source.starts_with("window:") {
        Shared::Window
    } else {
        let tab = source.strip_prefix("tab:");
        match tab.and_then(|id| serde_json::from_str(id).ok()) {
            Some(tab) => Shared::Tab(tab),
            None => return false,
        }
    };
    let Some(profile) =
        page_tab(page).and_then(|tab| STATE.with_borrow(|state| state.sidebar.profile_of(tab)))
    else {
        return false;
    };
    if !set_source(page, shared, source) {
        return false;
    }
    SHARING.set(Some(Sharing {
        page: page.clone(),
        site: site.to_string(),
        profile,
        shared,
        started: false,
        moving: false,
        movable: false,
        hidden: false,
    }));
    true
}

/// Puts the source in the preference of `page`'s profile, a tab's as its page.
fn set_source(page: &Browser, shared: Shared, source: &str) -> bool {
    let source = match shared {
        Shared::Tab(tab) => match awake_page(tab) {
            Some(browser) => format!("arbalete-tab:{}", browser.identifier()),
            None => return false,
        },
        _ => source.to_string(),
    };
    let Some(context) = page.host().and_then(|host| host.request_context()) else {
        return false;
    };
    let Some(mut value) = value_create() else {
        return false;
    };
    value.set_string(Some(&CefString::from(source.as_str())));
    // Not empty: cef passes an empty string as none, which CEF turns down.
    let mut error = CefString::from(" ");
    let set = context.set_preference(
        Some(&CefString::from(SOURCE)),
        Some(&mut value),
        Some(&mut error),
    ) == 1;
    if !set {
        eprintln!("cannot set the screen sharing's source: {error}");
    }
    set
}

/// The live page of a tab.
fn tab_page(tab: NodeId) -> Option<Browser> {
    PAGES.with_borrow(|pages| Some(pages.get(&tab)?.browser.clone()))
}

/// A tab's page, loaded hidden if it was not, and running: a frozen page draws nothing to share.
fn awake_page(tab: NodeId) -> Option<Browser> {
    load_hidden(tab);
    let (browser, frozen) = PAGES.with_borrow_mut(|pages| {
        let live = pages.get_mut(&tab)?;
        Some((live.browser.clone(), std::mem::take(&mut live.frozen)))
    })?;
    if frozen {
        set_frozen(browser.host().as_ref(), false);
    }
    Some(browser)
}

/// Whether a tab is a web page's, which a screen sharing can show.
fn is_web(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// The tabs the picker offers, like Chrome's: web pages' of the asking page's profile, in every
/// Space, but the asking one's; the loaded ones first, then by title. A page asleep loads once
/// picked.
pub(super) fn shareable_tabs(asking: &Browser) -> Vec<serde_json::Value> {
    let asking = page_tab(asking);
    let mut tabs: Vec<_> = STATE.with_borrow(|state| {
        let sidebar = &state.sidebar;
        let own = asking.and_then(|tab| sidebar.profile_of(tab));
        sidebar
            .tabs()
            .filter(|(id, tab)| Some(*id) != asking && is_web(&tab.url))
            .filter_map(|(id, tab)| {
                let profile = sidebar
                    .profile_of(id)
                    .filter(|p| own.is_none_or(|own| own == *p))?;
                let json = json!({
                    "id": id,
                    "title": tab.label(),
                    "url": shown_url(&tab.url),
                    "favicon": favicon_url(profile, tab),
                });
                let loaded = tab_page(id).is_some();
                Some(((!loaded, tab.label().to_lowercase()), json))
            })
            .collect()
    });
    tabs.sort_by(|(a, _), (b, _)| a.cmp(b));
    tabs.into_iter().map(|(_, json)| json).collect()
}

/// Whether a tab's row shows the recording dot, like Arc's: its page uses the camera, the
/// microphone or the screen.
pub(super) fn recording(tab: NodeId) -> bool {
    PAGES.with_borrow(|pages| pages.get(&tab).is_some_and(|live| live.capturing))
}

/// Whether a screen sharing shows `tab`, whose row is framed in yellow, like Arc's.
pub(super) fn shows_tab(tab: NodeId) -> bool {
    started(|sharing| sharing.shared == Shared::Tab(tab)).unwrap_or(false)
}

/// Whether a tab is shared: its page stays awake while hidden.
pub(super) fn is_shared(tab: NodeId) -> bool {
    SHARING.with_borrow(|current| {
        current
            .as_ref()
            .is_some_and(|sharing| sharing.shared == Shared::Tab(tab))
    })
}

/// Learns whether `page` shares its screen still, as its use of the camera, the microphone or
/// the screen changes.
pub(super) fn media_changed(page: &Browser) {
    let Some(host) = page.host() else { return };
    let sharing = host.can_execute_chrome_command(STOP) == 1;
    let movable = host.can_execute_chrome_command(SWITCH) == 1;
    let changed = SHARING.with_borrow_mut(|current| {
        let Some(sharing_page) = current
            .as_mut()
            .filter(|current| current.page.identifier() == page.identifier())
        else {
            return false;
        };
        if sharing {
            sharing_page.started = true;
            sharing_page.moving = false;
            sharing_page.movable = movable;
        } else if sharing_page.started && !sharing_page.moving {
            *current = None;
        }
        true
    });
    if changed {
        send_state();
        surfaces_changed();
    }
}

/// The screen sharing going on, if Chromium shares it, its page still open.
fn started<R>(read: impl FnOnce(&Sharing) -> R) -> Option<R> {
    SHARING.with_borrow_mut(|current| {
        if current
            .as_ref()
            .is_some_and(|sharing| sharing.page.is_valid() != 1)
        {
            *current = None;
        }
        current.as_ref().filter(|sharing| sharing.started).map(read)
    })
}

/// Whether `tab`'s row offers to share it instead: a tab is shared, the page lets it move, and
/// `tab`, at `url`, is a web page's of the same profile, neither shared nor sharing.
pub(super) fn offers_instead(tab: NodeId, url: &str, profile: ProfileId) -> bool {
    is_web(url)
        && started(|sharing| {
            matches!(sharing.shared, Shared::Tab(shared) if shared != tab)
                && sharing.movable
                && sharing.profile == profile
                && page_tab(&sharing.page) != Some(tab)
        })
        .unwrap_or(false)
}

/// Whether the tab shown in `tab` may be shared instead, from its url and its profile.
fn offers_instead_of(tab: NodeId) -> bool {
    let found = STATE.with_borrow(|state| {
        let sidebar = &state.sidebar;
        let NodeKind::Tab(found) = &sidebar.node(tab)?.kind else {
            return None;
        };
        Some((found.url.clone(), sidebar.profile_of(tab)?))
    });
    found.is_some_and(|(url, profile)| offers_instead(tab, &url, profile))
}

/// Moves the screen sharing to `tab`, as the page lets it.
pub(super) fn share_instead(tab: NodeId) {
    if !offers_instead_of(tab) {
        return;
    }
    let Some(page) = started(|sharing| sharing.page.clone()) else {
        return;
    };
    if !set_source(&page, Shared::Tab(tab), "") {
        return;
    }
    if let Some(host) = page.host() {
        host.execute_chrome_command(SWITCH, WindowOpenDisposition::CURRENT_TAB);
    }
    SHARING.with_borrow_mut(|current| {
        if let Some(sharing) = current.as_mut() {
            sharing.shared = Shared::Tab(tab);
            sharing.moving = true;
            sharing.hidden = false;
        }
    });
    send_state();
    surfaces_changed();
}

/// Ends the screen sharing.
pub(super) fn stop_sharing() {
    if let Some(host) = started(|sharing| sharing.page.host()).flatten() {
        host.execute_chrome_command(STOP, WindowOpenDisposition::CURRENT_TAB);
    }
}

/// Hides the bar over the page until the sharing changes.
pub(super) fn hide_sharing_bar() {
    SHARING.with_borrow_mut(|current| {
        if let Some(sharing) = current.as_mut() {
            sharing.hidden = true;
        }
    });
    surfaces_changed();
}

/// The bar over the page during a screen sharing, like Arc's, in the window: on the page sharing
/// and on the tab shared, it tells what is shared and ends it; on another tab, while a tab is
/// shared, it also offers to share that tab instead.
#[cfg(target_os = "macos")]
pub(super) fn sync_sharing_bar() {
    // The picker of what to share shows at the same place, over the page.
    let shown = win().shown.get().filter(|_| !picker_shown());
    let detail = shown.and_then(|shown| {
        tab_page(shown)?;
        let instead = offers_instead_of(shown);
        started(|sharing| {
            let sharing_shown = page_tab(&sharing.page) == Some(shown);
            let shared_shown = sharing.shared == Shared::Tab(shown);
            if sharing.hidden || !(sharing_shown || shared_shown || instead) {
                return None;
            }
            let what = match sharing.shared {
                Shared::Screen => "your screen".to_string(),
                Shared::Window => "a window".to_string(),
                Shared::Tab(_) if shared_shown => "this tab".to_string(),
                Shared::Tab(tab) => STATE.with_borrow(|state| {
                    match state.sidebar.node(tab).map(|node| &node.kind) {
                        Some(NodeKind::Tab(tab)) => format!("“{}”", tab.label()),
                        _ => "a tab".to_string(),
                    }
                }),
            };
            Some(json!({
                "text": format!("Sharing {what} with {}", sharing.site),
                "tab": shown,
                "instead": instead,
            }))
        })
        .flatten()
    });
    let card = card_rect().unwrap_or_default();
    let width = SHARING_BAR_WIDTH.min(card.width - 2 * PERMISSION_INSET);
    let frame = Rect {
        x: card.x + (card.width - width) / 2,
        y: card.y + PERMISSION_INSET,
        width,
        height: SHARING_BAR_HEIGHT,
    };
    let text = detail.clone().unwrap_or_default().to_string();
    crate::native::event(win().id, "sharing", "arbalete:sharing", &text);
    crate::native::surface(win().id, "sharing", detail.is_some(), &frame);
}

/// The bar's size, centered at the top of the card.
const SHARING_BAR_WIDTH: i32 = 560;
const SHARING_BAR_HEIGHT: i32 = 44;
