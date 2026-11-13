//! Sharing the screen, like Arc's: a site asking to share it shows the picker of the screens, the
//! windows and the tabs it can share (permission_prompt). amenophis/cef-codecs' CEF shares the
//! source picked from build 9 (patches/alloy-display-capture.patch in amenophis/cef-builds).

use super::*;

/// The profile's preference CEF takes the source picked from, once: "screen:<id>:0",
/// "window:<id>:0" or "arbalete-tab:<browser id>".
const SOURCE: &str = "arbalete.display_media_source";

/// Lets the app set the source picked in each profile's preferences.
pub(super) fn register_source(registrar: &mut PreferenceRegistrar) {
    if let Some(mut empty) = value_create() {
        empty.set_string(Some(&CefString::from("")));
        registrar.add_preference(Some(&CefString::from(SOURCE)), Some(&mut empty));
    }
}

/// Has `page` share `source`, from the picker: "screen:<id>:0", "window:<id>:0" or
/// "tab:<tab id>". Whether it is one the page can share.
pub(super) fn choose(page: &Browser, source: &str) -> bool {
    let source = if source.starts_with("screen:") || source.starts_with("window:") {
        source.to_string()
    } else {
        let tab = source.strip_prefix("tab:");
        match tab
            .and_then(|id| serde_json::from_str(id).ok())
            .and_then(awake_page)
        {
            Some(browser) => format!("arbalete-tab:{}", browser.identifier()),
            None => return false,
        }
    };
    set_source(page, &source)
}

/// Puts the source in the preference of `page`'s profile.
fn set_source(page: &Browser, source: &str) -> bool {
    let Some(context) = page.host().and_then(|host| host.request_context()) else {
        return false;
    };
    let Some(mut value) = value_create() else {
        return false;
    };
    value.set_string(Some(&CefString::from(source)));
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
