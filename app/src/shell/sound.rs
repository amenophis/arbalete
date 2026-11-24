//! Which tabs play sound: like Arc's, their rows show a speaker after the icon, which mutes them,
//! and a tab left while it plays shows a player in the sidebar (media.rs).
//!
//! CEF tells nothing of it (Chromium's audible state stays inside), so each frame of a page
//! watches its media elements, in a world of its own, and tells whether one plays with sound:
//! those of its document, and those it plays without adding them to it, like Spotify's player
//! or a call's voices, and whether its Web Audio plays to the speakers, like a call's mix.

use super::*;

/// The binding through which a page's frames tell, and the world they watch from.
const BINDING: &str = "__arbaleteSound";
const WORLD: &str = "arbalete-sound";

/// Tells, as it changes, whether a media element of the frame plays with sound and what its
/// Media Session says it plays, from the first time one does, and that none does as the frame's
/// document goes.
const WATCH_SOUND: &str = r#"(() => {
    const frame = Math.random().toString(36).slice(2);
    let told = null;
    // Media the page plays without adding it to its document, handed by HAND_DETACHED, and
    // whether its Web Audio plays to the speakers.
    const detached = new Set();
    let webAudio = false;
    const tell = (audible) => {
        if ((!audible && told === null) || typeof __arbaleteSound !== "function") return;
        const metadata = navigator.mediaSession?.metadata;
        const said = JSON.stringify({ frame, audible, title: metadata?.title || null,
            artist: metadata?.artist || null, artwork: metadata?.artwork?.at(-1)?.src || null });
        if (said === told) return;
        told = said;
        __arbaleteSound(said);
    };
    const check = () => tell(webAudio || [...document.querySelectorAll("video, audio"), ...detached]
        .some((media) => !media.paused && !media.ended && !media.muted && media.volume > 0));
    // Playing media's time updating also catches its Media Session's metadata changing, like a
    // player going to its next track in the same media element.
    const types = ["playing", "pause", "ended", "emptied", "volumechange", "loadedmetadata",
        "timeupdate"];
    for (const type of types) addEventListener(type, check, true);
    for (const [type, on] of [["arbalete-audio-on", true], ["arbalete-audio-off", false]]) {
        document.addEventListener(type, () => { webAudio = on; check(); });
    }
    // A detached media element's events reach neither its document nor its window.
    document.addEventListener("arbalete-media", (event) => {
        const media = event.relatedTarget;
        if (!(media instanceof HTMLMediaElement) || detached.has(media)) return;
        detached.add(media);
        for (const type of types) media.addEventListener(type, check);
    });
    addEventListener("pageshow", check);
    addEventListener("pagehide", () => tell(false));
    check();
})()"#;

/// Hands the sound's watch, in its world, each media element the page plays without adding it to
/// its document, as the related target of an event on the document, and tells it whether a
/// running audio context of the page plays to the speakers: only the page's world sees them.
const HAND_DETACHED: &str = r#"(() => {
    const hand = (media) => {
        if (!media.isConnected) {
            document.dispatchEvent(new FocusEvent("arbalete-media", { relatedTarget: media }));
        }
    };
    const play = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function () {
        hand(this);
        return play.apply(this, arguments);
    };
    // A call's voices may play from a stream given to an element that plays it by itself.
    const source = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, "srcObject");
    Object.defineProperty(HTMLMediaElement.prototype, "srcObject", { ...source, set(stream) {
        hand(this);
        source.set.call(this, stream);
    } });
    if (typeof AudioNode === "undefined") return;
    const watched = new WeakSet();
    const running = new Set();
    const update = (context) => {
        const before = running.size > 0;
        if (context.state === "running") running.add(context); else running.delete(context);
        if (before !== running.size > 0) {
            document.dispatchEvent(new Event(running.size ? "arbalete-audio-on" : "arbalete-audio-off"));
        }
    };
    const connect = AudioNode.prototype.connect;
    AudioNode.prototype.connect = function (target) {
        const context = this.context;
        if (target instanceof AudioDestinationNode && context instanceof AudioContext
            && !watched.has(context)) {
            watched.add(context);
            context.addEventListener("statechange", () => update(context));
            update(context);
        }
        return connect.apply(this, arguments);
    };
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
    let hand = json!({ "source": HAND_DETACHED, "runImmediately": true });
    let attach = json!({ "autoAttach": true, "waitForDebuggerOnStart": false, "flatten": true });
    for (method, params) in [
        ("Runtime.enable", json!({})),
        ("Runtime.addBinding", binding),
        ("Page.enable", json!({})),
        ("Page.addScriptToEvaluateOnNewDocument", script),
        ("Page.addScriptToEvaluateOnNewDocument", hand),
        ("Target.setAutoAttach", attach),
    ] {
        let mut message = json!({ "id": unanswered_id(), "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        host.send_dev_tools_message(Some(message.to_string().as_bytes()));
    }
}

/// Learns from a message of a page's DevTools session what one of its frames plays, or that
/// frames went: its main frame's document, whose frames play none yet, or a frame of another
/// site. Watches each frame of another site as it comes, and hands what the page tells of its
/// Media Session's actions to media.rs.
pub fn page_sound(browser: &Browser, message: &serde_json::Value) {
    // Frames are told apart by their session and the name each gives itself.
    let session = message["sessionId"].as_str().unwrap_or_default();
    let method = message["method"].as_str().unwrap_or_default();
    let params = &message["params"];
    let told = match method {
        "Runtime.bindingCalled" if params["name"] == BINDING => {
            let Some(Said { frame, media }) = payload(params) else {
                return;
            };
            Told::Frame(format!("{session}/{frame}"), media)
        }
        "Runtime.bindingCalled" if params["name"] == SESSION_BINDING && session.is_empty() => {
            return session_told(browser, payload(params));
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
        let before = (live.frames.clone(), live.played);
        match told {
            Told::Frame(frame, media) => {
                live.frames.insert(frame, media);
            }
            Told::Gone(prefix) => {
                live.frames.retain(|frame, _| !frame.starts_with(&prefix));
                if prefix.is_empty() {
                    live.session = None;
                }
            }
        }
        // Media playing while its page is hidden keeps its player, paused too; media paused while
        // its page shows has none once the page is left.
        let audible = live.frames.values().any(|media| media.audible);
        live.played = audible || (live.played && live.hidden_since.is_some());
        Some(before != (live.frames.clone(), live.played))
    });
    // Its row shows the speaker, or no longer, and its player what it plays.
    if changed == Some(true) {
        send_state();
    }
}

/// What a binding was called with.
fn payload<T: serde::de::DeserializeOwned>(params: &serde_json::Value) -> Option<T> {
    serde_json::from_str(params["payload"].as_str()?).ok()
}

/// What a frame tells of its media, by the name it gives itself.
#[derive(Deserialize)]
struct Said {
    frame: String,
    #[serde(flatten)]
    media: FrameMedia,
}

/// What a page's DevTools session tells of its sound.
enum Told {
    /// What a frame plays, by its session and its name.
    Frame(String, FrameMedia),
    /// The frames whose names start so went.
    Gone(String),
}

/// Whether a tab's page plays sound: its row shows the speaker.
pub(super) fn audible(tab: NodeId) -> bool {
    PAGES.with_borrow(|pages| {
        pages
            .get(&tab)
            .is_some_and(|live| live.frames.values().any(|media| media.audible))
    })
}
