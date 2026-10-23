import { send, useAppState } from "./bridge";

export function App() {
  const page = useAppState()?.page;
  return (
    <nav className="sidebar">
      <div className="toolbar">
        <button
          type="button"
          aria-label="Back"
          disabled={!page?.canGoBack}
          onClick={() => send("back")}
        >
          ‹
        </button>
        <button
          type="button"
          aria-label="Forward"
          disabled={!page?.canGoForward}
          onClick={() => send("forward")}
        >
          ›
        </button>
        <button type="button" aria-label="Reload" onClick={() => send("reload")}>
          ↻
        </button>
      </div>
      {page && (
        <div className="page">
          {page.favicon && <img className="favicon" src={page.favicon} alt="" />}
          <div className="page-text">
            <div className="title">{page.title || page.url}</div>
            <div className="url">{page.url}</div>
          </div>
        </div>
      )}
      {page?.loading && (
        <progress className="progress" aria-label="Loading" value={page.progress} />
      )}
    </nav>
  );
}
