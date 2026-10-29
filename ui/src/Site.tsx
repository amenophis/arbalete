import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { send } from "./bridge";
import { Icon } from "./Icon";

// An extension the user installed in the displayed Space's profile (installed in
// app/src/extensions.rs).
export type Extension = {
  id: string;
  name: string;
  icon: string | null;
  // Its button opens a popup, rather than telling the extension it was clicked.
  popup: boolean;
};

type Detail = {
  url: string;
  // Turned off, an extension does not run: its menu turns it on again. Pinned, its button shows
  // by the address too.
  extensions: (Extension & { enabled: boolean; pinned: boolean })[];
};

// The panel taking the focus makes Chromium focus its first button as if tabbed to, with a ring:
// only the Tab key may leave a button focused.
let tabbed = false;
addEventListener("keydown", (event) => {
  tabbed = event.key === "Tab";
});

// The site controls' actions, the items of the site menu (site_menu in app/src/menu.rs).
const ACTIONS = [
  { item: "share", label: "Share", d: "M8 9.5v-7M5.5 5L8 2.5 10.5 5M5.5 7.5h-2v6h9v-6h-2" },
  {
    item: "copy-link",
    label: "Copy Link",
    d: "M6.8 9.2a2.6 2.6 0 0 0 3.7 0l2.2-2.2a2.6 2.6 0 0 0-3.7-3.7l-.9.9M9.2 6.8a2.6 2.6 0 0 0-3.7 0L3.3 9a2.6 2.6 0 0 0 3.7 3.7l.9-.9",
  },
  {
    item: "clear-cache",
    label: "Clear Cache",
    d: "M3 4.5h10M6.5 4.5v-2h3v2M4.5 4.5l.6 9h5.8l.6-9",
  },
  {
    item: "clear-cookies",
    label: "Clear Cookies",
    d: "M8 2.5a5.5 5.5 0 1 0 5.5 5.5A2 2 0 0 1 11 6a2 2 0 0 1-2.5-2.5 1.5 1.5 0 0 1-.5-1zM6 7h.01M9 10.5h.01M5.5 10h.01",
  },
];

// The selected tab's site controls, in a panel under their button, like Arc's (show_site_controls
// in app/src/shell.rs): actions on its link and what the site keeps, the profile's extensions,
// whose popup a click opens in place of the panel, the Chrome Web Store to add more, and whether
// the connection is secure. The
// panel closes on losing the focus and on Escape, and sizes itself to what it shows.
// The pinned extensions' buttons, by the address (UiState::pinned_extensions in
// app/src/shell.rs): a click opens the extension's popup under its button, in a window of its own.
export function PinnedExtensions({
  extensions = [],
  view,
}: {
  extensions?: Extension[];
  view: "docked" | "revealed" | "toolbar";
}) {
  return extensions.map((extension) => (
    <button
      key={extension.id}
      type="button"
      className="pinned-extension"
      aria-label={extension.name}
      title={extension.name}
      disabled={!extension.popup}
      onClick={(event) => {
        const { x, y, width, height } = event.currentTarget.getBoundingClientRect();
        const button = [x, y, width, height].map(Math.round).join("/");
        send(`extension/popup/${extension.id}/${view}/${button}`);
      }}
    >
      {extension.icon ? <img src={extension.icon} alt="" /> : extension.name.slice(0, 1)}
    </button>
  ));
}

export function Site() {
  const panel = useRef<HTMLDivElement>(null);
  const [site, setSite] = useState<Detail>();
  useEffect(() => {
    const show = (event: Event) => setSite((event as CustomEvent<Detail>).detail);
    const hide = () => send("site/hide");
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") hide();
    };
    addEventListener("arbalete:show", show);
    addEventListener("blur", hide);
    addEventListener("keydown", onKey);
    return () => {
      removeEventListener("arbalete:show", show);
      removeEventListener("blur", hide);
      removeEventListener("keydown", onKey);
    };
  }, []);
  useLayoutEffect(() => {
    if (!site || !panel.current) return;
    send(`site/height/${panel.current.offsetHeight}`);
    // The app shows an extension's menu on a right-click, which the page never sees.
    const targets = [...panel.current.querySelectorAll<HTMLElement>("[data-extension]")].map(
      (tile) => {
        const { x, y, width, height } = tile.getBoundingClientRect();
        return { id: tile.dataset.extension, x, y, width, height };
      },
    );
    send(`site/targets/${encodeURIComponent(JSON.stringify(targets))}`);
  }, [site]);
  if (!site) return null;
  const secure = site.url.startsWith("https://");
  return (
    <div
      className="site"
      role="dialog"
      aria-label="Site controls"
      ref={panel}
      tabIndex={-1}
      onFocus={(event) => {
        if (!tabbed && event.target !== panel.current) panel.current?.focus();
      }}
    >
      <div className="site-actions">
        {ACTIONS.map(({ item, label, d }) => (
          <button
            key={item}
            type="button"
            className="site-button"
            aria-label={label}
            title={label}
            onClick={() => {
              send("site/hide");
              send(`menu/choose/site/any/${item}`);
            }}
          >
            <Icon d={d} size={18} />
          </button>
        ))}
      </div>
      <h2>Extensions</h2>
      <div className="site-extensions">
        {site.extensions.map((extension) => (
          <button
            key={extension.id}
            data-extension={extension.id}
            type="button"
            className="site-button"
            aria-label={extension.name}
            title={extension.enabled ? extension.name : `${extension.name} (off)`}
            disabled={!extension.popup || !extension.enabled}
            onClick={() => send(`site/extension/${extension.id}`)}
          >
            {extension.icon ? <img src={extension.icon} alt="" /> : extension.name.slice(0, 1)}
            {extension.pinned && (
              <span className="site-pinned" role="img" aria-label="Pinned">
                <Icon d="M6 2.5h4M7 2.5v4l-2 2.5h6L9 6.5v-4M8 9v4.5" size={10} />
              </span>
            )}
          </button>
        ))}
        <button
          type="button"
          className="site-button"
          aria-label="Add Extensions"
          title="Add Extensions"
          onClick={() => send("site/store")}
        >
          <Icon d="M8 3.5v9M3.5 8h9" size={18} />
        </button>
      </div>
      <div className="site-footer">
        <Icon
          d={
            secure
              ? "M4.5 7.5h7v6h-7zM6 7.5v-2a2 2 0 0 1 4 0v2"
              : "M4.5 7.5h7v6h-7zM6 7.5v-2a2 2 0 0 1 3.8-.9"
          }
        />
        {secure ? "Secure" : "Not Secure"}
      </div>
    </div>
  );
}
