import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { send } from "./bridge";
import { useDismiss } from "./dismiss";
import { Icon } from "./Icon";

// The "+" at the right of the footer opens a menu above it, pointing at it, to create a Space or
// a folder, open a tab, or manage the profiles. The arrow keys move between its items; Escape, a
// press outside or picking an item closes it.
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
  const item = (label: string, icon: string, run: () => void, shortcut?: [string, string]) => (
    <button
      type="button"
      role="menuitem"
      aria-label={label}
      aria-keyshortcuts={shortcut?.[1]}
      onClick={() => {
        setOpen(false);
        run();
      }}
    >
      <Icon d={icon} />
      <span className="label">{label}</span>
      {shortcut && <kbd>{shortcut[0]}</kbd>}
    </button>
  );
  return (
    <div className="new-menu" ref={root}>
      {open && (
        <div className="menu" role="menu" aria-label="New" onKeyDown={keyDown}>
          {item("New Space", ICONS.space, onNewSpace)}
          {item("New Folder", ICONS.folder, () => send("folder/new"), ["⇧⌘N", "Meta+Shift+N"])}
          <hr />
          {item("New Tab", ICONS.tab, () => send("overlay/show"), ["⌘T", "Meta+T"])}
          <hr />
          {item("Profiles…", ICONS.profiles, onProfiles)}
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

// Line icons on the 16 px grid of Icon.
const ICONS = {
  // A rounded square holding a plus.
  space: "M2.5 4.5a2 2 0 0 1 2-2h7a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-7a2 2 0 0 1-2-2zM8 5.5v5M5.5 8h5",
  folder: "M2.5 4.5a1 1 0 0 1 1-1h2.8l1.4 1.4h4.8a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1z",
  // A window with its title bar.
  tab: "M2.5 4.5a1.5 1.5 0 0 1 1.5-1.5h8a1.5 1.5 0 0 1 1.5 1.5v7a1.5 1.5 0 0 1-1.5 1.5H4a1.5 1.5 0 0 1-1.5-1.5zM2.5 6h11",
  // A head and shoulders.
  profiles: "M8 8a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM3.5 13.5a4.5 4.5 0 0 1 9 0",
};
