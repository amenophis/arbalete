//! The tabs' pages in the card: made as a tab first shows, shown, hidden and closed.

use super::*;

/// Loads a url in the shown page.
pub(super) fn load(url: &str) {
    if let Some(frame) = content().and_then(|browser| browser.main_frame()) {
        frame.load_url(Some(&CefString::from(url)));
    }
}

/// The page the window shows.
pub(super) fn content() -> Option<Browser> {
    let tab = win().shown.get()?;
    PAGES.with_borrow(|pages| Some(pages.get(&tab)?.browser.clone()))
}

/// Whether a browser is the shown page's.
pub(super) fn is_shown(browser: Option<&mut Browser>) -> bool {
    let (Some(browser), Some(mut shown)) = (browser, content()) else {
        return false;
    };
    browser.is_same(Some(&mut shown)) == 1
}

/// The tab whose page a browser is, if it is one's.
pub(super) fn page_tab(browser: &Browser) -> Option<NodeId> {
    let id = browser.identifier();
    PAGES.with_borrow(|pages| {
        pages
            .iter()
            .find(|(_, live)| live.browser.identifier() == id)
            .map(|(tab, _)| *tab)
    })
}

/// The selected tab's profile and url.
pub(super) fn selected() -> Option<(ProfileId, String)> {
    STATE.with_borrow_mut(|state| {
        let tab = state.sidebar.displayed().selected?;
        let profile = state.sidebar.profile_of(tab)?;
        Some((profile, state.sidebar.tab_mut(tab)?.url.clone()))
    })
}

/// Shows the selected tab's page, which opens the first time, once its profile's web data is
/// ready; or leaves the card empty when the displayed Space has no selected tab.
pub(super) fn show_selected() {
    let tab = STATE.with_borrow(|state| state.sidebar.displayed().selected);
    if let Some(tab) = tab {
        win().recent.with_borrow_mut(|recent| remember(recent, tab));
    }
    // The tab left and the tab shown both showed until now, for the archive.
    let now = now().try_into().unwrap_or_default();
    STATE.with_borrow_mut(|state| {
        for shown in [win().last_shown.replace(tab), tab].into_iter().flatten() {
            state.sidebar.touch(shown, now);
        }
    });
    prune_pages();
    let Some(tab) = tab.filter(|_| selected().is_some()) else {
        empty_card();
        return;
    };
    // A split's tabs show together, the selected one focused.
    let (tabs, layout) = STATE.with_borrow(|state| {
        let sidebar = &state.sidebar;
        let row = sidebar.row_of(tab);
        if row == tab {
            (vec![tab], None)
        } else {
            let tabs = sidebar
                .node(row)
                .map_or_else(Vec::new, |node| node.children.clone());
            (tabs, Some(sidebar.split_layout(row)))
        }
    });
    let opened: Option<Vec<_>> = tabs.iter().map(|&tab| page(tab)).collect();
    if opened.is_none() {
        return;
    }
    match layout {
        Some(layout) => show_split(tab, &tabs, layout),
        None => show_content(tab),
    }
}

/// Each other window that shows a page while it selects none, as a drag from another window
/// moved its tab out of its Space, shows what it selects now: nothing, like a tab moved to
/// another Space.
pub(super) fn show_selected_elsewhere() {
    let current = FOCUSED.get();
    let others: Vec<u32> =
        WINDOWS.with_borrow(|windows| windows.iter().map(|win| win.id).collect());
    for other in others.into_iter().filter(|&id| id != current) {
        focus(other);
        let selected = STATE.with_borrow(|state| state.sidebar.displayed().selected);
        let shows = win().shown.get().is_some() || !win().split.borrow().0.is_empty();
        if selected.is_none() && shows {
            show_selected();
        }
    }
    focus(current);
}

/// Starts loading a tab's page while it stays hidden, like a link opened behind the page shown:
/// its title and icon come in the sidebar meanwhile. Until its profile's web data is ready, it
/// loads once shown instead.
pub(super) fn load_hidden(tab: NodeId) {
    if page(tab).is_some() {
        PAGES.with_borrow_mut(|pages| {
            if let Some(live) = pages.get_mut(&tab) {
                live.hidden_since.get_or_insert_with(now);
            }
        });
    }
}

/// A tab's page, opened the first time, hidden, once its profile's web data is ready: until
/// then, none.
fn page(tab: NodeId) -> Option<()> {
    if PAGES.with_borrow(|pages| pages.contains_key(&tab)) {
        return Some(());
    }
    // An Easel shows its board instead of a page.
    if let Some(number) = easel_of(tab) {
        return easel_view(tab, number);
    }
    let (profile, url) = STATE.with_borrow_mut(|state| {
        let profile = state.sidebar.profile_of(tab)?;
        Some((profile, state.sidebar.tab_mut(tab)?.url.clone()))
    })?;
    // A tab reopened takes back its page, kept frozen and hidden: showing wakes it.
    if take_closed_page(tab, profile) {
        mute_page(tab);
        return Some(());
    }
    match CONTEXTS.with_borrow(|contexts| contexts.get(&profile).cloned()) {
        Some(Some(mut context)) => {
            let start = if url.is_empty() { "about:blank" } else { &url };
            if SNAPSHOTS.with_borrow(|snapshots| snapshots.contains_key(&tab)) {
                REVIVED.with_borrow_mut(|revived| revived.insert(tab));
            }
            MAKING.set(Some(tab));
            let browser = make_page(tab, profile, start, &mut context);
            MAKING.set(None);
            let browser = browser?;
            if let Some(host) = browser.host() {
                host.was_hidden(1);
            }
            PAGES.with_borrow_mut(|pages| {
                pages.insert(
                    tab,
                    Live {
                        profile,
                        browser,
                        hidden_since: None,
                        frozen: false,
                        capturing: false,
                        frames: HashMap::new(),
                        played: false,
                        session: None,
                    },
                )
            });
            mute_page(tab);
            Some(())
        }
        // The tab shows once its profile's web data is ready.
        Some(None) => None,
        None => {
            create_profile_context(profile);
            None
        }
    }
}

/// Makes a tab's page's browser at `url`, drawn in the view the native interface holds for it.
fn make_page(
    tab: NodeId,
    profile: ProfileId,
    url: &str,
    context: &mut RequestContext,
) -> Option<Browser> {
    #[cfg(target_os = "macos")]
    let info = WindowInfo {
        parent_view: crate::native::page_view(win().id, tab_number(tab))?,
        // Any size: the container gives the page its own.
        bounds: Rect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        },
        runtime_style: RuntimeStyle::ALLOY,
        ..Default::default()
    };
    browser_host_create_browser_sync(
        Some(&info),
        Some(&mut ContentClient::new(profile, false, None)),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        Some(context),
    )
}

/// The number the native interface knows a tab by.
pub(super) fn tab_number(tab: NodeId) -> i64 {
    serde_json::to_value(tab)
        .ok()
        .and_then(|number| number.as_i64())
        .unwrap_or(-1)
}

/// Gives a tab's page the keyboard, and its window, if it shows.
pub(super) fn focus_page(tab: NodeId) {
    #[cfg(target_os = "macos")]
    if has_easel_view(tab) {
        return crate::native::focus_page(tab_number(tab));
    }
    let Some(host) = PAGES.with_borrow(|pages| pages.get(&tab)?.browser.host()) else {
        return;
    };
    #[cfg(target_os = "macos")]
    crate::native::focus_page(tab_number(tab));
    host.set_focus(1);
}

/// Shows or hides a tab's page where the card lays it out; Chromium learns it, which stops
/// drawing a hidden page, unless its video goes on in Picture in Picture.
fn show_page(tab: NodeId, shown: bool) {
    let Some((host, profile)) = PAGES.with_borrow(|pages| {
        let live = pages.get(&tab)?;
        Some((live.browser.host()?, live.profile))
    }) else {
        return;
    };
    host.was_hidden((!shown).into());
    if !shown {
        left_page(profile, tab);
        page_hidden(tab);
    }
}

/// Mutes a tab's page as the tab says, or gives it its sound back.
pub(super) fn mute_page(tab: NodeId) {
    let muted =
        STATE.with_borrow_mut(|state| state.sidebar.tab_mut(tab).is_some_and(|tab| tab.muted));
    let host = PAGES.with_borrow(|pages| pages.get(&tab)?.browser.host());
    if let Some(host) = host {
        host.set_audio_muted(muted.into());
    }
}

/// Makes a page usable once its browser exists: the extensions learn it, and it takes its tab's
/// sound and, when shown, the card's corners.
pub(super) fn content_created(profile: ProfileId, browser: Option<&mut Browser>) {
    let Some(browser) = browser else { return };
    extensions::page_created(profile, browser);
    picture_page_created(browser);
    media_page_created(browser);
    sound_page_created(browser);
    #[cfg(target_os = "macos")]
    cover_revived(Some(browser), true);
    // Made in page(), it is not one of the pages yet, which mutes it once it is.
    let Some(tab) = page_tab(browser) else { return };
    mute_page(tab);
    if win().shown.get() == Some(tab) {
        send_page_state();
    }
}

/// Puts a tab's page in the window instead of the shown one, if any.
fn show_content(tab: NodeId) {
    if win().shown.get() == Some(tab) && win().split.borrow().0.is_empty() {
        return;
    }
    hide_content();
    let taken = take_page(tab);
    win().shown.set(Some(tab));
    wake_page(tab);
    // Square in full screen, now that a page shows alone.
    place_card();
    shown_on_card(tab);
    // A picture at this window's size, for when another window takes the page back: the one
    // taken as this window goes to the background comes once the page has left it.
    if taken {
        snapshot_shown();
    }
    // A picture of the page once drawn, for the pickers that show the tabs, like the screen
    // sharing's: a tab shown a few seconds has one.
    post_delayed_task(
        ThreadId::UI,
        Some(&mut SnapshotShown::new()),
        SNAPSHOT_AFTER_MS,
    );
}

/// How long after a tab shows its picture is taken.
const SNAPSHOT_AFTER_MS: i64 = 1_500;

wrap_task! {
    struct SnapshotShown {}

    impl Task {
        fn execute(&self) {
            snapshot_shown();
        }
    }
}

/// A page shows in one window at a time, like Arc's: this one takes a tab's page from another,
/// which shows its picture instead, dimmed by its Space's color. Whether one showed it.
pub(super) fn take_page(tab: NodeId) -> bool {
    let current = FOCUSED.get();
    let others: Vec<_> = WINDOWS.with_borrow(|windows| {
        windows
            .iter()
            .filter(|other| {
                other.id != current
                    && (other.shown.get() == Some(tab) || other.split.borrow().0.contains(&tab))
            })
            .cloned()
            .collect()
    });
    let taken = !others.is_empty();
    for other in others {
        let shown = other.shown.take();
        *other.split.borrow_mut() = Default::default();
        #[cfg(target_os = "macos")]
        if let (Some(card), Some(shown)) = (other.card_view.get(), shown) {
            // The latest picture, unless it was taken at another window's size: the one taken
            // there.
            let bar = bar_height(other.id);
            let latest = SNAPSHOTS.with_borrow(|snapshots| snapshots.get(&shown).cloned());
            let own = other.picture.take().filter(|(tab, _)| *tab == shown);
            let jpeg = match (latest, own) {
                (Some(latest), _) if crate::mac::fits(card, &latest, bar) => Some(latest),
                (latest, own) => own.map(|(_, jpeg)| jpeg).or(latest),
            };
            if let Some(jpeg) = &jpeg {
                crate::mac::cover_page(card, jpeg, Some(space_tint(shown)), bar);
            }
            // Without one yet, as windows open on the same tab, the next to come covers it.
            other
                .picture
                .replace(Some((shown, jpeg.unwrap_or_default())));
        }
    }
    taken
}

/// The height of the bar above the page alone in the window `id`, in toolbar or developer mode.
#[cfg(target_os = "macos")]
fn bar_height(id: u32) -> f64 {
    let was = FOCUSED.get();
    focus(id);
    let bar = shown_toolbar_height();
    focus(was);
    f64::from(bar)
}

/// Keeps a picture of a tab's page for the windows it was taken at the size of, which show the
/// page. Taken as another window took the page, it covers that window's card at once, unless the
/// picture there already fits it.
#[cfg(target_os = "macos")]
pub(super) fn keep_picture(tab: NodeId, jpeg: &str) {
    for window in WINDOWS.with_borrow(Clone::clone) {
        let Some(card) = window.card_view.get() else {
            continue;
        };
        let shows = window.shown.get() == Some(tab);
        let held = window
            .picture
            .borrow()
            .clone()
            .and_then(|(of, held)| (of == tab).then_some(held));
        // Empty, a picture waited for (take_page).
        let covered = !shows
            && held
                .as_ref()
                .is_some_and(|held| held.is_empty() || crate::mac::covered(card));
        if !(shows || covered) {
            continue;
        }
        let bar = bar_height(window.id);
        let fits = crate::mac::fits(card, jpeg, bar);
        if covered && (fits || !held.is_some_and(|held| crate::mac::fits(card, &held, bar))) {
            crate::mac::cover_page(card, jpeg, Some(space_tint(tab)), bar);
        } else if !fits {
            continue;
        }
        window.picture.replace(Some((tab, jpeg.to_owned())));
    }
}

/// Lets a page shown run again, if it was frozen or unloaded while hidden.
pub(super) fn wake_page(tab: NodeId) {
    let frozen = PAGES.with_borrow_mut(|pages| {
        let live = pages.get_mut(&tab)?;
        live.hidden_since = None;
        // Media paused while its page shows has no player once the page is left.
        live.played = live.frames.values().any(|media| media.audible);
        Some((std::mem::take(&mut live.frozen), live.browser.host()))
    });
    UNLOADED.with_borrow_mut(|unloaded| unloaded.remove(&tab));
    if let Some((true, host)) = frozen {
        set_frozen(host.as_ref(), false);
    }
    show_page(tab, true);
}

/// The shown page, `tab`'s, takes the keyboard, its container the card, and the toolbar its state.
pub(super) fn shown_on_card(tab: NodeId) {
    send_empty_card();
    send_panes();
    // Not while the Command Bar shows over it, which keeps the keyboard.
    let overlay = win()
        .overlay
        .with_borrow(|overlay| overlay.as_ref().is_some_and(|o| o.is_visible() == 1));
    if !overlay {
        focus_page(tab);
    }
    send_page_state();
}

/// The color dimming a picture of a tab's page in a window that does not show it: its Space's
/// leading one, or gray.
#[cfg(target_os = "macos")]
fn space_tint(tab: NodeId) -> u32 {
    STATE
        .with_borrow(|state| {
            let sidebar = &state.sidebar;
            let space = sidebar.space_of(tab)?;
            let space = sidebar.spaces().iter().find(|s| s.id == space)?;
            let lead = space.theme.colors.first()?;
            u32::from_str_radix(lead.trim_start_matches('#'), 16).ok()
        })
        .unwrap_or(0x80_8080)
}

/// Tells the UI how the shown page stands: whether it loads and its history are its own.
fn send_page_state() {
    let browser = content();
    // A page about to open loads; an Easel's board never does, nor an empty card.
    let opening = win().shown.get().is_some_and(|tab| !has_easel_view(tab));
    let loading = browser
        .as_ref()
        .map_or(opening, |browser| browser.is_loading() == 1);
    update_page(|page| {
        page.loading = loading;
        page.can_go_back = browser
            .as_ref()
            .is_some_and(|browser| browser.can_go_back() == 1);
        page.can_go_forward = browser
            .as_ref()
            .is_some_and(|browser| browser.can_go_forward() == 1);
    });
}

/// Leaves the card empty, an empty Space's panel: no page shows, so none goes back or forward.
pub(super) fn empty_card() {
    hide_content();
    send_page_state();
}

/// Hides the shown page, which keeps running. Without one, the card shows its own color, an empty
/// Space's panel.
pub(super) fn hide_content() {
    leave_page_full_screen();
    if let Some(shown) = win().shown.get() {
        show_page(shown, false);
    }
    let (split, _) = std::mem::take(&mut *win().split.borrow_mut());
    if !split.is_empty() {
        for &tab in &split {
            show_page(tab, false);
        }
        PAGES.with_borrow_mut(|pages| {
            for tab in &split {
                if let Some(live) = pages.get_mut(tab) {
                    live.hidden_since.get_or_insert_with(now);
                }
            }
        });
    }
    // A picture of a page another window took no longer stands in for it.
    #[cfg(target_os = "macos")]
    if let Some(card) = win().card_view.get() {
        crate::mac::uncover_page(card);
        win()
            .picture
            .borrow_mut()
            .take_if(|(_, jpeg)| jpeg.is_empty());
    }
    if let Some(tab) = win().shown.take() {
        PAGES.with_borrow_mut(|pages| {
            if let Some(live) = pages.get_mut(&tab) {
                live.hidden_since = Some(now());
            }
        });
    }
    // The empty card, without the split's panes; like Arc's, the hidden sidebar floats beside an
    // empty Space's panel, as its last tab closes.
    place_card();
}

/// Keeps the pages of the tabs closed or archived a while (keep_closed_page), closes those of the
/// tabs moved to another profile's Space, whose web data a page cannot change, and forgets the
/// pictures of the tabs gone.
pub(super) fn prune_pages() {
    let gone: Vec<(NodeId, bool)> = STATE.with_borrow(|state| {
        PAGES.with_borrow(|pages| {
            pages
                .iter()
                .filter(|(tab, live)| state.sidebar.profile_of(**tab) != Some(live.profile))
                .map(|(tab, _)| (*tab, state.sidebar.node(*tab).is_none()))
                .collect()
        })
    });
    for (tab, closed) in gone {
        if closed {
            keep_closed_page(tab);
        } else {
            close_page(tab);
        }
    }
    prune_easel_views();
    STATE.with_borrow(|state| {
        SNAPSHOTS.with_borrow_mut(|snapshots| {
            snapshots.retain(|tab, _| state.sidebar.node(*tab).is_some());
        });
    });
}

/// Closes a tab's page, if it has one: the tab opens again at its url. Its container then goes
/// (BrowserLifeSpan's do_close).
pub(super) fn close_page(tab: NodeId) {
    close_inspector(tab);
    // A page unloaded to save memory counts as open again (unload_page).
    UNLOADED.with_borrow_mut(|unloaded| unloaded.remove(&tab));
    let Some(live) = PAGES.with_borrow_mut(|pages| pages.remove(&tab)) else {
        return;
    };
    if win().shown.get() == Some(tab) {
        win().shown.set(None);
    }
    if let Some(host) = live.browser.host() {
        host.close_browser(1);
    }
}

/// Keeps a closed tab's page hidden, muted and frozen rather than closing it, like a tab
/// unloaded later (expire_closed_pages): reopened with ⌘Z or ⇧⌘T, the tab takes it back, its
/// back and forward history with it, which CEF cannot give a new page. An incognito window's
/// page closes, as its tabs cannot be reopened, and so does a page using the camera, the
/// microphone or the screen, or shared, whose use stops with it.
fn keep_closed_page(tab: NodeId) {
    let keep = PAGES.with_borrow(|pages| {
        pages.get(&tab).is_some_and(|live| {
            !live.capturing
                && !INCOGNITO.with_borrow(|incognito| incognito.contains_key(&live.profile))
        })
    });
    if !keep || is_shared(tab) {
        return close_page(tab);
    }
    close_inspector(tab);
    let Some(mut live) = PAGES.with_borrow_mut(|pages| pages.remove(&tab)) else {
        return;
    };
    if win().shown.get() == Some(tab) {
        win().shown.set(None);
    }
    if let Some(host) = live.browser.host() {
        host.was_hidden(1);
        host.set_audio_muted(1);
        // Its Picture in Picture window closes with it, as it would with a page closing.
        let expression = "self.documentPictureInPicture?.window?.close(); \
            document.pictureInPictureElement && document.exitPictureInPicture()";
        let params = json!({ "expression": expression });
        let message =
            json!({ "id": unanswered_id(), "method": "Runtime.evaluate", "params": params });
        host.send_dev_tools_message(Some(message.to_string().as_bytes()));
        set_frozen(Some(&host), true);
    }
    live.frozen = true;
    CLOSED_PAGES.with_borrow_mut(|closed| closed.insert(tab, (live, now())));
}

/// Gives a tab reopened, `tab`, the page it had as the tab `closed`, if it is kept still: its
/// view moves along.
pub(super) fn reopen_page(closed: NodeId, tab: NodeId) {
    let Some(page) = CLOSED_PAGES.with_borrow_mut(|pages| pages.remove(&closed)) else {
        return;
    };
    #[cfg(target_os = "macos")]
    crate::native::page_renumbered(tab_number(closed), tab_number(tab));
    CLOSED_PAGES.with_borrow_mut(|pages| pages.insert(tab, page));
}

/// Takes a reopened tab's kept page back among the pages, if it has one of its profile: its tab
/// went back to another profile's Space, which it cannot change, closes it.
fn take_closed_page(tab: NodeId, profile: ProfileId) -> bool {
    let Some((live, _)) = CLOSED_PAGES.with_borrow_mut(|pages| pages.remove(&tab)) else {
        return false;
    };
    if live.profile != profile {
        close_live(&live);
        return false;
    }
    PAGES.with_borrow_mut(|pages| pages.insert(tab, live));
    true
}

/// Closes the closed tabs' pages kept as long as a hidden page stays loaded, or all of them.
pub(super) fn expire_closed_pages(all: bool) {
    let due = now() - current_suspend_delays().1;
    let expired: Vec<Live> = CLOSED_PAGES.with_borrow_mut(|pages| {
        pages
            .extract_if(|_, (_, closed)| all || *closed <= due)
            .map(|(_, (live, _))| live)
            .collect()
    });
    for live in &expired {
        close_live(live);
    }
}

/// Closes a page that is no tab's: its container then goes (BrowserLifeSpan's do_close).
fn close_live(live: &Live) {
    if let Some(host) = live.browser.host() {
        host.close_browser(1);
    }
}

wrap_task! {
    pub(super) struct RemoveContainer {
        view: usize,
    }

    impl Task {
        /// Takes a closing page's view out of the interface with its container, which destroys
        /// its browser.
        fn execute(&self) {
            #[cfg(target_os = "macos")]
            crate::native::page_closed(self.view as *mut std::ffi::c_void);
        }
    }
}

wrap_task! {
    pub(super) struct RemovePeek {
        window: u32,
    }

    impl Task {
        /// Takes Peek's closing page's view out of its card, which destroys its browser.
        fn execute(&self) {
            #[cfg(target_os = "macos")]
            crate::native::peek_closed(self.window);
        }
    }
}
