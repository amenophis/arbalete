import { type CSSProperties, type KeyboardEvent, memo, useRef } from "react";
import {
  COLORS,
  type Item,
  type Space,
  type SpacePage,
  send,
  type Tab,
  useAppState,
  useRenaming,
  useWindowColors,
} from "./bridge";
import { useDrag } from "./drag";
import { Icon } from "./Icon";
import { Library } from "./Library";
import { Profiles } from "./Profiles";
import { usePager } from "./pager";
import { NewSpace, Spaces } from "./Spaces";

export function App() {
  const state = useAppState();
  const [renaming, setRenaming] = useRenaming("arbalette:rename");
  const { indicator, onPointerDown } = useDrag();
  const displayed = state ? state.spaces.findIndex((space) => space.id === state.space) : 0;
  // The displayed Space's color tints the sidebar, and blends into the next one's on the way.
  const pager = usePager(state?.spaces ?? [], displayed);
  const count = state?.spaces.length ?? 0;
  // The pages may rest on one that is not a Space's, while the app still displays a Space.
  const onSpace = pager.settled >= 0 && pager.settled < count;
  // Only the sidebar beside the page paints the window, not the one revealed above it.
  const space = state?.spaces[displayed];
  useWindowColors(space && location.hash === "" ? COLORS[space.color] : undefined);
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
          aria-label="Toggle sidebar"
          title="Toggle sidebar"
          onClick={() => send("sidebar/toggle")}
        >
          <Icon d="M2.5 4.5a2 2 0 0 1 2-2h7a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-7a2 2 0 0 1-2-2zM6.5 2.5v11" />
        </button>
        <span className="spacer" />
        <button
          type="button"
          aria-label="Back"
          disabled={!page?.canGoBack}
          onClick={() => send("back")}
        >
          <Icon d="M10 3.5 5.5 8l4.5 4.5" />
        </button>
        <button
          type="button"
          aria-label="Forward"
          disabled={!page?.canGoForward}
          onClick={() => send("forward")}
        >
          <Icon d="M6 3.5 10.5 8 6 12.5" />
        </button>
        <button type="button" aria-label="Reload" onClick={() => send("reload")}>
          <Icon d="M12.8 8.5a4.8 4.8 0 1 1-1.4-3.9M11.9 2.2v2.7H9.2" />
        </button>
      </div>
      {/* Kept empty without a selected tab, so the sections below do not move. */}
      <div className="url" title={active?.url}>
        <span className="title">{active && domain(active.url)}</span>
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
                space={state.spaces[index]}
                index={index}
                displayed={onSpace && index === displayed}
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
    </nav>
  );
}

// A Space's page: its profile's favorites, then its pinned and today's tabs, which scroll. Pages
// besides the displayed one stay out of reach of the keyboard and assistive technologies.
function Sections({
  page,
  space,
  index,
  displayed,
  renaming,
  onRename,
}: {
  page: SpacePage;
  space: Space;
  index: number;
  displayed: boolean;
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
      {/* Without favorites, a placeholder takes their place and receives the dropped tab. */}
      <ul
        className="favorites"
        aria-label="Favorites"
        data-parent={roots.favorites}
        style={{ "--columns": Math.min(Math.max(favorites.length, 1), 4) } as CSSProperties}
      >
        {favorites.length === 0 && (
          <li className="favorites-empty">Drag tabs here to keep them close</li>
        )}
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
      <div className="scroll">
        <h2 className="space-header">
          <span className="favicon" aria-hidden="true">
            {space.emoji ?? <span className="space-dot" />}
          </span>
          <span className="title">{space.title}</span>
        </h2>
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

// What the url bar shows of a page's address: its host, without "www.", or the whole address
// when it has no host, like about:blank.
export function domain(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "") || url;
  } catch {
    return url;
  }
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
      <li key={item.id} data-id={item.id} className="folder-item">
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
  const arrow = <FolderIcon open={expanded} />;
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
        {favicon ? (
          <img className="favicon" src={favicon} alt="" />
        ) : (
          <span className="favicon placeholder" />
        )}
        <span className="title">{title}</span>
      </button>
      {/* On the selected row, closing may only deselect a pinned tab (close_tab in shell.rs). */}
      <button
        type="button"
        className="close"
        aria-label={`Close ${title}`}
        onClick={() => send(`close/${id}`)}
      >
        {active ? "—" : "×"}
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

// A folder, open while its tabs show.
function FolderIcon({ open }: { open: boolean }) {
  return (
    <svg
      className="favicon"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {open ? (
        <path d="M2.5 12V4.5a1 1 0 0 1 1-1h2.8l1.4 1.4h4.3a1 1 0 0 1 1 1V7M2.5 12l1.6-4.3a1 1 0 0 1 .9-.7h8.6a.6.6 0 0 1 .6.8l-1.5 3.9a1 1 0 0 1-.9.6H3.3a.8.8 0 0 1-.8-.3z" />
      ) : (
        <path d="M2.5 4.5a1 1 0 0 1 1-1h2.8l1.4 1.4h4.8a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1z" />
      )}
    </svg>
  );
}
