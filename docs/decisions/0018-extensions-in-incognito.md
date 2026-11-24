# 0018 — Extensions in incognito windows

- Status: accepted
- Date: 2026-11-24

## Context

Extensions don't run in Arbalete's incognito windows (issue #211). Chrome runs one there once the user switches
on "Allow in Incognito" on its page (off by default). A password manager like Bitwarden is the main reason to.

An incognito window has a profile of its own (`Sidebar::open_incognito`), whose request context has no cache
path: CEF then makes it an off-the-record profile (`ChromeBrowserContext`), and Chromium runs in it the
extensions of the profile it is made of that the user allowed in incognito (checked in a test copy:
`chrome.extension.inIncognitoContext` is true in its content script). But CEF always makes it of its primary
profile, `Default`, which is only the app's global context: the user's extensions are in `Profile <id>`, so none
reach an incognito window. CEF has no API to choose the profile.

Chromium's own page of an extension (`arb://extensions/?id=…`, the extension menu's Manage Extension) already
has the "Allow in Incognito" switch, and it writes the setting to the profile's preferences
(`extensions.settings.<id>.incognito`).

## Decision

- **Of the profile it was opened from**, like Chrome's: an incognito window opened from a Space is made of that
  Space's profile. Our CEF build (amenophis/cef-builds, `incognito-of-profile.patch`) makes a request context
  whose cache path is `<profile>/Incognito <id>` a new off-the-record profile of the profile in `<profile>`,
  once that one is loaded. That folder is the incognito window's directory, which only the app writes in and
  which goes with the window, as its temporary folder did. The app opens the profile's web data first.
- **The switch stays Chromium's**, on the extension's page: no switch of Arbalete's, nor a DevTools method to
  set it. The app reads it from the preferences with the rest.
- **In the window**: the toolbar, the site controls and the extension menus of an incognito window list its
  profile's extensions allowed in incognito, and act on them in that profile. Their popups run in that profile,
  as Chrome runs a spanning extension's.
- **What the extensions see**: the incognito window's selected tab is their profile's active tab
  (`incognito: true`), only for those allowed in incognito. For the others, as in Chrome, none of their windows
  has the focus.

## Consequences

- Extensions work in incognito windows as in Chrome, off by default for each one.
- The app needs a CEF build with the patch: cef-codecs build 12 or later. On another CEF, CEF refuses that cache
  path and makes the profile of `Default` as before: no extension runs there.
- An incognito window shares the service workers and storage of its profile's extensions allowed there, as in
  Chrome. Its web data still stays in memory and goes with the window.
- Each CEF update must carry the patch forward, like the others.
- A split-mode extension (`"incognito": "split"`) runs a second copy in the incognito profile, which the shim
  doesn't reach: it sees no tab there.
