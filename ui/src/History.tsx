import { useEffect, useState } from "react";
import { send, useAppEvent } from "./bridge";
import { byDay, host } from "./time";

export type Visit = { url: string; title: string; lastVisit: number };

// How many pages the history shows at first, and adds each time more are asked for.
export const PAGE_SIZE = 200;

// The displayed Space's profile history, the latest visit first, grouped by day, under the
// library's search field: the app searches it (search_history in app/src/shell.rs) and sends
// the pages found, which open in a tab of the displayed Space, `onOpen` sliding back to it.
export function HistoryList({
  query,
  now,
  onOpen,
}: {
  query: string;
  now: number;
  onOpen: () => void;
}) {
  // "Show more" asks for more of one search: a new search shows its first page.
  const [more, setMore] = useState({ query, limit: PAGE_SIZE });
  const limit = more.query === query ? more.limit : PAGE_SIZE;
  const [found, setFound] = useState<{ text: string; entries: Visit[] }>();
  useEffect(() => send(`history/search/${limit}/${encodeURIComponent(query)}`), [query, limit]);
  // Only the answer to the latest search counts.
  useAppEvent<{ text: string; entries: Visit[] }>("arbalete:history", (detail) => {
    if (detail.text === query) setFound(detail);
  });
  if (!found) return null;
  if (found.entries.length === 0) {
    return <p className="library-empty">{query ? "No page matches" : "No history yet"}</p>;
  }
  return (
    <div className="history-list">
      {byDay(found.entries, (visit) => visit.lastVisit, now).map(([name, visits]) => (
        <section key={name} aria-label={name}>
          <h3>{name}</h3>
          <ul>
            {visits.map((visit) => (
              <li key={visit.url} className="tab-row visit">
                <button
                  type="button"
                  className="tab"
                  title={visit.url}
                  onClick={() => {
                    send(`open/${encodeURIComponent(visit.url)}`);
                    onOpen();
                  }}
                >
                  <span className="favicon placeholder" />
                  <span className="download-text">
                    <span className="title">{visit.title || visit.url}</span>
                    <span className="download-detail">
                      {`${time(visit.lastVisit)} · ${host(visit.url)}`}
                    </span>
                  </span>
                </button>
                <button
                  type="button"
                  className="close"
                  aria-label="Remove from history"
                  onClick={() => {
                    send(`history/remove/${encodeURIComponent(visit.url)}`);
                    setFound({
                      ...found,
                      entries: found.entries.filter((entry) => entry.url !== visit.url),
                    });
                  }}
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
      {found.entries.length === limit && (
        <button
          type="button"
          className="show-more"
          onClick={() => setMore({ query, limit: limit + PAGE_SIZE })}
        >
          Show more
        </button>
      )}
    </div>
  );
}

function time(lastVisit: number): string {
  return new Date(lastVisit).toLocaleTimeString(undefined, { timeStyle: "short" });
}
