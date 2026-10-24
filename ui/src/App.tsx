import { type CSSProperties, type KeyboardEvent, memo, useEffect, useRef } from "react";
import { COLORS, type Item, send, type Tab, useAppState, useRenaming } from "./bridge";
import { useDrag } from "./drag";
import { Profiles } from "./Profiles";
import { Spaces } from "./Spaces";
import { type Gesture, swipeTracker } from "./swipe";

export function App() {
  const state = useAppState();
  const [renaming, setRenaming] = useRenaming("arbalette:rename");
  const { dragging, indicator, onPointerDown } = useDrag();
  useSwipes();
  const page = state?.page;
  const active =
    state &&
    [...state.favorites, ...tabs([...state.pinned, ...state.today])].find(
      (tab) => tab.id === state.active,
    );
  const space = state?.spaces.find((s) => s.id === state.space);
  // The displayed Space's color tints the sidebar.
  const tint = space && ({ "--space": COLORS[space.color] } as CSSProperties);
  return (
    <nav className="sidebar" style={tint} onPointerDown={onPointerDown}>
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
      {/* Kept empty without a selected tab, so the sections below do not move. */}
      <div className="url">
        {active?.url}
        {active && page?.loading && (
          <progress className="progress" aria-label="Loading" value={page.progress} />
        )}
      </div>
      {/* An empty grid shows only while a tab is dragged, to drop it there. */}
      {state && (state.favorites.length > 0 || dragging === "tab") && (
        <ul className="favorites" aria-label="Favorites" data-parent={state.roots.favorites}>
          {state.favorites.map((tab) => (
            <Favorite
              key={tab.id}
              id={tab.id}
              title={tab.title || tab.url}
              favicon={tab.favicon}
              active={tab.id === state.active}
            />
          ))}
        </ul>
      )}
      <div className="scroll">
        <ul className="tabs pinned" data-parent={state?.roots.pinned}>
          <Items
            items={state?.pinned ?? []}
            active={state?.active}
            renaming={renaming}
            onRename={setRenaming}
          />
        </ul>
        <div className="divider-row" data-parent={state?.roots.pinned}>
          <hr className="divider" />
          <button
            type="button"
            className="new-folder"
            aria-label="New folder"
            title="New folder"
            onClick={() => send("folder/new")}
          >
            +
          </button>
        </div>
        <button type="button" className="tab" onClick={() => send("overlay/show")}>
          <span className="favicon">+</span>
          <span className="title">New tab</span>
        </button>
        <ul className="tabs today" data-parent={state?.roots.today}>
          <Items
            items={state?.today ?? []}
            active={state?.active}
            renaming={renaming}
            onRename={setRenaming}
          />
        </ul>
      </div>
      <div className="switcher">
        <Spaces spaces={state?.spaces ?? []} displayed={state?.space} />
        <Profiles profiles={state?.profiles ?? []} />
      </div>
      <div className="drop-indicator" ref={indicator} />
    </nav>
  );
}

function useSwipes() {
  useEffect(() => {
    const track = swipeTracker();
    const listener = (event: Event) => {
      const command = track((event as CustomEvent<Gesture>).detail);
      if (command) send(command);
    };
    addEventListener("arbalette:swipe", listener);
    return () => removeEventListener("arbalette:swipe", listener);
  }, []);
}

function tabs(items: Item[]): Tab[] {
  return items.flatMap((item) => ("children" in item ? tabs(item.children) : [item]));
}

type Rename = (folder: number | null) => void;

function Items({
  items,
  active,
  renaming,
  onRename,
}: {
  items: Item[];
  active?: number | null;
  renaming: number | null;
  onRename: Rename;
}) {
  return items.map((item) =>
    "children" in item ? (
      <li key={item.id} data-id={item.id}>
        <FolderRow
          id={item.id}
          title={item.title}
          expanded={item.expanded}
          renaming={item.id === renaming}
          onRename={onRename}
        />
        {item.expanded && (
          <ul className="folder-tabs" data-parent={item.id}>
            <Items items={item.children} active={active} renaming={renaming} onRename={onRename} />
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

// How long a click waits for a second one, which would make it a double click.
export const DOUBLE_CLICK_MS = 250;

// A click collapses or expands the folder, a double click edits its title. Enter and leaving the
// field save the title, unless it is blank; Escape keeps the old one.
const FolderRow = memo(function FolderRow({
  id,
  title,
  expanded,
  renaming,
  onRename,
}: {
  id: number;
  title: string;
  expanded: boolean;
  renaming: boolean;
  onRename: Rename;
}) {
  const arrow = (
    <span className="favicon" aria-hidden="true">
      {expanded ? "▾" : "▸"}
    </span>
  );
  const toggle = useRef<ReturnType<typeof setTimeout>>(undefined);
  if (!renaming) {
    return (
      <button
        type="button"
        className="folder"
        aria-expanded={expanded}
        // A mouse click waits to know it does not start a double click, so renaming does not
        // collapse or expand the folder; the keyboard toggles at once.
        onClick={(event) => {
          clearTimeout(toggle.current);
          if (event.detail > 1) return;
          const run = () => send(`toggle/${id}`);
          if (event.detail === 0) run();
          else toggle.current = setTimeout(run, DOUBLE_CLICK_MS);
        }}
        onDoubleClick={() => {
          clearTimeout(toggle.current);
          onRename(id);
        }}
      >
        {arrow}
        <span className="title">{title}</span>
      </button>
    );
  }
  const keyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") event.currentTarget.value = title;
    if (event.key === "Escape" || event.key === "Enter") event.currentTarget.blur();
  };
  return (
    <div className="folder">
      {arrow}
      <input
        className="title"
        aria-label="Folder name"
        defaultValue={title}
        // biome-ignore lint/a11y/noAutofocus: the field appears because the user asked to rename.
        autoFocus
        onFocus={(event) => event.currentTarget.select()}
        onKeyDown={keyDown}
        onBlur={(event) => {
          const value = event.currentTarget.value.trim();
          if (value && value !== title) send(`rename/${id}/${encodeURIComponent(value)}`);
          onRename(null);
        }}
      />
    </div>
  );
});

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
    <li className="tab-row" data-id={id}>
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

// Favorites are not closed from the grid: closing the selected one only deselects it.
const Favorite = memo(function Favorite({
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
    <li data-id={id}>
      <button
        type="button"
        className="favorite"
        aria-label={title}
        title={title}
        aria-current={active ? "page" : undefined}
        onClick={() => send(`select/${id}`)}
      >
        {favicon ? <img className="favicon" src={favicon} alt="" /> : <span className="favicon" />}
      </button>
    </li>
  );
});
