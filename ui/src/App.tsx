import { type CSSProperties, type KeyboardEvent, memo, useEffect, useRef, useState } from "react";
import {
  type Item,
  type Space,
  type SpacePage,
  type Split,
  send,
  showMenu,
  type Tab,
  useAppEvent,
  useAppState,
  useRenaming,
} from "./bridge";
import { Downloads } from "./Downloads";
import { favoriteColumns, previewTiles, type TilePreview, useDrag } from "./drag";
import { useFaviconColor } from "./faviconColor";
import { Icon } from "./Icon";
import { Library, LibraryButton } from "./Library";
import { useListMotion } from "./motion";
import { NewMenu } from "./NewMenu";
import { Profiles } from "./Profiles";
import { usePager } from "./pager";
import { NewSpace, Spaces } from "./Spaces";

// The revealed sidebar taking the focus makes Chromium focus its first button as if tabbed to,
// with a ring: only the Tab key may leave a toolbar button focused.
let tabbed = false;
addEventListener("keydown", (event) => {
  tabbed = event.key === "Tab";
});

function unlessTabbed(event: { target: EventTarget }) {
  if (!tabbed && event.target instanceof HTMLElement) event.target.blur();
}

export function App() {
  const state = useAppState();
  const [renaming, setRenaming] = useRenaming("arbalete:rename");
  const { indicator, preview, full, onPointerDown } = useDrag(state);
  // After the drag's own clean-up, which puts the dropped row back in the list.
  useListMotion(state);
  const [profiles, setProfiles] = useState(false);
  const displayed = state ? state.spaces.findIndex((space) => space.id === state.space) : 0;
  // The displayed Space's color tints the sidebar and the window, and blends into the next one's
  // on the way.
  const pager = usePager(state?.spaces ?? [], displayed);
  const count = state?.spaces.length ?? 0;
  // The pages may rest on one that is not a Space's, while the app still displays a Space.
  const onSpace = pager.settled >= 0 && pager.settled < count;
  // A tab's menu moves it into a new Space from the page creating one.
  useAppEvent("arbalete:new-space", () => pager.go(count));
  // Spaces are managed from the library, for now.
  useAppEvent("arbalete:library", () => pager.go(-1));
  const page = state?.page;
  const current = state?.pages.find((spacePage) => spacePage.space === state.space);
  const active =
    current &&
    [...current.favorites, ...tabs([...current.pinned, ...current.today])].find(
      (tab) => tab.id === current.selected,
    );
  return (
    <nav
      className="sidebar"
      ref={pager.sidebar}
      onPointerDown={onPointerDown}
      onPointerMove={holdOption}
      onPointerLeave={releaseOption}
      onKeyDown={holdOption}
      onKeyUp={holdOption}
      onContextMenu={showMenu}
    >
      <div className="toolbar" role="toolbar" onFocus={unlessTabbed}>
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
        {active && <CopyLink />}
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
                preview={preview}
                full={full === spacePage.roots.favorites}
              />
            );
          })}
          {state && (
            <Library
              closed={state.closed}
              spaces={state.spaces}
              profiles={state.profiles}
              shown={pager.settled === -1}
              onBack={() => pager.go(displayed)}
              onShow={pager.go}
              onNewSpace={() => pager.go(count)}
            />
          )}
          {state && <NewSpace index={count} shown={pager.settled === count} />}
        </div>
      </div>
      <Downloads />
      <div className="switcher">
        <LibraryButton
          closed={state?.closed ?? []}
          onOpen={() => pager.go(-1)}
          onReopen={() => pager.go(displayed)}
        />
        <Spaces
          spaces={state?.spaces ?? []}
          displayed={onSpace ? state?.space : undefined}
          onPage={pager.go}
        />
        <NewMenu onNewSpace={() => pager.go(count)} onProfiles={() => setProfiles(true)} />
        <Profiles profiles={state?.profiles ?? []} open={profiles} onOpen={setProfiles} />
      </div>
      <div className="drop-indicator" ref={indicator} />
      {/* Pulled out past the last Space, it opens the page creating one once armed. */}
      <div className="new-bubble" ref={pager.bubble} aria-hidden="true">
        <span>+</span>
      </div>
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
  preview,
  full,
}: {
  page: SpacePage;
  space: Space;
  index: number;
  displayed: boolean;
  renaming: number | null;
  onRename: Rename;
  preview: TilePreview | null;
  // A tab dragged over the favorites finds them full.
  full: boolean;
}) {
  const { roots, selected } = page;
  // While a tile or a row is dragged over them, the favorites open its slot.
  const favorites = previewTiles(page.favorites, roots.favorites, preview);
  return (
    <section
      className="space-page"
      data-space={page.space}
      inert={!displayed}
      style={{ left: `${index * 100}%` }}
    >
      {/* Without favorites, a box invites to drag tabs there, until it is dismissed. */}
      <ul
        className="favorites"
        aria-label="Favorites"
        data-parent={roots.favorites}
        style={{ "--columns": favoriteColumns(favorites.length) } as CSSProperties}
      >
        {favorites.length === 0 && page.favoritesHint && <FavoritesHint space={page.space} />}
        {full && (
          <li className="favorites-full" role="status">
            Favorites are full
          </li>
        )}
        {favorites.map((tab) =>
          tab ? (
            <Favorite
              key={tab.id}
              id={tab.id}
              title={tab.title || tab.url}
              favicon={tab.favicon}
              active={tab.id === selected}
            />
          ) : (
            <li key="slot" className="favorite-slot" />
          ),
        )}
      </ul>
      <div className="scroll">
        {/* A click collapses or expands the Space; hovered, its icon shows which. */}
        <h2 className="space-header">
          <button
            type="button"
            className="space-toggle"
            aria-expanded={!page.collapsed}
            onClick={() => send(`space/toggle/${page.space}`)}
          >
            <span className="favicon" aria-hidden="true">
              <span className="space-icon">{space.emoji ?? <span className="space-dot" />}</span>
              <span className="chevron">
                <Icon d="M6 3.5 10.5 8 6 12.5" />
              </span>
            </span>
            <span className="title">{space.title}</span>
          </button>
          <button
            type="button"
            className="space-more"
            aria-label={`${space.title} options`}
            title="Space options"
            onClick={() => send(`menu/space/${page.space}`)}
          >
            <Icon d="M4 8h.01M8 8h.01M12 8h.01" />
          </button>
        </h2>
        {!page.collapsed && (
          <>
            <ul className="tabs pinned" data-parent={roots.pinned}>
              <Items
                items={page.pinned}
                active={selected}
                renaming={renaming}
                onRename={onRename}
              />
            </ul>
            {/* Hovered, the divider offers to close today's tabs, which may be reopened. */}
            <div className="divider-row" data-parent={roots.pinned}>
              <hr className="divider" />
              {page.today.length > 0 && (
                <button
                  type="button"
                  className="clear-today"
                  title="Close today's tabs"
                  onClick={() => send("today/clear")}
                >
                  ↓ Clear
                </button>
              )}
            </div>
          </>
        )}
        <button type="button" className="tab" onClick={() => send("overlay/show")}>
          <span className="favicon">+</span>
          <span className="title">New tab</span>
        </button>
        {!page.collapsed && (
          <ul className="tabs today" data-parent={roots.today}>
            <Items items={page.today} active={selected} renaming={renaming} onRename={onRename} />
          </ul>
        )}
      </div>
    </section>
  );
}

// Option held over the sidebar shows what it changes, like a split row's separate buttons
// (index.css). The page hears keys only while focused: the pointer's events say it too, and
// leaving the sidebar forgets it, so that it never sticks.
function holdOption(event: { altKey: boolean; currentTarget: Element }) {
  event.currentTarget.toggleAttribute("data-option", event.altKey);
}

function releaseOption(event: { currentTarget: Element }) {
  event.currentTarget.removeAttribute("data-option");
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
  return items.flatMap((item) => {
    if ("children" in item) return tabs(item.children);
    return "split" in item ? item.split : [item];
  });
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
    "split" in item ? (
      <SplitItem key={item.id} split={item} active={active} />
    ) : "children" in item ? (
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
        <TabIcon favicon={favicon} />
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

// A split's row holds a pill per tab. Its tabs close from their menus or the keyboard, which
// leaves the other tab alone in the row. Holding Option over the row, each pill offers to take
// its tab out of the split, as the row after it.
function SplitItem({ split, active }: { split: Split; active?: number | null }) {
  return (
    <li className="split-row" data-id={split.id}>
      <div className="split">
        {/* Before the pills, which stay the row's first and last children. */}
        {split.split.map((tab) => (
          <button
            key={tab.id}
            type="button"
            className="separate"
            aria-label={`Separate ${tab.title || tab.url}`}
            title="Separate"
            onClick={() => send(`separate/${tab.id}`)}
          >
            <Icon d="M5.5 3.5v9M10.5 3.5v9M3.5 6l-2 2 2 2M12.5 6l2 2-2 2" size={14} />
          </button>
        ))}
        {split.split.map((tab) => (
          <Half
            key={tab.id}
            id={tab.id}
            title={tab.title || tab.url}
            favicon={tab.favicon}
            active={tab.id === active}
          />
        ))}
      </div>
    </li>
  );
}

const Half = memo(function Half({
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
    <button
      type="button"
      className="half"
      data-tab={id}
      title={title}
      aria-current={active ? "page" : undefined}
      onClick={() => send(`select/${id}`)}
    >
      <TabIcon favicon={favicon} />
      <span className="title">{title}</span>
    </button>
  );
});

function TabIcon({ favicon }: { favicon: string | null }) {
  return favicon ? (
    <img className="favicon" src={favicon} alt="" />
  ) : (
    <span className="favicon placeholder" />
  );
}

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
  // The selected tile takes its icon's color.
  const color = useFaviconColor(active ? favicon : null);
  return (
    <li data-id={id}>
      <button
        type="button"
        className="favorite"
        aria-label={title}
        title={title}
        aria-current={active ? "page" : undefined}
        style={color ? ({ "--tile-color": color } as CSSProperties) : undefined}
        onClick={() => send(`select/${id}`)}
      >
        {favicon ? <img className="favicon" src={favicon} alt="" /> : <span className="favicon" />}
      </button>
    </li>
  );
});

// The empty favorites' invitation to drag tabs there; its × hides it for the profile until it has
// favorites again.
function FavoritesHint({ space }: { space: number }) {
  return (
    <li className="favorites-empty">
      <svg viewBox="0 0 16 16" width="18" height="18" aria-hidden="true">
        <path
          d="m8 2.2 1.75 3.6 3.95.55-2.87 2.77.7 3.93L8 11.18l-3.53 1.87.7-3.93L2.3 6.35l3.95-.55z"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.3"
          strokeLinejoin="round"
        />
      </svg>
      <strong>Drag tabs here to keep them close</strong>
      <span>Favorites stay one click away in every Space of this profile</span>
      <button
        type="button"
        className="hint-close"
        aria-label="Hide this hint"
        title="Hide this hint"
        onClick={() => send(`favorites/hide-hint/${space}`)}
      >
        ×
      </button>
    </li>
  );
}

// How long the url bar says the link was copied.
export const COPIED_MS = 1200;

// Copies the selected tab's url, as its menu's Copy Link does, and says so for a moment.
function CopyLink() {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), COPIED_MS);
    return () => clearTimeout(timer);
  }, [copied]);
  return (
    <button
      type="button"
      className="copy-link"
      aria-label="Copy link"
      title="Copy link"
      data-copied={copied || undefined}
      onClick={() => {
        send("link/copy");
        setCopied(true);
      }}
    >
      {copied ? (
        <span role="status">Copied</span>
      ) : (
        <Icon d="M6.8 9.2a2.6 2.6 0 0 0 3.7 0l2.2-2.2a2.6 2.6 0 0 0-3.7-3.7l-.9.9M9.2 6.8a2.6 2.6 0 0 0-3.7 0L3.3 9a2.6 2.6 0 0 0 3.7 3.7l.9-.9" />
      )}
    </button>
  );
}

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
