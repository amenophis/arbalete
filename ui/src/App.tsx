import { type KeyboardEvent, memo, useRef } from "react";
import { type Item, type SpacePage, send, type Tab, useAppState, useRenaming } from "./bridge";
import { type Dragging, useDrag } from "./drag";
import { Library } from "./Library";
import { Profiles } from "./Profiles";
import { usePager } from "./pager";
import { NewSpace, Spaces } from "./Spaces";

export function App() {
  const state = useAppState();
  const [renaming, setRenaming] = useRenaming("arbalette:rename");
  const { dragging, indicator, onPointerDown } = useDrag();
  const displayed = state ? state.spaces.findIndex((space) => space.id === state.space) : 0;
  // The displayed Space's color tints the sidebar, and blends into the next one's on the way.
  const pager = usePager(state?.spaces ?? [], displayed);
  const count = state?.spaces.length ?? 0;
  // The pages may rest on one that is not a Space's, while the app still displays a Space.
  const onSpace = pager.settled >= 0 && pager.settled < count;
  const page = state?.page;
  const current = state?.pages.find((spacePage) => spacePage.space === state.space);
  const active =
    current &&
    [...current.favorites, ...tabs([...current.pinned, ...current.today])].find(
      (tab) => tab.id === current.selected,
    );
  return (
    <nav className="sidebar" ref={pager.sidebar} onPointerDown={onPointerDown}>
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
      {/* The app sends the displayed Space's page and its neighbors, the ones a swipe shows. */}
      <div className="pager">
        <div className="pages" ref={pager.track}>
          {state?.pages.map((spacePage) => {
            const index = state.spaces.findIndex((space) => space.id === spacePage.space);
            return (
              <Sections
                key={spacePage.space}
                page={spacePage}
                index={index}
                displayed={onSpace && index === displayed}
                dragging={dragging}
                renaming={renaming}
                onRename={setRenaming}
              />
            );
          })}
          {state && (
            <Library
              closed={state.closed}
              shown={pager.settled === -1}
              onReopen={() => pager.go(displayed)}
            />
          )}
          {state && <NewSpace index={count} shown={pager.settled === count} />}
        </div>
      </div>
      <div className="switcher">
        <Spaces
          spaces={state?.spaces ?? []}
          displayed={onSpace ? state?.space : undefined}
          onPage={pager.go}
        />
        <Profiles profiles={state?.profiles ?? []} />
      </div>
      <div className="drop-indicator" ref={indicator} />
      {/* In place of the page's card, which the app leaves empty without a selected tab. */}
      {current && current.selected === null && <div className="empty-space" />}
    </nav>
  );
}

// A Space's page: its profile's favorites, then its pinned and today's tabs, which scroll. Pages
// besides the displayed one stay out of reach of the keyboard and assistive technologies.
function Sections({
  page,
  index,
  displayed,
  dragging,
  renaming,
  onRename,
}: {
  page: SpacePage;
  index: number;
  displayed: boolean;
  dragging: Dragging;
  renaming: number | null;
  onRename: Rename;
}) {
  const { roots, favorites, selected } = page;
  return (
    <section
      className="space-page"
      data-space={page.space}
      inert={!displayed}
      style={{ left: `${index * 100}%` }}
    >
      {/* An empty grid shows only while a tab is dragged, to drop it there. */}
      {(favorites.length > 0 || dragging === "tab") && (
        <ul className="favorites" aria-label="Favorites" data-parent={roots.favorites}>
          {favorites.map((tab) => (
            <Favorite
              key={tab.id}
              id={tab.id}
              title={tab.title || tab.url}
              favicon={tab.favicon}
              active={tab.id === selected}
            />
          ))}
        </ul>
      )}
      <div className="scroll">
        <ul className="tabs pinned" data-parent={roots.pinned}>
          <Items items={page.pinned} active={selected} renaming={renaming} onRename={onRename} />
        </ul>
        <div className="divider-row" data-parent={roots.pinned}>
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
        <ul className="tabs today" data-parent={roots.today}>
          <Items items={page.today} active={selected} renaming={renaming} onRename={onRename} />
        </ul>
      </div>
    </section>
  );
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
