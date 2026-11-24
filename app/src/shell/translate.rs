//! Translating pages on the device, like Arc's translation but by macOS (decision 0016): a page
//! in a language the user does not read offers it by the address, and its text is translated in
//! place, the originals kept to show again.
//!
//! A script in a world of its own, in the page's main frame, tells the page's language and hands
//! its text; the native interface recognizes the language and translates (app/macos/Translate.swift).

use super::*;

/// The binding through which the main frame's script tells, and the world it runs in.
const BINDING: &str = "__arbaleteTranslate";
const WORLD: &str = "arbalete-translate";

/// Tells a sample of the page's text and its `lang` once it loaded, and keeps
/// `__arbaleteTranslation`, which hands the page's text in batches as it comes near the viewport,
/// puts their translations in place, translates the text added later and shows the originals
/// again. macOS takes a moment for each text, whatever the batch: the text read first is
/// translated first, in small batches, and a text translated once in the page is not handed again.
const TRANSLATE: &str = r#"(() => {
    if (window !== top || window.__arbaleteTranslation) return;
    const tell = (said) => {
        if (typeof __arbaleteTranslate === "function") __arbaleteTranslate(JSON.stringify(said));
    };
    // Code, what the user types and what the page marks so stay as they are.
    const skip = "script, style, noscript, template, code, pre, kbd, samp, var, textarea, select, "
        + "svg, math, [translate='no'], .notranslate, [contenteditable]:not([contenteditable='false'])";
    // Each translated node's text before, and the translation put in its place.
    const originals = new Map();
    const shown = new WeakMap();
    // Each text's translation, the texts of each batch handed, and the nodes waiting for each.
    const cache = new Map(), batches = new Map(), waiting = new Map();
    // The elements watched until they come near the viewport, and those that came.
    let watched = new WeakSet(), near = new WeakSet();
    let next = 0, on = false, queued = [], timer = 0;
    const wanted = (node) => /\p{L}/u.test(node.nodeValue) && !node.parentElement?.closest(skip);
    const later = () => { if (!timer) timer = setTimeout(send, 50); };
    const visible = new IntersectionObserver((entries) => {
        for (const entry of entries) {
            if (!entry.isIntersecting) continue;
            visible.unobserve(entry.target);
            near.add(entry.target);
            for (const node of entry.target.childNodes) {
                if (node.nodeType === Node.TEXT_NODE && wanted(node)) queued.push(node);
            }
        }
        later();
    }, { rootMargin: "50% 0px" });
    // A text node is handed once its element comes near the viewport, at once if it came.
    const add = (node) => {
        const parent = node.parentElement;
        if (near.has(parent)) queued.push(node);
        else if (!watched.has(parent)) {
            watched.add(parent);
            visible.observe(parent);
        }
    };
    const collect = (root) => {
        if (root.nodeType === Node.TEXT_NODE) {
            if (wanted(root)) add(root);
            return;
        }
        if (root.nodeType !== Node.ELEMENT_NODE || root.closest(skip)) return;
        const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
        for (let node; (node = walker.nextNode());) if (wanted(node)) add(node);
    };
    // Puts a node's translation in place, its spaces kept, unless it changed since.
    const put = (node, text, translation) => {
        const current = node.nodeValue;
        if (!node.isConnected || current.trim() !== text) return;
        if (!originals.has(node)) originals.set(node, current);
        const shownText = current.match(/^\s*/)[0] + translation + current.match(/\s*$/)[0];
        shown.set(node, shownText);
        node.nodeValue = shownText;
    };
    const send = () => {
        timer = 0;
        let texts = [], length = 0;
        const flush = () => {
            if (!texts.length) return;
            const batch = next++;
            batches.set(batch, texts);
            tell({ batch, texts });
            texts = []; length = 0;
        };
        for (const node of new Set(queued)) {
            if (!node.isConnected || shown.get(node) === node.nodeValue) continue;
            const text = node.nodeValue.trim();
            if (cache.has(text)) put(node, text, cache.get(text));
            else if (waiting.has(text)) waiting.get(text).push(node);
            else {
                waiting.set(text, [node]);
                texts.push(text);
                length += text.length;
                if (texts.length >= 5 || length >= 1000) flush();
            }
        }
        queued = [];
        flush();
    };
    const observer = new MutationObserver((records) => {
        for (const record of records) {
            if (record.type === "characterData") {
                // The page changed a translated text: its new text is the original.
                if (shown.get(record.target) === record.target.nodeValue) continue;
                originals.delete(record.target);
                collect(record.target);
            } else {
                record.addedNodes.forEach(collect);
            }
        }
        later();
    });
    window.__arbaleteTranslation = {
        start() {
            if (on || !document.body) return;
            on = true;
            collect(document.body);
            observer.observe(document.body, { childList: true, subtree: true, characterData: true });
        },
        apply(batch, translations) {
            const texts = batches.get(batch) ?? [];
            batches.delete(batch);
            texts.forEach((text, i) => {
                const nodes = waiting.get(text) ?? [];
                waiting.delete(text);
                if (!translations[i]) return;
                cache.set(text, translations[i]);
                if (on) for (const node of nodes) put(node, text, translations[i]);
            });
        },
        stop() {
            on = false;
            observer.disconnect();
            visible.disconnect();
            watched = new WeakSet();
            near = new WeakSet();
            clearTimeout(timer);
            timer = 0;
            queued = [];
            batches.clear();
            waiting.clear();
            for (const [node, text] of originals) {
                shown.delete(node);
                node.nodeValue = text;
            }
            originals.clear();
        },
    };
    const sample = () => tell({
        lang: document.documentElement.lang || "",
        sample: (document.title + "\n" + (document.body?.innerText ?? "")).slice(0, 3000),
    });
    if (document.readyState === "complete") sample();
    else addEventListener("load", sample, { once: true });
})()"#;

/// A page's language and its translation.
#[derive(Clone, Debug, Default)]
pub(super) struct Translation {
    /// The execution context of the script's world in the main frame's document.
    context: i64,
    /// The page's language, as the native interface recognizes it, like "de".
    language: String,
    /// Translated, or being.
    translated: bool,
    /// The batches of its text handed to macOS and not translated yet: the address shows the
    /// translation in progress.
    pending: usize,
}

/// Starts watching a tab's page's language, in its documents to come and its current one.
pub(super) fn translate_page_created(browser: &Browser) {
    let Some(host) = browser.host() else { return };
    let binding = json!({ "name": BINDING, "executionContextName": WORLD });
    let script = json!({ "source": TRANSLATE, "worldName": WORLD, "runImmediately": true });
    for (method, params) in [
        ("Runtime.enable", json!({})),
        ("Runtime.addBinding", binding),
        ("Page.enable", json!({})),
        ("Page.addScriptToEvaluateOnNewDocument", script),
    ] {
        let message = json!({ "id": unanswered_id(), "method": method, "params": params });
        host.send_dev_tools_message(Some(message.to_string().as_bytes()));
    }
}

/// What the main frame's script tells.
#[derive(Deserialize)]
#[serde(untagged)]
enum Said {
    /// The page's `lang` and a sample of its text.
    Language { lang: String, sample: String },
    /// A batch of the page's text to translate.
    Batch { batch: u32, texts: Vec<String> },
}

/// Learns from a message of a page's DevTools session its language, which it offers to
/// translate, or translates as its choices say, or its text to translate; forgets the language
/// as its main frame goes to another document.
pub fn page_translation(browser: &Browser, message: &serde_json::Value) {
    if !message["sessionId"].is_null() {
        return;
    }
    let params = &message["params"];
    match message["method"].as_str().unwrap_or_default() {
        "Runtime.bindingCalled" if params["name"] == BINDING => {
            let said = params["payload"]
                .as_str()
                .and_then(|payload| serde_json::from_str(payload).ok());
            let context = params["executionContextId"].as_i64().unwrap_or_default();
            match said {
                Some(Said::Language { lang, sample }) => {
                    language_told(browser, context, &lang, &sample)
                }
                Some(Said::Batch { batch, texts }) => batch_told(browser, batch, &texts),
                None => {}
            }
        }
        "Page.frameNavigated" if params["frame"]["parentId"].is_null() => {
            let gone = with_translation(browser, |translation| translation.take());
            // The batches of the document gone are not translated.
            crate::native::translate_stop(browser.identifier());
            // A page translated goes on translating the pages it leads to in its language.
            KEPT.with_borrow_mut(|kept| {
                let id = browser.identifier();
                kept.retain(|(page, _)| *page != id);
                if let Some(Some(gone)) =
                    gone.filter(|gone| gone.as_ref().is_some_and(|g| g.translated))
                {
                    kept.push((id, gone.language));
                }
            });
            send_state();
        }
        _ => {}
    }
}

thread_local! {
    /// The pages that were translated as their main frame left its document, by browser, with the
    /// language translated: the next document in the same language is translated too.
    static KEPT: RefCell<Vec<(i32, String)>> = const { RefCell::new(Vec::new()) };
}

/// Runs `change` on the translation of the page of `browser`, if it is a tab's.
fn with_translation<T>(
    browser: &Browser,
    change: impl FnOnce(&mut Option<Translation>) -> T,
) -> Option<T> {
    let id = browser.identifier();
    PAGES.with_borrow_mut(|pages| {
        let live = pages
            .values_mut()
            .find(|live| live.browser.identifier() == id)?;
        Some(change(&mut live.translation))
    })
}

/// Keeps the language of a page's document, unless it is one the user reads, and translates it
/// when the user always translates it or translated the page before in it.
fn language_told(browser: &Browser, context: i64, lang: &str, sample: &str) {
    let Some(language) = crate::native::page_language(lang, sample) else {
        return;
    };
    let id = browser.identifier();
    let kept = KEPT.with_borrow_mut(|kept| {
        let before = kept.iter().position(|(page, _)| *page == id);
        before.is_some_and(|before| kept.remove(before).1 == language)
    });
    let url = browser
        .main_frame()
        .map(|frame| CefString::from(&frame.url()).to_string())
        .unwrap_or_default();
    let settings = app_settings();
    let always = settings.translate_languages.get(&language) == Some(&true)
        && !settings
            .never_translate_sites
            .iter()
            .any(|never| never == site(&url));
    let translation = Translation {
        context,
        language,
        translated: false,
        pending: 0,
    };
    if with_translation(browser, |kept| *kept = Some(translation)).is_none() {
        return;
    }
    if always || kept {
        start(browser);
    }
    send_state();
}

/// Has the native interface translate a batch of a page's text from its language.
fn batch_told(browser: &Browser, batch: u32, texts: &[String]) {
    let language = with_translation(browser, |translation| {
        let translation = translation.as_mut().filter(|t| t.translated)?;
        translation.pending += 1;
        Some((translation.language.clone(), translation.pending == 1))
    });
    if let Some(Some((language, first))) = language {
        crate::native::translate(browser.identifier(), batch, &language, texts);
        // The address shows the translation in progress.
        if first {
            send_state();
        }
    }
}

/// The native interface translated a batch of a page's text, or could not: the script puts the
/// translations in place, or the page shows its originals again.
pub fn translated(page: i32, batch: u32, translations: Option<Vec<String>>) {
    let Some(browser) = PAGES.with_borrow(|pages| {
        pages
            .values()
            .find(|live| live.browser.identifier() == page)
            .map(|live| live.browser.clone())
    }) else {
        return;
    };
    let done = with_translation(&browser, |translation| {
        let translation = translation.as_mut()?;
        translation.pending = translation.pending.saturating_sub(1);
        Some(translation.pending == 0)
    });
    match translations {
        Some(translations) => {
            let expression = format!(
                "__arbaleteTranslation.apply({batch}, {})",
                json!(translations)
            );
            evaluate(&browser, &expression);
            if done == Some(Some(true)) {
                send_state();
            }
        }
        None => {
            stop(&browser);
            send_state();
        }
    }
}

/// Runs `expression` in the script's world of the page's main frame.
fn evaluate(browser: &Browser, expression: &str) {
    let context = with_translation(browser, |translation| {
        translation.as_ref().map(|t| t.context)
    });
    let (Some(Some(context)), Some(host)) = (context, browser.host()) else {
        return;
    };
    let params = json!({ "expression": expression, "contextId": context });
    let message = json!({ "id": unanswered_id(), "method": "Runtime.evaluate", "params": params });
    host.send_dev_tools_message(Some(message.to_string().as_bytes()));
}

fn start(browser: &Browser) {
    if with_translation(browser, |t| t.as_mut().map(|t| t.translated = true))
        .flatten()
        .is_some()
    {
        evaluate(browser, "__arbaleteTranslation.start()");
    }
}

fn stop(browser: &Browser) {
    crate::native::translate_stop(browser.identifier());
    evaluate(browser, "__arbaleteTranslation.stop()");
    // The batches waiting are dropped.
    with_translation(browser, |t| {
        t.as_mut().map(|t| {
            t.translated = false;
            t.pending = 0;
        })
    });
}

/// Translates a tab's page, from its popover, or shows its original again.
pub(super) fn translate_tab(tab: NodeId, on: bool) {
    let Some(browser) = PAGES.with_borrow(|pages| pages.get(&tab).map(|live| live.browser.clone()))
    else {
        return;
    };
    if on {
        start(&browser);
    } else {
        stop(&browser);
    }
    send_state();
}

/// What the address shows of a tab's page's language, while it offers translating it or is
/// translated: the language, whether translated, and whether the user always translates it.
pub(super) fn translation_json(tab: NodeId, url: &str) -> serde_json::Value {
    let translation = PAGES.with_borrow(|pages| pages.get(&tab)?.translation.clone());
    let Some(translation) = translation else {
        return serde_json::Value::Null;
    };
    let settings = app_settings();
    let choice = settings
        .translate_languages
        .get(&translation.language)
        .copied();
    let never = choice == Some(false)
        || settings
            .never_translate_sites
            .iter()
            .any(|never| never == site(url));
    if never && !translation.translated {
        return serde_json::Value::Null;
    }
    json!({ "language": translation.language, "translated": translation.translated, "pending": translation.pending > 0, "always": choice == Some(true) })
}

/// Always translates a language, never does, or forgets the choice: the pages in it show so.
pub(super) fn translate_language(language: String, choice: Option<bool>) {
    SETTINGS.with_borrow_mut(|settings| match choice {
        Some(choice) => {
            settings
                .translate_languages
                .insert(language.clone(), choice);
        }
        None => {
            settings.translate_languages.remove(&language);
        }
    });
    save_settings();
    send_settings();
    // A page in the language, shown, is translated at once.
    if choice == Some(true)
        && let Some(browser) = content()
        && with_translation(&browser, |t| {
            t.as_ref()
                .is_some_and(|t| t.language == language && !t.translated)
        }) == Some(true)
    {
        start(&browser);
    }
    send_state();
}

/// Never offers translating a site's pages, or does again: its host, and whether never.
pub(super) fn never_translate_site(site: String, never: bool) {
    SETTINGS.with_borrow_mut(|settings| {
        settings.never_translate_sites.retain(|kept| *kept != site);
        if never {
            settings.never_translate_sites.push(site.clone());
            settings.never_translate_sites.sort();
        }
    });
    save_settings();
    send_settings();
    send_state();
}
