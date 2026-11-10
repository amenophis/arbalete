# 0011 — Extensions without a helper extension

- Status: accepted
- Date: 2026-11-10

## Context

Decision 0001 (question 2) answered how Arbalete learns the tab id extensions know a page by: a small helper
extension, loaded with `--load-extension` from the bundle, holding the `debugger` permission, whose
`chrome.debugger.getTargets()` ties each page's DevTools target to its tab id. The helper then took on more, with
the `management` permission: turning extensions on or off from the library, listing them as Chromium has them
before it writes its preferences, and removing a turned-off one (Chromium confirming it, as one extension
removes another).

The helper is an extension like the user's: it shows on `arb://extensions` as "Arbalete", with a switch and
a Remove button, and to every extension holding `management`. Turned off or removed, extensions lose the active
tab's id, the library's switches do nothing, and extensions cannot be removed while turned off.

What Chromium and CEF offer without it:

- CEF has no API for any of it.
- Chromium's `Extensions` DevTools domain lists, removes and loads extensions, but only from the browser target,
  which the app's session cannot attach to (0001, question 1), and only unpacked ones. It has no switch.
- `Target.getTargetInfo` has no tab id.
- A component extension (location 5) would be hidden and locked, but the switch that loaded one from a folder is
  gone from Chromium.

## Decision

No helper extension: our CEF build (amenophis/cef-builds, `extensions-over-devtools.patch`) extends Chromium's
`Extensions` domain, and the app calls it from the DevTools session it already holds.

- From a trusted client's session of any target, the embedder's (never an extension's `chrome.debugger`, which
  Chromium treats as untrusted):
  - `getExtensions` with a `browserContextId`: every extension the user sees in that profile, turned on or not;
  - `uninstall` with a `browserContextId`: removes one of them, without Chromium's prompt;
  - `setEnabled`: turns one on or off;
  - `getTabId`: a page target's tab id.
- Each checks Chromium's management policy, like the extensions page. The browser target's own `getExtensions` and
  `uninstall` stay as they are.
- The app sends them in the bridge's session, with the profile's browser context, which a page of the profile
  reports. Each page's tab id is asked once, as its target is known.
- Removing an extension always asks the user in Arbalete's own dialog, turned on or not: the hidden page in which a
  turned-on extension removed itself and Chromium's window for a turned-off one both go.
- The `--load-extension` switch, the bundle's `arbalete-helper` folder and `app/extension` go. Profiles keep a
  stale entry for it in their preferences, which Chromium never loads again without the switch.

## Consequences

- Nothing a user can turn off or remove keeps extensions working; nothing extra runs in every profile.
- The app needs a CEF build with the patch: cef-codecs build 7 or later.
- The patch extends an experimental DevTools domain: each CEF update must carry it forward, like the others.
- As before, a profile's extensions are managed once one of its pages has told its browser context.
