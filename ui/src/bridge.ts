import { useEffect, useState } from "react";

// The app serves the UI and answers these requests itself (app/src/command.rs).
export function send(command: string) {
  void fetch(`/command/${command}`, { method: "POST" });
}

// Sent whole by the app on every change (UiState in app/src/shell.rs).
export type State = {
  page: Page;
  // The favorites of the displayed Space's profile.
  favorites: Tab[];
  pinned: Item[];
  today: Item[];
  active: number | null;
};

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

// The folder whose title is edited: one the app asks to rename once it creates it, or any the
// user picks.
export function useRenaming() {
  const [renaming, setRenaming] = useState<number | null>(null);
  useEffect(() => {
    const listener = (event: Event) => setRenaming((event as CustomEvent<number>).detail);
    addEventListener("arbalette:rename", listener);
    return () => removeEventListener("arbalette:rename", listener);
  }, []);
  return [renaming, setRenaming] as const;
}
