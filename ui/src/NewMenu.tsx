import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { send } from "./bridge";
import { useDismiss } from "./dismiss";
import { Icon } from "./Icon";

// The "+" at the right of the footer opens a menu above it, to open a tab, create a folder or a
// Space, or manage the profiles. The arrow keys move between its items; Escape, a press outside
// or picking an item closes it.
export function NewMenu({
  onNewSpace,
  onProfiles,
}: {
  onNewSpace: () => void;
  onProfiles: () => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const toggle = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (open) root.current?.querySelector<HTMLElement>("[role=menuitem]")?.focus();
  }, [open]);
  useDismiss(root, open, () => {
    if (root.current?.contains(document.activeElement)) toggle.current?.focus();
    setOpen(false);
  });
  const keyDown = (event: KeyboardEvent) => {
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key];
    if (!step) return;
    event.preventDefault();
    const all = [...event.currentTarget.querySelectorAll<HTMLElement>("[role=menuitem]")];
    all.at((all.indexOf(document.activeElement as HTMLElement) + step) % all.length)?.focus();
  };
  const item = (label: string, run: () => void) => (
    <button
      type="button"
      role="menuitem"
      onClick={() => {
        setOpen(false);
        run();
      }}
    >
      {label}
    </button>
  );
  return (
    <div className="new-menu" ref={root}>
      {open && (
        <div className="menu" role="menu" aria-label="New" onKeyDown={keyDown}>
          {item("New Tab", () => send("overlay/show"))}
          {item("New Folder", () => send("folder/new"))}
          {item("New Space", onNewSpace)}
          {item("Profiles…", onProfiles)}
        </div>
      )}
      <button
        type="button"
        className="footer-button new-toggle"
        ref={toggle}
        aria-label="New"
        title="New"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <Icon d="M8 3v10M3 8h10" />
      </button>
    </div>
  );
}
