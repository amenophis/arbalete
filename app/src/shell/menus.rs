//! The native menus of the sidebar's rows, its Spaces, the site and the extensions, like Arc's.

use super::*;

/// Shows the menu of a sidebar row at the pointer.
pub(super) fn show_menu(target: Target) {
    if let Target::Pinned(rank) = target {
        show_extension_menu(pinned_extensions(displayed_profile()).get(rank));
        return;
    }
    let Some(entries) = STATE.with_borrow(|state| menu::build(&state.sidebar, target)) else {
        return;
    };
    #[cfg(target_os = "macos")]
    crate::mac::pop_up(entries);
}

/// Shows the developer bar's extensions menu at the pointer, like Arc's: the displayed Space's
/// profile's extensions turned on, then adding and managing them.
pub(super) fn show_extensions_menu() {
    let extensions: Vec<_> = extensions::list(extension_profile())
        .into_iter()
        .filter(|extension| extension.enabled)
        .map(|extension| (extension.id, extension.name))
        .collect();
    let entries = menu::extensions_menu(&extensions);
    #[cfg(target_os = "macos")]
    crate::mac::pop_up(entries);
}

/// The profile whose extensions act: the Little Arbalete window's in front last, if any, else the
/// displayed Space's.
pub(super) fn extension_profile() -> ProfileId {
    let little = LITTLE.with_borrow(|littles| {
        Some(
            littles
                .iter()
                .find(|l| Some(l.id) == ACTIVE_LITTLE.get())?
                .profile,
        )
    });
    little.unwrap_or_else(displayed_profile)
}

/// Shows the menu of an extension of the displayed Space's profile, by its id.
pub(super) fn show_extension_menu(id: Option<&String>) {
    let profile = displayed_profile();
    let extension = id.and_then(|id| extensions::list(profile).into_iter().find(|e| &e.id == id));
    let Some(extension) = extension else { return };
    let pinned = pinned_extensions(profile).contains(&extension.id);
    let entries = menu::extension_menu(
        &extension.id,
        &extension.name,
        extension.options.is_some(),
        pinned,
    );
    #[cfg(target_os = "macos")]
    crate::mac::pop_up(entries);
}

/// Runs an item of a row's menu, as if it was picked.
pub(super) fn choose_menu(target: Target, item: &str) {
    let entries = STATE.with_borrow(|state| menu::build(&state.sidebar, target));
    if let Some(action) = entries.and_then(|entries| menu::find(entries, item)) {
        perform(action);
    }
}

/// Runs the item picked in a menu.
pub fn perform(action: Action) {
    match action {
        Action::Run(command) => run(command),
        Action::RunEach(commands) => commands.into_iter().for_each(run),
        Action::Ask(event, detail) => ask(event, &detail),
        #[cfg(target_os = "macos")]
        Action::CopyLink(url) => crate::mac::copy(&url),
        #[cfg(target_os = "macos")]
        Action::Share(url) => crate::mac::share(&url),
        #[cfg(target_os = "macos")]
        Action::EditPinnedUrl { tab, url } => {
            let detail = "This is what opens first when you click this tab.";
            if let Some(url) = crate::mac::prompt("Edit Pinned URL", detail, &url, "Save") {
                run(Command::SetPinnedUrl(tab, url));
            }
        }
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
    }
}

/// Dispatches an event in the sidebars, which takes the keyboard focus, even from the page.
pub(super) fn ask(event: &str, detail: &serde_json::Value) {
    focus_sidebar();
    let detail = detail.to_string();
    dispatch(&win().sidebar, event, &detail);
    dispatch(&win().reveal_view, event, &detail);
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
pub(super) fn target_at(targets: &[HitTarget], x: f64, y: f64) -> Option<Target> {
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
    let Some(size) = win()
        .window
        .with_borrow(|window| window.as_ref().map(|w| w.size()))
    else {
        return false;
    };
    // The site controls' panel, above the sidebar, shows its extensions' menu.
    let panel = win().site.with_borrow(|site| {
        let site = site.as_ref().filter(|site| site.is_visible() == 1)?;
        Some(site.bounds())
    });
    if let Some(panel) = panel {
        let (x, y) = (x - f64::from(panel.x), y - f64::from(panel.y));
        if (0.0..f64::from(panel.width)).contains(&x) && (0.0..f64::from(panel.height)).contains(&y)
        {
            let id = win().site_targets.with_borrow(|targets| {
                let target = targets.iter().find(|t| {
                    (t.x..t.x + t.width).contains(&x) && (t.y..t.y + t.height).contains(&y)
                })?;
                Some(target.id.clone())
            });
            show_extension_menu(id.as_ref());
            return true;
        }
    }
    // The bar above the page, in toolbar mode, shows its pinned extensions' menus.
    if shown_toolbar_height() > 0 {
        // At the card's top.
        let card = card_rect().unwrap_or_default();
        let bounds = Rect {
            x: 0,
            y: 0,
            width: card.width,
            height: shown_toolbar_height(),
        };
        let point = Point {
            x: x as i32 - card.x - bounds.x,
            y: y as i32 - card.y - bounds.y,
        };
        if (0..bounds.width).contains(&point.x) && (0..bounds.height).contains(&point.y) {
            let targets = |all: &[Vec<HitTarget>; 3]| {
                target_at(
                    &all[ButtonView::Toolbar as usize],
                    f64::from(point.x),
                    f64::from(point.y),
                )
            };
            if let Some(target) = win().menu_targets.with_borrow(targets) {
                show_menu(target);
            }
            return true;
        }
    }
    let Some((view, x, y)) = sidebar_point(win().sidebar_mode.get(), &size, x, y) else {
        return false;
    };
    let view = match view {
        SidebarView::Docked => ButtonView::Docked,
        SidebarView::Revealed => ButtonView::Revealed,
    };
    let target = win()
        .menu_targets
        .with_borrow(|targets| target_at(&targets[view as usize], x, y));
    if let Some(target) = target {
        show_menu(target);
    }
    true
}

/// The pages a history menu lists at most, the nearest first, like Chrome's.
const HISTORY_MENU_PAGES: usize = 12;

/// Shows the page's earlier pages, the last one first, or its later ones, the next one first,
/// with their titles and icons, in a popover under the button right-clicked, like Arc's: picking
/// one goes straight to it (Command::GoInHistory).
pub(super) fn show_history_menu(browser: &Browser, forward: bool) {
    let Some(host) = browser.host() else { return };
    let pages = Rc::new(RefCell::new(Vec::new()));
    // It visits every page in order, at once on the UI thread.
    let mut visitor = HistoryVisitor::new(pages.clone());
    host.navigation_entries(Some(&mut visitor), 0);
    let pages = pages.take();
    let Some(current) = pages.iter().position(|page: &HistoryPage| page.current) else {
        return;
    };
    // Like the Command Bar's pages, each shows the icon Chromium found for it or its site.
    let profile = displayed_profile();
    let dir = profile_dir(profile);
    let chromium = favicon::chromium(&dir);
    let pages: Vec<_> = history_offsets(pages.len(), current, forward)
        .into_iter()
        .map(|offset| {
            let page = &pages[(current as i32 + offset) as usize];
            let favicon = chromium
                .as_ref()
                .and_then(|db| favicon::of_page(&dir, db, &page.url))
                .map(|source| icon_url(profile, &source));
            let title = if page.title.is_empty() {
                &page.url
            } else {
                &page.title
            };
            json!({ "offset": offset, "title": title, "favicon": favicon })
        })
        .collect();
    let detail = json!({ "forward": forward, "pages": pages });
    send_surface("sidebar", "arbalete:history", &detail.to_string());
}

/// How far each page a history menu lists is from the current one, among `count` pages: the
/// nearest first, at most `HISTORY_MENU_PAGES`.
fn history_offsets(count: usize, current: usize, forward: bool) -> Vec<i32> {
    let pages = if forward {
        count - current - 1
    } else {
        current
    };
    let sign = if forward { 1 } else { -1 };
    (1..=pages.min(HISTORY_MENU_PAGES) as i32)
        .map(|offset| sign * offset)
        .collect()
}

/// Goes `offset` pages back, if negative, or forward in the page's history, like the page's own
/// `history.go`.
pub(super) fn go_in_history(browser: &Browser, offset: i32) {
    if let Some(frame) = browser.main_frame() {
        let code = CefString::from(format!("history.go({offset})").as_str());
        frame.execute_java_script(Some(&code), None, 0);
    }
}

struct HistoryPage {
    url: String,
    title: String,
    current: bool,
}

wrap_navigation_entry_visitor! {
    struct HistoryVisitor {
        pages: Rc<RefCell<Vec<HistoryPage>>>,
    }

    impl NavigationEntryVisitor {
        fn visit(&self, entry: Option<&mut NavigationEntry>, current: i32, _index: i32, _total: i32) -> i32 {
            if let Some(entry) = entry {
                self.pages.borrow_mut().push(HistoryPage {
                    url: CefString::from(&entry.url()).to_string(),
                    title: CefString::from(&entry.title()).to_string(),
                    current: current == 1,
                });
            }
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_offsets_list_the_nearest_pages_first() {
        assert_eq!(history_offsets(4, 2, false), [-1, -2]);
        assert_eq!(history_offsets(4, 2, true), [1]);
        assert_eq!(history_offsets(1, 0, false), [] as [i32; 0]);
        assert_eq!(history_offsets(1, 0, true), [] as [i32; 0]);
        assert_eq!(history_offsets(30, 20, false).len(), HISTORY_MENU_PAGES);
        assert_eq!(history_offsets(30, 20, false).last(), Some(&-12));
    }

    /// A window, in points.
    const SIZE: Size = Size {
        width: 1000,
        height: 600,
    };

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
        win().library.set(true);
        assert_eq!(
            at(SidebarMode::Shown, 400.0, 300.0),
            Some((SidebarView::Docked, 400.0, 300.0))
        );
        win().library.set(false);
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
}
