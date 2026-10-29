import { type MouseEvent, useEffect, useRef, useState } from "react";
import type { Extension } from "./Site";

// The app serves the UI and answers these requests itself (app/src/command.rs).
export function send(command: string) {
  void fetch(`/command/${command}`, { method: "POST" });
}

// Sent whole by the app on every change (UiState in app/src/shell.rs).
export type State = {
  page: Page;
  // The displayed Space, among every Space in sidebar order.
  space: number;
  spaces: Space[];
  // The pages of the displayed Space and its neighbors.
  pages: SpacePage[];
  profiles: Profile[];
  // The tabs closed since launch, the last closed first: the app lists the most recent ones.
  closed: ClosedTab[];
  // The bar above the page holds the navigation buttons and the address (View › Show Toolbar).
  toolbar: boolean;
  // The displayed Space's profile's pinned extensions, whose buttons show by the address.
  extensions: Extension[];
};

export type ClosedTab = {
  url: string;
  title: string;
  favicon: string | null;
  // When the tab closed, in milliseconds since the epoch.
  closedAt: number;
};

// What the sidebar shows of a Space: its tabs, and the favorites of its profile.
export type SpacePage = {
  space: number;
  // The tab the content view shows while the Space is displayed.
  selected: number | null;
  // Only the Space's header and its New Tab row show.
  collapsed: boolean;
  // The containers of the sections, to move tabs and folders into.
  roots: { favorites: number; pinned: number; today: number };
  favorites: Tab[];
  // The empty favorites show a box inviting to drag tabs there: the profile has not hidden it.
  favoritesHint: boolean;
  pinned: Item[];
  today: Item[];
};

export type Space = {
  id: number;
  title: string;
  emoji: string | null;
  color: Color;
  profile: number;
  // What the library shows of its pinned tabs: tabs, and folders without their tabs.
  pinned: Glance[];
};

export type Glance = Tab | { id: number; title: string; folder: true };

export type Profile = {
  id: number;
  name: string;
  color: Color;
};

// The palette of profiles and Spaces (Color in crates/arbalete-core/src/sidebar.rs).
export const COLORS = {
  purple: "#6e56cf",
  blue: "#3e7bfa",
  green: "#30a46c",
  yellow: "#e5b800",
  orange: "#f76b15",
  red: "#e5484d",
  pink: "#d6409f",
  gray: "#8b8d98",
};

export type Color = keyof typeof COLORS;

// `share` percent of `color` mixed into `base`, both #rrggbb, like CSS's color-mix in srgb.
export function mix(color: string, base: string, share: number): string {
  const channels = (hex: string) => [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
  const [from, to] = [channels(color), channels(base)];
  return `#${from
    .map((value, i) => Math.round((value * share + to[i] * (100 - share)) / 100))
    .map((value) => value.toString(16).padStart(2, "0"))
    .join("")}`;
}

// A Space's color, at a `strength` from 0, for a page that is no Space's, to 1: between two
// pages, the blend of theirs.
export type Tint = { color: string; strength: number };

// The command painting the window around the sidebar and the card, and an empty Space's card
// (WindowColors in app/src/command.rs): the end of the sidebar's gradient tinted as the sidebar
// tints it (index.css), and the panel's color tinted by the displayed Space's `card` color.
export function windowColors(tint: Tint, card: string, end: string, panel: string): string {
  const margins = mix(tint.color, end, 14 * tint.strength).slice(1);
  return `window/colors/${margins}/${mix(card, panel, 8).slice(1)}`;
}

// Paints the window with the colors last given, sending them only when they change, and again
// when the appearance changes the colors they derive from, read from index.css.
export class WindowPainter {
  private colors?: [Tint, string];
  private base?: [string, string];
  private sent = "";
  private readonly repaint = () => {
    this.base = undefined;
    this.send();
  };

  start() {
    window.matchMedia?.("(prefers-color-scheme: dark)").addEventListener?.("change", this.repaint);
  }

  stop() {
    window
      .matchMedia?.("(prefers-color-scheme: dark)")
      .removeEventListener?.("change", this.repaint);
  }

  paint(tint: Tint, card: string) {
    this.colors = [tint, card];
    this.send();
  }

  private send() {
    if (!this.colors) return;
    this.base ??= baseColors();
    // Without the stylesheet, as in tests, there is nothing to paint with.
    if (!this.base) return;
    const command = windowColors(...this.colors, ...this.base);
    if (command === this.sent) return;
    this.sent = command;
    send(command);
  }
}

function baseColors(): [string, string] | undefined {
  const style = getComputedStyle(document.documentElement);
  const end = style.getPropertyValue("--window-end").trim();
  const panel = style.getPropertyValue("--panel").trim();
  return end && panel ? [end, panel] : undefined;
}

export type Page = {
  progress: number;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
};

export type Item = Tab | Folder | Split;

// Two tabs in one row (NodeKind::Split): a click on either shows that tab alone, until Split View
// shows them side by side (docs/PLAN.md, phase 9).
export type Split = {
  id: number;
  split: Tab[];
};

export type Folder = {
  id: number;
  title: string;
  expanded: boolean;
  children: Item[];
};

export type Tab = {
  id: number;
  url: string;
  title: string;
  favicon: string | null;
  // Sidebar rows only: the tab plays no sound.
  muted?: boolean;
};

export function useAppState(): State | undefined {
  const [state, setState] = useState<State>();
  useEffect(() => {
    const listener = (event: Event) => setState((event as CustomEvent<State>).detail);
    addEventListener("arbalete:state", listener);
    send("ready");
    return () => removeEventListener("arbalete:state", listener);
  }, []);
  return state;
}

// The folder or profile whose name is edited: one the app asks to rename, with `event`, once it
// creates it, or any the user picks.
export function useRenaming(event: "arbalete:rename" | "arbalete:rename-profile") {
  const [renaming, setRenaming] = useState<number | null>(null);
  useEffect(() => {
    const listener = (event: Event) => setRenaming((event as CustomEvent<number>).detail);
    addEventListener(event, listener);
    return () => removeEventListener(event, listener);
  }, [event]);
  return [renaming, setRenaming] as const;
}

// Runs `handler` with the detail of every `event` the app dispatches, like an item of a native menu
// asking the sidebar to act (Action::Ask in app/src/menu.rs).
export function useAppEvent<T>(event: string, handler: (detail: T) => void) {
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(() => {
    const listener = (event: Event) => latest.current((event as CustomEvent<T>).detail);
    addEventListener(event, listener);
    return () => removeEventListener(event, listener);
  }, [event]);
}

// A right-click on a row or the header of the displayed Space's page asks the app for its native
// menu (app/src/menu.rs). Elsewhere, but in text fields, it shows no menu: the page's menu, with
// Back, Forward or View Page Source, has nothing to do with the sidebar. On macOS, real
// right-clicks reach no `contextmenu` listener: the app finds their target itself
// (ui/src/menuTargets.ts).
export function showMenu(event: MouseEvent) {
  const target = event.target as Element;
  if (target.closest("input, textarea")) return;
  event.preventDefault();
  const half = target.closest<HTMLElement>(".space-page .half[data-tab]");
  if (half) return send(`menu/node/${half.dataset.tab}`);
  const row = target.closest<HTMLElement>(".space-page li[data-id]");
  if (row) return send(`menu/node/${row.dataset.id}`);
  const page = target.closest(".space-header")?.closest<HTMLElement>(".space-page");
  if (page) send(`menu/space/${page.dataset.space}`);
}

const graphemes = new Intl.Segmenter();

// The first character as the user sees it: a whole emoji, not half of a surrogate pair.
export function firstGrapheme(text: string): string {
  return graphemes.segment(text)[Symbol.iterator]().next().value?.segment ?? "";
}

// The sidebar shown above the page while the sidebar is hidden opens when the pointer reaches it,
// at the window's edge, and closes when the pointer leaves it (set_sidebar in app/src/shell.rs).
// It floats a few points from the window's edge: the pointer that revealed it is left of it, and
// leaving towards the edge keeps it open.
export function revealOnHover(element: HTMLElement) {
  forgetHoverOnMove(element);
  element.addEventListener("mouseenter", () => send("sidebar/reveal"));
  element.addEventListener("mouseleave", (event) => {
    if (event.clientX > 0) send("sidebar/conceal");
    element.toggleAttribute("data-stale-hover", true);
  });
}

// A view hidden or moved under a resting pointer keeps the hover of the button the pointer last
// left there, the click that hid it say: until the pointer moves in it again, no hover shows.
function forgetHoverOnMove(root: HTMLElement) {
  const fresh = () => root.removeAttribute("data-stale-hover");
  addEventListener("pointermove", fresh, { passive: true });
  addEventListener("pointerdown", fresh, { passive: true });
}

// The docked sidebar's content slides out to the left as the sidebar hides, and back as it shows
// again, over its own background (index.css); it moves at once when it docks from above the page.
export function followDocking(root: HTMLElement) {
  forgetHoverOnMove(root);
  addEventListener("arbalete:docked", (event) => {
    const { docked, slide } = (event as CustomEvent<{ docked: boolean; slide: boolean }>).detail;
    root.toggleAttribute("data-instant", !slide);
    root.toggleAttribute("data-undocked", !docked);
    root.toggleAttribute("data-stale-hover", true);
  });
}
