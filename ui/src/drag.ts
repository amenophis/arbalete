import { type PointerEvent as ReactPointerEvent, useRef } from "react";
import { send } from "./bridge";

// Where a dragged node lands: an index among a container's or a folder's children.
export type Place = { parent: number; index: number };

// The end of a folder, whose children may be hidden: the app clamps the index.
export const END = Number.MAX_SAFE_INTEGER;

// How far the pointer moves before a press becomes a drag, so clicks still select.
const THRESHOLD = 4;

// Where a node dropped on a row lands, the pointer at `fraction` of the row's height (or width
// for a favorite tile): before or after a tab, before a folder or at the end of it.
export function rowPlace(parent: number, index: number, fraction: number, folder?: number): Place {
  if (folder !== undefined && fraction >= 0.25) return { parent: folder, index: END };
  return { parent, index: fraction < 0.5 ? index : index + 1 };
}

// The command that moves node `id` from `from` to `to`, or null when it would stay in place. The
// app counts the index once the node has left its parent.
export function moveCommand(id: number, from: Place, to: Place): string | null {
  const index = from.parent === to.parent && from.index < to.index ? to.index - 1 : to.index;
  if (from.parent === to.parent && index === from.index) return null;
  return `move/${id}/${to.parent}/${index}`;
}

type Kind = "tab" | "folder";

// Rows are `li[data-id]` in lists with `data-parent`. A press on a row that moves a few pixels
// drags it: the row dims and `indicator`, a fixed element, shows where it would land. The drop
// sends one move command; the app answers with the new state.
export function useDrag() {
  const indicator = useRef<HTMLDivElement>(null);
  const onPointerDown = (event: ReactPointerEvent) => {
    const target = event.target as Element;
    if (event.button !== 0 || target.closest(".close, input")) return;
    const item = target
      .closest(".tab-row, .folder, .favorite")
      ?.closest<HTMLElement>("li[data-id]");
    if (!item) return;
    const kind = item.querySelector(":scope > .folder") ? "folder" : "tab";
    const id = Number(item.dataset.id);
    const from = placeOf(item);
    const start = { x: event.clientX, y: event.clientY };
    let started = false;
    let to: Place | null = null;
    const move = (event: PointerEvent) => {
      if (!started) {
        if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < THRESHOLD) return;
        started = true;
        item.classList.add("dragged");
      }
      to = locate(event.clientX, event.clientY, item, kind, indicator.current);
    };
    const end = (event: PointerEvent) => {
      removeEventListener("pointermove", move);
      removeEventListener("pointerup", end);
      removeEventListener("pointercancel", end);
      if (!started) return;
      item.classList.remove("dragged");
      indicator.current?.removeAttribute("style");
      swallowClick();
      const command = event.type === "pointerup" && to && moveCommand(id, from, to);
      if (command) send(command);
    };
    addEventListener("pointermove", move);
    addEventListener("pointerup", end);
    addEventListener("pointercancel", end);
  };
  return { indicator, onPointerDown };
}

function placeOf(item: HTMLElement): Place {
  const list = item.parentElement as HTMLElement;
  return { parent: Number(list.dataset.parent), index: [...list.children].indexOf(item) };
}

// The click that ends a drag must not select the tab or toggle the folder.
function swallowClick() {
  const swallow = (event: Event) => event.stopPropagation();
  addEventListener("click", swallow, { capture: true, once: true });
  setTimeout(() => removeEventListener("click", swallow, { capture: true }));
}

// The place under the pointer, shown by the indicator: a line between rows or tiles, or a box
// around the folder or the section it goes into.
function locate(
  x: number,
  y: number,
  dragged: HTMLElement,
  kind: Kind,
  indicator: HTMLDivElement | null,
): Place | null {
  const target = document.elementFromPoint(x, y);
  const list = target?.closest<HTMLElement>("[data-parent]");
  const place =
    target &&
    list &&
    !dragged.contains(target) &&
    !(kind === "folder" && list.matches(".favorites"))
      ? placeAt(target, list, x, y)
      : null;
  if (indicator) {
    indicator.style.cssText = place
      ? `display: block; left: ${place.box.left}px; top: ${place.box.top}px; width: ${place.box.width}px; height: ${place.box.height}px`
      : "";
    indicator.classList.toggle("into", place?.into ?? false);
  }
  return place && { parent: place.parent, index: place.index };
}

type Box = { left: number; top: number; width: number; height: number };

function placeAt(target: Element, list: HTMLElement, x: number, y: number) {
  const parent = Number(list.dataset.parent);
  const row = target.closest(".tab-row, .folder, .favorite");
  const item = row?.closest<HTMLElement>("li[data-id]");
  if (!row || !item || item.parentElement !== list) {
    // An empty area of a section, or the divider: the end of the section, under its last row.
    const rect = list.getBoundingClientRect();
    const last =
      list.matches("ul:not(.favorites)") && list.lastElementChild?.getBoundingClientRect();
    if (!last) return { parent, index: END, into: true, box: box(rect) };
    const line = { left: last.left, top: last.bottom - 1, width: last.width, height: 2 };
    return { parent, index: END, into: false, box: line };
  }
  const rect = row.getBoundingClientRect();
  const index = [...list.children].indexOf(item);
  const tile = list.matches(".favorites");
  const fraction = tile ? (x - rect.left) / rect.width : (y - rect.top) / rect.height;
  const folder = row.matches(".folder") ? Number(item.dataset.id) : undefined;
  const place = rowPlace(parent, index, fraction, folder);
  if (place.parent !== parent) return { ...place, into: true, box: box(rect) };
  const after = place.index > index;
  const line: Box = tile
    ? { left: (after ? rect.right : rect.left) - 1, top: rect.top, width: 2, height: rect.height }
    : { left: rect.left, top: (after ? rect.bottom : rect.top) - 1, width: rect.width, height: 2 };
  return { ...place, into: false, box: line };
}

function box(rect: DOMRect): Box {
  return { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
}
