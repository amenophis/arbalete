# 0017 — AppleScript

- Status: accepted
- Date: 2026-11-24

## Context

Like Arc, scripts and automation tools (Shortcuts, Raycast, Alfred) should list and open windows, Spaces and
tabs, read a tab's title and address, and run JavaScript in a tab (issue #320).

Arc's dictionary offers windows with their tabs and Spaces, an active tab and an active Space; tabs with their
title and URL, which can be set, and select, close, back, forward, reload, stop and execute (JavaScript); Spaces
with their title and tabs, and focus. Chrome's is close to it. Scripts written for either use the same terms:
`URL of active tab of front window`, `execute … javascript "…"`.

What macOS and the app offer:

- Cocoa scripting reads a dictionary (`.sdef`) from the bundle and evaluates scripts against Objective-C
  objects through key-value coding, on the main thread. A command may suspend itself and answer later.
- Each window's interface already holds the state it shows (`AppState`, app/macos/Models.swift): its Spaces,
  every Space's tabs, the displayed Space and its selected tab; and it acts through the app's commands
  (app/src/command.rs).
- CEF runs JavaScript in a frame without giving its result; DevTools' `Runtime.evaluate` gives it, and the app
  already sends each page's DevTools session messages.

## Decision

An AppleScript dictionary of our own, `app/resources/mac/Arbalete.sdef`, with the terms scripts written for Arc
and Chrome use, served by Cocoa scripting from the native interface (app/macos/Scripting.swift).

- The application's windows are the main windows, front to back; a window has its tabs (those of the Space it
  shows), its Spaces, its active tab, its active Space, and whether it is incognito. A Space has its title and
  its tabs; a tab its title and URL.
- `select` shows a tab, `focus` a Space, each bringing its window in front; `close` closes a tab or a window;
  setting a tab's URL shows it and loads the address; `make new tab with properties {URL: …}` opens one, in a
  window or a Space; `make new window` opens one.
- The objects read the window's own interface state and act through its commands, like its sidebar: nothing
  new in the app's logic, and what a script sees is what the window shows.
- `execute` runs its script with `Runtime.evaluate` in the tab's page, awaiting a promise, and gives a string as
  itself and another value in JSON. A tab without a live page, unloaded or never shown, fails rather than loading.
- Back, forward, reload, stop and tab locations are left out until asked for: a script can run them in the page,
  and the sidebar's sections have no term of their own yet.

## Consequences

- Any app the user lets control Arbalete (macOS asks once per app) reads every tab's address and runs scripts
  in its pages, as with Arc and Chrome.
- Identifiers last for the run: tabs and Spaces keep theirs across launches, windows do not.
- A script's JavaScript runs with a user gesture, as a click in the page would.
