//! The main window and the UI it shows.
//!
//! The `cef` crate's default callbacks return 0, which means "no" or "cancel": every callback the app
//! relies on is implemented explicitly.

use crate::command::{Command, HitTarget, Target};
use crate::menu::{self, Action};
use arbalette_core::history::{self, History};
use arbalette_core::import::browsers::{self, Family};
use arbalette_core::import::{self, Choice, safari};
use arbalette_core::sidebar::{Color, NodeId, NodeKind, ProfileId, Sidebar, Space, SpaceId, Tab};
use arbalette_core::store;
use cef::wrapper::stream_resource_handler::StreamResourceHandler;
use cef::*;
use serde::Serialize;
use serde_json::json;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, LocalKey};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The UI is served by the app itself under this origin.
const UI_ORIGIN: &str = "https://arbalette.ui/";

const SIDEBAR_WIDTH: i32 = 260;

/// The docked sidebar is wider while it shows the library (LIBRARY_SHIFT in ui/src/pager.ts).
const LIBRARY_WIDTH: i32 = SIDEBAR_WIDTH + 180;

/// The page is a card inset from the window's edges and from the sidebar. The window paints its
/// margins with the colors the sidebar sends (Command::WindowColors).
const CARD_MARGIN: i32 = 9;

/// The window's color until the sidebar sends the displayed Space's: the dark one, its most
/// common appearance (--window-end in ui/src/index.css).
const WINDOW_COLOR: u32 = 0xff1b_0e1f;

/// The strip along the window's left edge that reveals the hidden sidebar when the pointer
/// reaches it. Views cannot be transparent, so it shows the sidebar's background.
const EDGE_WIDTH: i32 = 4;

/// How far the floating sidebar sits from the window's edges.
const FLOAT_INSET: i32 = 4;

/// How long the sidebar's moves last, in milliseconds: hiding or showing it, the card following;
/// the revealed sidebar floating in, with a spring, and out; and docking, from above the page or
/// beside an empty Space's panel.
const HIDE_DURATION: u32 = 85;
const REVEAL_DURATION: u32 = 250;
const CONCEAL_DURATION: u32 = 90;
const DOCK_DURATION: u32 = 70;
const FLOATING_DOCK_DURATION: u32 = 50;

/// The time between two steps of a native slide: about a frame.
const FRAME_MS: i64 = 8;

/// The overlay's size: one text field for now (ui/src/Overlay.tsx).
const OVERLAY_SIZE: Size = Size {
    width: 600,
    height: 56,
};

/// The import assistant's size, at most (ui/src/Import.tsx).
const IMPORT_SIZE: Size = Size {
    width: 560,
    height: 480,
};

/// The Privacy & Security pane of System Settings that lists the apps with Full Disk Access.
const FULL_DISK_ACCESS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// The height of the sidebar's toolbar, which holds the window buttons on macOS (ui/src/index.css).
const TOOLBAR_HEIGHT: f32 = 40.0;

/// The first tab of a new session.
const START_URL: &str = "https://www.wikipedia.org/";

thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
    /// Beside the card, at the window's left edge. It must not reach under the card: a web view
    /// below the overlays receives no real mouse event on macOS, only synthesized ones.
    static SIDEBAR: RefCell<Option<BrowserView>> = const { RefCell::new(None) };
    /// Holds the content views, inset from the window's edges and from the sidebar. An overlay, as
    /// macOS only rounds the corners of a window of its own. Without a selected tab, its own
    /// color paints an empty Space's panel.
    static CARD: RefCell<Option<OverlayController>> = const { RefCell::new(None) };
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
    /// The import assistant, shown above the page; created the first time it is shown, as most
    /// sessions never import.
    static IMPORT_VIEW: RefCell<Option<BrowserView>> = const { RefCell::new(None) };
    static IMPORT: RefCell<Option<OverlayController>> = const { RefCell::new(None) };
    /// The browsers the import assistant lists, which its choices refer to by index.
    static SOURCES: RefCell<Vec<browsers::Browser>> = const { RefCell::new(Vec::new()) };
    /// An import is reading the browsers' data, on a thread of its own.
    static IMPORTING: Cell<bool> = const { Cell::new(false) };
    /// The first launch shows the import assistant with the window, instead of the start page.
    static IMPORT_AT_LAUNCH: Cell<bool> = const { Cell::new(false) };
    static SIDEBAR_MODE: Cell<SidebarMode> = const { Cell::new(SidebarMode::Shown) };
    /// The docked sidebar shows the library, wider (LIBRARY_WIDTH).
    static LIBRARY: Cell<bool> = const { Cell::new(false) };
    /// Under way while the sidebar moves to its mode.
    static SLIDE: RefCell<Option<Slide>> = const { RefCell::new(None) };
    /// The sidebar's draggable regions, which move the window while the sidebar is shown, with
    /// the margins around the card.
    static DRAG_REGIONS: RefCell<Vec<DraggableRegion>> = const { RefCell::new(Vec::new()) };
    static STATE: RefCell<UiState> = RefCell::default();
    /// Where the sidebar is saved, once the previous session is restored.
    static SESSION_FILE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    /// CEF's root cache path, which holds every profile's web data.
    static DATA_DIR: RefCell<PathBuf> = RefCell::default();
    /// Each profile's history, opened on its first visit.
    static HISTORIES: RefCell<HashMap<ProfileId, History>> = RefCell::default();
    /// Browsers not closed yet: the app quits when the last one closes.
    static OPEN_BROWSERS: Cell<usize> = const { Cell::new(0) };
    #[cfg(target_os = "macos")]
    static SWIPE: RefCell<crate::swipe::Tracker> = RefCell::default();
    /// Where the docked sidebar's rows and Space header are, then the revealed one's.
    static MENU_TARGETS: RefCell<[Vec<HitTarget>; 2]> = RefCell::default();
}

/// The sidebar sits beside the page, or is hidden: then a strip at the window's left edge
/// reveals it above the page while the pointer is over it. Beside an empty Space's panel, it
/// floats instead of hiding. For this session only.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SidebarMode {
    Shown,
    Hidden,
    Revealed,
    Floating,
}

impl SidebarMode {
    /// Where the card and the revealed sidebar stand in this mode, in a window of `size`. While
    /// the sidebar is docked, the revealed one's place is the docked one's, which it docks into.
    fn layout(self, size: &Size) -> Layout {
        let left = match self {
            SidebarMode::Shown => docked_width() + CARD_MARGIN,
            SidebarMode::Floating => SIDEBAR_WIDTH + CARD_MARGIN,
            SidebarMode::Hidden | SidebarMode::Revealed => CARD_MARGIN,
        };
        let card = Rect {
            x: left,
            y: CARD_MARGIN,
            width: size.width - left - CARD_MARGIN,
            height: size.height - 2 * CARD_MARGIN,
        };
        let reveal = match self {
            SidebarMode::Shown => Rect {
                x: 0,
                y: 0,
                width: SIDEBAR_WIDTH,
                height: size.height,
            },
            SidebarMode::Hidden => Rect {
                x: 0,
                y: 0,
                width: EDGE_WIDTH,
                height: size.height,
            },
            SidebarMode::Revealed | SidebarMode::Floating => Rect {
                x: FLOAT_INSET,
                y: FLOAT_INSET,
                width: SIDEBAR_WIDTH,
                height: size.height - 2 * FLOAT_INSET,
            },
        };
        Layout { card, reveal }
    }

    /// Where the margins that move the window start: past the docked sidebar, which has its own
    /// drag areas, or at the window's edge.
    fn margins_start(self) -> i32 {
        if self == SidebarMode::Shown {
            docked_width()
        } else {
            0
        }
    }
}

/// The docked sidebar's width: wider for the library.
fn docked_width() -> i32 {
    if LIBRARY.get() {
        LIBRARY_WIDTH
    } else {
        SIDEBAR_WIDTH
    }
}

/// Where the card and the revealed sidebar stand.
#[derive(Clone, Debug)]
struct Layout {
    card: Rect,
    reveal: Rect,
}

/// The card, and the revealed sidebar when it docks, on their way to the sidebar's mode.
struct Slide {
    from: Layout,
    start: Instant,
    duration: Duration,
    /// The revealed sidebar grows into the docked one's place, losing its floating look.
    docking: bool,
}

/// How many closed tabs the UI lists, the last closed first: the state weighs on every update.
const RECENTLY_CLOSED: usize = 10;

/// What the UI shows.
#[derive(Default)]
struct UiState {
    page: Page,
    sidebar: Sidebar,
    /// Closed tabs, the last one at the end, until the app quits.
    closed: Vec<Tab>,
}

impl UiState {
    /// Sent whole to the UI on every change: the displayed Space, and its page and its
    /// neighbors', which the UI lays out side by side to swipe between them. The other pages are
    /// left out, as they would weigh on every update.
    fn to_json(&self) -> serde_json::Value {
        let spaces = self.sidebar.spaces();
        let displayed = self.sidebar.displayed().id;
        let index = spaces
            .iter()
            .position(|s| s.id == displayed)
            .expect("a known Space");
        json!({
            "page": self.page,
            "space": displayed,
            "spaces": spaces.iter().map(|s| json!({
                "id": s.id,
                "title": s.title,
                "emoji": s.emoji,
                "color": s.color,
            })).collect::<Vec<_>>(),
            "pages": spaces
                .iter()
                .enumerate()
                .filter(|(i, _)| i.abs_diff(index) <= 1)
                .map(|(_, s)| self.page_json(s))
                .collect::<Vec<_>>(),
            "profiles": self.sidebar.profiles().iter().map(|p| json!({
                "id": p.id,
                "name": p.name,
                "color": p.color,
            })).collect::<Vec<_>>(),
            "closed": self.closed.iter().rev().take(RECENTLY_CLOSED).map(|tab| json!({
                "url": tab.url,
                "title": tab.title,
                "favicon": tab.favicon,
            })).collect::<Vec<_>>(),
        })
    }

    /// A Space's tabs and its profile's favorites. `roots` names the sections' containers, which
    /// the UI moves nodes into.
    fn page_json(&self, space: &Space) -> serde_json::Value {
        let favorites = self
            .sidebar
            .favorites_of(space.id)
            .expect("a known profile");
        json!({
            "space": space.id,
            "selected": space.selected,
            "collapsed": space.collapsed,
            "roots": { "favorites": favorites, "pinned": space.pinned, "today": space.today },
            "favorites": self.tabs(favorites),
            "pinned": self.items(space.pinned),
            "today": self.items(space.today),
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

/// Moves the sidebar to `mode`. The views it shows on the way are shown first, the others hidden
/// once it gets there (settle). The card and the revealed sidebar slide natively, frame by frame;
/// the revealed sidebar floats in and out in the compositor, which clips it by its own window.
fn set_sidebar(mode: SidebarMode) {
    let before = SIDEBAR_MODE.replace(mode);
    let (Some(size), Some(card), Some(reveal)) = (
        WINDOW.with_borrow(|window| window.as_ref().map(|w| w.size())),
        CARD.with_borrow(Clone::clone),
        REVEAL.with_borrow(Clone::clone),
    ) else {
        return;
    };
    #[cfg(target_os = "macos")]
    let duration = if crate::mac::reduce_motion() {
        0
    } else {
        slide_duration(before, mode)
    };
    #[cfg(not(target_os = "macos"))]
    let duration = 0;
    let docked = mode == SidebarMode::Shown;
    if docked && let Some(sidebar) = SIDEBAR.with_borrow(Clone::clone) {
        sidebar.set_visible(1);
    }
    if docked != (before == SidebarMode::Shown) {
        let slide = duration > 0 && matches!(before, SidebarMode::Shown | SidebarMode::Hidden);
        let detail = json!({ "docked": docked, "slide": slide });
        dispatch(
            &SIDEBAR,
            &format!(r#"new CustomEvent("arbalette:docked", {{ detail: {detail} }})"#),
        );
    }
    let floating = |mode| matches!(mode, SidebarMode::Revealed | SidebarMode::Floating);
    #[cfg(target_os = "macos")]
    if let Some(view) = reveal_window() {
        let seconds = f64::from(duration) / 1000.0;
        let width = f64::from(SIDEBAR_WIDTH);
        match (before, mode) {
            (SidebarMode::Hidden, SidebarMode::Revealed) => {
                let positions = crate::motion::positions(crate::motion::spring, -width, 0.0, 30);
                crate::mac::slide(view, &positions, seconds);
            }
            (SidebarMode::Revealed, SidebarMode::Hidden) => {
                let positions = crate::motion::positions(crate::motion::ease_out, 0.0, -width, 30);
                crate::mac::slide(view, &positions, seconds);
            }
            _ => {}
        }
        if floating(mode) && !floating(before) {
            crate::mac::float(view, 1.0);
        }
    }
    if floating(mode) && !floating(before) {
        reveal.set_bounds(Some(&mode.layout(&size).reveal));
        reveal.set_visible(1);
    }
    let from = Layout {
        card: card.bounds(),
        reveal: reveal.bounds(),
    };
    let slide = Slide {
        from,
        start: Instant::now(),
        duration: Duration::from_millis(duration.into()),
        docking: docked && floating(before),
    };
    if SLIDE.replace(Some(slide)).is_none() {
        step_slide();
    }
}

/// How long the sidebar takes from one mode to another, in milliseconds; 0 for at once.
fn slide_duration(from: SidebarMode, to: SidebarMode) -> u32 {
    match (from, to) {
        (SidebarMode::Shown, SidebarMode::Hidden) | (SidebarMode::Hidden, SidebarMode::Shown) => {
            HIDE_DURATION
        }
        (SidebarMode::Hidden, SidebarMode::Revealed) => REVEAL_DURATION,
        (SidebarMode::Revealed, SidebarMode::Hidden) => CONCEAL_DURATION,
        (SidebarMode::Revealed, SidebarMode::Shown) => DOCK_DURATION,
        (SidebarMode::Floating, SidebarMode::Shown) => FLOATING_DOCK_DURATION,
        _ => 0,
    }
}

/// The mode the sidebar toggle leads to. A Space without a tab shows an empty panel, which has
/// nothing to grow into: the sidebar floats beside it instead of hiding.
fn toggled(mode: SidebarMode, has_tab: bool) -> SidebarMode {
    match mode {
        SidebarMode::Shown if has_tab => SidebarMode::Hidden,
        SidebarMode::Shown => SidebarMode::Floating,
        _ => SidebarMode::Shown,
    }
}

/// Moves the card, and the revealed sidebar while it docks, one frame further, until the slide
/// ends.
fn step_slide() {
    let Some((from, start, duration, docking)) = SLIDE.with_borrow(|slide| {
        let slide = slide.as_ref()?;
        Some((
            slide.from.clone(),
            slide.start,
            slide.duration,
            slide.docking,
        ))
    }) else {
        return;
    };
    let elapsed = start.elapsed().as_secs_f64();
    let progress = if duration.is_zero() {
        1.0
    } else {
        elapsed / duration.as_secs_f64()
    };
    let size = WINDOW.with_borrow(|window| window.as_ref().map(|w| w.size()));
    let (Some(size), true) = (size, progress < 1.0) else {
        SLIDE.set(None);
        settle_sidebar();
        return;
    };
    let eased = crate::motion::ease_out(progress);
    let to = SIDEBAR_MODE.get().layout(&size);
    if let Some(card) = CARD.with_borrow(Clone::clone) {
        card.set_bounds(Some(&lerp_rect(&from.card, &to.card, eased)));
    }
    if docking && let Some(reveal) = REVEAL.with_borrow(Clone::clone) {
        reveal.set_bounds(Some(&lerp_rect(&from.reveal, &to.reveal, eased)));
        #[cfg(target_os = "macos")]
        if let Some(view) = reveal_window() {
            crate::mac::float(view, 1.0 - eased);
        }
    }
    post_delayed_task(ThreadId::UI, Some(&mut StepSlide::new()), FRAME_MS);
}

wrap_task! {
    struct StepSlide {}

    impl Task {
        fn execute(&self) {
            step_slide();
        }
    }
}

/// `t` of the way from one rectangle to another.
fn lerp_rect(from: &Rect, to: &Rect, t: f64) -> Rect {
    use crate::motion::lerp;
    Rect {
        x: lerp(from.x, to.x, t),
        y: lerp(from.y, to.y, t),
        width: lerp(from.width, to.width, t),
        height: lerp(from.height, to.height, t),
    }
}

/// Shows the views of the sidebar's mode where it wants them, and hides the others.
fn settle_sidebar() {
    let mode = SIDEBAR_MODE.get();
    let docked = mode == SidebarMode::Shown;
    if let Some(sidebar) = SIDEBAR.with_borrow(Clone::clone) {
        sidebar.set_visible(docked.into());
    }
    place_card();
    place_reveal();
    update_drag_regions();
    #[cfg(target_os = "macos")]
    {
        if let Some(view) = reveal_window() {
            crate::mac::slide(view, &[], 0.0);
            let floating = matches!(mode, SidebarMode::Revealed | SidebarMode::Floating);
            crate::mac::float(view, if floating { 1.0 } else { 0.0 });
        }
        if let Some(window) = WINDOW.with_borrow(Clone::clone) {
            crate::mac::sidebar_shown(window.window_handle(), docked);
        }
    }
}

/// The revealed sidebar's window, once its browser is created.
#[cfg(target_os = "macos")]
fn reveal_window() -> Option<*mut std::ffi::c_void> {
    let browser = REVEAL_VIEW.with_borrow(|view| view.as_ref()?.browser())?;
    Some(browser.host()?.window_handle())
}

/// The sidebar the window shows: the docked one, or the one above the page.
fn shown_sidebar() -> &'static LocalKey<RefCell<Option<BrowserView>>> {
    match SIDEBAR_MODE.get() {
        SidebarMode::Revealed | SidebarMode::Floating => &REVEAL_VIEW,
        SidebarMode::Shown | SidebarMode::Hidden => &SIDEBAR,
    }
}

/// The window moves by its margins around the card, and by the sidebar's drag areas while it is
/// docked. CEF applies the regions in order, a region that is not draggable cutting out of the
/// ones before: the sidebar's come last, so its controls stay clickable.
fn update_drag_regions() {
    let Some(window) = WINDOW.with_borrow(Clone::clone) else {
        return;
    };
    let mode = SIDEBAR_MODE.get();
    let size = window.size();
    let mut regions = margin_regions(&size, &mode.layout(&size).card, mode.margins_start());
    if mode == SidebarMode::Shown {
        regions.extend(DRAG_REGIONS.with_borrow(Clone::clone));
    }
    window.set_draggable_regions(Some(&regions));
}

/// The four margins around the `card` of a window of `size`, from `start`: the docked sidebar's
/// edge, or the window's.
fn margin_regions(size: &Size, card: &Rect, start: i32) -> Vec<DraggableRegion> {
    let (width, height) = (size.width, size.height);
    let (right, bottom) = (card.x + card.width, card.y + card.height);
    [
        (start, 0, width - start, card.y),
        (start, bottom, width - start, height - bottom),
        (start, 0, card.x - start, height),
        (right, 0, width - right, height),
    ]
    .into_iter()
    .map(|(x, y, width, height)| DraggableRegion {
        bounds: Rect {
            x,
            y,
            width,
            height,
        },
        draggable: 1,
    })
    .collect()
}

/// Insets the card from the window's edges, and from the sidebar while it is docked or floating.
fn place_card() {
    let Some(card) = CARD.with_borrow(Clone::clone) else {
        return;
    };
    let size = WINDOW.with_borrow(|window| window.as_ref().map(|w| w.size()));
    let Some(size) = size else { return };
    card.set_bounds(Some(&SIDEBAR_MODE.get().layout(&size).card));
}

/// Moves the card `offset` points right of where it rests beside the docked sidebar, which pages
/// towards the library; the sidebar gets its own width back first. The compositor moves the
/// card's window content, clipped by the window: the page keeps its size.
fn shift_card(offset: u32) {
    if SIDEBAR_MODE.get() != SidebarMode::Shown {
        return;
    }
    widen_for_library(false);
    #[cfg(target_os = "macos")]
    if let Some(view) = card_window() {
        crate::mac::shift(view, f64::from(offset));
    }
    #[cfg(not(target_os = "macos"))]
    let _ = offset;
}

/// Widens the docked sidebar for the library, the card resting beside it.
fn show_library() {
    if SIDEBAR_MODE.get() != SidebarMode::Shown {
        return;
    }
    widen_for_library(true);
    #[cfg(target_os = "macos")]
    if let Some(view) = card_window() {
        crate::mac::shift(view, 0.0);
    }
}

/// Gives the docked sidebar the library's width, or its own, and places the card beside it.
fn widen_for_library(library: bool) {
    if LIBRARY.replace(library) == library {
        return;
    }
    if let Some(sidebar) = SIDEBAR.with_borrow(Clone::clone) {
        sidebar.invalidate_layout();
    }
    if let Some(window) = WINDOW.with_borrow(Clone::clone) {
        window.layout();
    }
    place_card();
    update_drag_regions();
}

/// The card's window, once a content view's browser is created: every content view draws in it.
#[cfg(target_os = "macos")]
fn card_window() -> Option<*mut std::ffi::c_void> {
    let view = content()
        .or_else(|| CONTENTS.with_borrow(|contents| contents.values().flatten().next().cloned()))?;
    Some(view.browser()?.host()?.window_handle())
}

/// The panel holding the content views.
fn card_panel() -> Option<Panel> {
    CARD.with_borrow(|card| card.as_ref()?.contents_view()?.as_panel())
}

/// Places the revealed sidebar above the page, or the strip that reveals it along the window's
/// left edge.
fn place_reveal() {
    let Some(reveal) = REVEAL.with_borrow(Clone::clone) else {
        return;
    };
    let mode = SIDEBAR_MODE.get();
    let size = WINDOW.with_borrow(|window| window.as_ref().map(|w| w.size()));
    let Some(size) = size else { return };
    reveal.set_bounds(Some(&mode.layout(&size).reveal));
    reveal.set_visible((mode != SidebarMode::Shown).into());
}

/// Loads the sidebar saved by the previous session; a file that cannot be read is replaced on the
/// next save. The first launch offers to import the browsers found, if any, before the start page.
fn restore(path: PathBuf) {
    let first_launch = match store::load(&path) {
        Ok(Some(sidebar)) => {
            STATE.with_borrow_mut(|state| state.sidebar = sidebar);
            false
        }
        Ok(None) => true,
        Err(error) => {
            eprintln!("cannot restore the session: {error:?}");
            false
        }
    };
    SESSION_FILE.set(Some(path));
    if first_launch && !browsers::detect(&crate::import_home()).is_empty() {
        IMPORT_AT_LAUNCH.set(true);
    } else if STATE.with_borrow_mut(|state| active_tab(&mut state.sidebar).is_none()) {
        open_tab(START_URL);
    }
}

/// The first launch's sidebar, which nothing has changed yet: an import replaces it.
fn is_untouched(sidebar: &Sidebar) -> bool {
    *sidebar == Sidebar::default()
}

/// Shows the import assistant above the page, with the browsers found now.
fn show_import() {
    let Some(window) = WINDOW.with_borrow(Clone::clone) else {
        return;
    };
    // It shows the import running.
    if IMPORTING.get() {
        return;
    }
    SOURCES.set(browsers::detect(&crate::import_home()));
    if IMPORT.with_borrow(Option::is_none) {
        let url = format!("{UI_ORIGIN}index.html#import");
        let view = browser_view(UiClient::new(), &url, no_width, None);
        let controller = window
            .add_overlay_view(Some(&mut View::from(&view)), DockingMode::CUSTOM, 1)
            .expect("import");
        IMPORT_VIEW.set(Some(view));
        IMPORT.set(Some(controller));
    }
    let Some(controller) = IMPORT.with_borrow(Clone::clone) else {
        return;
    };
    let size = window.size();
    let (width, height) = (
        IMPORT_SIZE.width.min(size.width),
        IMPORT_SIZE.height.min(size.height),
    );
    controller.set_bounds(Some(&Rect {
        x: (size.width - width) / 2,
        y: (size.height - height) / 2,
        width,
        height,
    }));
    controller.set_visible(1);
    // A page still loading asks for them once it listens (Command::ImportReady).
    send_sources();
    if let Some(view) = controller.contents_view() {
        view.request_focus();
    }
}

/// Sends the import assistant the browsers to list, and the profiles to merge them into: none
/// on the first launch, whose sidebar the import replaces.
fn send_sources() {
    let profiles = STATE.with_borrow(|state| {
        if is_untouched(&state.sidebar) {
            return Vec::new();
        }
        let profiles = state.sidebar.profiles().iter();
        profiles
            .map(|p| json!({ "id": p.id, "name": p.name }))
            .collect()
    });
    let browsers = SOURCES.with_borrow(|browsers| {
        let browsers = browsers.iter().map(|browser| {
            let profiles = browser.profiles.iter().map(|profile| {
                let denied = browser.family == Family::Safari
                    && safari::needs_full_disk_access(&profile.path);
                json!({ "name": profile.name, "needsFullDiskAccess": denied })
            });
            json!({
                "name": browser.name,
                "arc": browser.family == Family::Arc,
                "profiles": profiles.collect::<Vec<_>>(),
            })
        });
        browsers.collect::<Vec<_>>()
    });
    let detail = json!({ "browsers": browsers, "profiles": profiles });
    let event = format!(r#"new CustomEvent("arbalette:sources", {{ detail: {detail} }})"#);
    dispatch(&IMPORT_VIEW, &event);
}

fn hide_import() {
    if let Some(controller) = IMPORT.with_borrow(Clone::clone) {
        controller.set_visible(0);
    }
}

/// Closes the import assistant. Closed on the first launch, it leaves the session as it would
/// have been without it: with the start page. It stays while an import runs.
fn close_import() {
    if IMPORTING.get() {
        return;
    }
    hide_import();
    if STATE.with_borrow(|state| is_untouched(&state.sidebar)) {
        open_tab(START_URL);
    }
}

/// Imports browser profiles. Their data is read on a thread of its own, as a large history takes
/// long to read: the window keeps answering meanwhile, and the assistant shows the import
/// running. A second import while one runs is ignored.
fn run_import(choices: Vec<Choice>) {
    if IMPORTING.replace(true) {
        return;
    }
    let browsers = SOURCES.take();
    thread::spawn(move || {
        let sources = import::read(&browsers, &choices);
        let sources = Arc::new(Mutex::new(Some(sources)));
        post_task(ThreadId::UI, Some(&mut ApplyImport::new(sources)));
    });
}

wrap_task! {
    struct ApplyImport {
        sources: Arc<Mutex<Option<Vec<import::Source>>>>,
    }

    impl Task {
        fn execute(&self) {
            let sources = self.sources.lock().ok().and_then(|mut sources| sources.take());
            apply_import(sources.unwrap_or_default());
        }
    }
}

/// Adds the browser profiles read to the sidebar as it is now, then shows the first Space
/// imported with its first tab, or a blank page when it has none. Their histories are merged on
/// a thread of their own.
fn apply_import(sources: Vec<import::Source>) {
    IMPORTING.set(false);
    let (imported, shown) = update_sidebar(|sidebar| {
        let mut new = if is_untouched(sidebar) {
            Sidebar::empty()
        } else {
            sidebar.clone()
        };
        let before = new.spaces().len();
        let imported = import::apply(&mut new, sources);
        let shown = new.spaces().get(before).map(|space| space.id);
        if let Some(space) = shown {
            new.display(space).expect("an imported Space");
            if let Some(tab) = new.next_tab(space, true) {
                new.select(space, tab).expect("a tab of the Space");
            }
        }
        // Nothing was imported into an empty sidebar: it keeps the default one.
        if !new.spaces().is_empty() {
            *sidebar = new;
        }
        (imported, shown)
    });
    for error in &imported.errors {
        eprintln!("cannot import {error}");
    }
    let histories = imported.histories.into_iter();
    let histories: Vec<_> = histories
        .map(|(profile, entries)| (profile_dir(profile), entries))
        .collect();
    thread::spawn(move || {
        for (dir, entries) in histories {
            if let Err(error) = merge_history(&dir, &entries) {
                eprintln!("cannot merge the history: {error}");
            }
        }
    });
    close_import();
    if shown.is_some() {
        show_selected();
    }
}

/// Merges entries into the history of the profile in `dir`, with a connection of its own: the
/// browsing's may write to it meanwhile.
fn merge_history(dir: &Path, entries: &[history::Entry]) -> Result<(), String> {
    // An imported profile has no directory before its pages first load.
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let history = History::open(&dir.join(history::FILE_NAME));
    let mut history = history.map_err(|error| error.to_string())?;
    history.merge(entries).map_err(|error| error.to_string())
}

fn open_full_disk_access() {
    if cfg!(target_os = "macos")
        && let Err(error) = std::process::Command::new("open")
            .arg(FULL_DISK_ACCESS)
            .status()
    {
        eprintln!("cannot open System Settings: {error}");
    }
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
        remove_tab(tab)
    };
    if selected == Some(tab) {
        show_next(next);
    }
}

/// Removes an open tab, which may be reopened, and returns the tab to show instead.
fn remove_tab(tab: NodeId) -> Option<NodeId> {
    let next = STATE.with_borrow_mut(|state| {
        let closed = state.sidebar.tab_mut(tab).expect("an open tab").clone();
        state.closed.push(closed);
        state.sidebar.next_after_close(tab)
    });
    update_sidebar(|sidebar| sidebar.remove(tab)).expect("an open tab");
    next
}

/// Shows the tab that replaces the selected one, or the overlay when none is left.
fn show_next(next: Option<NodeId>) {
    match next {
        Some(next) => select_tab(next),
        None => {
            hide_content();
            show_overlay(true);
        }
    }
}

/// Closes the tabs of a folder, the selected one last, so the tab shown instead is not one about
/// to close.
fn close_folder_tabs(folder: NodeId) {
    let tabs = STATE.with_borrow(|state| {
        let mut tabs = state.sidebar.folder_tabs(folder).ok()?;
        let selected = state.sidebar.displayed().selected;
        tabs.sort_by_key(|&tab| Some(tab) == selected);
        Some(tabs)
    });
    for tab in tabs.unwrap_or_default() {
        close_tab(Some(tab));
    }
}

/// Removes a favorite for good, unlike closing it; it may be reopened like a closed tab.
fn remove_favorite(tab: NodeId) {
    let (favorite, selected) = STATE.with_borrow(|state| {
        let sidebar = &state.sidebar;
        (
            sidebar.is_favorite(tab),
            sidebar.displayed().selected == Some(tab),
        )
    });
    if !favorite {
        return;
    }
    let next = remove_tab(tab);
    if selected {
        show_next(next);
    }
}

/// Moves a tab to another Space; the selected tab takes the Space along, so it stays in view.
fn move_to_space(tab: NodeId, space: SpaceId) {
    let selected = STATE.with_borrow(|state| state.sidebar.displayed().selected == Some(tab));
    // Like a selection, an unknown tab or Space from a menu is ignored.
    if update_sidebar(|sidebar| sidebar.move_to_space(tab, space)).is_ok() && selected {
        select_tab(tab);
    }
}

/// Shows the menu of a sidebar row at the pointer.
fn show_menu(target: Target) {
    let Some(entries) = STATE.with_borrow(|state| menu::build(&state.sidebar, target)) else {
        return;
    };
    #[cfg(target_os = "macos")]
    crate::mac::pop_up(entries);
    #[cfg(not(target_os = "macos"))]
    let _ = entries;
}

/// Runs an item of a row's menu, as if it was picked.
fn choose_menu(target: Target, item: &str) {
    let entries = STATE.with_borrow(|state| menu::build(&state.sidebar, target));
    if let Some(action) = entries.and_then(|entries| menu::find(entries, item)) {
        perform(action);
    }
}

/// Runs the item picked in a menu.
pub fn perform(action: Action) {
    match action {
        Action::Run(command) => run(command),
        Action::Ask(event, detail) => ask(event, &detail),
        #[cfg(target_os = "macos")]
        Action::CopyLink(url) => crate::mac::copy(&url),
        #[cfg(target_os = "macos")]
        Action::Confirm {
            question,
            detail,
            button,
            command,
        } => {
            if crate::mac::confirm(&question, detail, button) {
                run(command);
            }
        }
        #[cfg(not(target_os = "macos"))]
        Action::CopyLink(_) | Action::Confirm { .. } => {}
    }
}

/// Dispatches an event in the sidebars, which takes the keyboard focus, even from the page.
fn ask(event: &str, detail: &serde_json::Value) {
    focus_sidebar();
    let event = format!(r#"new CustomEvent("{event}", {{ detail: {detail} }})"#);
    dispatch(&SIDEBAR, &event);
    dispatch(&REVEAL_VIEW, &event);
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

/// Creates a folder at the end of `parent`, or of the pinned tabs, and asks the sidebar to rename
/// it, even when the menu was used from the page. Like a selection, an unknown parent is ignored.
fn new_folder(parent: Option<NodeId>) {
    let folder = update_sidebar(|sidebar| {
        let parent = parent.unwrap_or(sidebar.displayed().pinned);
        sidebar.add_folder(parent, "New Folder")
    });
    if let Ok(folder) = folder {
        ask("arbalette:rename", &json!(folder));
    }
}

/// Gives the sidebar the keyboard focus, even from the page, whose card is a window of its own.
fn focus_sidebar() {
    if let Some(window) = WINDOW.with_borrow(Clone::clone) {
        window.activate();
    }
    if let Some(sidebar) = shown_sidebar().with_borrow(Clone::clone) {
        sidebar.request_focus();
    }
}

/// Creates a profile and asks the sidebar to rename it, like a new folder.
fn new_profile() {
    let profile = update_sidebar(|sidebar| {
        sidebar
            .create_profile("New Profile")
            .expect("a name that is not blank")
    });
    ask("arbalette:rename-profile", &json!(profile));
}

/// Creates a Space in the displayed Space's profile and shows it, with `tab` moved into it and
/// selected. Like a rename, a blank title from the UI is ignored, and like a selection, an unknown
/// tab.
fn create_space(title: &str, emoji: &str, color: Color, tab: Option<NodeId>) {
    let created = update_sidebar(|sidebar| {
        let profile = sidebar.displayed().profile;
        let space = sidebar.create_space(title, profile)?;
        sidebar.set_space_emoji(space, emoji)?;
        sidebar.set_space_color(space, color)?;
        if let Some(tab) = tab
            && sidebar.move_to_space(tab, space).is_ok()
        {
            sidebar.select(space, tab)?;
        }
        Ok::<_, arbalette_core::sidebar::Error>(())
    });
    if created.is_ok() {
        show_selected();
    }
}

/// Changes the Spaces, then shows the displayed Space's selected tab if the change displays
/// another Space or `space` itself, whose page may change. Like a selection, an unknown Space or
/// profile from the UI or a menu is ignored.
fn update_spaces(
    space: SpaceId,
    change: impl FnOnce(&mut Sidebar) -> Result<(), arbalette_core::sidebar::Error>,
) {
    let displayed = STATE.with_borrow(|state| state.sidebar.displayed().id);
    let changed = update_sidebar(change).is_ok();
    let now = STATE.with_borrow(|state| state.sidebar.displayed().id);
    if changed && (now != displayed || now == space) {
        show_selected();
    }
}

/// Attaches a Space to a new profile and asks the sidebar to rename it.
fn new_space_profile(space: SpaceId) {
    let profile = update_sidebar(|sidebar| {
        let profile = sidebar.add_profile("New Profile");
        sidebar.set_space_profile(space, profile).map(|()| profile)
    });
    if let Ok(profile) = profile {
        if STATE.with_borrow(|state| state.sidebar.displayed().id == space) {
            show_selected();
        }
        ask("arbalette:rename-profile", &json!(profile));
    }
}

/// Reopens a closed tab in the displayed Space, by its rank from the last closed one.
fn reopen_tab(rank: usize) {
    let closed = STATE.with_borrow_mut(|state| {
        let index = state.closed.len().checked_sub(rank + 1)?;
        Some(state.closed.remove(index))
    });
    let Some(closed) = closed else { return };
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

/// Shows the selected tab in its profile's content view, which is created the first time, or
/// leaves the card empty when the displayed Space has none.
fn show_selected() {
    let Some((profile, url)) = selected() else {
        hide_content();
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

/// Puts a profile's content view in the window instead of the shown one, if any.
fn show_content(profile: ProfileId, view: &BrowserView) {
    if SHOWN_PROFILE.get() == Some(profile) {
        return;
    }
    hide_content();
    SHOWN_PROFILE.set(Some(profile));
    view.set_visible(1);
    if let Some(panel) = card_panel() {
        panel.layout();
    }
    // The browser's window is the card's.
    #[cfg(target_os = "macos")]
    if let Some(host) = view.browser().and_then(|browser| browser.host()) {
        crate::mac::round_card(host.window_handle());
    }
}

/// Hides the shown content view, left on a blank page: only the selected tab runs. Without one,
/// the card shows its own color, an empty Space's panel.
fn hide_content() {
    if let Some(shown) = content() {
        load("about:blank");
        shown.set_visible(0);
    }
    SHOWN_PROFILE.set(None);
}

/// Paints the window around the sidebar and the card, and the card behind its content views. The
/// card shows from the sidebar's first colors on, so that it never flashes another color.
fn set_window_colors(window_color: u32, card_color: u32) {
    if let Some(window) = WINDOW.with_borrow(Clone::clone) {
        window.set_background_color(window_color);
    }
    if let Some(panel) = card_panel() {
        panel.set_background_color(card_color);
    }
    if let Some(card) = CARD.with_borrow(Clone::clone) {
        card.set_visible(1);
    }
}

/// Opens a profile's web data in its own directory, `Profile <id>`: Chromium only accepts profile
/// directories directly in the root cache path. CEF needs it initialized before a browser can
/// use it (ProfileContextHandler).
fn create_profile_context(profile: ProfileId) {
    CONTENTS.with_borrow_mut(|contents| contents.insert(profile, None));
    let dir = profile_dir(profile);
    let settings = RequestContextSettings {
        cache_path: CefString::from(dir.to_str().expect("a UTF-8 path")),
        ..Default::default()
    };
    request_context_create_context(
        Some(&settings),
        Some(&mut ProfileContextHandler::new(profile)),
    );
}

fn profile_dir(profile: ProfileId) -> PathBuf {
    let id = serde_json::to_string(&profile).expect("an id");
    DATA_DIR.with_borrow(|dir| dir.join(format!("Profile {id}")))
}

/// Changes a profile's history, kept in its directory beside its web data. A history that cannot
/// be written is not worth stopping the browsing for.
fn update_history<E: std::fmt::Display>(
    profile: ProfileId,
    change: impl FnOnce(&mut History) -> Result<(), E>,
) {
    HISTORIES.with_borrow_mut(|histories| {
        let history = match histories.entry(profile) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                // An imported profile has no directory before its pages first load.
                let dir = profile_dir(profile);
                if let Err(error) = std::fs::create_dir_all(&dir) {
                    return eprintln!("cannot create the profile's directory: {error}");
                }
                match History::open(&dir.join(history::FILE_NAME)) {
                    Ok(history) => entry.insert(history),
                    Err(error) => return eprintln!("cannot open the history: {error}"),
                }
            }
        };
        if let Err(error) = change(history) {
            eprintln!("cannot write the history: {error}");
        }
    });
}

fn now() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    since_epoch.as_millis().try_into().unwrap_or(i64::MAX)
}

wrap_request_context_handler! {
    struct ProfileContextHandler {
        profile: ProfileId,
    }

    impl RequestContextHandler {
        /// Adds the profile's content view to the window, hidden. Its browser is created after it
        /// (content_created).
        fn on_request_context_initialized(&self, context: Option<&mut RequestContext>) {
            let Some(card) = card_panel() else { return };
            let view = browser_view(ContentClient::new(self.profile), "about:blank", no_width, context);
            view.set_visible(0);
            card.add_child_view(Some(&mut View::from(&view)));
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
        (Command::ToggleSidebar, _) => {
            let has_tab = STATE.with_borrow(|state| state.sidebar.displayed().selected.is_some());
            set_sidebar(toggled(SIDEBAR_MODE.get(), has_tab));
        }
        (Command::RevealSidebar, _) if SIDEBAR_MODE.get() == SidebarMode::Hidden => {
            set_sidebar(SidebarMode::Revealed);
        }
        (Command::ConcealSidebar, _) if SIDEBAR_MODE.get() == SidebarMode::Revealed => {
            set_sidebar(SidebarMode::Hidden);
        }
        (Command::RevealSidebar | Command::ConcealSidebar, _) => {}
        (Command::CardOffset(offset), _) => shift_card(offset),
        (Command::ShowLibrary, _) => show_library(),
        (Command::Open(url), _) => {
            show_overlay(false);
            open_tab(&url);
        }
        (Command::Close(tab), _) => close_tab(tab),
        (Command::Reopen(rank), _) => reopen_tab(rank),
        (Command::Pin(tab), _) => toggle_pin(tab),
        (Command::NewFolder(parent), _) => new_folder(parent),
        (Command::CloseFolderTabs(folder), _) => close_folder_tabs(folder),
        (Command::RemoveFavorite(tab), _) => remove_favorite(tab),
        (Command::MoveToSpace(tab, space), _) => move_to_space(tab, space),
        (Command::Menu(target), _) => show_menu(target),
        (Command::MenuTargets { revealed, targets }, _) => {
            MENU_TARGETS.with_borrow_mut(|all| all[usize::from(revealed)] = targets);
        }
        (Command::ChooseMenu(target, item), _) => choose_menu(target, &item),
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
        (
            Command::CreateSpace {
                title,
                emoji,
                color,
                tab,
            },
            _,
        ) => create_space(&title, &emoji, color, tab),
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
        (Command::ToggleSpace(id), _) => {
            let _ = update_sidebar(|sidebar| sidebar.toggle_space(id));
        }
        // The selected tab shows in its new profile's web data.
        (Command::SetSpaceProfile(id, profile), _) => {
            update_spaces(id, |sidebar| sidebar.set_space_profile(id, profile));
        }
        (Command::NewSpaceProfile(id), _) => new_space_profile(id),
        (Command::DeleteSpace(id), _) => update_spaces(id, |sidebar| sidebar.delete_space(id)),
        (Command::ShowImport, _) => show_import(),
        (Command::ImportReady, _) => send_sources(),
        (Command::CloseImport, _) => close_import(),
        (Command::Import(choices), _) => run_import(choices),
        (Command::OpenFullDiskAccess, _) => open_full_disk_access(),
        (Command::WindowColors { window, card }, _) => set_window_colors(window, card),
        (Command::Back, Some(browser)) => browser.go_back(),
        (Command::Forward, Some(browser)) => browser.go_forward(),
        (Command::Reload, Some(browser)) => browser.reload(),
        (_, None) => {}
    }
}

/// Whether a point at `x` from the window's left edge is over the sidebar, shown or revealed.
#[cfg(target_os = "macos")]
pub fn is_over_sidebar(x: f64) -> bool {
    let width = match SIDEBAR_MODE.get() {
        SidebarMode::Hidden => return false,
        SidebarMode::Shown => docked_width(),
        SidebarMode::Revealed | SidebarMode::Floating => SIDEBAR_WIDTH,
    };
    x < f64::from(width)
}

/// Sends the swipes among the window's scroll events to the sidebar the window shows.
#[cfg(target_os = "macos")]
pub fn scroll(scroll: crate::swipe::Scroll) {
    use crate::swipe::Swipe;
    match SWIPE.with_borrow_mut(|tracker| tracker.track(scroll)) {
        Some(Swipe::Gesture(gesture)) => {
            let view = shown_sidebar();
            let detail = serde_json::to_string(&gesture).expect("a gesture");
            dispatch(
                view,
                &format!(r#"new CustomEvent("arbalette:swipe", {{ detail: {detail} }})"#),
            );
        }
        Some(Swipe::Step(next)) => show_found_space(|s| s.next_space(next)),
        None => {}
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

        // The card, the revealed sidebar and the margins follow the window's size.
        // A slide places them on its next step.
        fn on_layout_changed(&self, _view: Option<&mut View>, _new_bounds: Option<&Rect>) {
            if SLIDE.with_borrow(Option::is_none) {
                place_card();
                place_reveal();
            }
            update_drag_regions();
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let window = window.expect("window").clone();
            window.set_title(Some(&CefString::from("Arbalette")));
            // The sidebar at the left edge, the rest of the window its own background, below the
            // card and the other overlays.
            window.set_background_color(WINDOW_COLOR);
            let layout = window
                .set_to_box_layout(Some(&BoxLayoutSettings {
                    horizontal: 1,
                    cross_axis_alignment: AxisAlignment::STRETCH,
                    ..Default::default()
                }))
                .expect("box layout");
            let sidebar = browser_view(UiClient::new(), &format!("{UI_ORIGIN}index.html"), docked_width, None);
            let mut view = View::from(&sidebar);
            window.add_child_view(Some(&mut view));
            layout.set_flex_for_view(Some(&mut view), 0);
            SIDEBAR.set(Some(sidebar));
            let card = panel_create(None).expect("card");
            card.set_to_fill_layout();
            let controller = window
                .add_overlay_view(Some(&mut View::from(&card)), DockingMode::CUSTOM, 1)
                .expect("card");
            controller.set_visible(0);
            CARD.set(Some(controller));
            // Below the overlay, which is added after it.
            let reveal = browser_view(UiClient::new(), &format!("{UI_ORIGIN}index.html#reveal"), no_width, None);
            let controller = window
                .add_overlay_view(Some(&mut View::from(&reveal)), DockingMode::CUSTOM, 1)
                .expect("reveal");
            controller.set_visible(0);
            REVEAL_VIEW.set(Some(reveal));
            REVEAL.set(Some(controller));
            let overlay = browser_view(UiClient::new(), &format!("{UI_ORIGIN}index.html#overlay"), no_width, None);
            let controller = window
                .add_overlay_view(Some(&mut View::from(&overlay)), DockingMode::CUSTOM, 1)
                .expect("overlay");
            controller.set_visible(0);
            OVERLAY_VIEW.set(Some(overlay));
            OVERLAY.set(Some(controller));
            window.show();
            WINDOW.set(Some(window));
            place_card();
            // Only the selected tab loads: the others wait until they are selected.
            show_selected();
            if IMPORT_AT_LAUNCH.take() {
                show_import();
            }
        }

        /// Asks every browser to close; the window closes once they all have.
        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            let mut views: Vec<_> = [&SIDEBAR, &REVEAL_VIEW, &OVERLAY_VIEW, &IMPORT_VIEW]
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

/// An Alloy browser view, as wide as `width` says when the layout asks, or sized by the layout or
/// its overlay's bounds for 0 (no_width). Without a request context, it uses the global one,
/// which only the app's own UI uses.
fn browser_view(
    mut client: Client,
    url: &str,
    width: fn() -> i32,
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

fn no_width() -> i32 {
    0
}

wrap_browser_view_delegate! {
    struct AlloyViewDelegate {
        width: fn() -> i32,
    }

    impl ViewDelegate {
        // A zero height would make the size "empty" and CEF would ignore it.
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: (self.width)(), height: 1 }
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

/// The sidebar views a right-click may land on.
#[derive(Debug, PartialEq)]
enum SidebarView {
    Docked,
    /// Above the page, or the strip at the window's edge that reveals it.
    Revealed,
}

/// The sidebar view under a point of a window of `size`, in points from its top left corner,
/// with the point in the view's coordinates.
fn sidebar_point(
    mode: SidebarMode,
    size: &Size,
    x: f64,
    y: f64,
) -> Option<(SidebarView, f64, f64)> {
    let (view, rect) = match mode {
        SidebarMode::Shown => {
            let rect = Rect {
                x: 0,
                y: 0,
                width: docked_width(),
                height: size.height,
            };
            (SidebarView::Docked, rect)
        }
        _ => (SidebarView::Revealed, mode.layout(size).reveal),
    };
    let (left, top) = (f64::from(rect.x), f64::from(rect.y));
    let inside = (left..left + f64::from(rect.width)).contains(&x)
        && (top..top + f64::from(rect.height)).contains(&y);
    inside.then_some((view, x - left, y - top))
}

/// The target under a point of a sidebar view, if any.
fn target_at(targets: &[HitTarget], x: f64, y: f64) -> Option<Target> {
    let target = targets.iter().find(|target| target.contains(x, y));
    target.map(|target| target.target)
}

/// Shows the native menu of what is under a point of the window, in points from its top left
/// corner, at once like a native view, and returns whether a sidebar is there. Real right-clicks
/// reach neither the UI's `contextmenu` listener on macOS nor CEF's context menu handler, and
/// would show Chromium's page menu: the sidebars send where their rows are instead
/// (Command::MenuTargets).
#[cfg(target_os = "macos")]
pub fn menu_at(x: f64, y: f64) -> bool {
    let Some(size) = WINDOW.with_borrow(|window| window.as_ref().map(|w| w.size())) else {
        return false;
    };
    let Some((view, x, y)) = sidebar_point(SIDEBAR_MODE.get(), &size, x, y) else {
        return false;
    };
    let revealed = view == SidebarView::Revealed;
    let target =
        MENU_TARGETS.with_borrow(|targets| target_at(&targets[usize::from(revealed)], x, y));
    if let Some(target) = target {
        show_menu(target);
    }
    true
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
            update_drag_regions();
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
            Some(ContentDisplayHandler::new(self.profile))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(ContentLoadHandler::new(self.profile))
        }
    }
}

wrap_display_handler! {
    struct ContentDisplayHandler {
        profile: ProfileId,
    }

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
            let title = title.map(CefString::to_string).unwrap_or_default();
            if let Some(frame) = browser.as_ref().and_then(|browser| browser.main_frame()) {
                let url = CefString::from(&frame.url()).to_string();
                update_history(self.profile, |history| history.set_title(&url, &title));
            }
            if !is_shown(browser) {
                return;
            }
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
    struct ContentLoadHandler {
        profile: ProfileId,
    }

    impl LoadHandler {
        /// A page's navigation is committed: it is a visit, whether or not the page loads fully.
        fn on_load_start(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, _transition_type: TransitionType) {
            let Some(frame) = frame.filter(|frame| frame.is_main() == 1) else { return };
            let url = CefString::from(&frame.url()).to_string();
            update_history(self.profile, |history| history.visit(&url, now()));
        }

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
    use super::{
        CARD_MARGIN, DraggableRegion, EDGE_WIDTH, FLOAT_INSET, LIBRARY, LIBRARY_WIDTH, Rect,
        SIDEBAR_WIDTH, SidebarMode, Size, UiState, lerp_rect, margin_regions, slide_duration,
        toggled, ui_file, ui_path,
    };

    fn rect(rect: &Rect) -> (i32, i32, i32, i32) {
        (rect.x, rect.y, rect.width, rect.height)
    }

    fn regions(mode: SidebarMode) -> Vec<(i32, i32, i32, i32)> {
        let regions = margin_regions(&SIZE, &mode.layout(&SIZE).card, mode.margins_start());
        assert!(regions.iter().all(|region| region.draggable == 1));
        bounds(&regions)
    }

    #[test]
    fn the_margins_around_the_card_move_the_window_without_the_sidebar() {
        let m = CARD_MARGIN;
        let margins = [
            (0, 0, 1000, m),
            (0, 600 - m, 1000, m),
            (0, 0, m, 600),
            (1000 - m, 0, m, 600),
        ];
        assert_eq!(regions(SidebarMode::Hidden), margins);
        assert_eq!(regions(SidebarMode::Revealed), margins);
    }

    #[test]
    fn the_margins_start_past_the_sidebar_while_it_is_docked() {
        let (m, s) = (CARD_MARGIN, SIDEBAR_WIDTH);
        assert_eq!(
            regions(SidebarMode::Shown),
            [
                (s, 0, 1000 - s, m),
                (s, 600 - m, 1000 - s, m),
                (s, 0, m, 600),
                (1000 - m, 0, m, 600)
            ]
        );
    }

    #[test]
    fn the_window_moves_from_around_the_floating_sidebar() {
        let (m, s) = (CARD_MARGIN, SIDEBAR_WIDTH);
        assert_eq!(
            regions(SidebarMode::Floating),
            [
                (0, 0, 1000, m),
                (0, 600 - m, 1000, m),
                (0, 0, s + m, 600),
                (1000 - m, 0, m, 600)
            ]
        );
    }

    #[test]
    fn the_card_keeps_its_margin_on_all_sides_without_the_sidebar() {
        let m = CARD_MARGIN;
        let card = (m, m, 1000 - 2 * m, 600 - 2 * m);
        assert_eq!(rect(&SidebarMode::Hidden.layout(&SIZE).card), card);
        // The revealed sidebar floats above the page without resizing it.
        assert_eq!(rect(&SidebarMode::Revealed.layout(&SIZE).card), card);
        let docked = (
            SIDEBAR_WIDTH + m,
            m,
            1000 - SIDEBAR_WIDTH - 2 * m,
            600 - 2 * m,
        );
        assert_eq!(rect(&SidebarMode::Shown.layout(&SIZE).card), docked);
        // An empty Space's panel stays where it is while the sidebar floats beside it.
        assert_eq!(rect(&SidebarMode::Floating.layout(&SIZE).card), docked);
    }

    #[test]
    fn the_card_rests_beside_the_wider_library_while_the_sidebar_is_docked() {
        let m = CARD_MARGIN;
        LIBRARY.set(true);
        let docked = rect(&SidebarMode::Shown.layout(&SIZE).card);
        let start = SidebarMode::Shown.margins_start();
        let hidden = rect(&SidebarMode::Hidden.layout(&SIZE).card);
        LIBRARY.set(false);
        assert_eq!(
            docked,
            (
                LIBRARY_WIDTH + m,
                m,
                1000 - LIBRARY_WIDTH - 2 * m,
                600 - 2 * m
            )
        );
        assert_eq!(start, LIBRARY_WIDTH);
        // Hidden, the sidebar leaves the card its margins whatever it shows.
        assert_eq!(hidden, (m, m, 1000 - 2 * m, 600 - 2 * m));
    }

    #[test]
    fn the_floating_sidebar_is_inset_from_the_window_edges() {
        let i = FLOAT_INSET;
        let floating = (i, i, SIDEBAR_WIDTH, 600 - 2 * i);
        assert_eq!(rect(&SidebarMode::Revealed.layout(&SIZE).reveal), floating);
        assert_eq!(rect(&SidebarMode::Floating.layout(&SIZE).reveal), floating);
        assert_eq!(
            rect(&SidebarMode::Hidden.layout(&SIZE).reveal),
            (0, 0, EDGE_WIDTH, 600)
        );
        // It docks into the docked sidebar's place.
        let docked = (0, 0, SIDEBAR_WIDTH, 600);
        assert_eq!(rect(&SidebarMode::Shown.layout(&SIZE).reveal), docked);
    }

    #[test]
    fn the_toggle_hides_the_sidebar_beside_a_page_and_floats_it_beside_an_empty_panel() {
        assert_eq!(toggled(SidebarMode::Shown, true), SidebarMode::Hidden);
        assert_eq!(toggled(SidebarMode::Shown, false), SidebarMode::Floating);
        for mode in [
            SidebarMode::Hidden,
            SidebarMode::Revealed,
            SidebarMode::Floating,
        ] {
            assert_eq!(toggled(mode, true), SidebarMode::Shown);
            assert_eq!(toggled(mode, false), SidebarMode::Shown);
        }
    }

    #[test]
    fn the_sidebar_slides_quickly_and_floats_beside_an_empty_panel_at_once() {
        use SidebarMode::{Floating, Hidden, Revealed, Shown};
        assert_eq!(slide_duration(Shown, Hidden), slide_duration(Hidden, Shown));
        assert!((80..=90).contains(&slide_duration(Shown, Hidden)));
        assert!(slide_duration(Hidden, Revealed) > slide_duration(Revealed, Hidden));
        assert!(slide_duration(Revealed, Shown) < slide_duration(Shown, Hidden));
        assert!(slide_duration(Floating, Shown) > 0);
        assert_eq!(slide_duration(Shown, Floating), 0);
    }

    #[test]
    fn interpolates_rectangles() {
        let from = Rect {
            x: 269,
            y: 9,
            width: 722,
            height: 582,
        };
        let to = SidebarMode::Hidden.layout(&SIZE).card;
        assert_eq!(rect(&lerp_rect(&from, &to, 0.0)), rect(&from));
        assert_eq!(rect(&lerp_rect(&from, &to, 0.5)), (139, 9, 852, 582));
        assert_eq!(rect(&lerp_rect(&from, &to, 1.0)), rect(&to));
    }

    const SIZE: Size = Size {
        width: 1000,
        height: 600,
    };

    fn bounds(regions: &[DraggableRegion]) -> Vec<(i32, i32, i32, i32)> {
        regions
            .iter()
            .map(|region| {
                let rect = &region.bounds;
                (rect.x, rect.y, rect.width, rect.height)
            })
            .collect()
    }

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
        let page = &json["pages"][0];
        assert_eq!(page["space"], serde_json::json!(home));
        assert_eq!(page["today"][0]["url"], "https://example.com/");
        assert_eq!(page["today"][0]["id"], page["selected"]);
        let roots = serde_json::json!({
            "favorites": state.sidebar.profiles()[0].favorites,
            "pinned": state.sidebar.spaces()[0].pinned,
            "today": state.sidebar.spaces()[0].today,
        });
        assert_eq!(page["roots"], roots);
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
        let page = &state.to_json()["pages"][0];
        assert_eq!(page["pinned"][0]["title"], "Docs");
        assert_eq!(page["pinned"][0]["expanded"], true);
        assert_eq!(
            page["pinned"][0]["children"][0]["url"],
            "https://example.com/"
        );
        assert_eq!(page["today"], serde_json::json!([]));
    }

    #[test]
    fn sends_each_space_with_the_favorites_of_its_profile() {
        let mut state = UiState::default();
        let home = state.sidebar.spaces()[0].id;
        let favorites = state.sidebar.profiles()[0].favorites;
        let tab = state
            .sidebar
            .open_tab(home, "https://example.com/")
            .unwrap();
        state.sidebar.move_node(tab, favorites, 0).unwrap();
        state.sidebar.create_profile("Work").unwrap();
        let json = state.to_json();
        let page = &json["pages"][0];
        assert_eq!(page["favorites"][0]["url"], "https://example.com/");
        assert_eq!(page["favorites"][0]["id"], serde_json::json!(tab));
        assert_eq!(page["today"], serde_json::json!([]));
        let office = &json["pages"][1];
        assert_eq!(office["favorites"], serde_json::json!([]));
        let work_favorites = state.sidebar.profiles()[1].favorites;
        assert_eq!(
            office["roots"]["favorites"],
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
        let page = &json["pages"][1];
        assert_eq!(page["space"], serde_json::json!(work));
        assert_eq!(page["today"][0]["url"], "https://example.org/");
        assert_eq!(page["today"].as_array().unwrap().len(), 1);
        assert_eq!(page["selected"], serde_json::json!(tab));
        assert_eq!(json["pages"][0]["today"][0]["url"], "https://example.com/");
    }

    #[test]
    fn sends_only_the_pages_of_the_displayed_space_and_its_neighbors() {
        let mut state = UiState::default();
        let profile = state.sidebar.profiles()[0].id;
        let ids: Vec<_> = (1..5)
            .map(|n| {
                state
                    .sidebar
                    .add_space(&format!("Space {n}"), profile)
                    .unwrap()
            })
            .collect();
        let pages = |state: &UiState| {
            let json = state.to_json();
            let pages = json["pages"].as_array().unwrap();
            pages
                .iter()
                .map(|page| page["space"].clone())
                .collect::<serde_json::Value>()
        };
        let home = state.sidebar.spaces()[0].id;
        assert_eq!(pages(&state), serde_json::json!([home, ids[0]]));
        state.sidebar.display(ids[1]).unwrap();
        assert_eq!(pages(&state), serde_json::json!(ids[..3]));
        state.sidebar.display(ids[3]).unwrap();
        assert_eq!(pages(&state), serde_json::json!(ids[2..]));
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
    fn finds_the_sidebar_under_a_point_in_its_own_coordinates() {
        use super::{SidebarView, sidebar_point};
        let at = |mode, x, y| sidebar_point(mode, &SIZE, x, y);
        assert_eq!(
            at(SidebarMode::Shown, 10.0, 300.0),
            Some((SidebarView::Docked, 10.0, 300.0))
        );
        assert_eq!(at(SidebarMode::Shown, 270.0, 300.0), None);
        let inset = f64::from(FLOAT_INSET);
        for mode in [SidebarMode::Revealed, SidebarMode::Floating] {
            assert_eq!(
                at(mode, 10.0, 300.0),
                Some((SidebarView::Revealed, 10.0 - inset, 300.0 - inset))
            );
            assert_eq!(at(mode, 2.0, 300.0), None);
            assert_eq!(at(mode, 270.0, 300.0), None);
        }
        assert_eq!(
            at(SidebarMode::Hidden, 2.0, 300.0),
            Some((SidebarView::Revealed, 2.0, 300.0))
        );
        assert_eq!(at(SidebarMode::Hidden, 10.0, 300.0), None);
        LIBRARY.set(true);
        assert_eq!(
            at(SidebarMode::Shown, 400.0, 300.0),
            Some((SidebarView::Docked, 400.0, 300.0))
        );
        LIBRARY.set(false);
    }

    #[test]
    fn finds_the_target_under_a_point() {
        let targets: Vec<super::HitTarget> = serde_json::from_str(
            r#"[{"target":{"space":4},"x":7,"y":100,"width":250,"height":32},
                {"target":{"node":12},"x":7,"y":140,"width":250,"height":41}]"#,
        )
        .unwrap();
        let at = |y| super::target_at(&targets, 20.0, y).map(|t| format!("{t:?}"));
        assert_eq!(at(110.0), Some(format!("{:?}", targets[0].target)));
        assert_eq!(at(180.0), Some(format!("{:?}", targets[1].target)));
        assert_eq!(at(135.0), None);
        assert_eq!(at(181.0), None);
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
