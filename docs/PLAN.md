# Arbalette — implementation plan

Goal: a browser that brings back the ideas that made Arc popular (vertical sidebar, Spaces, profiles, link
routing…), **cross-platform (macOS, Linux, Windows)**, starting with macOS.
Stack: **Rust + React/TypeScript on Chromium, through CEF in Alloy style** (see
[decision 0001](decisions/0001-web-engine.md)).

Arbalette is an independent project, not affiliated with The Browser Company. Arc is mentioned only to describe
the features we take inspiration from and to import user data.

## 1. Features we take inspiration from

- **Vertical sidebar**: Favorites (icon grid) at the top, pinned tabs, then today's tabs. Tabs and folders form a
  tree. A pinned tab remembers its original URL and can be reset to it. Closing a tab returns to the tab that
  opened it.
- **Spaces**: separate sets of pinned and unpinned tabs, each with its own name, icon and color theme.
- **Profiles**: isolated web data (cookies, history, logins). **Several Spaces can share one profile**, and
  Favorites belong to the profile, so Spaces of the same profile share them.
- **Auto-archive**: unpinned tabs inactive for longer than a threshold (12 hours by default; 1 h, 6 h, 24 h, 7 d,
  30 d or never) move to a browsable archive and can be restored. The threshold is set per profile.
- **Link routing**: rules decide where links opened from other apps land. A rule is a set of URL conditions
  ("matches" or "contains") and a destination Space; a default destination applies when no rule matches.
- **Command Bar**: one field for URLs, search, open tabs, history and actions.
- **Resource saving**: tabs in the background are frozen, then unloaded, and reload when selected. The sidebar
  shows their state. Sites can be excluded, and the browser saves more when the computer is on low power.
- **Split View** and **Peek** (preview a link above the current page).
- **Incognito windows** (⌘⇧N): a separate dark window whose browsing leaves nothing behind: no history, no
  session, cookies and cache kept in memory only.
- **Chrome extensions**: installed from the Chrome Web Store, like in any Chromium browser. Password managers and
  content blockers are essential for users coming from Arc.

## 2. Technical choices

### Rust + React/TypeScript

- **Rust**: all the logic — model, persistence, profiles, auto-archive, link routing, history, imports,
  extensions, content view management.
- **React/TypeScript**: sidebar, Command Bar, settings. It displays state and sends commands to Rust.
  Shared types generated from Rust (`specta`).
- **Tabs**: each tab is a content view of the window, placed to the right of the UI view.

### The web engine

Decided in [0001 — Web engine](decisions/0001-web-engine.md), after a spike on macOS:

- **Chromium through CEF**, driven from Rust with the `cef` crate. Only Chromium runs Chrome extensions, and CEF
  gives every OS the same engine.
- **Alloy-style browsers in CEF Views windows**: one window holds the sidebar, the tabs, Split View and overlays.
  CEF's Chrome style supports extensions fully but allows only one page per window.
- **An extension compatibility layer** fills what Alloy style lacks: Arbalette hosts extension popups and answers
  `tabs.query`, `windows.*`, `tabs.create` and `tabs.update` through a shim injected with the DevTools protocol.
- **Fallback**: a maintained CEF patch allowing several Chrome-style pages per window.

All engine code lives in a single Rust module, `engine` (tabs, navigation, events, extensions). The rest of the code
never touches CEF directly. Following the doctrine, this module becomes a trait only if a second engine exists.

### UI rules (performance and native feel)

The UI is HTML/TS, but it must behave like a native UI. These rules apply from phase 0:

- **Rust is the source of truth.** The UI only keeps a copy of the state and sends commands.
- **Send changes, not the full state.** Rust emits targeted events (tab added, title changed, node moved…). The full
  state is loaded only at startup and when switching Spaces.
- **Fine-grained store** (Zustand or Jotai, with selectors): changing a tab re-renders only its row. Memoized row
  components, virtualized long lists.
- **Optimistic updates** for instant actions (selection, drag and drop, renaming): the UI reacts immediately, Rust
  confirms or corrects.
- **Keyboard shortcuts through the app's native menus** (`⌘T`, `⌘L`, `⌘S`, `⌘W`…), never in JavaScript alone:
  they must work even when the web page has focus.
- **Native where it shows**: app menus and native context menus, window translucency (macOS vibrancy, Windows
  Mica), macOS window buttons, cursors, native scrolling.
- **No title bar, like Arc**: a frameless window (`IsFrameless`) that keeps the macOS window buttons
  (`WithStandardWindowButtons`) at the top of the sidebar (`GetTitlebarHeight`); the window is dragged by the
  sidebar's empty areas, declared with the `-webkit-app-region: drag` CSS property.
- **Overlays above the page** (Command Bar, Peek, rich menus): overlay views of the window
  (`CefOverlayController`), designed in phase 0.
- **Animate only `transform` and `opacity`**, so animations stay smooth.
- **No heavy computation in the UI**: search, ranking and sorting happen in Rust.

### Resource saving

Arc manages each tab's page through four states — alive, frozen, discarded, closed — with a per-tab timer, a
reaction to memory pressure and to macOS Low Power Mode, a list of URL patterns that are never suspended, and
an optional state indicator in the sidebar. Arbalette follows the same model, implemented in the `engine` module:

| State | What it means | How (CEF) |
|---|---|---|
| **Active** | Visible tab | Normal browser |
| **Hidden** | Loaded but not visible | View hidden; Chromium throttles timers and rendering on its own |
| **Frozen** | Loaded, JavaScript and timers paused, memory kept | DevTools protocol `Page.setWebLifecycleState("frozen")`, sent by the app |
| **Unloaded** | Browser closed; the tab keeps its URL, title, favicon and a snapshot | Close the browser; recreate it when the tab is selected |

Rules:
- **Lazy loading**: on launch, restored tabs are unloaded; a tab loads only when selected.
- **Timers**: a hidden tab is frozen after a few minutes, then unloaded after a longer delay.
- **Memory pressure**: when the OS reports pressure, the least recently used tabs are unloaded first.
- **Low power** (macOS Low Power Mode, Windows energy saver, Linux power profile): shorter delays.
- **Never suspended**: tabs playing audio, using the camera, microphone or screen sharing, the tabs of an open
  Split View, and sites matching the user's exception patterns.
- **Settings**: off, balanced (default) or maximum savings; exception list. A discreet indicator in the sidebar
  marks unloaded tabs.

This is separate from auto-archive: an unloaded tab stays in the sidebar; an archived tab leaves it.

### Repository layout

Start with the minimum and split when the code calls for it (doctrine, rule 10):

```text
app/                           desktop app: windows, commands, engine module
crates/arbalette-core/         model (profiles, Spaces, node tree, Favorites) and pure logic, no engine
ui/                            React + TypeScript (Vite)
```

`arbalette-core` is separate from the start for one reason only: the logic must be testable without launching
the app or the engine. Persistence, auto-archive, link routing and imports start as modules of these two crates; a module becomes
a crate only if it grows enough to hurt readability or build times.

Tooling: pnpm, Vite, Vitest + Testing Library, `cargo test` / `clippy` / `rustfmt`, `cargo-deny` for dependency
licenses and advisories, GitHub Actions CI (macOS first, then Linux and Windows), installers (dmg, AppImage/deb, msi),
macOS signing and notarization.

## 3. Arbalette data model (Rust)

```rust
struct Profile    { id: ProfileId, name: String, color: Color, data_store: Uuid }  // isolated web data
struct Space      { id: SpaceId, title: String, icon: Icon, theme: Theme, profile: ProfileId,
                    pinned_root: NodeId, today_root: NodeId }                     // N Spaces -> 1 profile
struct Node       { id: NodeId, parent: Option<NodeId>, children: Vec<NodeId>, title: Option<String>,
                    created_at: Timestamp, kind: NodeKind }
enum   NodeKind   { Tab(Tab), Folder { expanded: bool }, Split(Split), Container }
struct Tab        { url: Url, saved_url: Option<Url>, saved_title: Option<String>, favicon: Option<Blob>,
                    opener: Option<NodeId>, last_active_at: Timestamp, is_unread: bool }
struct Sidebar    { profiles: Vec<Profile>, spaces: HashMap<SpaceId, Space>, space_order: Vec<SpaceId>,
                    nodes: HashMap<NodeId, Node>, favorites: HashMap<ProfileId, NodeId> }
struct ArchivedTab { node: Node, reason: ArchiveReason, archived_at: Timestamp, space: SpaceId }
struct RoutingRule { id: RuleId, enabled: bool, conditions: Vec<Condition>, destination: Destination }
enum   Condition   { UrlMatches(Pattern), UrlContains(String) }
enum   Destination { Space(SpaceId), MostRecentSpaceOf(ProfileId), MostRecentSpace }
```

This is a target, not a starting point: each type is added when a phase needs it.
History and imported bookmarks: SQLite **per profile**, like web data.

## 4. Imports

Import formats are documented here only as far as needed for interoperability: reading the user's own data
from the browsers they use.

### From Arc

Arc stores its data in `~/Library/Application Support/Arc/` on macOS (in the Arc app's `Packages` directory on
Windows):

| Data | Source | Conversion |
|---|---|---|
| Sidebar: Spaces, Favorites, pinned and today's tabs, folders | `StorableSidebar.json`, `sidebar.containers`: the entry with `items`, `spaces` and `topAppsContainerIDs`, each a flat array alternating keys (ids) and values. Items are linked by `parentID` / `childrenIds` and hold one kind in `data`: `tab` (`savedURL`, `savedTitle`; the item's `title` when renamed), `list` (a folder), `splitView` (its tabs as children), `itemContainer` (a root). A Space's `containerIDs` alternates `pinned` / `unpinned` and their root; its `customInfo` holds `iconType` (`emoji_v2`, or a named `icon`) and `windowTheme`. `topAppsContainerIDs` alternates a profile and its Favorites root. `spaces` is read in its own order; `sidebarSyncState` and `firebaseSyncState` are sync copies | Profiles, Spaces and node tree; split views become separate tabs at first |
| Profiles | Each Space's `profile` is `{"default": true}` or `{"custom": {"_0": {"directoryBasename": …}}}`; names in `User Data/Local State` (`profile.info_cache.<directory>.name`), no colors there | One Arbalette profile per Arc profile |
| Archive | `StorableArchive.json` | Arbalette archive |
| Link routing rules | `StorableLinkRouting.json` | Rules; destinations that do not exist in Arbalette fall back to the profile's most recent Space |
| History, bookmarks | `User Data/<directoryBasename>/History`, `Bookmarks` | Same as any Chromium browser (below) |

The sidebar format was checked against a real Arc file; custom profiles, Space titles, the order of several Spaces
and `windowTheme` (`primaryColorPalette.midTone`, components from 0 to 1) were absent from it and remain
to be checked.

### From system browsers

| Family | Browsers | Profiles | Bookmarks | History |
|---|---|---|---|---|
| Chromium | Chrome, Edge, Brave, Vivaldi, Opera, Dia, Arc | `Local State` (`profile.info_cache`) | `Bookmarks` (JSON) | `History` (SQLite, µs since 1601) |
| Firefox | Firefox, Zen, LibreWolf | `profiles.ini` | `places.sqlite` (`moz_bookmarks`) | `places.sqlite` (`moz_places`, `moz_historyvisits`) |
| Safari | Safari (macOS) | — | `~/Library/Safari/Bookmarks.plist` | `~/Library/Safari/History.db` (dates since 2001) |

Rules:
- Automatic detection of installed browsers and profiles, with OS-specific paths.
- SQLite databases are **copied before reading**: they are locked while the browser is open.
- Target choice: each source profile becomes an Arbalette profile or is merged into an existing one.
  The bookmarks bar can become the Favorites or a pinned folder.
- **Safari** requires **Full Disk Access** for Arbalette (macOS blocks even reading `~/Library/Safari` without it):
  the import assistant explains this and opens the right System Settings pane.
- **Out of scope at first**: passwords and cookies. They are encrypted with the system keychain and are sensitive
  data; they may come later, for example through the browsers' CSV export.

## 5. Phased plan

Each phase ends with a usable, tested app. Development and acceptance testing happen **on macOS**;
CI runs on macOS; Linux and Windows join it in phase 10.

### Phase 0 — Foundations

#### Engine spike — done

Results and decision in [0001 — Web engine](decisions/0001-web-engine.md). Its open questions are answered before
phase 4, which builds the extension compatibility layer.

#### Foundations
- Project scaffold (CEF through the `cef` crate, React/TypeScript UI, Cargo workspace), CI on macOS.
- One CEF Views window holding the sidebar and the content views (Alloy style), with an overlay view above the
  page.
- `engine` module: navigation (back, forward, reload), progress, title, URL, favicon.
- JSON persistence, session restore with lazy loading (a restored tab loads only when selected).
- **Performance prototype** (below), to be validated before starting phase 1.

#### Performance prototype

Goal: check that an HTML/TS UI matches native UI performance before investing in features. Measured on macOS,
in a production build, on the development machine.

Scenario: a sidebar with 500 tabs spread across folders in 5 Spaces, 10 content views loaded with real sites,
and the Rust ↔ UI bridge with its change events.

| Measure | Target |
|---|---|
| Switch tab: from click to display of an already loaded page | < 50 ms; UI part < 16 ms |
| Switch Space (500 tabs) | < 100 ms, no jank |
| Drag and drop in the sidebar | 60 fps, no dropped frame |
| Open the Command Bar (shortcut → field ready) | < 50 ms |
| Keystroke in the Command Bar → results displayed | next frame (< 16 ms) over 10,000 history entries |
| Tab title update (Rust → UI) | < 5 ms, without re-rendering other rows |
| Cold start to a usable window | < 1 s |
| UI memory (excluding web pages) | < 150 MB |
| Shortcuts | work while the web page has focus |
| Overlay | the Command Bar displays above the page |
| Page scrolling | as smooth as Chrome on the same page |

Tools: Web Inspector / DevTools (Performance), Rust-side timestamps on commands and events, Instruments on macOS.
Measurements are reproducible with a script and generated data.

Results: [docs/perf/phase-0.md](perf/phase-0.md).

If targets are missed: optimize first. If the gap remains large, reconsider the UI (cross-platform Rust UI such as
`gpui`, or native per OS) before phase 1. Only the UI would need rewriting, since the logic is in Rust.

#### Commit sequence

Rules in [CONTRIBUTING.md](../CONTRIBUTING.md#commits). Every commit builds and passes the checks.

| # | Branch | Commit |
|---|---|---|
| 1 | `main` | `docs: initialize repository with license, readme and project plan` |
| 2 | `docs/engine-decision` | `docs: record the web engine decision` |
| 3 | `build/scaffold` | `build(app): scaffold the app with a react and typescript ui` |
| 4 | | `build: add cargo workspace with arbalette-core crate` |
| 5 | | `build(ui): add lint, format and test tooling` |
| 6 | | `ci: build and test on macos` |
| 7 | | `ci: check dependency licenses and advisories with cargo-deny` |
| 8 | `feat/sidebar-model` | `feat(core): add sidebar model with profiles, spaces and tab tree` |
| 9 | | `feat(core): persist sidebar state as json` |
| 10 | `feat/content-view` | `feat(app): show sidebar and content views side by side` |
| 11 | | `feat(engine): navigate back, forward and reload` |
| 12 | | `feat(engine): report title, url, favicon and loading progress` |
| 13 | | `feat(ui): render tabs and select the active one` |
| 14 | | `feat(app): restore the previous session on launch` |
| 15 | `feat/native-shell` | `feat(app): use a frameless window with its buttons in the sidebar` |
| | | `feat(app): handle keyboard shortcuts through native menus` |
| 16 | | `feat(ui): show an overlay above the page` |
| 17 | `perf/prototype` | `test(perf): generate a 500-tab benchmark scenario` |
| 18 | | `test(perf): measure startup, tab switching and overlay latency` |
| 19 | | `docs(perf): record phase 0 measurements` |

The engine spike lives on the throwaway branch `spike/cef-shell` and is not part of this sequence.

Each following phase starts the same way: define its commit sequence before writing code.

### Phase 1 — Vertical sidebar
- Favorites (icon grid), pinned tabs, divider, today's tabs.
- Drag and drop between sections, nested folders, renaming, pin / unpin.
- Pinned tab: "go back to the saved URL". Closing a tab returns to the tab that opened it.
- Hide the sidebar, with reveal on hover at the left edge.
- Shortcuts (`⌘` on macOS, `Ctrl` elsewhere): close, reopen, move between tabs, `1…9` for Favorites.

Commit sequence (the phase 0 measurements put the state update first, before the sidebar grows):

| # | Branch | Commit |
|---|---|---|
| 20 | `feat/close-tabs` | `feat(ui): close tabs from the sidebar and with ⌘W` |
| 21 | | `feat(core): return to the opener tab when closing a tab` |
| 22 | | `feat(app): reopen the last closed tab with ⌘⇧T` |
| 23 | `perf/state-updates` | `perf(ui): re-render only the tab rows that change` |
| 24 | `feat/sidebar-sections` | `feat(ui): show pinned tabs and folders above today's tabs` |
| 25 | | `feat(core): add favorites to each profile` |
| 26 | | `feat(ui): show favorites as an icon grid` |
| 27 | | `feat(app): pin and unpin tabs, pinned tabs go back to their saved url` |
| 28 | `feat/sidebar-editing` | `feat(ui): create, rename and collapse folders` |
| 29 | | `feat(ui): drag and drop tabs and folders between sections` |
| 30 | `feat/sidebar-shortcuts` | `feat(app): move between tabs and open favorites with ⌘1…9` |
| 31 | | `feat(ui): hide the sidebar and reveal it at the left edge` |

### Phase 2 — Profiles and Spaces
- Profiles: creation, name, color; isolated web data (one data store per profile).
- Spaces: creation / editing (name, emoji, color or gradient), attached to a profile, several Spaces per profile.
- Switching Spaces: switcher at the bottom of the sidebar, horizontal swipe, shortcuts.
- Favorites follow the profile of the displayed Space.

Commit sequence:

| # | Branch | Commit |
|---|---|---|
| 32 | `feat/profiles` | `feat(app): give each profile its own web data` |
| 33 | | `feat(ui): create, rename and color profiles` |
| 34 | `feat/spaces` | `feat(ui): show one Space at a time with a switcher at the bottom` |
| 35 | | `feat(ui): create and edit Spaces (name, emoji, color)` |
| 36 | | `feat(app): switch Spaces with ⌃1…9 and a horizontal swipe` |
| 37 | | `feat(ui): show the favorites of the displayed Space's profile` |

### Phase 3 — Imports
- Import assistant on first launch and from the menu (there are no Settings yet).
- Chromium browsers, Firefox, Safari: profiles, bookmarks, history.
- Arc: profiles, Spaces, Favorites, pinned tabs, folders. Its archive and link routing rules are imported in
  phases 6 and 8, once Arbalette has an archive and rules to put them in.

Commit sequence (Arc first, the browser Arbalette users are most likely to come from):

| # | Branch | Commit |
|---|---|---|
| 38 | `feat/import-arc` | `feat(import): read the Arc sidebar into profiles, Spaces and tabs` |
| 39 | | `feat(app): import Arc on first launch` |
| 40 | `feat/import-bookmarks` | `feat(import): detect installed browsers and their profiles` |
| 41 | | `feat(import): import Chromium bookmarks` |
| 42 | | `feat(import): import Firefox bookmarks` |
| 43 | | `feat(import): import Safari bookmarks` |
| 44 | `feat/history` | `feat(core): record history per profile` |
| 45 | | `feat(import): import Chromium, Firefox and Safari history` |
| 46 | `feat/import-assistant` | `feat(ui): import assistant on first launch and from the menu` |

### Phase 4 — Chrome extensions
- First, answer the open questions of [decision 0001](decisions/0001-web-engine.md).
- Compatibility layer: popups hosted by Arbalette; shim in popups and service workers for `tabs.query`, `windows.*`,
  `tabs.create` and `tabs.update`; extension actions shown in the sidebar.
- Install from the Chrome Web Store and from a local folder (developer mode), update, enable, disable, remove.
- Extensions are installed **per profile**, like their data.
- Extension actions in the sidebar (pinned extensions, then an overflow menu), options pages, permission prompts.
- Acceptance list: uBlock Origin Lite, Bitwarden, 1Password, Dark Reader, plus the most used extensions found in
  imported Chromium profiles.

### Phase 5 — Command Bar
- Centered palette for "new tab" and "edit URL".
- Results: open tabs (all Spaces), Favorites and pinned tabs, history (including imported), search engine
  suggestions, actions.
- Ranking: recency, frequency, typed count, exact, domain or title-prefix match.

### Phase 6 — Auto-archive
- Thresholds: 1 h, 6 h, 12 h (default), 24 h, 7 d, 30 d, never; per-profile setting.
- Browsable, restorable archive (full node kept).
- Import Arc's archive.

### Phase 7 — Resource saving
- Freeze, then unload hidden tabs on timers; reload on selection from the saved URL, with the snapshot shown
  while loading.
- Memory pressure and low-power detection on each OS.
- Never-suspended cases (media, capture, Split View, exception patterns), settings and sidebar indicator.
- Acceptance: with 100 tabs open, memory used after unloading compared with all tabs alive, measured and recorded.

### Phase 8 — Link routing
- Registration as the default browser on each OS.
- Receiving external URLs (URL open event on macOS; single instance + command-line arguments elsewhere).
- Rules: "matches" / "contains", several conditions, per-rule toggle, default destination
  (most recent Space, optionally of a given profile).
- Import Arc's link routing rules.

### Phase 9 — Polish
- Split View, Peek. The sidebar already makes split rows, a tab dropped on the middle of another:
  until Split View shows both tabs side by side, a click on either half shows that tab alone.
- Per-OS finish: macOS vibrancy, Windows Mica, Wayland/X11.

### Phase 10 — Linux and Windows
- CI on Linux and Windows: system libraries CEF needs, loading `libcef` in tests, Windows sandbox bootstrap.
- Full acceptance testing on each OS, signed installers, automatic updates.

### Phase 11 — Developer mode
- A view for working on a site under development, like Arc's developer mode: to be studied (behavior and
  layout) when the phase starts.

### Phase 12 — Incognito
- ⌘⇧N (and File › New Incognito Window) opens a separate window on an off-the-record request context: cookies,
  cache and storage in memory only, discarded when the window closes; nothing written to history, the session or
  the sidebar state.
- A minimal sidebar like Arc's: the address field, an "Incognito" label, "New Tab" and today's tabs only (no
  Favorites, pinned tabs, Spaces or profiles); a dark theme that sets it apart.
- Extensions disabled unless allowed in incognito; downloads still saved.

### Phase 13 — Arc feature audit
- Use Arc as a user, systematically, to find the features this plan misses: every menu and submenu, keyboard
  shortcut, context menu, setting and empty state, with edge cases (many tabs, long titles, drag and drop
  everywhere, several windows and profiles).
- Start from an empty Arc (a fresh user account or a cleared Arc data folder, never the user's own) to audit the
  onboarding: first launch, sign-in or skip, import offer, theme and Space setup, default Favorites, tips and
  every screen until the first browsing session. Arbalette's first launch is then designed from it.
- Record each finding with what Arc does and whether Arbalette has it; turn the missing ones into plan entries,
  ranked by how much users coming from Arc rely on them.
- The audit is also the design reference, the goal being pixel perfect: for every screen, state and motion,
  record measurements (sizes, spacing, radii, colors, type, timings and easing) from screenshots and screen
  recordings, compare Arbalette side by side at the same window size, and fix every difference. Arbalette draws
  its own icons and copies no asset.

### Later
Sync, password import.

## 6. Open points

1. **Extensions**: the open questions of [decision 0001](decisions/0001-web-engine.md), to answer before phase 4.
2. **Phase order**: imports in phase 3 (right after profiles and Spaces) to ease migration from Arc.
