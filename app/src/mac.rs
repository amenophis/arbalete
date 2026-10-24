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
use objc2_app_kit::{NSApp, NSApplication, NSEvent, NSMenu, NSMenuItem};
use objc2_foundation::{NSString, ns_string};
use std::cell::Cell;

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

        #[unsafe(method(arbaletteBack:))]
        fn back(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::Back);
        }

        #[unsafe(method(arbaletteForward:))]
        fn forward(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::Forward);
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
            ],
        ),
        (
            ns_string!("View"),
            vec![item(
                mtm,
                ns_string!("Reload Page"),
                sel!(arbaletteReload:),
                ns_string!("r"),
            )],
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
