//! Which tabs play sound: like Arc's, their rows show a speaker after the icon, which mutes them.
//!
//! CEF tells nothing of it (Chromium's audible state stays inside), so each frame of a page
//! watches its media elements, in a world of its own, and tells whether one plays with sound.

use super::*;

/// The binding through which a page's frames tell, and the world they watch from.
const BINDING: &str = "__arbaleteSound";
const WORLD: &str = "arbalete-sound";

/// The ids of the messages sent to pages here, apart from the others' (boosts.rs).
const FIRST_MESSAGE: i32 = 28_000_000;

thread_local! {
    static NEXT_MESSAGE: Cell<i32> = const { Cell::new(FIRST_MESSAGE) };
}

/// Tells, as it changes, whether a media element of the frame plays with sound, and that it no
/// longer does as the frame's document goes.
const WATCH_SOUND: &str = r#"(() => {
    const frame = Math.random().toString(36).slice(2);
    let audible = false;
    const tell = (now) => {
        if (now === audible || typeof __arbaleteSound !== "function") return;
        audible = now;
        __arbaleteSound(JSON.stringify({ frame, audible }));
    };
    const check = () => tell([...document.querySelectorAll("video, audio")].some((media) =>
        !media.paused && !media.ended && !media.muted && media.volume > 0));
    for (const type of ["playing", "pause", "ended", "emptied", "volumechange"]) {
        addEventListener(type, check, true);
    }
    addEventListener("pageshow", check);
    addEventListener("pagehide", () => tell(false));
    check();
})()"#;

/// Starts watching a tab's page's sound, in its documents to come and its current one.
pub(super) fn sound_page_created(browser: &Browser) {
    if let Some(host) = browser.host() {
        watch(&host, None);
    }
}

/// Watches the sound of a page's frames, or of a frame of another site (`session`), which runs
/// apart and has a DevTools session of its own: a video embedded from another site, like
/// YouTube's, plays there.
fn watch(host: &BrowserHost, session: Option<&str>) {
    // New documents get the binding only while the Runtime domain is enabled, and run the
    // scripts added for them only while the Page domain is.
    let binding = json!({ "name": BINDING, "executionContextName": WORLD });
    let script = json!({ "source": WATCH_SOUND, "worldName": WORLD, "runImmediately": true });
    let attach = json!({ "autoAttach": true, "waitForDebuggerOnStart": false, "flatten": true });
    for (method, params) in [
        ("Runtime.enable", json!({})),
        ("Runtime.addBinding", binding),
        ("Page.enable", json!({})),
        ("Page.addScriptToEvaluateOnNewDocument", script),
        ("Target.setAutoAttach", attach),
    ] {
        // Chromium refuses a message whose id another one waiting for its answer has.
        let id = NEXT_MESSAGE.get();
        NEXT_MESSAGE.set(id.checked_add(1).unwrap_or(FIRST_MESSAGE));
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        host.send_dev_tools_message(Some(message.to_string().as_bytes()));
    }
}

/// Learns from a message of a page's DevTools session whether one of its frames plays sound, or
/// that frames went: its main frame's document, whose frames play none yet, or a frame of another
/// site. Watches each frame of another site as it comes.
pub fn page_sound(browser: &Browser, message: &serde_json::Value) {
    // Frames are told apart by their session and the name each gives itself.
    let session = message["sessionId"].as_str().unwrap_or_default();
    let method = message["method"].as_str().unwrap_or_default();
    let params = &message["params"];
    let told = match method {
        "Runtime.bindingCalled" if params["name"] == BINDING => {
            let payload: serde_json::Value = params["payload"]
                .as_str()
                .and_then(|payload| serde_json::from_str(payload).ok())
                .unwrap_or_default();
            let Some(frame) = payload["frame"].as_str() else {
                return;
            };
            Told::Frame(format!("{session}/{frame}"), payload["audible"] == true)
        }
        "Page.frameNavigated" if session.is_empty() && params["frame"]["parentId"].is_null() => {
            Told::Gone(String::new())
        }
        "Target.detachedFromTarget" => Told::Gone(format!(
            "{}/",
            params["sessionId"].as_str().unwrap_or_default()
        )),
        "Target.attachedToTarget" => {
            if params["targetInfo"]["type"] == "iframe"
                && let (Some(session), Some(host)) = (params["sessionId"].as_str(), browser.host())
            {
                watch(&host, Some(session));
            }
            return;
        }
        _ => return,
    };
    let page = browser.identifier();
    let changed = PAGES.with_borrow_mut(|pages| {
        let live = pages
            .values_mut()
            .find(|live| live.browser.identifier() == page)?;
        let before = live.audible.is_empty();
        match told {
            Told::Frame(frame, true) => {
                live.audible.insert(frame);
            }
            Told::Frame(frame, false) => {
                live.audible.remove(&frame);
            }
            Told::Gone(prefix) => live.audible.retain(|frame| !frame.starts_with(&prefix)),
        }
        Some(before != live.audible.is_empty())
    });
    // Its row shows the speaker, or no longer.
    if changed == Some(true) {
        send_state();
    }
}

/// What a page's DevTools session tells of its sound.
enum Told {
    /// A frame plays sound, or no longer.
    Frame(String, bool),
    /// The frames whose names start so went.
    Gone(String),
}

/// Whether a tab's page plays sound: its row shows the speaker.
pub(super) fn audible(tab: NodeId) -> bool {
    PAGES.with_borrow(|pages| pages.get(&tab).is_some_and(|live| !live.audible.is_empty()))
}
