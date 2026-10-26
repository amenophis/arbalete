import { useEffect, useRef, useState } from "react";
import { type ClosedTab, type Profile, type Space, send, useAppEvent } from "./bridge";
import { type Download, DownloadsList } from "./Downloads";
import { HistoryList } from "./History";
import { Icon } from "./Icon";
import { ManageSpaces } from "./ManageSpaces";
import { matches } from "./search";

export type Section = "spaces" | "closed" | "history" | "downloads";

// The sections the library holds so far, in its rail.
export const SECTIONS: { id: Section; label: string; icon: string }[] = [
  {
    id: "spaces",
    label: "Spaces",
    icon: "M2.5 4.5a2 2 0 0 1 2-2h2v11h-2a2 2 0 0 1-2-2zM6.5 2.5h3v11h-3zM9.5 2.5h2a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-2z",
  },
  {
    id: "closed",
    label: "Recently Closed",
    icon: "M2.8 8a5.2 5.2 0 1 0 1.5-3.7M2.6 2.4v2.4H5M8 5.2V8l2 1.3",
  },
  {
    id: "history",
    label: "History",
    icon: "M2.5 8a5.5 5.5 0 1 0 1.6-3.9M2.5 2.5v2.6h2.6M8 5v3.2l2.2 1.3",
  },
  {
    id: "downloads",
    label: "Downloads",
    icon: "M8 2.5v8M4.5 7 8 10.5 11.5 7M3 13.5h10",
  },
];

// The page before the first Space: a rail of sections, the one picked last kept until the app
// quits, and the picked section's content under a field searching it, but for the Spaces. Spaces are managed there;
// the tabs closed since launch reopen in the displayed Space, whose page `onBack` slides back to;
// the history searches the displayed Space's profile; the downloads are all listed.
export function Library({
  closed,
  downloads,
  spaces,
  profiles,
  shown,
  onBack,
  onShow,
  onNewSpace,
}: {
  closed: ClosedTab[];
  downloads: Download[];
  spaces: Space[];
  profiles: Profile[];
  shown: boolean;
  onBack: () => void;
  // Shows the Space at an index in sidebar order.
  onShow: (index: number) => void;
  onNewSpace: () => void;
}) {
  const [section, setSection] = useState<Section>("spaces");
  const [query, setQuery] = useState("");
  // The Space menu's Manage Spaces opens the library on its Spaces.
  useAppEvent("arbalete:library", () => pick("spaces"));
  useAppEvent("arbalete:show-downloads", () => pick("downloads"));
  useAppEvent("arbalete:show-history", () => pick("history"));
  const pick = (next: Section) => {
    setSection(next);
    setQuery("");
  };
  // The Spaces fill the window, the page hidden; the other sections rest beside it.
  useEffect(() => send(section === "spaces" ? "library/wide" : "library/narrow"), [section]);
  const label = SECTIONS.find(({ id }) => id === section)?.label;
  return (
    <section
      className="end-page library"
      aria-label="Library"
      inert={!shown}
      style={{ left: "-100%" }}
    >
      <div className="library-rail">
        <div role="tablist" aria-label="Library sections" aria-orientation="vertical">
          {SECTIONS.map(({ id, label, icon }) => (
            <button
              key={id}
              type="button"
              role="tab"
              className="library-section"
              aria-selected={id === section}
              onClick={() => pick(id)}
            >
              <Icon d={icon} size={22} />
              <span>{label}</span>
            </button>
          ))}
        </div>
        <button
          type="button"
          className="library-back"
          aria-label="Back to the Space"
          title="Back to the Space"
          onClick={onBack}
        >
          <Icon d="M13 8H3.5M7.5 4 3.5 8l4 4" />
        </button>
      </div>
      <div className="library-panel" role="tabpanel" aria-label={label} data-section={section}>
        {section !== "spaces" && (
          <input
            type="search"
            className="library-search"
            aria-label={`Search ${label}`}
            placeholder={`Search ${label}…`}
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
          />
        )}
        {section === "spaces" && (
          <ManageSpaces
            spaces={spaces}
            profiles={profiles}
            onShow={onShow}
            onNewSpace={onNewSpace}
          />
        )}
        {section === "closed" && <ClosedTabs closed={closed} query={query} onReopen={onBack} />}
        {section === "history" && <HistoryList query={query} now={Date.now()} onOpen={onBack} />}
        {section === "downloads" && (
          <DownloadsList downloads={downloads} query={query} now={Date.now()} />
        )}
      </div>
    </section>
  );
}

// The tabs closed since launch, the last closed first, which reopen in the displayed Space.
function ClosedTabs({
  closed,
  query,
  onReopen,
}: {
  closed: ClosedTab[];
  query: string;
  onReopen: () => void;
}) {
  if (closed.length === 0) return <p className="library-empty">No tab closed yet</p>;
  // Closed tabs have no id: they are reopened by rank, the last closed first.
  const found = closed
    .map((tab, rank) => ({ tab, rank }))
    .filter(({ tab }) => matches(query, tab.title, tab.url));
  if (found.length === 0) return <p className="library-empty">No closed tab matches</p>;
  const now = Date.now();
  return (
    <ul className="closed-tabs">
      {found.map(({ tab, rank }) => (
        <li key={rank}>
          <button
            type="button"
            className="closed-tab"
            title={tab.url}
            onClick={() => {
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
            <span className="when">{ago(tab.closedAt, now)}</span>
          </button>
        </li>
      ))}
    </ul>
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
