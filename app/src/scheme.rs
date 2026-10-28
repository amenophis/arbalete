//! The scheme Arbalete's own UI is served from (`arbalete://ui/`). Extensions' content scripts
//! only match http, https and file urls, so they never run in the UI. Every process registers it:
//! the app and its helper (app/src/bin/helper.rs).

use cef::{sys::cef_scheme_options_t::*, *};

pub const UI_SCHEME: &str = "arbalete";

/// Registers the UI's scheme as a secure standard one, like the https it replaces, which the UI
/// can fetch its commands from.
pub fn register(registrar: Option<&mut SchemeRegistrar>) {
    let Some(registrar) = registrar else { return };
    let options = CEF_SCHEME_OPTION_STANDARD as i32
        | CEF_SCHEME_OPTION_SECURE as i32
        | CEF_SCHEME_OPTION_CORS_ENABLED as i32
        | CEF_SCHEME_OPTION_FETCH_ENABLED as i32;
    registrar.add_custom_scheme(Some(&CefString::from(UI_SCHEME)), options);
}
