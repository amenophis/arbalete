import { memo } from "react";
import { send, useAppState } from "./bridge";

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
          <TabItem
            key={tab.id}
            id={tab.id}
            title={tab.title || tab.url}
            favicon={tab.favicon}
            active={tab.id === state.active}
          />
        ))}
      </ul>
    </nav>
  );
}

// Every state update brings new tab objects: primitive props let unchanged rows skip rendering.
const TabItem = memo(function TabItem({
  id,
  title,
  favicon,
  active,
}: {
  id: number;
  title: string;
  favicon: string | null;
  active: boolean;
}) {
  return (
    <li className="tab-row">
      <button
        type="button"
        className="tab"
        aria-current={active ? "page" : undefined}
        onClick={() => send(`select/${id}`)}
      >
        {favicon ? <img className="favicon" src={favicon} alt="" /> : <span className="favicon" />}
        <span className="title">{title}</span>
      </button>
      <button
        type="button"
        className="close"
        aria-label={`Close ${title}`}
        onClick={() => send(`close/${id}`)}
      >
        ✕
      </button>
    </li>
  );
});
