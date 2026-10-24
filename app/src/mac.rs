//! The NSApplication subclass CEF requires on macOS, and the menus.

use crate::command::Command;
use crate::swipe::{Phase, Scroll};
use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::{
    ClassType, DefinedClass, Encoding, MainThreadMarker, MainThreadOnly, RefEncode, define_class,
    extern_methods, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Bool, NSObjectProtocol, Sel},
    sel,
};
use objc2_app_kit::{
    NSApp, NSApplication, NSColor, NSDownArrowFunctionKey, NSEvent, NSEventModifierFlags,
    NSEventPhase, NSEventType, NSLeftArrowFunctionKey, NSMenu, NSMenuItem, NSRightArrowFunctionKey,
    NSUpArrowFunctionKey, NSView, NSWindowButton, NSWorkspace,
};
use objc2_foundation::{NSArray, NSNumber, NSString, ns_string};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;

/// The page's corner radius, in points.
const CARD_RADIUS: f64 = 8.0;

/// Core Graphics' color, which only passes through here: no need for its bindings.
#[repr(C)]
struct CGColor {
    _private: [u8; 0],
}

// SAFETY: the encoding Objective-C gives a CGColorRef.
unsafe impl RefEncode for CGColor {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("CGColor", &[]));
}

/// The width of the card's edge, in points: one pixel on a Retina display.
const CARD_EDGE: f64 = 0.5;

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
            // Seen on their way to the page, which still scrolls with them.
            if event.r#type() == NSEventType::ScrollWheel {
                crate::shell::scroll(scroll(event));
            }
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
            crate::shell::run(Command::Reopen(0));
        }

        #[unsafe(method(arbaletteTogglePin:))]
        fn toggle_pin(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::Pin(None));
        }

        #[unsafe(method(arbaletteNewFolder:))]
        fn new_folder(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::NewFolder);
        }

        #[unsafe(method(arbaletteImport:))]
        fn import(&self, _sender: Option<&AnyObject>) {
            crate::shell::run(Command::ShowImport);
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

/// Only scroll events have phases and deltas.
fn scroll(event: &NSEvent) -> Scroll {
    let phase = event.phase();
    let phase = if phase.is_empty() {
        Phase::None
    } else if phase.contains(NSEventPhase::Began) {
        Phase::Began
    } else if phase.contains(NSEventPhase::Changed) {
        Phase::Changed
    } else if phase.intersects(NSEventPhase::Ended | NSEventPhase::Cancelled) {
        Phase::Ended
    } else {
        Phase::Other
    };
    let (x, y) = main_window_point(event);
    Scroll {
        phase,
        momentum: !event.momentumPhase().is_empty(),
        dx: event.scrollingDeltaX(),
        dy: event.scrollingDeltaY(),
        y,
        time: event.timestamp(),
        inside: crate::shell::is_over_sidebar(x),
    }
}

/// Where an event happened in the main window, from its top left corner: the views above the
/// page, like the card, are windows of their own.
fn main_window_point(event: &NSEvent) -> (f64, f64) {
    let point = event.locationInWindow();
    let mtm = MainThreadMarker::new().expect("main thread");
    let Some(window) = event.window(mtm) else {
        return (point.x, point.y);
    };
    let main = window.parentWindow().unwrap_or_else(|| window.clone());
    let (frame, main) = (window.frame(), main.frame());
    // The system counts from the bottom of the screen.
    let top = main.origin.y + main.size.height;
    (
        frame.origin.x + point.x - main.origin.x,
        top - (frame.origin.y + point.y),
    )
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
                // No shortcut: imports are rare.
                item(
                    mtm,
                    ns_string!("Import…"),
                    sel!(arbaletteImport:),
                    ns_string!(""),
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

/// Rounds the corners of the card's window, which draws the page, and outlines it. `view` is
/// its content view, whose layer holds what the window's compositor draws.
pub fn round_card(view: *mut c_void) {
    // SAFETY: CEF gives the window's NSView, which the window keeps alive.
    let Some(view) = (unsafe { view.cast::<NSView>().as_ref() }) else {
        return;
    };
    // The corners show the window below.
    if let Some(window) = view.window() {
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
    }
    view.setWantsLayer(true);
    let layer: Option<Retained<AnyObject>> = unsafe { msg_send![view, layer] };
    if let Some(layer) = layer {
        let _: () = unsafe { msg_send![&*layer, setCornerRadius: CARD_RADIUS] };
        let _: () = unsafe { msg_send![&*layer, setMasksToBounds: Bool::YES] };
        // A darker edge, drawn above the page.
        let edge = NSColor::colorWithWhite_alpha(0.0, 0.25);
        let edge: *const CGColor = unsafe { msg_send![&*edge, CGColor] };
        let _: () = unsafe { msg_send![&*layer, setBorderColor: edge] };
        let _: () = unsafe { msg_send![&*layer, setBorderWidth: CARD_EDGE] };
    }
}

/// The floating sidebar's corner radius, in points.
const FLOAT_RADIUS: f64 = 10.0;

/// The floating sidebar's edge: one point, light.
const FLOAT_EDGE: f64 = 1.0;
const FLOAT_EDGE_ALPHA: f64 = 0.2;

/// Gives the revealed sidebar's window the floating look, rounded and outlined, with a shadow, by
/// `amount` from 0, a plain view, to 1. `view` is its content view, like the card's.
pub fn float(view: *mut c_void, amount: f64) {
    // SAFETY: CEF gives the window's NSView, which the window keeps alive.
    let Some(view) = (unsafe { view.cast::<NSView>().as_ref() }) else {
        return;
    };
    if let Some(window) = view.window() {
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        // The system's shadow follows the rounded corners once they are drawn.
        window.setHasShadow(amount >= 1.0);
        window.invalidateShadow();
    }
    view.setWantsLayer(true);
    let layer: Option<Retained<AnyObject>> = unsafe { msg_send![view, layer] };
    let Some(layer) = layer else { return };
    let edge = NSColor::colorWithWhite_alpha(1.0, FLOAT_EDGE_ALPHA * amount);
    let edge: *const CGColor = unsafe { msg_send![&*edge, CGColor] };
    unsafe {
        let _: () = msg_send![&*layer, setCornerRadius: FLOAT_RADIUS * amount];
        let _: () = msg_send![&*layer, setMasksToBounds: Bool::YES];
        let _: () = msg_send![&*layer, setBorderColor: edge];
        let _: () = msg_send![&*layer, setBorderWidth: FLOAT_EDGE * amount];
    }
}

/// Slides a window's content horizontally through `offsets`, in points from where it rests, spread
/// evenly over `duration` seconds, and keeps it at the last one. The compositor runs it, frame by
/// frame, clipped by the window: the window itself stays in place. No offsets stop the slide.
pub fn slide(view: *mut c_void, offsets: &[f64], duration: f64) {
    let key = ns_string!("slide");
    // SAFETY: CEF gives the window's NSView, which the window keeps alive.
    let Some(view) = (unsafe { view.cast::<NSView>().as_ref() }) else {
        return;
    };
    view.setWantsLayer(true);
    let layer: Option<Retained<AnyObject>> = unsafe { msg_send![view, layer] };
    let Some(layer) = layer else { return };
    let _: () = unsafe { msg_send![&*layer, removeAnimationForKey: key] };
    let Some(class) = AnyClass::get(c"CAKeyframeAnimation") else {
        return;
    };
    if offsets.is_empty() {
        return;
    }
    let values: Vec<_> = offsets
        .iter()
        .map(|&offset| NSNumber::new_f64(offset))
        .collect();
    let values = NSArray::from_retained_slice(&values);
    unsafe {
        let animation: Retained<AnyObject> =
            msg_send![class, animationWithKeyPath: ns_string!("transform.translation.x")];
        let _: () = msg_send![&*animation, setValues: &*values];
        let _: () = msg_send![&*animation, setDuration: duration];
        let _: () = msg_send![&*animation, setFillMode: ns_string!("forwards")];
        let _: () = msg_send![&*animation, setRemovedOnCompletion: Bool::NO];
        let _: () = msg_send![&*layer, addAnimation: &*animation, forKey: key];
    }
}

/// Moves a window's content `x` points right of where it rests, at once, clipped by the window:
/// the window itself stays in place. `view` is any view in the window, like the card's page.
pub fn shift(view: *mut c_void, x: f64) {
    // SAFETY: CEF gives the window's NSView, which the window keeps alive.
    let Some(view) = (unsafe { view.cast::<NSView>().as_ref() }) else {
        return;
    };
    let Some(content) = view.window().and_then(|window| window.contentView()) else {
        return;
    };
    content.setWantsLayer(true);
    let layer: Option<Retained<AnyObject>> = unsafe { msg_send![&*content, layer] };
    let (Some(layer), Some(transaction)) = (layer, AnyClass::get(c"CATransaction")) else {
        return;
    };
    let x = NSNumber::new_f64(x);
    unsafe {
        let _: () = msg_send![transaction, begin];
        let _: () = msg_send![transaction, setDisableActions: Bool::YES];
        let _: () = msg_send![
            &*layer,
            setValue: &*x,
            forKeyPath: ns_string!("transform.translation.x")
        ];
        let _: () = msg_send![transaction, commit];
    }
}

/// The user asked the system for less motion.
pub fn reduce_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}
