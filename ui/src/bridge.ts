import { useEffect, useState } from "react";

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
};

export type ClosedTab = {
  url: string;
  title: string;
  favicon: string | null;
};

// What the sidebar shows of a Space: its tabs, and the favorites of its profile.
export type SpacePage = {
  space: number;
  // The tab the content view shows while the Space is displayed.
  selected: number | null;
  // The containers of the sections, to move tabs and folders into.
  roots: { favorites: number; pinned: number; today: number };
  favorites: Tab[];
  pinned: Item[];
  today: Item[];
};

export type Space = {
  id: number;
  title: string;
  emoji: string | null;
  color: Color;
};

export type Profile = {
  id: number;
  name: string;
  color: Color;
};

// The palette of profiles and Spaces (Color in crates/arbalette-core/src/sidebar.rs).
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

// The command painting the window around the sidebar and the card, and an empty Space's card
// (WindowColors in app/src/command.rs): the end of the sidebar's gradient and the panel's color,
// tinted by the Space's `color` as the sidebar tints them (index.css).
export function windowColors(color: string, end: string, panel: string): string {
  return `window/colors/${mix(color, end, 14).slice(1)}/${mix(color, panel, 8).slice(1)}`;
}

// Sends the window's colors for a Space's `color`, and again when the appearance changes the
// colors they derive from, read from index.css.
export function useWindowColors(color: string | undefined) {
  useEffect(() => {
    if (!color) return;
    const paint = () => {
      const style = getComputedStyle(document.documentElement);
      const end = style.getPropertyValue("--window-end").trim();
      const panel = style.getPropertyValue("--panel").trim();
      if (!end || !panel) return false;
      send(windowColors(color, end, panel));
      return true;
    };
    // Without the stylesheet, as in tests, there is nothing to paint with.
    if (!paint()) return;
    const scheme = matchMedia("(prefers-color-scheme: dark)");
    scheme.addEventListener("change", paint);
    return () => scheme.removeEventListener("change", paint);
  }, [color]);
}

export type Page = {
  progress: number;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
};

export type Item = Tab | Folder;

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
};

export function useAppState(): State | undefined {
  const [state, setState] = useState<State>();
  useEffect(() => {
    const listener = (event: Event) => setState((event as CustomEvent<State>).detail);
    addEventListener("arbalette:state", listener);
    send("ready");
    return () => removeEventListener("arbalette:state", listener);
  }, []);
  return state;
}

// The folder or profile whose name is edited: one the app asks to rename, with `event`, once it
// creates it, or any the user picks.
export function useRenaming(event: "arbalette:rename" | "arbalette:rename-profile") {
  const [renaming, setRenaming] = useState<number | null>(null);
  useEffect(() => {
    const listener = (event: Event) => setRenaming((event as CustomEvent<number>).detail);
    addEventListener(event, listener);
    return () => removeEventListener(event, listener);
  }, [event]);
  return [renaming, setRenaming] as const;
}

const graphemes = new Intl.Segmenter();

// The first character as the user sees it: a whole emoji, not half of a surrogate pair.
export function firstGrapheme(text: string): string {
  return graphemes.segment(text)[Symbol.iterator]().next().value?.segment ?? "";
}

// The sidebar shown above the page while the sidebar is hidden opens when the pointer reaches it,
// at the window's edge, and closes when the pointer leaves it (set_sidebar in app/src/shell.rs).
export function revealOnHover(element: HTMLElement) {
  element.addEventListener("mouseenter", () => send("sidebar/reveal"));
  element.addEventListener("mouseleave", () => send("sidebar/conceal"));
}
