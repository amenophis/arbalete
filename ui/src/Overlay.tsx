import { useEffect, useRef, useState } from "react";
import { send, useAppEvent } from "./bridge";
import { Icon } from "./Icon";

type Results = {
  text: string;
  tabs: { id: number; url: string; title: string; favicon: string | null }[];
  pages: { url: string; title: string }[];
};

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
// opens a tab; holding the selected tab's address (⌘L), it loads another in that tab. Below what
// is typed, the address it opens or a search, then the open tabs and the history pages matching
// it; the arrows pick one, Enter or a click opens it, Escape closes the bar.
export function Overlay() {
  const input = useRef<HTMLInputElement>(null);
  const [text, setText] = useState("");
  const [editing, setEditing] = useState(false);
  const [results, setResults] = useState<Results | null>(null);
  const [picked, setPicked] = useState(0);
  useAppEvent<{ url: string | null } | null>("arbalete:show", (detail) => {
    const url = detail?.url ?? null;
    setText(url ?? "");
    setEditing(url !== null);
    setResults(null);
    setPicked(0);
    requestAnimationFrame(() => {
      input.current?.focus();
      input.current?.select();
    });
  });
  // Only the answer to what is typed now counts.
  useAppEvent<Results>("arbalete:results", (detail) => {
    if (detail.text === text) setResults(detail);
  });
  const rows = resultRows(text, results);
  useEffect(() => {
    const height = rows.length
      ? SEARCH_ROW + rows.length * RESULT_ROW + 2 * RESULTS_PADDING
      : SEARCH_ROW;
    send(`overlay/height/${height}`);
  }, [rows.length]);
  const open = (row: Row) => {
    if (row.kind === "tab") {
      send(`select/${row.id}`);
      send("overlay/hide");
    } else {
      send(`${editing ? "navigate" : "open"}/${encodeURIComponent(row.url)}`);
    }
  };
  const type = (value: string) => {
    setText(value);
    setPicked(0);
    send(`overlay/search/${encodeURIComponent(value)}`);
  };
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
        <Icon d="M7 11.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM10.3 10.3 13.5 13.5" size={18} />
        <input
          ref={input}
          aria-label="Search or enter URL"
          placeholder="Search or Enter URL…"
          value={text}
          spellCheck={false}
          onChange={(event) => type(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") send("overlay/hide");
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

// What is typed, opened as an address or searched, then the matching open tabs and history pages.
export function resultRows(text: string, results: Results | null): Row[] {
  const typed = text.trim();
  if (!typed) return [];
  const url = toUrl(typed);
  const go: Row = url
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
