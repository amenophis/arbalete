//! The sidebar's media players, like Arc's: a tab left while its media plays, or while it holds
//! a call, shows a player above the sidebar's footer, which plays or pauses its media, mutes it,
//! takes back to it, and toggles its call's microphone and camera or leaves the call
//! (decision 0014).
//!
//! What plays comes from the sound's watch (sound.rs); the previous and next tracks and a call's
//! controls are the page's own, the Media Session actions it offers, like Google Meet's.

use super::*;

/// The binding through which a page tells which of its Media Session's actions the sidebar offers.
pub(super) const SESSION_BINDING: &str = "__arbaleteSession";

/// What a frame of a page plays (sound.rs).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub(super) struct FrameMedia {
    pub audible: bool,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub artwork: Option<String>,
}

/// Which of a page's Media Session's actions the sidebar offers: going to the previous and next
/// tracks, and for its call, toggling its microphone and its camera, whether on, unless the page
/// offers no toggle, and leaving.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub(super) struct Session {
    pub previous: bool,
    pub next: bool,
    pub microphone: Option<bool>,
    pub camera: Option<bool>,
    pub hangup: bool,
}

/// Keeps the handlers a page gives its Media Session's actions, which the players run, and tells
/// which it offers as the page sets its actions and says whether its microphone and camera are
/// on. Runs in the page's world, where its handlers live.
const KEEP_ACTIONS: &str = r#"(() => {
    const session = navigator.mediaSession;
    if (!session || "__arbaleteActions" in session) return;
    const actions = {};
    Object.defineProperty(session, "__arbaleteActions", { value: actions });
    const on = { microphone: true, camera: true };
    const tell = () => {
        if (typeof __arbaleteSession !== "function") return;
        __arbaleteSession(JSON.stringify({
            previous: "previoustrack" in actions,
            next: "nexttrack" in actions,
            microphone: "togglemicrophone" in actions ? on.microphone : null,
            camera: "togglecamera" in actions ? on.camera : null,
            hangup: "hangup" in actions,
        }));
    };
    const set = session.setActionHandler.bind(session);
    session.setActionHandler = (action, handler) => {
        set(action, handler);
        if (handler) actions[action] = handler; else delete actions[action];
        if (["previoustrack", "nexttrack", "togglemicrophone", "togglecamera", "hangup"].includes(action)) {
            tell();
        }
    };
    for (const [method, device] of [["setMicrophoneActive", "microphone"], ["setCameraActive", "camera"]]) {
        const original = session[method]?.bind(session);
        if (!original) continue;
        session[method] = (active) => {
            original(active);
            on[device] = active;
            tell();
        };
    }
})()"#;

/// Runs a Media Session action in a frame: the page's own handler, or for playing and pausing
/// without one, its media elements.
const RUN_ACTION: &str = r#"((action) => {
    const handler = navigator.mediaSession?.__arbaleteActions?.[action];
    if (handler) return void handler({ action });
    for (const media of document.querySelectorAll("video, audio")) {
        if (action === "pause") media.pause();
        else if (action === "play" && media.paused && !media.ended && media.currentTime > 0) media.play();
    }
})(ACTION)"#;

/// Starts keeping a tab's page's Media Session actions, in its documents to come and its current
/// one.
pub(super) fn media_page_created(browser: &Browser) {
    let Some(host) = browser.host() else { return };
    // New documents get the binding only while the Runtime domain is enabled, and run the
    // scripts added for them only while the Page domain is.
    let script = json!({ "source": KEEP_ACTIONS, "runImmediately": true });
    for (method, params) in [
        ("Runtime.enable", json!({})),
        ("Runtime.addBinding", json!({ "name": SESSION_BINDING })),
        ("Page.enable", json!({})),
        ("Page.addScriptToEvaluateOnNewDocument", script),
    ] {
        let message = json!({ "id": unanswered_id(), "method": method, "params": params });
        host.send_dev_tools_message(Some(message.to_string().as_bytes()));
    }
}

/// Learns which of a page's Media Session's actions the sidebar offers.
pub(super) fn session_told(browser: &Browser, session: Option<Session>) {
    let page = browser.identifier();
    let changed = PAGES.with_borrow_mut(|pages| {
        let live = pages
            .values_mut()
            .find(|live| live.browser.identifier() == page)?;
        Some(std::mem::replace(&mut live.session, session.clone()) != session)
    });
    if changed == Some(true) {
        send_state();
    }
}

impl UiState {
    /// The players of the tabs left playing media or holding a call, of the Spaces the window
    /// reaches.
    pub(super) fn players(&self) -> Vec<serde_json::Value> {
        let shown = self.sidebar.displayed().id;
        let mut players: Vec<_> = PAGES.with_borrow(|pages| {
            pages
                .iter()
                .filter(|(_, live)| live.hidden_since.is_some() && (live.played || live.capturing))
                .filter(|(tab, _)| {
                    self.sidebar
                        .space_of(**tab)
                        .is_some_and(|space| self.sidebar.reaches(shown, space))
                })
                .filter_map(|(&tab, live)| {
                    let NodeKind::Tab(node) = &self.sidebar.node(tab)?.kind else {
                        return None;
                    };
                    let media = live.frames.values().find(|media| media.title.is_some());
                    let title = media.and_then(|media| media.title.as_deref());
                    let artist = media.and_then(|media| media.artist.as_deref());
                    let session = live.session.clone().unwrap_or_default();
                    Some(json!({
                        "tab": tab,
                        "title": title.unwrap_or(node.label()),
                        "subtitle": artist.unwrap_or(site(&node.url)),
                        "artwork": media.and_then(|media| media.artwork.as_deref()),
                        "favicon": favicon_url(live.profile, node),
                        "playing": live.frames.values().any(|media| media.audible),
                        "muted": node.muted,
                        "previous": session.previous,
                        "next": session.next,
                        "call": live.capturing.then_some(session),
                    }))
                })
                .collect()
        });
        players.sort_by_key(|player| player["tab"].as_u64());
        players
    }
}

/// Acts on a tab's media or call from its player.
pub(super) fn media_command(tab: NodeId, action: MediaAction) {
    let name = match action {
        MediaAction::Dismiss => return dismiss(tab),
        MediaAction::Play => "play",
        MediaAction::Pause => "pause",
        MediaAction::PreviousTrack => "previoustrack",
        MediaAction::NextTrack => "nexttrack",
        MediaAction::ToggleMicrophone => "togglemicrophone",
        MediaAction::ToggleCamera => "togglecamera",
        MediaAction::HangUp => "hangup",
    };
    let page = PAGES.with_borrow_mut(|pages| {
        let live = pages.get_mut(&tab)?;
        // Media plays in the frames that played it, of other sites too; the tracks' and a call's
        // actions are the page's own.
        let sessions: HashSet<String> = match action {
            MediaAction::Play | MediaAction::Pause => live
                .frames
                .keys()
                .filter_map(|frame| Some(frame.split_once('/')?.0.to_string()))
                .collect(),
            _ => HashSet::from([String::new()]),
        };
        // A page left paused may have been frozen since.
        let frozen = std::mem::take(&mut live.frozen);
        Some((live.browser.host()?, frozen, sessions))
    });
    let Some((host, frozen, sessions)) = page else {
        return;
    };
    if frozen {
        set_frozen(Some(&host), false);
    }
    // The click on the player stands for the user's gesture, which playing asks for.
    let expression = RUN_ACTION.replace("(ACTION)", &format!("({})", json!(name)));
    let params = json!({ "expression": expression, "userGesture": true });
    for session in sessions {
        let mut message =
            json!({ "id": unanswered_id(), "method": "Runtime.evaluate", "params": params });
        if !session.is_empty() {
            message["sessionId"] = json!(session);
        }
        host.send_dev_tools_message(Some(message.to_string().as_bytes()));
    }
}

/// Hides a tab's player until its media plays again.
fn dismiss(tab: NodeId) {
    PAGES.with_borrow_mut(|pages| {
        if let Some(live) = pages.get_mut(&tab) {
            live.played = false;
        }
    });
    send_state();
}
