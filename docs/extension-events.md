# Tab and window events for extensions

Arbalete's pages sit in no Chromium window, so Chromium tells extensions nothing about them:
[the shim](../app/src/extension-shim.js) answers `chrome.tabs` / `chrome.windows` and dispatches their
events itself ([decision 0011](decisions/0011-extensions-without-helper.md)). This list compares it with
Chrome's [tabs](https://developer.chrome.com/docs/extensions/reference/api/tabs) and
[windows](https://developer.chrome.com/docs/extensions/reference/api/windows) reference, so that a fix
checks against Chrome rather than against one extension. Windows an extension opens itself, like
Bitwarden's unlock window, are Chromium's: they get Chrome's events natively.

## What the shim knows

One window, id 1, and one tab in it: the displayed Space's selected tab (or Little Arbalete's), once its
page has a tab id. An incognito window's selected tab is its profile's, `incognito: true`, only for the extensions
allowed in incognito ([decision 0018](decisions/0018-extensions-in-incognito.md)): the others see no window
focused. A worker's first state is announced like a change, as its script ran before the shim.

| Field | Chrome | Shim |
|---|---|---|
| `id` | tab id | Chromium's own tab id of the page, the one `sender.tab.id` carries |
| `windowId`, `index` | its window, its place | always 1 and 0; `sender.tab` says -1 and -1 |
| `active`, `highlighted` | per tab | always true; `sender.tab.active` is false |
| `url`, `title`, `favIconUrl` | with `tabs` or a matching host permission | same rule |
| `status` | `loading` / `complete` (`unloaded` when discarded) | `loading` / `complete` |
| `pendingUrl`, `lastAccessed`, `audible`, `mutedInfo`, `openerTabId`, `width`, `height` | set | missing |

## Events, in Chrome's order

| What happens | Chrome | Shim |
|---|---|---|
| New tab, in front | `tabs.onCreated`, `onActivated`, `onHighlighted`, then `onUpdated` (below) | `onCreated` then `onActivated` once the page has a tab id, then `onUpdated`. No `onHighlighted` |
| Switch tab | `onActivated {tabId, windowId}`, `onHighlighted` | `onActivated`, plus `onCreated` before it for a tab the worker has not seen |
| Navigate | `onUpdated {status: "loading", url}`, `{title}`, `{status: "complete"}`, `{favIconUrl}`, around `webNavigation`'s events | the same keys, but `url` comes once the address changes, after `webNavigation.onCommitted`, often after `onCompleted` |
| Background tab changes | `onUpdated` for that tab | nothing: only the active tab is known |
| Move a tab | `onMoved`, or `onDetached` / `onAttached` across windows | nothing |
| Close a tab | `onRemoved {windowId, isWindowClosing}`, then `onActivated` for the next one | only `onActivated` (and `onCreated` if unseen) for the next one: `onRemoved` never fires |
| Focus another window, or another app | `windows.onFocusChanged(id)`, `-1` when none of the browser's windows has it | `onFocusChanged(1)` or `(-1)` as the displayed Space's profile comes and goes; `1` again once a Chromium window loses the focus or closes |
| Open or close a window | `windows.onCreated` / `onRemoved`, `onBoundsChanged` | nothing for Arbalete's windows, which all are window 1 |
| Worker starts | nothing: `tabs.query` lists every tab | `onFocusChanged`, `onCreated`, `onActivated` for the active tab, once the shim is in |

## Known gaps

- Only the active tab exists: `tabs.query({})` lists one tab, and an extension never hears of other tabs
  closing (`onRemoved`). Bitwarden's live-tab set only grows.
- `query` filters only on `active` and `windowId`: `url`, `windowType`, `status`… return the active tab
  too, which extensions then filter themselves.
- `sender.tab` (Chromium's) and the shim's tab disagree on `windowId`, `index` and `active`.
- The event that wakes a stopped worker runs before the shim: its first `tabs.query` finds nothing.
