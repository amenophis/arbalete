//! Imports other Chromium browsers' cookies, so that their sites stay signed in (decision 0012):
//! read with each browser's key on the import's thread, written through the cookie manager of
//! the profile each went into.

use super::*;
use arbalete_core::import::cookies::{self, Cookie as Imported};
use arbalete_core::import::safe_storage::{self, Key};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The cookies of each chosen browser profile, by its browser's and its own index, or why they
/// could not be read, as the user reads it.
pub(super) type Found = HashMap<(usize, usize), Result<Vec<Imported>, String>>;

/// Each chosen browser's key, by its index, or why it could not be had, for the profiles that
/// asked for their cookies or passwords: asked once per browser, as macOS shows its own dialog
/// for it. Slow: on the import's thread.
pub(super) type Keys = HashMap<usize, Result<Key, &'static str>>;

pub(super) fn read_keys(browsers: &[browsers::Browser], choices: &[Choice]) -> Keys {
    let mut keys = Keys::new();
    for choice in choices.iter().filter(|c| c.cookies || c.passwords) {
        let service = browsers
            .get(choice.browser)
            .and_then(|b| safe_storage::service(b.name));
        if let Some(service) = service {
            keys.entry(choice.browser)
                .or_insert_with(|| browser_key(service));
        }
    }
    keys
}

/// The browser profile, as the user reads it: its browser's name alone when it has one profile.
pub(super) fn label(browser: &browsers::Browser, profile: usize) -> String {
    match browser.profiles.as_slice() {
        [_] => browser.name.to_string(),
        profiles => format!("{} {}", browser.name, profiles[profile].name),
    }
}

/// Reads the cookies of the chosen profiles that asked for them, with their browser's key.
pub(super) fn read(browsers: &[browsers::Browser], choices: &[Choice], keys: &Keys) -> Found {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // Microseconds since 1601, as Chromium keeps them.
    let now = now.as_micros() as i64 + 11_644_473_600_000_000;
    let found = choices.iter().filter(|choice| choice.cookies);
    let found = found.filter_map(|choice| {
        let browser = browsers.get(choice.browser)?;
        let profile = browser.profiles.get(choice.profile)?;
        let key = keys.get(&choice.browser)?;
        let read = key.as_ref().map_err(|error| *error).and_then(|key| {
            cookies::read(&profile.path, key, now).map_err(|error| match error {
                cookies::Error::Key => "they could not be decrypted",
                cookies::Error::Io(_) | cookies::Error::Sqlite(_) => "they could not be read",
            })
        });
        let label = label(browser, choice.profile);
        let read = read.map_err(|why| format!("{label}’s cookies were not imported: {why}"));
        Some(((choice.browser, choice.profile), read))
    });
    found.collect()
}

fn browser_key(service: &str) -> Result<Key, &'static str> {
    #[cfg(target_os = "macos")]
    return match crate::mac::read_browser_key(service) {
        Ok(Some(password)) => Ok(Key::new(&password)),
        Ok(None) => Err("its key is not in the keychain"),
        Err(()) => Err("its key was not allowed"),
    };
    #[cfg(not(target_os = "macos"))]
    Err("not on this system")
}

/// Hands the cookies read to the profiles they went into, but those a profile already has, the
/// newer sign-in. Returns how many were read, and a line for each browser profile whose cookies
/// could not be.
pub(super) fn write(targets: &[(Choice, ProfileId)], mut found: Found) -> (usize, Vec<String>) {
    let (mut count, mut errors) = (0, Vec::new());
    for (choice, profile) in targets {
        match found.remove(&(choice.browser, choice.profile)) {
            None => {}
            Some(Err(error)) => errors.push(error),
            Some(Ok(cookies)) if cookies.is_empty() => {}
            Some(Ok(cookies)) => {
                let context = profile_context(*profile);
                let manager = context.as_ref().and_then(|c| c.cookie_manager(None));
                let (Some(context), Some(manager)) = (context, manager) else {
                    errors.push("Some cookies were not imported".to_string());
                    continue;
                };
                count += cookies.len();
                // Visited once its storage is ready; the cookies are set once every one it has
                // is seen, as the visitor goes.
                let pending = Arc::new(Pending {
                    context,
                    manager: manager.clone(),
                    cookies,
                    existing: Mutex::default(),
                });
                manager.visit_all_cookies(Some(&mut ExistingCookies::new(pending)));
            }
        }
    }
    (count, errors)
}

/// Cookies waiting for the profile's own to be listed.
struct Pending {
    /// Kept until the cookies are stored: for a profile no page has opened yet, it is the only
    /// one keeping the profile's web data open.
    context: RequestContext,
    manager: CookieManager,
    cookies: Vec<Imported>,
    /// The name, domain and path of each cookie the profile has.
    existing: Mutex<HashSet<(String, String, String)>>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        let existing = std::mem::take(self.existing.get_mut().unwrap_or_else(|e| e.into_inner()));
        let failed = Arc::new(Failed(AtomicUsize::new(0)));
        for cookie in &self.cookies {
            let id = (
                cookie.name.clone(),
                cookie.host.clone(),
                cookie.path.clone(),
            );
            if existing.contains(&id) {
                continue;
            }
            let (url, cookie) = cef_cookie(cookie);
            self.manager.set_cookie(
                Some(&CefString::from(url.as_str())),
                Some(&cookie),
                Some(&mut CookieSet::new(failed.clone())),
            );
        }
        let mut flushed = Flushed::new(self.context.clone());
        self.manager.flush_store(Some(&mut flushed));
    }
}

// Keeps the profile's web data open until the cookies are written.
wrap_completion_callback! {
    struct Flushed {
        _context: RequestContext,
    }

    impl CompletionCallback {}
}

/// How many cookies CEF turned down, as a site's could be: counted once all are set.
struct Failed(AtomicUsize);

impl Drop for Failed {
    fn drop(&mut self) {
        let failed = self.0.load(Ordering::Relaxed);
        if failed > 0 {
            eprintln!("cannot import {failed} cookies");
        }
    }
}

wrap_cookie_visitor! {
    struct ExistingCookies {
        pending: Arc<Pending>,
    }

    impl CookieVisitor {
        fn visit(
            &self,
            cookie: Option<&Cookie>,
            _count: i32,
            _total: i32,
            _delete: Option<&mut i32>,
        ) -> i32 {
            if let (Some(cookie), Ok(mut existing)) = (cookie, self.pending.existing.lock()) {
                let id = (cookie.name.to_string(), cookie.domain.to_string(), cookie.path.to_string());
                existing.insert(id);
            }
            1
        }
    }
}

wrap_set_cookie_callback! {
    struct CookieSet {
        failed: Arc<Failed>,
    }

    impl SetCookieCallback {
        fn on_complete(&self, success: i32) {
            if success == 0 {
                self.failed.0.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// The cookie as CEF sets it, and the url it is set for: a host-only cookie has no domain, which
/// CEF takes from the url.
fn cef_cookie(cookie: &Imported) -> (String, Cookie) {
    let scheme = if cookie.secure { "https" } else { "http" };
    let host = cookie.host.trim_start_matches('.');
    let url = format!("{scheme}://{host}{}", cookie.path);
    let domain = if cookie.host.starts_with('.') {
        cookie.host.as_str()
    } else {
        ""
    };
    let cef = Cookie {
        name: CefString::from(cookie.name.as_str()),
        value: CefString::from(cookie.value.as_str()),
        domain: CefString::from(domain),
        path: CefString::from(cookie.path.as_str()),
        secure: cookie.secure.into(),
        httponly: cookie.http_only.into(),
        creation: Basetime {
            val: cookie.creation,
        },
        last_access: Basetime {
            val: cookie.last_access,
        },
        has_expires: cookie.expires.is_some().into(),
        expires: Basetime {
            val: cookie.expires.unwrap_or_default(),
        },
        same_site: match cookie.same_site {
            0 => CookieSameSite::NO_RESTRICTION,
            1 => CookieSameSite::LAX_MODE,
            2 => CookieSameSite::STRICT_MODE,
            _ => CookieSameSite::UNSPECIFIED,
        },
        priority: match cookie.priority {
            0 => CookiePriority::LOW,
            2 => CookiePriority::HIGH,
            _ => CookiePriority::MEDIUM,
        },
        ..Default::default()
    };
    (url, cef)
}

/// Tells how the cookies and passwords went once the import assistant closes: the first browser
/// whose cookies or passwords were not imported, or how many were.
pub(super) fn tell(cookies: usize, passwords: usize, errors: &[String]) {
    let text = match (errors.first(), cookies, passwords) {
        (Some(error), _, _) => error.clone(),
        (None, 0, 0) => return,
        (None, cookies, 0) => format!("{cookies} cookies imported: you stay signed in"),
        (None, 0, passwords) => format!("{passwords} passwords imported"),
        (None, cookies, passwords) => {
            format!("{cookies} cookies and {passwords} passwords imported: you stay signed in")
        }
    };
    toast(json!({ "text": text, "icon": "key" }));
}
