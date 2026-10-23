import { send, type Tab, useAppState } from "./bridge";

export function App() {
  const state = useAppState();
  const page = state?.page;
  const active = state?.tabs.find((tab) => tab.id === state.active);
  return (
    <nav className="sidebar">
      <div className="toolbar">
        <button
          type="button"
          aria-label="Back"
          disabled={!page?.canGoBack}
          onClick={() => send("back")}
        >
          ‹
        </button>
        <button
          type="button"
          aria-label="Forward"
          disabled={!page?.canGoForward}
          onClick={() => send("forward")}
        >
          ›
        </button>
        <button type="button" aria-label="Reload" onClick={() => send("reload")}>
          ↻
        </button>
      </div>
      {active && <div className="url">{active.url}</div>}
      {page?.loading && (
        <progress className="progress" aria-label="Loading" value={page.progress} />
      )}
      <button type="button" className="tab" onClick={() => send("overlay/show")}>
        <span className="favicon">+</span>
        <span className="title">New tab</span>
      </button>
      <ul className="tabs">
        {state?.tabs.map((tab) => (
          <li key={tab.id}>
            <TabItem tab={tab} active={tab.id === state.active} />
          </li>
        ))}
      </ul>
    </nav>
  );
}

function TabItem({ tab, active }: { tab: Tab; active: boolean }) {
  return (
    <button
      type="button"
      className="tab"
      aria-current={active ? "page" : undefined}
      onClick={() => send(`select/${tab.id}`)}
    >
      {tab.favicon ? (
        <img className="favicon" src={tab.favicon} alt="" />
      ) : (
        <span className="favicon" />
      )}
      <span className="title">{tab.title || tab.url}</span>
    </button>
  );
}
