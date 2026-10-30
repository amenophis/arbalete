//! The native macOS interface (app/macos, in Swift). It gets the events the web UI gets, as their
//! name and JSON detail, and sends the same commands (app/src/command.rs).

use crate::command::Command;
use cef::Rect;
use std::ffi::{CStr, CString, c_char, c_void};

unsafe extern "C" {
    fn arbalete_ui_attach(handle: *mut c_void, send: extern "C" fn(*const c_char));
    fn arbalete_ui_event(surface: *const c_char, name: *const c_char, detail: *const c_char);
    fn arbalete_ui_place_sidebar(
        visible: bool,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        floating: bool,
        revealed: bool,
    );
}

/// Puts the native interface in the window `handle` is, or is in.
pub fn attach(handle: *mut c_void) {
    // SAFETY: CEF gives a live NSWindow or NSView, and `send` lives as long as the app.
    unsafe { arbalete_ui_attach(handle, send) };
}

/// Gives a surface of the native interface, like "sidebar", an event dispatched to its web
/// counterpart: `new CustomEvent("name", { detail: … })` or `new Event("name")`.
pub fn event(surface: &str, event: &str) {
    let Some((name, detail)) = parse_event(event) else {
        return;
    };
    let (Ok(surface), Ok(name), Ok(detail)) = (
        CString::new(surface),
        CString::new(name),
        CString::new(detail),
    ) else {
        return;
    };
    // SAFETY: the strings outlive the call, which copies what it needs.
    unsafe { arbalete_ui_event(surface.as_ptr(), name.as_ptr(), detail.as_ptr()) };
}

/// An event's name and its detail, `null` without one.
fn parse_event(event: &str) -> Option<(&str, &str)> {
    if let Some(rest) = event.strip_prefix(r#"new Event(""#) {
        return Some((rest.split_once('"')?.0, "null"));
    }
    let (name, rest) = event
        .strip_prefix(r#"new CustomEvent(""#)?
        .split_once('"')?;
    let detail = rest.strip_prefix(", { detail: ")?.strip_suffix(" })")?;
    Some((name, detail))
}

/// Shows the sidebar at `frame`, in the window's points from its top left corner, floating above
/// the page or docked; hides it without a frame. Revealed, it hides again once the pointer
/// leaves it.
pub fn place(frame: Option<&Rect>, floating: bool, revealed: bool) {
    let frame = frame.cloned().unwrap_or_default();
    // SAFETY: plain values.
    unsafe {
        arbalete_ui_place_sidebar(
            frame.width > 0,
            frame.x.into(),
            frame.y.into(),
            frame.width.into(),
            frame.height.into(),
            floating,
            revealed,
        );
    }
}

/// Runs a command the interface sends, on the main thread, which is CEF's UI thread on macOS.
extern "C" fn send(path: *const c_char) {
    // SAFETY: the interface passes a string that lives during the call.
    let path = unsafe { CStr::from_ptr(path) };
    if let Some(command) = path.to_str().ok().and_then(Command::parse) {
        crate::shell::run(command);
    }
}

/// The file behind a url of the app's UI, like a tab's cached icon, for the interface to load;
/// null for none. The caller frees it with arbalete_free_string.
#[unsafe(no_mangle)]
pub extern "C" fn arbalete_ui_file(url: *const c_char) -> *mut c_char {
    // SAFETY: the interface passes a string that lives during the call.
    let url = unsafe { CStr::from_ptr(url) };
    url.to_str()
        .ok()
        .and_then(crate::shell::ui_file_path)
        .and_then(|path| CString::new(path.to_str()?).ok())
        .map_or(std::ptr::null_mut(), CString::into_raw)
}

/// Frees a string this side gave the interface.
#[unsafe(no_mangle)]
pub extern "C" fn arbalete_free_string(string: *mut c_char) {
    if !string.is_null() {
        // SAFETY: made by CString::into_raw in arbalete_ui_file.
        drop(unsafe { CString::from_raw(string) });
    }
}

#[cfg(test)]
mod tests {
    use super::parse_event;

    #[test]
    fn parses_the_events_dispatched_to_the_web_ui() {
        assert_eq!(
            parse_event(r#"new CustomEvent("arbalete:state", { detail: {"space":1} })"#),
            Some(("arbalete:state", r#"{"space":1}"#))
        );
        assert_eq!(
            parse_event(r#"new Event("arbalete:show-history")"#),
            Some(("arbalete:show-history", "null"))
        );
        assert_eq!(parse_event("dispatchEvent()"), None);
    }
}
