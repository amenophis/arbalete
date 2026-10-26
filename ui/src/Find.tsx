import { useEffect, useRef, useState } from "react";
import { send } from "./bridge";
import { Icon } from "./Icon";

type Found = { count: number; active: number };

// Shown above the page by the app (show_find in app/src/shell.rs), to find text in it. Each
// keystroke searches; Enter goes to the next match, Shift-Enter to the previous one. The first
// time, the app shows it before it loads: its field takes the focus once loaded. A new search has
// no active match until Enter.
export function Find() {
  const input = useRef<HTMLInputElement>(null);
  const [found, setFound] = useState<Found>();
  useEffect(() => {
    const show = (event: Event) => {
      if (!input.current) return;
      input.current.value = (event as CustomEvent<string>).detail;
      input.current.select();
      setFound(undefined);
    };
    const result = (event: Event) => setFound((event as CustomEvent<Found>).detail);
    addEventListener("arbalete:show", show);
    addEventListener("arbalete:found", result);
    return () => {
      removeEventListener("arbalete:show", show);
      removeEventListener("arbalete:found", result);
    };
  }, []);
  const find = (forward: boolean) => {
    const text = input.current?.value ?? "";
    if (!text) setFound(undefined);
    send(`find/${forward ? "forward" : "backward"}/${encodeURIComponent(text)}`);
  };
  return (
    <div className="find">
      <input
        ref={input}
        // biome-ignore lint/a11y/noAutofocus: the find bar shows to be typed in.
        autoFocus
        aria-label="Find in page"
        placeholder="Find in page"
        onChange={() => find(true)}
        onKeyDown={(event) => {
          if (event.key === "Enter") find(!event.shiftKey);
          if (event.key === "Escape") send("find/hide");
        }}
      />
      {found && (
        <span className="find-count">
          {found.count ? `${found.active || "–"}/${found.count}` : "No results"}
        </span>
      )}
      <button type="button" aria-label="Previous match" onClick={() => find(false)}>
        <Icon d="M4 10l4-4 4 4" />
      </button>
      <button type="button" aria-label="Next match" onClick={() => find(true)}>
        <Icon d="M4 6l4 4 4-4" />
      </button>
      <button type="button" aria-label="Close" onClick={() => send("find/hide")}>
        <Icon d="M4.5 4.5l7 7M11.5 4.5l-7 7" />
      </button>
    </div>
  );
}
