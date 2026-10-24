import { useEffect, useState } from "react";

// The app serves the UI and answers these requests itself (app/src/command.rs).
export function send(command: string) {
  void fetch(`/command/${command}`, { method: "POST" });
}

// Sent whole by the app on every change (UiState in app/src/shell.rs).
export type State = {
  page: Page;
  // The containers of the sections, to move tabs and folders into.
  roots: { favorites: number; pinned: number; today: number };
  // The favorites of the displayed Space's profile.
  favorites: Tab[];
  pinned: Item[];
  today: Item[];
  active: number | null;
  profiles: Profile[];
};

export type Profile = {
  id: number;
  name: string;
  color: Color;
};

// The palette of profiles (Color in crates/arbalette-core/src/sidebar.rs).
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

// The folder or the profile whose name is edited: one the app asks to rename, with `event`, once
// it creates it, or any the user picks.
export function useRenaming(event: "arbalette:rename" | "arbalette:rename-profile") {
  const [renaming, setRenaming] = useState<number | null>(null);
  useEffect(() => {
    const listener = (event: Event) => setRenaming((event as CustomEvent<number>).detail);
    addEventListener(event, listener);
    return () => removeEventListener(event, listener);
  }, [event]);
  return [renaming, setRenaming] as const;
}

// The sidebar shown above the page while the sidebar is hidden opens when the pointer reaches it,
// at the window's edge, and closes when the pointer leaves it (set_sidebar in app/src/shell.rs).
export function revealOnHover(element: HTMLElement) {
  element.addEventListener("mouseenter", () => send("sidebar/reveal"));
  element.addEventListener("mouseleave", () => send("sidebar/conceal"));
}
