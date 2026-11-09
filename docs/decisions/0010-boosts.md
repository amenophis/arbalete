# 0010 — Boosts

- Status: proposed
- Date: 2026-11-09

## Context

Arc's Boosts change how a site looks or behaves, for the user only: hide parts of the page, recolor it, change
its font, text case or size, or run the user's own CSS and JavaScript on it. The website lists them as not
supported yet; a Chrome extension such as Stylus covers only part of them.

What Arc does, from its app bundle (never launched, no Arc data read):

- **A Boost** belongs to one site (its host), with a title and an on/off state; a site can have several. It holds
  any of: zaps (CSS selectors of hidden elements), a color scheme (a color wheel: a primary hue and saturation,
  a secondary hue offset, an "invert lightness" switch), a font family, a text case (capitalize, upper, lower),
  a text size (five steps around normal), custom CSS and custom JavaScript.
- **Injection** is Arc's own engine: per host, one stylesheet per kind (user CSS, font, text case, zaps), one
  script, a color filter for the colors and a zoom factor for the text size. The script runs as a plain page
  script, in the page's own JavaScript world, like a userscript; no isolated-world path shows. Zaps are
  `display: none !important` rules; the font and text case rules leave icon fonts alone.
- **The editor** floats over the page: a header (title, undo and redo, reset, rename, delete), the color wheel,
  the font, text case and text size buttons, zap, and code, which opens CSS and JavaScript in a window of its
  own. Zap highlights the element under the pointer and, on a click, offers a selector for this element or for
  all the related ones, with how many each hides; the zaps are listed and can be removed one by one.
- **Where they show**: a Boost tile and a Boosts section (each Boost with its switch and an edit button) in the
  site controls, New Boost in the menus and the Command Bar, and a Boosts page in Arc's library.
- **Sharing and sync** go through Arc's servers; Boosts holding JavaScript cannot be shared.

## Decision

Boosts like Arc's, every kind included, for the user only.

- **Model** (`arbalete-core`, `boosts`): a profile's Boosts, each with an id, a site, a title, on or off, and its
  zaps, colors, font, text case, text size, CSS and JavaScript. The site is the host without a leading `www.`,
  and a Boost applies to that host and its `www.` form only, like Arc's one host per Boost. The core turns the
  site's active Boosts into one stylesheet and one script, so every rule is tested there.
- **Storage**: `boosts.json` in the profile's directory, apart from `session.json`, which is saved on every tab
  change while Boosts change rarely and can be long. Incognito windows get no Boosts and cannot make one.
  No sharing and no sync; importing Arc's Boosts is left for later (Arc keeps them on its servers).
- **Injection, in CEF Alloy**: through each page's DevTools session, which Arbalete already opens
  (extensions).
  `Page.addScriptToEvaluateOnNewDocument` installs, in every frame of every new document and before the page's
  own scripts, a small script carrying the profile's active Boosts by site. It picks those of its frame's host,
  applies their CSS at once as a constructed stylesheet (`document.adoptedStyleSheets`), which needs no element
  and which a page's Content Security Policy does not block, so nothing flashes, and runs their JavaScript in the
  page's world once the DOM is ready. A change to the Boosts replaces that script for the next documents and
  re-applies the CSS live to the open pages of the site, from the same session (`Runtime.evaluate`); a change
  to the JavaScript reloads them, since a script cannot be taken back. Only `http` and `https` pages get Boosts.
  Frames of another site in another process (site isolation) are not reached, as their DevTools target differs:
  a Boost is meant for the site's own pages.
- **Colors** have no engine filter in CEF: they become a CSS filter on the root element, `hue-rotate` from the
  wheel's hue, `saturate` from its saturation, and for "invert lightness" `invert` with a `hue-rotate(180deg)`
  that keeps hues, images and videos inverted back. It is an approximation of Arc's color mapping, not the same
  palette. The text size is CSS `zoom` on the root element, apart from the page zoom the user sets.
- **The editor**, like Arc's: a panel over the page, a view of the main window like the other panels, with the
  header, the color wheel, the font, text case and text size buttons, zap and code. Code opens a window with a
  CSS and a JavaScript tab, a monospaced plain text editor, applied as it is typed (CSS) or on save
  (JavaScript). Zap runs a picker in the page's isolated world, through the DevTools session: it outlines the
  element under the pointer, and a click offers a selector for that element and one for its related elements,
  with their counts. Undo and redo keep the editor's states in the core.
- **Entry points**: the site controls' second tile becomes the Boost tile (Copy Link moves to the "…" menu), and
  a Boosts section lists the site's Boosts with a switch and an edit button; New Boost in the Plus menu and the
  Command Bar; a Boosts section in the Library lists all of them, by site.
- **User JavaScript** runs with the page's rights, as if the site had shipped it: it is the user's own code, never
  fetched or shared, so it is trusted like a bookmarklet. The code window says so above the JavaScript tab.

## Consequences

- Arc users find their Boost kinds, editor and places, but must make their Boosts again.
- Colors look close to Arc's, not the same: a hue rotation shifts every color, where Arc remaps them to a palette.
- Every page gets one more script before its own, carrying the profile's active Boosts: a few kilobytes, read
  once per document.
- A page that resets `document.adoptedStyleSheets` drops the Boost's CSS until the next load; few do.
- Boosts depend on the DevTools session Arbalete opens on each page, like the extensions' tab ids already do.
