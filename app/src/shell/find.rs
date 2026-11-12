//! The find bar above the page, which finds text in it. Each page has its own, shown or not, with
//! its own text and matches: the window shows the one of the page it searches.

use super::*;

/// The find bar's size, and its margin from the page's top right corner (app/macos/Panels.swift):
/// Arc's, measured from a recording.
const FIND_SIZE: Size = Size {
    width: 300,
    height: 32,
};
const FIND_MARGIN: i32 = 10;

/// A page's find bar.
#[derive(Default)]
struct Find {
    shown: bool,
    /// The text last searched for, which ⌘G searches again.
    text: String,
    /// The matches found, and the active one.
    found: Option<(i32, i32)>,
}

thread_local! {
    /// Whether the search last sent is a new one, which CEF ends without an active match.
    static NEW_SEARCH: Cell<bool> = const { Cell::new(false) };
    /// Each page's find bar, by its browser: a tab's page's or Peek's.
    static FINDS: RefCell<HashMap<i32, Find>> = RefCell::default();
}

/// Shows the find bar with the keyboard, the text last searched for selected and, like Arc's,
/// searched for again.
pub(super) fn show_find() {
    let Some(browser) = find_page() else {
        return;
    };
    let id = browser.identifier();
    let text = FINDS.with_borrow_mut(|finds| {
        let find = finds.entry(id).or_default();
        find.shown = true;
        find.found = None;
        find.text.clone()
    });
    make_find();
    win().find_for.set(Some(id));
    place_find();
    if let Some(controller) = win().find.with_borrow(Clone::clone) {
        controller.set_visible(1);
    }
    send_find(&text, None, true);
    if !text.is_empty()
        && let Some(host) = browser.host()
    {
        NEW_SEARCH.set(true);
        host.find(Some(&CefString::from(text.as_str())), 1, 0, 0);
    }
}

/// Tells the find bar what it shows: its text, its matches, and whether it takes the keyboard.
fn send_find(text: &str, found: Option<(i32, i32)>, focus: bool) {
    let detail = json!({
        "text": text,
        "found": found.map(|(count, active)| json!({ "count": count, "active": active })),
        "focus": focus,
    });
    dispatch(&win().find_view, "arbalete:show", &detail.to_string());
}

/// Makes the find bar's place, once.
fn make_find() {
    if win().find.with_borrow(Option::is_none) {
        let view = place_view(no_width, 1);
        let controller = Spot::default();
        controller.set_visible(0);
        win().find_view.set(Some(view));
        win().find.set(Some(controller));
    }
}

/// Shows the find bar of the page searched, or none, as another page comes to be searched: another
/// tab, a split's other page, Peek's; and keeps it in place.
pub(super) fn sync_find() {
    let Some(controller) = win().find.with_borrow(Clone::clone) else {
        return;
    };
    let page = find_page().map(|browser| browser.identifier());
    if win().find_for.replace(page) != page {
        let shown = page.and_then(|id| {
            FINDS.with_borrow(|finds| {
                let find = finds.get(&id).filter(|find| find.shown)?;
                Some((find.text.clone(), find.found))
            })
        });
        controller.set_visible(shown.is_some().into());
        if let Some((text, found)) = shown {
            send_find(&text, found, false);
        }
    }
    place_find();
}

/// Keeps the find bar at the top right corner of the page it searches: Peek's, under its bar, or
/// the shown page's, under the bar above it or its split's.
pub(super) fn place_find() {
    let Some(controller) = win().find.with_borrow(Clone::clone) else {
        return;
    };
    let page = if peek_shown() {
        let Some(size) = win()
            .window
            .with_borrow(|window| window.as_ref().map(|w| w.size()))
        else {
            return;
        };
        let frame = peek_frame(&size);
        Rect {
            y: frame.y + PEEK_BAR,
            height: frame.height - PEEK_BAR,
            ..frame
        }
    } else {
        let Some(frame) = page_frame() else {
            return;
        };
        frame
    };
    let width = FIND_SIZE.width.min(page.width - 2 * FIND_MARGIN);
    controller.set_bounds(Some(&Rect {
        x: page.x + page.width - width - FIND_MARGIN,
        y: page.y + FIND_MARGIN,
        width,
        height: FIND_SIZE.height,
    }));
}

/// The page find in page acts on: Peek's while it shows, or the shown one.
pub(super) fn find_page() -> Option<Browser> {
    PEEK.with_borrow(|peek| peek.as_ref().map(|peek| peek.browser.clone()))
        .or_else(content)
}

/// Finds text in `browser`, find_page's; the same text again goes to the next match. `None`
/// searches the page's last text again, or shows the find bar when there is none.
pub(super) fn find(browser: &Browser, text: Option<String>, forward: bool) {
    let id = browser.identifier();
    let (text, next) = FINDS.with_borrow_mut(|finds| {
        let find = finds.entry(id).or_default();
        let text = text.unwrap_or_else(|| find.text.clone());
        let next = std::mem::replace(&mut find.text, text.clone()) == text;
        if text.is_empty() {
            find.found = None;
        }
        (text, next)
    });
    let Some(host) = browser.host() else {
        return;
    };
    if text.is_empty() {
        host.stop_finding(1);
        return show_find_if_hidden(id);
    }
    NEW_SEARCH.set(!next);
    host.find(
        Some(&CefString::from(text.as_str())),
        forward.into(),
        0,
        next.into(),
    );
}

/// Tells `browser`'s find bar how many matches the search that ended found, and which one is
/// active; a new search goes to its first match first, like Arc's, rather than leaving none active.
pub(super) fn found(browser: &Browser, count: i32, active: i32) {
    let id = browser.identifier();
    let searched = win().find_for.get() == Some(id);
    if searched
        && NEW_SEARCH.replace(false)
        && count > 0
        && active == 0
        && let Some(host) = browser.host()
    {
        let text = FINDS.with_borrow(|finds| finds.get(&id).map(|find| find.text.clone()));
        let text = text.unwrap_or_default();
        return host.find(Some(&CefString::from(text.as_str())), 1, 0, 1);
    }
    FINDS.with_borrow_mut(|finds| {
        if let Some(find) = finds.get_mut(&id).filter(|find| !find.text.is_empty()) {
            find.found = Some((count, active));
        }
    });
    if searched {
        let found = json!({ "count": count, "active": active });
        dispatch(&win().find_view, "arbalete:found", &found.to_string());
    }
}

/// From ⌘G with nothing to search again.
fn show_find_if_hidden(id: i32) {
    if !FINDS.with_borrow(|finds| finds.get(&id).is_some_and(|find| find.shown)) {
        show_find();
    }
}

/// Hides the find bar and the page's highlights, keeping the active match selected and the text,
/// which ⌘G searches again; the page gets the keyboard back.
pub(super) fn hide_find() {
    close_find();
    if let Some(host) = PEEK.with_borrow(|peek| peek.as_ref()?.browser.host()) {
        host.set_focus(1);
    } else if let Some(tab) = win().shown.get() {
        focus_page(tab);
    }
}

/// Hides the find bar and the highlights of the page it searches, as Peek closes.
pub(super) fn close_find() {
    let Some(browser) = find_page() else {
        return;
    };
    FINDS.with_borrow_mut(|finds| {
        if let Some(find) = finds.get_mut(&browser.identifier()) {
            find.shown = false;
            find.found = None;
        }
    });
    if win().find_for.get() == Some(browser.identifier())
        && let Some(controller) = win().find.with_borrow(Clone::clone)
    {
        controller.set_visible(0);
    }
    if let Some(host) = browser.host() {
        host.stop_finding(0);
    }
}
