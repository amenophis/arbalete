import { memo } from "react";
import { type Item, send, type Tab, useAppState } from "./bridge";

export function App() {
  const state = useAppState();
  const page = state?.page;
  const active =
    state && tabs([...state.pinned, ...state.today]).find((tab) => tab.id === state.active);
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
      <ul className="tabs pinned">
        <Items items={state?.pinned ?? []} active={state?.active} />
      </ul>
      <hr className="divider" />
      <button type="button" className="tab" onClick={() => send("overlay/show")}>
        <span className="favicon">+</span>
        <span className="title">New tab</span>
      </button>
      <ul className="tabs today">
        <Items items={state?.today ?? []} active={state?.active} />
      </ul>
    </nav>
  );
}

function tabs(items: Item[]): Tab[] {
  return items.flatMap((item) => ("children" in item ? tabs(item.children) : [item]));
}

function Items({ items, active }: { items: Item[]; active?: number | null }) {
  return items.map((item) =>
    "children" in item ? (
      <li key={item.id}>
        <div className="folder">
          <span className="favicon">{item.expanded ? "▾" : "▸"}</span>
          <span className="title">{item.title}</span>
        </div>
        {item.expanded && (
          <ul className="folder-tabs">
            <Items items={item.children} active={active} />
          </ul>
        )}
      </li>
    ) : (
      <TabItem
        key={item.id}
        id={item.id}
        title={item.title || item.url}
        favicon={item.favicon}
        active={item.id === active}
      />
    ),
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
