import { useEffect, useState } from "react";
import { domain } from "./App";
import { send, useAppEvent } from "./bridge";
import { Icon } from "./Icon";
import { toUrl } from "./Overlay";

// Sent by the app as its page changes (send_little in app/src/shell.rs).
export type LittleDetail = {
  url: string;
  title: string;
  // The displayed Space, which "Open in" moves the page into.
  space: { title: string; emoji: string | null };
  // The profile's search engine: its name and its address before the query.
  search: { name: string; url: string };
};

// The bar of a Little Arbalete window (#little/<window id>), like Arc's Little Arc: beside the
// window buttons, the page's domain, which a click edits, or a field while there is no page yet;
// then "Open in <Space>" (⌘O), which moves the page into that Space and closes the window. The bar
// moves the window.
export function Little({ id }: { id: number }) {
  const [page, setPage] = useState<LittleDetail>();
  const [editing, setEditing] = useState(false);
  useAppEvent<LittleDetail>("arbalete:little", setPage);
  useEffect(() => send(`little/${id}/ready`), [id]);
  if (!page) return null;
  const empty = !page.url || page.url === "about:blank";
  const go = (text: string) => {
    const url = toUrl(text) ?? page.search.url + encodeURIComponent(text);
    send(`little/${id}/navigate/${encodeURIComponent(url)}`);
    setEditing(false);
  };
  return (
    <div className="little">
      <div className="little-address">
        {editing || empty ? (
          <input
            aria-label="Search or enter URL"
            placeholder={`Search ${page.search.name} or Enter URL…`}
            defaultValue={empty ? "" : page.url}
            spellCheck={false}
            // biome-ignore lint/a11y/noAutofocus: the field appears to be typed in.
            autoFocus
            onFocus={(event) => event.currentTarget.select()}
            onKeyDown={(event) => {
              const text = event.currentTarget.value.trim();
              if (event.key === "Enter" && text) go(text);
              if (event.key === "Escape") setEditing(false);
            }}
            onBlur={() => setEditing(false)}
          />
        ) : (
          <button
            type="button"
            className="little-domain"
            title={page.url}
            onClick={() => setEditing(true)}
          >
            <Icon d="M6.8 9.2a2.6 2.6 0 0 0 3.7 0l2.2-2.2a2.6 2.6 0 0 0-3.7-3.7l-.9.9M9.2 6.8a2.6 2.6 0 0 0-3.7 0L3.3 9a2.6 2.6 0 0 0 3.7 3.7l.9-.9" />
            <span>{domain(page.url)}</span>
          </button>
        )}
      </div>
      <button
        type="button"
        className="little-open"
        disabled={empty}
        onClick={() => send(`little/${id}/open`)}
      >
        Open in {page.space.emoji ? `${page.space.emoji} ` : ""}
        {page.space.title}
        <kbd>⌘O</kbd>
      </button>
    </div>
  );
}
