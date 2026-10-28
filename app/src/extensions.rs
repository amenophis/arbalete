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
use std::path::PathBuf;

/// The helper extension's directory in the bundle, if it is there: `cef` builds outside the
/// bundle run without it.
pub fn helper_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.parent()?.join("Resources/arbalete-helper");
    dir.join("manifest.json").exists().then_some(dir)
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
}

/// Starts watching the extensions from a profile's content view once its browser exists, and
/// learns which target the view is.
pub fn content_created(profile: ProfileId, browser: &Browser) {
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

/// Runs what an extension's shim asks for, through its binding.
fn binding_called(payload: &str) {
    let Ok(call) = serde_json::from_str::<Value>(payload) else {
        return;
    };
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
