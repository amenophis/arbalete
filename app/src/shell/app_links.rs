//! Links to another app, like `zoommtg://` or the `claude://` a sign-in ends on: handed to macOS,
//! once the user agrees in the permission prompt, like Chrome's "Open Zoom?". Chromium loads none
//! of them in CEF Alloy, which shows ERR_UNKNOWN_URL_SCHEME instead.

use super::*;

/// The schemes Chromium or Arbalete load themselves, and those Chrome never hands to another app.
const BROWSER_SCHEMES: &[&str] = &[
    "about",
    "arb",
    "blob",
    "chrome",
    "chrome-error",
    "chrome-extension",
    "chrome-search",
    "chrome-untrusted",
    "data",
    "devtools",
    "file",
    "filesystem",
    "ftp",
    "http",
    "https",
    "javascript",
    "view-source",
    "ws",
    "wss",
    // Chrome's own list of schemes it blocks (ExternalProtocolHandler).
    "afp",
    "disk",
    "disks",
    "hcp",
    "ie.http",
    "ms-help",
    "nntp",
    "res",
    "shell",
    "vbscript",
    "vnd.ms.radio",
];

/// Whether `url` leads to another app: its scheme is none the browser loads.
pub(super) fn is_app_link(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    let valid = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid && !BROWSER_SCHEMES.contains(&scheme.as_str())
}

/// A page going to another app's link stays where it was, rather than showing an error: the link
/// goes to macOS, once the user agrees in the permission prompt, if an app opens it. A page that
/// only opened for the link, like a tab a sign-in opened, closes. Whether `url` was such a link.
pub(super) fn leave_for_app(browser: &Browser, frame: &Frame, request: &Request) -> bool {
    let url = CefString::from(&request.url()).to_string();
    if !is_app_link(&url) {
        return false;
    }
    let page = CefString::from(&frame.url()).to_string();
    let blank = frame.is_main() == 1
        && (page.is_empty() || page == "about:blank")
        && browser.can_go_back() == 0;
    #[cfg(target_os = "macos")]
    let app = crate::mac::app_for_url(&url);
    #[cfg(not(target_os = "macos"))]
    let app: Option<String> = None;
    let Some(app) = app else {
        if blank {
            close_blank_page(browser);
        }
        return true;
    };
    // The site asking: the page's, or for a page that just opened, the one that redirected it.
    let from = Some(page)
        .filter(|page| origin(page).is_some())
        .or_else(|| {
            let entry = browser.host()?.visible_navigation_entry()?;
            Some(CefString::from(&entry.url()).to_string())
        })
        .unwrap_or_default();
    // A Little Arbalete window, like a sign-in's, has no permission prompt: an alert asks.
    #[cfg(target_os = "macos")]
    if little_of(browser).is_some() {
        let site = Some(site(&from).to_string()).filter(|site| !site.is_empty());
        let site = site.unwrap_or_else(|| "A page".to_string());
        let mut ask = AskInAlert::new(browser.clone(), url, app, site, blank);
        post_task(ThreadId::UI, Some(&mut ask));
        return true;
    }
    ask_app_link(browser, &from, url, app, blank);
    true
}

thread_local! {
    /// An alert asks about another app's link, which no other piles up behind.
    static ALERTING: Cell<bool> = const { Cell::new(false) };
}

wrap_task! {
    struct AskInAlert {
        browser: Browser,
        url: String,
        app: String,
        site: String,
        blank: bool,
    }

    impl Task {
        /// Once Chromium is done with the navigation, which an alert's loop would hold.
        fn execute(&self) {
            if ALERTING.replace(true) {
                return;
            }
            #[cfg(target_os = "macos")]
            if crate::mac::confirm_open_app(&self.site, &self.app) {
                crate::mac::open_in_app(&self.url);
            }
            ALERTING.set(false);
            if self.blank {
                close_blank_page(&self.browser);
            }
        }
    }
}

/// Closes a page that opened only for another app's link: its tab, once Chromium is done with its
/// navigation, or its window or card.
pub(super) fn close_blank_page(browser: &Browser) {
    match page_tab(browser) {
        Some(tab) => post(Command::Close(Some(tab))),
        None => {
            if let Some(host) = browser.host() {
                host.close_browser(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leads_to_another_app_only_for_a_scheme_no_browser_loads() {
        assert!(is_app_link("claude://claude.ai/login?code=1"));
        assert!(is_app_link("notion://www.notion.so/callback"));
        assert!(is_app_link("zoommtg://zoom.us/join?confno=1"));
        assert!(is_app_link("mailto:me@a.test"));
        assert!(is_app_link("x-apple.systempreferences:com.apple"));
        for url in [
            "https://a.test/",
            "HTTP://a.test/",
            "arb://settings",
            "chrome://version",
            "javascript:alert(1)",
            "about:blank",
            "data:text/html,a",
            "file:///tmp",
            "blob:https://a.test/1",
            "vbscript:x",
            "a.test/path:1",
            "",
        ] {
            assert!(!is_app_link(url), "{url}");
        }
    }
}
