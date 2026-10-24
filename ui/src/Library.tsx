import { type ClosedTab, send } from "./bridge";

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
