import { activeTab, domain, NavButtons, SiteControls } from "./App";
import { send, useAppState } from "./bridge";
import { Icon } from "./Icon";
import { PinnedExtensions } from "./Site";

// The bar above the page in toolbar mode (View › Show Toolbar, toggle_toolbar in
// app/src/shell.rs), like Arc's: back, forward and reload at its left, the selected tab's domain
// in its middle, which a click edits in the Command Bar, as ⌘L does, its site controls at its right,
// and the page loading along its bottom.
export function Toolbar() {
  const state = useAppState();
  const page = state?.page;
  const active = state && activeTab(state);
  return (
    <div className="page-toolbar">
      <div className="page-toolbar-buttons">
        <NavButtons page={page} />
      </div>
      {active && (
        <button
          type="button"
          className="page-toolbar-url"
          title={active.url}
          onClick={() => send("overlay/edit")}
        >
          <Icon d="M6.8 9.2a2.6 2.6 0 0 0 3.7 0l2.2-2.2a2.6 2.6 0 0 0-3.7-3.7l-.9.9M9.2 6.8a2.6 2.6 0 0 0-3.7 0L3.3 9a2.6 2.6 0 0 0 3.7 3.7l.9-.9" />
          <span>{domain(active.url)}</span>
        </button>
      )}
      {active && (
        <div className="page-toolbar-buttons page-toolbar-end">
          <PinnedExtensions extensions={state.extensions} view="toolbar" />
          <SiteControls view="toolbar" />
        </div>
      )}
      {active && page?.loading && (
        <progress className="progress" aria-label="Loading" value={page.progress} />
      )}
    </div>
  );
}
