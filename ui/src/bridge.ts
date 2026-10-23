import { useEffect, useState } from "react";

// The app serves the UI and answers these requests itself (app/src/command.rs).
export function send(command: string) {
  void fetch(`/command/${command}`, { method: "POST" });
}

// Sent whole by the app on every change (UiState in app/src/shell.rs).
export type State = {
  page: Page;
};

export type Page = {
  url: string;
  title: string;
  favicon: string | null;
  progress: number;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
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
