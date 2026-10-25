import { type PointerEvent as ReactPointerEvent, useLayoutEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { send } from "./bridge";
import { rowRects, slideFrom, startFrom } from "./motion";

// Where a dragged node lands: an index among a container's or a folder's children.
export type Place = { parent: number; index: number };

// The end of a section, below its last row: the app clamps the index.
export const END = Number.MAX_SAFE_INTEGER;

// How far the pointer moves before a press becomes a drag, so clicks still select.
export const THRESHOLD = 4;

// How many favorites a profile holds at most (MAX_FAVORITES in crates/arbalete-core/src/sidebar.rs).
export const MAX_FAVORITES = 12;

// How long "Favorites are full" stays once the pointer leaves full favorites.
export const FULL_HINT_MS = 1500;

// A favorite tile's height and the gap between tiles (index.css).
const TILE = 47;
const GAP = 8;

// How long tiles slide to their new place, unless motion is reduced.
const TILE_SLIDE = "200ms ease-out";

// What is dragged: a tab row, a folder, a split row or a favorite tile.
export type Kind = "tab" | "folder" | "split" | "tile";

// Where on a row the pointer drops: before or after it, into a folder, or beside a tab, as a
// split.
export type Zone = "before" | "after" | "into" | "split";

// The zone of a row under the pointer, at `fraction` of the row's height. A tab dropped on the
// middle of another makes a split with it, its edges insert; a folder takes what is dropped on
// its lower half.
export function zoneAt(fraction: number, row: "tab" | "folder" | "split", dragged: Kind): Zone {
  if (row === "folder") return fraction < 0.5 ? "before" : "into";
  const tab = dragged === "tab" || dragged === "tile";
  if (row === "tab" && tab && fraction >= 0.25 && fraction < 0.75) return "split";
  return fraction < 0.5 ? "before" : "after";
}

// Where a node dropped in `zone` of the row at `index` lands: into a folder means first in it.
export function rowPlace(parent: number, index: number, zone: Zone, folder?: number): Place {
  if (zone === "into" && folder !== undefined) return { parent: folder, index: 0 };
  return { parent, index: zone === "after" ? index + 1 : index };
}

// The command that moves node `id` from `from` to `to`, or null when it would stay in place. The
// app counts the index once the node has left its parent.
export function moveCommand(id: number, from: Place, to: Place): string | null {
  const index = from.parent === to.parent && from.index < to.index ? to.index - 1 : to.index;
  if (from.parent === to.parent && index === from.index) return null;
  return `move/${id}/${to.parent}/${index}`;
}

// The favorites share rows of up to 4 tiles, as evenly as they can: 6 make two rows of 3, not 4
// and 2. Without favorites, the placeholder takes the whole row.
export function favoriteColumns(count: number): number {
  const rows = Math.max(1, Math.ceil(count / 4));
  return Math.max(1, Math.ceil(count / rows));
}

// The cell under the pointer in a grid of `count` tiles: where the dragged tile, or the slot a
// row opens, goes.
export function tileSlot(
  x: number,
  y: number,
  grid: { left: number; top: number; width: number },
  count: number,
): number {
  const columns = favoriteColumns(count);
  const column = Math.floor(((x - grid.left) / grid.width) * columns);
  const row = Math.max(0, Math.floor((y - grid.top) / (TILE + GAP)));
  return Math.min(row * columns + Math.min(Math.max(column, 0), columns - 1), count - 1);
}

// How the favorites show while a tile or a row is dragged over them: the dragged tile, or an
// empty slot for a row, at `slot` among the others.
export type TilePreview = { parent: number; slot: number; tile: number | null };

// A preview stays once dropped, until the state that follows: the app's answer.
type Preview = TilePreview & { dropped?: unknown };

// The tiles of the favorites `parent` holds as `preview` shows them; null is the empty slot.
export function previewTiles<T extends { id: number }>(
  tiles: T[],
  parent: number,
  preview: TilePreview | null,
): (T | null)[] {
  if (!preview || preview.parent !== parent) return tiles;
  const moved = preview.tile === null ? null : tiles.find((tile) => tile.id === preview.tile);
  if (moved === undefined) return tiles;
  const rest = tiles.filter((tile) => tile !== moved);
  return [...rest.slice(0, preview.slot), moved, ...rest.slice(preview.slot)];
}

type Drop = { place: Place | null; split: HTMLElement | null };

type Found = { place: Place; line: Line } | { split: HTMLElement };

// Rows are `li[data-id]` in lists with `data-parent`. A press that moves a few pixels drags a row
// or a tile. A row stays in place, dimmed, while a ghost of it follows the pointer and a line
// shows where it would land, or the row it would split with shows both tabs. A tile moves with
// the pointer itself, the other tiles sliding to open its slot; a row over the favorites becomes
// a tile ghost and opens a slot too, unless the favorites are full: `full` then names them, to say
// so, and the drop does nothing. Escape cancels. The drop sends one command; once the app answers
// with the new state, the rows that moved slide to their place (useListMotion), the dropped one
// from its ghost.
export function useDrag(state: unknown) {
  const indicator = useRef<HTMLDivElement>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [full, setFull] = useState<number | null>(null);
  const fullTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  // Set on a drop, until the state that follows.
  const dropped = useRef<unknown>(null);
  const latest = useRef(state);
  latest.current = state;

  useLayoutEffect(() => {
    if (dropped.current === null || dropped.current === state) return;
    dropped.current = null;
    for (const element of document.querySelectorAll<HTMLElement>(".dragged, .lifted")) {
      element.classList.remove("dragged", "lifted");
      element.style.transform = "";
    }
    setPreview(null);
  }, [state]);
  const shown = preview && (!("dropped" in preview) || preview.dropped === state) ? preview : null;

  const onPointerDown = (event: ReactPointerEvent) => {
    const target = event.target as Element;
    if (event.button !== 0 || target.closest(".close, input")) return;
    const item = target
      .closest(".tab-row, .folder, .favorite, .split-row")
      ?.closest<HTMLElement>("li[data-id]");
    const page = item?.closest<HTMLElement>(".space-page");
    const grid = page?.querySelector<HTMLElement>(".favorites");
    if (!item || !page || !grid) return;
    const kind = dragKind(item);
    const id = Number(item.dataset.id);
    const from = placeOf(item);
    const favorites = Number(grid.dataset.parent);
    const start = item.getBoundingClientRect();
    const grab = { x: event.clientX - start.left, y: event.clientY - start.top };
    let ghost: HTMLElement | null = null;
    let splitView: HTMLElement | null = null;
    let slot: number | null = null;
    // Where the lifted tile rests in the grid, which its transform starts from.
    let base = start;
    let drop: Drop = { place: null, split: null };
    let overFull = false;
    // Says the favorites are full while the pointer is over them, and a moment after.
    const flagFull = (over: boolean) => {
      if (over === overFull) return;
      overFull = over;
      clearTimeout(fullTimer.current);
      if (over) setFull(favorites);
      else fullTimer.current = setTimeout(() => setFull(null), FULL_HINT_MS);
    };
    const origin = { x: event.clientX, y: event.clientY };
    let pointer = origin;

    // The favorites as they show with the dragged tile or a row's slot at `next`, or as they
    // are with null, the others sliding to their new place.
    const showSlot = (next: number | null) => {
      if (next === slot) return;
      slot = next;
      const before = rowRects(grid);
      flushSync(() =>
        setPreview(
          next === null
            ? null
            : { parent: favorites, slot: next, tile: kind === "tile" ? id : null },
        ),
      );
      slideFrom(before, grid, TILE_SLIDE);
      if (kind === "tile") {
        item.style.transform = "";
        base = item.getBoundingClientRect();
      }
    };
    const follow = () => {
      if (kind === "tile") {
        const dx = pointer.x - grab.x - base.left;
        const dy = pointer.y - grab.y - base.top;
        item.style.transform = `translate(${dx}px, ${dy}px)`;
      } else if (ghost) {
        const tile = ghost.classList.contains("tile");
        const x = pointer.x - (tile ? TILE / 2 : grab.x);
        const y = pointer.y - (tile ? TILE / 2 : grab.y);
        ghost.style.transform = `translate(${x}px, ${y}px)`;
      }
    };
    const showSplit = (row: HTMLElement | null) => {
      if (splitView?.dataset.target === row?.dataset.id) return;
      splitView?.remove();
      for (const element of page.querySelectorAll(".split-target")) {
        element.classList.remove("split-target");
      }
      splitView = row && splitPreview(row, item);
      if (splitView) document.body.append(splitView);
      row?.classList.add("split-target");
      ghost?.classList.toggle("hidden", row !== null);
    };
    const locate = () => {
      const { x, y } = pointer;
      const rect = grid.getBoundingClientRect();
      const overGrid =
        kind !== "folder" &&
        kind !== "split" &&
        x >= rect.left - 4 &&
        x <= rect.right + 4 &&
        y >= rect.top - 4 &&
        y <= rect.bottom + 4;
      const tiles = grid.querySelectorAll("li[data-id]").length;
      if (overGrid && kind !== "tile" && tiles >= MAX_FAVORITES) {
        flagFull(true);
        showLine(indicator.current, null);
        showSplit(null);
        drop = { place: null, split: null };
        return;
      }
      flagFull(false);
      if (overGrid) {
        const count = tiles + (kind === "tile" ? 0 : 1);
        const next = tileSlot(x, y, rect, count);
        showSlot(next);
        ghost?.classList.add("tile");
        showLine(indicator.current, null);
        showSplit(null);
        const index = kind === "tile" && next >= from.index ? next + 1 : next;
        drop = { place: { parent: favorites, index }, split: null };
        return;
      }
      // A tile dragged away leaves its slot open in the grid, as a row it may come back to.
      showSlot(kind === "tile" ? from.index : null);
      ghost?.classList.remove("tile");
      const found = findPlace(x, y, page, item, kind);
      const split = found && "split" in found ? found.split : null;
      showLine(indicator.current, found && "line" in found ? found.line : null);
      showSplit(split);
      drop = { place: found && "place" in found ? found.place : null, split };
    };

    let started = false;
    const move = (event: PointerEvent) => {
      pointer = { x: event.clientX, y: event.clientY };
      if (!started) {
        if (Math.hypot(pointer.x - origin.x, pointer.y - origin.y) < THRESHOLD) return;
        started = true;
        addEventListener("keydown", onKey, true);
        if (kind === "tile") {
          item.classList.add("lifted");
          showSlot(from.index);
        } else {
          item.classList.add("dragged");
          ghost = ghostOf(item, start.width);
          document.body.append(ghost);
        }
      }
      locate();
      follow();
    };
    const finish = (commit: boolean) => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", up);
      removeEventListener("pointercancel", cancel);
      removeEventListener("keydown", onKey, true);
      if (!started) return;
      flagFull(false);
      const command = commit ? commandOf(id, from, drop) : null;
      const before = rowRects(page);
      // The dropped row slides from where its ghost was.
      const shown = ghost?.firstElementChild;
      if (command && shown && !ghost?.classList.contains("hidden")) {
        startFrom(String(id), shown.getBoundingClientRect());
      }
      ghost?.remove();
      showLine(indicator.current, null);
      showSplit(null);
      if (command) {
        // The tile settles in the slot it was dropped in, which the grid keeps until the app
        // answers with the moved tiles.
        if (kind === "tile" && drop.place?.parent === favorites) {
          item.classList.remove("lifted");
          item.style.transform = "";
          slideFrom(before, grid, TILE_SLIDE);
        }
        dropped.current = latest.current;
        setPreview((shown) => shown && { ...shown, dropped: latest.current });
        send(command);
        return;
      }
      item.classList.remove("dragged", "lifted");
      item.style.transform = "";
      showSlot(null);
      if (kind === "tile") slideFrom(before, grid, TILE_SLIDE);
    };
    const up = () => {
      if (started) swallowClick();
      finish(true);
    };
    const cancel = () => finish(false);
    // The press goes on after Escape: its release must not click the row either.
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      addEventListener("pointerup", swallowClick, { once: true });
      finish(false);
    };
    addEventListener("pointermove", move);
    addEventListener("pointerup", up);
    addEventListener("pointercancel", cancel);
  };
  return { indicator, preview: shown, full, onPointerDown };
}

function dragKind(item: HTMLElement): Kind {
  if (item.closest(".favorites")) return "tile";
  if (item.classList.contains("split-row")) return "split";
  return item.querySelector(":scope > .folder") ? "folder" : "tab";
}

function commandOf(id: number, from: Place, drop: Drop): string | null {
  if (drop.split) return `split/${id}/${drop.split.dataset.id}`;
  return drop.place && moveCommand(id, from, drop.place);
}

function placeOf(item: HTMLElement): Place {
  const list = item.parentElement as HTMLElement;
  return { parent: Number(list.dataset.parent), index: [...list.children].indexOf(item) };
}

// The click that ends a drag must not select the tab or toggle the folder.
export function swallowClick() {
  const swallow = (event: Event) => event.stopPropagation();
  addEventListener("click", swallow, { capture: true, once: true });
  setTimeout(() => removeEventListener("click", swallow, { capture: true }));
}

type Line = { left: number; top: number; width: number };

// The place in the lists under the pointer, with the line showing it, or the tab row a tab
// would make a split with.
function findPlace(
  x: number,
  y: number,
  page: HTMLElement,
  dragged: HTMLElement,
  kind: Kind,
): Found | null {
  const target = document.elementFromPoint(x, y);
  const list = target?.closest<HTMLElement>("[data-parent]");
  if (!target || !list || !page.contains(list) || list.matches(".favorites")) return null;
  if (dragged.contains(target)) return null;
  const parent = Number(list.dataset.parent);
  const row = target.closest<HTMLElement>(".tab-row, .split-row, .folder");
  const item = row?.closest<HTMLElement>("li[data-id]");
  if (!row || !item || item.parentElement !== list) {
    // An empty area of a section, or the divider: the end of the section, under its last row.
    const last = list.matches("ul") ? list.lastElementChild?.getBoundingClientRect() : undefined;
    const rect = list.getBoundingClientRect();
    const line = {
      left: last?.left ?? rect.left,
      top: last?.bottom ?? rect.top,
      width: last?.width ?? rect.width,
    };
    return { place: { parent, index: END }, line };
  }
  const rect = row.getBoundingClientRect();
  const type = row.matches(".folder") ? "folder" : row.matches(".tab-row") ? "tab" : "split";
  const zone = zoneAt((y - rect.top) / rect.height, type, kind);
  if (zone === "split") return { split: item };
  const index = [...list.children].indexOf(item);
  const place = rowPlace(
    parent,
    index,
    zone,
    type === "folder" ? Number(item.dataset.id) : undefined,
  );
  const indent = zone === "into" ? 12 : 0;
  const top = zone === "before" ? rect.top : rect.bottom;
  return { place, line: { left: rect.left + indent, top, width: rect.width - indent } };
}

// Shows the insertion line, centered on the gap between two rows, or hides it.
function showLine(indicator: HTMLElement | null, line: Line | null) {
  if (!indicator) return;
  indicator.style.cssText = line
    ? `display: block; left: ${line.left + 8}px; top: ${line.top - 0.75}px; width: ${line.width - 12}px`
    : "";
}

// A copy of the dragged row that follows the pointer, as wide as the row.
function ghostOf(item: HTMLElement, width: number): HTMLElement {
  const ghost = document.createElement("div");
  ghost.className = "drag-ghost";
  ghost.style.setProperty("--width", `${width}px`);
  const row = item.querySelector(":scope > .tab, :scope > .folder, :scope > .split") ?? item;
  const copy = row.cloneNode(true) as HTMLElement;
  copy.classList.add("ghost-pill");
  for (const element of [copy, ...copy.querySelectorAll("[aria-current]")]) {
    element.removeAttribute("aria-current");
  }
  ghost.append(copy);
  return ghost;
}

// The split a tab dropped on `row` would make, drawn over that row: its tab, then the dragged one.
function splitPreview(row: HTMLElement, dragged: HTMLElement): HTMLElement {
  const preview = document.createElement("div");
  preview.className = "split-preview";
  preview.dataset.target = row.dataset.id;
  const split = document.createElement("div");
  split.className = "split";
  for (const tab of [row, dragged]) {
    const half = document.createElement("div");
    half.className = "half";
    const icon = tab.querySelector(".favicon")?.cloneNode(true);
    const title = document.createElement("span");
    title.className = "title";
    title.textContent =
      tab.querySelector(".title")?.textContent ??
      tab.querySelector(".favorite")?.getAttribute("title") ??
      "";
    if (icon) half.append(icon);
    half.append(title);
    split.append(half);
  }
  preview.append(split);
  const rect = row.getBoundingClientRect();
  preview.style.cssText = `left: ${rect.left}px; top: ${rect.top}px; width: ${rect.width}px`;
  return preview;
}
