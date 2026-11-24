# 0016 — Translating pages on the device

- Status: accepted
- Date: 2026-11-24

## Context

A page written in a language the user does not read should offer to translate it, and remember the choice per
language or site (issue #327).

How Arc does it (static analysis of Arc 1.173.1, behavior only): it runs Chromium's Translate, which sends the
page's text to Google's servers. Once Chromium detects a page's language, the address shows a translation button
and Arc opens a bubble on its own: "Translate this page to …?", then "Translated from ….", with Use Original and
Options, among them never translating the site, kept per host.

What this engine offers:

- Chromium's Translate is a Chrome layer feature (its `TranslateManager`, language detection and bubble): Alloy
  style has none of it, and it would send the text to Google.
- Chromium's on-device Translator API (`Translator` in pages) needs the Chrome layer's component updater to fetch
  its models: not in CEF Alloy.
- macOS translates on the device: the Translation framework's `TranslationSession` translates batches of strings
  between installed languages, with no network, and can be made directly since macOS 26. Before, a session only
  comes from a SwiftUI view's `translationTask`. Languages are downloaded once, by the system (its sheet, or
  System Settings › General › Language & Region › Translation Languages). The NaturalLanguage framework
  recognizes a text's language.
- The app already holds a DevTools session on each page, whose scripts run in worlds of their own and talk
  through bindings (decision 0014).

## Decision

Pages are translated by macOS, on the device, from macOS 26.

- A script in a world of its own, in the page's main frame, tells the app a sample of the page's text and its
  `lang` once its document is parsed. The NaturalLanguage framework recognizes the language, `lang` deciding when
  the text does not. A page in one of the user's preferred languages (System Settings' list) offers nothing.
- Like Arc's, a button by the address shows on a page in another language; it opens a popover: Translate, or
  Show Original once translated, always translating the language, never translating the language or the site.
  It does not open on its own: the button is the offer.
- Translating, the script hands the app the page's text nodes, in batches, skipping code, editable text and
  what the page marks `translate="no"`; macOS translates them to the user's first preferred language, and the
  script puts them in place, keeping the originals to show again. Text the page adds later is translated too.
  A language not downloaded yet is downloaded through the system's sheet, from the popover.
- The choices are the app's settings: a language always or never translated, a site never translated. A site
  never translated offers nothing; a language always translated is translated as its pages load. The settings'
  Advanced pane lists them, to forget them.

## Consequences

- No CEF patch and no text leaves the device. Translation is macOS's: its languages, and its quality.
- Before macOS 26 no page offers translation.
- Text is translated node by node: a sentence split by markup, like a word in bold or a link, is translated in
  pieces.
- Only the main frame is translated, its text nodes only: frames, attributes like `placeholder` or `alt`, and
  the title stay as they are.
- A page that rewrites its text nodes may undo a translation, or be confused by one, as with any page
  translator.
