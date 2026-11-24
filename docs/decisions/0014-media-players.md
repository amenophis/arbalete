# 0014 — Media players in the sidebar

- Status: accepted
- Date: 2026-11-23

## Context

A tab left while its media plays, or while it holds a call, should show a player in the sidebar (issue #326):
what plays, with its artwork, play or pause, the previous and next tracks, mute, a way back to the tab, and for a
call, its microphone, its camera and leaving it.

What CEF and Chromium offer:

- CEF tells nothing of a page's sound or its Media Session: Chromium's audible state, its `MediaSession` and the
  controls it drives for the system's Now Playing stay inside. The app already watches each frame's media
  elements through DevTools for the tab's speaker (`app/src/shell/sound.rs`).
- CEF tells whether a page uses the camera or the microphone (`on_media_access_change`), not whether either is
  muted, and cannot mute a capture.
- No DevTools domain plays, pauses or reads a page's Media Session.
- The Media Session API is the page's own: its `metadata` says what plays, its `play` and `pause` handlers play
  and pause however the page plays, its `previoustrack` and `nexttrack` handlers go through its playlist, and a
  call's `togglemicrophone`, `togglecamera` and `hangup` handlers, with `setMicrophoneActive` and
  `setCameraActive`, are what Chrome's own call controls use. Google Meet offers them.

## Decision

The players work through the page's own APIs, from the DevTools session the app already holds.

- The sound's watch, in its isolated world, also tells the frame's Media Session `metadata`: title, artist and
  its largest artwork. A frame says nothing until it first plays sound. A script in the page's world hands it
  each media element the page plays without adding it to its document, like Spotify's player, whose events
  reach neither the document nor the window.
- A script in the page's world keeps the handlers the page gives its Media Session's actions, and tells the app,
  through a binding, which track and call actions it offers and whether its microphone and camera are on.
- Playing and pausing run the page's handler, or without one its media elements, in each frame that played;
  the previous and next track buttons, shown only for a page that offers them, and the call's buttons run the
  page's handlers. The app's click stands for the user's gesture.
- A tab shows a player while its page is hidden and holds a call, or played sound since it was hidden; paused, it
  keeps its player until it shows again or the player is dismissed. Media paused before the tab was left shows
  none.
- Muting is the tab's own mute; going to the tab selects it.

## Consequences

- No CEF patch: the players work with any site that plays through media elements, and a call's buttons show only
  for a page that offers the Media Session's call actions, like Google Meet.
- A page playing only through Web Audio, without media elements, shows no player, like it shows no speaker.
- The page's world sees the binding and the kept actions, as it saw the kept `enterpictureinpicture` handler
  before (Picture in Picture now reads the same kept actions).
- A page frozen after being left paused runs again as its player plays it.
