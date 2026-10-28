//! Chrome extensions in Arbalete's Alloy pages (decision 0001): their content scripts and their
//! service workers run, but Chrome's tab and window APIs find no window, as the pages sit in none.
//! A DevTools session the app opens on a page of its own sees every target of the browser: it
//! attaches to each extension's service worker as it starts and injects a shim
//! (extension-shim.js), which answers with the profile's active tab and asks Arbalete to create or
//! update tabs. A helper extension (app/extension), loaded in every profile, tells each page's tab
//! id, which extensions know pages by.

use arbalete_core::sidebar::ProfileId;
use cef::*;
use serde::Serialize;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The helper extension's directory in the bundle, if it is there: `cef` builds outside the
/// bundle run without it.
pub fn helper_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.parent()?.join("Resources/arbalete-helper");
    dir.join("manifest.json").exists().then_some(dir)
}

/// An extension the user installed in a profile, as its button shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Installed {
    pub id: String,
    pub name: String,
    /// Its button's popup, a path in the extension, if it has one.
    #[serde(skip)]
    pub popup: Option<String>,
    /// Its button's icon file.
    #[serde(skip)]
    pub icon: Option<PathBuf>,
}

/// Where Chromium installs extensions, as its preferences record it: from the Web Store or
/// elsewhere into the profile (1), unpacked by the user (4), or by other programs (6, 7). Its own
/// components (5) and extensions loaded from the command line (8), like the helper, are not the
/// user's.
const USER_LOCATIONS: [i64; 4] = [1, 4, 6, 7];

/// The extensions installed and enabled in a profile's directory, by name.
pub fn installed(profile_dir: &Path) -> Vec<Installed> {
    let preferences = std::fs::read(profile_dir.join("Secure Preferences")).ok();
    let preferences: Value = preferences
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let Some(settings) = preferences["extensions"]["settings"].as_object() else {
        return Vec::new();
    };
    let mut list: Vec<_> = settings
        .iter()
        .filter(|(_, setting)| {
            let location = setting["location"].as_i64().unwrap_or_default();
            let disabled = setting["disable_reasons"]
                .as_array()
                .is_some_and(|reasons| !reasons.is_empty());
            USER_LOCATIONS.contains(&location) && !disabled
        })
        .filter_map(|(id, setting)| {
            let path = Path::new(setting["path"].as_str()?);
            let dir = if path.is_absolute() {
                path.to_path_buf()
            } else {
                profile_dir.join("Extensions").join(path)
            };
            read_manifest(id, &dir)
        })
        .collect();
    list.sort_by_key(|extension| extension.name.to_lowercase());
    list
}

/// An extension's name, popup and icon, from its manifest in `dir`.
fn read_manifest(id: &str, dir: &Path) -> Option<Installed> {
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).ok()?).ok()?;
    // Manifest V2 extensions have a browser action instead.
    let action = if manifest["action"].is_object() {
        &manifest["action"]
    } else {
        &manifest["browser_action"]
    };
    let icon = largest_icon(&action["default_icon"]).or_else(|| largest_icon(&manifest["icons"]));
    Some(Installed {
        id: id.to_string(),
        name: localized(dir, &manifest, manifest["name"].as_str()?),
        popup: action["default_popup"]
            .as_str()
            .filter(|popup| !popup.is_empty())
            .map(str::to_string),
        icon: icon.map(|icon| dir.join(icon.trim_start_matches('/'))),
    })
}

/// The path of an icon, or of the largest of several by size.
fn largest_icon(icons: &Value) -> Option<String> {
    if let Some(icon) = icons.as_str() {
        return Some(icon.to_string());
    }
    let icons = icons.as_object()?;
    let largest = icons
        .iter()
        .max_by_key(|(size, _)| size.parse::<u32>().unwrap_or_default());
    largest
        .and_then(|(_, icon)| icon.as_str())
        .map(str::to_string)
}

/// A manifest's text, its `__MSG_<key>__` placeholders read from the extension's default
/// locale's messages, whose keys ignore case.
fn localized(dir: &Path, manifest: &Value, text: &str) -> String {
    let Some(key) = text
        .strip_prefix("__MSG_")
        .and_then(|key| key.strip_suffix("__"))
    else {
        return text.to_string();
    };
    let locale = manifest["default_locale"].as_str().unwrap_or("en");
    let messages = std::fs::read(dir.join("_locales").join(locale).join("messages.json")).ok();
    let messages: Value = messages
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let message = messages.as_object().and_then(|messages| {
        let (_, message) = messages
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))?;
        message["message"].as_str()
    });
    message.unwrap_or(text).to_string()
}

/// The helper extension's service worker, which every extension url is told apart from.
const HELPER_SCRIPT: &str = "/arbalete-helper.js";

const SHIM: &str = include_str!("extension-shim.js");

/// The selected tab of a profile's content view, as extensions see it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActiveTab {
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
}

/// A profile's content view: its DevTools target, its browser context and, once the helper has
/// told it, its tab id.
struct Content {
    target: String,
    context: String,
    tab_id: Option<i64>,
}

/// What a message sent in the session answers.
enum Pending {
    /// Attaching to an extension's service worker, in a browser context.
    Attach { context: String, helper: bool },
    /// The helper's list of targets and their tab ids, in a browser context.
    TabIds(String),
}

#[derive(Default)]
struct Bridge {
    host: Option<BrowserHost>,
    registrations: Vec<Registration>,
    next_id: i32,
    pending: HashMap<i32, Pending>,
    /// The workers attached to, by target id.
    attached: HashSet<String>,
    /// The extensions' workers' sessions, with their browser context.
    workers: HashMap<String, String>,
    /// The helper's session in each browser context.
    helpers: HashMap<String, String>,
    contents: HashMap<ProfileId, Content>,
    tabs: HashMap<ProfileId, ActiveTab>,
}

thread_local! {
    static BRIDGE: RefCell<Bridge> = RefCell::default();
    /// Each profile's extensions, as their buttons show them, and its directory.
    static INSTALLED: RefCell<HashMap<ProfileId, (PathBuf, Vec<Installed>)>> = RefCell::default();
    /// The open popup's DevTools observer.
    static POPUP: RefCell<Option<Registration>> = const { RefCell::new(None) };
}

/// The extensions' icon files, which the UI loads by their profile and extension id
/// (icon_file), from the IO thread.
static ICONS: Mutex<Vec<(String, PathBuf)>> = Mutex::new(Vec::new());

/// A profile's extensions, as their buttons show them.
pub fn list(profile: ProfileId) -> Vec<Installed> {
    INSTALLED.with_borrow(|installed| {
        installed
            .get(&profile)
            .map(|(_, list)| list.clone())
            .unwrap_or_default()
    })
}

/// The UI path of an extension's icon, if it has one.
pub fn icon_path(profile: ProfileId, extension: &Installed) -> Option<String> {
    let profile = serde_json::to_string(&profile).expect("an id");
    extension
        .icon
        .as_ref()
        .map(|_| format!("extension-icon/{profile}/{}", extension.id))
}

/// The icon file a UI path names (icon_path), with its media type.
pub fn icon_file(path: &str) -> Option<(PathBuf, &'static str)> {
    let key = path.strip_prefix("extension-icon/")?;
    let icons = ICONS.lock().expect("not poisoned");
    let (_, file) = icons.iter().find(|(icon, _)| icon == key)?;
    let mime = match file.extension()?.to_str()? {
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        _ => "image/png",
    };
    Some((file.clone(), mime))
}

/// Learns an extension just installed from its service worker starting, at `url`, in a browser
/// context: Chromium writes its preferences, which list the extensions, only later. The
/// extension shows in its profile's list, and the windows Chromium opened for it close.
fn worker_started(context: &str, url: &str) {
    let Some(id) = url
        .strip_prefix("chrome-extension://")
        .and_then(|rest| rest.split('/').next())
    else {
        return;
    };
    let profile = BRIDGE.with_borrow(|bridge| {
        let (profile, _) = bridge
            .contents
            .iter()
            .find(|(_, content)| content.context == context)?;
        Some(*profile)
    });
    let Some(profile) = profile else { return };
    let new = INSTALLED.with_borrow(|installed| {
        let (dir, list) = installed.get(&profile)?;
        (!list.iter().any(|extension| extension.id == id)).then(|| dir.clone())
    });
    let Some(dir) = new else { return };
    // Its files are in a directory named after its version.
    let versions = std::fs::read_dir(dir.join("Extensions").join(id))
        .into_iter()
        .flatten()
        .flatten();
    let Some(extension) = versions
        .filter_map(|version| read_manifest(id, &version.path()))
        .next()
    else {
        return;
    };
    INSTALLED.with_borrow_mut(|installed| {
        if let Some((_, list)) = installed.get_mut(&profile) {
            list.push(extension);
            list.sort_by_key(|extension| extension.name.to_lowercase());
        }
    });
    update_icons(profile);
    crate::shell::extension_installed();
}

/// Removes an extension from a profile, through the profile's helper, which holds the
/// `management` permission.
pub fn uninstall(profile: ProfileId, id: &str) {
    let session = BRIDGE.with_borrow(|bridge| {
        let context = &bridge.contents.get(&profile)?.context;
        bridge.helpers.get(context).cloned()
    });
    let Some(session) = session else { return };
    let id_json = serde_json::to_string(id).expect("a string");
    // Without Chromium's own confirmation: the app asks first.
    let expression =
        format!("chrome.management.uninstall({id_json}, {{ showConfirmDialog: false }})");
    let params = json!({ "expression": expression });
    send_in(&session, "Runtime.evaluate", params, None);
    INSTALLED.with_borrow_mut(|installed| {
        if let Some((_, list)) = installed.get_mut(&profile) {
            list.retain(|extension| extension.id != id);
        }
    });
    update_icons(profile);
}

/// Reads a profile's extensions again.
fn refresh(profile: ProfileId, dir: &Path) {
    let list = installed(dir);
    INSTALLED.with_borrow_mut(|all| all.insert(profile, (dir.to_path_buf(), list)));
    update_icons(profile);
}

/// Lets the UI load a profile's extensions' icons (icon_file).
fn update_icons(profile: ProfileId) {
    let list = list(profile);
    let id = serde_json::to_string(&profile).expect("an id");
    let mut icons = ICONS.lock().expect("not poisoned");
    icons.retain(|(key, _)| !key.starts_with(&format!("{id}/")));
    icons.extend(
        list.iter().filter_map(|extension| {
            Some((format!("{id}/{}", extension.id), extension.icon.clone()?))
        }),
    );
}

/// The address of an extension's popup, if it has one.
pub fn popup_url(profile: ProfileId, id: &str) -> Option<String> {
    let extension = list(profile)
        .into_iter()
        .find(|extension| extension.id == id)?;
    Some(format!(
        "chrome-extension://{id}/{}",
        extension.popup?.trim_start_matches('/')
    ))
}

/// Prepares an extension's popup once its blank page has loaded, then loads it: the shim and its
/// binding are installed in the page before any of its scripts runs, with the profile's active
/// tab, and the page sizes the popup, like Chrome's, from 25 to 800 by 600 points.
pub fn popup_created(profile: ProfileId, browser: &Browser, url: &str) {
    let Some(host) = browser.host() else { return };
    host.set_auto_resize_enabled(
        1,
        Some(&Size {
            width: 25,
            height: 25,
        }),
        Some(&Size {
            width: 800,
            height: 600,
        }),
    );
    let mut observer = PopupObserver::new(url.to_string());
    let registration = host.add_dev_tools_message_observer(Some(&mut observer));
    POPUP.set(registration);
    let state = BRIDGE.with_borrow(|bridge| {
        let tab_id = bridge
            .contents
            .get(&profile)
            .and_then(|content| content.tab_id);
        state_json(bridge.tabs.get(&profile), tab_id)
    });
    let source = format!("self.__arbaleteInitial = {state};\n{SHIM}");
    // New documents get the binding only while the Runtime domain is enabled, and the scripts
    // added for them only while the Page domain is.
    for message in [
        json!({ "id": 3, "method": "Runtime.enable" }),
        json!({ "id": 1, "method": "Runtime.addBinding", "params": { "name": "__arbalete" } }),
        json!({ "id": 3, "method": "Page.enable" }),
        json!({ "id": 2, "method": "Page.addScriptToEvaluateOnNewDocument", "params": { "source": source } }),
    ] {
        host.send_dev_tools_message(Some(message.to_string().as_bytes()));
    }
}

/// Forgets the closed popup's observer.
pub fn popup_closed() {
    POPUP.set(None);
}

/// Starts watching the extensions from a profile's content view once its browser exists, learns
/// which target the view is, and reads the profile's extensions from its directory.
pub fn content_created(profile: ProfileId, browser: &Browser, dir: &Path) {
    refresh(profile, dir);
    let Some(host) = browser.host() else { return };
    let started = BRIDGE.with_borrow(|bridge| bridge.host.is_some());
    if !started {
        let registration = host.add_dev_tools_message_observer(Some(&mut BridgeObserver::new()));
        BRIDGE.with_borrow_mut(|bridge| {
            bridge.host = Some(host.clone());
            bridge.registrations.extend(registration);
        });
        let params = json!({ "discover": true });
        send(
            json!({ "method": "Target.setDiscoverTargets", "params": params }),
            None,
        );
    }
    // The view's own session answers with the view's target.
    let registration =
        host.add_dev_tools_message_observer(Some(&mut ContentObserver::new(profile)));
    BRIDGE.with_borrow_mut(|bridge| bridge.registrations.extend(registration));
    let message = json!({ "id": CONTENT_INFO, "method": "Target.getTargetInfo" });
    host.send_dev_tools_message(Some(message.to_string().as_bytes()));
}

/// The id of the message asking a content view's session for its own target, apart from the
/// bridge's, which start above it.
const CONTENT_INFO: i32 = 1;

/// Tells the extensions each profile's active tab, after the sidebar's state changes.
pub fn sync(tabs: HashMap<ProfileId, ActiveTab>) {
    let changed = BRIDGE.with_borrow_mut(|bridge| {
        let changed: Vec<_> = bridge
            .contents
            .iter()
            .filter(|(profile, _)| bridge.tabs.get(profile) != tabs.get(profile))
            .map(|(_, content)| content.context.clone())
            .collect();
        bridge.tabs = tabs;
        changed
    });
    for context in changed {
        push_state(&context);
    }
}

/// Sends a message in the bridge's session, recording first what its answer is for: CEF may
/// answer while sending, calling the observer back.
fn send(mut message: Value, pending: Option<Pending>) {
    let host = BRIDGE.with_borrow_mut(|bridge| {
        bridge.next_id += 1;
        let id = CONTENT_INFO + bridge.next_id;
        message["id"] = json!(id);
        if let Some(pending) = pending {
            bridge.pending.insert(id, pending);
        }
        bridge.host.clone()
    });
    if let Some(host) = host {
        host.send_dev_tools_message(Some(message.to_string().as_bytes()));
    }
}

/// Sends a message in an attached target's session.
fn send_in(session: &str, method: &str, params: Value, pending: Option<Pending>) {
    let message = json!({ "sessionId": session, "method": method, "params": params });
    send(message, pending);
}

/// Gives the extensions' workers of a browser context its profile's active tab, once its tab id
/// is known.
fn push_state(context: &str) {
    let (state, workers) = BRIDGE.with_borrow(|bridge| {
        let workers: Vec<String> = bridge
            .workers
            .iter()
            .filter(|(_, worker)| *worker == context)
            .map(|(session, _)| session.clone())
            .collect();
        let state = bridge
            .contents
            .iter()
            .find(|(_, content)| content.context == context)
            .map(|(profile, content)| state_json(bridge.tabs.get(profile), content.tab_id));
        (state, workers)
    });
    let Some(state) = state else { return };
    for session in workers {
        let expression = format!("self.__arbaleteSync?.({state})");
        send_in(
            &session,
            "Runtime.evaluate",
            json!({ "expression": expression }),
            None,
        );
    }
}

/// The state the shim answers with: the active tab, if any and its tab id is known.
fn state_json(tab: Option<&ActiveTab>, tab_id: Option<i64>) -> Value {
    let tabs: Vec<Value> = match (tab, tab_id) {
        (Some(tab), Some(id)) => vec![json!({
            "id": id,
            "index": 0,
            "windowId": 1,
            "active": true,
            "highlighted": true,
            "pinned": false,
            "incognito": false,
            "status": "complete",
            "url": tab.url,
            "title": tab.title,
            "favIconUrl": tab.favicon,
            "discarded": false,
            "autoDiscardable": true,
            "groupId": -1,
        })],
        _ => Vec::new(),
    };
    json!({ "tabs": tabs })
}

/// Whether a target is an extension's service worker, and the helper's.
fn worker_kind(target: &Value) -> Option<bool> {
    let url = target["url"].as_str()?;
    (target["type"] == "service_worker" && url.starts_with("chrome-extension://"))
        .then(|| url.ends_with(HELPER_SCRIPT))
}

/// Attaches to an extension's service worker as it starts.
fn target_seen(target: &Value) {
    let (Some(helper), Some(id)) = (worker_kind(target), target["targetId"].as_str()) else {
        return;
    };
    let context = target["browserContextId"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if !BRIDGE.with_borrow_mut(|bridge| bridge.attached.insert(id.to_string())) {
        return;
    }
    if !helper {
        worker_started(&context, target["url"].as_str().unwrap_or_default());
    }
    let params = json!({ "targetId": id, "flatten": true });
    let message = json!({ "method": "Target.attachToTarget", "params": params });
    send(message, Some(Pending::Attach { context, helper }));
}

/// Shims an extension's worker once attached, or asks the helper for the tab ids.
fn attached(session: &str, context: String, helper: bool) {
    if helper {
        BRIDGE
            .with_borrow_mut(|bridge| bridge.helpers.insert(context.clone(), session.to_string()));
        ask_tab_ids(&context);
        return;
    }
    BRIDGE.with_borrow_mut(|bridge| bridge.workers.insert(session.to_string(), context.clone()));
    send_in(
        session,
        "Runtime.addBinding",
        json!({ "name": "__arbalete" }),
        None,
    );
    send_in(
        session,
        "Runtime.evaluate",
        json!({ "expression": SHIM }),
        None,
    );
    push_state(&context);
}

/// Asks the helper of a browser context which tab id each page has.
fn ask_tab_ids(context: &str) {
    let Some(session) = BRIDGE.with_borrow(|bridge| bridge.helpers.get(context).cloned()) else {
        return;
    };
    let params = json!({ "expression": "self.arbaleteTargets()", "awaitPromise": true, "returnByValue": true });
    let pending = Pending::TabIds(context.to_string());
    send_in(&session, "Runtime.evaluate", params, Some(pending));
}

/// Records the tab id of the content views of a browser context.
fn tab_ids(context: &str, targets: &[Value]) {
    BRIDGE.with_borrow_mut(|bridge| {
        for content in bridge
            .contents
            .values_mut()
            .filter(|c| c.context == context)
        {
            let target = targets.iter().find(|t| t["id"] == content.target.as_str());
            content.tab_id = target.and_then(|t| t["tabId"].as_i64());
        }
    });
    push_state(context);
}

/// Runs what an extension's shim asks for, through its binding: in a popup, closing it too.
fn binding_called(payload: &str) {
    let Ok(call) = serde_json::from_str::<Value>(payload) else {
        return;
    };
    if call["method"] == "close" {
        crate::shell::post(crate::command::Command::CloseExtensionPopup);
        return;
    }
    let Some(url) = call["props"]["url"].as_str() else {
        return;
    };
    match call["method"].as_str() {
        Some("create") => crate::shell::run(crate::command::Command::Open(url.to_string())),
        Some("update") => crate::shell::run(crate::command::Command::Navigate(url.to_string())),
        _ => {}
    }
}

fn message_received(message: &Value) {
    match message["method"].as_str() {
        Some("Target.targetCreated" | "Target.targetInfoChanged") => {
            target_seen(&message["params"]["targetInfo"]);
        }
        Some("Target.targetDestroyed") => {
            if let Some(id) = message["params"]["targetId"].as_str() {
                BRIDGE.with_borrow_mut(|bridge| bridge.attached.remove(id));
            }
        }
        Some("Target.detachedFromTarget") => {
            if let Some(session) = message["params"]["sessionId"].as_str() {
                BRIDGE.with_borrow_mut(|bridge| {
                    bridge.workers.remove(session);
                    bridge.helpers.retain(|_, helper| helper != session);
                });
            }
        }
        Some("Runtime.bindingCalled") if message["params"]["name"] == "__arbalete" => {
            binding_called(message["params"]["payload"].as_str().unwrap_or_default());
        }
        _ => {}
    }
    let Some(id) = message["id"].as_i64().and_then(|id| i32::try_from(id).ok()) else {
        return;
    };
    match BRIDGE.with_borrow_mut(|bridge| bridge.pending.remove(&id)) {
        Some(Pending::Attach { context, helper }) => {
            if let Some(session) = message["result"]["sessionId"].as_str() {
                attached(session, context, helper);
            }
        }
        Some(Pending::TabIds(context)) => {
            let targets = message["result"]["result"]["value"].as_array().cloned();
            tab_ids(&context, &targets.unwrap_or_default());
        }
        None => {}
    }
}

/// Records a profile's content view's target, from its own session's answer.
fn content_info(profile: ProfileId, message: &Value) {
    let info = &message["result"]["targetInfo"];
    let (Some(target), Some(context)) =
        (info["targetId"].as_str(), info["browserContextId"].as_str())
    else {
        return;
    };
    let content = Content {
        target: target.to_string(),
        context: context.to_string(),
        tab_id: None,
    };
    BRIDGE.with_borrow_mut(|bridge| bridge.contents.insert(profile, content));
    ask_tab_ids(context);
}

fn parse(message: Option<&[u8]>) -> Option<Value> {
    serde_json::from_slice(message?).ok()
}

wrap_dev_tools_message_observer! {
    struct BridgeObserver {}

    impl DevToolsMessageObserver {
        // Not handled here: CEF still dispatches the message to the other observers.
        fn on_dev_tools_message(&self, _browser: Option<&mut Browser>, message: Option<&[u8]>) -> i32 {
            if let Some(message) = parse(message) {
                message_received(&message);
            }
            0
        }
    }
}

wrap_dev_tools_message_observer! {
    struct PopupObserver {
        url: String,
    }

    impl DevToolsMessageObserver {
        fn on_dev_tools_message(&self, browser: Option<&mut Browser>, message: Option<&[u8]>) -> i32 {
            let Some(message) = parse(message) else { return 0 };
            if message["method"] == "Runtime.bindingCalled" {
                binding_called(message["params"]["payload"].as_str().unwrap_or_default());
            }
            // The shim is added for new documents: the popup's may load.
            if message["id"] == 2
                && let Some(frame) = browser.and_then(|browser| browser.main_frame())
            {
                frame.load_url(Some(&CefString::from(self.url.as_str())));
            }
            0
        }
    }
}

wrap_dev_tools_message_observer! {
    struct ContentObserver {
        profile: ProfileId,
    }

    impl DevToolsMessageObserver {
        fn on_dev_tools_message(&self, _browser: Option<&mut Browser>, message: Option<&[u8]>) -> i32 {
            if let Some(message) = parse(message).filter(|m| m["id"] == CONTENT_INFO && m["sessionId"].is_null()) {
                content_info(self.profile, &message);
            }
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of its own under the system's temporary directory, removed on drop.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("arbalete-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn write(&self, path: &str, content: &str) {
            let path = self.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn lists_the_extensions_the_user_installed_and_enabled() {
        let profile = TestDir::new("extensions");
        let preferences = json!({ "extensions": { "settings": {
            "dark": { "location": 1, "path": "dark/1.0_0", "disable_reasons": [] },
            "block": { "location": 1, "path": "block/2.0_0", "disable_reasons": [] },
            "off": { "location": 1, "path": "off/1.0_0", "disable_reasons": [1] },
            "pdf": { "location": 5, "path": "/framework/pdf" },
            "helper": { "location": 8, "path": "/bundle/arbalete-helper" },
        } } });
        profile.write("Secure Preferences", &preferences.to_string());
        profile.write(
            "Extensions/dark/1.0_0/manifest.json",
            r#"{ "name": "Dark Reader", "action": { "default_popup": "ui/popup.html", "default_icon": { "19": "i19.png", "38": "i38.png" } } }"#,
        );
        profile.write(
            "Extensions/block/2.0_0/manifest.json",
            r#"{ "name": "__MSG_extName__", "default_locale": "en", "icons": { "16": "/i16.png", "128": "i128.png" } }"#,
        );
        profile.write(
            "Extensions/block/2.0_0/_locales/en/messages.json",
            r#"{ "extname": { "message": "uBlock Origin Lite" } }"#,
        );
        profile.write("Extensions/off/1.0_0/manifest.json", r#"{ "name": "Off" }"#);
        let list = installed(&profile.0);
        let names: Vec<_> = list
            .iter()
            .map(|extension| extension.name.as_str())
            .collect();
        assert_eq!(names, ["Dark Reader", "uBlock Origin Lite"]);
        assert_eq!(list[0].popup.as_deref(), Some("ui/popup.html"));
        assert_eq!(
            list[0].icon,
            Some(profile.0.join("Extensions/dark/1.0_0/i38.png"))
        );
        assert_eq!(list[1].popup, None);
        assert_eq!(
            list[1].icon,
            Some(profile.0.join("Extensions/block/2.0_0/i128.png"))
        );
        assert!(installed(&profile.0.join("missing")).is_empty());
    }

    #[test]
    fn tells_extension_workers_and_the_helper_from_other_targets() {
        let target = |kind: &str, url: &str| json!({ "type": kind, "url": url });
        assert_eq!(
            worker_kind(&target(
                "service_worker",
                "chrome-extension://abc/background.js"
            )),
            Some(false)
        );
        assert_eq!(
            worker_kind(&target(
                "service_worker",
                "chrome-extension://def/arbalete-helper.js"
            )),
            Some(true)
        );
        assert_eq!(
            worker_kind(&target("page", "chrome-extension://abc/popup.html")),
            None
        );
        assert_eq!(
            worker_kind(&target("service_worker", "https://site.test/sw.js")),
            None
        );
    }

    #[test]
    fn answers_with_the_active_tab_once_its_tab_id_is_known() {
        let tab = ActiveTab {
            url: "https://example.com/".into(),
            title: "Example".into(),
            favicon: None,
        };
        assert_eq!(state_json(Some(&tab), None), json!({ "tabs": [] }));
        assert_eq!(state_json(None, Some(7)), json!({ "tabs": [] }));
        let state = state_json(Some(&tab), Some(7));
        let tab = &state["tabs"][0];
        assert_eq!(
            (&tab["id"], &tab["url"], &tab["active"], &tab["windowId"]),
            (
                &json!(7),
                &json!("https://example.com/"),
                &json!(true),
                &json!(1)
            )
        );
    }
}
