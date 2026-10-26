import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { send, useAppEvent } from "./bridge";
import { Icon } from "./Icon";

type Results = {
  text: string;
  tabs: { id: number; url: string; title: string; favicon: string | null }[];
  pages: { url: string; title: string }[];
};

type Switching = { tabs: Results["tabs"]; picked: number };

type Row =
  | { kind: "go"; url: string; title: string; search: boolean }
  | { kind: "tab"; id: number; url: string; title: string; favicon: string | null }
  | { kind: "page"; url: string; title: string };

// The search row's height and a result's, in points, and the space around the results: the app
// fits the Command Bar to them (place_overlay in app/src/shell.rs).
export const SEARCH_ROW = 62;
export const RESULT_ROW = 44;
const RESULTS_PADDING = 8;

// The search engine for what is not an address.
const SEARCH_URL = "https://www.google.com/search?q=";

// The Command Bar, shown above the page by the app (open_overlay in app/src/shell.rs): empty, it
// opens a tab; holding the selected tab's address and icon (⌘L), over the sidebar's url bar, it
// loads another in that tab. Below what
// is typed, the address it opens or a search, then the open tabs and the history pages matching
// it; the arrows pick one, Enter or a click opens it, Escape closes the bar. Typing forward
// completes the field with the best result's address, selected: typing on replaces it, deleting
// drops it, → or End keeps it.
export function Overlay() {
  const input = useRef<HTMLInputElement>(null);
  const [text, setText] = useState("");
  const [editing, setEditing] = useState(false);
  // The edited page's icon, which the field shows instead of the search icon.
  const [favicon, setFavicon] = useState<string | null>(null);
  const [results, setResults] = useState<Results | null>(null);
  const [picked, setPicked] = useState(0);
  const [completing, setCompleting] = useState(false);
  const deleting = useRef(false);
  // The tabs selected last while ⌃⇥ is held, the picked one highlighted (switch_tab in
  // app/src/shell.rs): the app shows the picked tab once ⌃ is released.
  const [switching, setSwitching] = useState<Switching | null>(null);
  useAppEvent<Switching>("arbalete:switch", setSwitching);
  useAppEvent<{ url: string | null; favicon?: string | null } | null>("arbalete:show", (detail) => {
    const url = detail?.url ?? null;
    setText(url ?? "");
    setEditing(url !== null);
    setFavicon(detail?.favicon ?? null);
    setResults(null);
    setPicked(0);
    setCompleting(false);
    setSwitching(null);
    requestAnimationFrame(() => {
      input.current?.focus();
      input.current?.select();
    });
  });
  // Only the answer to what is typed now counts.
  useAppEvent<Results>("arbalete:results", (detail) => {
    if (detail.text === text) setResults(detail);
  });
  const completion = completing ? complete(text, results) : null;
  const value = text + (completion?.suffix ?? "");
  useLayoutEffect(() => {
    if (value !== text) input.current?.setSelectionRange(text.length, value.length);
  }, [text, value]);
  const rows = resultRows(text, results, completion);
  const height = switching
    ? switching.tabs.length * RESULT_ROW + 2 * RESULTS_PADDING
    : rows.length
      ? SEARCH_ROW + rows.length * RESULT_ROW + 2 * RESULTS_PADDING
      : SEARCH_ROW;
  useEffect(() => send(`overlay/height/${height}`), [height]);
  const open = (row: Row) => {
    if (row.kind === "tab") {
      send(`select/${row.id}`);
      send("overlay/hide");
    } else {
      send(`${editing ? "navigate" : "open"}/${encodeURIComponent(row.url)}`);
    }
  };
  const type = (typed: string) => {
    setCompleting(!deleting.current && typed.length > text.length);
    setText(typed);
    setPicked(0);
    send(`overlay/search/${encodeURIComponent(typed)}`);
  };
  if (switching) {
    return (
      <ul className="overlay overlay-results" aria-label="Recent tabs">
        {switching.tabs.map((tab, index) => (
          <li key={tab.id}>
            <div className="overlay-result" aria-current={index === switching.picked || undefined}>
              <RowIcon row={{ kind: "tab", ...tab }} />
              <span className="title">{tab.title || tab.url}</span>
            </div>
          </li>
        ))}
      </ul>
    );
  }
  return (
    <form
      className="overlay"
      onSubmit={(event) => {
        event.preventDefault();
        const row = rows[picked];
        if (row) open(row);
      }}
    >
      <label className="overlay-search">
        {editing && favicon ? (
          <img className="favicon" src={favicon} alt="" />
        ) : (
          <Icon d="M7 11.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM10.3 10.3 13.5 13.5" size={18} />
        )}
        <input
          ref={input}
          aria-label="Search or enter URL"
          placeholder="Search or Enter URL…"
          value={value}
          spellCheck={false}
          onChange={(event) => type(event.currentTarget.value)}
          onKeyDown={(event) => {
            deleting.current = event.key === "Backspace" || event.key === "Delete";
            if (event.key === "Escape") send("overlay/hide");
            if (completion && (event.key === "ArrowRight" || event.key === "End")) {
              setText(value);
              setCompleting(false);
            }
            const step = { ArrowDown: 1, ArrowUp: -1 }[event.key];
            if (step && rows.length) {
              event.preventDefault();
              setPicked((picked + step + rows.length) % rows.length);
            }
          }}
          onBlur={() => send("overlay/hide")}
        />
      </label>
      {rows.length > 0 && (
        <ul className="overlay-results" aria-label="Results">
          {rows.map((row, index) => (
            <li key={`${row.kind}:${"id" in row ? row.id : row.url}`}>
              <button
                type="button"
                className="overlay-result"
                aria-current={index === picked || undefined}
                // Keeps the field focused, which a click would blur, closing the bar.
                onMouseDown={(event) => event.preventDefault()}
                onMouseEnter={() => setPicked(index)}
                onClick={() => open(row)}
              >
                <RowIcon row={row} />
                <span className="title">{row.title}</span>
                {row.kind === "tab" && <span className="overlay-action">Switch to Tab →</span>}
              </button>
            </li>
          ))}
        </ul>
      )}
    </form>
  );
}

function RowIcon({ row }: { row: Row }) {
  if (row.kind === "tab" && row.favicon)
    return <img className="favicon" src={row.favicon} alt="" />;
  if (row.kind === "go" && row.search) {
    return <Icon d="M7 11.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM10.3 10.3 13.5 13.5" />;
  }
  // A globe.
  return (
    <Icon d="M8 13.5a5.5 5.5 0 1 0 0-11 5.5 5.5 0 0 0 0 11zM2.5 8h11M8 2.5c-2 2.8-2 8.2 0 11M8 2.5c2 2.8 2 8.2 0 11" />
  );
}

// What is typed, opened as an address, completed or not, or searched, then the matching open tabs
// and history pages.
export function resultRows(
  text: string,
  results: Results | null,
  completion: Completion | null = null,
): Row[] {
  const typed = text.trim();
  if (!typed) return [];
  const url = toUrl(typed);
  const go: Row = completion
    ? { kind: "go", url: completion.url, title: typed + completion.suffix, search: false }
    : url
      ? { kind: "go", url, title: typed, search: false }
      : {
          kind: "go",
          url: SEARCH_URL + encodeURIComponent(typed),
          title: `${typed} — Search Google`,
          search: true,
        };
  return [
    go,
    ...(results?.tabs ?? []).map(
      (tab): Row => ({ kind: "tab", ...tab, title: tab.title || tab.url }),
    ),
    ...(results?.pages ?? []).map(
      (page): Row => ({ kind: "page", ...page, title: page.title || page.url }),
    ),
  ];
}

type Completion = { suffix: string; url: string };

// What completes typed text: the address of the first result, history pages then open tabs,
// that starts with it, without scheme nor "www.": up to the end of its host, or all of it once a
// "/" is typed. It opens that site's home, or that page.
export function complete(text: string, results: Results | null): Completion | null {
  const typed = text.toLowerCase();
  if (!typed || /\s/.test(typed) || !results) return null;
  for (const { url } of [...results.pages, ...results.tabs]) {
    const address = url.replace(/^[a-z][a-z\d+.-]*:\/\//i, "").replace(/^www\./i, "");
    if (!address.toLowerCase().startsWith(typed)) continue;
    const page = typed.includes("/");
    const whole = page ? address.replace(/\/$/, "") : address.split("/")[0];
    if (whole.length > typed.length) {
      return { suffix: whole.slice(typed.length), url: page ? url : `${new URL(url).origin}/` };
    }
  }
  return null;
}

// The address typed text stands for, or null when it is a search: a URL with a scheme stays as it
// is, a host with a dot, localhost or an IP address opens over https.
export function toUrl(text: string): string | null {
  if (/^(https?|file|ftp):\/\//i.test(text) || /^(about|data|mailto|view-source):/i.test(text)) {
    return text;
  }
  if (/\s/.test(text)) return null;
  if (/^localhost(:\d+)?(\/|$)/i.test(text)) return `http://${text}`;
  return /^[^/]+\.[a-z\d-]{2,}(:\d+)?(\/|$)/i.test(text) || /^\d+(\.\d+){3}(:\d+)?(\/|$)/.test(text)
    ? `https://${text}`
    : null;
}
