//! Imports other Chromium browsers' saved passwords into Arbalete's password manager (decision
//! 0015): read with each browser's key on the import's thread, added by the password manager's
//! own page, without a window, in the profile each went into.

use super::import_cookies::{Keys, label};
use super::*;
use arbalete_core::import::cookies::Error;
use arbalete_core::import::passwords::{self, Password};

/// The passwords of each chosen browser profile, by its browser's and its own index, or why they
/// could not be read, as the user reads it.
pub(super) type Found = HashMap<(usize, usize), Result<Vec<Password>, String>>;

/// The page that adds them: Chromium's password manager, whose `passwordsPrivate` API stores a
/// password as its "Add password" does.
const PAGE: &str = "chrome://password-manager/passwords";

/// The page's title once every password is added, followed by how many were.
const DONE: &str = "arbalete-passwords-added:";

/// Reads the passwords of the chosen profiles that asked for them, with their browser's key.
pub(super) fn read(browsers: &[browsers::Browser], choices: &[Choice], keys: &Keys) -> Found {
    let found = choices.iter().filter(|choice| choice.passwords);
    let found = found.filter_map(|choice| {
        let browser = browsers.get(choice.browser)?;
        let profile = browser.profiles.get(choice.profile)?;
        let key = keys.get(&choice.browser)?;
        let read = key.as_ref().map_err(|error| *error).and_then(|key| {
            passwords::read(&profile.path, key).map_err(|error| match error {
                Error::Key => "they could not be decrypted",
                Error::Io(_) | Error::Sqlite(_) => "they could not be read",
            })
        });
        let label = label(browser, choice.profile);
        let read = read.map_err(|why| format!("{label}’s passwords were not imported: {why}"));
        Some(((choice.browser, choice.profile), read))
    });
    found.collect()
}

/// Adds the passwords read to the profiles they went into, but those a profile already has for
/// the same site and user name, the newer ones. Returns how many were read, and a line for each
/// browser profile whose passwords could not be.
pub(super) fn write(targets: &[(Choice, ProfileId)], mut found: Found) -> (usize, Vec<String>) {
    let (mut count, mut errors) = (0, Vec::new());
    for (choice, profile) in targets {
        match found.remove(&(choice.browser, choice.profile)) {
            None => {}
            Some(Err(error)) => errors.push(error),
            Some(Ok(passwords)) if passwords.is_empty() => {}
            Some(Ok(passwords)) => {
                count += passwords.len();
                if !add(*profile, &passwords) {
                    errors.push("Some passwords were not imported".to_string());
                }
            }
        }
    }
    (count, errors)
}

/// Opens the profile's password manager without a window, to add the passwords once it loads:
/// once the profile's web data is ready, which CEF waits for.
fn add(profile: ProfileId, passwords: &[Password]) -> bool {
    let info = WindowInfo {
        windowless_rendering_enabled: 1,
        ..Default::default()
    };
    let script = Arc::new(Mutex::new(script(passwords)));
    browser_host_create_browser(
        Some(&info),
        Some(&mut PasswordsClient::new(script)),
        Some(&CefString::from(PAGE)),
        Some(&BrowserSettings::default()),
        None,
        profile_context(profile).as_mut(),
    ) == 1
}

/// Adds the passwords one after the other, then tells how many were in its title: the password
/// manager turns down one it has for the same site and user name.
fn script(passwords: &[Password]) -> String {
    let list: Vec<_> = passwords
        .iter()
        .map(|p| json!({ "url": p.url, "username": p.username, "password": p.password }))
        .collect();
    let list = serde_json::Value::from(list);
    format!(
        "(async () => {{
          let added = 0;
          try {{
            for (const p of {list}) {{
              const password = {{ ...p, note: '', useAccountStore: false }};
              await chrome.passwordsPrivate.addPassword(password).then(() => added++, () => {{}});
            }}
          }} finally {{
            document.title = '{DONE}' + added;
          }}
        }})()"
    )
}

wrap_client! {
    struct PasswordsClient {
        // Taken once run.
        script: Arc<Mutex<String>>,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(PasswordsRender::new())
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(PasswordsLoad::new(self.script.clone()))
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(PasswordsDisplay::new())
        }
    }
}

wrap_render_handler! {
    struct PasswordsRender {}

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                *rect = Rect { x: 0, y: 0, width: 1, height: 1 };
            }
        }
    }
}

// The password manager has loaded: it adds the passwords, once.
wrap_load_handler! {
    struct PasswordsLoad {
        script: Arc<Mutex<String>>,
    }

    impl LoadHandler {
        fn on_load_end(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, _http_status_code: i32) {
            let Some(frame) = frame else { return };
            let page = CefString::from(&frame.url()).to_string();
            if frame.is_main() != 1 || !page.starts_with(PAGE) {
                return;
            }
            let script = self.script.lock().map(|mut script| std::mem::take(&mut *script));
            if let Some(script) = script.ok().filter(|script| !script.is_empty()) {
                frame.execute_java_script(Some(&CefString::from(script.as_str())), None, 0);
            }
        }
    }
}

// The passwords are added: the page closes.
wrap_display_handler! {
    struct PasswordsDisplay {}

    impl DisplayHandler {
        fn on_title_change(&self, browser: Option<&mut Browser>, title: Option<&CefString>) {
            let title = title.map(CefString::to_string).unwrap_or_default();
            let (Some(browser), Some(added)) = (browser, title.strip_prefix(DONE)) else { return };
            eprintln!("{added} passwords imported");
            // Not while CEF tells of the title.
            post_task(ThreadId::UI, Some(&mut ClosePasswords::new(browser.clone())));
        }
    }
}

wrap_task! {
    struct ClosePasswords {
        browser: Browser,
    }

    impl Task {
        fn execute(&self) {
            if let Some(host) = self.browser.host() {
                host.close_browser(1);
            }
        }
    }
}
