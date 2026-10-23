//! The NSApplication subclass CEF requires on macOS, and the app menu.

use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::{
    ClassType, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, extern_methods,
    msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObjectProtocol},
    sel,
};
use objc2_app_kit::{NSApp, NSApplication, NSEvent, NSMenu, NSMenuItem};
use objc2_foundation::ns_string;
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

/// The app menu, built in code: ⌘Q quits.
pub fn setup_menu() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let quit = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            ns_string!("Quit Arbalette"),
            Some(sel!(terminate:)),
            ns_string!("q"),
        )
    };
    let app_menu = NSMenu::new(mtm);
    app_menu.addItem(&quit);
    let app_item = NSMenuItem::new(mtm);
    app_item.setSubmenu(Some(&app_menu));
    let menu_bar = NSMenu::new(mtm);
    menu_bar.addItem(&app_item);
    NSApp(mtm).setMainMenu(Some(&menu_bar));
}
