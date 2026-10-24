//! The NSApplication subclass CEF requires on macOS, and the menus.

use crate::command::Command;
use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::{
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, extern_methods,
    msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObjectProtocol, Sel},
    sel,
};
use objc2_app_kit::{
    NSApp, NSApplication, NSDownArrowFunctionKey, NSEvent, NSEventModifierFlags,
    NSLeftArrowFunctionKey, NSMenu, NSMenuItem, NSRightArrowFunctionKey, NSUpArrowFunctionKey,
    NSView, NSWindowButton,
};
use objc2_foundation::{NSString, ns_string};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;

thread_local! {
    /// Its title says what it does: hide or show the sidebar.
    static SIDEBAR_ITEM: RefCell<Option<Retained<NSMenuItem>>> = const { RefCell::new(None) };
}

#[derive(Default)]
pub struct AppIvars {
    handling_send_event: Cell<Bool>,
}

define_class!(
    #[unsafe(super(NSApplication))]
    #[ivars = AppIvars]
    pub struct ArbaletteApplication;

    impl ArbaletteApplication {
        #[unsafe(method(sendEvent:))]
        unsafe fn send_event(&self, event: &NSEvent) {
            let was_sending = self.ivars().handling_send_event.get();
            self.ivars().handling_send_event.set(Bool::YES);
            let _: () = msg_send![super(self), sendEvent: event];
            self.ivars().handling_send_event.set(was_sending);
        }

        // Chromium must leave the run loop to shut down: quitting closes the window first.
        #[unsafe(method(terminate:))]
        unsafe fn terminate(&self, _sender: Option<&AnyObject>) {
            crate::shell::close();
        }

        // Menu actions reach the application at the end of the responder chain.
        #[unsafe(method(arbaletteNewTab:))]
        fn new_tab(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::ShowOverlay);
        }

        #[unsafe(method(arbaletteCloseTab:))]
        fn close_tab(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::Close(None));
        }

        #[unsafe(method(arbaletteReopenTab:))]
        fn reopen_tab(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::Reopen);
        }

        #[unsafe(method(arbaletteTogglePin:))]
        fn toggle_pin(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::Pin(None));
        }

        #[unsafe(method(arbaletteNewFolder:))]
        fn new_folder(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::NewFolder);
        }

        #[unsafe(method(arbaletteNextTab:))]
        fn next_tab(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::NextTab);
        }

        #[unsafe(method(arbalettePreviousTab:))]
        fn previous_tab(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::PreviousTab);
        }

        // The item's tag is the favorite's number.
        #[unsafe(method(arbaletteFavorite:))]
        fn favorite(&self, sender: Option<&NSMenuItem>) {
            if let Some(item) = sender {
                crate::shell::run(Command::Favorite(item.tag() as usize));
            }
        }

        #[unsafe(method(arbaletteNextSpace:))]
        fn next_space(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::NextSpace);
        }

        #[unsafe(method(arbalettePreviousSpace:))]
        fn previous_space(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::PreviousSpace);
        }

        // The item's tag is the Space's number.
        #[unsafe(method(arbaletteSpace:))]
        fn space(&self, sender: Option<&NSMenuItem>) {
            if let Some(item) = sender {
                crate::shell::run(Command::SpaceNumber(item.tag() as usize));
            }
        }

        #[unsafe(method(arbaletteBack:))]
        fn back(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::Back);
        }

        #[unsafe(method(arbaletteForward:))]
        fn forward(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::Forward);
        }

        #[unsafe(method(arbaletteToggleSidebar:))]
        fn toggle_sidebar(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::ToggleSidebar);
        }

        #[unsafe(method(arbaletteReload:))]
        fn reload(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::Reload);
        }
    }

    unsafe impl CrAppControlProtocol for ArbaletteApplication {
        #[unsafe(method(setHandlingSendEvent:))]
        unsafe fn set_handling_send_event(&self, value: Bool) {
            self.ivars().handling_send_event.set(value);
        }
    }

    unsafe impl CrAppProtocol for ArbaletteApplication {
        #[unsafe(method(isHandlingSendEvent))]
        unsafe fn is_handling_send_event(&self) -> Bool {
            self.ivars().handling_send_event.get()
        }
    }

    unsafe impl CefAppProtocol for ArbaletteApplication {}
);

impl ArbaletteApplication {
    extern_methods! {
        #[unsafe(method(sharedApplication))]
        fn shared_application() -> Retained<Self>;
    }
}

pub fn setup_application() {
    let _ = ArbaletteApplication::shared_application();
    let mtm = MainThreadMarker::new().expect("main thread");
    assert!(NSApp(mtm).isKindOfClass(ArbaletteApplication::class()));
}

/// The menus, built in code. Their key equivalents are the app's keyboard shortcuts.
pub fn setup_menu() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let menu_bar = NSMenu::new(mtm);
    for (title, items) in [
        (
            ns_string!("Arbalette"),
            vec![item(
                mtm,
                ns_string!("Quit Arbalette"),
                sel!(terminate:),
                ns_string!("q"),
            )],
        ),
        (
            ns_string!("File"),
            vec![
                item(
                    mtm,
                    ns_string!("New Tab"),
                    sel!(arbaletteNewTab:),
                    ns_string!("t"),
                ),
                item(
                    mtm,
                    ns_string!("Close Tab"),
                    sel!(arbaletteCloseTab:),
                    ns_string!("w"),
                ),
                // An uppercase key equivalent adds ⇧.
                item(
                    mtm,
                    ns_string!("Reopen Closed Tab"),
                    sel!(arbaletteReopenTab:),
                    ns_string!("T"),
                ),
                item(
                    mtm,
                    ns_string!("Pin or Unpin Tab"),
                    sel!(arbaletteTogglePin:),
                    ns_string!("d"),
                ),
                item(
                    mtm,
                    ns_string!("New Folder"),
                    sel!(arbaletteNewFolder:),
                    ns_string!("N"),
                ),
            ],
        ),
        (ns_string!("Tabs"), tabs_menu(mtm)),
        (ns_string!("Spaces"), spaces_menu(mtm)),
        (
            ns_string!("View"),
            vec![
                sidebar_item(mtm),
                item(
                    mtm,
                    ns_string!("Reload Page"),
                    sel!(arbaletteReload:),
                    ns_string!("r"),
                ),
            ],
        ),
        (
            ns_string!("History"),
            vec![
                item(
                    mtm,
                    ns_string!("Back"),
                    sel!(arbaletteBack:),
                    ns_string!("["),
                ),
                item(
                    mtm,
                    ns_string!("Forward"),
                    sel!(arbaletteForward:),
                    ns_string!("]"),
                ),
            ],
        ),
    ] {
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), title);
        for item in items {
            menu.addItem(&item);
        }
        let menu_item = NSMenuItem::new(mtm);
        menu_item.setSubmenu(Some(&menu));
        menu_bar.addItem(&menu_item);
    }
    NSApp(mtm).setMainMenu(Some(&menu_bar));
}

fn sidebar_item(mtm: MainThreadMarker) -> Retained<NSMenuItem> {
    let item = item(
        mtm,
        ns_string!("Hide Sidebar"),
        sel!(arbaletteToggleSidebar:),
        ns_string!("s"),
    );
    SIDEBAR_ITEM.set(Some(item.clone()));
    item
}

/// Updates the menu once the sidebar is shown or hidden, and hides the window buttons with it,
/// as they sit in its toolbar. `view` is the window's content view.
pub fn sidebar_shown(view: *mut c_void, shown: bool) {
    if let Some(item) = SIDEBAR_ITEM.with_borrow(Clone::clone) {
        item.setTitle(if shown {
            ns_string!("Hide Sidebar")
        } else {
            ns_string!("Show Sidebar")
        });
    }
    // SAFETY: CEF gives the window's NSView, which the window keeps alive.
    let Some(window) = unsafe { view.cast::<NSView>().as_ref() }.and_then(NSView::window) else {
        return;
    };
    for button in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(button) {
            button.setHidden(!shown);
        }
    }
}

/// Next and previous tab with ⌘⌥↓ and ⌘⌥↑, then the favorites with ⌘1 to ⌘9.
fn tabs_menu(mtm: MainThreadMarker) -> Vec<Retained<NSMenuItem>> {
    let mut items = vec![
        arrow_item(
            mtm,
            ns_string!("Next Tab"),
            sel!(arbaletteNextTab:),
            NSDownArrowFunctionKey,
        ),
        arrow_item(
            mtm,
            ns_string!("Previous Tab"),
            sel!(arbalettePreviousTab:),
            NSUpArrowFunctionKey,
        ),
    ];
    for number in 1..=9 {
        let title = if number == 9 {
            "Last Favorite".to_string()
        } else {
            format!("Favorite {number}")
        };
        let item = item(
            mtm,
            &NSString::from_str(&title),
            sel!(arbaletteFavorite:),
            &NSString::from_str(&number.to_string()),
        );
        item.setTag(number);
        items.push(item);
    }
    items
}

/// Next and previous Space with ⌘⌥→ and ⌘⌥←, as Spaces sit side by side, then the Spaces by
/// number with ⌃1 to ⌃9.
fn spaces_menu(mtm: MainThreadMarker) -> Vec<Retained<NSMenuItem>> {
    let mut items = vec![
        arrow_item(
            mtm,
            ns_string!("Next Space"),
            sel!(arbaletteNextSpace:),
            NSRightArrowFunctionKey,
        ),
        arrow_item(
            mtm,
            ns_string!("Previous Space"),
            sel!(arbalettePreviousSpace:),
            NSLeftArrowFunctionKey,
        ),
    ];
    for number in 1..=9 {
        let item = item(
            mtm,
            &NSString::from_str(&format!("Space {number}")),
            sel!(arbaletteSpace:),
            &NSString::from_str(&number.to_string()),
        );
        item.setKeyEquivalentModifierMask(NSEventModifierFlags::Control);
        item.setTag(number);
        items.push(item);
    }
    items
}

/// A menu item sending `action` with ⌘⌥ and an arrow `key`.
fn arrow_item(
    mtm: MainThreadMarker,
    title: &NSString,
    action: Sel,
    key: u32,
) -> Retained<NSMenuItem> {
    let key = char::from_u32(key).expect("a function key").to_string();
    let item = item(mtm, title, action, &NSString::from_str(&key));
    item.setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Option);
    item
}

/// A menu item sending `action` with ⌘ and `key`.
fn item(
    mtm: MainThreadMarker,
    title: &NSString,
    action: Sel,
    key: &NSString,
) -> Retained<NSMenuItem> {
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            title,
            Some(action),
            key,
        )
    }
}
