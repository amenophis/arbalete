//! The native macOS interface (app/macos, in Swift): the sidebar, attached to the main window,
//! gets the state the web UI gets and sends the same commands.

use crate::command::Command;
use std::ffi::{CStr, CString, c_char, c_void};

unsafe extern "C" {
    fn arbalete_sidebar_attach(handle: *mut c_void, width: f64, send: extern "C" fn(*const c_char));
    fn arbalete_sidebar_state(json: *const c_char);
}

/// Puts the sidebar, `width` points wide, in the window `handle` is, or is in.
pub fn attach(handle: *mut c_void, width: i32) {
    // SAFETY: CEF gives a live NSWindow or NSView, and `send` lives as long as the app.
    unsafe { arbalete_sidebar_attach(handle, f64::from(width), send) };
}

/// Gives the sidebar the app's state.
pub fn state(json: &str) {
    let Ok(json) = CString::new(json) else { return };
    // SAFETY: the string outlives the call, which copies what it needs.
    unsafe { arbalete_sidebar_state(json.as_ptr()) };
}

/// Runs a command the sidebar sends, on the main thread, which is CEF's UI thread on macOS.
extern "C" fn send(path: *const c_char) {
    // SAFETY: the sidebar passes a string that lives during the call.
    let path = unsafe { CStr::from_ptr(path) };
    if let Some(command) = path.to_str().ok().and_then(Command::parse) {
        crate::shell::run(command);
    }
}
