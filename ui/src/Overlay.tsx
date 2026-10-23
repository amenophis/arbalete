import { useEffect, useRef } from "react";
import { send } from "./bridge";

// Shown above the page by the app (show_overlay in app/src/shell.rs), to open a tab.
export function Overlay() {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const show = () => {
      if (!input.current) return;
      input.current.value = "";
      input.current.focus();
    };
    addEventListener("arbalette:show", show);
    return () => removeEventListener("arbalette:show", show);
  }, []);
  return (
    <form
      className="overlay"
      onSubmit={(event) => {
        event.preventDefault();
        const text = input.current?.value.trim();
        if (text) send(`open/${encodeURIComponent(toUrl(text))}`);
      }}
    >
      <input
        ref={input}
        aria-label="Open a URL"
        placeholder="Open a URL"
        onKeyDown={(event) => event.key === "Escape" && send("overlay/hide")}
        onBlur={() => send("overlay/hide")}
      />
    </form>
  );
}

// A URL without a scheme is a web address: "example.com" opens https://example.com.
export function toUrl(text: string) {
  return /^[a-z][a-z\d+.-]*:/i.test(text) ? text : `https://${text}`;
}
