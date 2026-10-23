import { send } from "./bridge";

export function App() {
  return (
    <nav className="sidebar">
      <div className="toolbar">
        <button type="button" aria-label="Back" onClick={() => send("back")}>
          ‹
        </button>
        <button type="button" aria-label="Forward" onClick={() => send("forward")}>
          ›
        </button>
        <button type="button" aria-label="Reload" onClick={() => send("reload")}>
          ↻
        </button>
      </div>
    </nav>
  );
}
