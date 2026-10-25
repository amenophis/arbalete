import { useEffect, useLayoutEffect, useRef } from "react";

// How long the rows below slide by, in ms, unless motion is reduced: a folder opening or closing,
// a row inserted, a row removed, and rows moved around.
export const FOLDER = 250;
export const INSERT = 125;
export const REMOVE = 110;
export const MOVE = 200;
// How long the tabs of a split separated show as rows.
export const SEPARATE = 50;
export const ROW_SLIDE = `${MOVE}ms ease-in-out`;

// The rows of a page, in the order they show, which folders show their tabs and which rows are
// splits, by node id.
export type Layout = { ids: string[]; expanded: string[]; splits: string[] };

// How a change of rows moves: how long the rows that stay slide and the rows that appear fade
// in, in ms; null when no row moves.
export type Motion = { slide: number; fade: number };

export function motionOf(before: Layout, after: Layout): Motion | null {
  if (before.expanded.join() !== after.expanded.join()) return { slide: FOLDER, fade: FOLDER };
  const was = new Set(before.ids);
  const is = new Set(after.ids);
  const added = after.ids.some((id) => !was.has(id));
  const removed = before.ids.filter((id) => !is.has(id));
  if (added && removed.some((id) => before.splits.includes(id))) {
    return { slide: INSERT, fade: SEPARATE };
  }
  if (added) return { slide: INSERT, fade: INSERT };
  if (removed.length > 0) return { slide: REMOVE, fade: 0 };
  return before.ids.join() === after.ids.join() ? null : { slide: MOVE, fade: 0 };
}

// The rows that slide, by node id: a folder's own row, without the tabs below it.
export function rowElements(root: ParentNode): Map<string, HTMLElement> {
  const rows = new Map<string, HTMLElement>();
  for (const item of root.querySelectorAll<HTMLElement>("li[data-id]")) {
    const row = item.classList.contains("folder-item")
      ? item.querySelector<HTMLElement>(":scope > .folder")
      : item;
    if (row) rows.set(item.dataset.id as string, row);
  }
  return rows;
}

export function rowRects(root: ParentNode): Map<string, DOMRect> {
  const rects = new Map<string, DOMRect>();
  for (const [id, row] of rowElements(root)) rects.set(id, row.getBoundingClientRect());
  return rects;
}

function layoutOf(rows: Map<string, HTMLElement>): Layout {
  const ids = [...rows.keys()];
  const expanded = ids.filter((id) => rows.get(id)?.getAttribute("aria-expanded") === "true");
  const splits = ids.filter((id) => rows.get(id)?.classList.contains("split-row"));
  return { ids, expanded, splits };
}

function reduced(): boolean {
  return !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

// Slides the rows of `root` from where they were, `before`, to where they are now, with the
// transform alone (FLIP); the lifted tile follows the pointer instead. Rows that were not there
// fade in over `fade` ms.
export function slideFrom(
  before: Map<string, DOMRect>,
  root: ParentNode,
  timing: string,
  fade = 0,
) {
  if (reduced()) return;
  const rows = [...rowElements(root)].filter(([, row]) => !row.classList.contains("lifted"));
  for (const [, row] of rows) {
    row.style.transition = "";
    row.style.transform = "";
  }
  const moves: [HTMLElement, number, number][] = [];
  for (const [id, row] of rows) {
    const from = before.get(id);
    if (!from) {
      // Without animations, as in jsdom, rows just appear.
      if (fade > 0) row.animate?.({ opacity: [0, 1] }, { duration: fade, easing: "ease-in-out" });
      continue;
    }
    const to = row.getBoundingClientRect();
    const [dx, dy] = [from.left - to.left, from.top - to.top];
    if (Math.abs(dx) >= 0.5 || Math.abs(dy) >= 0.5) moves.push([row, dx, dy]);
  }
  if (moves.length === 0) return;
  for (const [row, dx, dy] of moves) {
    row.style.transition = "none";
    row.style.transform = `translate(${dx}px, ${dy}px)`;
  }
  // The rows must show at their old place before they transition to the new one.
  document.body.getBoundingClientRect();
  for (const [row] of moves) {
    row.style.transition = `transform ${timing}`;
    row.style.transform = "";
    row.addEventListener("transitionend", () => row.style.removeProperty("transition"), {
      once: true,
    });
  }
}

// The rows a folder closing hides fade out where they were, over the rows sliding up.
function fadeOut(rows: [HTMLElement, DOMRect][], duration: number) {
  if (!document.body.animate) return;
  for (const [row, rect] of rows) {
    const layer = document.createElement("div");
    layer.className = "leaving";
    layer.style.cssText = `left: ${rect.left}px; top: ${rect.top}px; width: ${rect.width}px`;
    row.style.transform = "";
    layer.append(row);
    document.body.append(layer);
    const fading = layer.animate({ opacity: [1, 0] }, { duration, easing: "ease-in-out" });
    fading.onfinish = () => layer.remove();
  }
}

// Where a dropped row starts its slide from instead of its old place: where its ghost was.
const starts = new Map<string, DOMRect>();

export function startFrom(id: string, rect: DOMRect) {
  starts.set(id, rect);
}

type Snapshot = {
  state: unknown;
  space: string | undefined;
  rows: Map<string, HTMLElement>;
  rects: Map<string, DOMRect>;
  layout: Layout;
};

function displayedPage(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".space-page:not([inert])");
}

// The displayed Space's rows move to their place in each new state: when a folder opens or
// closes, a row comes or goes, or rows move around. Their places are read as the state arrives,
// before it renders; they slide from there once it has.
export function useListMotion(state: unknown) {
  const snapshot = useRef<Snapshot | null>(null);
  useEffect(() => {
    const take = (event: Event) => {
      const page = displayedPage();
      if (!page) return;
      const rows = rowElements(page);
      const rects = new Map<string, DOMRect>();
      for (const [id, row] of rows) rects.set(id, row.getBoundingClientRect());
      const { detail } = event as CustomEvent;
      const layout = layoutOf(rows);
      snapshot.current = { state: detail, space: page.dataset.space, rows, rects, layout };
    };
    // Before the state's own listener, which renders it.
    addEventListener("arbalete:state", take, true);
    return () => removeEventListener("arbalete:state", take, true);
  }, []);
  useLayoutEffect(() => {
    const before = snapshot.current;
    snapshot.current = null;
    const page = displayedPage();
    const dropped = new Map(starts);
    starts.clear();
    if (!before || !page || before.state !== state || reduced()) return;
    if (page.dataset.space !== before.space) return;
    const rows = rowElements(page);
    const motion = motionOf(before.layout, layoutOf(rows));
    if (!motion) return;
    for (const [id, rect] of dropped) before.rects.set(id, rect);
    slideFrom(before.rects, page, `${motion.slide}ms ease-in-out`, motion.fade);
    if (motion.slide !== FOLDER) return;
    const hidden = [...before.rows].filter(([id, row]) => !rows.has(id) && !row.isConnected);
    fadeOut(
      hidden.map(([id, row]) => [row, before.rects.get(id) as DOMRect]),
      FOLDER,
    );
  }, [state]);
}
