import { useEffect, useRef, useState } from "react";
import { type ClosedTab, send } from "./bridge";
import { Icon } from "./Icon";

// The page before the first Space. Of a library, Arbalette keeps only the tabs closed since launch
// so far; reopening one opens it in the displayed Space, whose page `onReopen` slides back to.
export function Library({
  closed,
  shown,
  onReopen,
}: {
  closed: ClosedTab[];
  shown: boolean;
  onReopen: () => void;
}) {
  return (
    <section
      className="end-page library"
      aria-label="Library"
      inert={!shown}
      style={{ left: "-100%" }}
    >
      <h2>Library</h2>
      <h3>Recently closed</h3>
      {closed.length === 0 && <p className="library-empty">No tab closed yet</p>}
      <ul className="tabs">
        {closed.map((tab, rank) => (
          // Closed tabs have no id: they are reopened by rank, the last closed first.
          // biome-ignore lint/suspicious/noArrayIndexKey: the rank is what reopens the tab.
          <li key={rank} className="tab-row">
            <button
              type="button"
              className="tab"
              title={tab.url}
              onClick={() => {
                send(`reopen/${rank}`);
                onReopen();
              }}
            >
              {tab.favicon ? (
                <img className="favicon" src={tab.favicon} alt="" />
              ) : (
                <span className="favicon" />
              )}
              <span className="title">{tab.title || tab.url}</span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

// How long the pointer rests on the Library button before its popover shows.
export const POPOVER_DELAY_MS = 300;

// The footer's Library button opens the library, `onOpen`; resting the pointer on it shows the
// tabs closed last above the footer, to reopen one at once, until the pointer leaves both.
export function LibraryButton({
  closed,
  onOpen,
  onReopen,
}: {
  closed: ClosedTab[];
  onOpen: () => void;
  onReopen: () => void;
}) {
  const [shown, setShown] = useState<number | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => () => clearTimeout(timer.current), []);
  const hide = () => {
    clearTimeout(timer.current);
    setShown(null);
  };
  return (
    // biome-ignore lint/a11y/noStaticElementInteractions: hovering only previews what the button opens.
    <div
      className="library-button"
      onMouseEnter={() => {
        clearTimeout(timer.current);
        timer.current = setTimeout(() => setShown(Date.now()), POPOVER_DELAY_MS);
      }}
      onMouseLeave={hide}
    >
      {shown !== null && (
        <div className="library-popover">
          <section className="popover-panel" aria-label="Recently closed">
            <h3>Recently closed</h3>
            {closed.length === 0 && <p className="library-empty">No tab closed yet</p>}
            <ul>
              {closed.map((tab, rank) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: the rank is what reopens the tab.
                <li key={rank}>
                  <button
                    type="button"
                    title={tab.url}
                    onClick={() => {
                      hide();
                      send(`reopen/${rank}`);
                      onReopen();
                    }}
                  >
                    {tab.favicon ? (
                      <img className="favicon" src={tab.favicon} alt="" />
                    ) : (
                      <span className="favicon placeholder" />
                    )}
                    <span className="title closed-title">{tab.title || tab.url}</span>
                    <span className="when">{ago(tab.closedAt, shown)}</span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        </div>
      )}
      <button
        type="button"
        className="footer-button"
        aria-label="Library"
        title="Library"
        onClick={() => {
          hide();
          onOpen();
        }}
      >
        <Icon d="M3 2.5v11M6 2.5v11M9 4.5v9M10.5 4.2l2.6 9.3" />
      </button>
    </div>
  );
}

// How long ago `time` was at `now`, both in milliseconds, in a few words: "2 min ago".
export function ago(time: number, now: number): string {
  const minutes = Math.floor((now - time) / 60_000);
  if (minutes < 1) return "Just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return `${Math.floor(hours / 24)} d ago`;
}
