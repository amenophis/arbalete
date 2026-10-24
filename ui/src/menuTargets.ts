import { send } from "./bridge";

// What a right-click on a part of the sidebar opens the menu of (Target in app/src/command.rs).
export type HitTarget = {
  target: { node: number } | { space: number };
  x: number;
  y: number;
  width: number;
  height: number;
};

// The rows and the Space header of the displayed Space's page, as much of them as shows in
// `viewport`. Real right-clicks reach no `contextmenu` listener on macOS: the app finds what is
// under the pointer among them and shows its menu at once, like a native view
// (menu_at in app/src/shell.rs).
export function menuTargets(root: ParentNode, viewport: DOMRect): HitTarget[] {
  const page = root.querySelector<HTMLElement>(".space-page:not([inert])");
  if (!page) return [];
  const scroll = page.querySelector(".scroll")?.getBoundingClientRect() ?? viewport;
  const targets: HitTarget[] = [];
  const add = (target: HitTarget["target"], element: Element | null, clip: DOMRect) => {
    if (!element) return;
    const rect = element.getBoundingClientRect();
    const left = Math.max(rect.left, clip.left, viewport.left);
    const top = Math.max(rect.top, clip.top, viewport.top);
    const right = Math.min(rect.right, clip.right, viewport.right);
    const bottom = Math.min(rect.bottom, clip.bottom, viewport.bottom);
    if (right <= left || bottom <= top) return;
    targets.push({ target, x: left, y: top, width: right - left, height: bottom - top });
  };
  for (const tile of page.querySelectorAll<HTMLElement>(".favorites li[data-id]")) {
    add({ node: Number(tile.dataset.id) }, tile, viewport);
  }
  add({ space: Number(page.dataset.space) }, page.querySelector(".space-header"), scroll);
  for (const row of page.querySelectorAll<HTMLElement>(".scroll li[data-id]")) {
    // A folder's own row, without the tabs it shows below.
    const element = row.classList.contains("folder-item")
      ? row.querySelector(":scope > .folder")
      : row;
    add({ node: Number(row.dataset.id) }, element, scroll);
  }
  return targets;
}

// Sends the app the targets of the sidebar `view` whenever they change, once a frame at most.
export function trackMenuTargets(root: HTMLElement, view: "docked" | "revealed") {
  let sent = "";
  let scheduled = false;
  const publish = () => {
    scheduled = false;
    const viewport = new DOMRect(0, 0, innerWidth, innerHeight);
    const json = JSON.stringify(menuTargets(root, viewport));
    if (json === sent) return;
    sent = json;
    send(`menu/targets/${view}/${encodeURIComponent(json)}`);
  };
  const schedule = () => {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(publish);
  };
  new MutationObserver(schedule).observe(root, {
    subtree: true,
    childList: true,
    attributes: true,
    characterData: true,
  });
  addEventListener("scroll", schedule, true);
  addEventListener("resize", schedule);
  root.addEventListener("transitionend", schedule);
  schedule();
}
